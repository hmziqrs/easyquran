# Implementation Plan — closing the gap to `my-plan-raw.md`

**Status:** proposed (not started)
**Source of intent:** `docs/my-plan-raw.md` (owner-authored, never edited by agents)
**Source of current state:** `docs/quran-system.md`, plus the code ledger in §0

Each workstream states: **goal · files · steps · invariants · tests · done-when**.

---

## 0. Gap ledger

| # | Gap | Domain | Workstream |
|---|---|---|---|
| 1 | No durable cross-restart API-demand signal for translation prewarm | Rust | W1 |
| 2 | `IP_SOURCE=ConnectInfo` in deploy config; wrong behind CF→Traefik | Rust | W2 |
| 3 | Quran routes never escalate to a ban; no export, no un-ban path | Rust | W3 |
| 4 | Engagement counter is a sessionStorage int | Web | W4 |
| 5 | Arabic `readRange` has no API fallback; validator dropped on API path | Web | W5 |
| 6 | `RangeReader` has no client data path | Web | W6 |
| 7 | Translated routes intentionally have no SSG tier; degradation and recovery are incomplete | Web | W7 + W9 |
| 8 | Zero web-side auth; cache and origin prerequisites are incomplete | Both | W8 |
| 9 | Doc divergences unrecorded | Docs | W9 |
| 10 | Cross-cutting delivery and cache safety requirements | Both | W10 |
| 11 | Warm surah/range navigation still resolves SvelteKit server data before client data | Web/Docs | W6 + W9 |

### Already satisfied — no workstream needed

- **`my-plan-raw.md:4`** Arabic fully in memory at boot. True (`quran/loader.rs`;
  `main.rs:438,448`; `exit(1)` at `:455`).
- **`my-plan-raw.md:7`** no Redis. True and deliberate (L1 in-mem + L2 SQLite).
  **Standing constraint** — W1 must not erode it.

### Planning facts

- **"SQLite database is 1.5 MB"** — true for the pinned Arabic DB (`quran-uthmani.sqlite` =
  1,593,344 B). But time-to-SPA also pays `sqlite3-*.wasm` ≈ 864,752 B → real cold cost
  ≈ **2.4 MB**. Size any "how long is the degraded window" reasoning off 2.4 MB.
- **`my-plan-raw.md:5`** literally reads "We *do* load all translated database on boot…". In
  context and against the code the intent is the opposite. This document records that reading;
  the owner-authored file remains unchanged.
- **`my-plan-raw.md:13,18` is partially satisfied.** Surah first paint comes from server/static
  data and adjacent pages can fill from the worker, but every surah and range route still keeps a
  server load. SvelteKit resolves `__data.json` before new props, and a complete initial surah page
  is not replaced from the worker. W6 adds post-paint range recovery; W9 records the remaining
  route-architecture divergence instead of marking warm navigation independent of SSG/SSR.
- Reader route count is **1,296** (114 surah + 548 surah-local-page + 604 global page + 30 juz).
  The corpus tiles into 662 surah-local pages, but localPage 1 is not a route: `entries()` emits
  `2..=n` and the loader 308-redirects `localPage === 1` to the surah root
  (`app/[surah]/page/[localPage]/+page.server.ts:9-16,25`). Counting 662 double-counts 114 surah
  roots. Build output confirms 1,308 prerendered `__data.json` = 1,296 reader + 12 non-reader.

### Global invariants

1. **Quran DBs immutable.** 2. **No SHA-256 over Quran data** (identity is the id; guarded by
`tests/quran_v1.rs` + `catalogue-sha-guard.test.ts`). 3. **Arabic pinned, only translations
evictable.** 4. **Reader hrefs use the `*For(ctx, …)` family** (guarded by `nav-guard.test.ts`).
5. **No per-user content in any shared cache.** 6. **No Redis.**
7. **`my-plan-raw.md` is read-only** — divergences go in W9.

---

## W1 — Durable API-demand prewarm (Rust)

**Goal:** preserve the existing bounded TTL + TinyLFU pool while carrying enough API-demand
evidence across restarts to prewarm likely cold-start translations (`my-plan-raw.md:5`).
**Files:** `quran/translation_pool.rs`, `services/{mod.rs,translation_popularity_store.rs}`,
`migration/src/{lib.rs,m000003_translation_popularity.rs}`, `config/settings.rs`, `main.rs`,
`modules/quran_v1/{controller.rs,dto.rs}`, `quran/mod.rs`, `deploy/.env.example`.

**Today:** TinyLFU already supplies in-process frequency-aware admission, with an idle TTL and a
post-build byte-bound LRU prune (`translation_pool.rs:76-97,168-193`). That satisfies bounded
runtime popularity. Missing piece is durable evidence for restart prewarm.

### What the durable signal means

The pool only sees requests reaching the Rust process. Translated pages are SSR behind a 7-day
HTML disk cache, and warm clients read their local worker DB. So pool hits measure **API
demand** — a more popular translation may produce *fewer* pool hits once caching works.

The pool exists to serve API demand, so name the persisted value `api_demand_score`, not
"reader popularity" (recorded in W9).

**Design decision:** durable demand governs **restart prewarm only**. Moka TinyLFU continues to
govern runtime admission; TTL/LRU/byte-bound govern eviction. Durable collection and prewarm are
one supported runtime contract, each independently operable through the settings in step 7.

### Steps

1. **Definition of a hit:** one increment per *successful* `get_or_build` call — i.e. per API
   request that needs and obtains the corpus. Not per ayah, not per surah. The function returns
   early on a catalogue miss (`:135-139`) and on build error (`:161`), both before the increment
   site, so neither contributes. This differs from `lookups` (`:131`), which counts invalid ids
   too.
2. **Counter is a lock-free side table, disjoint from cache state.** The catalogue is fixed at
   construction (`translation_pool.rs:66-70`, 115 entries), so the counter needs no growth and no
   lock: build `demand: HashMap<TranslationId, AtomicU64>` once in `new()`, pre-populated with one
   entry per catalogue id, and `fetch_add(1, Relaxed)` at the acquisition that already happens
   (`:162-163`).

   **The counter must not live in `residents`.** That map is the byte-bound victim index, and
   coupling the two is what forces every hard problem here. Widening `ResidentMeta` with a `hits`
   field means eviction must either drop undrained counts or leave a tombstone behind, and a
   tombstone is an entry in the victim index that is *not* in the cache:
   `enforce_byte_bound` picks `residents.iter().min_by_key(tick)` (`:179-186`), so
   `cache.invalidate(victim)` becomes a no-op, `resident_bytes` never drops, `victim` stays
   `Some`, the `break` at `:187` never fires, and the loop spins forever holding `prune_sem`
   **on the request hot path** (`:164`). Skipping `evicted` entries during selection does not
   close this: the entry that matters is created by a hit racing a *late* eviction — `:161`
   returns a hit, another task's `run_pending_tasks` evicts and tombstones the id, then the
   insert at `:163` clears the flag and stamps the newest tick. The result is a live-looking
   meta for an id absent from the cache, and the same spin.

   A side table has none of this: it never interacts with eviction, cannot grow, and cannot
   describe a residency that does not exist. Losing a count is not a concern worth a tombstone —
   the persisted value is a decayed heuristic, not an accounting ledger — and in fact **no count
   is lost**, because the side table outlives eviction entirely.

   **Leave the `residents` data shape and eviction listener unchanged** (`:90`). Make
   `enforce_byte_bound` self-healing: filter on
   `cache.contains_key` **inside** victim selection, and when the selected id turns out to be
   absent, `residents.remove(&victim)` before continuing. Checking membership without removing is
   not enough — selection is a deterministic `min_by_key`, so the same dead id is re-selected on
   every pass, consuming any iteration bound and leaving the byte bound silently unenforced for
   the rest of the process lifetime. `contains_key` is safe to call here: it is synchronous and
   deliberately does not touch the TinyLFU estimator or reset the idle timer.

   **Lock-ordering rule:** never hold the `residents` guard across an `.await` that can re-enter
   moka or SQLite. The listener is awaited inline inside `invalidate()` under moka's per-key lock
   (`:190`) as well as from `run_pending_tasks()` on the request hot path (`:175,192`) and on the
   health path (`:196`, inside `stats()`), so
   any guard held across those calls is both a latency coupling and a deadlock candidate. The
   flush reads atomics and never takes the guard at all.
3. **Durable store, owned by migrations.** Add
   `m000003_translation_popularity.rs`, after W3b's reserved
   `m000002_rate_limit_state.rs` — **not** raw DDL at boot.

   ```sql
   CREATE TABLE translation_popularity (
     id TEXT PRIMARY KEY, score REAL NOT NULL DEFAULT 0,
     hits_total INTEGER NOT NULL DEFAULT 0,
     last_hit_at INTEGER, updated_at INTEGER NOT NULL
   );
   ```

4. **Flush wiring and write budget.** The existing rate-limit flush is spawned
   at `main.rs:136`; the pool is built at `main.rs:459-492`, so the task closure cannot capture
   a pool that does not exist yet. Move the spawn below pool construction and pass a third
   clone. The call already passes `sea_db.clone()`, so that argument is unaffected; the clone to
   take is `translation_pool`, which is moved into `AppState` at `:519`.
   Only the spawn moves — `restore()` is a separate statement at `:135` and stays put, so
   restore-before-serve ordering is unaffected. Nothing between `:136` and `:492` depends on
   the flush task running (only `gate_store` clones at `:171`; no requests are served yet).
   Run popularity on every 6th tick (≈60 s) of the existing 10 s task rather than spawning a
   second task — a second flush task on the one shared SeaORM connection adds write
   amplification and cuts against invariant 6.
