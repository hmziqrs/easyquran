# A10 — Data-model & migration-risk audit of DESIGN-v0 (device identity)

Auditor: data-model / migration-risk subagent. Scope: the design's schema (Layer 2), its
migration (m000012), its ban placement, its lifecycle, and its concurrency/idempotency story.
Every claim below is grounded in a file read during this audit; quotes are verbatim.

Already-known items (SQLite pool of 1; identity tables FK `users NOT NULL CASCADE`; `audit_logs`
dormant; `auth_session_binding` has no FKs; `devices.token` globally unique with ownership
transfer; missing indices on `user_sessions`/`user_bans`/`audit_logs`; single-writer contention)
are not re-reported as findings; where they bear on the design they are cited as constraints.

---

## 0. Ground truth established by reading the repo

| Fact                                                                                                                             | Evidence                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Migrations run at boot **before** the router/listener exist                                                                      | `sea_connect.rs:87-93` (`Migrator::up` inside `try_connect`); `main.rs:168` `get_sea_connection()` runs before `AppState` (l.654) and before `start_scheduler` (l.761)                                       |
| **SQLite migrations are NOT transactional in sea-orm-migration 1.1.20**                                                          | `migrator.rs:261-272`: `DbBackend::Postgres => { let transaction = db.begin()… }` / `DbBackend::MySql \| DbBackend::Sqlite => { let manager = SchemaManager::new(db); f(&manager).await }`                   |
| The migration record is inserted **after** `up()` returns                                                                        | `migrator.rs:386-397`: `migration.up(manager).await?;` … then `seaql_migrations::Entity::insert(...)`                                                                                                        |
| No repo migration wraps itself in a transaction                                                                                  | `grep -rn "begin()\|transaction" migration/src/` → zero hits                                                                                                                                                 |
| `PRAGMA foreign_keys=ON` is live during migrations                                                                               | `sea_connect.rs:46-56` (`configure_sqlite`) runs before `Migrator::up`                                                                                                                                       |
| SQLite is 3.46.0, `SQLITE_ENABLE_JSON1` on, `SQLITE_ENABLE_UPDATE_DELETE_LIMIT` **off**                                          | `libsqlite3-sys-0.30.1/sqlite3/sqlite3.h:149`; `build.rs:123-136` flag list (JSON1, FTS5, RTREE, STAT4, DBSTAT_VTAB, COLUMN_METADATA, MEMORY_MANAGEMENT, API_ARMOR — no UPDATE_DELETE_LIMIT)                 |
| `RETURNING` is available but unused anywhere in the repo                                                                         | sqlite 3.46 ≥ 3.35; `grep -rn RETURNING src/ crates/ migration/` → zero hits                                                                                                                                 |
| `user_bans` has a model create path but **zero callers**; the admin ban API is 100% `rate_limit_state`                           | `user_ban/actions.rs:11` `Entity::create`; `grep NewUserBan` → only `auth.rs:531` `get_active_ban`; `admin_bans_v1/mod.rs` routes list/delete/export only                                                    |
| `admin_delete` hard-deletes the user (CASCADE everywhere)                                                                        | `user/actions.rs:486-497` `admin_delete` → `Self::delete_by_id(user_id).exec(&txn)`; route `user_v1/mod.rs:29` `/delete/{user_id}`                                                                           |
| `user_sessions.device` holds a **raw User-Agent** (login), `"{provider} OAuth"`, or `credential.device_type`                     | `auth_v1/controller.rs:183-187` `headers.get("user-agent")`; `oauth/login.rs:346`; `passkey_v1/controller.rs:262`; TOTP path hardcodes `None` (`controller.rs:348`)                                          |
| `/device/v1` mounts unconditionally; `/auth/v1`+OAuth+passkey+email+forgot nests are gated on `WEB_AUTH_ENABLED` (default false) | `router.rs:49,72-105,165-177`; `settings.rs:530` `env_bool("WEB_AUTH_ENABLED", false)`                                                                                                                       |
| Runtime `ensure_table` drift guard exists exactly once                                                                           | `rate_limit_store.rs:60-70` (`ensure_table`) + `:479-497` `ensure_table_ddl_matches_migration_ddl`                                                                                                           |
| Periodic-job precedents                                                                                                          | `scheduler.rs:11` (60s tick, one job) + `main.rs:761`; inline session sweep `main.rs:764-775` (3600s); `rate_limit_store::spawn_flush_task` 10s tick + `SLOW_FLUSH_EVERY_N_TICKS=6` companion, `main.rs:648` |
| `PERSISTENCE_LOCK` serializes durable rate-limit writes only                                                                     | `rate_limit_store.rs:47`, held in `delete_ban` (`admin_bans_v1/controller.rs:135`)                                                                                                                           |

---

## 1. Corrected schema

### 1.1 Resolutions of the five explicit questions

**(a) nullable `user_id` — keep, `ON DELETE SET NULL`, but it is NOT the evidence.**
`users` deletion is a hard `DELETE` (`admin_delete`). Every other identity table cascades, so
cascading here would delete the device record the moment the abusive account is deleted — i.e.
at exactly the moment the operator most needs the device. `SET NULL` is also the established
repo action for "the referenced user may vanish but this row must survive"
(`audit_logs.user_id` → `ON DELETE SET NULL`, m000001:262-263; `media.uploader_id` → `SET NULL`,
m000001:45-46). So: `user_id INTEGER` nullable, `ON DELETE SET NULL`, meaning "last-seen owner"
only. Attribution that must survive belongs in a table with **no FK at all**, following the
`auth_session_binding` precedent (m000004 has no FKs by design).

**(b) what happens to device evidence on user delete — cascade is NOT acceptable.**
Chain today: `DELETE users` → `user_sessions` rows CASCADE away (m000001:55-56) →
`auth_session_binding` rows for them are deleted by the reconcile SQL
(`auth.rs:401-406`, `DELETE FROM auth_session_binding WHERE user_session_id IN (SELECT id FROM
user_sessions WHERE revoked_at IS NOT NULL)`) → and a design FK `device_sessions.user_session_id
→ user_sessions(id) ON DELETE CASCADE` would take the device links with it. Result: after
deleting the offender's account, the cluster query "which other accounts used this device"
returns nothing. Fix: `device_sessions` (and `device_evidence`) carry a denormalized
`user_id INTEGER NOT NULL` with **no FK**, so the (device → account) pair survives user
deletion; `user_session_id`/`tower_session_id` are plain INTEGER/TEXT with no FK. Alternative if
a real FK is wanted: `ON DELETE SET NULL` on `user_session_id` (nullable) — still loses the
session-level link, so the denormalized `user_id` is the load-bearing part either way.

**(c) `device_key` UNIQUE — yes, and the failure mode is bounded.**
It must be a **plain** (non-partial) UNIQUE index because it is the upsert conflict target
(`ON CONFLICT(device_key)` requires a matching non-partial unique constraint). Collision math:
256-bit HMAC, birthday bound ~2^128 — not a practical concern; the real risks are two other
things, and both need schema support:

