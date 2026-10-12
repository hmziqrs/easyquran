# B8 — Retention & deletion EXECUTION audit

**Question (Q8):** can this system actually delete, prove it deleted, propagate to backups, and destroy the pepper?

**Inputs read:** `DESIGN-v1.md` B3/B4/B7; `A5-privacy-legal.md` findings 2/7/8/13/14; `A9-crypto-keys.md` findings 2/13; `A10-data-model.md` findings 1/3/15; `A8-ops-realism.md` findings 5/14.
**Repo read (verified, not inferred):** `src/services/scheduler.rs`, `src/main.rs:648-775`, `src/services/session_store.rs`, `src/services/rate_limit_store.rs`, `src/db/sea_connect.rs`, `src/utils/field_crypto.rs`, `src/state.rs:74-179`, `src/config/settings.rs:884-916`, `docker-compose.yml`, `deploy/Dockerfile.api`, `deploy/README.md` (full grep), `migration/src/lib.rs`, `m000001_init.rs:48-73,252-264`, `src/db/sea_models/user_ban/actions.rs`, `justfile`, `.github/workflows/images.yml`.
**Build facts verified on this machine:** `libsqlite3-sys-0.30.1/build.rs:122-138` has **no** `SQLITE_ENABLE_UPDATE_DELETE_LIMIT` and **no** `SQLITE_SECURE_DELETE`; SQLite docs confirm `auto_vacuum` default NONE, `journal_size_limit` default -1 (no limit), WAL reuse-not-truncate.

**Verdict up front.** Today: nothing can be deleted, nothing can be proven, there is no backup, and the pepper cannot be destroyed without losing all device enforcement. The retention ladder in B3/B7 is a sentence with no mechanism behind it. What follows is the mechanism, then the honest accounting of what stays impossible.

---

## 0. Repo ground truth (what the execution has to live with)

| Fact                                                                                                                                                                                                          | Evidence                                                              |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| One SQLite connection, `max_connections(1)`, `acquire_timeout(8s)`, `busy_timeout=5000`                                                                                                                       | `sea_connect.rs:32-44,46-56`                                          |
| WAL on, `foreign_keys=ON`, no `auto_vacuum`, no `secure_delete`, no `journal_size_limit`                                                                                                                      | `sea_connect.rs:47-55`; build flags                                   |
| `DELETE ... LIMIT` is a syntax error on this build                                                                                                                                                            | `libsqlite3-sys-0.30.1/build.rs:122-138` (flag absent)                |
| Migrations stop at m000011; no device table, and the only `expires_at` in the schema is `user_bans.expires_at` (unwired)                                                                                      | `migration/src/lib.rs:12-13`; `m000001_init.rs:63`                    |
| Periodic-job precedents: 60s publisher (`scheduler.rs:11-25`), inline 3600s session sweep (`main.rs:764-775`), 10s rate-limit flush + every-6th-tick companion (`rate_limit_store.rs:126-178`, `main.rs:648`) | as cited                                                              |
| The flush holds `PERSISTENCE_LOCK` across its transaction; the companion runs outside it; both use the same single connection                                                                                 | `rate_limit_store.rs:47,146-161,120-125`                              |
| No graceful-shutdown handler; `axum::serve` runs to process exit                                                                                                                                              | `main.rs:929-947` (no signal/select)                                  |
| DB file = `./data/easyquran.db` inside container, on named volume `api_data` (`/app/data`); single VPS                                                                                                        | `docker-compose.yml:128,159-160`; `Dockerfile.api:21-25`              |
| **No backup, restore, snapshot, or DR procedure anywhere** (deploy/README.md grep: only "backup-code" auth)                                                                                                   | `deploy/README.md` §index; `.github/workflows/images.yml` (no DB job) |
| `audit_logs` exists, zero writers, `user_id SET NULL`, no `expires_at`                                                                                                                                        | `m000001_init.rs:252-264`                                             |
| `user_bans` exists, expiry semantics correct (`revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now)`), zero writers                                                                                | `m000001_init.rs:58-73`; `user_ban/actions.rs:56-71`                  |
| `user_sessions.device` = **raw User-Agent**, `ip_address` = raw IP, no retention                                                                                                                              | `m000001_init.rs:48-57`; A8 F6                                        |
| Self-service account delete does **not** exist; only `POST /user/v1/admin/delete/{user_id}`                                                                                                                   | `user_v1/mod.rs:29`                                                   |
| Key lifecycle precedent: dev-default + prod panic + placeholder blocklist + `_PREV` decrypt slot + mid-process change rejected                                                                                | `state.rs:74-179`, `field_crypto.rs:17-66`                            |
| Metrics exist but are no-op unless `ENABLE_QUICKWIT_OTEL`; `init_pool_metrics` is the pattern                                                                                                                 | `telemetry.rs:168-190,304-329`; `main.rs:132`                         |

---

## 1. The cleanup implementation

### 1.1 Migration m0000NN — only the retention-relevant shape

Full DDL is A10 §1.2. The corrections this audit forces (all three are load-bearing):

1. **All retention columns are `INTEGER` unix seconds**, not TEXT. Reason: `CHECK (expires_at >= last_seen_at)` and `expires_at <= ?` must compare like-typed values. SQLite compares across storage classes by class order (NULL < INTEGER < TEXT < BLOB), so an INTEGER `expires_at` compared to a TEXT `created_at` is **always false** — a silent no-op CHECK and a silent no-op predicate. The repo already mixes formats (`user_bans.expires_at` is chrono RFC3339; `audit_logs.created_at` defaults to SQLite `CURRENT_TIMESTAMP` — a latent pre-existing bug). New tables do not repeat it: `first_seen_at/last_seen_at/created_at/expires_at/revoked_at` are epoch seconds; human-readable rendering stays in the API.
2. **`device_sessions` carries `device_record_id`, which is also the ban key**, so every sweeper probe is an index point lookup with no join (the anti-join-explosion move). `device_key` + `key_kid` are copied onto the row as evidence only.
3. **`device_bans` is keyed by `device_record_id`, and a live ban pins the record**: while the ban is live the record's `expires_at` = ban expiry + 30 d and the sweeper skips it, so the record survives exactly as long as the ban (finding 6). `device_key` is copied onto the ban row as evidence only — a digest ban would be a cohort ban (A2 F1). `ON DELETE SET NULL` keeps ended-ban history once the record is gone.