5. **Snapshot, don't drain; commit as one transaction.** `load()` each atomic into a local
   snapshot, execute one SQLite transaction for all rows, then `fetch_sub` exactly the snapshotted
   amount per id. A failed transaction leaves every count intact and the next tick retries;
   `fetch_sub`-after-commit means hits accrued during the write are preserved rather than lost.
   Skip ids with `committed_hits == 0` — decay happens at read, so rewriting an untouched row
   stores no information. W3b applies the same one-transaction rule to the existing rate-limit
   flush (`services/rate_limit_store.rs:89-105`). Per-row autocommits on the shared connection are
   forbidden for both stores.
6. **Persist one precise score invariant.** `score` is the value at `updated_at`. On a committed
   flush at `now`, compute in Rust:
   `score_now = score * 0.5f64.powf((now - updated_at) / HALF_LIFE_SECS) + committed_hits`, then
   set `updated_at = now`, increment `hits_total` **by `committed_hits`**, and set
   `last_hit_at = now`. Clamp negative elapsed time to zero. Reads apply the same decay from
   `updated_at` to read time. This prevents one new hit from making an old score fresh again.
   Inside the flush transaction, select the stored row, compute `score_now` in Rust, then write
   the complete new values with `INSERT … ON CONFLICT DO UPDATE`. The decay reference point is
   always the row's own `updated_at`.

   `pow()` / `exp()` / `ln()` **do not exist in this SQLite build** —
   `sqlx-sqlite` bundles `libsqlite3-sys 0.30.1` without `SQLITE_ENABLE_MATH_FUNCTIONS`, so
   `ORDER BY score * pow(...)` fails at runtime with `no such function: pow`. The table is
   ≤115 rows, so `SELECT` all and decay in Rust:
   `decayed = score * 0.5f64.powf(elapsed_secs / HALF_LIFE_SECS)`, `HALF_LIFE = 7 days`
   (const, not env). Decay **at read**, so rows that stopped being hit don't keep a frozen
   score and outrank recently-popular ones forever.
7. **Prewarm — bounded, non-self-reinforcing, yielding.** Two independent switches:
   `QURAN_DEMAND_COLLECT` (default on) governs counting and flushing;
   `QURAN_PREWARM_TRANSLATIONS` (default `2`, `0` = off) governs prewarm only. Collection off
   implies prewarm off; the reverse does not hold. Background `tokio::spawn` after pool
   construction:
   - `N = min(QURAN_PREWARM_TRANSLATIONS (default 2), max_resident_translations)`;
   - **must not increment the demand counter** (else prewarmed ids gain score every boot with
     zero traffic and the top-N freezes) **nor `lookups`** (`:131` — it would corrupt `hit_rate`);
   - **must stamp `tick = 0`** — `enforce_byte_bound` evicts the lowest tick (`:179-186`), so a
     fresh prewarm tick would evict a corpus a real user is reading;
   - the post-build merge may set `tick = 0` only when no `residents` entry exists yet; a real
     request racing the warm must keep its newer tick;
   - **must select only ids present in the current catalogue.** The durable table outlives
     catalogue changes, so a top-N read can return an id that no longer exists; it would fail at
     `path_for` (`:135-139`) and silently waste one of N prewarm slots. Filter by catalogue
     membership *before* truncating to N, and let the flush drop non-catalogue rows;
   - **must yield** while `build_sem.available_permits() < BUILD_CONCURRENCY`: each load holds
     one of two permits (`:149-152`) and each iteration takes `prune_sem` and runs
     `run_pending_tasks` twice. Give the yield loop a deadline — the condition is racy against
     the acquire at `:149-152` and a busy boot could otherwise starve prewarm indefinitely.
     Several prewarmed ids all carrying `tick = 0` tie under `min_by_key`; victim order among
     them is arbitrary and that is acceptable;
   - **must not block or fail boot.** Arabic boot is fail-fast; translations are not.
   Implement as a private `warm(id)` path that bypasses the metric increments rather than
   calling `get_or_build` directly.
8. **Effectiveness evidence.** `time_to_idle` defaults to 1800 s
   (`settings.rs:162`), so prewarmed entries evaporate 30 min after boot unless real traffic
   arrives. At each boot, retain the ranked candidate set chosen before any warm runs. Emit OTLP
   counters with a boot identifier for: all real cold builds, real cold builds for that candidate
   set within the idle TTL, successful prewarm builds, and first-request prewarm hits. Prewarm's
   own builds never enter real-cold-build counters — `builds` is incremented inside the `init`
   closure (`:158`) that `warm(id)` also uses. This measures whether the selected top-N prevented
   cold requests, rather than treating every unrelated early cold build as prewarm value.
9. **Observability contract.** Add `prewarmed: Vec<String>` and
   `top_demand: Vec<(String, f64)>` (cap 10) to `PoolStats`, `TranslationPoolHealth`, its OpenAPI
   shape, and `/quran/health/ready`. This drops `PoolStats: Copy`; keep `Clone`. Change
   `hit_rate` in both pool and DTO to `Option<f64>` serialized as `null` when `lookups == 0`,
   instead of today's misleading `1.0`.

   `stats()` must not read SQLite. The pool holds no `DatabaseConnection`, and the health route
   is on the request path, so `top_demand` reads a ranked snapshot the flush task leaves in a
   `std::sync::RwLock<Vec<(String, f64)>>`; the guard is never held across `.await`. Ranking is
   computed during the flush; the health path only clones the snapshot.

   Two existing assertions encode the old `hit_rate` contract and change with it:
   `tests/quran_v1.rs:498` asserts `hitRate == 1.0` while `lookups == 0` and becomes an
   `is_null()` assertion, and `translation_pool.rs:304` asserts `stats.hit_rate > 0.0` and must
   unwrap. Both are part of this step, not incidental fallout.

**Invariants:** resident metadata shape and eviction-listener behavior stay unchanged; the prune
loop intentionally removes stale victim-index entries; demand counting stays lock-free; the only
new lock is the read-mostly health snapshot and no guard crosses `.await`; no second flush task;
no Quran-DB writes (`translation_popularity` lives
in the app DB, `sqlite:./data/easyquran.db`, entirely separate from `db/quran/tanzil/**` — the
immutability rule is not touched).

**Tests:** decay halves across one half-life; a 30-day-old score plus one hit does not become a
fresh undiminished score; ordering prefers a recent lower score over an old high one; failed
transaction leaves every count intact and the next tick retries; counts accrued during a flush
survive `fetch_sub`; a demand count survives eviction and re-admission of the same id;
`enforce_byte_bound` terminates when a selected victim has already left the cache **and** drops
its stale `residents` entry, so the next call can still enforce the bound; prewarm clamps to
`max_resident_translations`, increments neither demand nor `lookups`, stamps tick 0; prewarm
skips ids absent from the current catalogue; prewarm on a bogus id warns without panicking; the
boot candidate counters include only matching real cold builds and exclude prewarm builds; the
two `hit_rate` assertions above move to the nullable contract and every other existing pool test
passes unchanged.

**Done when:** restart warms top-N by decayed API demand, metrics report actual candidate value,
`/quran/health/ready` uses the nullable/snapshot contract, and the byte bound self-heals without
changing TinyLFU admission or TTL behavior.

---

## W2 — Production client-IP correctness (Rust)

**Goal:** external rate limiting keys on the verified Cloudflare client identity, trusted
Docker-internal SSR has its own bounded service identity, and health never shares content or
escalation buckets.
**Files:** `crates/rux-request-gate/src/{ip.rs,layer.rs}`, `config/settings.rs`,
`middlewares/{route_blocker.rs,client_ip.rs,rate_limit.rs}`, `state.rs`, `utils/telemetry.rs`,
`db/sea_models/route_status/actions.rs`, `router.rs`,
`main.rs`, `web/src/lib/server/quran-translation-page.ts`, `deploy/.env.example`,
`docker-compose.yml`, `deploy/README.md`, `web/scripts/assert-headers.sh`.

**Today:** `settings.rs:98` defaults to `ConnectInfo` and `deploy/.env.example:42` ships
`IP_SOURCE=ConnectInfo` **uncommented**. Behind CF→Traefik every request carries the proxy's
IP, so per-IP limiting collapses into one global bucket. **The bug is set-but-wrong, not
unset** — a guard firing only on "unset" catches nothing.

### Steps

1. **One `is_production()` helper.** No such helper exists yet — `state.rs:107-114` is an inline
   `let is_prod` inside `derive_field_enc_key`, reading `RUST_ENV → NODE_ENV → APP_ENV`, and
   `route_blocker.rs:68-69` reads **only** `APP_ENV`, defaulting to `"development"`.
   (`utils/telemetry.rs:99-102` also reads `DEPLOYMENT_ENVIRONMENT`, but that is an OTLP resource
   label, not a gate; feed it from the same helper for consistency, nothing branches on it.)
   Since `deploy/.env.example:36` sets `RUST_ENV=production` and never `APP_ENV`, **the route
   blocker is disabled in production right now.** Extract the helper and convert `route_blocker`
   to it.

   Enabling the route blocker adds two SQLite round-trips per private-router request
   (`record_route_pattern` → `RouteStatus::ensure_exists`, `route_blocker.rs:79`, and
   `is_route_blocked`, `:89`) against the same DB file as `rate_limit_state`; it logs
   `info!("ROUTER BLOCKER WORKING")` per request (`:76`); and it is fail-closed, returning
   `CheckFailed` on any DB error (`:99-104`). `/healthz` sits on the private router
   (`router.rs:48`) and is the compose healthcheck target, so a DB hiccup becomes a restart loop.
   Drop the per-request log to `debug!` and exempt health per step 3. Route-status lookup remains
   **fail-closed** for matched private routes: only a successful `Ok(false)` proceeds. Route-pattern
   recording remains best-effort. A lookup failure returns the existing `CheckFailed` response and
   emits a structured error counter.

   Note the default flips direction. `state.rs:107-114` is a deny-list — unset means
   *production* — while `route_blocker.rs:69` treats unset as development. Adopting the helper
   enables the blocker in every environment that sets none of the three variables, including
   test runs that never call the setters at `state.rs:281-295`. Make the test harness set an
   explicit non-production value.
