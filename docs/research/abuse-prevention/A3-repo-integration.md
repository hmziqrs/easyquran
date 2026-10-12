# A3 — Integration-correctness audit of DESIGN-v0 (device identity)

Scope: does the design fit the Rust API + web + edge as they actually exist today. All claims
carry file:line evidence read from the working tree on 2026-10-12. Known-already items
(single-writer pool rule, `audit_logs` writer-less, `user_bans` create path, `devices` = FCM,
no revoke-all, no admin UI in web, ADDRESS_HEADER unset, closed `RequestIdentity`, no DB CSRF)
are **not** re-reported.

---

## 1. `AuthSessionState` wire format, size, and the 5-minute ban cache

### 1.1 The payload is JSON-inside-msgpack, and a new field breaks every live session

`AuthSessionState` (`crates/rux-auth/src/session/state.rs:6-26`) has **no serde attributes**.
tower-sessions 0.14 stores `Record.data` as `HashMap<String, serde_json::Value>`
(`tower-sessions-core-0.14.0/src/session.rs:16,26,919`), so `session.insert(SESSION_KEY, &state)`
is `serde_json::to_value(...)` (session.rs:171-174), and the whole record is then
`rmp_serde::to_vec(record)` (`src/services/session_store.rs:142,167`). Field names are stored as
JSON keys inside the msgpack blob.

Consequence of adding a field without `#[serde(default)]`:

- Old payload (missing key) → `serde_json::from_value` error → `session::Error::SerdeJson`
  (`session.rs:257-262`).
- `AuthSession::new` swallows it: `.ok().flatten()` → `None` (`extractor.rs:44-45`) → the user is
  silently **anonymous** (every `auth_guard` route 401s).
- The `FromRequestParts` path propagates it: `session.get(SESSION_KEY).await?`
  (`extractor.rs:239-241`) → `From<tower_sessions::session::Error>` → `AuthErrorCode::SessionError`
  (`crates/rux-auth/src/error.rs:175-179`) → **HTTP 500** (`error.rs:71`). This path is used by
  handlers that take `AuthSession` directly, e.g. `auth_v1::log_out` (`auth_v1/controller.rs:73-75`),
  `sessions_list`/`sessions_terminate` (`:976-1045`), and `admin_bans_v1` (`controller.rs:91,120`).

So the deploy that adds the first device field logs out every user and 500s a subset of endpoints,
with no test covering it. Fix: `#[serde(default)]` on every added field + append-only field order
(rmp/JSON both tolerate a trailing key), or a versioned key (`SESSION_KEY = "rux_auth_v2"`) plus a
one-release dual-read. Add a wire-compat test that inserts a hand-written legacy JSON payload
(field absent) and asserts the session still authenticates.

### 1.2 Size sensitivity is real but not the JSON key

`session_auth_hash: Vec<u8>` already serializes as an array of numbers under serde_json
(`state.rs:19`), which is the blob's dominant cost. Adding a 64-char hex device key is ~+70 bytes
per row; adding raw signal bytes (e.g. an extension-set hash as `Vec<u8>`) costs up to 4 chars per
byte. Rule: the session may carry the **device record id (i64) or the hex HMAC key only**; never
raw signal material. The row is rewritten whole on every `save()` (`session_store.rs:166-186`), and
`update_ban_status` forces a save (`extractor.rs:143-152`).

### 1.3 Where `with_metadata` must change, and why `device` is unusable as a device key

`AuthSessionState::with_metadata(device, ip_address)` (`state.rs:44-48`) is reached only through
`AuthSession::login_with_metadata` (`extractor.rs:111-129`). Call sites:

- `auth_v1/controller.rs:188-190` — `device` = the raw **User-Agent** header (`:183-186`).
- `auth_v1/controller.rs:347-351` — TOTP login passes `device: None, ip: None` (so 2FA sessions
  have no device/ip at all, and the audit row is NULL/NULL).
- `passkey_v1/controller.rs:262-265` — `device` = the WebAuthn `device_type`
  ("singleDevice"/"multiDevice").

So `AuthSessionState.device` and `user_sessions.device` are already overloaded across three shapes
(UA string / NULL / WebAuthn device_type). Any "device changed" comparison against that field fires
spuriously on browser auto-updates (UA changes) and on every 2FA login (NULL → UA). The design must
add a **new** field (`device_key` / `device_record_id`) and pass it through a new
`login_with_metadata` variant, leaving `device` untouched — also because `admin_list` filters
`Column::Device.contains("%…%")` (`db/sea_models/user_session/actions.rs:109-112`) and
`sessions_list` shows it to users (`auth_v1/controller.rs:1016-1038`).

### 1.4 The 5-minute ban cache: what it does and what it costs

Trace (`crates/rux-auth/src/middleware/guard.rs:46-56`):