1. **Pepper rotation is unrecoverable.** `device_key = HMAC(pepper, signals)` and the raw
   signals are never stored (design Layer 0). Rotating the pepper therefore makes every
   existing key unverifiable _and_ unrecomputable — all device bans silently stop matching.
   Fix: an immutable pepper (documented, backed up like `COOKIE_KEY`/field-encryption key) plus
   a `key_kid TEXT NOT NULL DEFAULT 'v1'` column; on lookup, an unknown kid must **fail open**
   (no ban, log loudly), never fail closed.
2. If a collision did happen, the upsert merges two physical devices into one row. That must
   never become a sole ban basis — which is exactly the design's own shared-device rule
   (Layer 3: "may only ban with corroboration"). Schema consequence: keep `cohort_key` and the
   coherence columns so a human review can see the corroboration.

**(d) coherence booleans — separate tri-state INTEGER columns, not JSON.**
`SQLITE_ENABLE_JSON1` is compiled in, but `json_extract()` is not indexable (no expression index
is worth it here) and the design's rule engine queries these values per cohort. Tri-state is
required: `NULL` = unknown/not collected, `1` = coherent, `0` = mismatch. Repo style is
`INTEGER NOT NULL DEFAULT 0` for real booleans (`users.is_verified`, `two_fa_enabled`,
`post_comments.hidden`, `email_suppression.permanent`) — those are two-state; ours are three-state,
so nullable with **no default**. The long tail (raw-ish derived detail that is never filtered on)
stays in one `evidence TEXT` JSON column, matching `posts.tag_ids TEXT NOT NULL DEFAULT '[]'`
and `passkey_credentials.transports TEXT`.

**(e) `trust_state` — TEXT enum, fail-closed default.**
Exactly the `rate_limit_state.block_scope TEXT NOT NULL DEFAULT 'Temp'` pattern
(m000002:6-12) plus `BlockScope::from_db_str` + "unknown → warn → default"
(`rate_limit_store.rs:92-101`). Also matches `users.role TEXT NOT NULL DEFAULT 'user'` backed by
`UserRole` (`DeriveActiveEnum`, `rs_type = "String"`, `db_type = "Text"`,
`enums/user.rs:4-23`) with a lenient `From<&str>` fallback (`user.rs:61-65`). Integers would be
cheaper but there is no integer-enum precedent in the schema (`UserRole::to_i32` is a _derived
level_, not the stored form), and a TEXT value is readable in the admin console and in
`sqlite3` without a decoder. Use `TEXT`, and mirror the fail-closed read.

### 1.2 Full corrected DDL (m000012, repo style)

```sql
-- device_records: one row per derived device key. Identity is device_key (HMAC-SHA256 of the
-- collected signal tuple under an IMMUTABLE server pepper, lowercase hex). No raw signals are
-- stored (design Layer 0): the key is one-way, so these rows cannot be rebuilt after a rollback.
CREATE TABLE IF NOT EXISTS device_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_key TEXT NOT NULL,
    key_kid TEXT NOT NULL DEFAULT 'v1',
    token_hash TEXT,
    user_id INTEGER,
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
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL
);
-- Plain UNIQUE: this is the upsert conflict target. Non-partial on purpose.
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_key ON device_records (device_key);
-- SQLite treats NULLs as distinct, so derived-only rows coexist.
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_token_hash ON device_records (token_hash);
-- Admin console "devices of a user", newest first (backward index scan).
CREATE INDEX IF NOT EXISTS idx_device_records_user_last_seen
    ON device_records (user_id, last_seen_at);
-- Retention cleanup predicate.
CREATE INDEX IF NOT EXISTS idx_device_records_last_seen ON device_records (last_seen_at);
-- Cohort cluster query: exact-match on an HMAC of the coarse class tuple.
CREATE INDEX IF NOT EXISTS idx_device_records_cohort ON device_records (cohort_key);

-- device_sessions: append-only link (device, authenticated session). NO FKs to users/user_sessions
-- on purpose: user_sessions cascades on user delete (m000001) and the boot reconcile deletes
-- bindings for revoked audit rows (services/auth.rs), so any FK here erases ban-evasion evidence.
CREATE TABLE IF NOT EXISTS device_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_record_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    user_session_id INTEGER NOT NULL,
    tower_session_id TEXT,
    ip_unit TEXT,
    linked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_record_id) REFERENCES device_records (id)
        ON UPDATE CASCADE ON DELETE CASCADE
);
-- Upsert conflict target for the per-login link write.
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_sessions_record_session
    ON device_sessions (device_record_id, user_session_id);
-- Admin console "devices of a user" reads THIS table (authoritative history), not device_records.user_id.
CREATE INDEX IF NOT EXISTS idx_device_sessions_user_linked
    ON device_sessions (user_id, linked_at);
-- Ban-evasion cluster query: (device -> accounts) becomes an index-only scan.
CREATE INDEX IF NOT EXISTS idx_device_sessions_record_user
    ON device_sessions (device_record_id, user_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_session ON device_sessions (user_session_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_linked ON device_sessions (linked_at);

-- device_bans: the DEVICE ban axis. Dedicated table — never rate_limit_state (see §6).
CREATE TABLE IF NOT EXISTS device_bans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_key TEXT NOT NULL,
    key_kid TEXT NOT NULL DEFAULT 'v1',
    device_record_id INTEGER,
    scope TEXT NOT NULL DEFAULT 'Long',
    reason TEXT,
    banned_by INTEGER,
    expires_at TEXT,
    rule_version INTEGER NOT NULL DEFAULT 0,
    risk_score INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked_at TEXT,
    revoked_by INTEGER,
    FOREIGN KEY (banned_by) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    FOREIGN KEY (revoked_by) REFERENCES users (id)
        ON UPDATE CASCADE ON DELETE SET NULL,
    FOREIGN KEY (device_record_id) REFERENCES device_records (id)
        ON UPDATE CASCADE ON DELETE SET NULL
);
-- One ACTIVE ban per key, unbounded history behind it. Makes concurrent ban proposals idempotent.
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_bans_active_key
    ON device_bans (device_key) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_device_bans_record ON device_bans (device_record_id);
CREATE INDEX IF NOT EXISTS idx_device_bans_key_created ON device_bans (device_key, created_at);
CREATE INDEX IF NOT EXISTS idx_device_bans_expires ON device_bans (expires_at);

-- device_evidence: append-only audit for device decisions. NO FKs to users/user_sessions.
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
CREATE INDEX IF NOT EXISTS idx_device_evidence_key_created
    ON device_evidence (device_key, created_at);
CREATE INDEX IF NOT EXISTS idx_device_evidence_user_created
    ON device_evidence (user_id, created_at);
CREATE INDEX IF NOT EXISTS idx_device_evidence_subject_created
    ON device_evidence (subject, created_at);
```

### 1.3 Why each choice (compressed)