2. `HttpSettings::from_env` returns `Result`, never panics on `IP_SOURCE`. Define environment
   precedence once: `RUST_ENV → NODE_ENV → APP_ENV`; production is `production`, accepted
   non-production values are `development|dev|test|testing|ci|local`, and unset/unknown values
   are configuration errors outside tests.
3. **Typed identities, resolved before rate limiting.** Add
   `RequestIdentity::{External(IpAddr), InternalService(InternalServiceId)}` as a request extension;
   the gate resolves this type instead of encoding a service name as a fake IP.
   - external request: require `CF-Connecting-IP` in production and store its parsed `IpAddr`;
   - Bun SSR request: require a constant-time match on server-only
     `X-EasyQuran-Internal-Token`, assign `InternalService(WebSsr)`, and use a separate
     non-escalating limiter configured by `QURAN_INTERNAL_REQUESTS_PER_MINUTE`;
   - `/healthz`: expose only on the Docker/internal monitoring path, outside route blocker and
     content limiters;
   - `/quran/health/ready`: use a separate non-escalating health limiter
     `QURAN_HEALTH_REQUESTS_PER_MINUTE` and never enter content/escalation state.
   Missing/invalid internal token never grants internal treatment. Missing CF header on an
   external production request is rejected, never collapsed into `unknown`.
4. Generate `INTERNAL_QURAN_API_TOKEN` per deployment. Bun reads it from private runtime env and
   sends it only to `INTERNAL_QURAN_API_BASE`; it never enters a public env variable, response,
   or log. Compose passes it to web and API containers.
5. Production boot with `ConnectInfo`, missing internal token, a non-positive internal limit, or
   a non-positive health limit returns a configuration error and exits non-zero. Log selected
   external source and both policies without logging the token.
6. Set `deploy/.env.example` to `IP_SOURCE=CfConnectingIp` — **exact PascalCase**;
   `cf-connecting-ip` is invalid. `docker-compose.yml` passes the environment to the API through
   `env_file: [.env]`.

### Trust, not just parsing

`CfConnectingIp` trusts the header. Before this configuration reaches production, Traefik or the
host firewall must restrict public origin ingress to current Cloudflare ranges.
`deploy/README.md` owns the range-update procedure and direct-origin negative test.
`assert-headers.sh` proves that a public caller cannot choose `CF-Connecting-IP`, an external
request without the header is rejected at origin, internal SSR succeeds only with the token,
the Docker health route is not publicly reachable, and public readiness uses only its isolated
health bucket. W3a remains disabled until this contract passes.

**Invariants:** only a verified external `IpAddr` enters external rate/escalation state; an
internal-service identity never masquerades as an IP; health shares neither content limits nor
route-blocker state; production configuration fails closed.

**Tests:** serialized env tests for production/dev/unset/unknown values; prod + `ConnectInfo` →
`Err`; prod + `CfConnectingIp` → `Ok`; dev + `ConnectInfo` → `Ok`; route blocker enabled under
`RUST_ENV=production`; external missing/spoofed CF header rejected; valid internal token gets
the service bucket; invalid token gets no privilege; internal, external, and readiness buckets
are isolated; Docker health never enters middleware state; route-blocker DB errors remain
fail-closed without affecting health.

**Done when:** production boots only with the complete ingress contract, route blocker is live,
external requests carry typed verified identities, internal SSR has its own bounded policy, and
health cannot enter a content, route-blocker, or escalation bucket.

---

## W3 — Ban escalation, persistence, inspection, and export (Rust)

**Goal:** `my-plan-raw.md:7` — persist longer IP bans, make them operable, and expose a safe
machine-readable feed for a deployment-owned proxy adapter.
**Files:** `Cargo.toml`, `crates/rux-request-gate/src/{layer.rs,store.rs,abuse.rs}`,
`services/rate_limit_store.rs`, `migration/src/m000002_rate_limit_state.rs`,
`migration/src/lib.rs`, `modules/{mod.rs,admin_bans_v1/}`, `modules/quran_v1/error.rs`,
`config/settings.rs`, `router.rs`, `docs.rs`, `main.rs`, `deploy/.env.example`,
`deploy/README.md`. Add the `ipnet` dependency explicitly.

**Today:** Quran routes have fixed-window limiting only. Existing Temp/Long abuse buckets lose
scope on restart, no side-effect-free ban lookup exists, `RateLimitStore::del` removes dedup
claims rather than limit buckets, and no operator API exists.

**Dependency order:** W3b → W3c → W3a. W3a stays disabled until W2 and W3c are complete.

### W3b — Migration-owned persistence

1. Add `block_scope` to `BucketSnapshot`, snapshot, flush, load, and restore. Unknown stored
   values fail closed as `Temp` and emit a warning; valid values round-trip exactly.
2. Add reserved migration `m000002_rate_limit_state.rs`. If `rate_limit_state` is absent, create
   the five-column table. If it already exists, inspect
   `PRAGMA table_info(rate_limit_state)` and conditionally add
   `block_scope TEXT NOT NULL DEFAULT 'Temp'`. Register the migration before replacing the
   boot-owned table creation, and keep migration-before-`load()` ordering.
3. Introduce one async persistence-operation lock shared by periodic snapshot/flush and admin
   mutations. Never hold the in-memory bucket mutex during SQLite I/O.
4. Expired rows are deleted on the next successful flush; active keys remain only until their
   fixed or block expiry. This retention rule covers IP, email, and user-id key classes. Only
   successfully parsed active IP ban keys may appear in an IP export.
5. Batch the flush into one transaction. `flush` currently issues one `DELETE` and then a
   per-row `INSERT … ON CONFLICT` in a bare loop with no transaction
   (`services/rate_limit_store.rs:80-107`), which is tolerable while rows are bounded by active
   rate-limit windows and becomes a per-10s write storm once W3a adds a long-lived row per banned
   identity. Same rule as W1 step 5: one transaction on the shared connection, per-row
   autocommits forbidden. `QURAN_ACTIVE_BAN_MAX` defaults to `2_000`. Flush prunes expired rows
   first and never evicts an active ban. At capacity it keeps existing active bans, declines
   creation of new ban state in both L1/L2, continues fixed limiting, and increments a saturation
   counter.

**Tests:** fresh migration; legacy table with rows gains scope without data loss; migration is
idempotent; `Long` survives restart; invalid scope restores as `Temp`; expired rows purge.
**Done when:** fresh and existing installs share one migration-owned schema and scope survives
restart without changing Quran DBs.

### W3c — Inspect, export, and lift bans

1. Add distinct store operations: `ban_status(key)` performs a read-only active-ban lookup;
   `clear_limit(key)` removes a limit bucket; dedup `del(key)` keeps its current claim semantics.
2. Define one `BanUnit`: IPv4 becomes canonical `/32`; IPv6 becomes canonical `/64`. External
   fixed limiting, suspicious history, active bans, persistence, inspection, and export all use
   this same unit. `GET /admin/bans` is admin-ACL gated, `no-store`, paginated, and returns active
   `{banUnit, scope, blockUntilAt}` rows only.
3. `DELETE /admin/bans` accepts JSON `{banUnit}` rather than putting CIDR text in a path segment.
   It acquires the persistence-operation lock, deletes exact `quran-ban:{banUnit}` and
   `ratelimit:{banUnit}:quran-v1` L2 rows in one transaction, then clears the corresponding L1
   buckets and suspicious history before releasing the lock. This prevents stale periodic
   snapshots from resurrecting a lifted ban and prevents the old fixed count from immediately
   recreating it. A DB failure returns failure without clearing L1, so the ban remains enforced
   and the logged operation is safe to retry.
4. `GET /admin/bans/export` returns neutral JSON containing active canonical ban units, scope,
   and expiry.
   Human access uses admin ACL. Machine access uses a separate read-only `BAN_EXPORT_TOKEN`
   compared in constant time; the token is never accepted by mutation routes. Never export
   `totp:*`, email, user-id, fixed-rate, or unparseable keys.
5. Traefik/Cloudflare mutation stays outside this repository. `deploy/README.md` documents the
   JSON contract and requires any deployment adapter to preserve expiry and un-ban semantics;
   this plan does not claim a proxy block exists merely because an export exists.

**Tests:** list/export exclude email and user-id keys; IPv4 `/32` and IPv6 `/64` round-trip through
JSON without URL encoding; invalid export token fails; successful delete clears L1/L2, suspicious
history, and fixed count; failed L2 transaction preserves the active L1 ban; barrier-controlled
flush/delete race cannot recreate the row; restart after delete does not restore it.
**Done when:** operator can inspect and lift bans without DB edits and a proxy adapter has a
stable, non-PII-leaking feed.

### W3a — Escalate suspicious repeated rate blocks (default off)

`on_block` remains a synchronous response hook and is not used for store mutation. Escalation
runs in `RateLimitMiddleware::call`, where resolved identity and store are available.

