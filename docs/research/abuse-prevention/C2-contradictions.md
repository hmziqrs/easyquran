# C2 — Contradiction referee

Referee: adversarial cross-check of `B10-falsification.md` §1 against the working tree, the
dependency source, and the other reports. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`.

Method: every claim below was re-read at source. Dependency sources read in
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` and the bun store
`node_modules/.bun/`. No claim is accepted from a report alone.

**Input-status correction.** B10 §"Scope note" says B1..B* do not exist. They do now — all of
`B1-token-storage.md` … `B10-falsification.md` are present in this directory (timestamps
02:20–02:27, after B10 was written at 02:22). The `B*` claims can therefore be audited.

---

## 1. Migration atomicity — A3 §8 vs A10 F8 / B6 F1

**Verdict: A10 and B6 are right; A3 §8/F13 is wrong.**

Claims:

- A3 `A3-repo-integration.md:309,321,490-497` — "sea-orm wraps each migration in a transaction
  (`migrator.rs:263-266`)", and "the migration runs inside a transaction, where `PRAGMA
foreign_keys` is a no-op".
- A10 `A10-data-model.md:19,310` / B6 `B6-rollout.md:90-98,379-390,664-671` — SQLite migrations
  are not transactional; the version row is inserted after `up()` returns.

Evidence, read from the registry source:

- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sea-orm-migration-1.1.20/src/migrator.rs:261-272`:

  ```rust
  match db.get_database_backend() {
      DbBackend::Postgres => {
          let transaction = db.begin().await?;
          let manager = SchemaManager::new(&transaction);
          f(&manager).await?;
          transaction.commit().await
      }
      DbBackend::MySql | DbBackend::Sqlite => {
          let manager = SchemaManager::new(db);
          f(&manager).await
      }
  }
  ```

  Only Postgres gets `begin()/commit()`. For SQLite every statement in `up()` runs in autocommit
  mode. A3 cites lines 263-266, which are inside the Postgres arm.

- `migrator.rs:378-398`: `migration.up(manager).await?` (:386), and only **after it returns** is the
  `seaql_migrations` row inserted (:391-397). A crash between the two leaves DDL applied and
  unrecorded; `M::get_pending_migrations` then lists it again on the next boot.

- Boot path: `rust/backend/api/src/db/sea_connect.rs:87-93` calls `Migrator::up(&conn, None)`
  inside `try_connect`; `src/main.rs:168` calls `get_sea_connection()` and `src/main.rs:939` binds
  the listener last. So migrations complete before the port opens (B6 §0.2 item 7 is accurate).
  A migration error panics (`sea_connect.rs:65-68`) → `restart: unless-stopped`
  (`docker-compose.yml:193`) → `up()` re-runs.

**Is SQLite DDL transactional at all? Yes — but only if the caller opens a transaction.** Verified
empirically:

```
$ sqlite3 /tmp/ddl.db "BEGIN; CREATE TABLE t(x); ROLLBACK; SELECT name FROM sqlite_master WHERE name='t';"
(empty output)
```

and the repo already relies on it: `src/services/rate_limit_store.rs:190`
(`db.transaction::<_, (), DbErr>(|txn| …)`) wraps a DELETE + upserts. B6:389's "SQLite DDL is
transactional; wrap the batch in `conn.transaction`" is correct and is the hardening the plan
should adopt. A3's derived FK-pragma claim is false for the same reason: `PRAGMA foreign_keys=ON`
is set at connect (`sea_connect.rs:46-56`) and is live during the DDL, so FK actions fire; only
inside an explicit transaction would the pragma be a no-op.

**Final plan must say:** m000012 is a bare multi-statement `up()` on SQLite; it must be
all-`IF NOT EXISTS` + `PRAGMA table_info`-guarded, with an up-twice test, and the DDL batch
wrapped in `conn.transaction::<_, (), DbErr>(…)` so a crash cannot leave a half-migrated schema.
Never rebuild a table; `down()` index-only. A3's transaction sentence must be struck.

---

## 2. Token storage — A4 F1 vs A1 F4 / B1

**Verdict: B1 and A1 are right; A4's "Break condition A" is overbroad.**

Evidence:

- Cache decision is a bare cookie-presence test on the request:
  `web/src/hooks.server.ts:346-351` (`!requestHasCookie`), `:427`
  (`const requestHasCookie = !!event.request.headers.get("cookie")`). It never reads cookie
  names or paths.
- `web/src/lib/server/quran-disk-cache.ts:12` (7-day TTL), `:51-57` (key = build id + source id +
  kind + index; no header component, no `Vary`).
- `web/src/hooks.server.ts:130-137`: any request that carries a cookie gets
  `Cache-Control: private, no-store`.
- Traefik routes `Host(DOMAIN) && PathPrefix(/api)` to `api:8888` and strips `/api`
  (`docker-compose.yml:181-192`); there is no `/api` route under `web/src/routes/`. So `/api/**`
  never reaches `hooks.server.ts` at all.
- RFC 6265 path-match: a `Path=/api` cookie is attached only to `/api…` requests. A `/t/en/…`
  navigation does not carry it → `requestHasCookie === false` → `cacheable === true`. A4's
  "token ever becomes a cookie" omits the Path attribute, which is the only variable that decides.

**B1's second claim — is `ruxlog.sid` `Path=/`? Yes.** `src/main.rs:781-788` builds
`SessionManagerLayer::new(...).with_name("ruxlog.sid")` with **no** `.with_path(...)` (grep:
no `with_path` in `rust/backend/api/src`). tower-sessions 0.14.0 defaults to `path: "/"` and always
sets it: `~/.cargo/registry/.../tower-sessions-0.14.0/src/service.rs:137` (`path: "/".into()`),
`:243` (`cookie.set_path(session_config.path)`). Consequence: after any visitor hits
`POST /csrf/v1/generate` — which mints an anonymous session
(`src/modules/csrf_v1/controller.rs:10-19`) — the browser sends `ruxlog.sid` on every reader
request, so the 7-day disk cache already serves only cookie-less visitors. The marginal cache cost
of a `Path=/api` device cookie is zero.

**Final plan must say:** adopt the device token as an HttpOnly cookie
`ruxlog.device=…; Max-Age=2592000; HttpOnly; SameSite=Lax; Secure; Path=/api` (explicit Path,
never defaulted). Keep A4's guards, but rewrite the premise: the rule is "no cookie on reader
paths", not "no cookie". The disk cache's real fix (separate decision) is `.with_path("/api")` on
the session layer.

---

## 3. Edge verdict — A1 (build) vs A7 (unjustified) vs A9 (defer) vs B9 (cut)

**Verdict: CUT from v1 scope (build nothing now); keep the design text as defer-until-consumer.**

Evidence:

- No edge code exists. `rg -n "x-eq-edge" -i .` → **0 hits** (repo-wide, excluding
  `node_modules`/lockfiles). `rg -n "wrangler" -i .` → **0 hits**. No `wrangler.toml`, no Worker
  source, no `X-Eq-Edge` header anywhere (`A9-crypto-keys.md:30,247`, `B9-second-order.md:390-402`
  both re-verified).
- The routing is Cloudflare → Traefik → containers (`docker-compose.yml:181-192`); Traefik's only
  api middleware is `stripprefix /api` — no header middleware.
- What the origin can verify today: only that the request arrived on the CF→Traefik path — via
  `CF-Connecting-IP` believed only when the TCP peer is inside `TRUSTED_PROXY_CIDRS`
  (`src/middlewares/client_ip.rs:64-74,113-121`) with ingress restricted to CF ranges out-of-band
  (`deploy/README.md:377-400`). `TRUSTED_PROXY_CIDRS` defaults to all private ranges
  (`docker-compose.yml:143`), so any container on the Docker network can forge the header.
- What the origin cannot verify: JA4/TLS fingerprint (Enterprise + Bot Management only), ASN,
  bot score, country — none is present as a header, and `rg cf-ray` in `rust/backend/api/src` → 0
  hits, so the origin does not even read the one header CF always adds. A Worker on the available
  plan can only forward fields derivable from `CF-Connecting-IP` (A7 F7,
  `A7-performance-cost.md:373`).
- Cost: +1–5 ms p50 TTFB, per-request billing, a second secret store, and a new deploy path
  (A7 F7); the origin already trusts the CF ingress path; the design's ladder consumes none of the
  edge fields. A1's "worth building" is unsupported by the cost analysis it did not do, and its
  own HMAC construction fails A1's stated property because the origin holds the same key
  (A9 §4.1(7)).

**Final plan must say:** v1 ships origin-only. Edge attestation is removed from scope, retained as
a documented future layer with the Ed25519 construction (`verify_ed25519`,
`crates/rux-webhook-crypto/src/lib.rs:101`) and a named consumer required before implementation.
Any implementation must first add header stripping (item 9) and a Worker deploy path that does not
exist.

---

## 4. Edge fail-open vs fail-closed — A3 vs A1 vs A9

**Verdict: A9's policy (drop the attestation, keep the request, warn/alert) matches the house
style. A3's fail-closed contradicts the existing precedent; A1's read/write split has no
precedent and no spec.**

Evidence — the repo's own optional-but-security-relevant header, the internal SSR token:

- `src/middlewares/client_ip.rs:92-106`: `x-easyquran-internal-token` present-but-invalid →
  `tracing::warn!(…)` and **falls through** to external resolution; the header is not removed and
  the request is not rejected because of it. The comment states the rule: "Present-but-invalid
  NEVER grants internal treatment — it falls through to external resolution".
- The only fail-closed gate in that middleware is the _identity_ requirement: production with no
  `CF-Connecting-IP` → 400 (`client_ip.rs:133-140`). That is a different control (identity, not
  attestation) and is mandatory, not optional.
- Mandatory controls do reject: billing webhooks reject a bad signature
  (`src/services/billing/*.rs`, `verify_hmac_sha256_hex`), because the webhook is not optional.
  The distinction is consistent: optional attestation degrades, mandatory auth rejects.

**Final plan must say:** invalid/unknown-kid/skewed attestation → log + metric, keep the request
un-attested, never 401/403; enforcement (if ever) is a per-endpoint opt-in flag that fails closed
only for the endpoints that declare it. A3's blanket fail-closed is rejected; A1's GET/POST split
is not adopted (no spec, no precedent, and it would 4xx writes on a clock-skew event).

---

## 5. `UNIQUE(device_key)` — A10 §1.2 keeps it; A2 F1 / A9 / DESIGN-v1 drop it

**Verdict: drop it. A10's normative DDL is stale.**

Evidence:

- A10 keeps it and calls it "the upsert conflict target" (`A10-data-model.md:134,725,767`).
- A2 F1 (`A2-false-positives.md:310-321`): `device_key` is a coarse cohort key, not a device key;
  `UNIQUE(device_key)` asserts 1:1 "false in both directions" (many devices → one key; one device
  with changed signals → many keys). Fix: "Drop `UNIQUE(device_key)`; key the table on a token
  hash and store the component key as a non-unique attribute."
- DESIGN-v1:10: "dropped; `device_key` non-unique, indexed for cohort lookups only."
- A10's own DDL already has `token_hash TEXT` + `CREATE UNIQUE INDEX … (token_hash)`
  (`A10-data-model.md:108,136`), so the conflict-target justification is circular: once the token
  hash is the lookup key, `UNIQUE(device_key)` has no job and forces record merges on cohort
  collisions (and makes an attacker-supplied coarse tuple an identity claim).

**Index that replaces it:**

```sql
-- identity + upsert conflict target (replaces UNIQUE(device_key))
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_token_hash ON device_records (token_hash);
-- cohort lookups only, non-unique (DESIGN-v1:10)
CREATE INDEX IF NOT EXISTS idx_device_records_key ON device_records (device_key);
-- cohort newest-first
CREATE INDEX IF NOT EXISTS idx_device_records_key_last_seen ON device_records (device_key, last_seen_at);
```

**Final plan must say:** m000012 ships `token_hash` UNIQUE as the only identity/conflict target;
`device_key` is a non-unique attribute with an ordinary index. A10 §1.2/§1.3, its migration-contract
test list (`:260`), its upsert examples (`:444,483,502,725,767`) and any test asserting "a second
`device_key` insert must fail" (`:266-267`) must be rewritten.

---

## 6. Two competing m000012 schemas — one corrected schema

**Verdict: neither A9 §2.4 nor A10 §1.2 can be written as-is; neither has `expires_at`. Corrected
schema below.** B10 §1.8 identified the conflict but produced no merged DDL.

Every disagreement:

| #   | A9 §2.4 (`A9-crypto-keys.md:137-168`)                                             | A10 §1.2 (`A10-data-model.md:100-219`)                                                           | Corrected                                                                                                                                     |
| --- | --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `id TEXT PRIMARY KEY` (ULID)                                                      | `id INTEGER PRIMARY KEY AUTOINCREMENT`                                                           | **TEXT PK** (DESIGN-v1:38; repo has TEXT-PK precedent: `m000007:14,27`, `m000004:12`; no `ulid` crate — `uuid` v4 exists at `Cargo.toml:116`) |
| 2   | `token_hash BLOB NOT NULL`                                                        | `token_hash TEXT` (nullable)                                                                     | **TEXT NOT NULL** (hex HMAC; repo style TEXT; the conflict target must be non-null)                                                           |
| 3   | identity = `token_hash` UNIQUE (`:155`), `fp_digest` non-unique (`:157`)          | identity = `UNIQUE(device_key)` (`:134`), `token_hash` UNIQUE (`:136`)                           | **token_hash UNIQUE = identity**; device_key non-unique (item 5)                                                                              |
| 4   | `trust_state TEXT DEFAULT 'new'` (`:146`)                                         | `trust_state TEXT DEFAULT 'observe'` (`:113`)                                                    | **'observe'** (fail-closed; DESIGN-v1 B2 "never new device")                                                                                  |
| 5   | no `expires_at`                                                                   | no `expires_at` on `device_records`                                                              | **`expires_at TEXT NOT NULL`** (A5 F2 `:275-277`; DESIGN-v1:22,38)                                                                            |
| 6   | no `device_bans` table (defers device bans)                                       | `device_bans` table (`:173-199`)                                                                 | **include `device_bans`** (DESIGN-v1:41)                                                                                                      |
| 7   | `device_sessions.user_session_id` FK → `user_sessions` ON DELETE CASCADE (`:163`) | no FK to `user_sessions`; denormalized `user_id` (`:145-159`)                                    | **A10's shape**: FK only to `device_records`; no FK to users/user_sessions (DESIGN-v1:39; A3 F10; A10 F1)                                     |
| 8   | `device_sessions` = 3 columns                                                     | + `user_id`, `tower_session_id`, `ip_unit`, `linked_at`, `last_seen_at`, UNIQUE(record, session) | **A10's shape**                                                                                                                               |
| 9   | `fp_digest BLOB` + `fp_key_id` + `fp_version`                                     | `device_key TEXT` + `key_kid` + `cohort_key`                                                     | **`device_key` + `key_kid`** (DESIGN-v1:38); A9's `fp_digest` is the same value under another name                                            |
| 10  | `coherence TEXT` JSON (`:148`)                                                    | tri-state `INTEGER` columns (`:116-119`)                                                         | **tri-state INTEGER** (DESIGN-v1:38)                                                                                                          |
| 11  | `revoked_at TEXT` on records (`:152`)                                             | absent                                                                                           | **keep `revoked_at`** (DESIGN-v1 B4)                                                                                                          |
| 12  | `device_epoch_keys`/`device_correlators` alternative                              | absent                                                                                           | **dropped** (A9 itself recommends schema 1, `:188`)                                                                                           |

Corrected m000012 (SQLite, repo style — `IF NOT EXISTS`, TEXT timestamps, no table rebuild):

```sql
CREATE TABLE IF NOT EXISTS device_records (
    id TEXT PRIMARY KEY,                         -- ULID or UUIDv4, server-minted, never exposed
    token_hash TEXT NOT NULL,                    -- HMAC(token_subkey, "tok:v1:" || token), lowercase hex
    token_key_id INTEGER NOT NULL DEFAULT 1,
    device_key TEXT NOT NULL,                    -- HMAC(fp_subkey, canonical components), lowercase hex; NON-unique
    key_kid TEXT NOT NULL DEFAULT 'v1',
    fp_version INTEGER NOT NULL DEFAULT 0,
    user_id INTEGER REFERENCES users (id) ON UPDATE CASCADE ON DELETE SET NULL,
    first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    seen_count INTEGER NOT NULL DEFAULT 1,
    trust_state TEXT NOT NULL DEFAULT 'observe',
    rule_version INTEGER NOT NULL DEFAULT 0,
    risk_score INTEGER NOT NULL DEFAULT 0,
    coherence_ua_platform INTEGER,
    coherence_gpu_platform INTEGER,
    coherence_hardware_triple INTEGER,
    coherence_screen_viewport INTEGER,
    spoof_webdriver INTEGER,
    spoof_native_tamper INTEGER,
    spoof_cdp INTEGER,
    hardware_class TEXT,
    platform_class TEXT,
    locale_class TEXT,
    screen_class TEXT,
    cohort_key TEXT,
    evidence TEXT,
    revoked_at TEXT,
    expires_at TEXT NOT NULL CHECK (expires_at <> ''),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_token_hash ON device_records (token_hash);
CREATE INDEX IF NOT EXISTS idx_device_records_key ON device_records (device_key);
CREATE INDEX IF NOT EXISTS idx_device_records_key_last_seen ON device_records (device_key, last_seen_at);
CREATE INDEX IF NOT EXISTS idx_device_records_user_last_seen ON device_records (user_id, last_seen_at);
CREATE INDEX IF NOT EXISTS idx_device_records_last_seen ON device_records (last_seen_at);
CREATE INDEX IF NOT EXISTS idx_device_records_cohort ON device_records (cohort_key);

CREATE TABLE IF NOT EXISTS device_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_record_id TEXT NOT NULL REFERENCES device_records (id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    user_id INTEGER NOT NULL,                    -- denormalized; NO FK (evidence must outlive users)
    user_session_id INTEGER NOT NULL,            -- NO FK (user_sessions cascades on user delete, m000001:55-56)
    tower_session_id TEXT,
    ip_unit TEXT,
    linked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_sessions_record_session
    ON device_sessions (device_record_id, user_session_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_user_linked ON device_sessions (user_id, linked_at);
CREATE INDEX IF NOT EXISTS idx_device_sessions_record_user ON device_sessions (device_record_id, user_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_session ON device_sessions (user_session_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_linked ON device_sessions (linked_at);

CREATE TABLE IF NOT EXISTS device_bans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_key TEXT NOT NULL,
    key_kid TEXT NOT NULL DEFAULT 'v1',
    device_record_id TEXT REFERENCES device_records (id) ON UPDATE CASCADE ON DELETE SET NULL,
    scope TEXT NOT NULL DEFAULT 'Long',
    reason TEXT,
    banned_by INTEGER REFERENCES users (id) ON UPDATE CASCADE ON DELETE SET NULL,
    expires_at TEXT,                             -- NULL = indefinite, admin-set only (mirrors user_bans)
    rule_version INTEGER NOT NULL DEFAULT 0,
    risk_score INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TEXT,
    revoked_by INTEGER REFERENCES users (id) ON UPDATE CASCADE ON DELETE SET NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_bans_active_key
    ON device_bans (device_key) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_device_bans_record ON device_bans (device_record_id);
CREATE INDEX IF NOT EXISTS idx_device_bans_key_created ON device_bans (device_key, created_at);
CREATE INDEX IF NOT EXISTS idx_device_bans_expires ON device_bans (expires_at);

CREATE TABLE IF NOT EXISTS device_evidence (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_key TEXT NOT NULL,
    key_kid TEXT NOT NULL DEFAULT 'v1',
    user_id INTEGER,
    user_session_id INTEGER,
    subject TEXT NOT NULL,
    detail TEXT,
    ip_unit TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_device_evidence_key_created ON device_evidence (device_key, created_at);
CREATE INDEX IF NOT EXISTS idx_device_evidence_user_created ON device_evidence (user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_device_evidence_subject_created ON device_evidence (subject, created_at);
```

**Final plan must say:** this is _the_ m000012. `device_records.expires_at` is NOT NULL and the
batched sweeper (`scheduler.rs`) enforces it; `device_bans` retention (12 mo) is enforced by the
sweeper deleting revoked/expired rows, not by NOT NULL. A10's migration-contract test must be
updated to assert `token_hash` NOT NULL + UNIQUE and to drop the "second `device_key` insert must
fail" pin.

---

## 7. Anonymous visitors mint device records — DESIGN-v1 vs its own attachment point

**Verdict: the hazard is real; the adopted attach point violates the design's own policy.**

Evidence:

- `web/src/lib/auth/auth-client.ts:257-260`: `unsafeRequest()` calls
  `await this.ensureAnonymousSession()` unless `init.exempt`.
- `:240-251`: `ensureAnonymousSession()` → `fetchCsrf()`.
- `:231-238`: `fetchCsrf()` is `#request("POST", "/csrf/v1/generate")`.
- `:269-283`: `getUser()` is `#request("GET", "/user/v1/get")` — called on page-load hydrate
  (A4 §7(b) cites `auth-state.svelte.ts:106-114`).
- `:211-227`: any response with `x-eq-session-rotated` re-enters `#request` via `fetchCsrf()`
  (guarded only by `#refreshing`).
- Server side: `src/modules/csrf_v1/controller.rs:10-25` mints an anonymous `ruxlog.sid` for any
  visitor who has none.
- DESIGN-v1:64 "no fingerprint for logged-out readers"; B2:33-35 attaches the payload inside
  `AuthClient.#request` (A4 §7(a) states the same).

So with an unconditional attach inside `#request`, **every anonymous visitor's first unsafe action
mints a device record** (CSRF bootstrap), and an unconditional `getUser()` attach would mint one on
first page load. The rotated path can mint again mid-session.

**How to prevent it:**

1. Client: attach only on an explicit path allowlist — login, register, 2FA verify, passkey,
   password/email change, step-up. **Never** `/csrf/v1/generate`, never `/user/v1/get`
   (A4 §7's own fix).
2. Server: the device mint/link endpoint must be a CSRF-guarded POST off the static-CSRF exempt
   list (`src/middlewares/static_csrf.rs:56-70`) and must require an authenticated session
   (login/link only). `/csrf/v1/generate` must touch no device table.
3. Server: "missing payload = unknown device, never new device" (DESIGN-v1 B2) — no payload, no
   record.
4. Test: assert `/csrf/v1/generate` and `/user/v1/get` create zero `device_records` rows, and a
   source assertion that the two call sites carry no device header (A4 §7's guard list).

**Final plan must say:** DESIGN-v1 B2 must state the attachment allowlist explicitly (not just
"inside `#request`"), and the server mint path must be gated on an authenticated link event.

---

## 8. The CSP claim — A1 F4 vs B10 §1.10

**Verdict: B10 is right; A1's "remove gstatic/googletagmanager" is false in effect.**

Evidence:

- `web/src/lib/firebase/analytics.ts:18-20`: `const { isSupported, getAnalytics } = await
import("firebase/analytics"); … analytics = getAnalytics(core);` — unconditional on boot.
- `web/src/lib/boot/analytics.ts:9,22-27`: `startAnalytics()` imports the module, calls
  `initAnalytics()`, applies consent, then `pageView(location.pathname)`.
- `web/src/routes/+layout.svelte:90,98`: `consent.hydrate(); … const cleanups =
[startAnalytics(), startCrashReporting()];` — runs on every app layout mount.
- `web/src/lib/stores/consent.svelte.ts:16`:
  `const DEFAULT_FLAGS: ConsentFlags = { analytics: true, performance: true, advertising: false };`
  → **consent analytics is ON by default**; `hydrate()` only overrides when a stored value exists.
- The URLs are inside the Firebase bundle, not the source. Verified:
  `node_modules/.bun/@firebase+analytics@0.10.22+80db1b7a714a6e79/node_modules/@firebase/analytics/dist/esm/index.esm.js`
  contains `https://www.googletagmanager.com/gtag/js`. GA4/gtag then loads its assets from
  gstatic (CSP allows it) and beacons to `google-analytics.com` (CSP `connect-src`,
  `hooks.server.ts:73`).
- CSP `script-src` pushes `https://www.gstatic.com`, `https://www.googletagmanager.com`
  (`hooks.server.ts:82`) and `https://challenges.cloudflare.com` (`:84`, with `frame-src` at `:96`);
  `web/server.ts:230-232` has gstatic + gtm but **no** `challenges.cloudflare.com` and no
  `frame-src`; `web/static/_headers` has all three. The original CSP commit (`85069d7d`) says the
  entries are for "firebase apis + GA" — they were added _because_ the app loads them.
- Turnstile (`web/src/lib/auth/turnstile.ts:30`, `SCRIPT_SRC =
https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit`) loads only when
  `turnstileEnabled()` — i.e. `PUBLIC_TURNSTILE_SITE_KEY` is non-empty; the compose default is
  empty (`docker-compose.yml:73`), and production password registration requires the API secret,
  so prod is expected to enable it.

**Definitive answer:** by default, scripts from `googletagmanager.com` (gtag.js) and
`gstatic.com` (GA4 loader assets) run on every page; `challenges.cloudflare.com` runs on the auth
pages once a Turnstile sitekey is configured. The real XSS surface is therefore the GA4 loader
chain plus Turnstile — exactly the surface A1 says "runs first-party and can read the token". The
correct fix is A1's _other_ recommendation (HttpOnly cookie), not deleting the CSP entries: deleting
them breaks live Analytics on every page.

**Final plan must say:** keep the three CSP origins; do not touch `script-src` for this feature.
Token must be HttpOnly. Separately (A5's domain, not this plan), analytics default-on with no
consent prompt is a compliance defect — `DEFAULT_FLAGS.analytics` should be `false` or a consent
gate should precede `getAnalytics`; note that even with `analytics:false`, `initAnalytics()` still
loads the script (Firebase consent mode gates storage, not script load).

---

## 9. Does anything strip client-supplied `X-Eq-Edge-*`?

**Verdict: nothing strips it. A5 §1 L4's "origin strips client-supplied headers — good" is
unsupported; A3 F12 and A9 §4.1(3) are right.**

Evidence:

- `rg -n "x-eq-edge" -i .` → **0 hits** repo-wide (no producer, no consumer, no stripper).
- `src/main.rs:827-858` — the private router layer stack, outermost first: `security_headers`,
  `private_no_store`, `RouteBlockerLayer`, `cors`, `session_layer`, `csrf_guard`, `Extension(state)`,
  `origin_guard`, `compression`, `InternalApiToken`, `TrustedProxyCidrs`, `ip_source`,
  `resolve_client_ip`. No header-removal middleware anywhere.
- `src/middlewares/client_ip.rs` reads headers (`x-easyquran-internal-token` :92, `ClientIp` :124)
  and never removes any.
- `docker-compose.yml:181-192` — Traefik api-router middlewares = `strip-api@docker` only
  (`stripprefix.prefixes=/api`); no `headers` middleware, and Traefik's headers middleware cannot
  delete request headers anyway.
- `crates/rux-webhook-crypto/src/lib.rs:28-34` (`header_str` uses `HeaderMap::get`) → first-value
  wins; a client-supplied header survives to any future verifier.

**Final plan must say:** add an outermost axum middleware (layered last, i.e. outermost in
`main.rs`'s stack) that removes every `X-Eq-Edge-*` header from the inbound request before any
other layer, and logs when it actually removed something (the smuggling detector). The verifier
must read with `get_all` and require exactly one value per name. A5's sentence must be struck; the
Worker, if ever built, must `set` never `append`.

---

## 10. `user_bans` writers and `check_ban` reachability

**Verdict: zero writers confirmed. `check_ban` is _not_ reachable from every auth path — it is
wired into the guarded routes that opt into `not_banned()`, and into every interactive login
controller.**

Evidence — writers:

- Actions exist: `src/db/sea_models/user_ban/actions.rs:11` (`create`), `:31` (`revoke`),
  `:56` (`get_active_ban`), `:102` (`admin_list`).
- Callers: `rg -n "user_ban::Entity" src crates` → only `src/services/auth.rs:530-531`
  (`get_active_ban`). No production path calls `create`/`revoke`; no route in `src/router.rs`
  creates a user ban (the only `ban` routes are `admin_bans_v1` = IP bans, `router.rs:153`).
- `NewUserBan::permanent()` (`db/sea_models/user_ban/slice.rs:5-12`) is callable but has no caller.

Evidence — `check_ban` reachability:

- Enforcement lives in `crates/rux-auth/src/middleware/guard.rs:46-55`, gated on
  `requirements.not_banned`, and only queries the DB when the session cache is stale
  (`ban_cache_duration` default 5 min, `crates/rux-auth/src/requirements/builder.rs:8,16,41-44`;
  `session/state.rs:50-63`).
- `not_banned` is opt-in and set by: `authenticated` (`src/middlewares/auth_guard.rs:31`),
  `verified` (`:66`), `verified_with_role` (`:84`). It is **not** set by `unverified` (`:53`) — so
  `email_verification_v1` routes are not ban-checked.
- Interactive logins check explicitly: password `src/modules/auth_v1/controller.rs:153`, TOTP
  `:285`, re-auth/step-up `:431`, OAuth `src/services/oauth/login.rs:318`, passkey
  `src/modules/passkey_v1/controller.rs:240`.
- Because there are zero writers, `get_active_ban` currently always returns `None`, so the entire
  ban path is inert today; and even after wiring, a new ban takes up to 5 minutes to reach an
  existing session (session `ban_checked_at` cache).

**Final plan must say:** wire `user_bans` create/revoke + `/admin/users/bans` (DESIGN-v1 B1) before
relying on any account-level ban; add `not_banned` to the `unverified` guard (or justify the
exception); document the ≤5-minute propagation lag and the fact that no writer exists today.

---

## Summary table

| item                       | TRUE claim                                                                                                                                                               | key evidence                                                                                                                                                                                                          | what the final plan must say                                                                                                                                                                                                     | NEW?                                                              |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 1 Migration atomicity      | A10 F8 / B6 F1: SQLite migrations are not transactional; version row written after `up()`; re-runs after a crash. SQLite DDL _is_ transactional when wrapped in `BEGIN`. | `sea-orm-migration-1.1.20/src/migrator.rs:261-272,386-397`; `sea_connect.rs:87-93`; `main.rs:168` vs `:939`; empirical `BEGIN; CREATE TABLE; ROLLBACK`; `rate_limit_store.rs:190`                                     | All-`IF NOT EXISTS` m000012 + `PRAGMA table_info` guards + up-twice test + wrap the DDL batch in `conn.transaction`. Strike A3 §8.                                                                                               | no (A10 §0/F8, B6 F1; B10 §1.1)                                   |
| 2 Token storage            | B1/A1: `Path=/api` cookie never reaches `/t/**`; cache predicate is bare cookie presence. `ruxlog.sid` is `Path=/`, so the cache is already dead for session-holders.    | `hooks.server.ts:346-351,427,130-137`; `quran-disk-cache.ts:12,51-57`; `docker-compose.yml:181-192`; `main.rs:781-788`; tower-sessions `service.rs:137,243`                                                           | HttpOnly `ruxlog.device` cookie with explicit `Path=/api`; rewrite A4's "no cookie" to "no cookie on reader paths"; keep the guards.                                                                                             | no (B1 F1/F2; B10 §1.3)                                           |
| 3 Edge verdict             | Cut from v1. No Worker/wrangler/`X-Eq-Edge` exists; origin can only trust the peer-gated CF ingress path and can verify none of the claimed fields.                      | `rg x-eq-edge`/`wrangler` → 0 hits; `docker-compose.yml:181-192,143`; `client_ip.rs:64-74,113-121`; `rg cf-ray` → 0 hits; A7 F7                                                                                       | v1 origin-only; edge removed from scope, retained as defer-until-consumer with a named decision it gates.                                                                                                                        | no (A7 F7, A9 §8, B9 §9)                                          |
| 4 Fail-open vs fail-closed | A9's policy matches house style: present-but-invalid optional header degrades + warns; only mandatory identity gates reject.                                             | `client_ip.rs:92-106` vs `:133-140`; billing webhooks reject because mandatory                                                                                                                                        | Invalid attestation never 401/403; enforcement only via per-endpoint opt-in flag. Reject A3's blanket fail-closed and A1's GET/POST split.                                                                                       | no (A9 F11; A3 §7 cites the same precedent)                       |
| 5 `UNIQUE(device_key)`     | Drop it (A2 F1/A9/DESIGN-v1); token_hash is the identity/conflict target.                                                                                                | `A10-data-model.md:134,136`; `A2-false-positives.md:310-321`; `DESIGN-v1.md:10`                                                                                                                                       | `UNIQUE(token_hash)` replaces it; `device_key` gets a non-unique index; rewrite A10's upsert/test text.                                                                                                                          | no (A2 F1, DESIGN-v1:10; B10 §1.7)                                |
| 6 Two m000012 schemas      | Neither is writable; both lack `expires_at`. One merged DDL produced (§6 above).                                                                                         | `A9-crypto-keys.md:137-168`; `A10-data-model.md:100-219`; `A5-privacy-legal.md:275-277`; `DESIGN-v1.md:22,38-42`                                                                                                      | Ship the merged schema: TEXT ULID PK, `token_hash TEXT NOT NULL UNIQUE`, non-unique `device_key`, `trust_state DEFAULT 'observe'`, `expires_at NOT NULL`, `device_bans` included, no user/user_session FKs on `device_sessions`. | **NEW** (merged DDL; B10 §1.8 only flagged the conflict)          |
| 7 Anonymous device records | Real: CSRF bootstrap and session probe are anonymous `#request` calls; an unconditional attach mints a record per visitor.                                               | `auth-client.ts:231-238,240-251,257-260,269-283`; `csrf_v1/controller.rs:10-25`; A4 §7                                                                                                                                | Explicit attach allowlist (never `/csrf/v1/generate`, never `/user/v1/get`); server mints only on authenticated link events; test zero records from those paths.                                                                 | no (A4 §7(a); B10 §1.9)                                           |
| 8 CSP                      | B10: Firebase Analytics is live and default-on; gstatic/gtm are loaded from inside the bundle; A1's removal breaks Analytics.                                            | `firebase/analytics.ts:18-20`; `boot/analytics.ts:9,22-27`; `+layout.svelte:90,98`; `consent.svelte.ts:16`; bundle URL `…/@firebase/analytics/dist/esm/index.esm.js`                                                  | Keep the CSP origins; fix token theft with HttpOnly, not CSP deletion. Note analytics default-on as a separate consent defect.                                                                                                   | no (B10 §1.10); bundle-URL evidence added                         |
| 9 Edge header stripping    | Nothing strips `X-Eq-Edge-*`; first-value-wins at any verifier.                                                                                                          | `rg x-eq-edge` → 0 hits; `main.rs:827-858`; `docker-compose.yml:181-192`; `rux-webhook-crypto/src/lib.rs:28-34`                                                                                                       | Add an outermost stripping middleware + `get_all` exactly-one verification; Worker must `set` not `append`. Strike A5's "origin strips".                                                                                         | no (A3 F12, A9 §4.1(3); B10 §1.11)                                |
| 10 `user_bans`             | Zero writers; `check_ban` only on guards that opt into `not_banned()` (not `unverified`) and on every login controller; 5-min cache.                                     | `user_ban/actions.rs:11,31,56`; `services/auth.rs:530-531`; `auth_guard.rs:31,53,66,84`; `builder.rs:16`; `guard.rs:46-55`; `auth_v1/controller.rs:153,285,431`; `oauth/login.rs:318`; `passkey_v1/controller.rs:240` | Wire create/revoke + `/admin/users/bans`; add `not_banned` to `unverified` or justify; document the 5-min lag and the zero-writer state.                                                                                         | partly — writers: no (A8 F1/B2); `unverified()` omission: **NEW** |

## Appendix — unresolved by this pass

- Nothing in the repo defines or consumes `cf-ray`; the "bind cf-ray + single-use dedupe"
  construction (DESIGN-v1:47) has no existing plumbing and A9's dedupe still conflicts with A7's
  hot-path budget (B10 §1.14). Not resolved here because it belongs to the enforcement design, not
  to a factual contradiction.
- The two `device_v1` (FCM push tokens) vs new `device_records` naming collision is real
  (`rust/backend/api/src/modules/device_v1/`, `m000005:13-23`); the merged schema keeps the
  `device_records` name to avoid it. No report disputes this.
