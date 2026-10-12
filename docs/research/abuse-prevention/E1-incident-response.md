# E1 — Minimum viable incident response: design, then attack

Auditor: incident-response designer (round 6). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree). Method: read `D5-final-gaps.md`, `C4-shipped-debt.md` §4 + findings, `A8-ops-realism.md`
§3–§8, `B8-retention-exec.md` §3–§7, `R2-ban-operations.md` §5–§8, `D2-v1-attack.md` F14/F15, `C3-base-rate.md`;
then read the actual primitives in `rust/backend/api/src/` and `web/src/` listed in §0. Every claim below was
read in the tree, not inferred. Read-only: no repo writes, no DB writes, no external system contacted except
`dig` for the two MX records (the only network call).

**The gap, restated in one line.** `D5` §4 is right: the capability is _irreversible_ in a way the other 35
findings are not, because the only removal and account actions that exist today **destroy the evidence the
incident needs** (§3, finding F1) and the only intake is a personal Gmail (`mailto:`) plus a Terms address
that has no MX (finding F6). This document designs the minimum, then cuts it to what survives attack.

---

## 0. Ground truth — what actually exists (verified this session)

| Primitive                              | State                                                                                                                                                                                                                                                                                  | Evidence                                                                             |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| User report intake                     | `comment_flags` + `POST /post/comment/v1/flag/{id}` — auth-only, no abuse limiter, free-text `reason`, no resolution state                                                                                                                                                             | `post_comment_v1/mod.rs:13`; `comment_flag/model.rs`; `controller.rs:341-384`        |
| Flag queue                             | list / summary / details / clear — all `ROLE_ADMIN`, no aging, no "new since", no assignee                                                                                                                                                                                             | `post_comment_v1/controller.rs:389-468`; `mod.rs:26-34`                              |
| Comment removal                        | `admin_hide` (sets `hidden=1`, row survives), `admin_unhide`, `admin_delete` (**hard delete → FK CASCADE deletes `comment_flags`**)                                                                                                                                                    | `post_comment/actions.rs:358-387`; `m000001_init.rs:298-308`                         |
| `admin_flags_clear`                    | sets `post_comments.flags_count = 0`; flag rows survive; `comment_flag::Entity::clear_flags` (`delete_many`) exists with **zero callers**                                                                                                                                              | `post_comment/actions.rs:389-400`; `comment_flag/actions.rs:179-186`                 |
| Account action                         | `admin_delete` only — hard delete, CASCADE, no audit row                                                                                                                                                                                                                               | `user_v1/controller.rs:269-318`; `user_v1/mod.rs:29`                                 |
| Ban primitive                          | `user_bans` model + `create` / `revoke` / `get_active_ban` / `is_banned` / `list_by_user` / `admin_list` — **zero callers, no admin route, no index**                                                                                                                                  | `user_ban/{model,actions}.rs`; `m000001_init.rs:58-73`                               |
| Ban enforcement                        | `check_ban` behind `not_banned()` auth guard, 5-min cache                                                                                                                                                                                                                              | `services/auth.rs:530-545`; `auth_guard.rs:27-33`                                    |
| Audit                                  | `audit_logs` exists, **zero writers**; `created_at` TEXT `CURRENT_TIMESTAMP`, has `ip_address`/`user_agent`                                                                                                                                                                            | `m000001_init.rs:252-266`; `audit_log/model.rs`                                      |
| Session evidence                       | `user_sessions.device` = raw UA, `.ip_address` = raw IP, **no expiry**, no retention job                                                                                                                                                                                               | `m000001_init.rs:48-57`; `auth_v1/controller.rs:182-186`                             |
| View evidence                          | `post_views.ip_address` + `.user_agent`, **live public writer** via `POST /post/v1/track_view/{id}` (unauthenticated, dedup 5 min)                                                                                                                                                     | `post_v1/mod.rs:63-67`; `controller.rs:346-386`; `post/actions.rs:608-647`           |
| Image moderation                       | **not a NoOp moderator — no moderator at all**: `IMAGE_MODERATION_ENABLED` default false → `state.storage.image_moderator = None` → the `if let Some(moderator)` block is skipped; `NoOpModerator` is dead code (tests only)                                                           | `main.rs:495-522`; `media_v1/controller.rs:272-305`; `image_moderation/mod.rs:34-49` |
| Upload gate                            | `ROLE_AUTHOR` + verified; role is admin-assigned, no self-serve path                                                                                                                                                                                                                   | `media_v1/mod.rs:19-21`; `user_v1/mod.rs:24-38`                                      |
| Mail                                   | templates: verification, forgot_password, welcome, newsletter_confirmation, payment_receipt, subscription_confirmation — **no enforcement template**; `is_transactional` = verification + password_reset only; suppressed transactional send is dropped with `debug!` and returns `Ok` | `services/mail/templates/`; `provider.rs:13-22`; `router.rs:251-259`                 |
| Reply-to                               | provider-level `MAIL_REPLY_TO`, `.env.example:12` = `support@easyquran.fyi`                                                                                                                                                                                                            | `main.rs:51`; `cloudflare.rs:20,66`                                                  |
| Notifications                          | `notify_user` + FCM fan-out works; **zero callers**; `NotificationKind` = NewComment / NewFollower / PaymentReceived / PostPublished / Mention / System                                                                                                                                | `services/notification/mod.rs:41-113`; `enums/notification.rs`                       |
| Scheduler                              | one job, 60 s tick, publishes scheduled posts; plus tower-session sweep hourly                                                                                                                                                                                                         | `services/scheduler.rs:9-25`; `main.rs:761-775`                                      |
| Health                                 | `/healthz` DB ping (Docker-network only, not public); `/quran/health/ready` public                                                                                                                                                                                                     | `router.rs:212-234`; `deploy/README.md:125-126`                                      |
| Roles                                  | SuperAdmin / Admin / **Moderator** / Author / User; Moderator assignable via `admin_update` role string; no route gated at Moderator                                                                                                                                                   | `ruxlog-types/enums/user.rs`; `auth_guard.rs:18-21`                                  |
| Contact                                | `/contact` `mailto:` → personal Gmail; Terms + FAQ publish `salam@` / `corrections@easyquran.app` — **no MX** (verified: `dig MX easyquran.app` empty; `easyquran.fyi` → Cloudflare MX)                                                                                                | `(marketing)/contact/+page.svelte:34`; `site.ts:10`; `web/messages/en.json:152,213`  |
| Legal hold / cases / appeals / reports | none                                                                                                                                                                                                                                                                                   | `migration/src/` stops at m000011; no tables, no routes                              |

**One-line summary of the operational state:** content surfaces are live; the only removal button deletes the
report; the only account button deletes the history; the only intake is a personal mailbox; nothing pages
anyone.

---

## 1. Intake — one channel, one durable store, one alias set

**The rule: the database is the channel; email is the fallback.** A report that exists only in a mailbox has no
SLA clock, no retention, no audit, and no existence after the operator's mail account is compromised. So:

### 1.1 The store (migration `m000012_incident_response.rs`)