1. Add explicit config:
   ```rust
   struct EscalationConfig {
     enabled: bool,                    // QURAN_BAN_ESCALATION_ENABLED, default false
     key_prefix: &'static str,         // "quran-ban"
     temp_after: u32,                  // 5 qualifying blocked windows
     temp_window_secs: u64,            // 3600
     temp_duration_secs: u64,          // 3600
     long_after: u32,                  // 20 qualifying blocked windows
     long_window_secs: u64,            // 86400
     long_duration_secs: u64,          // 604800
     suspicious_4xx_per_window: u32,   // 20
     max_tracked_identities: usize,     // 10_000
     max_active_bans: usize,            // 2_000
     allowlist: Vec<IpNet>,
   }
   ```
   Parse `QURAN_BAN_ALLOWLIST` as CIDRs at boot; enabled + missing/invalid allowlist is a boot
   error. Reject IPv6 allowlist prefixes narrower than `/64`, because proxy export blocks `/64`
   units and cannot preserve a narrower exception. Match the raw client address before
   normalization; any allowlisted unit never escalates or appears in export. Internal service
   identities and health routes can never enter this state machine. An identity that is not an
   external `IpAddr` never escalates.
2. Resolve the canonical `BanUnit` before the fixed-window increment and call side-effect-free
   `ban_status(quran-ban:{banUnit})`. The fixed key is
   `ratelimit:{banUnit}:quran-v1`. An active
   Temp/Long ban immediately returns 429 for every request, including after fixed-window rollover.
3. Attach a private typed classification extension before Quran errors are serialized. The outer
   limiter consumes that extension; it never parses JSON or message text. Record suspicious Quran
   4xx events in a bounded per-unit/window counter for a closed set:
   unknown source id, invalid range bounds, and unknown Quran route. Exclude normal search
   validation, ordinary not-found content, 5xx, and successful reads.
4. A rate window qualifies only when `count == max_requests + 1` **and** the suspicious counter
   meets its threshold. That equality is the one-event-per-window primitive; do not use
   `dedup_nx` or append on every 429. Raw volume alone never creates a ban.
5. `record_block_event` prunes old qualifying events, evaluates Long before Temp, and upgrades
   to Long when the long threshold is met. Active-ban lookup never appends events. Escalation
   history is L1-only; active ban scope/expiry is L2-persistent. Tracking holds at most
   `QURAN_ESCALATION_MAX_IDENTITIES` units (default `10_000`) with at most `long_after` timestamps
   per unit. At capacity prune expired and non-banned LRU history, never an active Long ban; if
   still full, decline new history/new ban creation, keep fixed limiting, and emit the saturation
   counter.
6. Wire only the outer `quran-v1` branch limiter. Search remains under its own 30/min limiter and
   the outer 600/min ceiling but contributes at most one outer qualifying event per window.
7. Prune expired `claims` in the existing store as W10b; escalation itself does not use claims.

**Hard gates:** W2 ingress contract · W3b migration · W3c un-ban · valid CIDR allowlist · exact
key namespace · default-off flag.

**Invariants:** one canonical `BanUnit` keys fixed limiting, history, bans, persistence, and
export; active bans are never capacity victims; un-ban commits durable deletion before clearing
memory; non-external identities and non-IP keys never enter escalation or export.

**Tests:** flag off preserves existing behavior; active ban blocks request 1 after rate-window
rollover and after restart; volume without suspicious shape never bans; qualifying windows reach
Temp then Long; Long wins when both thresholds match; active Temp can later upgrade after new
qualifying windows; flood stores one event per window; typed error classes exclude ordinary 4xx;
capacity saturation preserves active bans and fixed limiting; allowlisted/internal identities
never ban.

**Done when:** suspicious sustained abuse creates enforceable persistent bans, raw legitimate
volume cannot ban a CGNAT range, and every ban is visible and safely removable.

---

## W4 — Engagement: one-time viewer vs continuous reader (Web)

**Goal:** `my-plan-raw.md:21`.
**Files:** `lib/quran/engagement.ts`, `lib/quran/engagement-state.ts` (new),
`lib/storage/{safe-storage.ts,decoders.ts}`, `lib/stores/reader-persistence.svelte.ts`,
`lib/workers/download.ts`, engagement tests.

**Today** (`engagement.ts:15-20,83`): one `sessionStorage` integer, threshold 2. Resets every
tab, so a daily reader never accumulates while two clicks in one tab reads as "continuous".
Arabic views inflate the counter toward a *translation* download (`:68-70`).

### Design

- **`readerViews`** (all sources, `localStorage`): is this person a reader at all?
- **`sourceViews[id]`** (`localStorage`): has this person used *this* translation?

```ts
interface EngagementState {           // localStorage "eq:engagement"
  v: 1; totalViews: number; distinctDays: number;
  lastDay: string;                    // YYYY-MM-DD local
  firstSeen: number; lastSeen: number;
  qualified: boolean; legacySeeded: boolean;
  sourceViews: Record<string, number>;
}
isEngagedReader() = qualified || distinctDays >= 2 || totalViews >= 4;
```

`sessionViews` is tracked but is **not** a disjunct. Because `totalViews` includes the current
session, `sessionViews >= 3` can only fire at `totalViews == 3` — one view earlier than the
`totalViews >= 4` branch — so its sole effect is to admit a first-time visitor who opens three
ranges in one sitting. That is exactly the one-time viewer this workstream exists to exclude;
keeping the branch would move the defect from two clicks to three rather than removing it. The
counter stays because the settle/retry guards and diagnostics read it.

`sessionViews` lives under `sessionStorage "eq:reader-session-views"`. The legacy
`eq:reader-views` key is read once during migration, seeds both `totalViews` and
`sessionViews`, then is deleted only after the engagement blob containing `legacySeeded: true`
is written and read back successfully. The marker lives in the blob, so a partial cross-storage
write cannot lose the legacy value or seed it twice.

**Gate:** `isEngagedReader() && preBumpSourceViews[id] >= 1` — explicitly the **pre-bump**
value; reading post-bump would make it always true on first exposure and the second signal
would do nothing.

### Steps

1. New `lib/quran/engagement-state.ts` — load/save/bump. **Never read storage at module
   scope**: `readJSON` no-ops under SSR/prerender (`safe-storage.ts:4`), so a module-scope read
   evaluates to `undefined` on the server, and in the browser it snapshots once at hydration and
   never observes a later write. Read lazily inside the call.
2. **Validate the shape explicitly.** `isFutureSchema` returns `false` for an object with no
   `v` key (`safe-storage.ts:56-59`), so a legacy or garbage blob passes as valid current
   schema. Check field types with the existing `lib/storage/decoders.ts` helpers
   that `reader-persistence`, `prefs`, and `consent` already use. Add
   `asNumberRecord(raw, min, max)` for non-negative safe-integer `sourceViews`; `asStringRecord`
   is not valid for numeric counts.
3. **Keep the read-modify-write synchronous** — no `await` between `readJSON` and `writeJSON`,
   re-read immediately before writing. `localStorage` RMW is non-atomic across tabs; the
   current sessionStorage design has no such hazard, so this migration introduces it. (Do not
   add a `storage`-event sync; last-writer-wins on a view counter is acceptable.)
4. **Monotonic day guard:** increment `distinctDays` only when `today > lastDay`
   lexicographically. DST is harmless; a clock set forward then back would otherwise
   double-count.
5. `sessionViews` uses the new session key above. Migration order is: read validated durable
   state → read legacy session value → seed durable total only when `legacySeeded` is false →
   initialize new session counter → write the durable blob with `legacySeeded: true` → read it
   back → delete the legacy key only after confirmation. A failed write leaves the legacy key for
   retry.
6. Bump `readerViews` always; `sourceViews[id]` only for translation sources. Seed from durable
   reader evidence without inventing dates: a non-empty bookmark or note set in `easyquran.reader`
   sets `qualified = true`; `lastRead` has no timestamp and never changes `distinctDays`.
   `easyquran.reader.source` (`reader-settings.svelte.ts:53`) seeds one `sourceViews[id]` use for
   the chosen translation. Invalid/Arabic source ids do not seed translation counts.
7. Keep `PREFETCH_PREFIX` settle markers in `sessionStorage` and every existing guard:
   `saveData`/2g (`:27-32`), `whenIdle` (`:34-38`), one retry per source per tab (`:45-50`).
8. `noteTranslationChosen` (`:96`) keeps bypassing the gate — an explicit pick beats a heuristic.
9. **Migration is recoverable across storage areas.** The legacy
   `eq:reader-views` counter is **sessionStorage** (`engagement.ts:6,16-18`) — per-tab,
   dies with the tab. Follow step 5's write/read-back/delete protocol; JavaScript call ordering is
   not transactionality across localStorage and sessionStorage. Settle markers (`eq:tprefetch:*`)
   are untouched.
10. **Give `downloadBytes` a full-transfer timeout and byte ceiling.** One AbortController stays
    active through headers and complete body streaming, not only until `fetch()` resolves. Abort
    when elapsed budget expires or bytes exceed declared `sizeBytes`; always cancel reader and
    clear timer. One retry remains governed by existing per-source session logic.

**Do not** write a "`readJSON` throws" test — `safe-storage.ts:3-11` catches internally and
returns `undefined`. Test the `undefined` path.

**Invariants:** migration never deletes the legacy value before confirmed durable read-back;
timestamp-free state never invents a visit day; translation gating reads the pre-bump source
count; explicit source choice remains authoritative; no Quran data is modified or hashed.