| Choice                                                 | Justification                                                                                                                                                                                         |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `device_key TEXT` + `key_kid`                          | One-way key needs an immutability guard; kid makes a rotation fail loudly instead of silently un-banning everyone                                                                                     |
| `token_hash TEXT UNIQUE`                               | The client-held minted token is a _lookup hint_ only (avoids re-collecting signals); it must not be the ban key (storage clearing would evade). Plain UNIQUE is safe: SQLite treats NULLs as distinct |
| `user_id` nullable + `SET NULL`                        | "Last-seen owner" convenience; matches `audit_logs`/`media.uploader_id` precedent                                                                                                                     |
| No FK on `device_sessions.user_id` / `user_session_id` | The evidence chain must outlive both `users` and `user_sessions`; precedent `auth_session_binding` (no FKs, m000004)                                                                                  |
| `device_record_id` CASCADE on `device_sessions`        | Child of the record; if the record is gone the link is meaningless. Retention must be _blocked_ by active bans instead (see §7)                                                                       |
| Partial UNIQUE on active ban                           | Makes `INSERT … ON CONFLICT (device_key) WHERE revoked_at IS NULL DO UPDATE` atomic; without it two concurrent proposals create two rows and an un-ban revokes only one                               |
| `scope TEXT DEFAULT 'Long'`                            | Reuses `BlockScope::from_db_str` vocabulary + fail-closed read; no new enum type needed                                                                                                               |
| `expires_at TEXT` nullable                             | Mirrors `user_bans.expires_at`; NULL = indefinite, admin-set only (design forbids auto-permanent)                                                                                                     |
| tri-state `INTEGER` coherence columns                  | Queryable/indexable per cohort; `NULL` = not collected. JSON cannot be filtered cheaply                                                                                                               |
| `cohort_key TEXT`                                      | The only indexable way to express "same coarse device class" (see §5, unindexable query)                                                                                                              |

---

## 2. Does the design need the runtime `ensure_table` + drift-test pattern?

**No — for `device_records`, `device_sessions`, `device_bans`, `device_evidence`.** That pattern
exists for one historical reason: `rate_limit_state` was boot-owned before a migration owned it,
and `sessions` is still runtime-created (`session_store.rs:15-19,39-50`). `ensure_table` remains
as a fallback for the admin path, which is why `ensure_table_ddl_matches_migration_ddl`
(`rate_limit_store.rs:479-497`) compares `PRAGMA table_info` output from both DDL constants.
New tables get a migration plus `IF NOT EXISTS`; `Migrator::up` runs inside
`get_sea_connection()` before `AppState` exists, so no request path can ever touch these tables
before the migration. Adding a second DDL constant would _create_ drift risk, not remove it.