```sql
CREATE TABLE reports (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  public_ref    TEXT    NOT NULL UNIQUE,     -- "RPT-<ulid>"; shown to the reporter
  kind          TEXT    NOT NULL,            -- csam|threat|harassment|spam|copyright|impersonation|privacy|other
  severity      TEXT    NOT NULL,            -- p0|p1|p2|p3, derived from kind at intake, human-changeable
  subject_type  TEXT    NOT NULL,            -- comment|post|user|media|other
  subject_id    INTEGER,                     -- nullable: "other" / off-platform
  subject_url   TEXT,                        -- what the reporter saw (<=2048)
  description   TEXT    NOT NULL,            -- the reporter's words
  reporter_email TEXT,                       -- optional; never shown to the subject
  reporter_kind TEXT    NOT NULL,            -- account|anonymous|email
  reporter_user_id INTEGER,                  -- nullable
  reporter_ip_hash TEXT,                     -- HMAC, not raw; for endpoint abuse only, 90 d
  state         TEXT    NOT NULL DEFAULT 'new',   -- new|triaged|actioned|closed|duplicate|rejected
  assigned_to   INTEGER,                     -- user id (operator), no FK
  case_id       TEXT,                        -- links to cases; no FK (self-contained)
  created_at    INTEGER NOT NULL,            -- epoch seconds (repo convention for new columns)
  updated_at    INTEGER NOT NULL,
  closed_at     INTEGER,
  close_reason  TEXT
);
CREATE INDEX idx_reports_state_created ON reports(state, created_at);
CREATE INDEX idx_reports_kind_created  ON reports(kind, created_at);

CREATE TABLE cases (                         -- the case record; survives its subject
  case_id          TEXT PRIMARY KEY,         -- ULID
  report_id        INTEGER,                  -- nullable (drill / self-initiated)
  subject_user_id  INTEGER,                  -- NO FK: must survive user hard-delete
  subject_type     TEXT NOT NULL,
  subject_id       INTEGER,
  level            TEXT NOT NULL,            -- p0|p1|p2|p3
  state            TEXT NOT NULL,            -- open|actioned|appeal|closed
  opened_at        INTEGER NOT NULL,
  closed_at        INTEGER,
  retention_class  TEXT NOT NULL,            -- csam_1y|le_90d90d|enforcement_12mo
  expires_at       INTEGER NOT NULL,
  is_drill         INTEGER NOT NULL DEFAULT 0,
  single_operator_control INTEGER NOT NULL DEFAULT 0,
  notes            TEXT
);

CREATE TABLE case_evidence (                 -- append-only; no UPDATE; no FK that can null/delete
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  case_id       TEXT NOT NULL,
  kind          TEXT NOT NULL,               -- comment_row|flag_row|user_row|session_row|post_view|media_meta|mail_eml|le_request|ncmec_draft|note
  payload_json  TEXT NOT NULL,               -- verbatim copy at capture time
  source_table  TEXT,
  source_id     TEXT,
  captured_at   INTEGER NOT NULL,
  captured_by   INTEGER,
  prev_hash     TEXT,
  record_hash   TEXT
);
CREATE INDEX idx_case_evidence_case ON case_evidence(case_id);
```

`reports` + `cases` + `case_evidence` are the whole ticketing system. No third-party service, no new process.

### 1.2 The endpoints

| Route                                    | Auth                                         | Notes                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /report/v1/submit`                 | none (CSRF via existing `/csrf/v1/generate`) | Rate limit 5/h + 20/day per IP. **Public and unauthenticated** — the most likely CSAM/threat reporter is not a logged-in user. Returns `{ref, message}` uniformly; never echoes case existence.                                                                                                                               |
| `POST /report/v1/status/{ref}`           | none                                         | Optional; uniform response, rate-limited, no case detail beyond state.                                                                                                                                                                                                                                                        |
| `POST /report/v1/admin/queue`            | `ROLE_MODERATOR`                             | The triage list: filters `state`, `kind`, `severity`, `age_hours`; returns reports + comment-flag rows merged, P0/P1 first, oldest first inside a class. Fields per row: `ref`, `kind`, `severity`, `state`, `age`, `subject_url`, `subject_excerpt` (comment text truncated 280), `flags_count`, `reporter_kind`, `case_id`. |
| `POST /report/v1/admin/triage/{id}`      | `ROLE_MODERATOR`                             | claim + set severity/state.                                                                                                                                                                                                                                                                                                   |
| `POST /report/v1/admin/capture/{id}`     | `ROLE_MODERATOR`                             | runs the §3 preserve transaction (idempotent).                                                                                                                                                                                                                                                                                |
| `POST /report/v1/admin/action/{case_id}` | `ROLE_ADMIN`                                 | the §4 one-action flow.                                                                                                                                                                                                                                                                                                       |
| `POST /report/v1/admin/close/{id}`       | `ROLE_MODERATOR`                             | state + `close_reason`; never deletes.                                                                                                                                                                                                                                                                                        |
| `POST /appeal/v1/submit`                 | none (CSRF; **never** behind `not_banned()`) | §4.4.                                                                                                                                                                                                                                                                                                                         |
| `POST /appeal/v1/admin/decide/{id}`      | `ROLE_ADMIN`                                 | §4.4.                                                                                                                                                                                                                                                                                                                         |
| `/report` (web, public)                  | none                                         | Form page: kind picker, subject URL, description, optional email. Linked from footer, contact, Terms. `X-Robots-Tag: noindex` on the page; no analytics.                                                                                                                                                                      |
| Report control on posts                  | none                                         | The existing flag control is comment-only; add the same control on post pages, posting to `/report/v1/submit` with `subject_type=post`.                                                                                                                                                                                       |

### 1.3 The alias set (DNS/Cloudflare Email Routing, not code)

`easyquran.fyi` already uses Cloudflare MX (`route1/2/3.mx.cloudflare.net`). Create four routing rules to the
operator's monitored mailbox, then **test each with a real external send**:

| Address                 | Purpose                                                                   | Published where                               |
| ----------------------- | ------------------------------------------------------------------------- | --------------------------------------------- |
| `abuse@easyquran.fyi`   | reports, threats, illegal content (the canonical address)                 | contact page, footer, `/report`, security.txt |
| `support@easyquran.fyi` | already the mail `reply_to` (`.env.example:12`); general + appeal replies | contact page, notice mails                    |
| `legal@easyquran.fyi`   | law-enforcement process, preservation requests                            | LE guidelines text, notices                   |
| `privacy@easyquran.fyi` | DSAR / rights requests                                                    | privacy policy                                |

**Remove `salam@easyquran.app` and `corrections@easyquran.app` from `web/messages/en.json:152,213`** (dead
addresses) and repoint `corrections@` at the support alias. The Gmail `mailto:` on `/contact` stays as a
last-resort human address but is no longer the canonical one.

### 1.4 Capture-at-intake (the strongest single move)

For `kind IN (csam, threat)`, `POST /report/v1/submit` runs the §3 evidence transaction **in the same
transaction as the report insert**, then auto-quarantines the subject if it is a comment
(`admin_hide`, reversible, row survives). Rationale: the reporter's report is the one moment the evidence is
guaranteed to still exist; every later human step is a chance to destroy it. Abuse cost: a malicious reporter
can force one hidden comment + one evidence copy; bounded by the per-IP caps, the recorded reporter identity,
and `admin_unhide`. `AUTO_QUARANTINE_P0` is a config flag so it can be turned off without a deploy.

---

## 2. Triage — who sees it, in what order, with what SLA

**Who:** the operator (solo). The queue is gated at `ROLE_MODERATOR` so a second human can be added without a
code change; today the operator holds that role too. **Order:** severity class first (P0 → P3), age second.
Nothing is auto-actioned except the P0 quarantine in §1.4. Every state change writes an `audit_logs` row
(first writer).

**The decision tree (realistic cases, in the order the operator will actually meet them):**

1. **CSAM / sexual content involving a minor** → **P0-CSAM.** _Do not open, view, download, or forward the
   material._ Hide it. Preserve metadata (never re-render). NCMEC report ≤24 h. Terminate the account with a
   minimal notice. No public statement. See §5 and §10.
2. **Credible threat of violence / imminent harm** → **P0-threat.** Hide if public and specific. Preserve.
   Check emergency-disclosure conditions (§5.3). Refer to LE. Make a documented decision on warning the
   target — do not silently sit on it.
3. **Harassment / doxxing / targeted abuse** → **P1.** Hide the worst content, warn or restrict the account,
   notice + appeal. Expedite anything involving a minor.
4. **Copyright / IP claim** → **P1-legal.** Preserve, check the claim, respond as a courtesy. Honest gap: no
   DMCA agent is registered (§512 safe harbour requires one) — record it as a named gap, do not pretend.
5. **Impersonation** → **P1.** Verify, then hide/rename or restrict; notice + appeal.
6. **Privacy complaint** (someone posted another person's data) → **P1.** Hide, notice, route to the DSAR path.
7. **Spam / scam / link farming** → **P2.** Hide/delete the content, notice, no ban unless repeated.
8. **"Just annoying" / theological disagreement / translation preference / "I disagree with the tafsir"** →
   **P3 — not abuse.** Reply pointing at `corrections@`/the content-accuracy path. **Never enforce for
   disagreement about religion.** For a Quran app this will be the majority of reports; say so in the runbook.

**SLA (published = what one person can keep):**

| Class     | First action                   | Final             | Notes                                                                        |
| --------- | ------------------------------ | ----------------- | ---------------------------------------------------------------------------- |
| P0-CSAM   | quarantine at intake (seconds) | NCMEC ≤24 h       | "as soon as reasonably possible" (2258A); the clock runs from report receipt |
| P0-threat | quarantine at intake           | LE referral ≤24 h | emergency disclosure is same-day                                             |
| P1        | ≤72 h                          | ≤7 days           | expedite minors/harassment                                                   |
| P2        | ≤7 days                        | ≤14 days          | batch with the weekly review                                                 |
| P3        | ≤14 days or no reply           | —                 | templated reply; no enforcement                                              |

**The published sentence must be true:** "Best effort, single operator. Severe reports (child safety,
credible threats) are actioned same-day when we are reachable. We do not staff a 24/7 desk." Anything stronger
is a false promise and the first thing a complainant quotes back.

---

## 3. Preserve-first — the exact sequence and the SQL

**The three facts that make this mandatory:**

1. `admin_delete` on a comment is `delete_by_id` → the `comment_flags` FK is `ON DELETE CASCADE`
   (`m000001_init.rs:298-308`) → **removing the content deletes the reports on it.**
2. `user_v1::admin_delete` hard-deletes the user and CASCADEs away `user_bans`, `user_sessions`,
   `comment_flags`, `post_comments` → **the only account action destroys the appeal/LE record.**
3. `user_sessions` holds raw IP + raw UA with no expiry, and `post_views` has a live public writer of raw IP +
   UA (`POST /post/v1/track_view/{id}`, unauthenticated); `audit_logs` has zero writers. These are the only
   identity-to-network evidence that exists, and the planned 90-day sweeper (C4 §3) will NULL them.

**Sequence (order is load-bearing):**

```
0. DO NOT press delete. Comment removal is admin_hide until the case closes.
1. Open the case          → INSERT cases (case_id ULID, level, retention_class, expires_at)
2. Capture evidence       → INSERT case_evidence SELECT ... (all in one transaction)
3. Audit the open         → INSERT audit_logs (action='case_open')
4. Hold                   → INSERT legal_holds + legal_hold_subjects (preserve_until)
5. Act (§4)               → hide / ban / notice / appeal token — one transaction
6. Back up                → VACUUM INTO snapshot + off-host encrypted copy (§3.3)
```

**Step 2, SQL-level (the content of `POST /report/v1/admin/capture/{id}`, one transaction):**

```sql
BEGIN IMMEDIATE;