**Tests:** one view/one day → no prefetch; four views same day → prefetch; two views across two
distinct days → prefetch; Arabic-only views never prefetch; `readJSON` → `undefined` falls back
to session-only without throwing; legacy key seeds `totalViews` once; gate
reads pre-bump `sourceViews`; numeric source counts round-trip; timestamp-free `lastRead` never
creates a day; bookmarks/notes qualify and chosen translation seeds only its source;
`downloadBytes` aborts on timeout. Failed durable write retains the legacy key; repeat-load
migration seeds durable history once while the new session counter continues; an oversized
stream aborts before unbounded allocation.

**Done when:** a reader returning on a second day gets the translation prefetched, a one-time
visitor does not, and no code path can hang forever on a download.

---

## W5 — Unify and repair the fallback chain (Web)

**Goal:** `my-plan-raw.md:14` — "fall back to API first".
**Files:** `lib/quran/{offline.ts,worker-client.ts,api-client.ts,fetch.ts,range-fetch.ts}`,
`lib/server/quran-translation-page.ts`, `app/_reader/SurahReader.svelte`,
`src/service-worker.ts`, browser + server loader tests.

**Today:** translations get an ordered chain (`worker-client.ts:108-156`); Arabic gets ad-hoc
early-returns — `readSurah:259-269` hits the API only if the worker was never constructed;
`readRange:286-291` is worker-only with no fallback. Translated range SSR bypasses
`quranApi.readRange` and sends one request from `quran-translation-page.ts:32-49`, so juz
19/23/27/29/30 exceed the backend cap and degrade to empty data even while the API is healthy.

### API cache boundary

**The SW `/quran/` bypass is already dead in production.** `service-worker.ts:252` matches
`/quran/`, but prod `PUBLIC_QURAN_API_BASE=https://easyquran.fyi/api/quran`
(`deploy/.env.example:20`) → real paths are `/api/quran/…`, which falls past all five bypasses
into `swrApp()` and lands in **`eq-app-${version}`**. Quran API JSON is being cached there
today. Add an unconditional same-origin `/api/` bypass before adding fallback traffic. Every API
GET uses browser/HTTP-edge cache policy only; no API response enters `eq-app-*`, `eq-pages-v1`,
or `eq-data-v1`.

Two boot metadata calls also use `/api/quran`: `/scripts` and `/sources`. They must not delay
worker construction. Start from baked manifest/catalogue immediately, then refresh validated
metadata asynchronously after `quranWorker.start()` and update the catalogue through the existing
refresh path. Remote manifest data never controls delivery fields (W10a). A metadata timeout or
failure keeps baked state without delaying Arabic boot.

### Steps

1. Generalise `withTranslationFallback` into `withSourceFallback` with a `hasLocal` probe:
   translations → `hasTranslation(id)`; Arabic → worker readiness. `quranWorker.ready` is a
   synchronous getter, so type the probe `() => boolean | Promise<boolean>`.
   The warm-local side effect must become a per-kind parameter. `withTranslationFallback:139`
   unconditionally calls `quranWorker.ensureTranslation(args.reader)` before the API attempt;
   for an Arabic reader that resolves to `translationRunner(arabicSourceId)` and starts a
   pointless artifact download whose failure is swallowed by a bare `catch`. Pass it as an
   optional `onMiss` supplied only by the translation kind.
2. Preserve tier diagnostics even when fallback succeeds. Each read records
   `{servedBy: "local"|"api", workerFailure?: ReadFailure, apiFailure?: ReadFailure}` through a
   typed status callback. `ReadFailure` distinguishes timeout, transport, HTTP, malformed data,
   and worker failure without exposing URLs or response bodies. Final failure retains both tier
   causes rather than only `lastErr`.
3. **Fix the boot window.** When worker startup has begun, wait up to
   `LOCAL_BOOT_BUDGET_MS = 1500` for readiness before choosing API. Merely checking
   `startPromise !== null || worker !== null` is insufficient because the worker rejects reads
   until initialization completes. Timeout falls through to API; worker readiness within budget
   prevents network use when local DB already exists.
4. **One shared range fetcher for browser and SSR.** `range-fetch.ts` accepts base URL, source,
   bounds, validator, and fetch implementation. Both `quranApi.readRange` and
   `fetchTranslationRange` use it, so no server path can bypass chunking.
5. Chunk reads to ≤300 ayahs (`RESPONSE_CAP = 300`, enforced at
   `controller.rs:1237-1239`).
   **Five juz exceed the cap — 19 (339), 23 (357), 27 (399), 29 (431), 30 (564)** — computed
   from the juz start table in `web/static/quran-meta/quran-data.json`. Global pages max at 42,
   so they are safe. Max chunk count is 2; every chunk is at most 300.
6. **Stitch exact complete results.** Per chunk: apply coordinate validator; require first/last
   indices to equal requested chunk bounds; require exact count. Across chunks: require adjacency;
   concatenate ayahs; merge normalizations by surah; accept byte-equivalent duplicates created
   by a split inside one surah and reject conflicting duplicates. Final result must cover
   `[from,to]` exactly. Any failure rejects the whole read; partial renders are forbidden.
7. `fetchJsonWithTimeout` owns AbortController through headers **and complete body decoding**.
   Range chunks use `RANGE_CHUNK_TIMEOUT_MS = 10_000`; timeout cancels body consumption. The
   30-second worker request timeout remains unrelated.
8. **Thread `validateCoordinate` through `quranApi`.** `api-client.ts` takes no validator, so
   the API path decodes without the check the worker path performs (`worker-client.ts:288`,
   `SurahReader.svelte:417-419`). W5/W6 make the API path routine, silently weakening the
   invariant. This covers `search` as well: `api-client.ts:68` calls `decodeSearchResponse`
   with no validator while the worker path passes one — same invariant, same file, and it is
   the one API path that is already routine today.
9. **Remove both reader gates.** Neither `quran.status === "error"` nor
   `!quranWorker.ready` may prevent a read. Worker status drives UI only; `loadPage` always calls
   the fallback chain. A successful API read clears network degradation but may leave worker
   degradation visible.
10. Resulting order for both source kinds: `local → API → local re-check → typed failure`.

**Invariants:** a ready worker always beats the API; the API is strictly a fallback; no
same-origin `/api/` request is handled by service-worker Cache Storage; live metadata never gates
worker startup.

**Tests:** worker fatal/store status error + API healthy → API result; worker healthy → API never
called; worker becomes ready inside boot budget → no Quran content API request; boot budget
expires → API; all
five oversized juz work through browser and translated server loaders; split-inside-surah merges
normalization; conflicting duplicate/truncated/wrong-boundary chunk rejects; chunk failure → no
partial result; timeout covers stalled body; API path receives validator; `/api/` never enters
any Cache Storage bucket; baked boot starts before delayed `/scripts`/`/sources`; later validated
catalogue refresh succeeds; metadata failure leaves baked state; translation surah behavior
unchanged.

**Done when:** Arabic and translation browser reads share one fallback ladder, browser and SSR
share one exact chunk contract, oversized juz return complete data, tier failures stay
observable, and API responses never enter service-worker caches.

---

## W6 — Client data path for range routes (Web)

**Goal:** upgrade range routes from server first paint to exact local/API data without blanking,
mixing route state, or allowing unbounded SvelteKit data cache growth.
**Files:** `app/_reader/RangeReader.svelte`, `lib/quran/worker-client.ts`,
`src/service-worker.ts`, `app/__tests__/nav-guard.test.ts`, component + service-worker tests.

**Today:** `RangeReader` renders server `data` and nothing else — no worker read, no
`onStatus`.

**Scope:** the component serves **four** routes — `app/juz/[n]`, `app/page/[n]`
(`prerender = true`) and `app/t/[lang]/[translator]/juz/[n]`,
`app/t/[lang]/[translator]/page/[n]` (**`prerender = false`**). Both source kinds must be
designed for, and the prerender flag differs between them.

### Mechanism: component-level post-paint swap

Keeping `+page.server.ts` means SvelteKit resolves `__data.json` before new route props reach the
component. A universal `+page.ts` does not remove that dependency: it receives server data and
runs after the server node resolves. Therefore this workstream is a post-paint data upgrade, not
network-free warm navigation. Removing that dependency requires a separate route architecture;
W9 records the divergence from `my-plan-raw.md:13,22` explicitly.

For Arabic this costs little: the route is prerendered, so `__data.json` is a **static build
artifact** served from the CDN/`eq-data-v1`, not an API call.

Two properties of that path are load-bearing for this workstream and for W7, so state them
rather than assuming them. First, `handleData` is **cache-first** stale-while-revalidate: a
cached entry is returned immediately and revalidated in the background. Repeat visits to a
*translated* range route therefore read `eq-data-v1`, not a fresh SSR render — which is why step
5's translation branch must not freeze on server data, and why W7 step 2 must evict pending
entries rather than wait them out. Second, nothing precaches prerendered `__data.json`: the
precache list is `build + files` plus a few fixed paths, and `build`/`files` exclude prerendered
pages. A cold client pays a network round trip on its first visit to any range route; the
offline pack covers these keys only once a user has opted into it.

### Steps

1. Define route key `{sourceId, kind, index}` and one `$state.raw` display snapshot containing
   `{ayahs, normalizations, surahs}`. These fields always change atomically; never combine client
   ayahs with server normalization or metadata.
2. When props change, immediately install the matching server snapshot, capture its route key,
   then start `quranWorker.readRange` through W5 **without a worker-ready gate**. Install the
   result only if captured key still equals current key. A slow result for an old route is
   discarded.
3. When first paint is complete, skip duplicate client read. When first paint is degraded
   (`ayahs=[]`) or a later navigation occurs, run the chain. Worker readiness events may retry
   only the current degraded key.
4. Derive canonical surah metadata from `loadQuranData()` for every represented surah; never use
   a non-null assertion against possibly empty `data.surahs`.