```sql
CREATE TABLE IF NOT EXISTS device_records (
    id            TEXT PRIMARY KEY,              -- ULID, stable, never exposed
    device_key    TEXT NOT NULL,                 -- fp digest, NON-unique (cohort axis)
    key_kid       TEXT NOT NULL DEFAULT 'v1',
    token_hash    TEXT,                          -- HMAC(token_subkey, tok) UNIQUE
    user_id       INTEGER,
    first_seen_at INTEGER NOT NULL,
    last_seen_at  INTEGER NOT NULL,
    expires_at    INTEGER NOT NULL,              -- last_seen + 30d, or ban/hold-extended
    seen_count    INTEGER NOT NULL DEFAULT 1,
    trust_state   TEXT NOT NULL DEFAULT 'observe',
    revoked_at    INTEGER,
    cohort_key    TEXT,
    evidence      TEXT,
    created_at    INTEGER NOT NULL,
    CHECK (expires_at >= last_seen_at),
    CHECK (expires_at > first_seen_at),
    FOREIGN KEY (user_id) REFERENCES users (id) ON UPDATE CASCADE ON DELETE SET NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_records_token ON device_records (token_hash);
CREATE INDEX IF NOT EXISTS idx_device_records_key   ON device_records (device_key);
CREATE INDEX IF NOT EXISTS idx_device_records_expires ON device_records (expires_at);
CREATE INDEX IF NOT EXISTS idx_device_records_user  ON device_records (user_id, last_seen_at);

CREATE TABLE IF NOT EXISTS device_sessions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    device_record_id TEXT NOT NULL,
 device_key TEXT NOT NULL, -- evidence copy (the ban guard uses device_record_id)
    key_kid          TEXT NOT NULL DEFAULT 'v1',
    user_id          INTEGER NOT NULL,           -- NO FK (evidence must outlive users)
    user_session_id  INTEGER NOT NULL,           -- NO FK
    linked_at        INTEGER NOT NULL,
    last_seen_at     INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,           -- last_seen + 30d
    CHECK (expires_at >= last_seen_at)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_sessions_pair ON device_sessions (device_record_id, user_session_id);
CREATE INDEX IF NOT EXISTS idx_device_sessions_expires ON device_sessions (expires_at);
CREATE INDEX IF NOT EXISTS idx_device_sessions_key     ON device_sessions (device_key);

CREATE TABLE IF NOT EXISTS device_bans (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
 device_key TEXT NOT NULL, -- evidence copy only (cohort key; never the ban key)
    key_kid          TEXT NOT NULL DEFAULT 'v1',
 device_record_id TEXT, -- the ban key; pinned while the ban is live
    scope            TEXT NOT NULL DEFAULT 'Long',
    reason           TEXT,
    banned_by        INTEGER,
    expires_at       INTEGER,                    -- NULL = live until revoked (admin-only)
    revoked_at       INTEGER,
    revoked_by       INTEGER,
    created_at       INTEGER NOT NULL,
    CHECK (expires_at IS NULL OR expires_at > created_at),
    FOREIGN KEY (device_record_id) REFERENCES device_records (id) ON DELETE SET NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_bans_active_record ON device_bans (device_record_id) WHERE revoked_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_device_bans_retention ON device_bans (revoked_at, expires_at);

CREATE TABLE IF NOT EXISTS device_evidence (     -- append-only, self-contained, NO CASCADE FKs
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    case_id          TEXT NOT NULL,
    device_key       TEXT NOT NULL,              -- digest frozen at creation
    key_kid          TEXT NOT NULL DEFAULT 'v1',
    device_record_id TEXT,                       -- plain pointer, no FK action
    user_id          INTEGER,                    -- NO FK
    subject          TEXT NOT NULL,
    detail           TEXT,                       -- decision metadata only
    created_at       INTEGER NOT NULL,
    expires_at       INTEGER NOT NULL,           -- case retention class, copied at write
    CHECK (expires_at > created_at)
);
CREATE INDEX IF NOT EXISTS idx_device_evidence_key  ON device_evidence (device_key, created_at);
CREATE INDEX IF NOT EXISTS idx_device_evidence_case ON device_evidence (case_id);
CREATE INDEX IF NOT EXISTS idx_device_evidence_expires ON device_evidence (expires_at);

CREATE TABLE IF NOT EXISTS legal_holds (
    id            TEXT PRIMARY KEY,              -- ULID
    authority     TEXT NOT NULL,                 -- LE/regulator/court ref
    reason        TEXT NOT NULL,
    owner         TEXT NOT NULL,                 -- named operator
    scope_json    TEXT NOT NULL,                 -- what was demanded, verbatim
    received_at   INTEGER NOT NULL,
    preserve_until INTEGER NOT NULL,             -- 90d default, renewable
    status        TEXT NOT NULL DEFAULT 'active',-- active|released
    released_at   INTEGER,
    released_by   TEXT,
    created_at    INTEGER NOT NULL,
    CHECK (preserve_until > created_at)
);
CREATE INDEX IF NOT EXISTS idx_legal_holds_active ON legal_holds (status, preserve_until);

CREATE TABLE IF NOT EXISTS legal_hold_subjects (
    hold_id      TEXT NOT NULL,
    subject_kind TEXT NOT NULL,                  -- device_record|user|case|evidence
    subject_id   TEXT NOT NULL,
    PRIMARY KEY (hold_id, subject_kind, subject_id)
);
CREATE INDEX IF NOT EXISTS idx_hold_subjects_lookup ON legal_hold_subjects (subject_kind, subject_id);

CREATE TABLE IF NOT EXISTS erasure_receipts (    -- replay ledger; see §5/§7
    id           TEXT PRIMARY KEY,               -- ULID
    request_ref  TEXT NOT NULL,                  -- ticket / self-service request id
    subject_kind TEXT NOT NULL,                  -- device_record|user
    subject_ref  TEXT NOT NULL,                  -- deleted row id (ULID) — replayable
    action       TEXT NOT NULL,                  -- delete|strip
    deleted_json TEXT,                           -- fields removed
    retained_json TEXT,                          -- fields kept + basis + until
    created_at   INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL,               -- created + 24mo
    CHECK (expires_at > created_at)
);
CREATE INDEX IF NOT EXISTS idx_erasure_receipts_subject ON erasure_receipts (subject_kind, subject_ref);
CREATE INDEX IF NOT EXISTS idx_erasure_receipts_expires ON erasure_receipts (expires_at);
```

`up()` is pure DDL, all `IF NOT EXISTS` (sea-orm-migration 1.1.20 does not wrap SQLite migrations — A10 F8), so a crash mid-run re-runs cleanly. No `ALTER TABLE` (ADD COLUMN has no `IF NOT EXISTS`; that is why receipts are a new table, not a column on `audit_logs`).

### 1.2 The sweeper — `src/services/device_retention.rs` (new module)

```rust
//! Device retention sweeper. Own task, own cadence. Never rides the 10s
//! rate-limit flush's every-6th-tick companion and never takes
//! PERSISTENCE_LOCK (it touches only device_*/legal_hold_*/erasure_receipts,
//! none of which the flush reads). Each DELETE is one autocommit statement:
//! on a pool of 1 the shared connection is held for microseconds, then the
//! 50ms pause lets request handlers and the flush interleave.

use std::time::Duration;

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement, Value};
use tracing::{info, warn};

use crate::config::env::env_u64;
use crate::state::AppState;

const DEFAULT_INTERVAL_SECS: u64 = 6 * 3600;
const DEFAULT_BATCH: i64 = 500;
const DEFAULT_MAX_BATCHES: u32 = 20;   // ≤10k rows/table/tick
const BATCH_PAUSE_MS: u64 = 50;
const DAY: i64 = 86_400;
const RECORD_TTL: i64 = 30 * DAY;
const BAN_TTL: i64 = 365 * DAY;
const EVIDENCE_TTL: i64 = 365 * DAY;
const RECEIPT_TTL: i64 = 730 * DAY;

fn epoch_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SweepReport {
    pub sessions: u64,
    pub records: u64,
    pub bans: u64,
    pub evidence: u64,
    pub receipts: u64,
    pub backlog: bool,
}

pub fn start(state: AppState) {
    tokio::spawn(async move {
        let interval = Duration::from_secs(env_u64(
            "DEVICE_RETENTION_INTERVAL_SECS",
            DEFAULT_INTERVAL_SECS,
        ));
        let batch = env_u64("DEVICE_RETENTION_BATCH", DEFAULT_BATCH as u64) as i64;
        let max_batches = env_u64("DEVICE_RETENTION_MAX_BATCHES", DEFAULT_MAX_BATCHES as u64) as u32;
        // Off-tick start: no stampede on redeploy, no alignment with the 10s flush.
        let jitter = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0)
            % 300;
        let mut ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(jitter),
            interval,
        );
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match sweep_once(&state.sea_db, epoch_now(), batch, max_batches).await {
                Ok(r) if r.sessions + r.records + r.bans + r.evidence + r.receipts > 0 => info!(
                    sessions = r.sessions, records = r.records, bans = r.bans,
                    evidence = r.evidence, receipts = r.receipts, backlog = r.backlog,
                    "device retention sweep complete"
                ),
                Ok(_) => {}
                Err(e) => warn!(error = %e, "device retention sweep failed (non-fatal)"),
            }
        }
    });
}
```