INSERT INTO cases (case_id, report_id, subject_user_id, subject_type, subject_id, level, state,
                   opened_at, retention_class, expires_at, is_drill, single_operator_control)
VALUES (:case_id, :report_id, :uid, :stype, :sid, :level, 'open',
        :now, :class, :expires_at, 0, 1);

-- 1. the comment row verbatim: the only copy that survives admin_delete
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'comment_row',
       json_object('id',id,'post_id',post_id,'user_id',user_id,'content',content,
                   'hidden',hidden,'flags_count',flags_count,'created_at',created_at,'updated_at',updated_at),
       'post_comments', CAST(id AS TEXT), :now, :admin_id
FROM post_comments WHERE id = :comment_id;

-- 2. every report on it (CASCADE-deleted with the comment)
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'flag_row',
       json_object('id',id,'comment_id',comment_id,'user_id',user_id,'reason',reason,'created_at',created_at),
       'comment_flags', CAST(id AS TEXT), :now, :admin_id
FROM comment_flags WHERE comment_id = :comment_id;

-- 3. the author account row (CASCADE-deleted with the user)
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'user_row',
       json_object('id',id,'name',name,'email',email,'role',role,'is_verified',is_verified,'created_at',created_at),
       'users', CAST(id AS TEXT), :now, :admin_id
FROM users WHERE id = :uid;

-- 4. network evidence: raw IP + raw UA per session (no retention today; the planned sweep would NULL these)
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'session_row',
       json_object('id',id,'device',device,'ip_address',ip_address,'last_seen',last_seen,'revoked_at',revoked_at),
       'user_sessions', CAST(id AS TEXT), :now, :admin_id
FROM user_sessions WHERE user_id = :uid;

-- 5. reader-view rows for the subject content (raw IP+UA; live writer via /post/v1/track_view)
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'post_view',
       json_object('id',id,'ip_address',ip_address,'user_agent',user_agent,'user_id',user_id,'created_at',created_at),
       'post_views', CAST(id AS TEXT), :now, :admin_id
FROM post_views WHERE post_id = :post_id;

-- 6. media: metadata only, never a re-render or a download
INSERT INTO case_evidence (case_id, kind, payload_json, source_table, source_id, captured_at, captured_by)
SELECT :case_id, 'media_meta',
       json_object('id',id,'object_key',object_key,'bucket',bucket,'mime_type',mime_type,'size',size,
                   'content_hash',content_hash,'created_at',created_at),
       'media', CAST(id AS TEXT), :now, :admin_id
FROM media WHERE id = :media_id;

-- 7. the operator action record — audit_logs gets its first writer
INSERT INTO audit_logs (user_id, action, resource_type, resource_id, metadata, created_at)
VALUES (:admin_id, 'case_open', 'case', :case_id,
        json_object('report_id', :report_id, 'level', :level, 'retention_class', :class), :now);

-- 8. the hold (see §5 for class → preserve_until)
INSERT INTO legal_holds (case_id, authority, request_type, scope_json, received_at, preserve_until, status)
VALUES (:case_id, :authority, :rtype, :scope_json, :now, :preserve_until, 'active');
-- last_insert_rowid() → :hold_id
INSERT INTO legal_hold_subjects (hold_id, subject_kind, subject_id, expires_at)
VALUES (:hold_id, 'user', CAST(:uid AS TEXT), :preserve_until),
       (:hold_id, 'case', :case_id, :preserve_until);