**What must be added instead** (migration contract test, the m000004 style: assert exact columns
and index presence against the migration's own DDL — `m000004:87-115`, `m000010:83-117`):

```rust
#[tokio::test]
async fn migration_creates_device_tables_with_expected_columns_and_indices() {
    // fresh in-memory db + users fixture (FK target), then Migration.up
    // for each table: PRAGMA table_info(name) → assert exact column set, and
    // for each column assert (type, notnull, dflt_value, pk) so an accidental
    // NOT NULL / default change fails the test.
    // PRAGMA index_list(name) → assert:
    //   device_records: idx_device_records_key (unique), idx_device_records_token_hash (unique),
    //                   idx_device_records_user_last_seen, idx_device_records_last_seen,
    //                   idx_device_records_cohort
    //   device_sessions: idx_device_sessions_record_session (unique), + 4 more
    //   device_bans: idx_device_bans_active_key (unique, partial), + 3 more
    //   device_evidence: 3 indices
    // plus the two behavioural pins: (1) a second device_key insert must fail;
    // (2) two ACTIVE rows for one device_key must fail while a revoked row may coexist.
}
```

**If** the design later adds any runtime `ensure_table` for a device table (e.g. a
`device_ban_store::ensure_table` mirroring `rate_limit_store`), the drift test is then
**mandatory** and must be the exact `table_info`-equality form already in the repo — a second
`CREATE TABLE` string is the only way drift can appear:

```rust
#[tokio::test]
async fn ensure_table_ddl_matches_migration_ddl() {
    // runtime db: ensure_table(&db).await;
    // migration db: Migrator::up(&db, None).await;
    // assert_eq!(table_info(runtime_db), table_info(migration_db),
    //   "ensure_table DDL and m000012 migration DDL have drifted apart");
}
```

---

## 3. Migration safety on a live DB (single connection, requests in flight)

### 3.1 What SQLite actually does

- **`ALTER TABLE … ADD COLUMN`** — metadata-only, **O(1)**, no table rewrite; existing rows read
  the constant default. Hard restrictions (SQLite docs, unchanged in 3.46):
  - no `PRIMARY KEY`, no `UNIQUE`;
  - **default may not be `CURRENT_TIME`/`CURRENT_DATE`/`CURRENT_TIMESTAMP` or a parenthesized
    expression** → `ADD COLUMN created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP` **fails**.
    Workaround: add nullable, backfill, or use `NOT NULL DEFAULT ''`;
  - `NOT NULL` requires a non-NULL default;
  - **with `foreign_keys=ON` (which this repo has during migrations), a column with `REFERENCES`
    must default to NULL.**
    Repo precedent: `m000002:33-36` adds `block_scope TEXT NOT NULL DEFAULT 'Temp'` to a populated
    legacy table without touching rows.
- **`CREATE UNIQUE INDEX` on a populated table** — O(n) index build, takes a write lock for the
  schema change. WAL lets readers continue; other writers block. With `busy_timeout=5000`
  (`sea_connect.rs:49`) a competing writer waits 5s then returns `SQLITE_BUSY` (a 500 to a live
  request if that writer is a _second process_ — e.g. the old binary during a rolling deploy, or
  `cargo run` next to a running server). In-process there is no contention: migrations complete
  before the listener binds.
- **Backfill** — O(n) per pass and the only unbounded cost. This design should have **none**.
- **The migration is not atomic.** sea-orm-migration 1.1.20 wraps _only_ Postgres
  (`migrator.rs:261-272`), and the `seaql_migrations` row is inserted after `up()` returns
  (`migrator.rs:386-397`). A crash mid-migration therefore leaves DDL applied but **unrecorded**,
  so `up()` **re-runs on the next boot**. That is precisely why every migration in this repo is
  written idempotently (`IF NOT EXISTS` everywhere in m000002/m000004/m000005/m000007/m000010,
  and m000004 even has an explicit "a prior partial run may have created the table" branch).

### 3.2 What m000012 must do

1. **Pure DDL, all `IF NOT EXISTS`, no backfill.** Every table is new; index creation is on an
   empty table (O(1)). Total cost ≈ a few ms; downtime = the process restart that was already
   happening.
2. **Do not backfill from `user_sessions`.** `user_sessions.device` is a raw User-Agent
   (`auth_v1/controller.rs:183-187`), `"{provider} OAuth"` (`oauth/login.rs:346`), a passkey
   `device_type` (`passkey_v1/controller.rs:262`), a hardcoded `None` on the TOTP path
   (`controller.rs:348`), or a seed display string
   (`seed_v1/controller.rs:391-396`, e.g. `"MacOS · Chrome 126"`). It is not a device key, and
   hashing it would (a) fabricate keys that never match a real derived key and (b) store a hash
   of the raw UA the design's Layer 0 forbids collecting. Rows start empty and fill on login.
3. **Optionally wrap in an explicit transaction.** Because sea-orm will not, the author can:
   `conn.transaction::<_, (), DbErr>(|txn| Box::pin(async move { … }))` — the repo already uses
   this form in `rate_limit_store::flush` and `delete_ban_rows`. This gives all-or-nothing DDL on
   SQLite (DDL is transactional there). It is not the repo's migration habit, so if omitted the
   `IF NOT EXISTS` discipline is the safety net.
4. **Idempotency proof, in the m000011 style** — its test calls `Migration.up` twice
   (`m000011:56-57`). m000012 must do the same, and must also prove `down` then `up` is clean.

### 3.3 The m000011 precedent, quoted

```rust
conn.execute_unprepared(
    "DELETE FROM devices WHERE token IN (
        SELECT token FROM devices GROUP BY token HAVING COUNT(DISTINCT user_id) > 1
    )",
).await?;
conn.execute_unprepared(
    "DELETE FROM devices WHERE id NOT IN (SELECT MAX(id) FROM devices GROUP BY token)",
).await?;
conn.execute_unprepared(
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_token_owner ON devices (token)",
).await?;
```

Order is the lesson: **dedupe by the exact uniqueness predicate first, then create the unique
index**. If m000012 ever creates a UNIQUE index on a table that already has rows (it should not),
it must carry the same two-pass dedupe — m000008 is the richer precedent (normalize → dedupe
keeping `MAX(updated_at)`, tie-break `max(rowid)` → unique index, `m000008:30-54,60-69`).

---

## 4. Downgrade / rollback

`down` must drop children before parents (with `foreign_keys=ON`, `DROP TABLE` on a parent runs
an implicit `DELETE FROM`, which fires FK actions on live children):

```rust
async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    for stmt in [
        "DROP INDEX IF EXISTS idx_device_evidence_subject_created",
        "DROP INDEX IF EXISTS idx_device_evidence_user_created",
        "DROP INDEX IF EXISTS idx_device_evidence_key_created",
        "DROP TABLE IF EXISTS device_evidence",
        "DROP INDEX IF EXISTS idx_device_bans_expires",
        "DROP INDEX IF EXISTS idx_device_bans_key_created",
        "DROP INDEX IF EXISTS idx_device_bans_record",
        "DROP INDEX IF EXISTS idx_device_bans_active_key",
        "DROP TABLE IF EXISTS device_bans",
        "DROP INDEX IF EXISTS idx_device_sessions_linked",
        "DROP INDEX IF EXISTS idx_device_sessions_session",
        "DROP INDEX IF EXISTS idx_device_sessions_record_user",
        "DROP INDEX IF EXISTS idx_device_sessions_user_linked",
        "DROP INDEX IF EXISTS idx_device_sessions_record_session",
        "DROP TABLE IF EXISTS device_sessions",
        "DROP INDEX IF EXISTS idx_device_records_cohort",
        "DROP INDEX IF EXISTS idx_device_records_last_seen",
        "DROP INDEX IF EXISTS idx_device_records_user_last_seen",
        "DROP INDEX IF EXISTS idx_device_records_token_hash",
        "DROP INDEX IF EXISTS idx_device_records_key",
        "DROP TABLE IF EXISTS device_records",
    ] {
        conn.execute_unprepared(stmt).await?;
    }
    Ok(())
}
```

(Dropping the indices explicitly is belt-and-braces — SQLite drops a table's indices with it —
but it matches m000006/m000011's habit of naming what it removes.)

**Data lost: everything, permanently.** `device_key` is a one-way HMAC of signals that are never
stored (design Layer 0), so the records cannot be reconstructed from `user_sessions`, logs, or
anything else in the DB. A rollback therefore:

- deletes every device identity, every (device → account) link, and every device ban;
- cannot be undone by re-running `up` — the table comes back empty;
- turns an active device ban into _no ban_ — an offender who was being throttled is immediately
  un-throttled, and re-banning requires re-collection over days.

Honest-downgrade precedent to cite, `m000009:121-124`:

```rust
async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
    // Original casing is unrecoverable; nothing to revert.
    Ok(())
}
```

The design must state this loss explicitly (the way m000009 does) rather than pretend the
rollback is reversible, and the m000012 comment must carry the same sentence. Two further
requirements:

- **The new binary must fail open on missing tables.** After a rollback the process can boot
  against a DB without these tables. Every read path must behave like
  `rate_limit_store::load` (`:111-115`: warn + empty) and every write path must be non-fatal;
  a device-ban check that 500s on `no such table` would take the whole API down.
- An old binary rolled back across m000012 is safe (additive-only tables it never reads) — the
  same reasoning as the m000004 registration comment in `lib.rs:24-25`.

---

## 5. Query paths — exact SQL + the index each needs

### 5.1 Device lookup at login (get-or-create, one statement)

```sql
INSERT INTO device_records
    (device_key, key_kid, token_hash, user_id, last_seen_at, seen_count)
VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP, 1)
ON CONFLICT(device_key) DO UPDATE SET
    last_seen_at = excluded.last_seen_at,
    seen_count   = device_records.seen_count + 1,
    token_hash   = COALESCE(excluded.token_hash, device_records.token_hash),
    user_id      = COALESCE(excluded.user_id, device_records.user_id);
```

Index: `idx_device_records_key` (UNIQUE, the conflict target). Then, to get the id + risk state:

```sql
SELECT id, trust_state, risk_score, key_kid, seen_count
FROM device_records WHERE device_key = ?;
```

Same index. (`RETURNING id` also works on 3.46 and saves the round trip, but it is unused in the
repo — either is acceptable; do not SELECT-then-INSERT.)

### 5.2 Per-login link write (idempotent under concurrent logins)

```sql
INSERT INTO device_sessions
    (device_record_id, user_id, user_session_id, tower_session_id, ip_unit, last_seen_at)
VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
ON CONFLICT(device_record_id, user_session_id) DO UPDATE SET
    last_seen_at = excluded.last_seen_at;
```

Index: `idx_device_sessions_record_session` (UNIQUE).

### 5.3 Device → user lookup for the ban check (must NOT be SQL per request)

Enforcement read, hydrated once at boot into an in-process map and mutated on admin ban/unban:

```sql
SELECT device_key, scope, expires_at, rule_version
FROM device_bans
WHERE revoked_at IS NULL
  AND (expires_at IS NULL OR expires_at > ?);
```

Index: `idx_device_bans_active_key` (partial UNIQUE `WHERE revoked_at IS NULL`). This runs **once
at boot**, not per request — the repo's own rule is explicit: "Do NOT add a DB write to the
request hot path" (`rate_limit_store.rs:1`), and with a pool of 1 a per-request read is a
per-request serialization point. Per request the middleware does one hash-map lookup on the
device key carried in a header.

If an operator needs the live attribution for a device (admin only, not hot):

```sql
SELECT user_id, first_seen_at, last_seen_at, seen_count, trust_state, risk_score
FROM device_records WHERE device_key = ?;
```

Index: `idx_device_records_key`.

### 5.4 User → devices for the admin console

```sql
SELECT d.device_key, d.key_kid, d.first_seen_at, d.last_seen_at, d.seen_count,
       d.trust_state, d.risk_score,
       (SELECT COUNT(DISTINCT s.user_id) FROM device_sessions s
         WHERE s.device_record_id = d.id) AS account_count
FROM device_records d
WHERE d.id IN (
    SELECT s.device_record_id FROM device_sessions s
    WHERE s.user_id = ?
    GROUP BY s.device_record_id
)
ORDER BY d.last_seen_at DESC
LIMIT ? OFFSET ?;
```

Indices: `idx_device_sessions_user_linked (user_id, linked_at)` for the inner probe,
`idx_device_records_key`/PK for the outer fetch. Reads `device_sessions` (history), **not**
`device_records.user_id` (last-seen owner only — a shared device would flip owners and vanish
from the first user's list).

### 5.5 Ban-evasion cluster query

```sql
SELECT s.device_record_id,
       d.device_key,
       COUNT(DISTINCT s.user_id) AS accounts,
       MIN(s.linked_at) AS first_link,
       MAX(s.linked_at) AS last_link
FROM device_sessions s
JOIN device_records d ON d.id = s.device_record_id
WHERE s.user_id IN (/* banned user ids */)
GROUP BY s.device_record_id, d.device_key
HAVING accounts >= 2
ORDER BY accounts DESC, last_link DESC
LIMIT 200;
```

Indices: `idx_device_sessions_user_linked` for the IN-list probe;
`idx_device_sessions_record_user (device_record_id, user_id)` makes the grouping an index-only
scan; PK join for the key. **Not** a join on `device_records.user_id` — that column holds one
owner and cannot express a shared device.

### 5.6 Retention cleanup

```sql
-- Batched. `DELETE ... LIMIT` is NOT available: SQLITE_ENABLE_UPDATE_DELETE_LIMIT is absent
-- from libsqlite3-sys 0.30.1's build flags (build.rs:123-136), so use the id-IN-subquery form.
DELETE FROM device_sessions
 WHERE id IN (
    SELECT s.id FROM device_sessions s
     WHERE s.linked_at <= ?
       AND NOT EXISTS (
           SELECT 1 FROM device_bans b
            WHERE b.device_record_id = s.device_record_id
              AND b.revoked_at IS NULL
              AND (b.expires_at IS NULL OR b.expires_at > ?))
     LIMIT 500
 );

DELETE FROM device_records
 WHERE id IN (
    SELECT d.id FROM device_records d
     WHERE d.last_seen_at <= ?
       AND NOT EXISTS (
           SELECT 1 FROM device_bans b
            WHERE b.device_record_id = d.id
              AND b.revoked_at IS NULL
              AND (b.expires_at IS NULL OR b.expires_at > ?))
       AND NOT EXISTS (
           SELECT 1 FROM device_sessions s WHERE s.device_record_id = d.id)
     LIMIT 500
 );
```

Indices: `idx_device_sessions_linked`, `idx_device_records_last_seen`,
`idx_device_bans_record` (both EXISTS probes). Note the ban guard is on **both** tables: the
30-day key ladder must never delete a record whose ban is live (that would make the ban
unenforceable and un-auditable), and it must not delete a record that still has link rows.

### 5.7 The unindexable query, flagged

Anything shaped `WHERE device_key LIKE '%…%'` (substring), `substr(device_key,1,8) = ?`, or a
hash-prefix bucket used to find "similar" devices **cannot use an index in SQLite** — the LIKE
optimization requires a case-sensitive prefix pattern with a BINARY-collation index
(`PRAGMA case_sensitive_like=ON`, or a `COLLATE NOCASE` index), and substring patterns are never
optimizable. Workable alternatives, in order:

1. **`cohort_key` exact match** (recommended): `HMAC(pepper, hardware_class ‖ platform_class ‖
locale_class ‖ screen_class)` — a small, typed tuple hashed with the same immutable pepper so
   it is not enumerable. Index `idx_device_records_cohort`. Gives "same class of device" without
   ever touching a hash prefix.
2. **An explicit indexed prefix column** (`device_key_prefix TEXT`, first 8 hex chars, own
   index) if a bounded bucket is genuinely needed — an indexed column, never a `LIKE`.
3. **`GLOB 'abc*'`** — case-sensitive, so it _can_ be optimized to a range scan on a plain
   index, unlike `LIKE` under the default `case_sensitive_like=OFF`.

---

## 6. Where a DEVICE ban must live

**Recommendation: the dedicated `device_bans` table above, hydrated into an in-process map at
boot, mutated only by the admin API.** Not `rate_limit_state`; not `user_bans`; not
`BanUnit::Device`.

Evidence for why each alternative fails:

- **`user_bans` is impossible.** `user_bans.user_id INTEGER NOT NULL` + FK
  `ON DELETE CASCADE` (m000001:58-73) — a device ban has no user, and even if it borrowed one,
  `admin_delete` would cascade the device ban away with the account. `user_bans` also has no
  create path today (only `auth.rs:531 get_active_ban` reads it), so "add a device axis to
  `BanUnit`" would not even reach it.
- **`rate_limit_state` is the wrong namespace and breaks the admin API.** Keys are
  `quran-ban:{unit}` where `{unit}` must `parse` as a `BanUnit` (m000003-era convention,
  `admin_bans_v1/dto.rs:31-43`). `filter_active_ban_rows` skips any key whose suffix does not
  parse (`controller.rs:44-49`), so a device ban stored there is **invisible to
  `GET /admin/bans` and to `/admin/bans/export`** and therefore **cannot be listed or unbanned**
  through the API. Additional damage:
  - `ban_unit_history_count()` counts every `quran-ban:` key (`store.rs:231-238`) → device bans
    consume `QURAN_ESCALATION_MAX_IDENTITIES` and push real IP units out of the escalation
    capacity gate;
  - `active_ban_count()` counts every active block (`store.rs:207-215`) → device bans consume
    `QURAN_ACTIVE_BAN_MAX`, so an admin device-ban wave starts declining new _IP_ bans and
    incrementing the saturation counter;
  - `delete_ban` builds `fixed_key = ratelimit:{unit}:quran-v1` (`controller.rs:129`) — a
    meaningless key for a device, so un-ban leaves a stale row behind;
  - `escalation.rs` only ever constructs ban keys from `RequestIdentity::External` IPs
    (`escalation.rs:129-134,203-209`), so nothing in the escalation engine can ever match a
    device key: the row would be pure dead weight that also breaks the counters.
- **`BanUnit::Device(device_key)` breaks the invariant the whole admin surface rests on.** The
  type's doc comment is explicit: "One canonical ban unit … `parse()` accepts only a /32 or /64
  width, so an unparseable or wrong-width store key … can never pose as an exportable ban unit"
  (`dto.rs:7-15`). Adding a device variant:
  - makes `BanUnit::parse` / `Deserialize` accept a non-network value, so the export stream
    (`ExportResponse` → "the deployment-owned proxy adapter consumes canonical ban units",
    `dto.rs:90-92`) starts emitting values **no proxy can enforce** — Traefik/CF cannot block a
    device key, so a device ban exported there is a silent no-op at the edge and a false sense of
    enforcement at the origin;
  - `DeleteBanPayload.ban_unit` is validated `length(min=1, max=64)` (`dto.rs:83-88`) — a 64-char
    hex key fits, so validation would _pass_ and the confusion would ship.

**Placement, concretely:**

| Requirement                           | How `device_bans` satisfies it                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| (a) enforced per-request cheaply      | Boot `SELECT` (§5.3) → `HashMap<device_key, (scope, expires_at)>` behind a `std::sync::RwLock`; per request a read-lock + hash lookup (sub-µs), no SQL, no pool. Admin mutations take the write lock and write the DB **first**, then update the map (mirroring `delete_ban`'s "durable delete FIRST, then clear L1", `controller.rs:139-153`) |
| (b) listed/unbanned by the admin API  | A new `/admin/devices` nest (design Layer 5) queries `device_bans` directly; list = `WHERE revoked_at IS NULL AND (expires_at IS NULL OR expires_at > ?)` on `idx_device_bans_active_key`; unban = `UPDATE … SET revoked_at=?, revoked_by=?` (history kept, unlike the IP path's hard `DELETE`)                                                |
| (c) survives restarts                 | Plain table + boot hydration; no dependency on the 10s flush loop                                                                                                                                                                                                                                                                              |
| (d) does not pollute the IP namespace | Separate table, separate counters, separate key space; `rate_limit_state` and every `quran-ban:` counter are untouched                                                                                                                                                                                                                         |
| export contract preserved             | `device_bans` is deliberately **not** in `/admin/bans/export`; if an operator wants edge enforcement, the design must say the device key is origin-only, or the export needs an explicit, versioned `kind` field (a separate, deliberate change)                                                                                               |

---

## 7. Data lifecycle — the retention ladder

**Ladder, restated as concrete table semantics:**

- `device_records` + `device_sessions`: delete when idle **and** unlinked **and** not under a live
  ban. The design says "keyed device key 30d"; implement as `last_seen_at <= now-30d`.
- `device_bans` (revoked or expired) and `device_evidence`: keep **12 months**
  (`created_at <= now-365d`), so the ban record and its corroboration outlive the key.
- Aggregates: computed counters (`risk_score`, per-cohort counts) may be kept indefinitely —
  nothing in this schema stores a raw signal, so an aggregate row cannot be reversed.

**SQL** — §5.6 for the two record/session deletes; the ban/evidence prune:

```sql
DELETE FROM device_bans
 WHERE id IN (
    SELECT id FROM device_bans
     WHERE revoked_at IS NOT NULL AND revoked_at <= ?
     LIMIT 500
 );

DELETE FROM device_evidence
 WHERE id IN (SELECT id FROM device_evidence WHERE created_at <= ? LIMIT 500);
```

Indices: `idx_device_bans_expires` (or `created_at`), `idx_device_evidence_key_created`.

**Where it runs.** Three registration precedents exist:

1. `services/scheduler.rs` — `start_scheduler(state)` spawns one 60s loop (`scheduler.rs:11-25`),
   called at `main.rs:761`;
2. the session sweep — an inline `tokio::spawn` with a 3600s interval (`main.rs:764-775`);
3. the rate-limit flush — a 10s task with an optional every-6th-tick companion
   (`rate_limit_store.rs:126-178`, `main.rs:648`).

**Recommended: register the device-retention sweep in `scheduler.rs` as its own `tokio::spawn`
with its own interval (e.g. 6h), alongside the existing publisher** — that file already owns
"periodic maintenance with `AppState`", and the inline session sweep is the precedent for a
separate cadence. Do **not** ride the flush task's every-6th-tick companion: that callback is
documented as running outside `PERSISTENCE_LOCK` on the shared connection
(`rate_limit_store.rs:120-125`), and a batched DELETE there would contend with the 10s flush on a
pool of one.

**Is batched DELETE safe on the single-writer pool?** Yes, with rules:

- ≤ 500 rows per statement (`id IN (SELECT … LIMIT 500)`), loop until `rows_affected == 0`;
- `tokio::time::sleep` a few ms between batches so request handlers and the 10s flush interleave;
- never hold a lock (neither `PERSISTENCE_LOCK` nor the device map's write lock) across a batch —
  the flush code's discipline ("the in-memory bucket mutex is never held across I/O",
  `rate_limit_store.rs:44-46`) applies here too;
- the whole sweep is best-effort and non-fatal, exactly like the session sweep's
  `warn!(… "session store expired-row sweep failed (non-fatal)")` (`main.rs:770-772`).

---

## 8. `WEB_AUTH_ENABLED=false`

Verified gating: `router.rs:49` reads `web_auth().enabled`; `router.rs:72-105` mounts `/auth/v1`,
all five OAuth nests, `/passkey/v1`, `/email_verification/v1`, `/forgot_password/v1` **only when
true**; the default is `false` (`settings.rs:530`); `main.rs:184-207` gates the boot session
reconcile on the same flag; `/device/v1` (FCM push tokens) mounts **unconditionally**
(`router.rs:165-177`).

**Does the device feature degrade gracefully? Mechanically yes, semantically it becomes a
different feature — and the design must say which one it is.**

- With auth off there are no logins → `device_records.user_id` stays NULL, `device_sessions`
  stays empty → the ban-evasion cluster query returns nothing and `device_bans` can only be
  created by hand. So device bans are near-useless with auth off: correct, but the design should
  state that the device axis is auth-scoped.
- **Two hard requirements regardless of the flag:**
  1. **Never fail closed on a missing device key.** Anonymous reader traffic (`/t/**` is bypassed
     by cookies, `/quran/v1` is public) must not 403 because a header is absent. A device ban is
     an _additional_ block condition, never a gate on its own.
  2. **Do not write device rows for unauthenticated traffic.** The design's lawful basis is
     "Art 6(1)(f) + ePrivacy strictly-necessary **for auth**" (Layer 6). Persisting a derived
     device identity for logged-out readers is a different purpose with no stated basis. Gate the
     collection endpoint _and_ the boot hydration on `web_auth().enabled`, exactly like the
     session reconcile.
- **If** the collector endpoint is mounted unconditionally (e.g. under `/device/v1`), it needs
  (i) its own `rate_limit_layer` — the collector is attached to `AuthClient.#request`, i.e. it
  fires on _every_ `/api/**` call, so an unlimited endpoint is a write-amplification vector on a
  pool of one — and (ii) an explicit "pre-auth collection" mode with its own lawful basis in the
  design. Otherwise: gate it.
- `/t/**` cache safety is unaffected at the data-model level (no device header on those routes),
  so nothing here changes the 7-day disk cache.

---

## 9. Idempotency and concurrency — the exact constraints/upserts

| Race                                           | Mechanism that makes it safe                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Two simultaneous logins, same brand-new device | One statement, no read-then-write: `INSERT … ON CONFLICT(device_key) DO UPDATE` (§5.1). Pool=1 serializes statements, but _not_ an `await`-separated SELECT→INSERT pair, so the single statement is what actually closes the race. `seen_count` increments atomically in the same statement                                                                                                                                                                                                                                                          |
| Same device record created twice               | `idx_device_records_key` (plain UNIQUE) + the upsert conflict target. A second `up()`-like re-run cannot duplicate because the key is the constraint, not a queried value                                                                                                                                                                                                                                                                                                                                                                            |
| Two concurrent link writes for one session     | `idx_device_sessions_record_session` (UNIQUE) + `ON CONFLICT(device_record_id, user_session_id) DO UPDATE`                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Two concurrent ban proposals for one device    | `idx_device_bans_active_key` (partial UNIQUE `WHERE revoked_at IS NULL`) + `INSERT INTO device_bans (device_key, scope, reason, banned_by, expires_at) VALUES (?,?,?,?,?) ON CONFLICT (device_key) WHERE revoked_at IS NULL DO UPDATE SET scope = excluded.scope, expires_at = MAX(device_bans.expires_at, excluded.expires_at)` — one row, longest expiry wins. Without the partial unique index (i.e. if `device_bans` copied `user_bans`, which has none) the two proposals create two rows and the un-ban revokes only one → "unban didn't work" |
| Ban proposal vs. capacity gate                 | The engine's own pattern: check `active_ban_count()` before creating, and on saturation `incr_saturation()` + decline (`escalation.rs:254-261`) — fail-closed, never evict an active ban. Mirror it for devices with a device-scoped count, not the shared one                                                                                                                                                                                                                                                                                       |
| Cleanup job vs. ban proposal                   | Ban row written **first**, keyed on `device_key` (not only `device_record_id`), inside one transaction; cleanup's `NOT EXISTS (SELECT 1 FROM device_bans …)` guard then sees it. `device_bans.device_record_id` is `ON DELETE SET NULL` so a retention-driven record delete cannot delete the ban — the ban survives with its key. Residual race with a pool of one is closed by putting both the guard and the delete in one transaction (`conn.transaction`, as `delete_ban_rows` does)                                                            |
| Un-ban vs. periodic writers                    | Device bans have **no** periodic flusher (unlike `rate_limit_state`), so the "snapshot resurrects a lifted ban" hazard does not exist here. Still: write the DB first, then update the in-process map — a DB failure must leave the ban enforced and the op retryable, exactly as `delete_ban` documents (`controller.rs:139-148`)                                                                                                                                                                                                                   |
| User delete vs. evidence                       | `device_records.user_id` → `SET NULL` (row survives); `device_sessions.user_id` has **no FK** (pair survives); `device_evidence.user_id` has **no FK** (attribution survives). This is the whole point of §1.1(b)                                                                                                                                                                                                                                                                                                                                    |

---

## 10. Findings

### 1. NEW — `device_sessions`/`device_records` linked to `user_sessions` destroys the ban-evasion evidence on user delete

_Severity: critical._
`user_sessions` is `user_id INTEGER NOT NULL … ON DELETE CASCADE` (m000001:48-57) and the only
user-delete path is a hard delete (`user/actions.rs:486-497`, routed at `user_v1/mod.rs:29`).
Any design FK `device_sessions.user_session_id → user_sessions(id)` therefore erases the
device↔account linkage exactly when the offender's account is deleted — the moment the linkage is
needed. The boot reconcile independently deletes bindings for revoked audit rows
(`services/auth.rs:401-406`).
**Fix:** `device_sessions` has NO FK to `users`/`user_sessions`; it carries a denormalized
`user_id INTEGER NOT NULL` (precedent: `auth_session_binding` has no FKs at all, m000004), and
`device_evidence` likewise. `device_records.user_id` keeps `ON DELETE SET NULL` as "last-seen
owner" only.

### 2. NEW — Nullable `user_id` must be `SET NULL` **and** attribution must live in a no-FK table

_Severity: high._
`SET NULL` alone is correct for the live column (precedent `audit_logs`/`media.uploader_id`) but
loses attribution on delete: `admin_delete` → `SET NULL` → the admin "user → devices" query no
longer finds the device. Anonymous/pre-auth records must stay NULL-owner, which is fine, but they
must not be written for unauthenticated _reader_ traffic (lawful basis is auth-scoped, Layer 6).
**Fix:** `device_records.user_id INTEGER NULL` + `SET NULL` for "current owner"; all history in
`device_sessions.user_id` (no FK) and `device_evidence.user_id` (no FK).

### 3. NEW — `device_key` UNIQUE yes, but pepper rotation is unrecoverable and must be schema-guarded

_Severity: critical._
`device_key = HMAC(pepper, signals)` with raw signals never stored (Layer 0) means a pepper
rotation makes every existing key both unverifiable and unrecomputable — all device bans silently
stop matching, with no error. HMAC collision (2^128 birthday bound) is not the real risk; the
real risk is key generation.
**Fix:** `key_kid TEXT NOT NULL DEFAULT 'v1'`, the pepper is immutable and backed up like
`COOKIE_KEY`/the field-encryption key, and an unknown kid fails **open** (no ban, loud log).
Keep `idx_device_records_key` a plain (non-partial) UNIQUE because it is the upsert conflict
target. Collision failure mode is a merged record, which must never be a sole ban basis — the
design's own shared-device corroboration rule.

### 4. NEW — "server-minted opaque device token" contradicts "HMAC of components": a wiped token evades a ban

_Severity: high._
Design Layer 1 says the client stores a _server-minted opaque id_; Layer 2 says `device_key` is
_HMAC-SHA256 of components_. If the ban keys on a server-minted token, clearing localStorage/IDB
yields a fresh key → instant ban evasion. If it keys on the derived HMAC, storage clearing is
harmless but the "opaque id" framing is wrong.
**Fix:** two columns — `device_key` (derived, stable, UNIQUE, the ban key) and
`token_hash` (HMAC of the minted token, UNIQUE, a lookup hint to avoid re-collecting signals).

### 5. NEW — Coherence booleans as JSON TEXT are not queryable; use tri-state INTEGER columns

_Severity: medium._
`SQLITE_ENABLE_JSON1` is compiled in, but `json_extract()` is not indexable, and the rule engine
filters on these per cohort. Repo style is scalar boolean columns.
**Fix:** `coherence_*` / `spoof_*` as nullable `INTEGER` (NULL = unknown, 1 = coherent, 0 =
mismatch), one `evidence TEXT` JSON tail for the un-queried detail.

### 6. NEW — `trust_state` should be a TEXT enum with a fail-closed default, not an integer

_Severity: low._
**Fix:** `trust_state TEXT NOT NULL DEFAULT 'observe'`, read with the
`BlockScope::from_db_str` + warn-and-default discipline (`rate_limit_store.rs:92-101`);
TEXT enums are the schema's only precedent (`users.role`, `block_scope`, `post.status`).

### 7. NEW — No new `ensure_table` is needed; the drift test is the wrong test here

_Severity: medium._
`ensure_table_ddl_matches_migration_ddl` exists only because `rate_limit_store` keeps a second
DDL string (`rate_limit_store.rs:479-497`). New tables are migration-owned and unreachable before
`Migrator::up` (which runs inside `get_sea_connection()` before `AppState`).
**Fix:** no `ensure_table`, therefore no drift test; add a migration **contract** test in the
m000004 style (exact `table_info` + `index_list` assertions + the two behavioural pins). If a
runtime `ensure_table` is added anyway, the exact `table_info`-equality drift test becomes
mandatory.

### 8. NEW — SQLite migrations are not transactional in sea-orm-migration 1.1.20 → m000012 must be re-runnable

_Severity: critical._
`migrator.rs:261-272` wraps only Postgres; `migrator.rs:386-397` records the version only after
`up()` returns. A crash mid-migration re-runs `up()` on the next boot.
**Fix:** every statement `IF NOT EXISTS` (m000002/m000004/m000005/m000010 precedent), a
`up()`-twice idempotency test (m000011:56-57 style), and optionally an explicit
`conn.transaction` wrapper (the repo's own form in `rate_limit_store::flush`).

### 9. NEW — `ALTER TABLE ADD COLUMN` cannot take `DEFAULT CURRENT_TIMESTAMP`; a `REFERENCES` column must default NULL

_Severity: high._
SQLite rejects non-constant defaults in `ADD COLUMN`; with `foreign_keys=ON` (live during
migrations, `sea_connect.rs:46-56`) an added FK column must default NULL. `CREATE UNIQUE INDEX` on
a populated table is O(n) with a write lock.
**Fix:** m000012 is pure DDL on empty tables (O(1)); any future ADD COLUMN uses
`NOT NULL DEFAULT '<constant>'` (m000002:33-36 precedent) or nullable-then-backfill; any future
UNIQUE index on a populated table carries m000011's dedupe-first pair of DELETEs.

### 10. NEW — Do not backfill `device_records` from `user_sessions.device` — it is a raw User-Agent

_Severity: medium._
`user_sessions.device` holds a raw UA (`auth_v1/controller.rs:183-187`), `"{provider} OAuth"`
(`oauth/login.rs:346`), a passkey `device_type` (`passkey_v1/controller.rs:262`), `None`
(`controller.rs:348`), or a seed string (`seed_v1/controller.rs:391-396`). Hashing it fabricates
keys that can never match a real derived key and violates Layer 0 by persisting a UA-derived
value.
**Fix:** no backfill; rows fill on login. (Bonus: `user_sessions.device` is pre-existing raw-UA
storage that the DPIA should note.)

### 11. NEW — Downgrade is permanently lossy and the new binary must fail open

_Severity: high._
`down` drops four tables; `device_key` is one-way, so nothing can be rebuilt; an active device ban
becomes no ban. The m000009 precedent is honest about this ("Original casing is unrecoverable;
nothing to revert", `m000009:121-124`).
**Fix:** document the loss in the m000012 comment; drop children before parents; every device read
path warns-and-empties like `rate_limit_store::load` (`:111-115`) and every write is non-fatal, so
a post-rollback boot does not 500.

### 12. NEW — A device ban in `rate_limit_state` is invisible to the admin API and pollutes the IP counters

_Severity: critical._
`filter_active_ban_rows` skips keys whose suffix does not `parse` as a `BanUnit`
(`admin_bans_v1/controller.rs:44-49`) → not listable, not unbannable, not exportable.
`ban_unit_history_count()` (`store.rs:231-238`) and `active_ban_count()` (`store.rs:207-215`)
count every `quran-ban:` key / every active block → device bans consume
`QURAN_ESCALATION_MAX_IDENTITIES` and `QURAN_ACTIVE_BAN_MAX`. `delete_ban`'s
`fixed_key = ratelimit:{unit}:quran-v1` (`controller.rs:129`) is meaningless for a device.
**Fix:** dedicated `device_bans` table (§1.2), hydrated to an in-process map at boot, listed and
revoked by a new `/admin/devices` API; `rate_limit_state` untouched.

### 13. NEW — `BanUnit::Device(device_key)` breaks the canonical-unit invariant and the export contract

_Severity: high._
`dto.rs:7-15` documents the invariant; `ExportRow` is consumed by a proxy adapter that cannot
enforce a device key (`dto.rs:90-92`), and `DeleteBanPayload`'s `length(1..64)` (`dto.rs:83-88`)
would happily accept a 64-char hex key.
**Fix:** do not extend `BanUnit`. Keep the device axis origin-only and out of
`/admin/bans/export`; if edge enforcement is ever wanted, add an explicit versioned `kind` field
to the export DTO as a separate change.

### 14. NEW — Reusing `audit_logs` for device evidence is unsafe as-is

_Severity: medium._
`audit_logs` has no index on `(resource_type, resource_id)` (m000001:252-264), no retention
column, `user_id … ON DELETE SET NULL`, and it is a general audit table — mixing 30-day
DSAR-scoped device evidence with it makes both retention and erasure ambiguous.
**Fix:** dedicated `device_evidence` (no FKs, `subject`, indexed `(device_key, created_at)`).
If `audit_logs` is reused anyway: add the `(resource_type, resource_id)` index, add an explicit
retention column, and keep attribution in a no-FK column — otherwise the SET NULL loses exactly
what §1.1(b) requires.

### 15. NEW — Retention must use `id IN (SELECT … LIMIT n)` (no `DELETE … LIMIT`), and the sweep must be a separate task

_Severity: medium._
`SQLITE_ENABLE_UPDATE_DELETE_LIMIT` is absent from libsqlite3-sys 0.30.1's build flags
(`build.rs:123-136`) → `DELETE … LIMIT` is a syntax error on this build. Riding the flush task's
every-6th-tick companion would contend with the 10s flush on the shared single connection, and
that callback is documented as lock-free on purpose (`rate_limit_store.rs:120-125`).
**Fix:** batched `DELETE FROM t WHERE id IN (SELECT id … LIMIT 500)` (§5.6/§7), ≤500 rows per
statement with a short sleep between batches, no lock held across a batch, best-effort/non-fatal;
register the job in `scheduler.rs` as its own `tokio::spawn` with its own interval (precedent:
the inline session sweep, `main.rs:764-775`). Guard both deletes with `NOT EXISTS (SELECT 1 FROM
device_bans …)` so retention can never delete a record under a live ban.
