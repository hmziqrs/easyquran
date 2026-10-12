# E4 — The ban contract bundle: implementation plan + adversarial audit

Auditor: E4, implementation-plan pass. Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran` @ HEAD `53d3d421`.
Target: the FIRST real feature release in the recommended plan — D2 §2.3/§12 "WS2a ban contract" (the five
items D2 F4 names: bans API + index + session kill + banned UX + audit row + notice row), plus C1 §8.1
PR2/PR3 as scoped by C1 §7.1 (A2/A3/A4-email-only/A5-subset) and Gate 0's prerequisites (D2 §10.3).
Method: read D2, C1, C5 (D20/D26/D27), A3 (F1/F3/F10/F13/F14), B8 (F6/F7/F12/F13), D1 (claims 7/8/9/14),
C2 (item 10); then read the actual code for every piece; then run the load-bearing DDL/planner experiments
against real SQLite (`sqlite3 :memory:`, transcript quoted inline).

**Verdict up front.** The bundle is the right release and it is implementable in one deploy. Three things in
the plan as written are wrong or under-specified, and all three are load-bearing:

1. **C1's index is not the right index.** `(user_id, revoked_at, expires_at)` is non-unique, so it does not
   give the DB-level idempotency the bundle needs, and it indexes dead rows. A **partial UNIQUE index**
   `(user_id) WHERE revoked_at IS NULL` serves the exact `get_active_ban` query (verified with
   `EXPLAIN QUERY PLAN`) _and_ makes a second active ban impossible — which is the real answer to
   "two admins ban the same user concurrently". One index, two jobs. Requires `create` to revoke
   expired rows in the same transaction first.
2. **The banned-UX fix as implied ("remove AUTH_006 from `CREDENTIAL_TYPES`") does not work.** `403` is in
   `CREDENTIAL_STATUSES`, so `403 AUTH_006` still classifies as a credential failure. The classifier needs
   an explicit `banned` branch _before_ the credential-status fallback, covering `AUTH_006` (login
   `ErrorResponse`) **and** `AUTH_BANNED` (the middleware body, `rux-auth/src/error.rs:39-40,77`).
3. **The mail-suppression fix must be enforcement-scoped.** D2 F6 is right that
   `transactional + suppressed` is dropped with `Ok` (`mail/router.rs:241-259`), but the router's
   `lookup_suppressed` folds complaint/permanent-bounce into one bool (`router.rs:119-135`); a global
   "send on Complaint" change would also start delivering verification/reset mail to complained
   addresses. The send-despite-suppression rule must key on the template.

Also verified: the session-kill primitive is a **single `UPDATE users SET session_auth_secret`** in the ban
transaction — deleting `sessions` rows is unnecessary and costs a binding lookup + N deletes on the
pool-of-1 connection. The `unverified` guard omission is real (`auth_guard.rs:46-55`) and must be fixed,
but note the integration test cannot observe it once the secret rotates (the extractor kills the session
first), so the guard fix needs a requirements-level test. `NotificationKind` gains a variant safely (no
exhaustive matches). The migration takes the **next free number** at implementation time (`m000012` is unused in code today,
but `next-features.md` reserves it for reading sessions).

**Corrected estimate: 7–10.5 dev-days** for the bundle as defined (D2's 4–6 d "WS2a" excludes the notice
email, the suppression fix, the banned UX, and the ar i18n; C1's A2+A3+A4+A5-subset is 6.5–10 d but A4
includes a portal this bundle drops). Item-by-item in §11.

---

## 0. Scope (what is in the bundle, exactly)

**In (must land together):**

| #   | Item                                                                                         | Why it is in the contract                                                                                                               |
| --- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | m0000NN: one-active-ban index + `enforcement_notices`                                        | Without the index, `get_active_ban` is a scan per login + per 5-min refresh on pool-of-1; without the table there is no decision record |
| 2   | `/admin/users/bans` list/create/revoke/resend-notice, ROLE_ADMIN + CSRF                      | Today no account can be banned at all (zero writers; `user_ban/actions.rs:11,31` unused)                                                |
| 3   | Session kill (rotate `session_auth_secret` in the ban transaction)                           | A ban that takes ≤5 min and leaves the session alive is not an enforcement primitive (C1 F9)                                            |
| 4   | Banned UX: classifier + panel + i18n (en+ar)                                                 | Today the banned user is told "Email or password is incorrect." (`auth-copy.ts:80-88,137-139`)                                          |
| 5   | Audit rows (`ban.create`/`ban.revoke`)                                                       | `audit_logs` has zero writers (D1 claim 14)                                                                                             |
| 6   | Notice row + enforcement email + delivery state                                              | Silent permanent lockout otherwise; Art 17 owed                                                                                         |
| 7   | Enforcement-correctness fixes that make 3–4 true: `unverified` guard + clear-on-ban ordering | Otherwise the contract is false in two specific ways (429 instead of ban; verify/resend reachable)                                      |

**Out (explicit non-goals, with owners):** the appeal portal/token page (D2 §3.2: build at the first real
ban), `enforcement_cases` + `case_events` state machine (D2 F3; defer with the device tranche), queue
caps/aging (zero cases), the retention sweeper (WS6), limiter correctness (`abuse_peek`/`abuse_record` —
C1 PR1, separate), comment hide/delete → notice wiring (D2 F14 §11.1 — **recommended as a same-release
sibling PR**, +0.5–1 d once the notice table exists; it is a live Art 17 surface and shares this bundle's
template), newsletter-500 fix (D2 F16, separate), web admin UI (D30: API + TUI only).

---

## 1. Migration

### 1.1 Number

**Take the next free number when this lands** — `migration/src/lib.rs:3-13` lists m000001–m000011 and no
code uses `m000012`, but `next-features.md` reserves it for reading sessions, so do not hard-code it. If
another migration lands first, renumber **both** the file and the `Migrator::migrations()` entry
(`migration/src/lib.rs:20-56`). File: `migration/src/m0000NN_ban_contract.rs` + `mod m0000NN_ban_contract;` +
`Box::new(...)` entry.

### 1.2 Exact DDL (verified against real SQLite)

```sql
-- up(): one conn.transaction, every statement IF NOT EXISTS.
-- SQLite DDL is transactional when wrapped in BEGIN (C2 item 1); sea-orm-migration 1.1.20
-- runs SQLite up() with no outer transaction and writes the version row after it
-- (sea-orm-migration-1.1.20/src/migrator.rs:261-272), so the wrap is what makes a crash re-runnable.

-- 0. Defensive pre-dedupe (m000011 precedent): keep the newest active ban per user,
--    auto-revoke the rest, so the UNIQUE index can always be created.
UPDATE user_bans SET revoked_at = created_at, revoked_by = NULL
 WHERE revoked_at IS NULL
   AND id NOT IN (SELECT MAX(id) FROM user_bans WHERE revoked_at IS NULL GROUP BY user_id);

-- 1. One active ban per user, enforced by the DB. Also serves get_active_ban:
--    verified `SEARCH user_bans USING INDEX idx_user_bans_one_active (user_id=?)`.
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_bans_one_active
    ON user_bans (user_id) WHERE revoked_at IS NULL;