COMMIT;
```

Notes:

- `case_evidence` is **append-only**: no UPDATE anywhere; corrections are new rows with a `note` kind. Rows are
  self-contained (no FK to `users`/`post_comments`) so a later hard delete cannot orphan them.
- Timestamps on new columns are **INTEGER epoch seconds** (the `rate_limit_state`/`sessions` convention), not
  the TEXT/RFC3339 mix the old tables carry — a cross-format comparison in SQLite is silently false (C4 §3 note 1).
- The `media_meta` copy records the object key + hash; it does **not** re-fetch or re-encode the object. If the
  object itself must be preserved, that is a hold on the storage key, decided per §5/§10 — never an operator
  download.
- `json_object` requires SQLite ≥3.38 (built-in JSON). If the bundled version is older, the same rows are
  serialized in Rust before insert — the shape does not change.

**Step 6, the backup.** There is no backup anywhere today (`deploy/README.md` has no backup section; B8 §7).
A case open during a bad migration is lost. Minimum after any P0/P1: `VACUUM INTO` a dedicated maintenance
connection in a quiet window, `PRAGMA integrity_check` the copy, copy off-host encrypted, keep 7 daily + 4
weekly with a hard 30-day cap (B8 §7.2). This is the only protection the case has until the retention/backup
workstream lands.

**The rule the runbook repeats three times:** _no destructive action precedes capture._

- `admin_delete` on a comment with flags or an open case must be refused (or converted to `admin_hide`) until
  capture exists. Minimum v1 guard: `admin_delete` refuses when `flags_count > 0` unless
  `confirm_evidence=true`; a log line records it either way.
- `admin_flags_clear` sets `flags_count = 0` while the flag rows survive (`post_comment/actions.rs:389-400`) —
  the queue's count signal is destroyed even though the rows are not. Make it write a `cleared_at` marker (or
  capture first). The dead `comment_flag::Entity::clear_flags` (`delete_many`, zero callers) is a trap for the
  next caller — delete it or make it unreachable.
- `user_v1::admin_delete` is forbidden for any subject with an open case. The ban path (§4) is the action to
  use; delete is for the closed, uncontested, post-retention case only.

---

## 4. Act — ban, notice, appeal in one flow

### 4.1 What must be added to `user_bans` (nothing to invent)

The entity functions already exist: `create`, `revoke`, `get_active_ban`, `is_banned`, `list_by_user`,
`admin_list` (`user_ban/actions.rs:11-145`). What is missing is the **admin route + controller** and the
coupling to the notice:

- `POST /user/v1/admin/bans` — create (body: `user_id`, `reason_code`, `reason_text`, `duration_days|null`,
  `case_id`). Calls `Entity::create`, then writes `enforcement_notices` + mints the appeal token **in the same
  transaction**. A ban without a notice is the Art 17 violation, so they are one action.
- `POST /user/v1/admin/bans/list` — calls `Entity::admin_list` (already paginated/filterable).
- `POST /user/v1/admin/bans/revoke/{ban_id}` — calls `Entity::revoke`; on appeal reversal this is step one.
- Add an index on `user_bans.user_id` in the same migration (the table has none).
- On create, **revoke the subject's sessions** (the existing session-revocation primitive is used by the admin
  password path) so the ban is effective immediately rather than at the next `not_banned()` cache expiry.

### 4.2 The one-action flow

`POST /report/v1/admin/action/{case_id}` body: `{decision: hide|suspend|terminate, duration_days, reason_code,
reason_text, scope: content|account|service}`. One transaction writes:

1. `user_bans` row (suspend/terminate) or `post_comments.hidden=1` (hide) — never `admin_delete` while the case is open.
2. `enforcement_notices` row (below) with `reason_code`, `reason_text`, `facts` (plain language, categories not
   weights), `automated_means=0`, `contract_ground='terms:acceptable-use'`, `decided_by`, `decided_at`.
3. The appeal token: 32 random bytes, base64url, stored `hmac_sha256(server_key, token)`; `appeal_window_ends_at = now + 6 months`.
4. `audit_logs` rows (`action='enforcement_action'`, `resource_type='case'`, `resource_id=case_id`).
5. Notification + mail dispatch (below).

```
CREATE TABLE enforcement_notices (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  case_id TEXT,
  subject_type TEXT NOT NULL, subject_id INTEGER NOT NULL,
  decision TEXT NOT NULL, scope TEXT NOT NULL,
  reason_code TEXT NOT NULL, reason_text TEXT NOT NULL, facts TEXT NOT NULL,
  starts_at INTEGER NOT NULL, ends_at INTEGER,              -- NULL = permanent
  automated_means INTEGER NOT NULL DEFAULT 0,
  contract_ground TEXT NOT NULL,
  decided_by INTEGER, decided_at INTEGER NOT NULL,
  second_reviewer_id INTEGER,
  notice_channel TEXT, notice_sent_at INTEGER,
  notice_delivery_state TEXT,        -- sent|suppressed|no_contact_details|pending
  appeal_token_hash TEXT,
  appeal_window_ends_at INTEGER,
  retention_class TEXT NOT NULL DEFAULT 'enforcement_12mo'
);

CREATE TABLE appeals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  notice_id INTEGER NOT NULL, case_id TEXT,
  token_hash TEXT NOT NULL,
  submitted_at INTEGER NOT NULL,
  body TEXT NOT NULL, shared_device_claim TEXT, contact TEXT,
  state TEXT NOT NULL DEFAULT 'new',    -- new|in_review|decided
  sla_due_at INTEGER NOT NULL,          -- submitted_at + 7 days
  reviewer_id INTEGER, decision TEXT, decided_at INTEGER, reason_code TEXT
);
```

### 4.3 The notice channel (all three, because each can fail)

- **Transactional mail**: add `TEMPLATE_ENFORCEMENT_NOTICE = "enforcement_notice"` and include it in
  `is_transactional` (`provider.rs:17-22`), plus a Tera template in `services/mail/templates/`. The body is C4
  §4.2's template verbatim. Set the reply-to to `support@easyquran.fyi` (the provider already supports a
  reply-to; today it is a single static value, so either set it globally or extend `OutboundEmail`).
- **Delivery state**: a suppressed address must be recorded, not silently dropped. Today a suppressed
  transactional send logs `debug!` and returns `Ok(SendReceipt::default())` (`router.rs:251-259`) — the caller
  cannot tell. Change the enforcement path to write `notice_delivery_state='suppressed'`; that is the DSA Art 17
  "where contact details are known" conditional, recorded rather than assumed.
- **In-app**: add `NotificationKind::Enforcement` and call the existing `notify_user` (zero callers today; the
  FCM fan-out works). This is also the operator's own alert path (§6).
- **The public appeal URL**: `https://easyquran.fyi/appeal/<token>` — works while suspended, no auth, no
  analytics, `X-Robots-Tag: noindex`, `Referrer-Policy: no-referrer`. A suspended user cannot sign in, so this
  page is the only durable notice.

### 4.4 The appeal

`POST /appeal/v1/submit` (public, CSRF, rate-limited per token and per IP, uniform responses) writes `appeals`.
`POST /appeal/v1/admin/decide/{id}` records `decision` + `reason_code`; a reversal calls `Entity::revoke` and
sends the reversal notice. **The reviewer is the same person** — the API records `single_operator_control=1`
on the case and `second_reviewer_id=NULL`; the notice says the review was single-operator. When a second human
exists, the queue is already `ROLE_MODERATOR` and the decide route is `ROLE_ADMIN`: the separation exists in
the ACL, unused.

---

## 5. Escalate — NCMEC, §2703(f), and the honest limits

### 5.1 Retention classes and calendar

| Class              | Trigger                                      | `preserve_until`                     | Basis                                                                                  |
| ------------------ | -------------------------------------------- | ------------------------------------ | -------------------------------------------------------------------------------------- |
| `csam_1y`          | P0-CSAM                                      | report + 1 year, do not auto-release | 18 U.S.C. §2258A(h) — a completed CyberTipline report is a 1-year preservation request |
| `le_90d90d`        | P0-threat / LE preservation request          | request + 90 days, renewable +90     | 18 U.S.C. §2703(f)                                                                     |
| `enforcement_12mo` | P1/P2/P3, bans, notices, appeals, audit logs | 12 months                            | GDPR Art 5(1)(e) + Art 17(3)(e) for live disputes                                      |

The mechanism is `legal_holds.preserve_until` + `legal_hold_subjects.expires_at` (denormalized so a future
sweeper needs no join). **The repo cannot send a reminder** — so: the daily digest (§6) lists "holds expiring
in 14 days" and the operator calendars the renewal date. A hold is never auto-released; a lapsed hold alerts.

**Sequencing constraint (finding F7):** the first retention sweeper (C4 D8 / B8 §1) must ship **with** the
`NOT EXISTS legal_hold_subjects` probe in every delete, or it becomes an evidence-destruction mechanism. Until
the sweeper exists, preservation is trivially true (nothing deletes) — do not ship the sweeper without the hold.

### 5.2 NCMEC report (US 18 U.S.C. §2258A) — short template

Report via `report.cybertip.org` (the CyberTipline form). Keep this text ready:

> **Reporter:** EasyQuran (easyquran.fyi), operator contact legal@easyquran.fyi. **Incident ref:** {case_id}.
> On {UTC date/time} an account ({user_id}, {email if known}, created {date}) posted {text|image|link} at
> {URL}. We removed it from public view at {time} and preserved the records below for one year per 18 U.S.C.
> §2258A(h). We have not viewed or downloaded the material. Account action: {terminated|suspended}.
> Preserved: content row, report rows, account row, session rows (IP/user-agent, timestamps), media object key
>
> - hash. **This is a provider report under 18 U.S.C. §2258A.** We will respond to follow-up at the address above.

Statutory points to keep straight: report **as soon as reasonably possible**; the report itself is a 1-year
preservation request; preserved material must be **secure, access-limited, and consistent with NIST's CSF**;
knowing/willful failure carries fines up to **$600k first / $850k subsequent** for small providers. Do not
wait for a preservation letter to report.

### 5.3 §2703(f) preservation letter — short template

Sent _to_ the provider (the operator), received at `legal@easyquran.fyi`:

> Re: Preservation request pursuant to 18 U.S.C. §2703(f).
> Please preserve all records and other evidence in your possession pertaining to {account/identifier} for
> {date range}, pending the issuance of a court order or other process. Records: registration data; session
> records (IP address, user agent, timestamps); enforcement and ban records; authored content; reports and
> case records. Preserve for 90 days from receipt; we will notify you if an extension is required.
> **This is a preservation request only and does not authorize disclosure of the contents of communications.**
> {name, title, agency, published callback number}