The batch runner (one statement per batch, never a transaction spanning awaits):

```rust
async fn run_batches(
    db: &DatabaseConnection,
    sql: &str,
    args: Vec<Value>,
    batch: i64,
    max_batches: u32,
) -> Result<(u64, bool), DbErr> {
    let mut total = 0u64;
    for _ in 0..max_batches {
        let res = db
            .execute(Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, args.clone()))
            .await?;
        let n = res.rows_affected();
        total += n;
        if n < batch as u64 {
            return Ok((total, false));
        }
        tokio::time::sleep(Duration::from_millis(BATCH_PAUSE_MS)).await;
    }
    Ok((total, true)) // hit the cap: backlog remains, next tick continues
}
```

`args` is `[now, now, batch]` for the record/session deletes, `[cutoff, cutoff, batch]` for the ban prune.

### 1.3 The SQL (no `DELETE ... LIMIT` anywhere)

Sessions first — they are children of records and their absence is the record's own guard. The ban guard is a direct `device_record_id` probe (no join), and the hold guard is a point lookup.

```sql
-- 1. device_sessions: 30d inactivity, no live ban on the record, no hold.
DELETE FROM device_sessions
 WHERE id IN (
   SELECT s.id FROM device_sessions s
    WHERE s.expires_at <= ?1
      AND NOT EXISTS (
            SELECT 1 FROM device_bans b
 WHERE b.device_record_id = s.device_record_id
               AND b.revoked_at IS NULL
               AND (b.expires_at IS NULL OR b.expires_at > ?2))
      AND NOT EXISTS (
            SELECT 1 FROM legal_hold_subjects hs
             WHERE hs.subject_kind = 'device_record'
               AND hs.subject_id = s.device_record_id)
    ORDER BY s.expires_at
    LIMIT ?3);

-- 2. device_records: candidate expiry, no live ban on the record, no hold, no remaining links.
DELETE FROM device_records
 WHERE id IN (
   SELECT d.id FROM device_records d
    WHERE d.expires_at <= ?1
      AND NOT EXISTS (
            SELECT 1 FROM device_bans b
 WHERE b.device_record_id = d.id
 AND b.revoked_at IS NULL
 AND (b.expires_at IS NULL OR b.expires_at > ?2))
      AND NOT EXISTS (
            SELECT 1 FROM legal_hold_subjects hs
             WHERE hs.subject_kind = 'device_record'
               AND hs.subject_id = d.id)
      AND NOT EXISTS (
            SELECT 1 FROM device_sessions s WHERE s.device_record_id = d.id)
    ORDER BY d.expires_at
    LIMIT ?3);

-- 3. device_bans: only dead bans (revoked, or expired and unrevoked). A live
--    ban (revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now)) is
--    never a candidate. 12mo measured from revocation, else from expiry.
DELETE FROM device_bans
 WHERE id IN (
   SELECT id FROM device_bans
    WHERE ((revoked_at IS NOT NULL AND revoked_at <= ?1)
        OR (revoked_at IS NULL AND expires_at IS NOT NULL AND expires_at <= ?1))
      AND NOT EXISTS (
            SELECT 1 FROM legal_hold_subjects hs
             WHERE hs.subject_kind = 'device_record'
 AND hs.subject_id = device_bans.device_record_id)
    ORDER BY COALESCE(revoked_at, expires_at)
    LIMIT ?3);

-- 4. device_evidence: own expiry (case retention class copied at write).
--    The subject record may already be gone — that is the point.
DELETE FROM device_evidence
 WHERE id IN (
   SELECT e.id FROM device_evidence e
    WHERE e.expires_at <= ?1
      AND NOT EXISTS (
            SELECT 1 FROM legal_hold_subjects hs
             WHERE (hs.subject_kind = 'case'          AND hs.subject_id = e.case_id)
                OR (hs.subject_kind = 'evidence'      AND hs.subject_id = e.id)
                OR (hs.subject_kind = 'device_record' AND hs.subject_id = e.device_record_id))
    ORDER BY e.expires_at
    LIMIT ?3);

-- 5. erasure_receipts: 24mo, then the ledger itself is erased (backups are long gone).
DELETE FROM erasure_receipts
 WHERE id IN (SELECT id FROM erasure_receipts WHERE expires_at <= ?1 LIMIT ?3);
```

`sweep_once` runs 1→2→3→4→5 in that order; each table's `run_batches` result is collected; a per-table `DbErr` is warned and the next table proceeds (mirrors the session sweep's non-fatal contract, `main.rs:770-772`). `no such table` is treated as skip (A10 §4 fail-open after a rollback) — the tick logs it and moves on. No panic path, no `?` out of the loop.

### 1.4 Scheduling + registration

`services/mod.rs`: add `pub mod device_retention;`.
`main.rs`, immediately after `start_scheduler` (`main.rs:761`): `ruxlog::services::device_retention::start(state.clone());`
Do **not** add it to the flush's `every_sixth_tick` callback — that callback is documented as sharing the connection with the 10s flush (`rate_limit_store.rs:120-125`), and a batched DELETE there is exactly the contention A10 §7 warned about. Own spawn, own 6h interval, 0-300s jitter.

### 1.5 Why this does not stall the request path or the 10s flush

| Hazard                                            | Mitigation                                                                                                                                                                                     |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Pool of 1 → every statement serializes            | ≤500-row DELETE per statement; `id IN (subquery)` uses `idx_device_*_expires` for the scan and the PK for the delete                                                                           |
| A long write transaction blocking readers/writers | No explicit transaction: each DELETE is one autocommit statement, atomic by itself. The ban-not-exists guard and the delete are the same statement, so there is no TOCTOU window               |
| `PERSISTENCE_LOCK` contention with the flush      | Not taken. The flush's lock is for `rate_limit_state` rows only; the sweeper never touches that table                                                                                          |
| `SQLITE_BUSY` after `busy_timeout=5000`           | Non-fatal: warn + skip the rest of that table's batches, next tick retries                                                                                                                     |
| Restart stampede                                  | `interval_at(now + jitter)`; `MissedTickBehavior::Skip`; the loop `await`s the sweep so ticks never overlap                                                                                    |
| Long backlog (first run after enabling)           | ≤20 batches/table/tick; when a batch is full the tick reports `backlog=true`; converges over ticks instead of one long stall                                                                   |
| WAL auto-checkpoint firing inside a big delete    | Batches are small; the 50ms pause lets the checkpoint's write land between batches                                                                                                             |
| In-process ban map consistency                    | The sweeper only deletes bans that are already dead (`revoked_at` set, or expired). Enforcement reads `expires_at > now`, so a pruned expired row changes nothing; a live ban is never touched |

---

## 2. `expires_at` design

### 2.1 What expires, per row

| Row                 | `expires_at` | Rule                                                                                                                                                                               | CHECK                                                      |
| ------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `device_records`    | NOT NULL     | `last_seen_at + 30d` (sliding), or the hold/ban extension (§4), or `now` on explicit revoke                                                                                        | `expires_at >= last_seen_at`, `expires_at > first_seen_at` |
| `device_sessions`   | NOT NULL     | `last_seen_at + 30d`                                                                                                                                                               | `expires_at >= last_seen_at`                               |
| `device_bans`       | NULL allowed | NULL = live until revoked (admin-only, never automatic); on revoke set `revoked_at`, and the sweeper retains 12mo from `revoked_at` (or from `expires_at` if it expired naturally) | `expires_at IS NULL OR expires_at > created_at`            |
| `device_evidence`   | NOT NULL     | `created_at + case retention class` (12mo default; L7/CSAM ≥ 1yr from report), copied at write so the row is self-contained                                                        | `expires_at > created_at`                                  |
| `enforcement_cases` | NOT NULL     | same class as its evidence                                                                                                                                                         | `expires_at > created_at`                                  |
| `legal_holds`       | NOT NULL     | `preserve_until` = received + 90d, renewable; **never NULL**                                                                                                                       | `preserve_until > created_at`                              |
| `erasure_receipts`  | NOT NULL     | `created_at + 24mo`                                                                                                                                                                | `expires_at > created_at`                                  |