-- 2. The decision record: C4 §4.4 DDL + ban_id + expires_at/created_at (D27 requires an
--    executable retention column; C4's DDL lacks it). No FKs anywhere: the record must
--    outlive the user (D23/A3 #10/B8 F12).
CREATE TABLE IF NOT EXISTS "enforcement_notices" (
    "id" INTEGER PRIMARY KEY AUTOINCREMENT,
    "subject_type" TEXT NOT NULL,            -- 'user' | 'comment' | 'post'
    "subject_id" INTEGER NOT NULL,
    "ban_id" INTEGER,                        -- user_bans.id when the decision is a ban
    "decision" TEXT NOT NULL,                -- 'hide' | 'remove' | 'suspend' | 'terminate'
    "scope" TEXT NOT NULL,                   -- 'content' | 'account' | 'service'
    "starts_at" INTEGER NOT NULL,            -- epoch seconds
    "ends_at" INTEGER,                       -- NULL = permanent
    "reason_code" TEXT NOT NULL,
    "reason_text" TEXT NOT NULL,             -- plain language, shown to the user
    "facts" TEXT NOT NULL,                   -- what was relied on, plain language
    "automated_means" INTEGER NOT NULL DEFAULT 0 CHECK (automated_means IN (0,1)),
    "contract_ground" TEXT NOT NULL,         -- 'terms:acceptable-use'
    "decided_by" INTEGER,                    -- admin user id (no FK)
    "decided_at" INTEGER NOT NULL,
    "second_reviewer_id" INTEGER,            -- always NULL with one operator (O6)
    "notice_channel" TEXT,                   -- 'email' | 'in_app' | 'none'
    "notice_sent_at" INTEGER,
    "notice_delivery_state" TEXT, -- 'sent' | 'suppressed' | 'no_contact_details' | 'send_failed'
    "appeal_token_hash" TEXT,                -- unused in this bundle (email-first appeals)
    "appeal_window_ends_at" INTEGER,
    "appeal_submitted_at" INTEGER,
    "appeal_outcome" TEXT,                   -- 'upheld' | 'reversed' | 'partially_reversed'
    "appeal_decided_at" INTEGER,
    "appeal_reviewer_id" INTEGER,
    "retention_class" TEXT NOT NULL DEFAULT 'enforcement_12mo',
    "expires_at" INTEGER NOT NULL CHECK (expires_at >= decided_at),
    "created_at" INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_enforcement_notices_subject
    ON enforcement_notices (subject_type, subject_id, decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_enforcement_notices_ban
    ON enforcement_notices (ban_id);
```

Verified in `sqlite3 :memory:`: the file runs clean twice (idempotent); the CHECK rejects
`expires_at < decided_at`; the partial UNIQUE index rejects a second active ban with
`UNIQUE constraint failed: user_bans.user_id`; after `UPDATE ... revoked_at=...` a new ban inserts.

### 1.3 Why this index and not C1's

C1 §5.1.3 / D2 §2.3.2 propose
`idx_user_bans_user_active ON user_bans (user_id, revoked_at, expires_at)` (non-unique). Measured:

| Index                                                | `EXPLAIN QUERY PLAN` for the real `get_active_ban` SQL                                               | Enforces one-active-ban |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | ----------------------- |
| `(user_id, revoked_at, expires_at)`                  | `SEARCH user_bans USING INDEX ... (user_id=? AND revoked_at=?)` + `USE TEMP B-TREE FOR ORDER BY`     | no                      |
| `(user_id) WHERE revoked_at IS NULL` (partial)       | `SEARCH user_bans USING INDEX idx_user_bans_one_active (user_id=?)` + `USE TEMP B-TREE FOR ORDER BY` | **yes**                 |
| `(user_id, revoked_at, expires_at, created_at DESC)` | still `USE TEMP B-TREE` (the `expires_at IS NULL OR > ?` term breaks ordering)                       | no                      |

The partial unique index is strictly better: it is smaller (indexes only live bans), serves the hot query
exactly, and turns "create is idempotent" from an application check into a DB guarantee — the only thing
that survives two concurrent creates. `classify_db_error` already maps SQLite's
`UNIQUE constraint failed` to `ErrorCode::DuplicateEntry` (409) (`src/error/database.rs:4-11`), so the
concurrent loser gets a clean 409 with no string parsing in the controller.

**Required companion change:** `create` must auto-revoke _expired_ active rows first, or a user whose ban
expired can never be re-banned (the partial index would see a still-"active" row). One statement inside the
same transaction:

```sql
UPDATE user_bans SET revoked_at = <now RFC3339>, revoked_by = NULL
 WHERE user_id = ? AND revoked_at IS NULL AND expires_at IS NOT NULL AND expires_at <= <now>
```

`revoked_by = NULL` marks it as system-expiry, not operator revoke.

### 1.4 Columns needed on `user_bans` — none

- `banned_at`: no — `created_at` already is it (TEXT, `m000001_init.rs:63`).
- `reason_code`: no — the structured code lives on `enforcement_notices.reason_code`; the ban row keeps
  free-text `reason` and is not the decision record.
- `case_id`: no — there is no case machine in this bundle (D2 F3); the notice row _is_ the case, and
  `enforcement_notices.ban_id` gives the exact 1:1 link (finding F7).
- `retention_class`/`expires_at` INTEGER: no — do **not** migrate `user_bans.expires_at` from TEXT to
  INTEGER (C1 §5.1.5: keep Rust-only writes/comparisons); the WS6 sweeper's ban-row rule is
  `max(expires_at, revoked_at, created_at) + 12 mo`, expressed in the sweeper config, not a new column
  (finding F12).

### 1.5 Live-DB safety

- All statements are `IF NOT EXISTS`; the whole batch is inside `conn.transaction::<_, (), DbErr>` so a
  failure (disk full, `SQLITE_BUSY`) rolls back and the version row is never written; re-running is safe.
- The index build is O(rows) and `user_bans` has **zero rows** today (C3 §3), so it is instant even
  without the pre-dedupe.
- Run migrations with the old container drained (rolling-deploy ordering, C1/D32): a concurrent writer
  holding the pool's single connection can otherwise make the DDL wait on `SQLITE_BUSY`.
- `down()`: **drop only `idx_user_bans_one_active`**, leave `enforcement_notices` and its indexes.
  Precedent: m000008 keeps irreversible data and drops only the index (`m000008:74-82`), while
  m000005/m000007/m000010 drop new tables (`:85-93`, `:64-72`, `:51-57`). A production rollback that drops
  the Art 17 record is the one rollback this repo must not ship (finding F11). Document "roll forward".

---

## 2. Ban create/revoke API

### 2.1 Routes, auth, CSRF

New module `src/modules/admin_user_bans_v1/{mod.rs,dto.rs,controller.rs}`, merged (not nested) in
`src/router.rs` next to `admin_bans_v1` (`router.rs:150-153`) so the absolute operator URLs are exact:

| Route                                      | Method | Auth                                             | CSRF                                     | Notes                                    |
| ------------------------------------------ | ------ | ------------------------------------------------ | ---------------------------------------- | ---------------------------------------- |
| `/admin/users/bans`                        | GET    | session `ROLE_ADMIN` (`verified_with_role::<3>`) | n/a (safe method)                        | list; `?user_id=&active_only=&page=`     |
| `/admin/users/bans`                        | POST   | same                                             | yes (global `csrf_guard`, `main.rs:842`) | create                                   |
| `/admin/users/bans/{ban_id}/revoke`        | POST   | same                                             | yes                                      | revoke + optional appeal outcome         |
| `/admin/users/bans/{ban_id}/notice/resend` | POST   | same                                             | yes                                      | re-send after `send_failed`/`suppressed` |

- **No export token on this module.** The bearer pattern stays read-only IP-bans only
  (`admin_bans_v1/mod.rs:13-33`; A8 F11). Do not merge the two DTOs; `/admin/bans` is IP bans and
  `BanUnit::parse` cannot parse a user id (C1 §5.1.8).
- Revoke is POST-with-optional-body, not DELETE: the appeal outcome must be recorded on the notice row, and
  the repo's admin mutations are POST (`user_v1/mod.rs:24-36`, `admin_route_v1/mod.rs`). If a bodyless
  DELETE is preferred, it can be added as an alias, but it cannot carry `appeal_outcome`.

### 2.2 DTOs (exact)

```rust
// src/modules/admin_user_bans_v1/dto.rs
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserBanPayload {
    pub user_id: i32,
    #[validate(length(min = 1, max = 2000))]
    pub reason: String,          // -> notice.reason_text (shown to the user)
    #[validate(length(min = 1, max = 64))]
    pub reason_code: String,     // closed set: abuse:spam|abuse:harassment|abuse:csam|abuse:evasion|abuse:other
    #[validate(length(min = 1, max = 4000))]
    pub facts: String,           // -> notice.facts ("what we looked at")
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>, // RFC3339; None requires permanent=true
    pub permanent: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RevokeUserBanPayload {
    pub appeal_outcome: Option<String>,  // upheld|reversed|partially_reversed
    #[validate(length(max = 2000))]
    pub resolution_note: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UserBanListQuery {
    pub user_id: Option<i32>,
    pub active_only: Option<bool>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,   // clamp 1..=100
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserBanResponse {
    pub ban: user_ban::Model,
    pub notice_id: i32,
    pub notice_delivery_state: Option<String>,  // None = send failed (row is the queue)
}
```

Validation rules beyond `Validate`: `user_id > 0`; target exists; `target.id != actor.id`; target role
`< actor.role` (mirror `user_v1/controller.rs:269-291` admin_delete); `expires_at.is_none()` requires
`permanent == Some(true)` (no accidental permanent ban); `reason_code` in the closed set.

### 2.3 Idempotency — the exact decision

**Refuse a second active ban with 409 + the existing ban id.** Rationale: `get_active_ban` returns one row
(`actions.rs:59-65`), revoke is by id, and revoke-then-create silently discards the first decision's
notice link (F7). The DB-level guard is the partial unique index; the app-level pre-check gives the nicer
error. Concurrent losers get `DB_003 DuplicateEntry` (409) from `classify_db_error` — same outcome.

### 2.4 Function signatures (exact)

```rust
// src/modules/admin_user_bans_v1/controller.rs
pub async fn list_bans(State(state): State<AppState>, _auth: AuthSession,
    query: ValidatedQuery<UserBanListQuery>) -> Result<impl IntoResponse, ErrorResponse>;

pub async fn create_ban(State(state): State<AppState>, auth: AuthSession,
    payload: ValidatedJson<CreateUserBanPayload>) -> Result<impl IntoResponse, ErrorResponse>;

pub async fn revoke_ban(State(state): State<AppState>, auth: AuthSession,
    Path(ban_id): Path<i32>, payload: ValidatedJson<RevokeUserBanPayload>)
    -> Result<impl IntoResponse, ErrorResponse>;

pub async fn resend_notice(State(state): State<AppState>, _auth: AuthSession,
    Path(ban_id): Path<i32>) -> Result<impl IntoResponse, ErrorResponse>;
```

`create_ban` orchestration (order is the contract):

```
1. target checks (outside txn)                  -> 404 / 403 / 409
2. BEGIN
3.   user_ban::revoke_expired(&txn, user_id)     // expired rows out of the partial index
4.   user_ban::create(&txn, NewUserBan{..})      // the ban row
5.   user::rotate_session_auth_secret(&txn, id)  // the session kill (same txn)
6.   enforcement_notice::create(&txn, ..)        // the decision record, delivery_state NULL
7.   audit::write(&txn, "ban.create", ..)        // the audit row
8. COMMIT
9. send the email OUTSIDE the txn (network I/O must never hold the pool-of-1 connection)
10. UPDATE the notice row: notice_sent_at + delivery_state
```

- `reason_text` for `abuse:csam` must be generic ("content that violates our Terms") — the notice is
  emailed to the offender; never repeat material details (A5-class).
- `ends_at` = ban expiry epoch, or NULL for permanent; `automated_means = 0`;
  `contract_ground = 'terms:acceptable-use'`; `second_reviewer_id = NULL`;
  `single_operator_control: true` in the audit metadata (O6).
- `expires_at` (retention) = `decided_at + 365 d`; `retention_class = 'enforcement_12mo'` (D27; the
  sweeper is WS6).
- `revoke_ban`: idempotent (already revoked -> 200 no-op), one transaction: `revoke` + audit
  `ban.revoke` + (if provided) stamp `appeal_outcome`/`appeal_decided_at`/`appeal_reviewer_id` on the
  notice row found by `ban_id`. **Do not rotate the secret on revoke** — nothing to kill; the old sessions
  are already dead, and the user must log in fresh.
- `resend_notice`: load the active ban -> notice by `ban_id` -> re-render + re-send -> update state.
  This is the retry path that makes "the row is the queue" true (D2 §3.1).

### 2.5 New DB primitives (exact signatures)

```rust
// src/db/sea_models/user_ban/actions.rs — widen to ConnectionTrait (precedent: user/actions.rs:208,251)
pub async fn create<T: ConnectionTrait>(conn: &T, new_ban: NewUserBan) -> DbResult<Model>;
pub async fn revoke<T: ConnectionTrait>(conn: &T, ban_id: i32, revoked_by: Option<i32>) -> DbResult<Option<Model>>;
pub async fn revoke_expired<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<u64>;

// src/db/sea_models/user/actions.rs — the session-kill primitive (single UPDATE, no read)
pub async fn rotate_session_auth_secret<T: ConnectionTrait>(conn: &T, user_id: i32) -> DbResult<u64>;

// src/db/sea_models/enforcement_notice/{model,slice,actions}.rs — new entity
pub async fn create<T: ConnectionTrait>(conn: &T, new: NewEnforcementNotice) -> DbResult<Model>;
pub async fn find_by_ban_id(conn: &DbConn, ban_id: i32) -> DbResult<Option<Model>>;
pub async fn mark_notice_sent<T: ConnectionTrait>(conn: &T, id: i32, sent_at: i64, state: &str) -> DbResult<u64>;
pub async fn set_appeal_outcome<T: ConnectionTrait>(conn: &T, ban_id: i32, outcome: &str, decided_at: i64, reviewer_id: i32) -> DbResult<u64>;

// src/db/sea_models/audit_log/actions.rs — new (model exists, no actions.rs)
pub async fn write<T: ConnectionTrait>(conn: &T, new: NewAuditLog) -> DbResult<Model>;

// src/services/audit.rs — one typed call site helper
pub struct AuditEvent<'a> { pub actor_id: Option<i32>, pub action: &'a str, pub resource_type: &'a str,
    pub resource_id: String, pub metadata: Option<serde_json::Value>,
    pub ip_address: Option<String>, pub user_agent: Option<String> }
pub async fn write(conn: &impl ConnectionTrait, event: AuditEvent<'_>) -> DbResult<()>;
```

Audit rows: `action='ban.create' | 'ban.revoke'`, `resource_type='user_ban'`,
`resource_id=ban.id.to_string()`, metadata `{user_id, reason_code, notice_id, expires_at,
single_operator_control}`. (D2 §10.3's Gate B query counts `ban.create`; `enforcement.decision` is the
comment-moderation action, a sibling PR.)

---

## 3. Session kill

### 3.1 The mechanism: rotate `session_auth_secret` in the ban transaction. Nothing else.

The primitive already exists in two halves:

- The session payload binds to `users.session_auth_secret` (`services/auth.rs:491-499`;
  fallback to the password hash only when the secret is empty, which cannot happen — the column is
  `TEXT NOT NULL`, `m000001_init.rs:25`, and `ActiveModelBehavior` backfills empties, `user/model.rs:180-192`).
- On every request the extractor recomputes it and **deletes the session on mismatch**
  (`extractor.rs:53-56` and `:248-251`), logging "Session auth hash mismatch — invalidating stale session".
- Rotation precedent: password change (`user/actions.rs:223-228`), email change (`:146-154`), admin update
  (`:416-418`).

So the ban's effect is: **at the target's next request on any device, the session is deleted and the user is
anonymous**. Immediate (not ≤5 min), one `UPDATE`, inside the same transaction as the ban row.

### 3.2 What we do NOT do, and why

**Do not delete `sessions` rows.** To delete them you must resolve tower ids via `auth_session_binding`
(`lookup_tower_by_audit_id`, `services/auth.rs:267-287`), then call `terminate` per id
(`services/auth.rs:49-75`) or `session_store.delete` — N reads + N deletes + a revoked-set insert on the
pool-of-1 connection, all for sessions that the hash mismatch already neutralises on first touch. The
hourly sweeper (`main.rs:764-775` -> `session_store.rs:52-62`) reaps them at TTL (14 d) anyway. The
in-memory `revoked_sessions` set cannot be populated without the same binding lookup, so it adds nothing.
One statement, same outcome, no new failure mode.

### 3.3 Consequences, enumerated

- **All devices die.** Every session of the target user, everywhere, on next request. Intended.
- **The admin's own session is unaffected** — only the target's secret rotates. Self-ban and
  equal/higher-role bans are refused (§2.2), mirroring `admin_delete` (`user_v1/controller.rs:281-291`),
  so an operator cannot lock themselves (or a peer/superior admin) out of the console mid-incident.
  Banning a co-admin requires a SUPER_ADMIN.
- **Unban restores login immediately** — the login path is uncached (`auth_v1/controller.rs:153`), and
  revoke does not rotate anything. The user's old sessions stay dead; they log in fresh. Correct.
- **The 5-min ban cache is now defense in depth, not the enforcement path.** It still runs on
  `not_banned()` guards (`guard.rs:46-56`) and costs one indexed `get_active_ban` per session per 5 min.
  Keep it: it catches any future session-creating path that forgets `check_ban` within 5 minutes.
- **Mid-request race:** a request already past the extractor completes; the next request dies. At most one
  in-flight request, bounded, unavoidable without request-level fencing. State it in the SOP, not the code.
- **`user_sessions` audit rows stay unrevoked** (they are audit-only; `revoked_at` does not kill a tower
  session — see the V-HIGH-2 comment, `auth_v1/controller.rs:1061-1086`). The session list will show stale
  "active" rows until TTL. Acceptable for this bundle; if it matters, a follow-up can stamp `revoked_at`
  on the target's rows in the same transaction (one extra UPDATE) — but it is cosmetic, not enforcement.

### 3.4 What breaks if the kill is too aggressive

The only over-broad case is rotating a user we did not mean to. The transaction makes that impossible
(single `user_id`); the role/self checks bound the social blast radius. If the operator _wants_ to ban a
peer admin, the super-admin path exists. Document it.

---

## 4. Enforcement correctness

### 4.1 `unverified` guard omission (C2 item 10, D1 claim 7) — confirmed

`src/middlewares/auth_guard.rs:46-55`: `unverified` builds `auth_requirements().authenticated().unverified()`
with **no `.not_banned()`**; `authenticated` (`:31`), `verified` (`:64-67`), `verified_with_role`
(`:79-86`) and `admin_or_export_token` (`admin_bans_v1/controller.rs:209-216`) all include it. Scope of the
omission today: `/email_verification/v1/verify|resend` (`email_verification_v1/mod.rs:8-12`).

**Fix (one line):** `auth_requirements().authenticated().unverified().not_banned()`.

**But the honest note:** with the session kill, a banned user's session is deleted before any guard runs,
so the integration test cannot observe the omission — it observes the extractor. The guard fix is still
required (defense in depth for any future session path that skips `check_ban`), and it must be proven at
the requirements level: a test asserting each of the four guard requirement sets has `not_banned == true`
(and `unauthenticated` does not). `AuthRequirements` fields are `pub(crate)`; add the test inside
`rux-auth` (or expose a `has_ban_check()` accessor) — `builder.rs:72-113` already has a unit-test module.

### 4.2 Limiter-before-ban ordering (D1 claim 8, C1 F10) — confirmed, and the naive fix is a trap

`auth_v1/controller.rs`: limiter at `:133-140`, `authenticate_password` at `:142-145`, ban check at
`:153-166`. The banned user burns the `login:<email>` counter and on the 3rd attempt gets 429
`Retry-After: 3600` instead of the ban.

**Do not hoist the ban check above `authenticate_password`**: the ban is only known after a correct
password, and checking it by email first would turn `403 AUTH_006` into a ban/account-existence oracle
for anyone who knows the address.

**Correct fix — clear the credential counter on the ban path.** In the banned branch, before returning
`AccountLocked`:

```rust
// A banned user's retries must never become a 429 (and must not consume the
// credential's budget); the ban check only runs with a correct password.
let _ = state.gate_store.clear_limit(&format!("login:{login_key}")).await;
```

`clear_limit` exists on the store (`rux-request-gate/src/store.rs:53,391-395`) and is used from controllers
already (`admin_bans_v1/controller.rs:152-153`). Because the counter is cleared on every banned attempt,
it can never reach the 3-attempt temp block. This is the minimal ordering fix; the full
`abuse_peek`/`abuse_record` redesign is C1 PR1 (separate).

### 4.3 Is the 5-min cache acceptable? Yes — and the honest UI copy

After the secret rotation the cache is no longer the ban's propagation path (§3.3). It stays as a
backstop. **UI copy must not promise anything about timing or duration**: the panel says the account is
suspended and how to appeal; it must not say "for 5 minutes", "temporarily", or "soon" (permanent bans
exist, and the cache is not a user-facing concept).

### 4.4 Guard fns that must change

| Fn                                                    | Change                                               |
| ----------------------------------------------------- | ---------------------------------------------------- |
| `unverified` (`auth_guard.rs:46-55`)                  | add `.not_banned()`                                  |
| `authenticated`, `verified`, `verified_with_role`     | none (already correct)                               |
| `unauthenticated`                                     | none — must stay ban-free (public/login surface)     |
| `admin_or_export_token`                               | none (already correct; export token stays read-only) |
| `check_requirements` (`guard.rs:46-56`)               | none (ban logic already correct)                     |
| `log_in` ban branch (`auth_v1/controller.rs:153-166`) | add `clear_limit` before returning AccountLocked     |

---

## 5. Banned UX

### 5.1 The classifier — the exact bug and fix

`web/src/lib/auth/auth-copy.ts`:

- `CREDENTIAL_STATUSES = {401,403,404}` (`:80`) and `CREDENTIAL_TYPES` includes `"AUTH_006"` (`:81-88`).
- `classifyAuthError` checks `CREDENTIAL_STATUSES.has(status) || (type && CREDENTIAL_TYPES.has(type))`
  (`:137-139`) -> `{kind:"credential", message: CREDENTIAL_FAILURE}` ("Email or password is incorrect.",
  `:3`).
- Consumed by `credentialFailure` (`flows.svelte.ts:118-134`) -> `LoginFlow.fail` (`:181-185`).

**Removing `"AUTH_006"` from `CREDENTIAL_TYPES` alone changes nothing** — `403` is in
`CREDENTIAL_STATUSES`, so `403 AUTH_006` still classifies as a credential failure. Exact fix:

```ts
export const ACCOUNT_BANNED = "This account is suspended.";
export type ErrorKind =
  "field" | "credential" | "verified-only" | "banned" | "rate-limit" | "transport" | "server";
const BANNED_TYPES = new Set(["AUTH_006", "AUTH_BANNED"]);
// in classifyAuthError, next to the VERIFIED_ONLY branch and BEFORE the credential fallback:
if (type && BANNED_TYPES.has(type)) {
  return { kind: "banned", fieldErrors: {}, message: ACCOUNT_BANNED };
}
// and remove "AUTH_006" from CREDENTIAL_TYPES (belt and braces; now unreachable)
```

Both wire codes are needed: the login path returns `{"type":"AUTH_006",...}` (`ErrorResponse` renames the
code to `type`, `src/error/response.rs:8-10`; 403 at `ruxlog-types/src/error.rs:20-21,231`), and the
middleware path returns `{"error":{"code":"AUTH_BANNED",...}}` (`rux-auth/src/error.rs:39-40,77`). The web
decoder already normalises both shapes (`auth-client.ts:112-117`).

### 5.2 Flow + panel

`LoginFlow` (`flows.svelte.ts`):

- add `banned = $state(false)`; reset it in `submitCredentials` (`:190`) and `reset()` (`:157-165`);
- in `fail` (`:181-185`): `if (classifyAuthError(...).kind === "banned") { this.banned = true; this.genericError = null; this.fieldErrors = {}; return; }`.

`SignInForm.svelte` (credentials branch): when `flow.banned`, render a banned panel (heading + body +
appeal link) and disable the submit; do not render the credential error. Same component serves the page and
the modal, so both surfaces get it. No retry loop exists to stop (the flow is user-initiated); the point is
that the banned state never falls into the credential path.

### 5.3 Appeal link — placeholder decision

Email-first (D2 §3.2): `mailto:appeals@easyquran.fyi`. No page, no token, no i18n route, no enumeration
surface. **Pre-ship check: the mailbox must actually exist** (Cloudflare Email Routing for
`easyquran.fyi`; D1 claim 4 verified the MX exists, C4 D11 requires the address to be real). The notice
email says "reply to this email or write to appeals@easyquran.fyi"; the panel shows the same address.
The tokenized page is deferred to the first real ban (D2 §3.2, C1 F11's portal half).

### 5.4 i18n keys and the gate

`web/scripts/check-i18n.mjs`: `LOCALES = ["en","ar"]`, `DOMAINS` includes `messages/auth`; every key must
exist in both locales with a matching signature. `i18n-namespaces.json:127-129` claims prefix `auth` for
the auth namespace, so new `auth_*` keys flow into the generated barrel `src/lib/i18n/m/auth.js`
(`gen-message-namespaces.ts`; `i18n:check` runs the generator in write mode and `--check` for staleness).

Add to `messages/auth/en.json` and `messages/auth/ar.json` (exact drafts):

| key                   | en                                                                                                                                                                       | ar                                                                                                                                               |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `auth_banned_heading` | `Your account is suspended`                                                                                                                                              | `تم إيقاف حسابك`                                                                                                                                 |
| `auth_banned_body`    | `This account can't sign in right now. Reading stays open — no account needed. If you think this is a mistake, email appeals@easyquran.fyi and a person will review it.` | `لا يمكن لهذا الحساب تسجيل الدخول الآن. تظل القراءة متاحة دون حاجة إلى حساب. إذا كنت تعتقد أن هذا خطأ، راسل appeals@easyquran.fyi وسيراجعه شخص.` |
| `auth_banned_appeal`  | `Email appeals@easyquran.fyi`                                                                                                                                            | `راسل appeals@easyquran.fyi`                                                                                                                     |

Then: extend `AuthCopy` + `getAuthCopy()` (`src/lib/i18n/auth-copy.ts:80-160`), run
`bun run i18n:namespaces` (regenerates `m/auth.js`), keep `bun run check/lint/test` green.

Note: the existing hardcoded English error strings in `lib/auth/auth-copy.ts` (CREDENTIAL_FAILURE etc.) are
outside the i18n catalogs. The banned panel is new user-facing UI -> it goes through the gate. Do not add a
catalog key for `ACCOUNT_BANNED` (it is a classifier fallback like its siblings).

---

## 6. Notice

### 6.1 Template

- `src/services/mail/templates/enforcement_notice.html` (new; mirror `forgot_password.html` structure),
  registered in `templates/mod.rs:14-63` with `include_str!` + a render test (`mod.rs:75-160` pattern).
- `TEMPLATE_ENFORCEMENT: &str = "enforcement_notice"` in `provider.rs:13-15`; add to `is_transactional`
  (`provider.rs:17-22`).
- `send_enforcement_notice(mailer, to, ctx) -> Result<SendReceipt, MailError>` in `services/mail/mod.rs`
  next to `send_email_verification_code` (`mod.rs:34-68`). Context: `reason_text`, `facts`,
  `decided_at`, `ends_at` (or "permanent"), appeal address, "you can still read without an account",
  "this decision was made by a person", "single-operator review" honesty line (O6).
- Tera auto-escapes; the template must not include the CSAM details rule (§2.4).

### 6.2 Suppression — verified, and D2 F6 needs one correction

Verified: `mail/router.rs:241-259` — `transactional` + suppressed -> `Ok(SendReceipt::default())`
(dropped, indistinguishable from a send). `lookup_suppressed` (`:119-135`) returns a **bool** folding
`row.permanent || (reason == Bounce && within_cooldown)`; a Complaint is `permanent` (webhook path,
`mail_v1/controller.rs:149`, `email_suppression/actions.rs:36`). So the reason is present in the row but
discarded by the router. Correct fix:

```rust
// router.rs: replace lookup_suppressed with:
async fn lookup_suppression(&self, recipient: &str) -> Result<Option<SuppressionReason>, ()>;
// in send():
if let Some(reason) = self.lookup_suppression(&msg.to).await? {
    if transactional && msg.template == Some(TEMPLATE_ENFORCEMENT)
        && reason == SuppressionReason::Complaint {
        // CAN-SPAM exempts transactional messages; a marketing complaint must not
        // block a legal notice. Fall through and send.
    } else {
        router_metrics.suppressed.add(1, &[]);
        return Ok(SendReceipt { suppressed: true, ..Default::default() });
    }
}
```

- `SendReceipt` gains `#[serde(default)] pub suppressed: bool` (`provider.rs:24-32`); update the two
  struct literals (`cloudflare.rs:150-154`, `smtp.rs:119-123`).
- Enforcement-scoped, not global: verification/reset keep today's drop-on-complaint behaviour (deliverability +
  the anti-enumeration shape), and no other template changes.
- **Bounce (permanent or in-cooldown) and Manual suppression:** do not send; receipt `suppressed: true`;
  the caller records `notice_delivery_state='suppressed'` (H3 #22; D2 F6's second half).
- Transport/provider error: record nothing yet; the row stays with `notice_sent_at IS NULL` and the
  response reports it; `resend_notice` is the retry path.
- The caller maps the receipt: sent -> `'sent'`; suppressed -> `'suppressed'`; Err -> `'send_failed'`.

### 6.3 `NotificationKind` + in-app row

- Add `Enforcement` to `ruxlog-types/src/enums/notification.rs` (`#[serde(rename="Enforcement")]`,
  `sea_orm(string_value="enforcement")`). Safe: no exhaustive `match` on the enum outside its definition
  (`grep NotificationKind::` -> only `services/notification/mod.rs:186`), and string values are the
  stored contract — appending is additive.
- Write the in-app row in the same transaction (`notification::Entity::create`, `slice.rs` takes the kind).
- **Do not count it as a notice channel**: the list is behind `auth_guard::authenticated`
  (`notification_v1/mod.rs:14-22`) and the banned session is dead -> unreachable while banned; it becomes
  visible after unban. `notice_channel = 'email'`; the in-app row is the durable second copy. The
  `notifications` table has no retention column — note it for WS6 (finding F10).

### 6.4 No-email fallback

Every user row has an email (no nullable column), so "no address" cannot happen today. The real fallback is
a suppressed/bouncing address: `suppressed` on the row, the in-app copy, and the admin response
surfaces the state so the operator can reach the user another way (the decision still stands; the SOP says
how). Do **not** block the ban when the mailer has no provider configured — an operator must be able to act
when mail is down; instead the response reports `send_failed`/`suppressed` loudly (log at `warn!`).

---

## 7. Tests

Rust, in-memory SQLite + `oneshot` (pattern: `tests/admin_bans_export.rs:55-120`; new file
`tests/ban_contract.rs`):

1. `ban_create_kills_live_session_and_blocks_relogin` — login (real cookie), ban, next request with the old
   cookie -> 401 (extractor deleted the session), re-login -> 403 `AUTH_006` (ban, not credential).
2. `banned_user_cannot_pass_any_guard` — after the ban, hit one route per guard class with the old cookie:
   `/user/v1/get` (authenticated), `/email_verification/v1/resend` (unverified),
   `/post/comment/v1/create` (verified), `/user/v1/admin/list` (verified_with_role) — each 401; plus
   (2b) a `rux-auth` requirements test asserting `not_banned` is set by all four guard builders and not by
   `unauthenticated` (the load-bearing proof for the guard fix; the integration path cannot see it).
3. `banned_login_is_never_rate_limited` — 5 banned logins -> always 403 `AUTH_006`, never 429; assert the
   `login:<email>` bucket is empty via `gate_store.snapshot()`.
4. `second_active_ban_is_rejected` — 409 + existing ban id (and the DB constraint path:
   `DuplicateEntry` when the pre-check is bypassed).
5. `ban_create_rotates_session_auth_secret` — secret differs before/after; `get_active_ban` is Some.
6. `ban_revoke_restores_login_immediately` — revoke -> login 200; old sessions still dead.
7. `ban_create_writes_audit_and_notice_rows` — audit `ban.create` + notice row with `ban_id` set and
   `notice_sent_at IS NULL` when the mailer has no providers (`MailRouter::new(HashMap::new(), ...)`,
   as in `admin_bans_export.rs:69-76`); then `resend_notice` sets `send_failed`/sent.
8. `revoke_records_appeal_outcome_on_the_notice` — `{appealOutcome:"reversed"}` -> notice fields stamped.
9. `admin_cannot_ban_self_or_equal_or_higher_role` — 403/422 per case.
10. `get_active_ban_uses_the_partial_index` — `EXPLAIN QUERY PLAN` contains `idx_user_bans_one_active`.
11. `enforcement_notice_sent_despite_complaint_suppression` — router unit test: suppression row reason
    `complaint` + `TEMPLATE_ENFORCEMENT` -> receipt `suppressed == false`; reason `bounce` -> `suppressed ==
true`; verification + complaint -> `suppressed == true` (regression guard for §6.2).
12. `m0000NN_is_idempotent_and_keeps_notices_on_down` — up twice; pre-dedupe keeps MAX(id) and revokes the
    older duplicate; index present; `down()` drops the index and **the table survives** (migration-crate
    tests, m000008 pattern).
13. `notice_survives_user_deletion` — create ban + notice, `User::admin_delete`, notice row still present
    with `subject_id` intact (evidence preservation, F8).

Web (vitest, `web/`):

14. `auth-copy.test.ts` — `403 AUTH_006 -> banned` (this is the regression the one-line fix fails),
    `403 AUTH_BANNED -> banned`, `403 AUTH_004 -> credential` unchanged, `429 -> rate-limit` wins.
15. `login-flow.test.ts` — banned login sets `flow.banned`, clears `genericError`, does not call
    `setUser`/`transition`.
16. `i18n:check` + `i18n:namespaces:check` green (en+ar parity, barrel regenerated).

All three web gates (`bun run check`, `bun run lint`, `bun run test`) and the touched-crate Rust gates
(`cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test`) must be green (D2 WS0 scope).

---

## 8. Rollout

**Flag: none.** The bundle is inert until an admin calls the route (ROLE_ADMIN + CSRF + verified), and the
only config-struct change would break the 8 exhaustive literals A3 F14 lists. The safety comes from the
pre-ship checklist, not a flag.

**Order (one release; three commits, deploy only when C is green):**

1. **A — migration** (`m0000NN` + migration tests). No behavior change; runs at boot.
2. **B — backend contract** (module + session kill + audit + notice + guard fix + clear-on-ban + mail
   changes). With B alone and zero bans, behavior is unchanged; do not let it sit deployed across a ban
   (an operator could ban before C ships and the login would still say "Email or password is incorrect").
3. **C — web banned UX + i18n.** Then deploy.
4. **Pre-first-ban checklist:** `appeals@easyquran.fyi` receives mail (send a test); `docs/moderation-sop.md`
   written (who reads appeals, response window, self-review recorded, reversal = revoke + outcome stamp,
   CSAM handling); dry-run on dev: ban a test account, confirm the session dies on the next request, the
   email arrives, the login panel shows, revoke restores login.

**Verification SQL (post-deploy):**

```sql
-- migration landed
SELECT name FROM sqlite_master WHERE type='index' AND name IN
  ('idx_user_bans_one_active','idx_enforcement_notices_subject','idx_enforcement_notices_ban');
EXPLAIN QUERY PLAN
SELECT "user_bans"."id" FROM "user_bans"
 WHERE "user_bans"."user_id" = 1 AND "user_bans"."revoked_at" IS NULL
   AND ("user_bans"."expires_at" IS NULL OR "user_bans"."expires_at" > '2026-10-12T00:00:00+00:00')
 ORDER BY "user_bans"."created_at" DESC LIMIT 1;
-- expect: SEARCH user_bans USING INDEX idx_user_bans_one_active (user_id=?)
SELECT COUNT(*) FROM user_bans WHERE revoked_at IS NULL;   -- 0 before the first ban
SELECT COUNT(*) FROM enforcement_notices;                  -- 0
-- after the first ban
SELECT b.id, b.user_id, b.expires_at, b.revoked_at,
       n.id AS notice_id, n.notice_delivery_state, n.notice_sent_at
  FROM user_bans b LEFT JOIN enforcement_notices n ON n.ban_id = b.id
 ORDER BY b.id DESC LIMIT 3;
SELECT action, resource_type, resource_id, created_at FROM audit_logs
 WHERE action IN ('ban.create','ban.revoke') ORDER BY id DESC LIMIT 5;
```

**Rollback:** revoke the ban (POST revoke) -> login restores immediately; redeploy the previous binaries
(additive schema is ignored by the old binary); **do not run `migration down`** while any
`enforcement_notices` row exists (down is index-only by design, F11). If the UX regresses, C can be
reverted alone — B's wire codes are backward compatible (the old classifier already treats `AUTH_006` as
credential, i.e. the old bug, not a crash).

---

## 9. Attack it

**9.1 The index migration fails on a live DB.** `IF NOT EXISTS` + the transaction wrapper means a failure
rolls back and the version row is never written; the retry re-runs clean. The pre-dedupe protects against a
hand-inserted duplicate active ban (none exist). The realistic failure is `SQLITE_BUSY` from the old
container still holding the single connection during a rolling deploy — drain first, and the migration job
retries. If the index is missing (migration applied but index dropped), correctness is unaffected: full
scan, latency only, and `EXPLAIN QUERY PLAN` in CI catches it (test 10).

**9.2 The session kill is too aggressive — does it lock the admin out mid-incident?** Only the target's
secret rotates; self-ban and equal/higher-role bans are refused, so the operator cannot kill their own or a
peer's session. Banning a co-admin requires SUPER_ADMIN. The over-broad case is a _mistaken_ ban (wrong
user id): the admin UI is a CLI/TUI with the user id in the path, and revoke restores login immediately —
the blast radius is one user's sessions, recoverable in one call. Accepted.

**9.3 A ban is created for a user with an active appeal.** There is no appeal system in this bundle
(email-first). The concrete sequence: user emails appeals@; operator decides to reverse -> revoke ->
login restored immediately; the operator passes `appealOutcome:"reversed"` so the notice row records the
decision (test 8). If the operator instead _re-bans_ after an appeal, create is refused while the first ban
is active; they must revoke first, which leaves the first notice with its outcome unset unless they stamp
it — the SOP must say "stamp the outcome before re-banning". Known gap: no endpoint records
`appeal_submitted_at` for an inbound email (the mailbox is the record). Acceptable at zero cases; the
tokenized page (deferred) closes it.

**9.4 Two admins ban the same user concurrently.** Without the DB constraint both would read
`get_active_ban = None` and insert (SQLite's default deferred transactions do not serialise the
read-then-insert; pool=1 serialises the statements, not the check). With the partial UNIQUE index the
second insert fails with `UNIQUE constraint failed`, mapped to 409 `DB_003` by `classify_db_error` — one
active ban, two audit trails, no ambiguity about which ban to revoke. This is why the index must be
UNIQUE, not C1's 3-column index.

**9.5 The user is mid-request when the ban lands.** The in-flight request completes (its extractor already
validated the hash); the next request's extractor deletes the session. At most one request. If it was a
write (comment create), it lands — the ban is account-level, not content-level; the operator can hide the
comment with the existing admin route (whose notice wiring is the sibling PR). No fencing is added.

**9.6 What the banned user sees on each surface:**

| Surface                                  | Behaviour                                                                                             | Notes                                                                                            |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Login                                    | 403 `AUTH_006` -> banned panel + appeal address; never 429 (clear-on-ban); never "incorrect password" | the contract's user-visible proof                                                                |
| Reader (`/`, `/surah`, `/t/**`, offline) | unchanged — public routes, no session required                                                        | correct by design; the panel/notice say so, so "banned but reading" is not mistaken for a bypass |
| Comments / bookmarks / profile writes    | session dead -> 401; the app shows signed-out                                                         | existing comments stay visible; content removal is a separate admin action                       |
| API (any authenticated endpoint)         | 401 (dead session) or 403 `AUTH_BANNED` if a live session ever survives                               | `AUTH_BANNED` is in the classifier set                                                           |
| Email                                    | the notice: reason, facts, "decided by a person", appeal address                                      | the only channel reachable while banned                                                          |
| In-app notification                      | written, unreachable until unban                                                                      | not counted as a channel (§6.3)                                                                  |

---

## 10. File-by-file change list

**Rust — backend (`rust/backend/api/`)**

| File                                                                       | Change                                                                                                                                   |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `migration/src/m0000NN_ban_contract.rs`                                    | new: pre-dedupe, partial unique index, `enforcement_notices` + 2 indexes, transactional up, index-only down, up-twice/keep-on-down tests |
| `migration/src/lib.rs`                                                     | register `m0000NN_ban_contract`                                                                                                          |
| `src/db/sea_models/user_ban/actions.rs`                                    | widen `create`/`revoke` to `ConnectionTrait`; add `revoke_expired`                                                                       |
| `src/db/sea_models/user/actions.rs`                                        | add `rotate_session_auth_secret<T: ConnectionTrait>`                                                                                     |
| `src/db/sea_models/enforcement_notice/{mod,model,slice,actions}.rs`        | new entity: model, `NewEnforcementNotice`, create/find_by_ban_id/mark_notice_sent/set_appeal_outcome                                     |
| `src/db/sea_models/audit_log/actions.rs` (+ `mod.rs`)                      | new: `write(conn, NewAuditLog)`                                                                                                          |
| `src/db/sea_models/mod.rs`                                                 | register `enforcement_notice`                                                                                                            |
| `src/services/audit.rs`                                                    | new: `AuditEvent` + `write()`                                                                                                            |
| `src/modules/admin_user_bans_v1/{mod,dto,controller}.rs`                   | new: routes, DTOs, create/revoke/resend/list                                                                                             |
| `src/modules/mod.rs`, `src/router.rs`                                      | register + merge `/admin/users/bans*`                                                                                                    |
| `src/modules/auth_v1/controller.rs`                                        | `clear_limit("login:<email>")` in the ban branch (`:153-166`)                                                                            |
| `src/middlewares/auth_guard.rs`                                            | `.not_banned()` on `unverified` (`:53`) + requirements-level test                                                                        |
| `src/services/mail/provider.rs`                                            | `TEMPLATE_ENFORCEMENT`, `is_transactional`, `SendReceipt.suppressed`, enforcement helper                                                 |
| `src/services/mail/router.rs`                                              | `lookup_suppression` -> reason; enforcement+Complaint sends; suppressed receipt                                                          |
| `src/services/mail/templates/enforcement_notice.html` + `templates/mod.rs` | new template + registration + render test                                                                                                |
| `src/services/mail/mod.rs`                                                 | `send_enforcement_notice`                                                                                                                |
| `src/services/mail/{cloudflare,smtp}.rs`                                   | `SendReceipt { suppressed: false, .. }` literals                                                                                         |
| `rust/crates/ruxlog-types/src/enums/notification.rs`                       | `NotificationKind::Enforcement`                                                                                                          |
| `tests/ban_contract.rs`                                                    | new integration tests 1–13                                                                                                               |
| `crates/rux-auth/src/requirements/builder.rs` (or guard tests)             | requirements-level `not_banned` test (2b)                                                                                                |

**Web (`web/`)**

| File                                                             | Change                                                                                   |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `src/lib/auth/auth-copy.ts`                                      | `ACCOUNT_BANNED`, `banned` kind, `BANNED_TYPES`, remove AUTH_006 from `CREDENTIAL_TYPES` |
| `src/lib/auth/flows.svelte.ts`                                   | `LoginFlow.banned` state + `fail` branch + reset                                         |
| `src/lib/auth/components/SignInForm.svelte`                      | banned panel (heading/body/appeal), disable submit                                       |
| `messages/auth/en.json`, `messages/auth/ar.json`                 | 3 keys each                                                                              |
| `src/lib/i18n/auth-copy.ts`                                      | `AuthCopy` + `getAuthCopy()` fields                                                      |
| `src/lib/i18n/m/auth.js`                                         | regenerated (`bun run i18n:namespaces`)                                                  |
| `src/lib/auth/__tests__/auth-copy.test.ts`, `login-flow.test.ts` | tests 14–15                                                                              |

**Docs/ops**

| File                     | Change                                                                                                                                              |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/moderation-sop.md` | new: appeal mailbox SOP, response window, single-operator self-review, reversal = revoke + outcome stamp, CSAM notice rule, mid-request/5-min notes |

**Same-release sibling (recommended, not in the five-item contract):**
`src/modules/post_comment_v1/controller.rs` hide/delete -> `enforcement_notices` row + notice email
(D2 F14 §11.1; the live Art 17 surface; +0.5–1 d now that the table and template exist).

---

## 11. Dev-day estimate

| Item                                                                                                                  | Days         | Basis                                                        |
| --------------------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------ |
| m0000NN + migration tests (incl. pre-dedupe, up-twice, keep-on-down)                                                  | 0.5          | m000008/m000011 patterns exist                               |
| Admin module: DTOs, validation, role rules, list (with notice state join), create/revoke/resend, router               | 2.0–3.0      | C1 A3 (2.5–3.5) minus the index/session-kill (counted below) |
| Session-kill primitive + transaction orchestration + audit writer                                                     | 0.5–1.0      | C1 A5-subset                                                 |
| Notice: template, tera, transactional semantics, suppression reason, `SendReceipt.suppressed`, delivery state, resend | 1.0–1.5      | D2's email-only WS3a (1.5–2.5) minus appeal/SOP split        |
| Banned UX: classifier branch + flow + panel + en/ar + barrel regen                                                    | 1.0–1.5      | C1 A2 (0.5–1) + i18n gate (D2 §9.1's +0.25) + ar copy        |
| Enforcement correctness: `unverified` + clear-on-ban + requirements test                                              | 0.25–0.5     | two edits + one test                                         |
| Rust integration tests 1–13                                                                                           | 1.0–1.5      | 13 tests, several multi-request                              |
| Web tests 14–15                                                                                                       | 0.25–0.5     |                                                              |
| SOP + mailbox verification + dry-run + verification SQL                                                               | 0.5–1.0      |                                                              |
| **Total**                                                                                                             | **7.0–10.5** |                                                              |

Delta vs D2: D2's WS2a 4–6 d is the admin-API subset; adding the notice email + suppression fix + banned
UX + ar i18n + the test list gives 7–10.5. Delta vs C1: A2+A3+A4+A5-subset = 6.5–10 d, but C1's A4 includes
the appeal portal (dropped here per D2 §3.2) and its A5 includes non-ban audit call sites.

---

## 12. Findings (12, all NEW)

**F1 — C1's `user_bans` index is neither the idempotency guarantee nor the minimal index; ship a partial UNIQUE index and revoke expired rows first. NEW. High.**
Evidence: `user_ban/actions.rs:59-65` (get_active_ban); `m000001_init.rs:58-73` (no index); SQLite
experiments: `(user_id) WHERE revoked_at IS NULL` -> `SEARCH ... USING INDEX idx_user_bans_one_active
(user_id=?)`; second active insert -> `UNIQUE constraint failed: user_bans.user_id`; C1 §5.1.3's 3-col
index is non-unique. Fix: §1.2 DDL + `revoke_expired` in the create transaction.

**F2 — Removing `AUTH_006` from `CREDENTIAL_TYPES` does not fix the banned UX; `403` is in `CREDENTIAL_STATUSES`, so the credential branch still wins. NEW. High.**
Evidence: `web/src/lib/auth/auth-copy.ts:80-88,137-139`; `flows.svelte.ts:118-134`; the login body is
`{"type":"AUTH_006"}` (`error/response.rs:8-10`) and the guard body is `{"error":{"code":"AUTH_BANNED"}}`
(`rux-auth/src/error.rs:39-40,77`). Fix: explicit `BANNED_TYPES` branch before the credential fallback
(§5.1); test 14 is the regression guard.

**F3 — The `unverified` omission is real but unobservable from an integration test once the session kill lands; prove it at the requirements level. NEW. Medium.**
Evidence: `auth_guard.rs:46-55` vs `:31,66,84`; `extractor.rs:53-56,248-251` deletes the session before any
guard runs; `email_verification_v1/mod.rs:8-12`. Fix: add `.not_banned()` + the builder/requirements test
(§4.1, test 2b).

**F4 — Hoisting the ban check above `authenticate_password` would create a ban/account-existence oracle; the correct ordering fix is clear-on-ban. NEW. Medium-High.**
Evidence: `auth_v1/controller.rs:133-166` (ban known only after a correct password); `abuse_limiter.rs:34-52`
(appends before the password); `store.rs:305` (blocked path also appends); `clear_limit` exists
(`store.rs:53,391-395`). Fix: `clear_limit("login:<email>")` in the banned branch (§4.2); test 3.

**F5 — The session kill is one `UPDATE`; deleting `sessions` rows (or populating the revoked set) is unnecessary and costs binding lookups + N deletes on pool-of-1. NEW. High.**
Evidence: `services/auth.rs:267-287` (tower-id lookup needed to delete), `:49-75` (terminate),
`session_store.rs:212-222`; `extractor.rs:53-56` already deletes on mismatch; hourly sweep
`main.rs:764-775` + `session_store.rs:52-62`. Fix: rotate in the ban transaction, touch nothing else (§3).

**F6 — The D2 F6 suppression fix is under-specified and would regress verification/reset delivery if applied globally; it must be enforcement-scoped and needs a receipt flag. NEW. High.**
Evidence: `mail/router.rs:241-259` (transactional suppressed -> `Ok`), `:119-135` (`lookup_suppressed`
discards the reason; Complaint is `permanent`, `email_suppression/actions.rs:36`), `provider.rs:17-22`.
Fix: `lookup_suppression` returns the reason; send-despite-suppression only for `TEMPLATE_ENFORCEMENT` +
`Complaint`; `SendReceipt.suppressed` → `suppressed` (§6.2); test 11.

**F7 — `enforcement_notices` needs `ban_id`; subject_id alone is ambiguous after a re-ban, and the revoke path must update the exact decision record. NEW. Medium.**
Evidence: C4 §4.4 DDL (no ban link); `user_ban/actions.rs:31-54` (revoke by ban id);
`actions.rs:102-145` (history can hold many bans per user). Fix: `ban_id INTEGER` (no FK) + the two
indexes (§1.2); test 8.

**F8 — `user_bans` has `ON DELETE CASCADE`, so `admin_delete` destroys the ban row; the legal record must be the no-FK notice row (and the audit row). NEW. Medium.**
Evidence: `m000001_init.rs:58-73` (CASCADE), `user/actions.rs:486-497` (hard delete),
`audit_log/model.rs:11-16` (SET NULL), A10 F1/B8 F12/D23. Fix: keep `enforcement_notices` FK-free;
document the ban row as operational state; test 13.

**F9 — The banned login response's `message` is stripped in release builds, so the panel cannot use server copy. NEW. Medium.**
Evidence: `src/error/response.rs:12-17` (`#[cfg(not(debug_assertions))] #[serde(skip)]`). Fix: all banned
copy client-side (i18n keys, §5.4); never render `error.message`.

**F10 — The in-app notification is unreachable while banned and `notifications` has no retention column; write the row but count only email as the channel. NEW. Low-Medium.**
Evidence: `notification_v1/mod.rs:14-22` (list behind `authenticated`), `notification/model.rs:8-19` (no
expiry), C4 §4.3's "also add `NotificationKind::Enforcement`" vs D2 F4's unreachable in-app channel. Fix:
`notice_channel='email'`, row written for durability/post-unban; note the table for WS6 (§6.3).

**F11 — The migration's `down()` must not drop `enforcement_notices`; the repo convention (m000005/7/10) drops new tables, which would destroy Art 17 records on a rollback. NEW. Medium.**
Evidence: `m000010:51-57`, `m000007:64-72`, `m000005:85-93` (DROP) vs `m000008:74-82` (index-only for
irreversible data). Fix: index-only down + a keep-on-down test + "roll forward" in the SOP (§1.5).

**F12 — `user_bans.expires_at` is TEXT/RFC3339 while the new retention columns are INTEGER epoch; the WS6 sweeper must not compare them, and the ban-row retention rule needs to be written down (no `retention_class` column exists). NEW. Medium.**
Evidence: `m000001_init.rs:63` (TEXT), `user_ban/actions.rs:12,18` (chrono writes), B8 F7 (mixed
storage-class comparisons are silently always-false). Fix: keep Rust-only comparisons for `user_bans`; no
column migration; sweeper rule `max(expires_at, revoked_at, created_at) + 12 mo` recorded in the WS6
config; all new tables INTEGER (§1.4).

---

## 13. Confidence notes

- Every repo claim above is a source read at HEAD this session; the DDL/planner claims were executed in
  `sqlite3 :memory:` (transcripts in §1.2/§1.3). The test suite was not run; the estimate is engineering
  judgement calibrated to the module sizes cited.
- Production env values (mail provider config, `WEB_AUTH_ENABLED`) are not readable from the tree; the
  notice fallback behaviour assumes the router's documented semantics, which were read.
- The appeal mailbox (`appeals@easyquran.fyi`) is assumed deliverable from D1 claim 4's MX check; the
  pre-ship checklist verifies it by sending a real message.
- The partial-unique-index recommendation is the one place this plan deliberately departs from C1; the
  measurement that justifies it is in §1.3 and it is reversible (drop/replace in a later migration).

_End of E4._