5. Arabic total failure keeps matching server snapshot. Translation worker failure reaches API;
   total failure keeps matching server snapshot and typed degradation state. No path blanks or
   restores data from a previous route.
6. For the rendered range, take bounds from the server payload — both loaders already return
   `startGlobal`/`endGlobal` on `RangePageData`, so no lookup is needed and no cold-miss network
   fetch is introduced. `loadQuranData()` is required only for the coordinate validator and for
   ranges other than the current one. Preserve translation context on every reader link:
   use `juzPathFor`/`globalPagePathFor` and the `surah*For(ctx, ...)` family with `ctx` derived
   from `surahRouteContext(sourceId)` or `routeContextFromParams(page.params)`. Components never
   call Arabic-only path helpers or hand-build `/app/` navigation strings.
7. Bound `eq-data-v1` with count `DATA_MAX = 400` and
   `DATA_BUDGET_BYTES = 32 * 1024 * 1024`. Store one IDB record per normalized data key:
   `{key, lastUsed, sizeBytes}`. Cache write + metadata update run through one serialized worker
   operation; hits update only that key, avoiding the current whole-object read-modify-write race.
   Evict least-recent keys until both bounds hold. Every deletion path — ordinary eviction, W7
   pending cleanup, W8 auth purge, failed cache write, and cache reset — removes matching metadata.
   Startup reconciliation drops orphan metadata and measures cache entries missing metadata before
   enforcing bounds. Offline-pack entries live in their separate cache and do not count.
8. Extend `nav-guard.test.ts` to reject component calls to Arabic-only reader path helpers and
   navigational `/app/` literals used by links, `goto`, or `resolve`. Add translated range-route
   fixtures that prove the active source context survives page, juz, surah, and ayah navigation.

**Invariants:** display data is one route-keyed atomic snapshot; late results cannot cross route
or source boundaries; reader links preserve translation context; `eq-data-v1` remains bounded;
offline-pack entries are neither counted nor evicted.

**Tests:** complete first paint does not duplicate-read; empty translated SSR recovers complete
ayahs/normalizations/surah metadata; worker unavailable reaches API; Arabic total failure keeps
server data; delayed old result after index/source navigation is discarded; juz 30 works on all
paths; concurrent touches preserve true LRU order; count/byte caps both hold; cache/metadata
orphans reconcile; every deletion path clears metadata; offline pack untouched;
translation-context navigation assertions and `nav-guard.test.ts` pass.

**Done when:** range-route props upgrade to matching client data without mixed metadata, stale
results, blanks, or unbounded `eq-data-v1`; remaining server-load dependency is documented rather
than presented as network-free SPA navigation.

---

## W7 — Pagination-aware degradation and recovery (Web)

**Goal:** implement API-first recovery and Arabic static-data fallback without document reloads;
state the translated-route divergence from `my-plan-raw.md:22` exactly.
**Files:** `src/service-worker.ts`, `hooks.server.ts`,
`app/_reader/SurahReader.svelte`, reader status UI, server-loader and SW tests.

### Delivery contract

- Arabic reader routes are prerendered. Their `__data.json` is a static build artifact, so a
  SvelteKit data load already provides the Arabic SSG-last tier while the static origin is up.
  The offline pack already contains every prerendered `__data.json` — the generator collects all
  of them, giving 1,296 reader entries of its 1,308 — so no second staging mechanism is needed
  and importing `prerendered` from `$service-worker` would only duplicate it. The pack is
  **opt-in** and is read only after a network miss, so it is a user-enabled deepening of this
  tier, not a default one; if it should serve prerendered paths earlier, that is a change to the
  data-cache lookup order, not a new cache. No document reload is added.
- Translated routes are SSR + bounded disk cache and have **no SSG artifact**. Prerendering all
  115 translations would create at least 13,110 surah routes before local-page/range routes.
  Translation recovery is local DB → API → matching server data; it is never called SSG-last.
  W9 records this deliberate divergence.
- Navigation/reload is not a fallback. Reload discards the worker, re-pays the ~2.4 MB Arabic
  cold cost, and still cannot manufacture a translated SSG artifact.

### Steps

1. A translated server load that returns empty pending data sets
   `X-EQ-Translation-Pending: 1` and `Cache-Control: no-store` on both document and
   `__data.json` responses. `hooks.server.ts` preserves `no-store` instead of overwriting it.
2. Service worker treats `no-store` **or** `X-EQ-Translation-Pending` as uncacheable. On lookup,
   a cached response carrying the pending marker is deleted with its W6 metadata and the network
   path continues, so an old empty 200 cannot survive recovery. A pending revalidation never
   overwrites **or deletes** an existing successful response; immutable known-good content remains
   the fallback during a transient upstream outage. With no successful hit, return the pending
   response without caching it.
3. Multi-page surah failure keeps the current page, shows an inline retry, and retries the
   missing page through W5. Single-page empty server data first performs one SvelteKit
   `invalidateAll()` retry, then uses W5 for the same page if server data remains empty. Neither
   branch performs document navigation.
4. Consume W5 typed outcomes. Track worker and API degradation independently:
   - worker failure + API success: content renders; local-offline warning remains;
   - worker success: clear worker warning;
   - API failure + matching server/worker content: content remains; network warning shows;
   - any successful API read clears network warning.
   Never infer API health from `navigator.onLine`.
5. Retry operations are single-flight per route/page. Every degraded state has an explicit
   success-clearing rule; route-key changes discard old state.

**Invariants:** pending responses never enter Cache Storage; pending revalidation never replaces
or deletes a known-good immutable response; recovery never reloads the document; translated
routes are never represented as SSG.

**Tests:** pending document/data response is `no-store`; SW evicts old pending key and its metadata;
API outage → empty pending → API recovery → first retry returns content; successful cached data +
pending revalidation keeps the successful entry; multi-page failure retries in-page;
single-page failure invalidates once then uses W5; no document navigation; worker-down/API-up,
worker-up/API-down, both-down, and recovery produce distinct clearable states.

**Done when:** cached failure cannot survive recovery, worker/API failures remain distinct,
pagination behavior never reloads the document, Arabic static fallback is truthful, and no
translated route is described as SSG.

---

## W8 — Web authentication and cache isolation (Web + Rust)

**Goal:** client-hydrated web authentication for email/password, OAuth, passkeys, and account
management without placing user-specific content in shared server or browser caches.
**Files:** `deploy/.env.example`, `docker-compose.yml`, `src/service-worker.ts`,
`web/src/hooks.server.ts`, new `web/src/lib/auth/`, new auth/account routes,
`migration/src/{lib.rs,m000004_auth_session_binding.rs}`, user-session model/service,
`modules/{auth_v1,email_verification_v1,forgot_password_v1,passkey_v1}`, OAuth provider modules,
`services/{auth.rs,webauthn.rs,oauth/,mail/}`, `middlewares/{security_headers.rs,cors.rs}`,
`utils/cors.rs`, `router.rs`, `main.rs`.

### W8a — Origin and cache prerequisites

1. **Set and validate `ALLOWED_ORIGINS`.** The private router runs `origin_guard`
   (`main.rs:663`), which rejects any request whose `Origin` is not allowlisted
   (`middlewares/cors.rs:20-28`; list at `utils/cors.rs` = localhost + `hmziq.rs` +
   `hzmiqrs.com` + `blog.hmziq.rs`). `easyquran.fyi` is **not** among them and the variable
   appears in neither `deploy/.env.example` nor `docker-compose.yml`. Same-origin POSTs still
   send `Origin`, so **every login and register would 403.** Set
   `ALLOWED_ORIGINS=https://easyquran.fyi,https://hmziq.rs,https://hzmiqrs.com,https://blog.hmziq.rs`
   in `deploy/.env.example`, with every origin spelled literally.
   `docker-compose.yml` already forwards it through `env_file: [.env]`; do not also declare it
   under `environment:`, which only creates a second place to drift.

   **Write the domain out; do not interpolate.** Compose does not expand `${…}` inside values
   loaded via `env_file`, so `ALLOWED_ORIGINS=https://${DOMAIN}` reaches the API as that literal
   string. Every byte in it is printable ASCII, so the parse below **succeeds** — no panic, no
   warning, and `origin_guard` still rejects every login. This is the repo's existing convention
   for exactly this reason: `PUBLIC_API_BASE_URL` (`deploy/.env.example:19`) spells the domain
   out even though `DOMAIN` is set four lines earlier.

   Parse each value as an absolute HTTP(S) origin, not merely a `HeaderValue`. Reject empty
   elements, non-ASCII, placeholders such as `${DOMAIN}`, credentials, path other than `/`, query,
   and fragment. Production accepts HTTPS origins only, requires the EasyQuran origin, and takes
   the complete production consumer list from env; localhost/LAN defaults exist only in explicit
   non-production modes.
   Build one immutable `AllowedOrigins` value during settings load, store it in `AppState`, and
   share it between `CorsLayer` and `origin_guard`; no environment read or parsing occurs per
   request. Keep parsing as a pure function so the serialized environment matrix remains testable.
   Log the resolved production origins once at boot.
2. W5's `/api/` SW bypass is a hard dependency. No authenticated API response may enter Cache
   Storage.
3. **Private API `no-store` without harming public Quran caching.** Every response from the
   private API router receives `Cache-Control: private, no-store`; it need not infer authentication
   from `ruxlog.sid`, because CSRF generation creates that cookie for anonymous sessions too.
   The separate public Quran router never applies this middleware and preserves its existing
   immutable/public cache policy.