Operator's response: verify the sender against the agency's published contact (call the published number, not
the number in the email); classify (preservation / subpoena / warrant / emergency); freeze via §3's hold;
acknowledge in writing with the case ref and window end; **disclose nothing** without process; log every
disclosure in `audit_logs` (`action='legal_disclosure'`); calendar the renewal. **Emergency disclosure** is
permitted only under §2702(b)(8)/(c)(4) (imminent danger of death or serious injury): verify the officer,
disclose the minimum (account email, last IP, session times), log it.

### 5.4 Honest limits for a solo operator (put these in the LE guidelines text, verbatim in spirit)

- **No 24/7 desk.** "We respond to urgent legal process as fast as we can, typically within 24 hours."
- **No MLAT, no foreign-process verification.** Foreign agencies route through the US DOJ/MLAT or their
  national hotline; we honour §2703(f) preservation from any agency but do not disclose to foreign LE without
  US process.
- **If the operator is outside the US**, §2258A's provider duty may not bind him directly and local possession
  law may differ. The defensible default is unchanged: report apparent CSAM to NCMEC (it accepts reports from
  anywhere) and to the national hotline (INHOPE member) if one exists; preserve the minimum (identifiers,
  URL, hash — not necessarily the bytes) and get local legal advice before implementing any byte-preservation
  path (§10).
- **A lawyer is not on retainer.** Action: identify 2–3 internet/criminal-defense lawyers, record names + fees
  in the runbook, retain only when process arrives. Do not claim counsel in the policy.
- **No DMCA agent registered** — the copyright path is courtesy-only until one is (§2 case 4).

---

## 6. Alert — minimum viable for one person

There is no pager, no on-call, no metrics dashboard, and stdout is the only log sink (`RUST_LOG=info`,
Dokploy log view). The design uses what exists:

