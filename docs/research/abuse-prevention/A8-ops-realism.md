# A8 — Ops / trust-and-safety realism audit of DESIGN-v0

Scope: operational realism of the L0–L7 ladder, the proposed `/admin/devices` console and appeal flow, the evidence model, false-positive monitoring, entity-graph clustering, legal-escalation readiness, and the L6 "second look" — for a one-operator, non-commercial Quran web app with an API-only ops surface.

Inputs: `DESIGN-v0.md`; `R2-ban-operations.md` (ban models, evasion linkage, appeals, evidence, KPIs, small-team limits); `R2-evasion-counters.md` §6–8 (account linking, entity graph, CIB). Repo: `rust/backend/api/src/modules/{admin_bans_v1,admin_route_v1,admin_acl_v1,device_v1,notification_v1,post_comment_v1,user_v1,analytics_v1}`, `src/services/{auth,acl_service,escalation,rate_limit_store,route_blocker_config,scheduler}.rs`, `src/middlewares/auth_guard.rs`, `migration/src/m000001..m000011`, `web/src/routes/`, `deploy/README.md`.

---

## 0. Verified ops inventory (what actually exists today)

| Surface                    | Where                         | Auth                                                                  | Notes                                                                                                                                                                                                                                                                   |
| -------------------------- | ----------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GET/DELETE /admin/bans`   | `admin_bans_v1/controller.rs` | session, `ROLE_ADMIN`                                                 | List/lift **IP bans only**. Rows are `rate_limit_state` keys; `BanRow` = `{banUnit, scope, blockUntilAt}` — **no reason, no actor, no case id, no created_at, no history** (`dto.rs`). List is derived from live L1/L2 keys; expired rows are reaped by the flush task. |
| `GET /admin/bans/export`   | same                          | session `ROLE_ADMIN` **or** `BAN_EXPORT_TOKEN` bearer (constant-time) | Token is **never** accepted by mutation routes (`mod.rs` comment). Export = canonical CIDR + scope + expiry, no PII.                                                                                                                                                    |
| `/admin/route/v1/*`        | `admin_route_v1/`             | `ROLE_ADMIN`                                                          | Route blocker + periodic sync loop (default 30 min, pause/resume/restart).                                                                                                                                                                                              |
| `/admin/acl/v1/*`          | `admin_acl_v1/`               | `ROLE_SUPER_ADMIN`                                                    | `app_constants` CRUD, sensitive values masked.                                                                                                                                                                                                                          |
| `/analytics/v1/*`          | `analytics_v1/`               | `ROLE_ADMIN`                                                          | Raw-SQL dashboard summary; the repo's precedent for an ops metrics endpoint.                                                                                                                                                                                            |
| `/user/v1/admin/*`         | `user_v1/mod.rs`              | `ROLE_ADMIN`                                                          | User CRUD; **no ban routes**.                                                                                                                                                                                                                                           |
| `/post/comment/v1/admin/*` | `post_comment_v1/mod.rs`      | `ROLE_ADMIN`                                                          | The only existing moderation queue: comment list/hide/unhide/delete, flag list/summary/details/clear.                                                                                                                                                                   |
| `/notification/v1/create`  | `notification_v1/mod.rs`      | `ROLE_ADMIN`                                                          | Writes a durable `notifications` row + best-effort FCM push (`services/notification/mod.rs`). No web inbox UI exists.                                                                                                                                                   |
| `/mail/v1/suppression`     | `mail_v1/mod.rs`              | `ROLE_ADMIN`                                                          | Bounce/permanent suppression list.                                                                                                                                                                                                                                      |
| Web admin UI               | —                             | —                                                                     | **None.** `web/src/routes/` has no admin route (confirmed: `(account)`, `(application)`, `(auth)`, `(marketing)`, `_quran`, `auth`, `design`, `health`, `__tests__`). Ops = curl/scripts, documented in `deploy/README.md` § Ban inspection & export.                   |

**Enforcement primitives that exist but are not wired to any operator surface:**

1. **`user_bans`** (m000001): `user_id, reason, banned_by, expires_at, created_at, revoked_at, revoked_by`. Full model + actions (`create`, `revoke`, `get_active_ban`, `admin_list`, `list_by_user`) in `db/sea_models/user_ban/`. Enforced by `check_ban` → `BanStatus` (`rux-auth/src/traits.rs`, supports reason/expiry/banned_by; permanent = no expiry) behind `auth_guard` `not_banned()` with a 5-min cache. **Zero callers create or revoke a ban; no admin route exists.** `NewUserBan::permanent()` is callable directly.
2. **Role ladder** `User 0 / Author 1 / Moderator 2 / Admin 3 / SuperAdmin 4` (`auth_guard.rs`, `ruxlog-types/enums/user.rs`). No route _gate_ uses `ROLE_MODERATOR`, but inline `is_moderator()` (= role ≥ 2) checks already grant delete-any-media, read-any-email, paywall bypass and hidden-post comments (F3 §3.5) — a live privilege tier.
3. **`audit_logs`** table + sea_orm model exist; **zero writers** (grep across `src/` finds only the model). Columns include `ip_address`, `user_agent` — in tension with the design's Layer 0.
4. **IP escalation engine** (`services/escalation.rs`, `config/settings.rs:313`): default **OFF** (`QURAN_BAN_ESCALATION_ENABLED=false`), requires `QURAN_BAN_ALLOWLIST`, temp ban 1 h after 5 attempts/1 h, long ban 7 d after 20 attempts/24 h, `max_active_bans` cap, **never permanent**. Observation state is process-local (`SuspiciousWindow`, capped 10k identities) — lost on restart.
5. **Notifications**: `notifications` table (`user_id, kind, title, body, data, read_at, created_at`), `NotificationKind` = `new_comment, new_follower, payment_received, post_published, mention, system` — **no enforcement kind**.
6. **Mail**: `MailRouter` with templates (`verification, password_reset, newsletter, welcome, payment_receipt, subscription_confirmation`); `is_transactional()` covers only verification + password_reset; suppression list blocks bounced addresses.
7. **Raw IP + raw User-Agent are already stored durably**: `user_sessions.device` = the raw `user-agent` header, `user_sessions.ip_address` = the client IP, written on every login (`auth_v1/controller.rs:182–184`, `create_bound_session`), returned to the user's "devices currently signed in" list (`annotate_session_row` serializes the whole row). No retention policy in code.

**Consequence for the design:** the ladder's upper rungs are not representable today. IP bans are anonymous keys with no record; account bans have a table but no operator surface; device bans do not exist; there is no case, no notice, no appeal, no queue, and no metrics. Every recommendation below is scoped to what one operator can actually run.

---

## 1. L0–L7 operational realism

Legend: **Sustain** = can a solo operator keep this level correct indefinitely (≤15 min/week) without a silent-permanent state? **Silent-perm** = does an unreviewed case become permanent by default?

| Level             | Needs                                                | Exists today                                                                                       | Sustain                                                    | Silent-perm              | Automation required                                                                                                                                                                                                       |
| ----------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L0 observe        | durable observation store                            | in-memory only (`SuspiciousWindow`, 10k cap, lost on restart)                                      | yes (no action)                                            | n/a                      | Persist observation rows or accept amnesia; emit a count so "L0" is not invisible.                                                                                                                                        |
| L1 throttle       | time-boxed limiter                                   | yes (`rux-request-gate` temp block, 1 h)                                                           | **yes**                                                    | no                       | none (already auto-expires).                                                                                                                                                                                              |
| L2 challenge      | Turnstile + client integration                       | Turnstile service exists (`services/turnstile.rs`), CSP already allows `challenges.cloudflare.com` | yes                                                        | no                       | Wire challenge to the challenged path only (login/new-device), never the reader path.                                                                                                                                     |
| L3 cooldown       | read-only state for N hours                          | **no mechanism**                                                                                   | yes if automated                                           | no                       | v1 approximation: short-expiry `user_bans` row (public reader routes are unauthenticated, so a banned user can still read — this _is_ a read-only cooldown functionally). Blunt but correct; label the reason `cooldown`. |
| L4 hold ≤72 h     | a review queue + a user-visible state + a 72 h timer | **nothing**                                                                                        | **no**                                                     | **YES**                  | **Cut as a user-visible state.** See below.                                                                                                                                                                               |
| L5 suspend 3–30 d | account suspension with expiry + notice              | `user_bans` supports expiry; **nothing creates it or notifies**                                    | yes (expiry is implicit — `get_active_ban` stops matching) | no                       | Create-on-proposal + notice + close-the-record job.                                                                                                                                                                       |
| L6 ban            | permanent, evidence frozen, second look              | `user_bans` permanent path exists but ungated                                                      | only with the gate in §8                                   | **YES without the gate** | `proposed → final` state machine; permanent only via finalize.                                                                                                                                                            |
| L7 legal          | SOP + hold + retention class                         | nothing                                                                                            | yes if SOP written                                         | no                       | `legal_holds` table + hold-aware retention job (§7).                                                                                                                                                                      |

### The three silent-permanent risks, precisely

1. **L4 is the worst offender.** "Hold ≤72 h" needs a human at T+72 h. With no queue UI and no auto-expiry, an unreviewed L4 either (a) blocks nothing — it is theatre, or (b) blocks indefinitely — a de-facto permanent restriction with no decision record. Worse, the design's L4 semantics ("actions queued, no new content published") barely apply to this product: the only user-publishing surface is comments (`post_comments`); posts are author/admin-gated; bookmarks are private. **Recommendation: L4 v1 = "case opened + evidence frozen + queued for review, no user-facing change."** The review SLA then applies to _case disposition_, not to a live restriction. This deletes an entire class of silent states, one notice template, and one SLA. If L4 must restrict, it must carry a hard `expires_at = created_at + 72h` that auto-downgrades to L3/L2 and records `expired_without_review`.
2. **L5 self-heals only because `user_bans` expiry is implicit** — enforcement stops matching when `expires_at` passes. But nothing _closes_ the record, so the operator's queue grows monotonically unless a job closes cases past expiry. Add: daily job closing L5 cases whose `expires_at` has passed, with `outcome='expired'`.
3. **L6 permanent is one function call away.** `user_bans::NewUserBan::permanent()` exists and any future handler can call it. The gate must be structural, not conventional: the only code path allowed to write `expires_at=NULL` is `finalize_case()`, and a test must assert no other call site exists.

### Self-heal requirements (the ladder must be a state machine, not a mood)

- **Auto-expiry** on every level with a duration (L1–L3 today; L5 by `user_bans.expires_at`; L4 if kept).
- **Auto-close**: a job (model on `route_blocker_config`'s loop or `scheduler.rs` tick) that closes cases past `expires_at` / past `effective_not_before` and records the outcome.
- **Queue aging**: a metric + a daily digest email to the operator when any case is open >24 h (L4/L5) or >48 h (L6 second look). The mail stack exists; this is one template.
- **Boot invariant check**: on boot (alongside the existing `auth.rs` reconciliation), scan for (a) permanent `user_bans`/device bans with no `second_look_at`, (b) cases enforced past their level's max duration with no decision. Log + surface in metrics. Fail loud, don't auto-fix.
- **Notice completeness**: every L4+ case must have `notice_sent_at`; alert when null.
- **Retention job with a counter**: expired evidence deleted, failures counted (§3, §7).

**Observation amnesia (minor):** L0/L1 thresholds reset on every deploy because the escalation history is in-memory. For a solo operator this is tolerable (bans persist in L2), but it means the rule engine's "observe" tier has no memory; if the design wants L0 to be meaningful it needs a persisted observation row, which is also the input to every threshold review.

---

## 2. `/admin/devices` and the minimum viable console

### What the operator must see

Per device record (one screen per case, list view with filters):

- `device_record_id` (opaque, ULID), **`device_key` prefix only** (the full HMAC key is a linkable identifier — never render it; show e.g. first 8 hex chars + a copyable id for LE bundles).
- `first_seen`, `last_seen`, `seen_count`, `trust_state`, coherence booleans (count + which fired), spoof/tamper flags (booleans only).
- **Account fan-out**: number of distinct accounts bound (`device_sessions`), with the account ids; and the reverse (other devices for those accounts) — this is the §6 "linked accounts" panel, and it is the single most useful field for the operator (family phone vs farm).
- **Shared-device annotations**: `shared=true` / `kiosk` / `cleared`, who annotated and when.
- **Case linkage**: open/closed cases for this device, level, state, age, whether a notice was sent, whether an appeal is pending.
- Network context _if retained_: last IP /64 + ASN bucket (retention class separate; see §3).

### Actions (and their ACL)

| Action                              | Effect                                                                             | Role                              |
| ----------------------------------- | ---------------------------------------------------------------------------------- | --------------------------------- |
| `annotate` (shared / kiosk / clear) | sets `device_records.trust_state` / flags; forces corroboration for future actions | Moderator                         |
| `challenge` (L2)                    | queues a Turnstile challenge for the device                                        | Moderator                         |
| `cooldown` (L3)                     | short-expiry restriction on the bound account(s)                                   | Moderator propose / Admin confirm |
| `suspend` (L5)                      | `user_bans` row with `expires_at`, reason, `banned_by`                             | Admin                             |
| `propose ban` (L6)                  | creates case in `proposed`; **not permanent**                                      | Admin                             |
| `finalize ban` (L6)                 | the only permanent path; requires second-look fields (§8)                          | Admin                             |
| `lift`                              | revoke `user_bans` / deactivate device ban; keep the key                           | Admin                             |
| `export evidence`                   | frozen case bundle (JSON)                                                          | Admin                             |
| policy/thresholds                   | rule version, thresholds, allowlist                                                | SuperAdmin                        |

**ACL fit:** `ROLE_MODERATOR` (level 2) has no route gate today, but its inline `is_moderator()` powers (delete any media, read any user's email, paywall bypass — F3 §3.5) must be narrowed before anyone is promoted. Then seat it here: **read + propose + annotate = Moderator; suspend/lift/finalize = Admin; policy/ACL = SuperAdmin** (matching `admin_acl_v1`). This gives the design's "different reviewer" a real seat the day a second human exists, at zero extra work — and it keeps the operator from needing SuperAdmin for routine work.

### Minimum viable UI: API + TUI/CLI, not a web admin app

There is no admin UI and building one is the largest single cost in the design. The repo's own precedent (`deploy/README.md`) is API + external tooling, and the export-token pattern shows how a machine consumer is authorized. Recommendation:

1. **JSON API** (session + CSRF for mutations, exactly like `/admin/bans`; the export token is **never** accepted by mutations):
   - `GET /admin/device-records` (filters: trust_state, min account fan-out, last_seen window, has-open-case)
   - `GET /admin/device-records/{id}` (detail + linked accounts + case list)
   - `POST /admin/device-records/{id}/annotate|challenge|cooldown|suspend|propose-ban|lift`
   - `GET /admin/enforcement/cases`, `GET /admin/enforcement/cases/{id}`, `POST .../finalize`
   - `GET /admin/enforcement/metrics` (§5)
   - Optional read-only `GET /admin/device-records/export` with a `DEVICE_EXPORT_TOKEN` bearer (same constant-time pattern as `BAN_EXPORT_TOKEN`) so the TUI can list without a session cookie.
2. **A repo-local CLI/TUI** (`scripts/admin/` or a `just` recipe; bun, ~1 day): `just admin devices list|show <id>|annotate <id> shared|suspend <id> --days 7 --reason …|propose-ban <id>|finalize <case>|appeals list|appeal show|decide`. Prints API JSON, writes nothing locally. This is testable, needs no web build, and keeps the operator out of a browser session for an action that is security-sensitive.
3. **Defer the SvelteKit admin page.** It duplicates the session/CSRF stack, adds a second place admin auth can go wrong, and buys nothing the CLI does not. Revisit only if a non-technical reviewer ever joins.

**Naming collision (papercut, fix now):** the repo already has `devices` (FCM push tokens, `device_v1`) and `/device/v1`. The design's `device_records` + `/admin/devices` will confuse every future reader and every API client. Rename the ops surface to `/admin/device-records` (or `/admin/identities`) and never expose the FCM table through it.

---

## 3. Evidence model

### What must be captured at proposal time (frozen, case-scoped)

The design puts evidence on `device_records.evidence` and reuses `audit_logs`. Both are wrong scope. The freeze must be a **case row + append-only evidence rows**:

| Field                                                                                                                     | Why                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `case_id` (ULID), `created_at` (UTC), `level`, `state`                                                                    | joins everything; appeal/LE/dispute key                                                |
| `subject_user_id` (nullable), `device_record_id`, `device_key_prefix`                                                     | subject identity without storing the full key in the record                            |
| `parent_case_id` / `parent_device_record_id`                                                                              | proves the linkage the ban rests on                                                    |
| `score`, `threshold_ban` (80), `threshold_restore` (40), `rule_version`, `rules_fired[]`                                  | reproducibility; "which rule generation acted"                                         |
| **corroboration families present** (device / network / credential / content / behaviour) as booleans + which edge matched | the design's own "device alone may not ban" rule must be _auditable_, not aspirational |
| **derived features as evaluated** (coherence booleans, hardware/platform/locale class labels, screen bucket, spoof flags) | a 3-days-later review cannot re-derive them; the vector changes within hours           |
| **per-signal keyed digests** (HMAC of each component with the server pepper)                                              | prove equality/inequality without storing raw values                                   |
| IP /64 + ASN bucket at evaluation time (separate retention class)                                                         | appeal context + LE preservation                                                       |
| `proposer` (rule id or operator id), `notice_template`, `notice_language`, `notice_channel`, `notice_sent_at`             | Art 17-style statement of reasons; appeal SLA                                          |
| `retention_class`, `expires_at`                                                                                           | anti-hoard                                                                             |
| `prev_hash`, `record_hash`                                                                                                | cheap tamper-evidence (research §8); one column each                                   |

### The "raw signals are never stored" tension

Without the features the rules saw, the decision is unreproducible and an appeal three days later is a rubber stamp. Resolve it explicitly: **raw values are never stored; the decision-relevant derived features + keyed digests are frozen per case.** That is enough to (a) re-read what the machine saw, (b) show a human reviewer the categories, and (c) avoid retaining a raw fingerprint database. Write this sentence into the design — currently the design says only "raw signals never stored", which reads as "no evidence at all".

### Why not `audit_logs` and not `device_records.evidence`

- `audit_logs` is a general, **mutable** table with `ip_address`/`user_agent` columns and no case id, no retention class, no chain. Using it for evidence mixes operator-action auditing with subject evidence and drags the Layer 0 policy into contradiction (writing a UA into it would violate "no raw UA storage"). **Use `audit_logs` for operator actions only** (who viewed/decided what), and a dedicated `case_evidence` for the freeze.
- `device_records.evidence` is per-device and mutable: a re-evaluation rewrites it, the 30-day key retention deletes the row, and `user_id ON DELETE SET NULL` severs the subject. **Case evidence must be self-contained** — no FK that can null or delete. (A FK to `device_records.id` for convenience is fine only if the evidence row survives the device row's deletion.)

### (a) Justify to a human reviewer — sufficient if the fields above exist

Today: nothing exists, so nothing is sufficient. With the fields above, a reviewer can see the rule, the corroboration families, the parent case, and the notice. Add a **redaction rule**: the internal bundle keeps rule ids and weights; the user-facing copy shows categories only ("device + content pattern + timing"), per research §5.

### (b) DSAR — needs a bundle, and there is no DSAR code in the repo

Verified: no DSAR/GDPR/export module exists. Minimum: `GET /user/v1/admin/export/{user_id}` (ROLE_ADMIN) returning one JSON bundle: `users` row, `user_sessions` (ip/device/last_seen/revoked_at), `user_bans`, `device_records` for that user (keyed ids + timestamps + trust_state), `cases` + evidence summaries, notices sent, appeals, and a written explanation of the keyed identifiers and the retention schedule. Note the bundle **must include `user_sessions.device` (raw UA) and `ip_address`**, because those are already stored — a DSAR that omits them is incomplete.

### (c) Survive a dispute — needs immutability, access log, and self-containment

- Append-only `case_evidence` (no UPDATE; corrections are new rows).
- Hash chain + UTC timestamps + recorded clock source.
- **Access log** on evidence reads (research §6): write a row to `audit_logs` on every bundle read/export.
- Self-containment: the bundle must be exportable to a file that does not depend on 30-day-retained rows.

### Gaps summary (task 3)

Missing: case scope; immutability; notice fields; corroboration flags; rule version; retention class; hash chain; access log; DSAR endpoint; a stated resolution of the raw-signal tension; and a decision on whether the keyed `device_key` outlives 30 days (§7).

---

## 4. Appeal flow

### The channel problem

- `notifications` gives a durable row + push, but **no web inbox exists** (`web/src/routes/` has no notifications page; the web notification surface is an FCM toast + settings toggle only). So in-app alone is not durable notice.
- Mail exists and can reach verified users, but `is_transactional()` covers only verification + password_reset — an enforcement template must be classified transactional or it rides marketing limits and the provider circuit breaker. The suppression list will also block bounced addresses — a banned user whose email bounced cannot be notified by email. Hence: **email + durable notification row + a public appeal URL that works while suspended.**

### Flow (v1, no ticketing system)

1. **Action** → case created (`state='proposed'` or `active`), `notice_sent_at` required.
2. **Notice** (transactional email template `enforcement_notice` + `notifications` row with a new `NotificationKind::Enforcement`): what happened, which policy, duration, whether automated means were used, and a **stable public URL** `https://easyquran.fyi/appeal/<token>` where `<token>` = 32 random bytes, base64url, stored as `hmac_sha256(server_key, token)`. Not the case id.
3. **Appeal page** (web, public route, no auth): shows the notice text + the 3-field form (what were you doing / who else uses this device / free text) + optional contact email. Fetches a CSRF token from `/csrf/v1/generate` (the private stack's CSRF guard covers this POST; add `/appeal/v1/*` to the public, `unauthenticated`-guarded nest — **never** behind `not_banned()`).
4. **Submit** → `POST /appeal/v1/submit` (rate-limited per IP): writes `appeals` row, sends auto-ack email, returns a reference.
5. **Triage (automated)**: auto-reverse when the action was device-signal-only or the account had no prior violation; everything else → human queue. Auto-reversals count as proactive reinstatements (Santa Clara § Numbers).
6. **Human review**: v1 = the same operator, but a **new decision record** with a fresh evidence read and a fixed checklist; `single_operator_control=true` logged. When a second human exists: Moderator proposes, Admin decides.
7. **SLA**: first response ≤72 h, final ≤7 days; expedite anything involving a minor or harassment. Overdue appeals surface in the daily digest.
8. **Reversal (device ban)**: set case `state='overturned'`, `overturned_at`, `overturn_reason_code`; `device_records.trust_state='cleared'`; **keep the key** (the anti-evasion asset) but stop enforcement; if the shared-device claim is verified, annotate `shared=true` → future actions on that device require corroboration; un-revoke any sessions revoked at ban time; send the reversal notice; if the ban was exported to the edge, ensure the adapter re-polls (already a documented requirement in `deploy/README.md`).
9. **Reversal (account ban)**: `user_bans.revoke(ban_id, revoked_by)` — the action already exists; note the ≤5 min ban-cache propagation lag.
10. **Evidence after reversal**: keep the case and evidence (12 mo), add the overturn fields; feed precision/overturn KPIs. Do not delete.

### Data model (minimum)

`appeals`: `id`, `case_id`, `token_hash`, `submitted_at`, `body`, `shared_device_claim`, `contact`, `state`, `sla_due_at`, `reviewer_id`, `decision`, `decided_at`, `reason_code`, `notice_message_hash`. No file uploads, no threads in v1.

### Why this and not email-only or a ticket system

Email-only fails the "durable, accessible while suspended" requirement and gives no structured intake or SLA clock. A ticket system is a new service to run and back up for a handful of cases per month. **A table + two endpoints + one mail template is the whole mechanism.** The reply-to must be a monitored mailbox (`support@`), not `noreply@`.

---

## 5. False-positive monitoring

Minimum dashboard a solo operator can maintain. All numbers computed from `enforcement_cases`, `appeals`, `device_records`, `user_bans`, `rate_limit_state` — **one endpoint, on demand** (pool = 1 connection; never a scheduled job writing metrics).

| Metric                                                            | Source                             | Cadence              | Target (normative — no platform publishes achieved rates) |
| ----------------------------------------------------------------- | ---------------------------------- | -------------------- | --------------------------------------------------------- |
| Open cases by level + oldest age                                  | `enforcement_cases`                | daily glance (2 min) | oldest < 72 h                                             |
| **Precision** = upheld ÷ (upheld + overturned)                    | cases                              | weekly               | ≥95% permanent, ≥90% suspension                           |
| Overturn rate = overturned appeals ÷ decided appeals              | `appeals`                          | weekly               | <5% (red >15%)                                            |
| Proactive reinstatements (auto-reversals + second-look reversals) | cases                              | weekly               | >0, trending up                                           |
| `expired_without_review` count                                    | cases                              | weekly               | 0                                                         |
| Evidence-freeze coverage                                          | cases                              | weekly               | 100%                                                      |
| Notice coverage (L4+)                                             | cases                              | weekly               | 100%                                                      |
| Second-look lag (median proposal→finalize)                        | cases                              | weekly               | ≥24 h                                                     |
| SLA breaches (appeal overdue)                                     | `appeals`                          | weekly               | 0                                                         |
| Retention job: expired evidence deleted / failures                | evidence                           | weekly               | 100% / 0                                                  |
| L5 cases past expiry not closed                                   | cases                              | daily                | 0                                                         |
| Device fan-out (devices with ≥3 accounts)                         | `device_records`/`device_sessions` | monthly              | informational, **not** an abuse metric                    |
| **NOT "bans issued"**                                             | —                                  | —                    | never a headline                                          |

Implementation: one `GET /admin/enforcement/metrics` (ROLE_ADMIN) following the `analytics_v1::dashboard_summary` raw-SQL pattern, and a `just admin metrics` printer. Cadence: daily glance, weekly 15-minute review, monthly threshold replay ("what would new thresholds have done to the last 90 days of proposals?" — possible only because cases are stored), and an immediate second look + KPI check after any L6.

**Why this is the right size:** it is one query per number over indexed columns, it takes 15 minutes a week, and every number is a _precision_ number. A dashboard that rewards ban count is the documented false-positive factory (research §9).

---

## 6. Entity graph: go/no-go

**Recommendation: NO-GO for automated clustering in v1.** Build a bounded linked-accounts panel instead.

Reasons:

1. **Infrastructure mismatch.** Pool = 1 SQLite connection. Any graph job must be offline; a materialized cluster table is a second dataset to retain and audit. Community detection (Louvain) on a graph this small is over-engineering with a real maintenance tail.
2. **The user base is the worst case for components.** Research §6.2 is explicit: exact-match connected components over shared IPs "link thousands of unrelated users". For a Quran app the shared-hardware population (family phones, madrasa labs, CGNAT) is the _core_ audience; components will routinely mix real readers.
3. **No observed threat to justify it.** Research §8.1 found no public incident of CIB against Quran apps; the documented attacks are scraping, review-bombing (only if ratings exist), and off-platform sabotage. The design has no ratings.
4. **Poisoning.** An attacker can deliberately share a device/IP with victims to drag them into a cluster ("ant trap"). Asymmetric edge weights mitigate this only if low-precision edges never accumulate — which means the graph must never feed bans, which means it is advisory only.
5. **Credential reuse is not available as an edge.** Passwords are Argon2id PHC strings with per-hash salts (`services/auth.rs`), so hashes are not comparable across accounts. Using password reuse as a linkage signal would require storing a keyed HMAC of the password — a new sensitive dataset with a real privacy cost. Not worth it here.

**Smallest useful version (v1):** an on-demand **linked-accounts panel** on the case/device view:

- `device_record → user_ids` (via `device_sessions`)
- `user_ids → other device_record_ids`
- `other device_record_ids → user_ids` (hop 2, bounded: stop expanding any device with >20 accounts — that is a shared device, and its expansion is noise)
  Three to four indexed queries, a few hundred rows, computed per request with `LIMIT`s. This answers the operator's actual question — "is this a family phone?" — with zero false-positive machinery. Cost: negligible on SQLite; no new tables.

**If a real T3 incident ever appears:** add a nightly offline job (after the flush task) that reads all edges in one query and runs union-find in Rust (`O(E α(V))`, <100 ms for 100k rows), with device + registration-cohort edges only (IP edges excluded from components, used only as soft context), writing to a `clusters` table. Review clusters by hand; never auto-ban. Budget 1–2 days; defer until evidence demands it.

---

## 7. Legal-escalation readiness

### What exists

- `user_sessions` (raw IP + raw UA, `revoked_at`), `auth_session_binding`, `user_bans`, `audit_logs` (empty), `notifications`, `devices` (FCM tokens), `sessions` (tower msgpack), `users`. The Quran DBs carry no PII and are immutable.
- Ban records today are live-only `rate_limit_state` keys — **there is no 12-month ban ledger**, and the design's "ban record 12mo" has no storage. The case table (§3) is what makes it real.

### SOP for a preservation request (write this down before it is needed)

The generic workflow — intake to one address with a case id and sender verification, classify (preservation /
subpoena / court order / emergency), freeze, acknowledge in writing with the window end (90 + 90 days), never
disclose without process, calendar the expiry, respond narrowly, and the mandatory CSAM path (NCMEC report,
1-year preservation) — is R2-ban-operations §8. What this repo has to add:

1. **Freeze**: a `legal_holds` row (new table: `id, case_id, received_at, agency, agent_contact, request_type,
scope_json, preserve_until, status, notes`) with linked subjects (`legal_hold_subjects`: `hold_id, user_id,
device_record_id, case_id`); **the retention job must consult this table and skip held rows.** This is the
   single most important addition — today no retention job exists at all, and once one does, a hold must be able
   to stop it.
2. **Preserve from**: `users`, `user_sessions` (ip_address, device, last_seen, revoked_at), `auth_session_binding`,
   `user_bans`, `audit_logs`, `notifications`, `devices`, `enforcement_cases` + `case_evidence`, `appeals`, tower
   `sessions`. Intake address: `legal@` (the contact addresses need a working MX first).
3. **Disclosure log**: every disclosure writes `audit_logs` with `action='legal_disclosure'`,
   `resource_type='case'`, `resource_id=case_id`, metadata = scope (the table exists but has zero writers — the
   design must wire writers, or add `legal_disclosures`).

### Retention conflicts the design must resolve

- **30-day device-key retention < §2703(f) 90+90 days < §2258A 1 year.** A preservation request arriving 60 days after an incident finds the key gone. Fix: case evidence (derived features + digests) lives 12 months and is self-contained; raw per-signal values are never stored anyway; and **the keyed `device_key` must survive for the ban's lifetime**, or a 12-month ban record cannot match anything and evasion recapture silently ends at 30 days. State this trade-off explicitly in the design; keep the 30-day rule for _non-banned_ devices.
- **L7 retention class**: CSAM-adjacent cases need ≥1 year from report, independent of the general ladder.
- **`user_sessions` raw IP/UA has no retention policy today.** It will appear in every DSAR and preservation bundle; the ladder must name it.

### What the design must add

`legal_holds` + `legal_hold_subjects`, hold-aware retention job, disclosure logging, the L7 retention class, an LE-guidelines page with a single contact (model: Reddit's), and the SOP as a repo doc. A lawyer on retainer is out of scope but worth one line in the doc.

---

## 8. The L6 second look with one operator

Theatre is the default outcome of "review your own decision later". Make it mechanical:

1. **The second look is a gate, not a review of a done deal.** L6 proposal creates `state='proposed'`; enforcement during the window is L5-equivalent (temporary suspension with `expires_at`). The permanent state cannot exist before finalization.
2. **Mandatory delay**: `effective_not_before = created_at + 24h` (72 h if a minor is involved). The finalize endpoint rejects earlier timestamps.
3. **No synchronous path**: no endpoint accepts `permanent=true`; the only writer of `expires_at=NULL` is `finalize_case()`, and a test asserts the call-site count.
4. **Checklist (stored, fixed)**: (1) evidence bundle re-read, (2) parent linkage is a prior _case_, not just a shared IP, (3) ≥2 corroboration families, (4) notice sent and readable, (5) shared-device signals checked (fan-out ≥3, kiosk annotation, CGNAT), (6) no open appeal, (7) retention class set. All fields non-null to finalize.
5. **Fresh-evidence requirement**: the finalize handler requires `evidence_read_at` from the reviewer's session within the last N minutes — you cannot finalize without opening the bundle.
6. **Lapse handling**: if not finalized by T+7d, auto-downgrade to L5 and mark `lapsed_without_second_look`; alert. Never silently permanent.
7. **Different reviewer role**: solo operator uses the same human, but the API records `proposer_id`, `reviewer_id`, `second_look_at`, and `single_operator_control=true`. When a second person exists, Moderator/Admin separation is already in the ACL (§2).
8. **Anti-theatre audit**: publish the second-look lag and overturn rate; quarterly, re-read 3 closed L6 cases cold and log the outcome. A rubber stamp shows up as <1 h median lag and 0 overturns.

---

## 9. Ranking: impact ÷ effort, and what to cut

| Feature                                                                                                                                           | Abuse-reduction impact                      | Operator effort | Eng effort | Verdict                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------- | --------------- | ---------- | ---------------------------- |
| Per-credential rate limit correctness (`login:<email>` and five other account keys exist; they count successes and can lock a victim out — B2 F2) | **H** (also fixes credential stuffing)      | L (automated)   | L          | **v1 — do first**            |
| L1–L3 auto + device axis + auto-expiry                                                                                                            | H                                           | L               | M          | **v1**                       |
| Case table + notices + auto-close/expiry job                                                                                                      | H (auditability, precision)                 | M               | M          | **v1**                       |
| Evidence freeze on L4+                                                                                                                            | H (defensibility)                           | L (automated)   | M          | **v1**                       |
| Appeal channel (email + token page + table)                                                                                                       | M (legal risk)                              | M               | M          | **v1**                       |
| Second-look gate                                                                                                                                  | M                                           | M               | L          | **v1**                       |
| Legal hold + SOP                                                                                                                                  | M (rare, high stakes)                       | L               | L          | **v1**                       |
| Metrics endpoint                                                                                                                                  | M                                           | L               | L          | **v1**                       |
| Device console API + TUI                                                                                                                          | M                                           | M               | M          | **v1 (thin)**                |
| L4 as a user-visible state                                                                                                                        | L (no publishing surface)                   | M               | M          | **cut → queue-only case**    |
| Web admin UI                                                                                                                                      | 0 (same actions as CLI)                     | M               | H          | **cut**                      |
| Entity graph / clustering                                                                                                                         | L (no observed threat)                      | M               | H          | **cut (§6)**                 |
| Automatic device bans                                                                                                                             | M but negative (FP risk on shared hardware) | —               | M          | **cut — operator-only bans** |
| Appeal uploads / case threads                                                                                                                     | 0                                           | M               | M          | **cut**                      |
| Payment/phone/email-reputation linkage                                                                                                            | 0 (no payments)                             | —               | —          | **cut**                      |

**Cut list rationale, one line each:** the entity graph and web admin UI are the two largest builds with the least abuse reduction; L4 as a visible state creates the silent-permanent risk the design otherwise avoids; automatic device bans contradict the design's own shared-device rule; uploads/threads add attack surface (file handling) to a channel that needs three text fields.

**Sequencing note:** per-credential limits and L1–L3 automation reduce the number of cases that ever reach a human. Build those before the console — the best ops feature is a smaller queue.

---

## Findings

1. **The account-ban primitive already exists and is completely unwired.**
   Severity: High.
   Evidence: `migration/src/m000001_init.rs:58` (`user_bans`), `db/sea_models/user_ban/{model,actions,slice}.rs` (create/revoke/get_active_ban/admin_list, `NewUserBan::permanent()`), enforced by `services/auth.rs:530` `check_ban` → `rux-auth/src/traits.rs` `BanStatus`, behind `auth_guard` `not_banned()` (5-min cache). Grep: no module creates or revokes a ban; no `/admin` route for it.
   Fix: build L5/L6 on `user_bans` (reason/expiry/banned_by/revoked_at already fit the ladder); add `/admin/users/bans` (list/create/revoke) rather than inventing an account-ban table.

2. **There is no enforcement-case table, so the ladder above L3 is unrepresentable.**
   Severity: High.
   Evidence: ban state = `rate_limit_state` rows (`m000002`), `admin_bans_v1/dto.rs` `BanRow` has only `{banUnit, scope, blockUntilAt}`; the list is derived from live keys and expired rows are reaped; `audit_logs` exists with zero writers.
   Fix: add `enforcement_cases` (id, level, state, subject, parent, score/rule_version, notice, retention, timestamps, second-look fields) + `case_evidence`; every ladder level writes a case.

3. **L4 hold has no auto-expiry and will silently become permanent.**
   Severity: High.
   Evidence: design Layer 3 "L4 hold for review (<=72h)" with no timer/queue; no job scans cases; the app's only publishing surface is comments (`post_comment_v1`), so a "no new content published" hold is nearly a no-op — or, if implemented as an account freeze, an unreviewed one never ends.
   Fix: v1 = queue-only L4 (case opened, evidence frozen, no user-facing change). If a restriction is required, hard `expires_at = +72h`, auto-downgrade, record `expired_without_review`.

4. **Evidence is scoped per device, not per case, and depends on rows that die at 30 days.**
   Severity: High.
   Evidence: design Layer 2 `device_records.evidence`, `user_id ON DELETE SET NULL`, retention "keyed device key 30d"; design Layer 5 "evidence" via `/admin/devices`.
   Fix: case-scoped, append-only `case_evidence` with derived features + keyed digests + rule/threshold/rule_version + corroboration booleans + notice fields + hash chain; self-contained (no FK that can null/delete).

5. **`audit_logs` is the wrong store for evidence.**
   Severity: Medium.
   Evidence: `m000001_init.rs:252` schema is mutable, has `ip_address`/`user_agent`, no case id, no retention class, no chain; zero writers.
   Fix: use `audit_logs` for operator-action auditing (and evidence-read access logging) only; keep subject evidence in `case_evidence`.

6. **Raw UA and IP are already stored durably, contradicting Layer 0 and unaccounted for in the retention ladder.**
   Severity: High.
   Evidence: `auth_v1/controller.rs:182–184` writes the raw `user-agent` and client IP into `user_sessions.device` / `user_sessions.ip_address` on every login; `annotate_session_row` returns the whole row to the "devices currently signed in" UI. Design Layer 0: "No raw UA/UA-CH storage… raw signals never stored".
   Fix: either stop storing the raw UA (store a keyed hash / coarse platform class), or name `user_sessions` in the retention ladder, DSAR bundle, and privacy notice. Do not claim "raw signals never stored" while this column exists.

7. **There is no appeal channel and no durable notice surface.**
   Severity: High.
   Evidence: no appeal module/table/endpoint; `notifications` exists but no web inbox (`web/src/routes/` has no notifications page — only an FCM toast + settings toggle); `MailRouter` templates contain no enforcement template and `is_transactional()` covers only verification + password_reset; suppression list can block bounced addresses.
   Fix: `appeals` table + public `POST /appeal/v1/submit` (unauthenticated, CSRF via `/csrf/v1/generate`, rate-limited) + tokenized web page `/appeal/<token>` + transactional `enforcement_notice` mail template + `NotificationKind::Enforcement`; reply-to a monitored mailbox.

8. **No queue aging or self-heal job exists.**
   Severity: Medium-High.
   Evidence: no scanner over bans/cases; the patterns to copy exist (`route_blocker_config` loop, `scheduler.rs` tick, boot reconciliation in `auth.rs`).
   Fix: daily job — close cases past expiry, alert on open >24 h/48 h, boot invariant check for permanent bans without `second_look_at`, notice-completeness check, retention counter.

9. **The L6 second look is unenforceable as designed.**
   Severity: High.
   Evidence: `NewUserBan::permanent()` is callable from anywhere; no `second_look`/`effective_not_before` fields anywhere; design says "L6 ban + second look" with no mechanism.
   Fix: `proposed → final` state machine; permanent only via `finalize_case()`; mandatory 24 h/72 h delay; fixed checklist + fresh-evidence read; lapse auto-downgrades to L5.

10. **`ROLE_MODERATOR` has no route gate and is the natural seat for the device console — once its inline powers are narrowed.**
    Severity: Medium.
    Evidence: `auth_guard.rs:18–22` defines level 2; no module's route gate uses `ROLE_MODERATOR` (grep over `modules/*/mod.rs`), but inline `is_moderator()` checks grant delete-any-media, read-any-email and paywall bypass (`media_v1/controller.rs:539-548`, `user/model.rs:49-51`; F3 §3.5).
    Fix: first restrict media delete to Admin and remove the email/paywall reads from the inline checks; then read/propose/annotate = Moderator; suspend/lift/finalize = Admin; policy/ACL = SuperAdmin — this also gives the "different reviewer" a real ACL seat.

11. **The export-token pattern must not be extended to device mutations.**
    Severity: Medium.
    Evidence: `admin_bans_v1/mod.rs` — `BAN_EXPORT_TOKEN` is accepted only by `/admin/bans/export` and "NEVER accepted by the mutation routes".
    Fix: mutations = session + CSRF only; a read-only `DEVICE_EXPORT_TOKEN` may serve the TUI's list view; never reuse a bearer token for ban/suspend.

12. **`/admin/devices` collides with the existing FCM `devices` table and `/device/v1`.**
    Severity: Low-Medium.
    Evidence: `m000005`/`m000011` create `devices` (push tokens, unique ownership) and `device_v1` exposes `/device/v1`; the design adds `device_records` + `/admin/devices`.
    Fix: name the ops surface `/admin/device-records` (or `/admin/identities`) and document the distinction; never serve FCM tokens from it.

13. **Entity-graph clustering is a no-go at this size; use a bounded linked-accounts panel.**
    Severity: Medium.
    Evidence: pool = 1 SQLite connection; shared phones/madrasa labs are the core audience (research §6.2 warns exact-match components "link thousands of unrelated users"); no observed CIB against Quran apps (research §8.1); ant-trap poisoning; Argon2id per-hash salts make credential-reuse edges impossible without a new keyed password hash.
    Fix: on-demand 2-hop panel (device→accounts→devices→accounts, expansion capped at 20 accounts) on the case view; defer any nightly union-find/clustering until a real incident appears; never let clusters feed bans.

14. **Legal preservation has no hold, no disclosure log, and a retention schedule shorter than the statutes it will be served under.**
    Severity: High.
    Evidence: design retention = 30 d device key / 12 mo ban record, but ban records live only as live `rate_limit_state` keys (no ledger); no `legal_holds` table; `audit_logs` zero writers; `user_sessions` raw IP/UA has no retention policy.
    Fix: `legal_holds` + `legal_hold_subjects`, hold-aware retention job, disclosure logging, L7 = ≥1 yr retention class, keyed `device_key` retained for the ban's lifetime, LE-guidelines page + single contact, SOP checked into the repo.

---

_End of A8. Repo facts verified against the working tree at the time of writing; line references are to the files named._