4. **Authenticated web document/data isolation.** `translationRouteCacheKey` keys on
   `(sourceId, kind, index)` only and `cacheable` never inspects cookies
   (`hooks.server.ts:11-27,104-105`), while the cached artifact is the **full HTML document
   including the app shell**. Cookie-bearing or session-setting web responses are never read
   from/written to SSR disk cache and receive `Cache-Control: private, no-store`. This includes
   SvelteKit documents and `__data.json`.
5. On successful login, logout, or account switch, send an explicit SW message that deletes
   `eq-pages-v1`, `eq-data-v1`, and W6 metadata. Use `MessageChannel`; the auth transition awaits
   the worker acknowledgement before rendering new user state. OPFS Quran DBs and offline pack
   remain untouched. SW refuses to cache any request/response marked `private` or `no-store`.

**Tests:** production Origin passes; placeholder, empty, Unicode, credentialed, path/query, HTTP,
and localhost production origins fail boot; non-production LAN origins stay gated; private API is
always private no-store; anonymous Quran headers remain public even with an anonymous CSRF cookie;
cookie-bearing web document/data responses are private no-store; SSR disk cache never contains
user data; login/logout await page/data/metadata purge acknowledgement; OPFS/offline pack remain.

### W8b — Session and CSRF foundation

1. Add `auth-client.ts` as the only web API wrapper for auth. Requests use credentials and the
   same-origin `/api` base; errors decode existing API envelopes without treating 401 as a crash.
2. Add request-scoped client state in `auth-state.svelte.ts`: `unknown | anonymous |
   authenticated`, user profile, verification/2FA state, and one in-flight session probe.
   Nothing reads browser storage or session at module scope during SSR.
3. Hydrate after mount with `GET /api/user/v1/get`; 200 authenticates, 401/403 becomes anonymous,
   transport/5xx remains retryable unknown. `/app/**` is prerendered by layout default and never
   embeds user data in build output. Four nodes override `prerender = false` — the translated
   surah, surah-local-page, juz, and global-page routes — and those are precisely the SSR'd
   documents W8a step 4 protects; auth hydration must behave identically on both sets.
4. Fetch CSRF through `POST /api/csrf/v1/generate`. Token stays in memory, never localStorage.
   The same single-flight bootstrap creates the anonymous session needed by CSRF-gated POSTs and
   OAuth state. `auth-client.ts` completes it before **every** non-exempt unsafe auth request
   (login, register, forgot-password, passkey, exchange, account mutations) and before navigating
   to any OAuth `/login` route. Call sites never own this ordering.
5. Add readable `X-EQ-Session-Rotated: 1` to every successful response that calls `cycle_id()` and
   expose it through CORS. The client refreshes CSRF after that header, login, logout,
   OAuth/passkey login, current-session termination, and 2FA verify/disable. Browser code never
   attempts to read `Set-Cookie`. Two-factor setup does not rotate until verification succeeds.
6. Auth transitions invoke W8a cache purge before new user state renders.

### W8c — Email/password, verification, 2FA

1. Build login and registration UI over `/api/auth/v1/log_in` and `/api/auth/v1/register`, with
   field errors, pending state, keyboard/focus behavior, and uniform credential failure copy.
   Registration does not create a session: successful registration proceeds through login, waits
   for rotated-session CSRF refresh, then enters the unverified account flow. Do not call
   verification endpoints before authentication.
2. Support TOTP continuation through `/api/auth/v1/login/totp`; do not mark session authenticated
   before continuation succeeds.
3. Add email-verification request/confirm flow through
   `/api/email_verification/v1/resend` and `/api/email_verification/v1/verify`. Verified-only
   account actions explain the required next step instead of looping on 403.
4. Add password recovery through `/api/forgot_password/v1/request|verify|reset`. Reset tokens and
   codes stay in memory, never URLs or browser storage; responses use uniform account-existence
   copy.
5. Add 2FA setup, confirmation, and disable controls over
   `/api/auth/v1/2fa/setup|verify|disable`. Setup secret/QR data stays in memory; verify/disable
   consume the rotation header and refresh CSRF.
6. Logout posts `/api/auth/v1/log_out`, clears in-memory CSRF/auth data, purges SW caches, then
   probes session once to confirm anonymous state.

### W8d — OAuth and passkeys

1. Web OAuth supports Google, Apple, Facebook, and GitHub through existing
   `/api/auth/{provider}/v1/login|callback|exchange` routes. Native clients use provider-local
   `/api/auth/{provider}/v1/token` endpoints, which verify provider credentials and establish the
   same cookie session without entering the browser OAuth flow.
2. Implement `/auth/{provider}/success` and `/auth/{provider}/failure`. Extend backend callbacks
   to accept provider cancellation/error query shapes and redirect failures to the allowlisted
   frontend failure path with an opaque error code only; API messages, provider payloads, code,
   and state never enter the URL. Return targets are same-origin paths stored in sessionStorage,
   consumed once, and cleared after success/failure. Success refreshes CSRF, purges caches, probes
   profile, then consumes the return target.
3. Passkey login uses `/api/passkey/v1/login/begin|finish`; verified account registration uses
   `/api/passkey/v1/register/begin|finish`, and management uses
   `/api/passkey/v1/list|remove`. Challenges stay in memory; cancellation is not reported as
   server failure.

### W8e — Account UI and session lifecycle

1. Add prerendered account shell hydrated from W8b. Anonymous users see login CTA; unknown state
   shows neutral loading/retry; authenticated users see profile and security controls.
2. Profile update uses `/api/user/v1/update`; session list/termination uses
   `/api/auth/v1/sessions/list|terminate/{id}`; 2FA/passkey controls use existing verified routes.
3. Add migration-owned durable binding from each `user_session` audit row to its opaque tower
   session id. Login/passkey/OAuth creation writes the binding before returning success; every
   `cycle_id()` replaces it before returning; binding failure destroys the rotated session and
   fails closed. Logout revokes the audit row, deletes the tower session, and clears the binding.
   Session list returns `isCurrent` computed server-side without exposing the tower id. Existing
   unbound sessions are revoked and removed during startup reconciliation, producing one clean
   re-authentication boundary.
4. Terminating current session follows the acknowledged logout/cache-purge transition.
   Terminating another session deletes it through the durable binding and refreshes the list.
   Both work after process restart and after 2FA/passkey session rotation.
5. No server-rendered user name, email, token, CSRF value, OAuth payload, or passkey challenge may
   enter HTML disk cache, Cache Storage, localStorage, logs, analytics, or error URLs.

### W8f — Production auth configuration and privacy

1. Add `WEB_AUTH_ENABLED=true` and `WEB_OAUTH_PROVIDERS=google,apple,facebook,github` to production
   config. Every listed provider must have its exact credentials and HTTPS callback URI:
   `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GOOGLE_REDIRECT_URI`;
   `APPLE_CLIENT_ID`, `APPLE_TEAM_ID`, `APPLE_KEY_ID`, one of `APPLE_PRIVATE_KEY` or
   `APPLE_PRIVATE_KEY_PATH`, and `APPLE_REDIRECT_URI`;
   `FACEBOOK_CLIENT_ID`, `FACEBOOK_CLIENT_SECRET`, `FACEBOOK_REDIRECT_URI`; and
   `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `GITHUB_REDIRECT_URI`.
2. Require `FRONTEND_URL=https://easyquran.fyi` and
   `OAUTH_ALLOWED_REDIRECT_ORIGINS=https://easyquran.fyi`. Provider callbacks and frontend
   success/failure targets must match this origin exactly.
3. Require `WEBAUTHN_RP_ID=easyquran.fyi`, `WEBAUTHN_RP_ORIGIN=https://easyquran.fyi`, and a
   non-empty `WEBAUTHN_RP_NAME`. Production rejects localhost/default WebAuthn configuration.
4. Production auth rejects `MAIL_PROVIDER=none`. Require a configured SMTP or Cloudflare mail
   provider, non-empty `MAIL_FROM_ADDRESS`/`MAIL_FROM_NAME`, provider credentials, and a deployment
   delivery smoke test to a controlled inbox before verification/recovery UI is enabled.
5. Boot fails when enabled auth/provider/mail/WebAuthn configuration is missing or inconsistent.
   Readiness reports provider names and ready/not-ready state without secrets.
6. Remove email, provider subject, OAuth payload, session id, verification code, and recipient
   address from logs. Emit opaque user id, provider name, result code, and trace id only. Add log
   capture tests for login, registration, OAuth, verification, recovery, and mail failure paths.

**Invariants:** user state is client-hydrated; auth secrets stay in memory or server-side stores;
private responses never enter shared caches; every session rotation refreshes CSRF; remote session
termination uses durable binding; logs and error URLs contain no sensitive auth values.

**Tests:** single-flight bootstrap precedes every unsafe/OAuth-start flow; registration → login →
verification; password recovery; CSRF refresh after every rotation header; email/password + TOTP;
all four OAuth success/cancel/error redirects; passkey login/register/remove; durable current vs
other session termination before/after restart and session rotation; legacy sessions invalidate;
production config rejects missing provider, localhost WebAuthn, redirect mismatch, and no-op mail;
delivery smoke procedure passes; log capture contains no sensitive values; anonymous prerender
contains no user data; account-switch cache isolation; accessibility and `pnpm check` pass.

**Done when:** login, registration, verification, recovery, 2FA, four OAuth providers, passkeys,
profile, and durable session management have complete web flows; production dependencies are
ready; CSRF tracks every session rotation; logs are scrubbed; no shared cache can replay one
user's content to another.

---

## W9 — Record the divergences (Docs)

**Goal:** keep `quran-system.md` and repository guidance aligned with the delivery architecture
this plan actually implements.
**Files:** `docs/quran-system.md`, `AGENTS.MD`.