1. **Intake → push + mail.** `POST /report/v1/submit` with `kind IN (csam, threat)` sends: (a) a mail to
   `abuse@` (Cloudflare routing → the operator's phone mail app), and (b) an FCM push to the operator's own
   account via the existing `notify_user` + `devices` + `rux-fcm` stack — the primitive that exists with zero
   callers today. This is the 3 a.m. path, and it is the only real-time one.
2. **Daily digest at 08:00** (the operator's timezone), from the existing scheduler tick + mailer: new reports,
   open cases by age, **oldest P0/P1 age**, appeals due in 48 h, **holds expiring in 14 days**, 429 count for
   the last 24 h, `/healthz` DB status. If the digest mail fails, the failure is logged and visible in Dokploy.
3. **External uptime monitor** (free tier) on the public `https://easyquran.fyi/api/quran/health/ready` every
   5 min → email/SMS. This covers "the site is down", not abuse — but it is the only thing that notices a dead
   API at 3 a.m.
4. **Promote the 429 log from `debug!` to `warn!`** (C3 F4, one word) so limiter activity is visible in the
   Dokploy log stream forever. This is the cheapest abuse sensor the repo can have.
5. **The queue endpoint** (`POST /report/v1/admin/queue`) is the glance; a `just incident queue` recipe wraps it.
6. **What the design does _not_ do:** it does not promise a page in <5 minutes. The honest statement is: the
   push/mail is best-effort; the P0 content quarantine is automatic and does not depend on the operator being
   awake; the human actions wait for the operator. The one mitigation for the gap: the quarantine.

**A deliberate v1 cut:** no web admin console (A8 §9 cut it for good reasons). The 3 a.m. tool is the phone
mail app + an SSH client (Termius) running `just incident` recipes. The single mobile-friendly queue page is
the first v2 addition if the volume ever justifies it.

---

## 7. Drill — the quarterly script that makes this "tested, owned"

**Pre:** quiet hour; laptop + phone with mail; the runbook doc open; a non-admin test account and a second
account for reporting; a published post to comment on. Do not use real names in the synthetic threat.

| #   | Step                                                                                                                                                          | Expected outcome                                                                                                                                                      | Evidence to keep            |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| 1   | T-0: account A posts `[DRILL 2026-Q4] I will hurt <alias> tomorrow at <place>` on a published post                                                            | comment visible                                                                                                                                                       | comment id                  |
| 2   | T-0: account B submits `/report` (kind=threat, URL, description). Record wall-clock                                                                           | report accepted, `ref` returned                                                                                                                                       | `ref`, timestamp            |
| 3   | Measure **time-to-detect**: phone mail + push arrival                                                                                                         | ≤5 min                                                                                                                                                                | arrival times               |
| 4   | T-0+5: open the queue (`just incident queue`)                                                                                                                 | report is P0, subject auto-quarantined (`hidden=1`), `case_id` present                                                                                                | queue output                |
| 5   | Verify capture: `case_evidence` has comment_row, flag_row, user_row, session_row; `audit_logs` has `case_open`; `legal_holds` has a row with `preserve_until` | all present, before any delete                                                                                                                                        | row counts                  |
| 6   | T-0+15: run `POST /report/v1/admin/action/{case_id}` → suspend 7 days                                                                                         | `user_bans` row, `enforcement_notices` row `notice_delivery_state='sent'`, mail arrives, in-app notification row exists, account A blocked within 5 min (try to post) | mail `.eml`, `user_bans` id |
| 7   | T-0+25: open `/appeal/<token>` in a logged-out private window; submit                                                                                         | page loads (no auth), appeal row created, ack mail arrives                                                                                                            | screenshot, appeal id       |
| 8   | T-0+40: decide the appeal → reverse                                                                                                                           | `user_bans.revoked_at` set, reversal notice sent, account A can post again ≤5 min                                                                                     | decision row                |
| 9   | T-0+50: escalation dry run — fill the §2703(f) letter and the NCMEC text with the drill case (**do not submit**)                                              | both texts complete from the runbook alone                                                                                                                            | the two drafts              |
| 10  | T-0+60: `VACUUM INTO` snapshot + `integrity_check` + off-host copy                                                                                            | copy ok, integrity ok                                                                                                                                                 | command output              |
| 11  | T-0+75 (quarterly, alternate with 10 if short): restore the copy into a scratch DB, replay erasure receipts, re-apply holds                                   | row counts match, holds active                                                                                                                                        | restore log                 |
| 12  | T-0+90: write the drill log                                                                                                                                   | `cases` row `is_drill=1`, timings, failures filed as issues, `docs/incident-response.md` updated                                                                      | log entry                   |

**Pass criteria:** every step within its SLA; **zero steps require undocumented knowledge** (the runbook alone
was sufficient); the notice arrived; the appeal worked; the hold existed before any delete. Any step that
required guessing fails the drill and the runbook is fixed before the next one.

**Evidence retention:** drill log + timings + the mail `.eml` + the appeal screenshot + the restore output,
12 months, in the same `case_evidence` store (`is_drill=1`).

**Cadence:** quarterly, in the calendar, with the owner's name on it. An undrilled runbook is a document, not a
capability — the drill is the only thing that separates this design from the 35 reports it summarises.

---

## 8. Ownership + budget

| Role                          | Person           | Decision rights                                     | Notes                                                                                        |
| ----------------------------- | ---------------- | --------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Incident commander / operator | hmziqrs          | all enforcement, all NCMEC/LE, all disclosures      | sole                                                                                         |
| Reviewer (appeals)            | hmziqrs          | uphold/reverse; records `single_operator_control=1` | second reviewer = `ROLE_MODERATOR` when one exists; the ACL is ready                         |
| Escalation contact            | _does not exist_ | —                                                   | action: identify 2–3 lawyers, names + fees in the runbook; no retainer until process arrives |
| Platform (VPS/DNS/mail)       | hmziqrs          | —                                                   | Dokploy, Cloudflare, mail aliases                                                            |

**Time budget:** steady state 15 min/day (queue + digest) + 15 min/week (appeals, holds, 429s) ≈ 2.5 h/month;
P0 4–8 h day-of + 2 h follow-up; P1 ≈ 2 h; P2 ≈ 30 min; drill 2 h/quarter. **Build: 4–5 dev-days for the
irreducible core** (§11) — not the 12–14 days of the full design.

**Kill criterion (falsifiable, so this does not become another 65-day programme):** if after one quarter the
queue has had zero reports and zero flags, the core stays (it is the precondition for the retention work) but
the v2 items — case ladder, evidence hash chain, queue web page, metrics endpoint — stay unbuilt until the
first non-zero counter. Write the measurement date in the runbook.

---

## 9. Attack 1 — what fails at 3 a.m., on holiday, and at a dead address

**3 a.m., one person, no pager.**

- The chain is report → mail to `abuse@` → Cloudflare routing → the operator's mail app notification. If the
  phone is on DND, the app is signed out, the routing rule is misconfigured, or the operator is asleep, nothing
  fires until the morning. The FCM push is a second path but iOS FCM delivery is best-effort.
- **What still works:** the automatic P0 quarantine. The illegal content is out of public view within seconds of
  the report, with no human in the loop. That is the part of the 3 a.m. problem that _can_ be automated, and it is.
- **What waits:** ban, notice, NCMEC, LE. The 2258A clock ("as soon as reasonably possible") starts at receipt;
  an 8-hour sleep plus a workday is tight but defensible; a flight or illness is not. The runbook's rule: if you
  cannot complete the report within 24 h, submit what you have and amend — do not let the deadline pass in silence.
- **What fails hardest:** if the _DB_ is down or the API is down, the report form fails and only the mail alias
  works. That is why both exist, and why the mail copy is not redundant.

**Holiday / one person away.**

- No redundancy. Mitigations, in order of honesty: (a) an availability state in the runbook (`available | away
until <date>`) with an auto-reply on `abuse@` that says what is happening and what to do in an emergency
  (call local police) — a true statement, not a promise; (b) a named backup human with `ROLE_MODERATOR` and a
  one-page card — but **the backup must only be able to hide content, never view illegal material**, which is
  itself a reason to keep the backup's access to the queue at `ROLE_MODERATOR` and the material out of any UI;
  (c) accept and document the gap. Do not promise 24/7 in the Terms.
- The digest keeps arriving (the host runs it), so the queue does not silently grow — the _detection_ survives
  a holiday; the _response_ does not.

**A report to a dead address (D1 confirmed `easyquran.app` has no MX).**

- A user who follows the Terms gets a bounce and may stop there. The fix is trivial and must ship first:
  replace the `.app` addresses, create the four `.fyi` aliases, and **test each with a real external send every
  quarter**. The mail `reply_to` is already `support@easyquran.fyi` (`.env.example:12`) — verify that mailbox
  actually routes before relying on it.
- Second failure mode: a report that arrives _only_ by email has no case row until the operator creates one.
  The runbook's first action for any email report is `just incident open --from-email <eml>` which creates the
  report + case + capture. An email-only report is the most likely path for a non-user reporter.

**One more 3 a.m. failure the design must own:** the operator cannot act without a laptop (no admin UI). The
quarantine is automatic, but the ban/notice requires the API. Honest v1 answer: SSH from the phone. The queue
web page is the v2 fix and is deliberately cut from the core (§11).

---

## 10. Attack 2 — the legal exposure of preserve-first (the honest answer)

**Retaining illegal content.** For CSAM, US law _requires_ preservation (2258A(h): 1 year, secure,
access-limited, NIST-CSF-consistent) — the risk is not retention but handling: no copies, no re-rendering, no
sharing, access-logged, minimal staff. For other illegal content (threats, fraud, terrorism material) there is
no general preservation statute; retention must rest on Art 6(1)(f) + Art 17(3)(e), be scoped, and be
time-bound — which is exactly what `legal_holds.preserve_until` enforces.

**GDPR vs the obligation to preserve.** Storage limitation (Art 5(1)(e)) is not violated by a _documented,
necessary, time-bound_ preservation. The bases are Art 6(1)(c) (legal obligation) and Art 6(1)(f)/17(3)(e)
(defence of legal claims). The audit's own rule applies: **no category rule, per-matter hold** (R2-legal-privacy
§3/§8). The design complies. Residual risks to handle explicitly:

- the **reporter's** email/IP is personal data — hash the IP, drop the email after the case closes unless the
  case is open, and disclose the category in the policy;
- a **DSAR from the subject** must not expose the reporter or another person's data — the bundle needs
  redaction rules (A8 §3(b)'s missing DSAR endpoint is a real prerequisite, not a nicety);
- a **comment about religion** is potential Art 9 data — minimization applies to the evidence copy too.

**The operator viewing CSAM — the honest answer: do not view.**

- Text content: reading a text comment is not a depiction. Triage of text (threats, harassment, spam) is done
  by reading, and that is what the job is.
- Image/video: **do not open, download, render, or forward.** Rely on (a) the reporter's description, (b) the
  moderator provider's verdict _if one is enabled_ — and note that today none is (finding F5), (c) hashes if
  available. Preserve the object key + hash + metadata; the bytes stay in storage under the hold, not on the
  operator's screen or disk.
- If the operator has already seen it: stop; make no copy; write a case note ("operator viewed the item at
  03:12 during triage; no copy made"); report. There is no immunity for viewing, but the context that matters
  to a prosecutor/regulator is _acting in the course of a mandatory report, with minimal access_ — the reckless
  act is downloading and keeping it outside the process. The runbook's hard rule for `kind=csam` is:
  **hide → preserve metadata → report. Never open.**
- If the operator is outside the US, local possession law may have no provider safe harbour. Then the correct
  move is: report to NCMEC + the national hotline, preserve **identifiers and metadata only** (not necessarily
  the bytes), and take local legal advice before implementing a byte-preservation path. Say this in the doc
  rather than assuming the US framework.
- The **notice in a CSAM case** must not reveal the reporter or the detection method — use a minimal
  termination notice citing the Terms, and treat the appeal as constrained by law (R2's L7).

**The risk of _not_ preserving.** A preservation letter that arrives after the content was deleted in the
ordinary course is a spoliation/obstruction argument. The defence is the documented retention policy + the
hold system + the fact that the deletion was policy-driven, not evidence-driven. Preserve-first is what makes
that defence available; it is not sentimentality about data.

---

## 11. Attack 3 — the irreducible core vs the theatre (cutting my own design)

**The irreducible core (4–5 dev-days, ~1 day of it the runbook + drill):**

1. **Intake** (1.5 d): the four aliases routed + tested; `reports` table; public `POST /report/v1/submit`
   (CSRF, rate-limited, capture-at-intake for P0); the `/report` page; the dead `.app` addresses fixed; the
   report control on posts.
2. **Preserve** (0.5 d): `cases` + `case_evidence`; the capture transaction; the `just incident capture` CLI for
   email-arrived reports; the `admin_delete`/`flags_clear` guard.
3. **Act** (1.5 d): `/user/v1/admin/bans` create/list/revoke on the existing entity; `enforcement_notices` +
   template + transactional class + `NotificationKind::Enforcement`; the appeal token + public page +
   `POST /appeal/v1/submit` + `appeals` table; the one-action endpoint.
4. **Escalate** (0.5 d): `legal_holds` + `legal_hold_subjects`; the templates in the runbook; the retention
   classes.
5. **Alert** (0.5 d): intake mail + FCM push to the operator; the daily digest; the external uptime monitor;
   the 429 `warn!`.
6. **Drill** (1 d): `docs/incident-response.md` (runbook + log); one drill run end-to-end.

**Cut, with reasons:**

- `enforcement_cases` full ladder, auto-scoring, second-look gate, auto-reversal → wait for a non-zero numerator (C3).
- Evidence **hash chain** → plain append-only + timestamps for v1; a chain is for contested disputes and there are none.
- Device-layer holds (`subject_kind='device_record'`) → no device layer shipped; add the kind when it ships.
- Full metrics dashboard → the digest counts are enough; A8 §5's dashboard is 15 min/week once there is data.
- Web admin console → the `just` CLI is the 3 a.m. tool; the single queue page is the first v2 item.
- DMCA agent registration → legal work, not code; named gap.
- 24/7 pager / on-call rotation → theatre at this size; a phone + push is what exists.
- Statement-of-reasons records for _every_ hide → required for account actions and contestable content actions;
  v1 makes the reason field optional for a plain hide.
- The retention sweeper (C4 D8) → **not cut from the programme**, but sequenced: when it ships it must carry
  the `NOT EXISTS legal_hold_subjects` probe. Until then preservation is trivially true.

**Theatre to name and refuse:** "we report to NCMEC" without a drill; a legal-hold table with no sweeper (false
assurance — worse than no table, because it looks like a control); an appeal page with no staffing behind it;
"24/7" language; a second reviewer who does not exist; auto-bans.

**The single sentence that defines defensible vs theatre here:** _the minimum is "a report survives, the
evidence survives, the action is recorded, the subject is told, and one person has run the whole chain once."_
Everything else is optimisation for a volume that does not exist yet.

---

## 12. Attack 4 — the chicken-and-egg: is this premature?

**The case for "premature."** C3 measured zero: zero comments, zero flags, zero bans, zero suppressions, zero
audit rows, production request-to-session ratio normal. The probability of a CSAM report this quarter is very
low. A case system for zero cases is exactly the error D5 names elsewhere ("optimises a control for a threat
with no measured base rate"), and the same 4–5 days are owed to the retention/DSAR work that is _already_
non-compliant (Art 5(1)(e) on `user_sessions`; the Art 17 notice on the live hide/delete). An untested
practice cannot be tuned; you will rewrite it after the first real case anyway.

**The case for "not premature."**

1. **The failure is irreversible.** The only removal button destroys the reports on the content; the only
   account button destroys the ban/session/history. The first incident, handled by instinct, will delete the
   evidence — that is the documented behaviour of every platform that has no preserve-first step.
2. **The live compliance debt is not conditional on a base rate.** DSA Art 17 notice attaches to the hide/delete
   that is live today, and the Terms publish a dead address today. Those ship regardless of the incident design.
3. **The evidence window is open _now_ only because nothing deletes.** The moment C4's `user_sessions` sweeper
   ships without a hold, the IP/UA an LE request needs is NULLed at 90 days. Building the hold **before** the
   sweeper is the correct order, and that order only exists now.
4. **The flag button is live.** The first report can arrive tonight. The cost asymmetry is stark: 4–5 days
   versus a $600k-class statutory failure or a person harmed while a report sits in a dead mailbox.
5. **The 4–5 days are the entry ticket, not a competing workstream.** The retention work needs the hold table;
   the notice work needs the enforcement template; the alert work needs the digest. The core is the foundation
   the rest of the shipped-debt list stands on.

**Decision: build the irreducible core now; everything else waits for the first non-zero counter.**
What must be true _before_ this is needed — the precondition list the design itself creates:

| #   | Precondition                                                              | Today                                                    |
| --- | ------------------------------------------------------------------------- | -------------------------------------------------------- |
| 1   | A published address that does not bounce, tested quarterly                | **false** (`.app` has no MX)                             |
| 2   | A queue that exists outside a personal mailbox                            | **false** (Gmail mailto only)                            |
| 3   | A way to remove content without destroying the report                     | **false** (delete CASCADEs the flags)                    |
| 4   | A way to restrict an account that preserves its history                   | **false** (only hard delete)                             |
| 5   | A notice that reaches a banned user (and a recorded state when it cannot) | **false** (no template; suppression silent)              |
| 6   | A hold that exists before the first deletion job                          | **false** (no hold, no sweeper — order is still fixable) |
| 7   | An owner and one completed drill                                          | **false**                                                |

The question "is it premature?" is malformed. The capability is not a bet on a threat; it is the precondition
for controls that are already planned and partly owed. Build the core, write the drill date, and let the first
non-zero counter decide everything else.

---

## 13. The runbook (one page — this is the artefact `D5` §4 asks for)

```
EASYQURAN INCIDENT RESPONSE — v1 (owner: hmziqrs; review: quarterly)
Addresses: abuse@ / support@ / legal@ / privacy@easyquran.fyi  (test sends each quarter)

1. RECEIVE.  Form report → DB row (auto).  Email → `just incident open --from-email <eml>`.
2. CLASSIFY. CSAM→P0  threat→P0  harassment/impersonation/privacy→P1  copyright→P1  spam→P2  disagreement→P3.
3. PRESERVE (before any removal).  `just incident capture <ref>` → cases + case_evidence + audit row + hold.
   NEVER: admin_delete, flags_clear, user delete, media download.
4. QUARANTINE.  P0 comments auto-hidden at intake.  Anything else: admin_hide (never delete).
5. ACT.  `just incident action <case_id> suspend|terminate <days> <reason_code>` → ban + notice + token + audit.
6. ESCALATE.  CSAM → NCMEC (24h, template in §5.2).  Threat → LE + emergency-disclosure check (§5.3).
7. CLOSE.  state + close_reason.  Holds keep the retention.  No deletion before `expires_at`.
8. RECORD.  Every step writes audit_logs.  The digest at 08:00 shows overdue P0/P1 and expiring holds.
9. BACKUP.  After P0/P1: VACUUM INTO + off-host copy (B8 §7.2).
10. REVIEW.  Quarterly drill (§7).  Fail → fix the runbook, not the drill.
```

---

## 14. Findings (all NEW relative to the 35 prior reports)

**F1 — Every removal/triage action available today destroys the report evidence. NEW. High.**
Evidence: `post_comment/actions.rs:384-387` `admin_delete` = `delete_by_id`; `m000001_init.rs:298-308`
`comment_flags` FK `ON DELETE CASCADE` → deleting a comment deletes the reports on it. `admin_flags_clear`
(`post_comment/actions.rs:389-400`) zeroes `flags_count` while the flag rows survive, so the queue's count
signal is destroyed even when the rows are not; the dead `comment_flag::Entity::clear_flags`
(`delete_many`, `comment_flag/actions.rs:179-186`, **zero callers**) is a trap for the next caller. D5/C4/A8
describe the missing notice/audit/hold; none states the CASCADE-destroys-the-report mechanism or the dead
delete helper.
Fix: capture-at-intake + capture-before-action (§3); make `admin_delete` refuse when `flags_count>0` or a case
is open unless `confirm_evidence=true`; make `admin_flags_clear` write a `cleared_at` marker; delete or guard
`clear_flags`. A test asserts the flag rows survive an `admin_delete` when a case is open.

**F2 — `user_v1::admin_delete` is the only account action, it is irreversible, and it CASCADEs away the ban,
session, and report history an appeal or LE request needs. NEW. High.**
Evidence: `user_v1/controller.rs:269-318`; `user_v1/mod.rs:29`; FKs `ON DELETE CASCADE` on `user_sessions`,
`user_bans`, `post_comments`, `comment_flags` (`m000001_init.rs:48-73, 283-308`). D5 item 10 states the hard
delete + no audit row; the new fact is the operational consequence: with no restrict action that preserves,
the operator's only "act" destroys the record, and the appeal is impossible after the fact.
Fix: build the ban path (§4) before anyone uses delete; forbid `admin_delete` while a case is open; soft-delete
window; write `audit_logs` from the delete path.

**F3 — `post_views` has a live public writer of raw IP + raw UA, contradicting the retention plan's
"seed-only writers". NEW. High.**
Evidence: `post_v1/mod.rs:63-67` puts `/track_view/{post_id}` on the **public** (no-auth) router;
`post_v1/controller.rs:346-386` reads `ClientIp` + the `User-Agent` header and calls
`post/actions.rs:608-647`, which inserts `ip_address` + `user_agent` into `post_views`
(`m000001_init.rs:165-176`). C4 §3 line 162 records the intended rule: "seed-only writers — **never enable** for
live traffic". The web client does not call it (grep: 0 hits), but any client can.
Fix: drop the IP/UA from the insert (the product need is a view count), or hash the IP + coarsen the UA; add
`post_views` to the retention schedule; rate-limit the route.

**F4 — The only report queue cannot represent "triaged": `comment_flags` has no resolution state, no assignee,
no aging. NEW. Medium-High.**
Evidence: `comment_flag/model.rs` (id, comment_id, user_id, reason, created_at only); `admin_flags_list` sorts
by `created_at` with no state filter (`controller.rs:389-421`); `admin_flags_summary` returns a count
(`comment_flag/actions.rs:150-160`). A cleared flag is indistinguishable from a never-seen one.
Fix: `state`, `resolved_at`, `resolved_by`, `resolution_code` on `comment_flags` (or fold flags into `reports`);
triage writes state, never deletes; the queue endpoint filters `state='new'`.

**F5 — The image moderator is not a no-op moderator; the shipped default is no moderator at all, so uploads are
unmoderated and the CSAM path has no detector. NEW. High (correction).**
Evidence: `main.rs:499` `IMAGE_MODERATION_ENABLED` defaults **false** → `state.storage.image_moderator = None`
(`main.rs:519-521`); `media_v1/controller.rs:272` `if let Some(moderator)` is skipped entirely when `None`;
`NoOpModerator` (`image_moderation/mod.rs:34-49`) is used only by its own unit test. D5 §1/B2 fact 13 describe a
"NoOp moderator returning `safe: true`" — the outcome is the same (nothing is checked) but the mechanism is
different (no call, not a permissive verdict), and the distinction matters for the runbook: there is no verdict
to rely on.
Fix: correct the record; either enable a provider (`IMAGE_MODERATION_URL` + key) or state in the runbook that
uploads are unmoderated and the CSAM path must never depend on a classifier; gate author creation.

**F6 — There is no public, unauthenticated report channel; the only live intake is a comment flag plus a
personal Gmail, and the published Terms address has no MX. NEW. High.**
Evidence: `post_comment_v1/mod.rs:13` flag is `verified` (auth-only); `/contact` is `mailto:` to the owner's
Gmail (`(marketing)/contact/+page.svelte:34`, `site.ts:11`); Terms/FAQ publish `salam@`/`corrections@easyquran.app`
(`web/messages/en.json:152,213`); `dig MX easyquran.app` → empty. D5 item 9 and C4 finding 1 cover the parts;
the new fact is the combination: the most likely reporter of CSAM or a threat (a non-user, or a user who cannot
sign in) has **no working channel into the system at all**.
Fix: the four `.fyi` aliases + `POST /report/v1/submit` public + the `/report` page + fix the dead addresses;
quarterly test sends.

**F7 — The first retention sweeper will delete law-enforcement evidence unless it ships hold-aware; the
`user_sessions` 90-day fix and the §2703(f) 90+90-day window collide. NEW. High.**
Evidence: `user_sessions.device`/`ip_address` are the only identity-to-network evidence and have no expiry
(`m000001_init.rs:48-57`); C4 §3 lines 154-155 propose 90 days + NULL on revoke; B8 §4 designs the hold for the
device layer only; A8 §7 point 4 lists `user_sessions` in the preserve-from set but does not sequence the
sweeper. A §2703(f) request arriving at day 85 of inactivity finds the columns NULLed at day 90.
Fix: `legal_holds` + `legal_hold_subjects` ship **in the same PR** as the first sweeper; every delete carries
`NOT EXISTS legal_hold_subjects`; `subject_kind='session'` included; the hold extends `expires_at` in the same
transaction (B8 §4.2 mechanism 1 + 2).

**F8 — The ban button is 2–3 days of controller work, not a feature: the entity functions exist, the
enforcement read path exists, and a ban without a notice in the same transaction is the Art 17 violation. NEW. High.**
Evidence: `user_ban/actions.rs:11-145` (`create`/`revoke`/`admin_list` all implemented); `services/auth.rs:530-545`
`check_ban` behind `not_banned()` (`auth_guard.rs:27-33`); `user_v1/mod.rs` has no bans route; A8 F1/D5 state
zero writers. New: the minimal diff (3 controller fns + route registration + a `user_id` index) and the coupling
rule — the ban and the notice are one transaction, or the ban is unlawful under Art 17.
Fix: `/user/v1/admin/bans` create/list/revoke + `enforcement_notices` write in the same tx + session revocation
on create.

**F9 — A suppressed enforcement notice is dropped silently and returns `Ok`, so a banned user may never learn
why and the platform cannot record that it could not tell them. NEW. High.**
Evidence: `mail/router.rs:251-259` — suppressed transactional send logs `debug!` and returns
`Ok(SendReceipt::default())`; `is_transactional` covers only verification + password_reset
(`provider.rs:17-22`); `mail/templates/` has six templates, none enforcement. C4 §4.3 says to add the template
to the transactional set; the new fact is the silent-drop mechanism and the missing
`notice_delivery_state='suppressed'` record that DSA Art 17's "where contact details are known" conditional
requires.
Fix: template + transactional class + a delivery-state write on the enforcement path + a test that a suppressed
enforcement send records `suppressed` (not a bare `Ok`).

**F10 — Nothing alerts anyone; the push primitive that would do it (`notify_user`) has zero callers, and there
is no uptime monitor, digest, or 429 signal. NEW. High.**
Evidence: `services/notification/mod.rs:41-113` (`notify_user` + FCM fan-out, no callers); `enums/notification.rs`
has no Enforcement kind; `scheduler.rs:9-25` publishes posts only; `deploy/README.md` has no monitoring section;
429s are `debug!` (`layer.rs:323` per C3 F4). D5 says "no alerting/paging anywhere"; the new contribution is the
minimum viable design: reuse FCM + `devices` for the operator's own P0 push; digest from the existing scheduler;
public `/quran/health/ready` for the free external monitor.
Fix: intake → push + mail on `kind IN (csam,threat)`; daily digest; external uptime monitor; 429 `warn!`.

**F11 — The runbook has no home, no owner, and no drill; `docs/security.md` still says "nothing is deployed
yet", and no report assigns the work. NEW. Medium-High.**
Evidence: no `docs/incident-response.md`; `docs/security.md:9` stale; D5 §4 asks for the runbook; D5 §4's
runner-up states no report assigns an owner to the blocking list.
Fix: write `docs/incident-response.md` (§13 one-pager + §7 drill + the dated log); run one drill; put the
quarterly date in the calendar with the owner's name.

**F12 — The Moderator role exists, is assignable, and gates nothing; the cheapest second-reviewer path is
already built and unused. NEW. Medium.**
Evidence: `ruxlog-types/enums/user.rs:16-19` (`moderator`, i32 = 2); `auth_guard.rs:20` `ROLE_MODERATOR`;
`user_v1` `admin_update` accepts the role string (`controller.rs:333-346`); no route uses `ROLE_MODERATOR` (grep).
A8 §4/§8 design the single-operator second look; the new fact is that the ACL separation the design asks for is
already present.
Fix: gate the triage queue at `ROLE_MODERATOR`; record `single_operator_control=1` on every case now; when a
second human exists, `ROLE_MODERATOR` proposes and `ROLE_ADMIN` decides with no code change.

---

## Appendix — verification commands (all read-only)

```bash
# the destroy-the-evidence mechanisms
sed -n '358,400p' rust/backend/api/src/db/sea_models/post_comment/actions.rs   # admin_delete = delete_by_id
sed -n '298,308p' rust/backend/api/migration/src/m000001_init.rs               # comment_flags ON DELETE CASCADE
rg -n "clear_flags" rust/backend/api/src                                        # 1 hit, zero callers
sed -n '269,318p' rust/backend/api/src/modules/user_v1/controller.rs           # unlogged hard delete

# the live public IP+UA writer
sed -n '63,67p' rust/backend/api/src/modules/post_v1/mod.rs                    # track_view on the public router
sed -n '346,386p' rust/backend/api/src/modules/post_v1/controller.rs           # reads ClientIp + User-Agent
sed -n '608,647p' rust/backend/api/src/db/sea_models/post/actions.rs           # inserts both columns

# the moderator that is not
sed -n '495,522p' rust/backend/api/src/main.rs                                 # default false → None
sed -n '270,305p' rust/backend/api/src/modules/media_v1/controller.rs          # skipped when None

# the ban primitive + the notice gap
sed -n '11,145p' rust/backend/api/src/db/sea_models/user_ban/actions.rs        # create/revoke/admin_list exist
sed -n '530,545p' rust/backend/api/src/services/auth.rs                        # check_ban
sed -n '17,22p' rust/backend/api/src/services/mail/provider.rs                 # is_transactional
sed -n '251,259p' rust/backend/api/src/services/mail/router.rs                 # suppressed txn → Ok, debug!

# the addresses
dig +short MX easyquran.app        # empty
dig +short MX easyquran.fyi        # Cloudflare
rg -n "easyquran.app" web/messages/en.json                                     # salam@ / corrections@

# the alert primitive with zero callers
rg -n "notify_user\(" rust/backend/api/src                                         # 0 callers
```
