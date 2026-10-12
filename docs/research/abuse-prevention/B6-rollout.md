# B6 — Rollout sequencing, feature flags, shadow mode, rollback, migration safety on a live DB

Scope: DESIGN-v1 §B1–B7, answer to Q6. Every mechanism below is grounded in the repo as it
exists at HEAD, plus the round-3 findings it depends on (A3 #1/#13/#14, A6 #1/#7, A7 #1/#2/#12,
A9 #14, A10 #8/#11/#15).

The one fact that shapes everything: **there is no staging environment, the pipeline auto-deploys
every master merge, and the pipeline currently has zero test gates.** Shadow mode in production
_is_ the staging environment. Every step below is therefore designed so that the site stays up and
the only switch that can change behavior is one a human flips.

---

## 0. Verified ground truth (what a deploy actually does)

Read from the tree, 2026-10-12.

### 0.1 Triggers

- `.github/workflows/images.yml`: `on: push branches [master], tags ["v*"], pull_request,
workflow_dispatch` (:18–23). Jobs `meta → web + api → publish → deploy`.
  - `publish` (:239–266) runs only for non-PR events, is the only job with `packages: write`,
    and pushes every tag baked into the image tars.
  - `deploy` (:269–292) `needs: publish`, `if: github.ref == 'refs/heads/master'`, environment
    `production`, and POSTs `compose.redeploy` to Dokploy using `DOKPLOY_API_URL`,
    `DOKPLOY_API_KEY`, `DOKPLOY_COMPOSE_ID` (:277–279). **A tag push does not redeploy. A
    master push always does.**
  - `workflow_dispatch:` has **no inputs** (:23). There is no way to push images without
    deploying from the UI, despite `DEPLOYMENT.md:22` claiming otherwise.
- `deploy/dokploy-auto-update.sh` is a **second, independent deploy trigger** (Dokploy Schedule
  Job, cron `*/10 * * * *`, :8–9) that pulls and calls `compose.redeploy` itself (:54–81). Its
  header (:3–5) says images.yml "never pokes Dokploy", which is false at HEAD. If this cron job
  is installed, two triggers race the same stack.
- Rollback today (`DEPLOYMENT.md:208–212`): set `VERSION` to a previous `sha-<short>` tag in the
  Dokploy Environment tab and redeploy. Immutable tags never move, so the cron job leaves a pin
  alone (`dokploy-auto-update.sh:21–22`).
- **No DB backup step exists anywhere in the documented rollback path.**

### 0.2 Boot order inside the api container

`main.rs`:

1. `Settings::from_env()` (:134) — any error `eprintln!` + `exit(1)` (:136–139). **Before** the DB.
2. `load_field_enc_key()` (:166) — installs the process-wide `OnceLock`.
3. `get_sea_connection()` (:168) → `sea_connect.rs`: connect → `PRAGMA journal_mode=WAL`,
   `busy_timeout=5000`, `foreign_keys=ON` (:46–56) → `ping` → `Migrator::up(&conn, None)`
   (:87–93). A migration error panics (:65–68) → process exits → `restart: unless-stopped`
   restarts it → `up()` re-runs.
4. `gate_store.restore(rate_limit_store::load(&sea_db))` (:170–174), session store (:176),
   web-auth boot reconcile gated on `WEB_AUTH_ENABLED` (:189–209).
5. Periodic tasks: popularity flush companion (:588–651), `scheduler::start_scheduler` (:761),
   hourly session sweep (:764–775).
6. Rate-limit/escalation config parsed independently at :860–895 (`EscalationConfig::from_env`),
   which exits(1) on a bad flag — **after** migrations, **before** the listener.
7. Listener binds **last** (:929–939).

Consequences:

- **Migrations run before the port opens.** There is no window where the new binary serves
  requests against an old schema.
- A config error at step 1 blocks the deploy _before_ touching the DB; a config error at step 6
  blocks it _after_ migrations (migrations are additive/idempotent, so this is acceptable, but
  put new device config parsing in the early block).
- A migration failure is a **crash loop**, not a failed deploy.

### 0.3 Compose topology

`docker-compose.yml`:

- `quran-init` (`:29–38`) runs the **new** api image first, one-shot, `restart: "no"`.
- `api` (`:113–193`): `pull_policy: always` (:117), `restart: unless-stopped` (:193),
  `depends_on: quran-init: service_completed_successfully` (:156–158), healthcheck
  `curl -fsS http://localhost:8888/healthz && curl -fsS http://localhost:8888/quran/health/ready`
  every 30s, 3 retries, 15s start period (:171–180). `DATABASE_URL: sqlite:./data/easyquran.db?mode=rwc`
  (:128) on the `api_data` named volume (:160, 201). `APP_ENV: production` is a **literal**
  (:132) — not overridable.
- `web` (`:40–111`): `pull_policy: always` (:50), `depends_on: api: service_healthy` (:79–81).
- Deploy sequence: stop old api → run new quran-init → start new api (migrates, then binds) →
  wait healthy → recreate web. **The old api process is gone before the new one runs**, so two
  processes never share the DB on a Dokploy redeploy. The A3 §8 `busy_timeout` write-lock
  concern applies only to out-of-band processes (a laptop running `just api-migrate`, a manual
  `cargo run`) — not to the deploy path.
- Failure shape: if api never becomes healthy, web is never recreated; the **old** web container
  keeps serving static/prerendered pages, `/api/*` is dead, translated SSR fails. Degraded, not
  blacked out.

### 0.4 Migration engine (verified in the crate, not in the reports)

`sea-orm-migration-1.1.20/src/migrator.rs`:

- `exec_with_connection` (:252–273) wraps migrations in a transaction **only for Postgres**
  (:261–267). `DbBackend::MySql | DbBackend::Sqlite` get `SchemaManager::new(db)` with **no
  transaction** (:268–271).
- The `seaql_migrations` version row is inserted **after** `migration.up(manager).await?`
  returns (:386–397).

So on SQLite: DDL auto-commits per statement, the version row is written last, and a crash at any
point leaves `up()` to be re-run on the next boot. This **corrects A3 §8**, which claims each
migration is wrapped in a transaction. Idempotency is the only thing between a crash and a boot
loop.

### 0.5 Observability available in production

- `utils/telemetry.rs`: OTLP export is gated on `ENABLE_QUICKWIT_OTEL` (:247–252). That variable
  is **absent from `docker-compose.yml` and `.env.example`** → production ships logs to stdout
  (Dokploy) only; there is no metrics backend.
- Real surfaces: `GET /healthz` (Docker network only, `router.rs:212–234`), `GET /quran/health/ready`
  (`controller.rs:1037–1060`), web `GET /health/quran` (disk-cache counters), admin status
  endpoints.
- Runtime-config precedent to copy: `services/route_blocker_config.rs` (atomics + `Notify`) and
  its admin routes `POST /admin/route/v1/sync_interval/{pause,resume,restart}` behind
  `verified_with_role::<ROLE_ADMIN>` (`admin_route_v1/mod.rs:36–38`, `controller.rs:187–257`).
  **This is the only flip-without-redeploy mechanism in the stack.** Every env flag is read at
  boot; changing any of them requires a container recreate (Dokploy redeploy).
- Keys are process-immutable (`load_field_enc_key` OnceLock pattern; A9 #14): rotation = restart,
  single instance.

### 0.6 Flag style to match

`EscalationConfig` (`settings.rs:306–379`): default-off `env_bool("QURAN_BAN_ESCALATION_ENABLED",
false)`; enabled + missing `QURAN_BAN_ALLOWLIST` = boot error (:349–363); parsed **independently**
of `RateLimitSettings` "so existing struct literals in other modules compile unchanged"
(`main.rs:863–866`); the engine is attached as `Option<Arc<dyn Escalation>>` and every other
limiter holds `None` (:860–895). A3 #14 lists the eight exhaustive struct literals that break if
`HttpSettings`/`RateLimitSettings` gains a field. **Device config must be its own struct.**

---

## 1. Build order and dependency graph

Each row is one PR unless marked. "Gates green" means `bun run check && bun run lint && bun run test`
in `web/`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`cargo test --workspace -- --test-threads=1` in `rust/`.

```
PR-0  CI gates restore ────────────────────────────────► gates every later step
        │
PR-1  m000012 pure DDL ─┬─► PR-2 AuthSessionState compat (serde defaults + legacy decode test)
        │              │        │
        │              │        ├─► PR-4 device mint/link + device_records writer ──► PR-6 shadow evaluator
        │              │        │        (flag DEVICE_COLLECTION_ENABLED)              (flag DEVICE_ENFORCEMENT_ENABLED=false)
        │              │        │            + retention sweeper in the SAME release
        │              │        │
        │              └─► PR-3 web collector (flag off, no server dependency) ──► PR-4
        │
        └─► PR-5 admin device API (list/propose/unban, Admin role) ──┐
                                                                     ├─► PR-8 device bans L5/L6 + appeals + NotificationKind
PR-6 ──► PR-7 challenge/step-up L2 (needs Turnstile configured) ─────┘        (human-approve path first)
PR-6 ──► GATE: 14d shadow, numeric thresholds (§7) ──► PR-7 / PR-8

PR-9  edge attestation verifier, observe-only (flag EDGE_ATTEST_REQUIRED=false) — INDEPENDENT of device work
PR-10 edge enforcement — only if PR-9's absence/mismatch data justifies it; never a sole gate
```

### 1.1 What can land independently

| PR   | Content                                                                                                                                                     | Why independent                                                                                  |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| PR-0 | Restore the two gate blocks in `images.yml` (web: check/lint/test; rust: fmt/clippy/test). Audits excluded.                                                 | Pure CI. No runtime change.                                                                      |
| PR-1 | `migration/src/m000012_device_identity.rs` + `lib.rs` registration + inline tests. Pure `CREATE TABLE/INDEX IF NOT EXISTS`, empty tables, no reader/writer. | Nothing in `src/` references it. Old binaries ignore additive tables (`lib.rs:24–25` precedent). |
| PR-3 | `web/src/lib/device/**` collector, lazy dynamic import, flag off; no header attached until PR-4.                                                            | Client-only; dead code when the flag is false.                                                   |
| PR-5 | `modules/admin_device_v1/**` list/propose/unban behind `verified_with_role::<ROLE_ADMIN>`; export feed untouched (IP-only, `deploy/README.md:411–436`).     | Reads tables that are empty until PR-4; useful immediately as a manual ban surface.              |
| PR-9 | `X-Eq-Edge-*` strip + Ed25519 verify middleware, observe-only, unknown `kid` drops claims.                                                                  | Independent of device identity; `EDGE_ATTEST_REQUIRED` stays false.                              |

### 1.2 What must land together (same release)

| Bundle                  | Members                                                                                                                                                 | Reason                                                                                                                    |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Writer + retention      | PR-4 + the `device_records`/`device_sessions`/`device_evidence` sweeper (own 60 s task, `id IN (SELECT … LIMIT 500)`, `NOT EXISTS` ban guard — A10 #15) | Rows must never exist without a sweeper; `expires_at NOT NULL` is unenforceable otherwise (DESIGN v0→v1 row 22).          |
| Evaluator + kill switch | PR-6 + `POST /admin/device/v1/pause                                                                                                                     | resume` (runtime atomics, Admin role)                                                                                     | Shadow can be stopped in seconds without a redeploy. |
| Collector + writer      | PR-3 + PR-4                                                                                                                                             | Flag-gated both ways; the server must treat a missing payload as **unknown/allow** from day one (A3 #14 behavioral list). |
| Bans + appeals          | PR-8 + `appeals` + one mail template + `NotificationKind::Enforcement`                                                                                  | A ban with no appeal path is the DSA/Art-17 problem DESIGN §B7 is trying to avoid.                                        |

### 1.3 Hard ordering constraints

1. **PR-2 before PR-4.** Adding any field to `AuthSessionState` without `#[serde(default)]` makes
   every live session fail to decode: `AuthSession::new` → anonymous (401s), the
   `FromRequestParts` path → `SessionError` → **HTTP 500** (A3 #1, `state.rs:6–26`,
   `extractor.rs:44–45,239–241`, `error.rs:71,175–179`). Fields are appended, never reordered.
   Same-release is acceptable _only_ with the legacy-payload decode test; separate releases are
   safer.
2. **PR-1 before any writer.** The migration must exist before `device_records` is written.
3. **PR-4 before PR-6** (scoring needs records), **PR-6 before PR-7/PR-8** (no enforcement before
   shadow), **PR-5 before PR-8** (no ban without list/unban).
4. **PR-0 before everything** if you want any of the above to be machine-checked. Today it is not
   (A6 #1: master CI runs zero tests; `53d3d421` deleted the gates that `37d90c41` added).

---

## 2. Feature flags

All parsed in one independent struct, `DeviceConfig::from_env()`, called from `main.rs` **before**
`get_sea_connection()` (so a bad pepper fails before the DB opens), mirroring `EscalationConfig`
(`settings.rs:348–378`) and its independent-parse comment (`main.rs:863–866`). No new fields on
`HttpSettings`/`RateLimitSettings` (A3 #14).

| Flag                                  | Default | Prod-gate behavior                                                                                                                                                                                                                          | Read at                                                                                                                                                                                                                    | Flip without redeploy?                                   |
| ------------------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `DEVICE_COLLECTION_ENABLED`           | `false` | `true` requires: `DEVICE_KEY_PEPPER` set + not in the placeholder blocklist, and `WEB_AUTH_ENABLED=true` (device axis is auth-scoped, A10 §8). Else `exit(1)` before the DB.                                                                | `DeviceConfig` (early `main.rs`); `router.rs` nest gate for `/device/identity/v1` (a **new** nest — not `/device/v1`, which is FCM and mounts unconditionally, `router.rs:165–177`); boot hydration; sweeper registration. | No — boot-only.                                          |
| `DEVICE_ENFORCEMENT_ENABLED`          | `false` | `true` requires: collection on, `DEVICE_BAN_ALLOWLIST` **present** (may be empty — same shape as `QURAN_BAN_ALLOWLIST`, `settings.rs:349–363`), and `DEVICE_RULE_PACK_VERSION` non-empty and equal to the compiled-in pack. Else `exit(1)`. | `DeviceConfig`; the evaluator's `Option<Arc<dyn DeviceRisk>>` attachment (mirror `main.rs:867–895`).                                                                                                                       | No — and deliberately not runtime-flippable.             |
| `DEVICE_BAN_ALLOWLIST`                | unset   | Required to exist when enforcement is on. Comma-separated **account/user ids**, not CIDRs (a device unit cannot be CIDR-exempt — A3 #6). Unknown ids are a boot error.                                                                      | `DeviceConfig`.                                                                                                                                                                                                            | No.                                                      |
| `DEVICE_RULE_PACK_VERSION`            | unset   | Required when enforcement is on; pinned string, logged at boot.                                                                                                                                                                             | `DeviceConfig` + rule pack.                                                                                                                                                                                                | No.                                                      |
| `DEVICE_KEY_PEPPER`                   | unset   | Required when any device flag is on; 32 B (`openssl rand -hex 32`); placeholder blocklist (`.env.example` value included); `Zeroizing` `OnceLock`s via HKDF subkeys `ruxlog-device-token-v1` / `ruxlog-device-fp-v1` (A9 #1).               | Early `main.rs`, next to `load_field_enc_key()` (`main.rs:166`).                                                                                                                                                           | **No — process-immutable.** Rotation = restart (A9 #14). |
| `DEVICE_KEY_PEPPER_PREV`              | unset   | Optional; if set must differ from current; read-only fallback for token verification; kept ≥ retention window.                                                                                                                              | Early `main.rs`.                                                                                                                                                                                                           | No.                                                      |
| `PUBLIC_DEVICE_COLLECTION`            | `false` | None (web only). Read via `$env/dynamic/public` (runtime, not build-baked — `docker-compose.yml:45–46`), gates the lazy collector import.                                                                                                   | Web collector boot path.                                                                                                                                                                                                   | No — container env.                                      |
| `EDGE_ATTEST_REQUIRED`                | `false` | `true` requires `EDGE_PUBLIC_KEYS` non-empty + a valid skew value. Never a sole gate; only named endpoints (A9 #11).                                                                                                                        | Early `main.rs`; verifier middleware.                                                                                                                                                                                      | No.                                                      |
| `EDGE_PUBLIC_KEYS`                    | unset   | Required when attestation is required; `kid:base64` list. Unknown `kid` → drop claims, keep request.                                                                                                                                        | Early `main.rs`.                                                                                                                                                                                                           | No.                                                      |
| (runtime) `device_enforcement_paused` | `false` | None. `AtomicBool` read by the evaluator; `POST /admin/device/v1/pause                                                                                                                                                                      | resume`behind`verified_with_role::<ROLE_ADMIN>`(copy`admin_route_v1/mod.rs:36–38`, `controller.rs:236–248`).                                                                                                               | Evaluator; status endpoint.                              | **Yes — the only no-restart switch.** |

Notes:

- "Flip without redeploy" is a property of **Dokploy, not of the flag**: env changes require a
  container recreate because `env_file`/`environment` are injected at create time. The runtime
  admin toggle is the only exception, and it is the emergency brake.
- A bad flag value is a crash loop, not a failed deploy: `exit(1)` at step 1 of boot, then
  `restart: unless-stopped` retries forever. That is fail-closed and correct, but the operator
  must know the recovery: pin `VERSION` to the previous sha.
- Which flags are safe to flip while enforcement is off: all of them. Which must never be flipped
  live: `DEVICE_KEY_PEPPER`/`_PREV` (process-immutable, orphans records — A9 #2), and
  `DEVICE_ENFORCEMENT_ENABLED`/`EDGE_ATTEST_REQUIRED` (each is a two-step deliberate change with
  a restart).

---

## 3. Shadow mode

Goal: every scored rule runs, records what it _would_ do, and changes nothing — with **zero extra
DB writes on the request path**.

### 3.1 Where the evaluation lives

- Not a router-level middleware (A3 #8: a device middleware at router level runs before every
  limiter and doubles session/user reads). The evaluation is a pure function called from inside
  the already-loaded session context (`check_requirements`, `guard.rs:9–67`) and at login/link
  time. Inputs: the device fields already carried in the session payload (with their own TTL) and
  the in-process trust map. No `SELECT` on the request path; no `INSERT`/`UPDATE` on the request
  path.
- Missing payload ⇒ `unknown` ⇒ verdict `pass`. A device ban can never be the sole block
  condition (A3 #14 behavioral list; A10 §8).
- Reader routes are untouched: no payload on `/t/**`, `/{surah}/t/**`, `/quran/v1`, or internal
  SSR (A7 #12; guarded by the web `nav-guard`-style tests).

### 3.2 Where observations live (the zero-write design)

Two layers, both off the request path:

1. **In-process (request path):** `AtomicU64` counters in a fixed array indexed by
   `(rule_id, verdict)` where `verdict ∈ {pass, challenge, throttle, ban, unknown_device}`, plus a
   bounded `Mutex<VecDeque<ShadowSample>>` (cap 4096, drop-oldest) for would-fire cases only.
   `ShadowSample = { ts, rule_id, rule_version, score, verdict, device_record_id: Option<ULID>,
user_id: Option<i64> }`. Raw signals never enter a sample (G4 guard, A6 §3). The push is a
   short mutex, no `.await`, no SQL — the same shape as the existing lock-free demand counters
   (`translation_popularity_store.rs:1–5`).
2. **Off-path flush (its own task, not the 10 s durability task):** a dedicated
   `tokio::spawn` with a 60 s interval (precedent: the hourly session sweep, `main.rs:764–775`;
   A10 #15 says do not ride the rate-limit flush task for new writers). Each tick:
   - drains the ring buffer and writes ≤500 rows per statement into `device_shadow_observations`
     (14-day retention), plus one aggregate row per `(rule_id, hour)` into `device_shadow_stats`;
   - budget ≤50 ms per tick; best-effort and non-fatal (warn + continue, like
     `rate_limit_store.rs:162–164`);
   - retention deletes use `DELETE FROM t WHERE id IN (SELECT id FROM t WHERE created_at < ?
LIMIT 500)` — no `DELETE … LIMIT` on this SQLite build (A10 #15).

Schema (part of m000012, so PR-1 carries it):

```sql
CREATE TABLE IF NOT EXISTS device_shadow_observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    rule_id TEXT NOT NULL,
    rule_version TEXT NOT NULL,
    score INTEGER NOT NULL,
    verdict TEXT NOT NULL,          -- 'challenge' | 'throttle' | 'ban'
    device_record_id TEXT,
    user_id INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_device_shadow_created ON device_shadow_observations (created_at);
CREATE INDEX IF NOT EXISTS idx_device_shadow_rule_created ON device_shadow_observations (rule_id, created_at);

CREATE TABLE IF NOT EXISTS device_shadow_stats (
    bucket_hour INTEGER NOT NULL,
    rule_id TEXT NOT NULL,
    evals INTEGER NOT NULL DEFAULT 0,
    would_challenge INTEGER NOT NULL DEFAULT 0,
    would_throttle INTEGER NOT NULL DEFAULT 0,
    would_ban INTEGER NOT NULL DEFAULT 0,
    unknown_device INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_hour, rule_id)
);
```

Both tables are empty in PR-1, so PR-1 remains reversible with zero data loss.

### 3.3 Live view and the guard

- `GET /admin/device/v1/shadow/status` (Admin role) returns the atomics directly — no DB read, so
  an operator can see counters even when the flush is failing.
- A guard test in the house style (source scan) asserts: the shadow module contains no
  `sea_orm`/`Statement`/`execute` on the request-path function; the request-path function is
  `fn` (not `async fn`); `device_records` is not written outside the login/link module. Mirror
  `rate_limit_store.rs:1` + its flush-only tests, and A6 §3's guard list.
- Zero-write proof in production: counter `device.shadow.request_path_writes` must stay 0, and
  the SQLite write count attributable to the api (flush rows ÷ 60 s) must match the tick budget.

### 3.4 Promotion gate (shadow → enforce)

A rule may be promoted only when **all** of these hold. Evaluate per rule, not per pack.

| #   | Gate              | Measurement                                                                                                                                                                                |
| --- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| G1  | Window            | ≥14 consecutive days with `DEVICE_COLLECTION_ENABLED=true`, including ≥1 full weekend, and ≥7 days since the last collector change.                                                        |
| G2  | Volume            | ≥50,000 evaluations of the rule; ≥200 distinct accounts in the would-fire set. If not met, extend — never lower.                                                                           |
| G3  | Human precision   | Two moderators independently label a random sample of ≥100 would-fire cases (all if fewer). Cohen's κ ≥0.6; confirmed false-positive rate ≤5% for challenge rules, ≤1% for throttle rules. |
| G4  | Coverage          | Unknown-device share ≤15% of authenticated requests; payload present on ≥95% of logins.                                                                                                    |
| G5  | Allowlist hygiene | Zero would-fire events on `DEVICE_BAN_ALLOWLIST` accounts, internal service identities, or test accounts.                                                                                  |
| G6  | Performance       | Request p99 delta ≤ +1 ms vs the pre-shadow baseline; zero request-path DB ops (G3.3); flush tick ≤50 ms; DB growth ≤40 MB/30 d.                                                           |
| G7  | Ban rules only    | Additionally ≥30 days of shadow data, ≤1% confirmed FP, a working auto-reversal path, and two-person sign-off.                                                                             |

Abort the promotion (and keep shadow running) if any rule's would-fire set exceeds 20% on a single
device cohort (the shared-phone signal) or if G3/G5 fails for two consecutive reviews.

---

## 4. Migration rollout: m000012 on the live SQLite DB

### 4.1 Pre-flight (T−1 day, on the Dokploy host)

The volume is `<project>_api_data` (project defaults to `easyquran`, `docker-compose.yml:20`),
mounted at `/app/data`; the DB is `/app/data/easyquran.db`. The api image has `curl` + `jq` but no
`sqlite3` (`deploy/Dockerfile.api:19`), so use a one-off sqlite3 container.

```bash
# 0. Confirm the volume exists and get its real name.
docker volume ls | grep api_data

# 1. Consistent online backup (WAL-safe). A raw `cp` of easyquran.db is NOT a backup:
#    journal_mode=WAL (sea_connect.rs:48) leaves committed data in easyquran.db-wal.
mkdir -p /opt/easyquran/backups
docker run --rm \
  -v easyquran_api_data:/data \
  -v /opt/easyquran/backups:/backup \
  keinos/sqlite3:latest /data/easyquran.db \
  ".backup '/backup/easyquran-$(date -u +%Y%m%dT%H%M%SZ).db'"

# 2. Record the pre-state.
docker run --rm -v easyquran_api_data:/data keinos/sqlite3:latest /data/easyquran.db \
  "SELECT version, applied_at FROM seaql_migrations ORDER BY applied_at DESC LIMIT 3;"
docker run --rm -v easyquran_api_data:/data keinos/sqlite3:latest /data/easyquran.db \
  "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'device_%';"
# Expect: only the last m000011 row, and NO device_* tables.

# 3. Prove the migration on a COPY first (never on the live file).
docker run --rm -v /opt/easyquran/backups:/backup keinos/sqlite3:latest \
  /backup/easyquran-<stamp>.db ".tables"
```

Also verify the restore path once, before you need it: copy the backup to a scratch volume and
point a throwaway container at it.

### 4.2 Deploy steps

1. **Freeze the deploy triggers.** No merges to master until verification is done. If
   `deploy/dokploy-auto-update.sh` is installed as a Dokploy schedule job, **disable the schedule**
   — otherwise it can redeploy mid-verification (`dokploy-auto-update.sh:8–9,69–81`). Optionally
   pin `VERSION` to the current `sha-<short>` so neither trigger moves the stack.
2. Merge/push the PR. CI builds both images, publishes, calls `compose.redeploy`.
3. Watch the sequence:
   ```bash
   cd /etc/dokploy/compose/<app>/code
   docker compose ps                       # quran-init -> api -> web
   docker compose logs -f api | grep -Ei "migration|migrat|panic|initialization failed"
   ```
   Expected log lines: `Starting database migrations` → `Applying migration
'm000012_device_identity'` → `Migration 'm000012_device_identity' has been applied` →
   `Database migrations completed successfully` (`sea_connect.rs:88–92`).
4. Migrations run before the port opens, so the healthcheck cannot pass with a half-applied
   schema. Wait for `api` healthy and `web` healthy (`docker compose ps`), ~30 s total.

### 4.3 If the api container restarts mid-migration

Because sea-orm does **not** wrap SQLite migrations in a transaction and records the version only
after `up()` returns (§0.4):

- every statement already executed stays committed (DDL is auto-committed);
- the version row is absent, so `Migrator::up` re-runs m000012 on the next boot;
- with every statement `IF NOT EXISTS` / `PRAGMA table_info`-guarded, the re-run is a no-op and
  boot proceeds. There is no partial-state class beyond "some of the five tables exist";
- if any statement is not idempotent, the re-run errors → `init_db` panics → `restart:
unless-stopped` loops forever. **This is the entire reason the up-twice test (m000011:56–57
  style) is a merge blocker, not a nicety.**
- Recommended hardening: wrap the DDL batch in `conn.transaction::<_, (), DbErr>(...)` (SQLite DDL
  is transactional; the repo already uses this form in `rate_limit_store::flush`) so a crash
  cannot leave partial DDL at all. The version-insert gap remains, so idempotency is still
  required.

### 4.4 Verification (after the deploy)

```bash
# a. Container state.
docker compose ps                                   # api + web healthy, quran-init exited 0

# b. Public readiness (proves the API bound and the corpus loaded).
curl -fsS https://easyquran.fyi/api/quran/health/ready | jq '{ready, auth}'

# c. Migration recorded.
docker run --rm -v easyquran_api_data:/data keinos/sqlite3:latest /data/easyquran.db \
  "SELECT version FROM seaql_migrations ORDER BY applied_at DESC LIMIT 1;"
# Expect: m000012_device_identity

# d. Tables + indexes present, rows zero.
docker run --rm -v easyquran_api_data:/data keinos/sqlite3:latest /data/easyquran.db \
  "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'device_%' ORDER BY name;"
# Expect: device_bans, device_evidence, device_records, device_sessions, device_shadow_observations, device_shadow_stats
docker run --rm -v easyquran_api_data:/data keinos/sqlite3:latest /data/easyquran.db \
  "SELECT (SELECT COUNT(*) FROM device_records) AS records,
          (SELECT COUNT(*) FROM device_sessions) AS sessions,
          (SELECT COUNT(*) FROM device_bans) AS bans,
          (SELECT COUNT(*) FROM device_evidence) AS evidence;"
# Expect: 0|0|0|0

# e. No new 5xx, no restart loop.
docker compose logs --since 10m api | grep -cE "ERROR|panic" || true
```

### 4.5 The two compatibility windows

| Window                      | Reachable?                                                                                                                                                                                                                                                                                                | Behavior                                                                                                                                                                                                                                                              | Verify                                                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| **Old binary + new schema** | Yes — that is exactly the `VERSION`-pin rollback after m000012.                                                                                                                                                                                                                                           | Safe: the old code never reads `device_*`; additive-only tables are the documented m000004 pattern (`lib.rs:24–25`).                                                                                                                                                  | After pinning the previous sha: `/healthz` ok, `/quran/health/ready` `ready:true`, one login + one logout succeed, logs clean. |
| **New binary + old schema** | No in the container: `get_sea_connection()` (migrations) completes before the listener binds (`main.rs:168` vs `:929–939`). Reachable only via out-of-band DB surgery (a manual `down --steps 1` while the new binary is running) or a hypothetical `run_migrations=false` build (exists only for tests). | Next boot re-runs `up()` → tables restored, idempotent. If a table is missing while serving, every device read must warn-and-empty (`rate_limit_store::load` shape, `:111–115`) and every write must be non-fatal — otherwise `no such table` 500s take the API down. | Ship the fail-open behavior in PR-4 and test it with a dropped table.                                                          |

### 4.6 The rule that must never be broken

**Never run a bare `Migrator::down`.** `migration/bin/migrate.rs` with `steps: None` rolls back
**everything**, and m000001's `down` drops every table (A3 #13). Only `--steps 1`, only after an
export, only with the API stopped.

---

## 5. Rollback, per step

| Step                              | Rollback action                                                                                                                                                                                     | Data loss                                                                                   | Irreversible?                                                                                                     |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| PR-0 CI gates                     | Revert the workflow commit.                                                                                                                                                                         | None.                                                                                       | No.                                                                                                               |
| PR-1 m000012                      | Revert code (schema stays, harmless) **or** `migrate down --steps 1` with the API stopped.                                                                                                          | None — tables are empty until PR-4 lands. After PR-4: total (see below).                    | No before PR-4; effectively yes after.                                                                            |
| PR-3 collector                    | Set `PUBLIC_DEVICE_COLLECTION=false` + redeploy (or revert). Browser-local token remains until the user clears site data; it is inert.                                                              | None server-side.                                                                           | No.                                                                                                               |
| PR-4 mint/link writer             | Flag off + revert. Rows become unreferenced; the sweeper (if it stays) expires them at 30 d. **If the sweeper is reverted too, rows persist forever** — keep the sweeper when disabling the writer. | None (rows are regenerable on next login).                                                  | No.                                                                                                               |
| PR-6 shadow                       | Runtime `pause` (instant) + flag off. Shadow tables retain ≤14 d of data for the review.                                                                                                            | Shadow data only.                                                                           | No.                                                                                                               |
| PR-7 challenge (L2)               | Runtime `pause` → flag off. In-flight challenge state in session payloads must be ignored on the next release (add a version check).                                                                | None.                                                                                       | No.                                                                                                               |
| PR-8 device bans (L5/L6)          | `DELETE /admin/device/v1/bans` (un-ban) + runtime pause. Reverting the code stops enforcement (fail-open) — the safe direction.                                                                     | Ban rows are deleted on un-ban; evidence rows are append-only and should be exported first. | Ban rows: no. Evidence: dropping them is lossy — export first.                                                    |
| PR-9/10 edge attestation          | `EDGE_ATTEST_REQUIRED=false` + redeploy. Never fail-closed on absence (A9 #11).                                                                                                                     | None.                                                                                       | No.                                                                                                               |
| Pepper rotation (`_PREV` removal) | None — `token_hash` is one-way.                                                                                                                                                                     | Every record is orphaned; every device ban stops matching (A9 #2).                          | **Yes.** Make reversible by keeping `_PREV` ≥30 d and never deleting it in the same deploy that adds the new key. |

### 5.1 Irreversible steps and how to make them reversible

1. **`m000012.down` after writers land.** `device_key` is a one-way HMAC of signals that are
   never stored; the records cannot be reconstructed from `user_sessions`, logs, or anything else
   (A10 #11). _Make it reversible:_ forbid `down` in the runbook; roll back by pinning the
   previous sha (tables stay, old binary ignores them). If the tables must actually be dropped,
   export `device_records` + `device_bans` + `device_evidence` to JSON first and keep the file
   for 12 months.
2. **Pepper rotation / `_PREV` deletion.** Same reason. _Make it reversible:_ dual-key read
   window, `token_key_id` column, `_PREV` retained for the full retention window, deletion as a
   separate deliberate deploy after 30 d.
3. **Deleting evidence rows.** Append-only by design. _Make it reversible:_ export before any
   bulk delete; never let the retention sweeper touch `device_evidence` without the ban guard
   (`NOT EXISTS (SELECT 1 FROM device_bans …)`, A10 #15).

---

## 6. Health and verification per step, with abort criteria

Baseline (capture before any step): `docker compose ps`, the three endpoints below, api log 5xx
count, `device_*` table counts, DB file size, and `web /health/quran` disk-cache counters.

| After   | Check                                                                                                                                                                                                      | Abort if                                                                                                                                       |
| ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| PR-0    | CI run on a throwaway PR is green; the same run on master goes green before the deploy job.                                                                                                                | Any gate fails → do not merge; fix or scope down the gate.                                                                                     |
| PR-1    | §4.4 a–e.                                                                                                                                                                                                  | api not healthy within 5 min, or `docker compose ps` shows `Restarting`, or logs show `Database initialization failed` → pin the previous sha. |
| PR-3    | Web bundle loads; no `x-eq-device` header on any request (`web/scripts/assert-headers.sh`); `/t/**` disk cache still warm-hits (`/health/quran` counters); collector worker absent when the flag is false. | Any reader-route header, any disk-cache regression, any CSP/console error on `/t/**`.                                                          |
| PR-4    | `device_records` row count grows only on login (sample 10 logins, count +10±1); `rate_limit_state` untouched; no new 5xx on `/auth/v1/*`; login p99 unchanged.                                             | Any write during a non-login request; `rate_limit_state` row count change; 5xx rate > baseline + 0.1%.                                         |
| PR-6    | `GET /admin/device/v1/shadow/status` counters advance; `device_shadow_observations` grows by ≤4096/60 s; request p99 delta ≤1 ms; zero request-path writes.                                                | Flush tick >200 ms; p99 delta >20%; any request-path write.                                                                                    |
| PR-7    | Challenge shown only on the would-fire set; Turnstile success rate; no challenge on allowlisted accounts; challenge→pass latency.                                                                          | Challenge rate >5% of authenticated requests, or any allowlist hit → runtime pause.                                                            |
| PR-8    | Ban list/unban round trip; export feed still contains only `quran-ban:` IP units (`GET /admin/bans/export`); un-ban propagates within one flush interval; appeal path reachable unauthenticated.           | Any device unit in the export feed; un-ban not propagating within 2 flush intervals; appeal page behind the `ready` gate.                      |
| PR-9/10 | Absence rate and mismatch rate of `X-Eq-Edge-*`; forged header test (A6 §8 manual procedure); origin strips client headers outermost.                                                                      | Any request blocked while `EDGE_ATTEST_REQUIRED=false`; any forged claim accepted.                                                             |

Universal abort: **any container restart loop, any `/healthz` failure, or any 5xx increase**
→ `VERSION=<previous sha>` in the Dokploy Environment tab → redeploy → re-verify §4.4.

---

## 7. Real-traffic verification before any enforcement

Shadow mode must run on production traffic. Minimum:

- **Duration:** 14 consecutive days, including a full weekend, with no collector change in the
  last 7. Ban-level rules: 30 days.
- **Volume:** ≥50,000 evaluations per rule; ≥200 distinct accounts in the would-fire set.
- **What to look at, in order:** (1) `device_shadow_stats` per-rule totals; (2) the sampled
  `device_shadow_observations` for the would-fire set, reviewed independently by two people;
  (3) the unknown-device share; (4) request p99 vs baseline; (5) SQLite growth; (6) the api log
  for any `no such table`/write errors; (7) `device_records` retention counts to confirm the
  sweeper keeps up.
- **Numeric gate to proceed:** all of G1–G7 in §3.4. The binding numbers: ≤5% confirmed
  false-positive rate for challenge rules, ≤1% for throttle/ban rules, ≤15% unknown-device share,
  ≤+1 ms p99, zero request-path DB writes, zero allowlist hits.
- **Then, and only then:** flip `DEVICE_ENFORCEMENT_ENABLED=true` in a deliberate deploy with the
  runtime pause endpoint pre-verified, and re-run §6's PR-7 checks for 48 h.

---

## 8. What the operator must configure outside the repo

### 8.1 Hard prerequisites (in order)

1. **CI gates restored (PR-0).** Without this, no later step is machine-verified.
2. **`WEB_AUTH_ENABLED=true` in production** (already set per `DEPLOYMENT.md:157`). The device axis
   is auth-scoped; without it, device rows are never written and bans can only be hand-made
   (A10 §8).
3. **A proven DB backup/restore procedure** (§4.1), including a restore rehearsal. The documented
   rollback path has no backup step today.
4. **`DEVICE_KEY_PEPPER`** — `openssl rand -hex 32`, stored only in the Dokploy Environment tab
   (never a `PUBLIC_*` var, never logged). Required before any device flag is enabled. The boot
   must reject the `.env.example` placeholder.
5. **Origin ingress restricted to Cloudflare ranges** (`deploy/README.md:377–392`). Everything
   edge- or `CF-Connecting-IP`-derived depends on this; verify it is actually in place rather
   than trusting the doc.
6. **Turnstile configured** (`TURNSTILE_SECRET_KEY` api + `PUBLIC_TURNSTILE_SITE_KEY` web,
   already in the compose env block, `:73`) before L2 challenges; production registration already
   503s without the secret.
7. **A second human** for L6 two-person approval and for the §3.4 sample review.

### 8.2 Dokploy (Environment tab, exact values)

```env
# Stage 2 (collection) — after PR-1..PR-4
DEVICE_COLLECTION_ENABLED=true
PUBLIC_DEVICE_COLLECTION=true
DEVICE_KEY_PEPPER=<openssl rand -hex 32>
DEVICE_KEY_PEPPER_PREV=            # empty until a rotation

# Stage 3 (enforcement) — only after the §7 gate
DEVICE_ENFORCEMENT_ENABLED=true
DEVICE_BAN_ALLOWLIST=              # empty allowed; account ids, comma-separated
DEVICE_RULE_PACK_VERSION=<pinned pack id>

# Stage 4 (edge, optional) — only after PR-9 proves absence/mismatch handling
EDGE_ATTEST_REQUIRED=false
EDGE_PUBLIC_KEYS=

# Optional observability (only if you want OTLP metrics; not required by the plan)
ENABLE_QUICKWIT_OTEL=true
QUICKWIT_INGEST_URL=<collector url>
QUICKWIT_ACCESS_TOKEN=<token>
```

Also:

- **Disable Dokploy's own git auto-deploy** (`DEPLOYMENT.md:170–172`) and the
  `dokploy-auto-update.sh` schedule during the migration window; pick **one** deploy trigger
  permanently.
- Do **not** set a Dokploy domain (the Traefik labels own routing, `DEPLOYMENT.md:174–176`).
- Keep `VERSION` unpinned only if you accept a redeploy on every master merge; pin it during the
  rollout.

### 8.3 Cloudflare

- **Origin ingress allowlist to CF ranges** — the only thing that makes `CF-Connecting-IP` and any
  edge claim trustworthy (`deploy/README.md:377–392`). Hard prerequisite.
- **Turnstile**: production hostname allowlisted; sitekey/secret paired. Hard prerequisite for L2.
- **Worker for edge attestation** (Ed25519, private key only in the Worker): required only if
  `EDGE_ATTEST_REQUIRED=true`; the Worker is a deployment-owned component with no code in this
  repo. Not a prerequisite for device identity.
- **WAF**: do not add rules that block `/api/auth/*` during the shadow window — they would bias
  the shadow data. Optional rate-limit rules only after the gate.

### 8.4 GitHub

- No new secrets. The deploy job needs `DOKPLOY_API_URL`, `DOKPLOY_API_KEY`,
  `DOKPLOY_COMPOSE_ID` in the `production` environment (`images.yml:277–279`). The
  `DOKPLOY_DEPLOY_WEBHOOK` secret named in `DEPLOYMENT.md:92` is **not read by the workflow**.

---

## 9. One-page runbook

```
STAGE 0 — GATES (PR-0)
  merge CI-restore PR ──► confirm green CI on master ──► confirm deploy job ran
  ABORT: any gate red → stop, fix the gate, do not proceed.

STAGE 1 — SCHEMA (PR-1: m000012 only)
  1. freeze merges; disable the Dokploy auto-update schedule
  2. backup:  docker run --rm -v easyquran_api_data:/data -v /opt/easyquran/backups:/backup \
                keinos/sqlite3:latest /data/easyquran.db ".backup '/backup/<stamp>.db'"
  3. record pre-state (seaql_migrations tail; no device_* tables)
  4. merge PR-1 ──► watch: docker compose logs -f api | grep -i migrat
  5. verify §4.4 a–e (ready:true, m000012 row, 6 tables, 0 rows)
  ABORT: api Restarting / "Database initialization failed" → Dokploy VERSION=<previous sha>
         → redeploy → re-verify → stop the rollout.

STAGE 2 — COLLECT + SHADOW (PR-2 session compat → PR-3 collector → PR-4 writer+sweeper → PR-6 shadow)
  1. merge PR-2; verify one login on an existing session survives the deploy (no logout storm)
  2. set DEVICE_KEY_PEPPER, DEVICE_COLLECTION_ENABLED=true, PUBLIC_DEVICE_COLLECTION=true;
     merge PR-3+PR-4; verify §6 PR-4 row checks
  3. merge PR-6; open /admin/device/v1/shadow/status; confirm counters advance
  ABORT: any reader-route header, any request-path write, any 5xx rise, any
         rate_limit_state change → set COLLECTION=false + redeploy (or runtime pause).

STAGE 3 — SHADOW WINDOW (14 days, 30 for ban rules)
  daily: shadow status counters; unknown share; p99; SQLite growth; sweeper lag
  day 14: sample review (2 people, ≥100 cases/rule) → §3.4 G1–G6
  ABORT: G3/G5 fail twice consecutively, or any rule fires >20% on one cohort → keep shadowing.

STAGE 4 — ENFORCE (PR-7 challenge first; PR-8 bans after 30d + two-person sign-off)
  1. verify runtime pause endpoint works BEFORE enabling
  2. set DEVICE_ENFORCEMENT_ENABLED=true + allowlist + pack version → deploy
  3. 48h watch: challenge rate, allowlist hits, appeal path
  ABORT: challenge rate >5% → POST /admin/device/v1/pause (instant) → investigate.

STAGE 5 — EDGE (optional, PR-9 → PR-10)
  PR-9 observe-only; inspect absence/mismatch rates ≥14 days; only then EDGE_ATTEST_REQUIRED=true.
  ABORT: any blocked request while the flag is false → revert the middleware.

ROLLBACK (any stage): Dokploy → VERSION=<previous sha-<short>> → redeploy → §4.4.
  Never `migrate down` without an export and a stopped API.
```

---

## 10. The smallest first PR that is useful, safe, and reversible

**PR-1 = m000012 alone.** One new file
(`rust/backend/api/migration/src/m000012_device_identity.rs`), one line in
`migration/src/lib.rs`, and inline tests. Nothing in `src/` reads or writes it.

- **Useful:** it is the only step with a live-DB component, and it is executed while the stakes
  are exactly zero. It also unblocks every later PR to be code-only, and it carries the shadow
  tables so PR-6 needs no schema change.
- **Safe:** additive `CREATE TABLE/INDEX IF NOT EXISTS` on empty tables; a few milliseconds; the
  current binary ignores the tables (m000004 precedent, `lib.rs:24–25`); no reader/writer exists
  yet, so no behavior can change.
- **Reversible:** `down` drops empty tables; zero data loss by construction. And because the
  writer has not landed, there is no one-way HMAC data to lose yet — the one genuinely
  irreversible property of this feature does not exist at this point in time.
- **Green gates:** `cargo test -p migration` (up-twice, down-then-up, defaults, UNIQUE,
  `PRAGMA table_info`), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt
--check`; and once PR-0 is merged, the restored CI job runs them on the PR.

Rejected alternatives:

- **CI-gate restore as "the first PR"** — it is a prerequisite (PR-0), not the feature; it is
  independent and should land first or alongside, but it is not a device-identity step.
- **Wiring the existing `user_bans` create/revoke/`check_ban` path (DESIGN §B1)** — arguably the
  highest-value first _enforcement_ PR and the right thing to do next, but it is bigger (admin
  API + guard behavior) and it is not reversible by dropping empty tables; it belongs as PR-5's
  sibling, not as the migration-safety step.
- **Collector first** — useless without a server that accepts the payload, and it invites the
  `AuthSessionState` compat bug (A3 #1) to be discovered in production instead of in a test.

---

## Findings

All marked **NEW** (not restatements of A3/A6/A7/A9/A10; each is a rollout/ops finding or a
correction of a report's claim).

1. **`up()` is not transactional on SQLite and the version row is written after it returns — a
   crash mid-migration re-runs the migration** · **critical** · NEW
   _Evidence:_ `sea-orm-migration-1.1.20/src/migrator.rs:261–272` (transaction only for
   `DbBackend::Postgres`; `MySql | Sqlite` run bare) and `:386–397` (version insert after
   `up()`). A3 §8 asserts the opposite ("sea-orm wraps **each** migration in a transaction").
   _Fix:_ m000012 all-`IF NOT EXISTS` + `PRAGMA table_info` guards, an up-twice test
   (`m000011:56–57` style), and optionally an explicit `conn.transaction` around the DDL batch.
   Never merge a non-idempotent migration in this repo.

2. **A failed migration is a boot panic → `restart: unless-stopped` crash loop, and the
   documented rollback path has no DB backup step (WAL makes a naive file copy invalid)** ·
   **high** · NEW
   _Evidence:_ `sea_connect.rs:59–69` (`panic!("Database initialization failed")`),
   `docker-compose.yml:193` (`restart: unless-stopped`), `:171–180` (healthcheck),
   `DEPLOYMENT.md:208–212` (rollback = pin `VERSION` only), `sea_connect.rs:48`
   (`journal_mode=WAL`); no backup script exists in the repo (grep).
   _Fix:_ runbook mandates a WAL-safe `.backup`/`VACUUM INTO` before any migration deploy and a
   restore rehearsal; abort criteria name the crash-loop signature.

3. **The pipeline auto-deploys every master push with zero test gates, so no rollout step is
   machine-verified** · **critical** · NEW (rollout framing of A6 #1)
   _Evidence:_ `images.yml:269–292` (deploy on `refs/heads/master` after `publish`), `53d3d421`
   ("fix(ci): remove security publication gates") deleted the `Web security gates`, `Rust
security gates`, and audit blocks added by `37d90c41`; `DEPLOYMENT.md:74–76` admits it.
   _Fix:_ restore the two gate blocks (web `check`/`lint`/`test`; rust `fmt`/`clippy`/`test`) and
   make `deploy` `needs` them; leave the audits out if they were the reason for removal.

4. **Two independent deploy triggers race the same stack, and the auto-update script's header
   contradicts the workflow** · **medium** · NEW
   _Evidence:_ `images.yml:286–292` (CI calls `compose.redeploy`) vs
   `deploy/dokploy-auto-update.sh:3–5` ("images.yml only builds and pushes — it never pokes
   Dokploy") and `:69–81` (its own `compose.redeploy` on a `*/10` cron). A rollback pin is safe
   from the cron (`:21–22`), but a redeploy can land mid-verification.
   _Fix:_ choose one trigger; disable the schedule during the rollout; fix the stale header.

5. **`DEPLOYMENT.md` documents two mechanisms the workflow does not have** · **medium** · NEW
   _Evidence:_ `images.yml:23` (`workflow_dispatch:` with no inputs — no "untick deploy"),
   `:271` (`if: github.ref == 'refs/heads/master'` — a manual master dispatch deploys), `:277–279`
   (requires `DOKPLOY_API_URL`/`DOKPLOY_API_KEY`/`DOKPLOY_COMPOSE_ID`) vs `DEPLOYMENT.md:22`
   (untick `deploy`), `:87–100` (`DOKPLOY_DEPLOY_WEBHOOK` preferred).
   _Fix:_ add a boolean `deploy` input to `workflow_dispatch` and accept the webhook secret, or
   correct the runbook.

6. **There is no staging environment; every stage of this rollout runs in production, and
   `APP_ENV=production` is a literal** · **high** · NEW
   _Evidence:_ `deploy/README.md:482` (a non-production instance is a suggestion for mail only),
   `docker-compose.yml:132` (`APP_ENV: production`), no second compose app in the repo.
   _Fix:_ either stand up a second Dokploy compose app (same images, separate volumes/DOMAIN) or
   accept prod-only and treat the 14-day shadow window as the staging gate (this document does
   the latter).

7. **Production has no metrics backend configured, so any "metrics" gate must be served from
   inside the repo** · **medium** · NEW
   _Evidence:_ `utils/telemetry.rs:247–252` (OTLP only when `ENABLE_QUICKWIT_OTEL`),
   `docker-compose.yml:120–152` and `.env.example` (no OTEL/Quickwit vars).
   _Fix:_ shadow status endpoint + SQLite aggregates as the source of truth; if a backend is
   wanted, set `ENABLE_QUICKWIT_OTEL`/`QUICKWIT_INGEST_URL` as an operator prerequisite (and note
   it is not part of the repo).

8. **The session-payload change and the writer must be ordered, or the first device deploy logs
   out every live session and 500s part of the API** · **high** · NEW (sequencing of A3 #1)
   _Evidence:_ `crates/rux-auth/src/session/state.rs:6–26` (no serde attrs), `extractor.rs:44–45`
   (anonymous) and `:239–241` (`SessionError`), `error.rs:71,175–179` (500); writer paths
   `auth_v1/controller.rs:183–190,347–351`, `passkey_v1/controller.rs:262–265`.
   _Fix:_ PR-2 lands fields with `#[serde(default)]`, append-only order, plus a legacy-payload
   decode test; PR-4 (the writer) follows.

9. **Shadow mode must be in-process counters + an off-path flush; `audit_logs` or any
   request-path write blows the single-writer budget and the flush must not ride the 10 s
   durability task** · **high** · NEW (rollout framing of A7 #1/#2, A10 #15)
   _Evidence:_ `rate_limit_store.rs:1` ("Do NOT add a DB write to the request hot path"),
   `translation_popularity_store.rs:1–5`, A7 #1 (1 fsync/commit → ~950 req/s ceiling),
   A7 #2 (the 10 s flush holds the connection 60–200 ms), A10 #15 (`DELETE … LIMIT` is a syntax
   error; do not contend with the flush).
   _Fix:_ `AtomicU64` + bounded ring buffer on the request path; a dedicated 60 s task
   (session-sweep precedent, `main.rs:764–775`) writes ≤500 rows/statement; a source-scan guard
   test asserts no sea-orm in the request-path function.

10. **Any new field on `HttpSettings`/`RateLimitSettings` breaks eight exhaustive struct literals
    plus a unit fixture; device config must be its own struct** · **medium** · NEW (rollout
    framing of A3 #14)
    _Evidence:_ `tests/admin_bans_export.rs:79–108`, `tests/quran_prod_gate.rs:74–103`,
    `tests/api_access_security_regressions.rs:76–105`, `tests/bookmarks_v1.rs:83–112`,
    `tests/auth_rotation_header.rs:130–163`, `tests/quran_v1.rs:74–103`,
    `src/middlewares/route_blocker.rs:187–216`, `src/config/settings.rs:1462`; precedent
    `main.rs:863–866` (`EscalationConfig` parsed independently).
    _Fix:_ `DeviceConfig::from_env()` in `config/settings.rs`, called early in `main.rs`; no field
    additions to the existing settings structs.