`quran-system.md:112` already records "translated pages = SSR on Bun". Record the exact
boundaries that implementations must not blur:

1. **Translated pages are SSR + 7-day disk cache, not SSG.** Prerendering would be
   114 × 115 ≈ 13,110 pages for the top-level surah route, and the surah tree does not stop
   there: surah-local pages add 548 × 115 ≈ 63,020, so the surah shape alone is ~76,000 routes
   before juz and global pages. Quote the full figure — the 13,110 number alone understates the
   cost by roughly six times. Prerendering also requires a live API at build time
   (`quran-translation-page.ts` fetches ranges over HTTP), which Arabic prerender does not.
   SEO is served by hreflang alternates
   (`routes/sitemap.xml/+server.ts:31-42`). This deliberately diverges from
   `my-plan-raw.md:19,22`; no translated path may be described as SSG.
2. **Translated delivery is not surah-only.** The same source context spans surah,
   surah-local-page, global-page, and juz routes so reader navigation cannot fall back to
   Arabic. The sitemap groups those route shapes with language alternates. This is the exact
   runtime boundary behind the shorter intent in `my-plan-raw.md:19`.
3. **"SSG last" holds for Arabic only,** because those routes are prerendered and their
   `__data.json` is a static artifact. Translated routes recover through local DB, API, or
   matching server data only.
4. **The durable pool metric is API demand across process restarts, not reader popularity.**
   Moka TinyLFU already owns in-process popularity admission (W1).
5. **Surah and range routes keep `+page.server.ts`,** so `__data.json` resolves before client
   props on navigation. Surah adjacent-page reads and W6 range reads upgrade content after paint;
   neither removes the server-load dependency. Removing it requires a route-architecture
   workstream, not a claim that hydration already replaced SSG/SSR.
6. **Authentication is client-hydrated everywhere.** Arabic reader and account shells are
   prerendered; four translated reader route families remain auth-neutral SSR. User state enters
   neither build output, translated HTML disk cache, nor Cache Storage (W8).
7. **Artifact safety is the W10 contract.** Document exact baked id/size/delivery mapping, staged
   validation, active-pointer/legacy-file behavior, and bounded `eq-data-v1` metadata. Do not
   imply that remote metadata selects Quran bytes or that a partially written DB can become active.

**Invariants:** do not edit `my-plan-raw.md`; do not describe translated routes as SSG; every
artifact-safety claim must match W10's executable contract.
**Tests:** documentation search finds no translated-SSG claim, no `/app/** is prerendered` claim,
and no unsupported artifact-safety claim; AGENT links resolve.
**Done when:** every divergence above is recorded.

---

## W10 — Cross-cutting delivery and cache safety requirements

### W10a — Baked artifact delivery contract

**Goal:** remote metadata may describe availability but can never select Quran bytes, size, or a
delivery origin.
**Files:** `lib/quran/{catalogue.ts,manifest.ts,wire.ts,environment.ts}`, catalogue/manifest tests.

1. Build authoritative maps from baked data: `{id, sizeBytes, r2Path, sameOriginDeliveryPath}`.
   Translation catalogue ids absent from the baked map reject the whole API payload.
2. Parse API URLs as HTTPS with no credentials, query, or fragment. Compare their canonical R2
   pathname against the baked `r2Path`; production API URLs use `r2.easyquran.fyi` while browser
   delivery uses `/_quran`, so raw URL equality is not the contract. Require API `sizeBytes` to
   equal baked size. After validation construct download URL and size **only** from baked fields.
3. Arabic script decoding already accepts only registered Arabic ids with positive size. Apply
   the same exact baked-size and canonical-R2-path comparison before localization; do not claim it
   has the translation catalogue's unknown-id hole.
4. Unknown id, size/path mismatch, invalid URL, or malformed payload falls back to baked data and
   emits a structured, consent-gated browser telemetry event. Web has no OTLP exporter; Rust OTLP
   remains separate.

**Invariants:** source identity is id; delivery never trusts remote size/path; no Quran hash;
fallback is visible but never blocks boot.
**Tests:** valid production R2 URL localizes to baked `/_quran` path; malicious origin,
credentials/query/fragment, canonical-path mismatch, size mismatch, and unknown translation id
reject; registered Arabic script payload succeeds; fallback telemetry respects consent.
**Done when:** every production download spec is reconstructed from baked id/size/path fields.

### W10b — Bounded dedup claims

**Goal:** expired dedup claims cannot accumulate without bound.
**Files:** `crates/rux-request-gate/src/store.rs` and store tests.

`InMemoryStore::prune()` removes expired entries from both `limits` and `claims`. Add a
high-cardinality expiry regression test and a claims-count diagnostic.

**Invariants:** pruning claims never releases an unexpired claim.
**Tests:** mixed expired/live high-cardinality claims prune only expired entries and diagnostic
count returns to the live set size.
**Done when:** expired high-cardinality claims return to zero after prune.

### W10c — Validated crash-safe OPFS replacement

**Goal:** failed or interrupted downloads never replace a readable corpus.
**Files:** `lib/workers/{download.ts,opfs-cache.ts,quran.worker.ts}`, OPFS metadata/listing code,
worker/OPFS tests.

1. Make `sizeBytes` required at every production `DownloadSpec` boundary. Generic tests may build
   invalid specs only through an explicit unsafe fixture helper.
2. Download into an id-scoped temporary file, close it, and verify exact baked size. Open the
   staged SQLite through the worker before commit and assert expected schema, exactly 6,236
   contiguous coordinates/rows, source id/profile, and existing Quran content invariants. These are
   content assertions, never hashes.
3. Store `{sourceId, activeFile}` in IDB. After validation, switch the pointer in one IDB
   transaction, reopen through the active pointer, then remove the old file. Failure before or
   after pointer switch keeps/reverts to the last validated active file and cleans temp files.
4. On first pointer-aware boot, adopt a valid legacy `<id>.sqlite` as active. Listing, retention,
   and deletion ignore temp filenames as sources, remove abandoned temps, and operate through
   pointers. Invalid legacy files stay inactive and trigger normal redownload.

**Invariants:** Quran DBs remain immutable; id remains identity; temp/active filenames are neither
versions nor cache identities; W4 timeout/byte ceiling applies to staging.
**Tests:** interrupt download, close, validation, pointer transaction, reopen, and old-file cleanup;
old corpus remains readable at every failure point; valid legacy file adopts once; invalid legacy
and abandoned temp never appear as sources.
**Done when:** only a fully validated staged DB can become active and legacy installs migrate
without data loss.

W4 owns the transfer timeout/ceiling, W2 owns health isolation, and W6 owns bounded
`eq-data-v1`; those requirements are not duplicated here.

---

## 11. Dependencies and verification

```
W10a + W10c ─► W4
W3b ─► W3c
W2 + W3c + W10b ─► W3a
W5 ─► W6 ─► W7
W5 + W7 ─► W8a ─► W8b ─► W8c/W8d ─► W8e ─► W8f
W1 (after m000002; owns m000003)
W8e (after m000003; owns m000004)
W9
```

**Hard constraints:** W2 ingress proof + W3b + W3c + valid allowlist before W3a. W5 before W6
before W7. W10a/W10c before W4 can trigger more translation downloads. W5 API bypass and W7
pending-response rules before auth. `m000002_rate_limit_state` precedes
`m000003_translation_popularity`, which precedes `m000004_auth_session_binding`.

### Operational requirements

- **Feature flags:** W3a uses `QURAN_BAN_ESCALATION_ENABLED` because it bans clients. W1 uses
  `QURAN_DEMAND_COLLECT` and `QURAN_PREWARM_TRANSLATIONS`; collection off forces prewarm off, while
  prewarm `0` preserves collection. W8 uses `WEB_AUTH_ENABLED`; production sets it true only with
  W8f ready. Recovery/cache correctness does not get a second behavior branch.
- **Rollback:** W4 deletes its per-tab legacy key only after durable read-back (step 9); W3b's
  column and W8's session binding are additive and readable by old binaries. Pre-binding sessions
  intentionally re-authenticate once and are not restored by rollback.
- **Migration ordering:** boot runs migrations before loading persisted state. `m000002` upgrades
  legacy rate rows; `m000003` creates popularity state; `m000004` adds durable auth-session
  binding and startup reconciliation. Test old binary/new schema and new binary/legacy fixtures.
  Read `rust/backend/api/migration/README.md` for every schema change.
- **Partial deploy:** web and API are separate images. W2 deploy order is fixed: provision the
  shared internal token; deploy web that sends the header while old API ignores it; configure
  CF-only origin ingress and `IP_SOURCE=CfConnectingIp`; pass ingress/header assertions; deploy
  API recognition and hard failures. W8f secrets/readiness land before auth UI is enabled. W5
  remains compatible with an older API envelope.
- **Observability:** Rust counters for prewarm, escalation/bans, rate-store saturation, and auth
  provider readiness leave through OTLP; provision their dashboard queries with the counters.
  Browser fallback/degradation/artifact-rejection/auth events use existing consent-gated Firebase
  telemetry. Do not add browser OTLP without an explicit exporter, collector, privacy, and test
  workstream.
- **Verification per workstream:** from `rust/` — `cargo fmt --all -- --check`,
  `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`; from `web/` —
  `pnpm check`, `pnpm test`, `pnpm lint`, `pnpm format:check`. Always include
  `tests/quran_v1.rs`, `catalogue-sha-guard.test.ts`, and
  `nav-guard.test.ts`. Schema work runs fresh + legacy migration fixtures; cache work runs
  service-worker/browser tests. Nothing is done while any required check fails.

---