**The device key is the row.** There is no separate "device key" object to expire: `token_hash` (client token match), `device_key` (fingerprint digest), the profile columns, and the sessions all live on rows covered above. Expiring the record expires all of it. A live ban **pins** the record (`expires_at` = ban expiry + 30 d); once the ban has ended, the ban row may outlive the record on its own 12-month clock.

### 2.2 CHECK constraint (the one that matters)

```sql
CHECK (expires_at >= last_seen_at)          -- device_records / device_sessions
CHECK (expires_at IS NULL OR expires_at > created_at)   -- device_bans
CHECK (expires_at > created_at)             -- evidence / holds / receipts
```

`expires_at >= last_seen_at` is the load-bearing one: it makes "sliding window" a schema invariant, not a convention. A writer that bumps `last_seen_at` without bumping `expires_at` gets a constraint failure, not a silently immortal row. Both columns are INTEGER epoch seconds so the comparison is meaningful (see finding 7).

### 2.3 Sliding window vs fixed expiry — the honest answer

Active use **does** extend retention under this design (`expires_at = last_seen + 30d` on every login/link write). That is defensible _because the purpose is live_ (recognising a returning device, detecting ban evasion) and it must be stated as such in the retention schedule — A5 defect B.

But "active forever" is not acceptable without a bound, so the design needs a **token rotation cap**, enforced on the write path, not the sweeper:

> If a record's `first_seen_at <= now - 365d` at a login/link write: rotate `token_hash` **in place** (same ULID, new random token), reset `first_seen_at = now`, and hand the client the new token. The old row is then reaped by the ordinary sweeper once its sessions age out. The digest continuity preserves enforcement (a live ban on the digest still matches); the identity history is capped at one year.

Orphans (no `user_id`, no ban, no case, no hold) die at 30 days of inactivity with no cap needed. The cap only bounds the always-active signed-in device. Test: `active_record_token_rotates_in_place_after_one_year`.

Sliding updates happen in the same upsert as the sighting, so there is no second write:

```sql
INSERT INTO device_records (id, device_key, key_kid, token_hash, user_id,
                            first_seen_at, last_seen_at, expires_at, created_at, seen_count)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6 + 2592000, ?6, 1)
ON CONFLICT(token_hash) DO UPDATE SET
  last_seen_at = excluded.last_seen_at,
  expires_at   = excluded.expires_at,
  seen_count   = device_records.seen_count + 1,
  device_key   = excluded.device_key,
  key_kid      = excluded.key_kid,
  user_id      = COALESCE(excluded.user_id, device_records.user_id);
```

(The conflict target is `token_hash`, the UNIQUE identity — DESIGN-v1 dropped `UNIQUE(device_key)` deliberately; `device_key` is the non-unique cohort/enforcement axis.)

---

## 3. Evidence that must outlive its subject

### 3.1 Exact retention rule for a device record

A record survives its 30-day candidate expiry **iff any** of:

1. a **live ban** on the record (`device_bans.device_record_id`, `revoked_at IS NULL AND (expires_at IS NULL OR expires_at > now)`) — the ban needs the record for token-level attribution and for the appeal;
2. an **open case** (`enforcement_cases.state IN ('proposed','active','appeal')` for the same digest);
3. an **active legal hold** naming the record;
4. **remaining sessions** (children are deleted first; the `NOT EXISTS` guard makes this automatic).

Otherwise it is deleted. Evidence does **not** retain the record: `device_evidence` carries its own frozen `device_key` + `key_kid` + `expires_at`, so deleting the record does not orphan it (finding 12). That is the whole point of the self-contained design.

### 3.2 Why this is not a join explosion

- The sweeper's candidate set is a bounded range scan on `idx_device_records_expires` (`expires_at <= now`), capped at 500 rows.
- Each guard is a `NOT EXISTS` **semi-join**, which SQLite evaluates as a coroutine that stops at the first match — O(1) per candidate with the right index:
- `idx_device_bans_active_record` (partial, `WHERE revoked_at IS NULL`) for guard 1;
- `idx_device_sessions_key` / `idx_device_sessions_expires` for sessions;
- `idx_hold_subjects_lookup (subject_kind, subject_id)` for the hold probe.
- Never a `LEFT JOIN` + `DISTINCT`: a left join fans out one row per ban/case/hold per device and destroys early exit. Rule for reviewers: **no JOIN in the sweeper, `NOT EXISTS` only.**
- The record's delete guard is by `device_record_id`. A live ban always carries it (that is the pin), and `SET NULL` cannot fire while the guard keeps the record alive.

---

## 4. The hold mechanism

### 4.1 Schema

`legal_holds` + `legal_hold_subjects` exactly as in §1.1. `authority` is the LE/regulator/court reference, `reason` free text, `owner` a named operator (never a shared account), `scope_json` the verbatim demand, `preserve_until` mandatory (90-day default, renewable), `status active|released`, release fields. Subject kinds: `device_record | user | case | evidence`.

### 4.2 How the sweeper consults it

Two mechanisms, one source of truth:

1. **Denormalized extension (primary, join-free):** hold creation runs in **one transaction**: insert `legal_holds`, insert `legal_hold_subjects`, then `UPDATE` every subject row's `expires_at = MAX(expires_at, preserve_until)` (records, sessions by `device_record_id`, evidence by `case_id`/`device_record_id`, bans by `device_key`). The sweeper's predicate is then just `expires_at <= now`; it never reads holds in the hot path. Cost is O(#subjects) once per hold (dozens of rows), not O(candidates) per sweep.
2. **Explicit semi-join (belt and braces):** the record, session, ban, and evidence deletes in §1.3 also carry the `NOT EXISTS legal_hold_subjects` probe. This is the correctness backstop: a hold that failed to extend `expires_at` still cannot cause a deletion. With `idx_hold_subjects_lookup` it is a point probe per candidate.

**Release:** in one transaction: `status='released'`, `released_at`, `released_by`, then for every subject row recompute the natural expiry (`last_seen + 30d`, `created_at + class`, ban rule) — **if the natural expiry is already past, set `expires_at = now + 30d`** so the operator gets a grace window to finish the release workflow before the sweeper removes it; if the natural expiry is later, keep it. Delete the `legal_hold_subjects` rows. The receipt of the release is an `audit_logs` row (operator action), not a subject row.

### 4.3 Invariants (self-checked, §9)

- No row whose id appears in `legal_hold_subjects` with an active hold may have `expires_at <= now` (proves mechanism 1 ran).
- No active hold may have `preserve_until <= now` (proves renewals happen; alert, don't auto-release).
- A hold is never deleted by any sweeper (only `status='released'` rows older than 24mo may be pruned, with their subjects).

---

## 5. DSAR / erasure procedure

### 5.1 Steps an operator runs

**Step 0 — intake.** Authenticated self-service (`POST /account/device-records/delete`) or emailed request. Every request gets a `request_ref` (ULID) and a timestamp. Today neither the endpoint nor the SOP exists (finding 9).

**Step 1 — verification.** Account-linked: the authenticated session is sufficient. Device-only: the client-held token may verify **that device's own record only**, never another subject's (A5 §5.2).

**Step 2 — identify.** By account:

```sql
SELECT id, device_key, key_kid, first_seen_at, last_seen_at, trust_state, user_id
  FROM device_records WHERE user_id = ?;
SELECT device_record_id, device_key, user_id, user_session_id, linked_at
  FROM device_sessions WHERE user_id = ?;
SELECT id, device_record_id, device_key, scope, reason, expires_at, revoked_at
 FROM device_bans WHERE device_record_id IN (/* record ids above */);
SELECT subject_id FROM legal_hold_subjects
 WHERE subject_kind = 'device_record' AND subject_id IN (/* record ids */);
```

**Step 3a — no live ban, no hold (the normal case):** one transaction:

```sql
DELETE FROM device_sessions WHERE user_id = ?;
DELETE FROM device_records  WHERE user_id = ?;
```

**Step 3b — live ban:** strip to the ban, one transaction:

```sql
-- keep: id, device_key, key_kid, revoked_at, ban rows. Remove everything else.
UPDATE device_records
   SET token_hash = NULL, user_id = NULL, trust_state = 'restricted',
       seen_count = 0, cohort_key = NULL, evidence = NULL,
       expires_at = MIN(expires_at, ?now + 2592000)
 WHERE id = ?;                     -- profile fields die with the key's life
DELETE FROM device_sessions
 WHERE user_id = ? AND device_record_id NOT IN (/* banned record ids */);
-- keep the sessions that are the ban's linkage evidence, only for the ban's duration
```

The ban row itself is retained: record id + digest copy + reason class + window + reviewer + `expires_at`. That is Art 17(3)(e) restriction, not free retention.

**Step 3c — active hold:** do not delete the held rows; strip nothing from held evidence; return a partial response that says exactly which rows are frozen, under which authority, until when.

**Step 4 — receipt** (one transaction with step 3):

```sql
INSERT INTO erasure_receipts
  (id, request_ref, subject_kind, subject_ref, action, deleted_json, retained_json,
   created_at, expires_at)
VALUES (?1, ?2, 'device_record', ?3, 'delete',
        '{"fields":["token_hash","device_key","profile","sessions"]}',
        '{"retained":[],"backup_window_days":30}', ?4, ?4 + 63072000);
```

`subject_ref` is the **deleted row's ULID**, not the digest and not the token. It is the replay key for a post-restore re-delete (§7) and it re-identifies nothing on its own once the row is gone. Plus one `audit_logs` row (`action='device.delete'`) for the operator-action audit.

**Step 5 — respond** (Art 12(3), within one month) with the receipt contents, not a template:

```
Deleted from the live database on <UTC date>:
  - device record(s) <opaque ULIDs>
  - account/device links: <n>
  - profile fields: trust state, sighting counts, coherence flags
Kept (only if a ban/hold is live):
  - enforcement record for an active ban until <expiry> — legal basis Art 17(3)(e):
    device key digest, reason class, decision date, reviewer
Copies: encrypted backups may still contain the deleted rows for up to 30 days;
after that they are destroyed. If a backup is ever restored, we re-apply this
receipt so the deletion is repeated.
Next: reading without signing in creates no device record. Signing in again
creates a new record with no link to the deleted one — except that if you are
under an active ban, retained enforcement evidence can be linked to the new
record by the fingerprint digest for human review. That is deliberate ban
enforcement and is disclosed here.
```

### 5.2 The "continued use recreates the record" lie — avoid it, don't paper it

The honest position, and the one that matches this design:

- v1's token is **random, server-minted**, not a deterministic HMAC of the device (`A9 §3.4`; DESIGN-v1 B4). So a deleted record is genuinely gone; a new record is a new random token. There is no "same record comes back".
- The **fingerprint digest is deterministic** under the pepper. A new record computed from the same device signals produces the same `device_key`. If that digest appears in retained enforcement evidence (active ban/open case/hold), the new record can be linked to that evidence for human review — never banned on the digest alone (a digest is a cohort, A2 F1). That is the one real limit, and the DSAR text must say it.
- If no ban/case/hold exists, the new record has no link to anything deleted.
- **Anonymous reading creates nothing.** B2 forbids the collector on reader routes and B3 writes only on login/link. So "sign out and read" is a true statement, not a hedge.

Do not write "erasure is permanent against future use". Do not promise pepper destruction per user (finding 11).

### 5.3 What is missing to execute today

No endpoint, no SOP doc, no receipt table, no self-service account delete (only `POST /user/v1/admin/delete/{user_id}`), no DSAR bundle endpoint, and the live policy already promises "delete your account and its synced data" (`web/messages/en.json:190`). Either build the mechanism or change the sentence — not both missing.

---

## 6. Pepper destruction

### 6.1 What `DEVICE_KEY_PEPPER` actually protects

32-byte secret → HKDF → `token_subkey` (hashes the client token) and `fp_subkey` (hashes the fingerprint components). Stored values are `token_hash` and `device_key` (digest). Because the component space is small and enumerable (hardware/platform/locale/screen classes, UA-CH values), **the pepper is the only thing between the stored digests and offline brute-force**. Destroying it is the only cryptographic erasure for residual digests in evidence and backups. Deleting rows is erasure of the rows; destroying the pepper is erasure of the _derivability_ of what remains.

### 6.2 Can it be destroyed while the system keeps working?

Technically yes, functionally no. On the next boot with a new pepper:

- every `token_hash` fails to match → every returning browser is a "new device" → global step-up storm, and since bans attach to records reached through the token, **all device bans stop matching** (including permanent ones);
- every `device_key` digest fails to match → all cohort lookups return nothing;
- every `device_evidence` digest becomes unmatchable → past cases cannot be linked to new devices, so ban-evasion detection for existing cases ends;
- the app boots and serves normally; nothing crashes. It is a silent enforcement amnesia, which is why it must be a deliberate, logged event (A9 finding 2, failure mode 3/6).

### 6.3 Correct procedure for a true "erase everything device-related"

1. Decide and record: this is a global invalidation event (incident response, regulator order, or "stop device processing entirely"). It is never a per-user DSAR action.
2. Freeze new device writes (`DEVICE_COLLECTION_ENABLED=false` or equivalent boot flag) so nothing is re-created mid-procedure.
3. Export/retain nothing that you are not legally required to keep; for the rest: `DELETE FROM device_sessions; DELETE FROM device_records;` (bans: revoke first and record why, then `DELETE FROM device_bans;`). Evidence under hold stays.
4. Rotate `DEVICE_KEY_PEPPER` to a fresh 32-byte CSPRNG value; ensure `DEVICE_KEY_PEPPER_PREV` is unset (the token subkey has no valid `_PREV` story — A9 §1.4). Restart.
5. Remove the old value from **every** store that held it: Dokploy env tab (and Dokploy's own DB/backups), GitHub environment secrets if mirrored, any host `.env`, shell history, password manager, CI logs. Grep the deploy host for the old value; a secret you rotated but left in a backup is not destroyed.
6. Post-conditions: `SELECT COUNT(*) FROM device_records` = 0; no row carries `key_kid` referencing the destroyed key; `PRAGMA wal_checkpoint(TRUNCATE)` so the WAL's old frames are gone; then swap the live file for a `VACUUM INTO` copy in a maintenance window — API stopped, swap, restart (§8.3 step 8) — since the freelist still holds deleted bytes otherwise (finding 2).
7. Accept and record the consequences: all users re-enrol, all device bans reset, evidence digests dead. If any of that is unacceptable, you cannot destroy the pepper — you can only delete rows.

### 6.4 Operationally viable?

**As a routine erasure tool: no.** It costs every user a step-up and every ban its effect, and it cannot be undone. **As a nuclear option: yes, and it is the only true "forget every device" action.** The alternative for the 99% case is per-subject row deletion (sweeper + DSAR) with the pepper retained until the last evidence row expires, then destroyed. Say exactly that in the SOP so nobody promises a per-user pepper rotation.

---

## 7. Backups

### 7.1 Current state: none

`deploy/README.md` has no backup/restore/DR section; `docker-compose.yml` mounts `api_data` (a single Docker volume on one VPS) and nothing copies it; no CI job, no script, no cron. So today:

- a VPS/volume loss destroys every account, ban, case, and receipt — the retention _proofs_ die with the data;
- "deletion propagates to backups" is vacuously true and also meaningless, because there are no backups to propagate to;
- there is no way to restore after a bad migration, and no way to prove a restore works.

### 7.2 Minimum backup policy that makes the retention claims honest

1. **Nightly snapshot with `VACUUM INTO`** (a consistent, compact copy; safe with a live writer) to `/app/data/backup/easyquran-<UTC ts>.db`, taken in a quiet hour, then `PRAGMA integrity_check` on the copy.
2. **Off-host copy immediately** (rclone/scp to object storage, encrypted with age/gpg — the DB is personal data). Same-VPS copies are not backups.
3. **Retention: 7 daily + 4 weekly, and a hard 30-day cap on any copy containing device rows.** 30 days is the disclosed propagation window; anything longer makes "deleted at 30 days" false.
4. **Backup expiry is enforced by a job, not by convention**: delete files older than the cap and record the deletion count.
5. **Restore runbook in `deploy/README.md`**: stop api → restore the file → delete stale `-wal`/`-shm` → start api → `integrity_check` → **run the retention sweep immediately** (reaps rows whose expiry passed while the backup was cold) → **replay `erasure_receipts`** (`DELETE FROM device_records WHERE id IN (SELECT subject_ref FROM erasure_receipts WHERE subject_kind='device_record')`; same for sessions) → re-apply active holds → verify counts.
6. **Keys are not in the backup.** `COOKIE_KEY`, `FIELD_ENC_KEY`, `DEVICE_KEY_PEPPER` live in the secret store; a restore must re-supply them. A pepper restore older than a rotation means global re-enrolment (A9 failure mode 6) — state it in the runbook.
7. **Quarterly restore drill** into a scratch container, asserting row counts + `integrity_check` = ok. An untested backup is a hope, not a backup.

### 7.3 The restore-resurrection fact

A restore from a backup taken before an erasure brings the erased row back. The retention claim is only honest if (a) the backup window is bounded and disclosed, and (b) the erasure ledger is replayed on restore. That is why `erasure_receipts.subject_ref` stores the deleted row's ULID: the boot replay (§9) and the restore runbook re-delete exactly those rows, idempotently. Without a replayable ledger, the only honest disclosure is "we cannot guarantee re-deletion after a restore" — which is not a claim to ship.

---

## 8. File growth

### 8.1 WAL

- `journal_mode=WAL` (`sea_connect.rs:48`), default `wal_autocheckpoint=1000` pages (~4 MiB at 4 KiB pages). WAL grows with writes and resets at checkpoint; **it is reused, not truncated** — `journal_size_limit` default is -1 (no limit), verified in SQLite docs. So the WAL keeps its high-water mark and does not shrink.
- Fix: `PRAGMA journal_size_limit = 67108864` (64 MiB) on the app connection (add to `configure_sqlite`, and to the maintenance connection) so the WAL is truncated to ≤64 MiB after a checkpoint.
- A long read transaction (the sweeper's batches are short; `VACUUM INTO` is not) blocks checkpointing → WAL grows during the backup copy. Take backups in a quiet window.
- A `wal_checkpoint(TRUNCATE)` at the end of the nightly maintenance cycle reclaims it now. It can return `busy=1` if a reader is active — non-fatal, retry next cycle.

### 8.2 No `auto_vacuum`

Default NONE (verified): deleted pages go to the freelist, are reused, and the file **never shrinks**. A `DELETE` therefore does not reduce the file, and without `secure_delete` the deleted row bytes remain readable in free pages and WAL frames (finding 2). Track `PRAGMA freelist_count` / `page_count` as a metric; vacuum only when the freelist ratio is high (e.g. >25%) and disk is tight.

### 8.3 Concrete `VACUUM INTO` procedure that does not stall the pool

```
1. Never on the shared pool. The app's connection is pool=1; a VACUUM INTO on it
   holds the single connection for the whole copy → every request queues.
2. Open a dedicated maintenance connection in the same process (WAL supports
   concurrent readers + one writer):
     Database::connect("sqlite:./data/easyquran.db?mode=rwc" with max_connections(1),
                       busy_timeout=5000, journal_size_limit=64MiB)
3. PRAGMA wal_checkpoint(PASSIVE);                 -- cheap, non-blocking
4. VACUUM INTO '/app/data/backup/easyquran-<UTC ts>.db';
   -- destination must not exist; unique name per run; dir owned by appuser
   -- consistent snapshot; source stays untouched; source read txn holds back
   -- checkpointing for the copy's duration → run in the quiet window
5. PRAGMA integrity_check;                          -- on the copy
6. PRAGMA wal_checkpoint(TRUNCATE);                 -- reclaim WAL after the copy
7. off-host copy, then prune by the §7.2 policy
8. Never in-place VACUUM on the live connection: it rewrites the file (2x disk),
   blocks writers, and cannot run inside a transaction. If in-place compaction
   is ever needed, stop the API, run `VACUUM` offline, restart.
```

`VACUUM INTO` doubles as the backup mechanism: the copy is compact, the live file is not. The live file shrinks only through an offline swap in a maintenance window (step 8). Do not add a second, separate VACUUM job.

---

## 9. Verification — how to PROVE retention works

### 9.1 Tests (`device_retention.rs` + migration contract test)

Exact assertions:

| Test                                                         | Assertion                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `expired_unbanned_record_is_deleted`                         | fixture: record `expires_at = now-1`, no ban/session → after `sweep_once`: `COUNT(*) WHERE id=? == 0`                                                                                                                                                          |
| `record_with_live_ban_survives_past_expiry`                  | record expired, ban `revoked_at IS NULL, expires_at = now+30d` → row still present, `report.records == 0`                                                                                                                                                      |
| `record_under_hold_survives`                                 | expired record + active hold subject → row present; and after `release_hold` + expiry extension `now+30d`, still present; after that passes, deleted                                                                                                           |
| `record_with_remaining_sessions_survives`                    | expired record with one session `expires_at = now+1d` → present; after the session expires and is swept, record deleted in the same tick                                                                                                                       |
| `revoked_ban_pruned_after_12mo` / `live_ban_never_pruned`    | `revoked_at = now-366d` → deleted; `revoked_at IS NULL, expires_at IS NULL` → present                                                                                                                                                                          |
| `evidence_survives_record_deletion`                          | evidence row for a record; sweep deletes the record; `COUNT(device_evidence WHERE case_id=?) == 1` and its `device_key` unchanged                                                                                                                              |
| `sweep_is_idempotent`                                        | second `sweep_once` returns all zeros, row counts unchanged                                                                                                                                                                                                    |
| `sweep_is_bounded`                                           | insert 1200 expired records, `batch=500, max_batches=2` → first tick deletes exactly 1000, `backlog == true`; second tick finishes                                                                                                                             |
| `active_record_token_rotates_in_place_after_one_year`        | login write with `first_seen_at = now-366d` → same `id`, new `token_hash`, old token no longer matches, a live ban on the record still applies                                                                                                                 |
| `hold_extension_is_atomic`                                   | force an error mid-transaction on hold create → no `legal_holds` row, no `expires_at` change                                                                                                                                                                   |
| `check_rejects_sliding_without_expiry`                       | `INSERT ... last_seen_at = now, expires_at = now-1` → CHECK violation                                                                                                                                                                                          |
| `sweeper_queries_use_indexes`                                | `EXPLAIN QUERY PLAN` for each DELETE contains `USING INDEX idx_device_records_expires` / `idx_device_sessions_expires` / `idx_device_bans_active_record` / `idx_hold_subjects_lookup` — pins that a dropped index cannot silently become a full scan on pool=1 |
| `migration_creates_tables_with_expected_columns_and_indices` | m000004-style `PRAGMA table_info` + `index_list` equality; `up()` twice clean (m000011 style)                                                                                                                                                                  |

### 9.2 Metric

Follow `telemetry.rs` (no-op unless `ENABLE_QUICKWIT_OTEL`, so the log line is the primary signal):

- `device.retention.deleted` — Counter, attribute `table` ∈ {sessions, records, bans, evidence, receipts};
- `device.retention.candidates` — ObservableGauge: `SELECT COUNT(*)` on the candidate predicate (cheap with the expiry indexes; compute only if the OTel endpoint is enabled);
- `device.retention.last_success_unixtime` — Gauge; alert if older than `2 × interval`;
- `device.retention.errors` — Counter, attribute `table`;
- `device.retention.hold_mismatch` — Gauge from the self-check below; must be 0.

### 9.3 Periodic self-check (boot + daily, inside the sweeper task)

One query per invariant; any non-zero result logs `error!` and increments a gauge:

```sql
-- (a) held rows that are already a deletion candidate → hold extension failed
SELECT COUNT(*) FROM device_records d
  JOIN legal_hold_subjects hs
    ON hs.subject_kind='device_record' AND hs.subject_id = d.id
  JOIN legal_holds h ON h.id = hs.hold_id AND h.status='active'
 WHERE d.expires_at <= :now;                       -- must be 0

-- (b) active holds past their preserve_until → renewal missed
SELECT COUNT(*) FROM legal_holds WHERE status='active' AND preserve_until <= :now;  -- must be 0

-- (c) sweeper lag: candidates that no guard explains
SELECT COUNT(*) FROM device_records d
 WHERE d.expires_at <= :now
   AND NOT EXISTS (SELECT 1 FROM device_sessions s WHERE s.device_record_id = d.id)
   AND NOT EXISTS (SELECT 1 FROM device_bans b
 WHERE b.device_record_id = d.id AND b.revoked_at IS NULL
                      AND (b.expires_at IS NULL OR b.expires_at > :now));  -- must be 0 after a sweep

-- (d) live ban whose record is gone → the pin failed
SELECT COUNT(*) FROM device_bans b
 WHERE b.revoked_at IS NULL AND (b.expires_at IS NULL OR b.expires_at > :now)
 AND NOT EXISTS (SELECT 1 FROM device_records d WHERE d.id = b.device_record_id); -- must be 0

-- (e) growth: SELECT page_count, freelist_count FROM pragma_page_count(), pragma_freelist_count();
```

Boot replay (non-fatal, after migrations, before the listener): `DELETE FROM device_sessions WHERE device_record_id IN (SELECT subject_ref FROM erasure_receipts WHERE subject_kind='device_record'); DELETE FROM device_records WHERE id IN (SELECT subject_ref FROM erasure_receipts WHERE subject_kind='device_record');` — idempotent, bounded by the receipt table (24-month expiry), and the reason a restore cannot silently resurrect an erased row.

---

## 10. Retention claims the current repo CANNOT honor

| #   | Claim (where it lives)                                                  | Status today                                                                                                                                | Fix                                                                                                                                        |
| --- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | "keyed device key 30d, ban row 12mo, aggregates forever" (DESIGN-v1 B3) | **Cannot.** No tables, no `expires_at`, no sweeper.                                                                                         | m0000NN + `device_retention.rs` + tests (§1, §9)                                                                                           |
| 2   | "`expires_at` enforced by a batched sweeper" (B7)                       | **Cannot.** No sweeper exists; the only sweeps are sessions (`main.rs:764`) and the post publisher.                                         | register `device_retention::start` (§1.4)                                                                                                  |
| 3   | "evidence outlives its subject" (A9 F13)                                | **Cannot** if `device_evidence`/`device_sessions` get CASCADE FKs; not yet built at all.                                                    | self-contained evidence, no CASCADE (§1.1, §3)                                                                                             |
| 4   | "deletion propagates within the backup rotation window" (A5 F6)         | **Vacuous then false.** No backups exist; and no receipt ledger to replay.                                                                  | §7 policy + `erasure_receipts` replay                                                                                                      |
| 5   | "delete your account and its synced data" (live policy, `en.json:190`)  | **Cannot.** Only `POST /user/v1/admin/delete/{user_id}` exists; no self-service path, no device-data delete, no receipt.                    | self-service delete + DSAR SOP or reword the policy                                                                                        |
| 6   | "raw signals never stored" (A5 §2.1)                                    | **False while `user_sessions.device` stores a raw UA and `ip_address` stores the IP** with no retention.                                    | name them in the retention schedule + DSAR bundle, or stop storing raw UA (keyed class), and add them to the sweeper                       |
| 7   | "aggregates forever" (B3)                                               | **False if aggregates carry `cohort_key`/digests** — keyed values are re-identifiable while the pepper lives.                               | aggregates = counts only; no keyed digests beyond the evidence window                                                                      |
| 8   | "the pepper is destroyed on erasure" (if anyone writes it)              | **False.** Pepper destruction is a global invalidation event (§6).                                                                          | never claim it; SOP says row deletion is the per-user mechanism                                                                            |
| 9   | "deleted" (unqualified)                                                 | **Cannot mean physical erasure.** `secure_delete` off, `auto_vacuum=NONE`, WAL retains frames.                                              | say "removed from the live database; backups within 30 days"; add `secure_delete` + checkpoint + VACUUM INTO swap for the physical variant |
| 10  | India 30-day retention vs DPDP Rule 8(3) 1-year floor (A5 defect C)     | **Cannot be reconciled by extending to 1 year** (wrong for the purpose) and cannot be claimed as 30d for Indian users.                      | jurisdiction gate: no device processing for India (A5 §3(d)) — then the 30d claim is true only for non-India users                         |
| 11  | "12-month ban record"                                                   | **Cannot.** Device bans do not exist; IP bans are live `rate_limit_state` keys with no ledger and no reason/actor (`admin_bans_v1/dto.rs`). | `device_bans` + `user_bans` wiring (A8 F1) before the claim ships                                                                          |
| 12  | "deletion receipts" (A9 F13)                                            | **Cannot.** `audit_logs` has zero writers and no `expires_at`.                                                                              | `erasure_receipts` table (§1.1) + one `audit_logs` row per action                                                                          |
| 13  | "minor flag → records deleted" (B7)                                     | **Cannot.** No minor flag, no delete path.                                                                                                  | minor flag + the same delete path as DSAR, tested                                                                                          |
| 14  | "hold-aware retention" (A8 F14)                                         | **Cannot.** No `legal_holds`, no hold-aware sweeper.                                                                                        | §4                                                                                                                                         |

---

## Findings

All NEW relative to A5/A8/A9/A10.

### 1. Retention has no mechanism at all — the ladder is a sentence

**Severity: Critical.**
Evidence: DESIGN-v1 B3/B7 assert `expires_at NOT NULL` + "batched sweeper registered in `scheduler.rs`"; `migration/src/lib.rs:12-13` stops at m000011; no `expires_at` on any device table (the schema's only expiry is `user_bans.expires_at`, which has zero writers); `scheduler.rs:11-25` publishes posts only; `main.rs:764-775` sweeps sessions only.
Fix: m0000NN + `device_retention.rs` + registration + the §9 tests, before any retention sentence ships.

### 2. `DELETE` on this build is not physical erasure

**Severity: High.**
Evidence: `libsqlite3-sys-0.30.1/build.rs:122-138` — no `SQLITE_SECURE_DELETE`; SQLite docs — `auto_vacuum` default NONE, deleted bytes stay in freelist and WAL frames; `journal_size_limit` default -1 (WAL reused, never shrinks).
Fix: qualify the claim ("removed from the live DB; backups within 30 days"); `PRAGMA secure_delete=ON` on the app connection; `wal_checkpoint(TRUNCATE)` after sweeps; a `VACUUM INTO` file swap in a maintenance window (API stopped) for the physical variant.

### 3. No backup exists anywhere, so deletion propagation is unbacked

**Severity: Critical (for truthfulness of the retention claim).**
Evidence: `deploy/README.md` has no backup/restore/DR section (grep); `docker-compose.yml:159-160` — one `api_data` volume on one VPS; no CI job (`.github/workflows/images.yml`); no script.
Fix: §7 policy (nightly `VACUUM INTO`, off-host encrypted copy, 30-day cap, enforced expiry, restore runbook, quarterly drill).

### 4. Restore resurrects erased rows and there is no replay ledger

**Severity: High.**
Evidence: no erasure code, no receipts, no restore procedure; A9 F13 suggests receipts but not replayability.
Fix: `erasure_receipts.subject_ref` = deleted row ULID; boot + restore replay deletes those ids; disclosed 30-day backup window.

### 5. `legal_holds` does not exist and nothing can stop a deletion

**Severity: High.**
Evidence: A8 F14; `grep legal_hold` in repo → 0 hits; no retention job exists to be stopped either.
Fix: §4 schema + transactional hold create/release that extends `expires_at` + the `NOT EXISTS` backstop + the three invariants.

### 6. What a device ban is keyed on, given the 30-day record, is unresolved

**Severity: High.**
Evidence: DESIGN-v1 B3 says `device_bans` "own table" and "keyed record 30d, ban row 12mo" but never states the ban key; B3 also drops `UNIQUE(device_key)` and calls it a cohort axis. If the ban keys on the record id, deleting the record (30d) breaks it unless the record is pinned; if it keys on the client token, clearing storage evades it; the fingerprint digest survives everything but is exactly the cohort key that A1/design-v1 rejected as a sole ban unit.
Fix: key the ban on `device_bans.device_record_id` and **pin** the record while the ban is live (`expires_at` = ban expiry + 30 d; skipped by the sweeper), so the 30-day rule never breaks a live ban. Not the digest (a cohort ban, A2 F1), and no new record at the 1-year cap — rotate `token_hash` in place on the same id (§2.3). Copy the digest onto the ban row as evidence only.

### 7. `expires_at` as TEXT with mixed formats is a silent no-op

**Severity: High.**
Evidence: `user_bans.expires_at` is chrono RFC3339 (`user_ban/actions.rs:12,18`); `audit_logs.created_at` defaults to SQLite `CURRENT_TIMESTAMP` (`m000001_init.rs:261`); SQLite compares across storage classes by class order, so INTEGER vs TEXT comparisons are always false.
Fix: all retention columns INTEGER epoch seconds (like `rate_limit_state`/`sessions`), CHECKs comparing like-typed columns, Rust-only writes.

### 8. Sliding-window expiry can retain an always-active device indefinitely

**Severity: Medium.**
Evidence: A5 defect B; DESIGN-v1 B3 has `last_seen` but no outer cap.
Fix: `expires_at = last_seen + 30d` is the rule; plus a one-year in-place `token_hash` rotation on the write path (§2.3) and a test.

### 9. DSAR cannot be executed today

**Severity: High.**
Evidence: no DSAR/erasure module (`grep` DSAR/erasure → 0 hits); no self-service delete (`user_v1/mod.rs:29` is admin-only); no receipt; the live policy already promises deletion (`web/messages/en.json:190`).
Fix: §5 endpoint + SOP + receipt + response template, or reword the policy until built.

### 10. Raw UA + IP in `user_sessions` poison every device-retention claim

**Severity: High.**
Evidence: `m000001_init.rs:48-57`; `auth_v1/controller.rs:183-187`; A8 F6.
Fix: name them in the retention schedule and the DSAR bundle, add them to a retention rule (or store a keyed class instead of the raw UA). Any "device data is deleted after 30 days" sentence is false until this is handled.

### 11. Pepper destruction is a global invalidation, not a per-user erasure

**Severity: High.**
Evidence: A9 §1.4/finding 2; DESIGN-v1 B4 makes the token subkey non-rotatable; the secret lives in Dokploy env + possibly GitHub secrets + host `.env`.
Fix: §6 SOP; never promise per-user pepper rotation; staged "stop processing → delete rows → rotate → purge every secret store" procedure; `key_kid` guard so a mixed-key deployment is visible.

### 12. Evidence vs subject: CASCADE FKs would destroy the evidence the ban needs

**Severity: Critical.**
Evidence: A10 finding 1 (`user_sessions` CASCADE, `admin_delete` hard delete); `device_evidence` does not exist yet; A9 F13 says self-contained.
Fix: no CASCADE FKs on evidence/sessions; evidence carries its own digest + kid + expiry; record deletable while evidence survives; test `evidence_survives_record_deletion`.

### 13. Sweeper placement/contention rules are unstated

**Severity: Medium.**
Evidence: `rate_limit_store.rs:120-125` (companion shares the connection); `sea_connect.rs:35` (pool=1); A10 §7 recommends scheduler registration but does not specify batching/pause/locks.
Fix: own 6h spawn, 0-300s jitter, ≤500-row batches, 50ms pause, ≤20 batches/table, no `PERSISTENCE_LOCK`, no flush-companion ride, per-table non-fatal errors.

### 14. No retention verification: no test, no metric, no self-check

**Severity: High.**
Evidence: no deletion counters in `telemetry.rs`; no boot invariant; A8 F8 asked for self-heal jobs; nothing exists.
Fix: §9 tests (13 exact assertions), OTel counters/gauges, boot + daily invariant queries (a)-(e), boot erasure replay.

---

## Cleanup code shape (compact reference)

```rust
// services/device_retention.rs — registered in services/mod.rs, started in main.rs:761 area
pub fn start(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(jitter_0_300s()),
            Duration::from_secs(env_u64("DEVICE_RETENTION_INTERVAL_SECS", 6 * 3600)),
        );
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match sweep_once(&state.sea_db, epoch_now(), batch, max_batches).await {
                Ok(r) => report_metrics_and_log(r),
                Err(e) => warn!(error = %e, "device retention sweep failed (non-fatal)"),
            }
        }
    });
}

async fn sweep_once(db, now, batch, max_batches) -> Result<SweepReport, DbErr> {
    let mut r = SweepReport::default();
    // order matters: sessions -> records -> bans -> evidence -> receipts
    r.sessions = run_batches(db, SESSIONS_SQL, vec![now.into(), now.into(), batch.into()], batch, max_batches).await?;
    r.records  = run_batches(db, RECORDS_SQL,  vec![now.into(), now.into(), batch.into()], batch, max_batches).await?;
    r.bans     = run_batches(db, BANS_SQL,     vec![now.into(), batch.into()],               batch, max_batches).await?;
    r.evidence = run_batches(db, EVIDENCE_SQL, vec![now.into(), batch.into()],               batch, max_batches).await?;
    r.receipts = run_batches(db, RECEIPTS_SQL, vec![now.into(), batch.into()],               batch, max_batches).await?;
    run_self_check(db, now).await;   // §9.3, logs error! on any invariant violation
    Ok(r)
}

async fn run_batches(db, sql, args, batch, max_batches) -> Result<u64, DbErr> {
    let mut total = 0;
    for _ in 0..max_batches {
        let n = db.execute(Statement::from_sql_and_values(DatabaseBackend::Sqlite, sql, args.clone()))
                  .await?.rows_affected();
        total += n;
        if n < batch { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;  // yield to flush + requests
    }
    Ok(total)
}
```

Each `*_SQL` is the §1.3 statement: `DELETE FROM t WHERE id IN (SELECT ... WHERE <expiry> AND NOT EXISTS <live ban> AND NOT EXISTS <hold> ORDER BY <expiry> LIMIT ?)`. Never `DELETE ... LIMIT` (unsupported), never a `LEFT JOIN`, never a lock held across the sleep.

**Minimum ship order:** m0000NN → sweeper + tests → erasure receipts + boot replay → `legal_holds` → backup job + restore runbook → policy rewrite. Until the first three land, remove every retention sentence from the policy and the design.