```rust
if requirements.not_banned {
    if state.ban_cache_stale(requirements.ban_cache_duration) {   // default 5 min
        let ban_status = auth.backend().check_ban(&user.id()).await?;   // DB read
        auth.update_ban_status(&ban_status).await?;                     // session WRITE
        if ban_status.is_banned() { return Err(Banned); }
    } else if state.is_banned { return Err(Banned); }
}
```

- Default TTL: `AuthRequirements::new()` → `Duration::minutes(5)` (`requirements/builder.rs:16`).
- `ban_checked_at`/`is_banned` live in the session payload (`state.rs:14-16`), are refreshed only
  here, and are read **nowhere else** in the codebase (grep: only `guard.rs:53` reads
  `state.is_banned`).
- `not_banned` is only attached by `authenticated()`, `verified()`,
  `verified_with_role()` (`middlewares/auth_guard.rs:31,66,84`) and `admin_or_export_token`
  (`admin_bans_v1/controller.rs:209-216`). Routes without those guards never refresh it.

Consequences for the design:

1. **A device ban cannot ride this cache.** The cache is a _user_-ban state; reusing `is_banned`
   for a device ban means a lifted device ban keeps blocking for up to 5 min (hysteresis mismatch
   with the design's restore<=40 rule) and a banned device is invisible for up to 5 min after the
   ban lands. A separate `device_checked_at` + device state field (or an L1 store read) is required.
2. **A device-change step-up is also delayed by the same window** if implemented as another cached
   boolean. For "device changed mid-session", the only immediate mechanisms that exist are the
   in-memory revocation set (`services/auth.rs:583-595`) or deleting the session — not a cache field.
3. **Cost of a per-request device check** — exact numbers:
   - A per-request **DB** read: +1 `SELECT` per authenticated request. Today an authenticated
     request already does: 1 lazy session-store `SELECT` (`session_store.rs:188-198`, lazily loaded
     once per request and memoized — `tower-sessions-core-0.14.0/src/session.rs:108-130`) + 1 user
     `SELECT` (`services/auth.rs:520-527`) + a `user_ban` `SELECT` every 5 min. With
     `max_connections(1)` (`db/sea_connect.rs:32-44`) all of these serialize on one connection,
     which the 10 s flush task also holds (`rate_limit_store.rs:126-178`). So the marginal cost is
     ~+50 % round-trips on the private hot path, and it is the third serialized read per request.
   - A **cached** read (session field + TTL): 0 extra reads; at most 1 extra session-row write per
     TTL per session (the same write the ban refresh already does).
   - An **L1** read (`InMemoryStore::ban_status`, `rux-request-gate/src/store.rs:370-389`): 0 DB
     I/O, one extra lock acquisition on the store mutex; durability via the existing snapshot/flush.
     Recommendation: L1 for the ban/trust decision, session field for the device identity, DB only at
     login and on refresh.

## 2. `BanUnit::Device` — what must change and which invariants break

`BanUnit` (`modules/admin_bans_v1/dto.rs:16-43`) is deliberately IP-only and its doc comment states
the invariant: _"parse() accepts only a /32 or /64 width, so an unparseable or wrong-width store key
(email, user-id, totp, fixed-rate) can never pose as an exportable ban unit."_

Adding `BanUnit::Device(...)` breaks, in order of severity:

1. **The export contract (external, out of repo).** `deploy/README.md:421-436`: the export feed
   emits "**only** active `quran-ban:` rows whose suffix parses as a valid `/32` or `/64` unit",
   the shape is `{"banUnit","scope","expiresAt"}`, and the consumer is a _deployment-owned_
   Traefik/Cloudflare adapter that blocks CIDRs at the edge. A device unit in that feed is
   unenforceable by the adapter (or a parse failure in it). `ExportRow`
   (`dto.rs:93-100`) has no unit-type discriminator, so the adapter cannot tell them apart.
   Fix: keep `BanUnit` IP-only; filter the export to IP units; if a device axis must be visible,
   add `unitType` and version the feed.
2. **Admin list/delete.** `filter_active_ban_rows` (`controller.rs:41-70`) only accepts keys whose
   suffix `BanUnit::parse` succeeds. If parse accepts device keys, device bans appear in
   `/admin/bans`; if not, an operator **cannot lift a device ban at all** (the delete route
   `controller.rs:118-160` also requires `BanUnit::parse`). Either way the un-ban path must be
   extended with a device route.
3. **`delete_ban`'s two-key cleanup is IP-shaped.** It deletes `quran-ban:{unit}` and
   `ratelimit:{unit}:quran-v1` (`controller.rs:128-153`). The limiter never increments
   `ratelimit:{device}:…`: `id_key` is IP/internal/unknown only
   (`rux-request-gate/src/layer.rs:228-243,278`). So the fixed-counter half of the cleanup would
   target a key that never exists, while the device's real enforcement key lives elsewhere.
4. **`canonical_unit_key` mirror.** `layer.rs:368-390` mirrors `BanUnit::from_ip` byte-for-byte and
   is pinned by `layer.rs:569-594`. A device unit cannot be mirrored (there is no IP); the mirror
   test and the "one canonical BanUnit keys fixed limiting + history + bans + persistence + export"
   invariant (`layer.rs:229-238`) must be restated as IP-only, with the device axis in its own
   namespace (`device-ban:{key}`).
5. **`is_external_ip()` gating.** `RequestIdentity` is a closed enum
   (`rux-request-gate/src/ip.rs:38-57`) and the limiter/escalation only enter the ban state machine
   for `RequestIdentity::External` (`layer.rs:252-267,297-321`;
   `services/escalation.rs:72-78,129`). A device identity is not an `IpAddr`, so it can never reach
   `Escalation::active_ban`/`on_first_block`. Device bans need a separate evaluation path, not a new
   `BanUnit` variant threaded through the existing engine.
6. **Allowlist validation is CIDR-only and boot-fails.** `EscalationConfig.allowlist: Vec<IpNet>`
   (`config/settings.rs:313-326`) is parsed from comma-separated CIDRs (`:384-405`) and
   `enabled=true` with an unset allowlist is a boot error (`:348-363`). The comment at `:381-383`
   explains the /64 floor is _export-coupled_ ("proxy export blocks /64 units and cannot preserve a
   narrower exception"). There is no way to allowlist a device key, so staff/test devices have no
   exemption path, and the "enabled ⇒ allowlist present" guarantee silently does not cover the
   device axis. Fix: a separate device allowlist (explicit device ids), validated at boot with the
   same fail-closed rule.
7. **Capacity accounting is cross-contaminated.**
   `InMemoryStore::active_ban_count()` counts _every_ bucket with an active block
   (`store.rs:207-215`) and gates new ban creation (`escalation.rs:254-261`); device bans would
   consume `QURAN_ACTIVE_BAN_MAX` and can stop IP ban creation. `ban_unit_history_count()` counts
   only `quran-ban:` keys (`store.rs:231-238`), so device history escapes
   `QURAN_ESCALATION_MAX_IDENTITIES` (10 000) entirely. `set_block` itself is key-agnostic
   (`store.rs:397-409`) and `snapshot`/`flush` persist all keys (`store.rs:123-155`,
   `rate_limit_store.rs:180-229`), so a device key _will_ be persisted and _will_ count toward the
   active-ban cap unless the counters are made axis-aware.
8. **Attacker-rotatable key space.** The device token is client-held (DESIGN-v0 §Layer 1: server
   minted, stored in localStorage + IDB); clearing storage yields a fresh key. IP keys are bounded
   by the capacity gates plus the practical cost of rotating IPs; device keys can be rotated per
   request. Do not let device keys create Long bans on volume alone (the design's own "ban only with
   corroboration" rule) and cap device history separately.

## 3. Where a per-request device evaluation can live (middleware order, exact)

Layer semantics in this repo: `.layer()` added later is **outer** and runs first
(`src/main.rs:838-841` comment: "Layers added later run first"). Verified order for a private API
request (`main.rs:827-858`):

```
security_headers → private_no_store → route_blocker → cors → session_layer → csrf_guard
  → Extension(state) → origin_guard → compression → InternalApiToken → TrustedProxyCidrs
  → ip_source → resolve_client_ip → [nest-level rate_limit → module route_layer auth_guard → handler]
```

The rate limiters are **inside** `router::router(state)` (`src/router.rs:72-177`), i.e. inner to
every private-router layer. Therefore:

- A device layer added at the private-router level **cannot** run after the rate limiter, no matter
  where it is inserted. It also would double the session+user load (`middlewares/auth_guard.rs:8-15`
  already builds `AuthSession` per guard) — two `SELECT`s per request instead of one.
- To have the limiter run first, the layer must be inserted _between_ the module router and the
  nest's limiter in `router.rs`, e.g.
  `auth_v1::routes().layer(middleware::from_fn(device_eval)).layer(rate_limit::rate_limit_layer(...))`
  — `rate_limit` is added last ⇒ outer ⇒ runs first (`router.rs:73-76`).
- **Recommended:** put the check in `check_requirements` (`crates/rux-auth/src/middleware/guard.rs:9-67`)
  behind a new `AuthRequirements` flag (e.g. `.not_device_banned()`), because `auth_guard` is
  _already_ inner to the limiter and already holds the loaded `AuthSession`, the user row and the
  ban cache. Coverage then equals the coverage of `.not_banned()` exactly (routes without a guard
  today also skip user-ban checks — e.g. `/csrf/v1/generate`, `/analytics/v1`).

Affordability with pool = 1 (`db/sea_connect.rs:32-44`): a per-request device **DB** read is the
third serialized read per authenticated request (session load + user load + device) and it competes
with the 10 s flush writer and `PERSISTENCE_LOCK` (`rate_limit_store.rs:47,126-178`). Even at the
private API's volumes (60-200 req/min/IP per nest, `router.rs:73-177`) it would take most of the
authenticated DB budget (A7 F1: 60–95 %), and the public Quran path (600/min/IP,
`middlewares/rate_limit.rs:57-63`) rules it out. Use L1 + a session field, not a per-request `SELECT`.

## 4. Indexes needed, and the write-amplification bill

Existing schema has **no** indexes beyond PKs and the two UNIQUEs created by later migrations:
`user_sessions` (`migration/src/m000001_init.rs:48-57`), `user_bans` (`:58-73`), `audit_logs`
(`:252-264`), `sessions` (`src/services/session_store.rs:15-19`).

| Table             | Query today                                                                                                          | Index needed                                                                                                                       |
| ----------------- | -------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `user_sessions`   | `list_by_user`: user_id + `revoked_at IS NULL` + `ORDER BY last_seen DESC` (`user_session/actions.rs:83-88`)         | `(user_id, revoked_at, last_seen)`                                                                                                 |
| `user_sessions`   | `admin_list` filters `seen_since`, sorts `last_seen` (`actions.rs:125-148`)                                          | `(last_seen)`                                                                                                                      |
| `user_sessions`   | `admin_list` device/ip filters use `LIKE '%…%'` (`actions.rs:109-117`)                                               | **no index can help** (leading wildcard); say so explicitly                                                                        |
| `user_bans`       | `get_active_ban`: user_id + `revoked_at IS NULL` + expiry + `ORDER BY created_at DESC` (`user_ban/actions.rs:59-64`) | `(user_id, revoked_at, expires_at, created_at)`                                                                                    |
| `sessions`        | hourly sweep `WHERE expiry_date <= ?` (`session_store.rs:52-62`)                                                     | `(expiry_date)` — currently a full scan every hour                                                                                 |
| `device_records`  | design §Layer 2                                                                                                      | `UNIQUE(device_key)`, `(user_id)`, `(last_seen)`                                                                                   |
| `device_sessions` | per-session lookup + per-device history                                                                              | `UNIQUE(user_session_id)` (mirrors `auth_session_binding`'s `idx_auth_session_binding_user`, m000004:17-18) + `(device_record_id)` |
| `audit_logs`      | evidence lookup by device/resource                                                                                   | `(resource_type, resource_id)`, `(user_id, created_at)`                                                                            |

Write amplification on SQLite: every added index is one extra B-tree insert per row write, inside
the same implicit transaction. That is cheap for `user_sessions` (written at login/terminate only —
`Entity::touch` has **no production caller**; only `tests/fresh_auth_security.rs:78` calls
`AuthSession::touch`). It is **not** cheap for `device_records` if `last_seen`/`seen_count` are
updated per request: that is a hot-path write, which `services/rate_limit_store.rs:1` forbids
("Do NOT add a DB write to the request hot path") and which multiplies by each index. Coalesce
device observations in memory and flush them on the existing 10 s tick (pattern:
`rate_limit_store.rs:126-178`).

## 5. `device_sessions` FK/cascade policy

`PRAGMA foreign_keys=ON` is set at connect (`db/sea_connect.rs:46-56`), so declared cascades are
live. `user_sessions.user_id → users ON DELETE CASCADE` (`m000001:55-56`) and
`user_bans.user_id → users ON DELETE CASCADE` (`:67-68`); `audit_logs.user_id → users ON DELETE SET
NULL` (`:262-263`). Account deletion goes through `User::admin_delete` (`user_v1/controller.rs:294`).

- `device_sessions` must **not** declare `user_session_id … ON DELETE CASCADE`: deleting a user
  cascades to `user_sessions` (`m000001:55-56`), which would cascade again and erase the
  ban-evasion evidence.
- It must **not** declare `RESTRICT`/default `NO ACTION` either: with FKs enforced, the
  `user_sessions` cascade would then fail and account deletion would 500 while evidence exists.
- Correct: mirror `auth_session_binding` (m000004:11-15) and declare **no FK** (app-level
  invariant, documented), or make the column nullable with `REFERENCES user_sessions(id) ON DELETE
SET NULL`. Keep `device_records.user_id` nullable with `ON DELETE SET NULL` exactly as the design
  says, so the device record survives; link evidence through `device_records`, not through
  `user_sessions`.
- Add a test: delete a user, assert the `device_records`/`device_sessions` rows survive with a NULL
  user link. Existing precedent for "survives account deletion" is `audit_logs`' SET NULL FK.

## 6. Step-up: can `fresh_auth` express "this session's device changed"?

Machinery (`src/services/fresh_auth.rs`):

- `Grant` has `purpose, session_id, user_id, credential_secret, password, factor_secret,
factor_enabled` (`:25-34`) — **no device axis**; `matches` compares session id + credential
  secrets only (`:36-45`). A grant issued on device A is therefore valid on device B. The design's
  "device change triggers step-up" cannot be implemented with this struct as-is; add
  `device_key: String` to `Grant` (and to `pending_login::PendingGrant`, `pending_login.rs:20-40`)
  and compare it in `matches`.
- Grants are process-local (`static GRANTS: OnceLock<Mutex<GrantMap>>`, `:59-64`) while the token is
  persisted in the session payload (`:181-187`, key `rux_auth_fresh_grant`, `:13`). After a restart
  the token is still in the session but the map is empty → `available()` is false → fail-closed
  (users re-auth after a deploy). That is a usability cost, not a security hole; document it.
  The _hole_ is the opposite direction: because `Grant` does not bind the device, a stolen session
  cookie carries a valid token and can consume the grant from a new device.
- There is **no way to invalidate a grant** (only `consume`, `:246-290`). "Force step-up now" needs
  a new `fresh_auth::invalidate(auth, purpose)` (map remove + session key remove) and a new
  `Purpose` variant + `SESSION_KEY` so a device-change grant does not fight the login grant —
  `issue_for` already deletes any existing grant with the same `(session_id, purpose)`
  (`:173-175`).
- **"New device at login" is already covered**: `create_bound_session` issues a `SecurityAction`
  grant at the end of every login (`auth_v1/controller.rs:1167-1172`). A device layer must not
  duplicate or pre-empt it.

## 7. Edge signing: nothing strips `X-Eq-Edge-*`; reuse the internal-token pattern

- Traefik config: `docker-compose.yml:181-192` — only `strip-api` (StripPrefix) and `compress`. No
  header middleware, and Traefik's `headers` middleware cannot _remove_ a request header. So a
  client-supplied `X-Eq-Edge-*` arrives at `api:8888` untouched.
- The origin never strips either: `resolve_client_ip` reads `x-easyquran-internal-token`, compares
  constant-time, and on a **present-but-invalid** token only warns and falls through
  (`middlewares/client_ip.rs:92-106`) — the header is not removed from the request.
- The peer gate that makes `CF-Connecting-IP` unforgeable is only as narrow as
  `TRUSTED_PROXY_CIDRS`, whose compose default is **all of RFC1918 + fd00::/8**
  (`docker-compose.yml:143`); `deploy/README.md:377-409` states the real guarantee comes from
  restricting origin ingress to Cloudflare ranges outside the app.
- Existing mechanism to reuse: the `InternalApiToken` pattern — secret read from settings, layered
  as an `Extension` in `main.rs:824-835`, constant-time compare, never logged
  (`client_ip.rs:12-20`), with the SSR caller setting the header only for the internal base
  (`web/src/lib/server/quran-translation-page.ts:43-77`). For edge signing the same shape applies,
  but the new middleware must additionally: (1) be **outermost** (before `resolve_client_ip` at
  `main.rs:828` and before the route blocker) and **remove** every inbound `x-eq-edge-*` header
  before verification, (2) verify HMAC-SHA256 over a canonical string with `kid` + a ±30 s
  timestamp, using a secret layered as an extension, (3) fail **closed** in production when the
  header is present but invalid (unlike the internal token, which falls through). Derive the HMAC
  key with the repo's labeled-HKDF precedent (`static_csrf.rs:17-47`) instead of reusing
  `COOKIE_KEY` raw. Also: if the device token is sent as a new browser header and the deployment
  ever sets `PUBLIC_API_BASE_URL` cross-origin, `main.rs:799-805` (`allow_headers`) must list it or
  preflight fails.

## 8. Migration mechanics: rollback, safety, precedent

- Auto-run at boot: `db/sea_connect.rs:87-93` → `Migrator::up(&conn, None)`. On SQLite sea-orm does
  **not** wrap a migration in a transaction — only Postgres is wrapped (`sea-orm-migration-1.1.20/src/migrator.rs:261-272`) —
  and the version row is inserted only after `up()` returns (`:386-397`), so a crash mid-migration re-runs a
  partly applied `up()`. Every statement must be idempotent, and a multi-statement batch should be wrapped in an
  explicit `conn.transaction`.
- **Rollback (`down`)** is only reachable manually (`migration/bin/migrate.rs:63-69`,
  `Migrator::down(&connection, steps)`); `steps: None` rolls back **everything**, i.e. m000001's
  `down` drops all tables (`m000001:318-347`). Never document a bare `down`; use `--steps 1`.
- A new migration that adds columns with data must not attempt to restore data in `down()`. The
  precedents: m000002's `down` drops the whole table (`m000002:45-51`), m000008's `down` drops only
  the index ("dedupe and normalization are irreversible data changes", `m000008:74-82`), m000009's
  `down` is a documented no-op (`m000009:121-124`), m000011's `down` drops only the unique index
  (`m000011:27-33`). m000011 is also the data-migration precedent: clean conflicting rows first
  (`m000011:10-19`), then create the UNIQUE index — i.e. any new UNIQUE on device data must
  pre-dedupe in the same `up`.
- **Never rebuild a table in the new migration.** `PRAGMA foreign_keys=ON` is set at connect
  (`sea_connect.rs:50`) and stays live during migration DDL, and inside an explicit transaction
  `PRAGMA foreign_keys` is a **no-op** — so a 12-step table rebuild cannot disable FK enforcement: `DROP TABLE user_sessions`
  would fire `ON DELETE CASCADE` into child tables (including the design's `device_sessions`) or
  fail. Keep the new migration (next free number — see `docs/next-features.md` reservations) strictly additive: `CREATE TABLE/INDEX IF NOT EXISTS`, and `ALTER TABLE …
ADD COLUMN … NOT NULL DEFAULT <constant>` guarded by a `PRAGMA table_info` check exactly like
  m000002:19-38. Note the CLI path (`migrate.rs`) does **not** call `configure_sqlite`, so FK
  enforcement differs between `just api-migrate` and boot — another reason not to rely on FK
  behavior inside migrations.
- Live-DB lock: SQLite has no `CREATE INDEX CONCURRENTLY`. A large index build inside the migration
  transaction takes the write lock; during a rolling deploy the old container's writes hit
  `busy_timeout=5000` (`sea_connect.rs:49`) and can fail (the 10 s flush logs and continues; login
  writes would 500). Build indexes while the old container is drained, or accept and document the
  window. Add the idempotency + index-presence tests in the m000004 style (`m000004:87-115`).

## 9. Tests that fail when the design lands, and guards to extend

**Compile-break (build failure) if `HttpSettings` or `RateLimitSettings` gains a field** — these
struct literals are exhaustive:

- `rust/backend/api/tests/admin_bans_export.rs:79-108`
- `rust/backend/api/tests/quran_prod_gate.rs:74-103`
- `rust/backend/api/tests/api_access_security_regressions.rs:76-105`
- `rust/backend/api/tests/bookmarks_v1.rs:83-112`
- `rust/backend/api/tests/auth_rotation_header.rs:130-163`
- `rust/backend/api/tests/quran_v1.rs:74-103`
- `rust/backend/api/src/middlewares/route_blocker.rs:187-216` (unit-test fixture)
- `rust/backend/api/src/config/settings.rs:1462` (unit-test literal)

The repo already solved this once: `EscalationConfig` is "Parsed independently here (not as a
RateLimitSettings field) so existing struct literals in other modules compile unchanged"
(`main.rs:863-866`). The edge-signing secret and any device config must follow that precedent.

**Behavioral failures if the device check fails closed on a missing device/binding** — these tests
build sessions with `AuthSession::login` (no `device`, no audit row, no `auth_session_binding`):

- `tests/admin_bans_export.rs:357`
- `tests/api_access_security_regressions.rs:211` (backs all 8 IDOR/bookmark tests in that file)
- `tests/fresh_auth_security.rs:55`
- `tests/bookmarks_v1.rs:222`

So the device check must treat "no device signal" and "session has no device link" as
**unknown/allow** (which is also what "never penalize privacy browsers" demands) — and that is
exactly why device bans can only be corroborated bans, never a standalone enforcement axis.

**Web failures** if the device token is minted/attached inside `AuthClient.#request`
(`web/src/lib/auth/auth-client.ts:167-229`):

- `web/src/lib/auth/__tests__/auth-client.test.ts:49,125,137,174,192,207` — exact fetch call counts
  and URL sequences (an extra mint request breaks them).
- `…/auth-client.test.ts:153-160` — asserts `Storage.prototype.setItem` is **never** called on the
  `#request` path; a device-token write to localStorage there fails it.
- `web/src/lib/server/__tests__/quran-translation-page.test.ts:227-238` — asserts the internal token
  is sent only to the internal base; any header plumbing change must keep `/t/**` untouched.

**Invariant guards to extend:**

- `crates/rux-request-gate/src/layer.rs:569` `canonical_unit_key_mirrors_ban_unit` — restate as
  IP-only and add a guard that device units never enter `quran-ban:`/`ratelimit:{unit}`.
- `modules/admin_bans_v1/controller.rs:299` `ban_unit_rejects_wrong_widths_and_non_ip_keys` and
  `:319` `filter_excludes_email_user_id_totp_and_fixed_rate_keys` — add device-key assertions and an
  export-contains-only-IP-units assertion.
- `services/escalation.rs:505` `allowlisted_identity_never_bans` and `:522`
  `internal_identity_never_bans` — mirror for the device path.
- New: (a) `AuthSessionState` legacy-payload decode guard (the bug in §1.1 has no test today);
  (b) evidence-survives-account-deletion guard (§5); (c) migration index-presence guard
  (m000004:72-85 pattern); (d) no-hot-path-write guard for `device_records` (mirror
  `rate_limit_store.rs:1` + its flush-only tests); (e) `web/scripts/assert-headers.sh:193-215`
  extension for forged `X-Eq-Edge-*`; (f) a `COOKIE_KEY`-rotation test — if the device key HMAC is
  derived from `COOKIE_KEY` (which is what `AppState.secret_key` is: `main.rs:668`,
  `state.rs:50`), rotating it invalidates every device key and strands every device ban.

---

## Findings

1. **`AuthSessionState` field addition silently logs out / 500s every live session** — severity
   **high** — NEW
   Evidence: `crates/rux-auth/src/session/state.rs:6-26` (no serde attrs);
   `crates/rux-auth/src/session/extractor.rs:44-45` (`.ok().flatten()` → anonymous) and
   `:239-241` (`?` → `AuthErrorCode::SessionError`); `crates/rux-auth/src/error.rs:71,175-179`
   (500); `src/services/session_store.rs:142,167` + tower-sessions-core 0.14 `session.rs:16,26,919`
   (JSON-inside-msgpack). Fix: `#[serde(default)]` on all new fields, append-only order (or a new
   versioned `SESSION_KEY`), plus a legacy-payload decode test.

2. **`with_metadata` must grow a device axis; `device` is unusable for device comparison** —
   severity **high** — NEW
   Evidence: `extractor.rs:111-129`; `auth_v1/controller.rs:183-186` (UA), `:347-351`
   (TOTP → `None`), `passkey_v1/controller.rs:262-265` (WebAuthn `device_type`);
   `user_session/actions.rs:109-112` (LIKE on `device`). Fix: add `device_key` as a new field +
   a new `login_with_metadata` variant; never compare the legacy `device` field.

3. **The 5-minute ban cache cannot express a device axis, and a per-request DB device check is the
   wrong shape** — severity **high** — NEW
   Evidence: `requirements/builder.rs:16` (5 min), `middleware/guard.rs:46-56` (read + session
   write), `session/state.rs:50-63`, `extractor.rs:143-152`; `db/sea_connect.rs:32-44` (pool = 1);
   `session_store.rs:188-198` + `services/auth.rs:520-527` (2 serialized reads/request already).
   Fix: device state in `InMemoryStore` (`store.rs:370-389`) + a `device_checked_at` session field
   with its own TTL; never a per-request `SELECT`; device bans must not reuse `is_banned`.

4. **`BanUnit::Device` breaks the export/list/un-ban contract that the external proxy consumes** —
   severity **high** — NEW
   Evidence: `admin_bans_v1/dto.rs:7-17,31-43,93-100`; `controller.rs:41-70,118-160,163-181`;
   `deploy/README.md:421-436` (canonical CIDR-only feed, adapter outside repo); `layer.rs:228-243`
   (fixed bucket keys are IP/internal/unknown). Fix: keep `BanUnit` IP-only; new
   `device-ban:{key}` namespace + a separate admin list/delete route; export filters to IP units.

5. **`is_external_ip()` + closed `RequestIdentity` make the escalation engine unable to see a
   device; capacity counters cross-contaminate** — severity **high** — NEW
   Evidence: `rux-request-gate/src/ip.rs:38-57`; `layer.rs:252-267,297-321`;
   `services/escalation.rs:72-78,129`; `store.rs:207-215` (device bans consume
   `QURAN_ACTIVE_BAN_MAX`) vs `store.rs:231-238` (`quran-ban:`-only history counter). Fix: a
   separate device evaluation path with its own capacity accounting; never thread a device through
   `RequestIdentity`.

6. **IP allowlist logic is incompatible with a device unit (no exemption path, boot guarantee
   gap)** — severity **high** — NEW
   Evidence: `config/settings.rs:313-326`, `:348-363` (enabled + unset allowlist = boot error),
   `:384-405` (CIDR-only, /64 floor justified by the proxy export). Fix: a separate boot-validated
   device allowlist; document that `QURAN_BAN_ALLOWLIST` cannot exempt devices.

7. **Attacker-rotatable device keys in a bounded, persistent key space = trivial evasion +
   capacity exhaustion** — severity **high** — NEW
   Evidence: DESIGN-v0 §Layer 1 (token in localStorage/IDB); `store.rs:231-238,245-264`
   (capacity gates + prune); `rate_limit_store.rs:180-229` (all keys persist to L2). Fix: device
   bans require corroboration (design §Layer 3 already says so) and get their own cap; never a
   volume-only Long ban on a device key.

8. **A router-level device middleware runs before every rate limiter and doubles the session/user
   DB reads; the correct insertion point is `check_requirements`** — severity **high** — NEW
   Evidence: `main.rs:827-858` ("layers added later run first") vs `router.rs:72-177` (limiters are
   nest-level, inner to all private layers); `auth_guard.rs:8-15,30-33` (one load per guard);
   `guard.rs:9-67`. Fix: a new `AuthRequirements` flag handled inside `check_requirements`
   (already inner to the limiter, already holds the loaded session), or a nest-level layer inserted
   as `routes().layer(device_eval).layer(rate_limit_layer(...))` so the limiter stays outer.

9. **Index list + write amplification; per-request `device_records` updates are a forbidden
   hot-path write** — severity **high** — NEW
   Evidence: `m000001:48-73,252-264` (no indexes), `session_store.rs:15-19,52-62` (expiry scan),
   `user_session/actions.rs:83-88,109-148`, `user_ban/actions.rs:59-64`;
   `rate_limit_store.rs:1` ("Do NOT add a DB write to the request hot path"), `:126-178` (10 s
   flush). Fix: the index set in §4; coalesce device observations in memory and flush on the
   existing tick.

10. **`device_sessions` must have no CASCADE (or a nullable SET NULL) or account deletion destroys
    ban evidence** — severity **high** — NEW
    Evidence: `db/sea_connect.rs:50` (`foreign_keys=ON`); `m000001:55-56` (`user_sessions`
    CASCADE), `:262-263` (`audit_logs` SET NULL); `m000004:11-15` (`auth_session_binding` has no
    FKs); `user_v1/controller.rs:294` (delete path). Fix: mirror `auth_session_binding` (no FK) or
    nullable `ON DELETE SET NULL`; never CASCADE/RESTRICT; add an evidence-survives-deletion test.

11. **`fresh_auth` cannot express or revoke "this session's device changed"** — severity **high** —
    NEW
    Evidence: `fresh_auth.rs:25-45` (no device in `Grant`/`matches`), `:59-64` (process-local),
    `:173-175` (same-purpose grants are replaced), `:246-290` (consume only, no invalidate);
    `auth_v1/controller.rs:1167-1172` (login already issues a grant). Fix: add `device_key` to
    `Grant` + `PendingGrant`, add `fresh_auth::invalidate` + a new `Purpose`/key; accept and
    document the restart fail-closed behavior.

12. **Nothing strips client-supplied `X-Eq-Edge-*`; the internal-token pattern must be reused with
    HMAC + strip + fail-closed** — severity **high** — NEW
    Evidence: `docker-compose.yml:181-192` (no header middleware; Traefik cannot delete request
    headers), `:136-143` (peer gate defaults to all private ranges);
    `middlewares/client_ip.rs:92-106` (present-but-invalid token falls through, header not
    removed); `main.rs:817-835` (extension-layered secret pattern);
    `web/src/lib/server/quran-translation-page.ts:43-77`. Fix: outermost middleware that removes
    inbound `x-eq-edge-*`, verifies HMAC + kid + ±30 s, then sets identity; fail closed in prod on
    a present-but-invalid header; labeled-HKDF key derivation; update `main.rs:799-805` CORS
    `allow_headers` if a browser device header is added.

13. **Migration mechanics: additive-only, idempotent, no table rebuild (FK enforcement is live during
    migrations; SQLite migrations are not transactional), `down()` index-only, never a bare `down`** — severity **high** —
    NEW
    Evidence: `sea_connect.rs:46-56,87-93`; sea-orm-migration-1.1.20 `migrator.rs:261-272,386-397` (SQLite unwrapped; version row after `up()`);
    `migration/bin/migrate.rs:63-69` (`steps: None` = all, and m000001:318-347 drops every table);
    precedents m000002:19-51, m000008:74-82, m000009:121-124, m000011:8-33. Fix: explicit `conn.transaction` around the batch + `CREATE TABLE/INDEX
IF NOT EXISTS` + `PRAGMA table_info`-guarded `ADD COLUMN … NOT NULL DEFAULT <const>`;
    pre-dedupe before any new UNIQUE; build indexes while the old container is drained.

14. **Eight settings struct literals break compilation if `HttpSettings`/`RateLimitSettings` gains a
    field, and four integration test files break if the device check fails closed on a missing
    device** — severity **high** — NEW
    Evidence (compile): `tests/admin_bans_export.rs:79-108`, `quran_prod_gate.rs:74-103`,
    `api_access_security_regressions.rs:76-105`, `bookmarks_v1.rs:83-112`,
    `auth_rotation_header.rs:130-163`, `quran_v1.rs:74-103`,
    `src/middlewares/route_blocker.rs:187-216`, `src/config/settings.rs:1462`; precedent
    `main.rs:863-866`. Evidence (behavioral, sessions via `auth.login`):
    `admin_bans_export.rs:357`, `api_access_security_regressions.rs:211`,
    `fresh_auth_security.rs:55`, `bookmarks_v1.rs:222`. Fix: parse device/edge config independently
    like `EscalationConfig`; treat unknown device as allow.

15. **Invariant guards to extend, and the missing ones the design needs** — severity **medium** —
    NEW
    Evidence: `layer.rs:569-594`; `admin_bans_v1/controller.rs:299,319`;
    `escalation.rs:505,522`; `web/src/lib/auth/__tests__/auth-client.test.ts:49,125,137,153-160,174,192,207`;
    `web/src/lib/server/__tests__/quran-translation-page.test.ts:227-238`;
    `web/scripts/assert-headers.sh:193-215`. Fix: extend the four Rust guards; add (a)
    `AuthSessionState` legacy-payload decode guard, (b) export-only-IP-units guard, (c) migration
    index-presence guard, (d) no-hot-path-write guard for device data, (e)
    evidence-survives-account-deletion guard, (f) forged `X-Eq-Edge-*` script case, (g)
    `COOKIE_KEY`-rotation/device-key-derivation guard (`main.rs:668` + `state.rs:50`).
