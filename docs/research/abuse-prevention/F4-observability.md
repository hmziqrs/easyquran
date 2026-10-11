# F4 — Minimum observability + kill-switch layer for a solo operator (design, then attack)

Auditor: observability/kill-switch designer (round 7). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree, HEAD `53d3d421`). Method: read `C3-base-rate.md`, `D3-measurement-attack.md`,
`E1-incident-response.md` (F10/F11), `E5-fresh-eyes.md` (Part 4/Part 5), `A8-ops-realism.md`
(findings 8/14), `B6-rollout.md` (finding 7, §2/§3/§7/§8), `C6-plan-shape.md` (§6/§7); then read the
actual primitives in `rust/backend/api/src/`, `web/src/`, `docker-compose.yml`, `deploy/README.md`,
`justfile`; then validated every SQL against the dev DB read-only (`immutable=1`, aggregate counts only,
no writes). No repo writes, no DB writes, no external system contacted.

**Marking:** **NEW** = not stated in C3/D3/E1/E5/A8/B6/C6. Everything else is a re-derivation.

---

**Verdict up front.** Round 6 was right that the operator is blind, but the blindness is narrower and
more fixable than "no metrics backend". The repo already has: an HTTP metrics middleware with correct
cardinality discipline, a full in-process counter set (limiter, auth, mail, HTTP) that is _write-only_
because OTLP is off, a working no-restart runtime-config pattern, working FCM push with zero callers,
health endpoints, and a scheduler. What is missing is **four small things**:

1. **A durable block ledger.** `rate_limit_state` deletes expired rows every 10 s and the per-route
   layer never sets `block_until_at` at all (`layer.rs:315-330`), so "blocks by key class" and "429s by
   route" have **no durable source** after any fix. One append-only `rate_limit_events` table written
   **inside the existing 10 s flush transaction** on state transitions fixes it with zero request-path
   writes.
2. **One readable output.** `GET /api/admin/abuse/v1/summary` (one SQL statement + in-process atomics +
   file stats) and `just abuse-report` (WAL-safe copy, same SQL). Not a metrics backend. Not the
   `analytics_v1` dashboard (it is broken today: `dashboard_summary` selects from the missing
   `newsletter_subscribers` table, D3 F9).
3. **A heartbeat.** The digest and the self-heal jobs can die silently and nothing notices. One new
   public `/health/ops` (or an extension of `/quran/health/ready`) reporting last-run ages, watched by
   a free external uptime monitor, turns "the monitor is dead" into an uptime alert.
4. **Persisted kill switches.** The only no-restart switch in the stack is in-memory atomics
   (`route_blocker_config.rs:12-20`), and the stack auto-redeploys on every master push
   (`deploy/README.md:21-28`) — so any pause flipped during an incident is silently reverted by the
   next merge. Switch state must persist in `app_constants` and be re-read at boot.

The honest minimum before **any** enforcement ships is items 1–4 plus the audit writer the ban contract
already contains. OTLP/Quickwit is **not** on that list — and the "two env lines" framing is wrong (§7
attack, F2): there is no collector, the default ingest URL is `http://localhost:7280`, and export
failures are dropped.

---

## 0. Ground truth — what actually exists (verified this session)

| Primitive                                   | State                                                                                                                                                                                                                                                                                                                                                                         | Evidence                                                                                                                                                                                                   |
| ------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HTTP metrics middleware                     | Exists, **correct cardinality discipline**: template path from `MatchedPath`, `"unmatched"` fallback, labels `http.method`/`http.route`/`http.status_code`                                                                                                                                                                                                                    | `middlewares/http_metrics.rs:8-46`                                                                                                                                                                         |
| OTLP export                                 | **Off by default**; gated on `ENABLE_QUICKWIT_OTEL` (`1                                                                                                                                                                                                                                                                                                                       | true`); default endpoint `http://localhost:7280`; **no Quickwit/collector service anywhere** in the repo; `.env:39` `OTEL_SDK_DISABLED=true` is **not read by this code** (only `ENABLE_QUICKWIT_OTEL` is) | `utils/telemetry.rs:247-252,264-268`; `docker-compose.yml` (no OTEL/Quickwit var, no such service) |
| Metric families defined                     | `http.server.*`, `db.pool.connections`, `auth.login.{attempts,success,failure}`, `auth.session.created`, `auth.password.verification.duration`, `image.optimization.*`, `limiter.{checks,allowed,blocked,blocked.temp,blocked.long}`, `mail.{sent,failed,send.duration}`, `mail.{suppressed,throttled,deduped,bounced.sync,suppression.check.failed}`                         | `utils/telemetry.rs:343-537`                                                                                                                                                                               |
| Counter readability                         | **None.** `opentelemetry::metrics::Counter` has no read API; the only readable counter is `InMemoryStore::saturation_count()`                                                                                                                                                                                                                                                 | `telemetry.rs` statics; `rux-request-gate/src/store.rs:220-226`                                                                                                                                            |
| Limiter hooks                               | `TelemetryHooks` increments the OTel counters on every check/allow/block (request path, in-memory only)                                                                                                                                                                                                                                                                       | `services/abuse_limiter.rs:11-31`                                                                                                                                                                          |
| Block state                                 | `rate_limit_state(key, fixed_count, fixed_expires_at, block_until_at, block_scope)`; flush every 10 s **deletes expired rows first**; `prune()` after every flush. **Snapshot, not ledger**                                                                                                                                                                                   | `services/rate_limit_store.rs:22-28,180-229`; `store.rs:123-155,176`                                                                                                                                       |
| Per-route 429                               | `RateLimitLayer` increments `fixed_count` and returns 429 when `count > max_requests`; **never calls `set_block`** → route 429s leave no `block_until_at` and no block row                                                                                                                                                                                                    | `rux-request-gate/src/layer.rs:279-330`                                                                                                                                                                    |
| Per-identity block                          | `abuse_check` → `set_block` (Temp/Long); WARN `"Request blocked by abuse limiter"` with span `scope=key_prefix`; block event re-uses `scope` for `BlockScope`                                                                                                                                                                                                                 | `abuse.rs:100-115`; `abuse.rs:57-63`                                                                                                                                                                       |
| Runtime config (the only no-restart switch) | `route_blocker_config.rs` — `lazy_static` atomics + `Notify`; admin routes `GET/POST /admin/route/v1/sync_interval`, `pause`, `resume`, `restart` behind `ROLE_ADMIN`; **not persisted** — a redeploy resets it                                                                                                                                                               | `route_blocker_config.rs:12-20`; `admin_route_v1/controller.rs:187-257`; `main.rs:694-756`                                                                                                                 |
| Scheduler                                   | One job, 60 s tick, publishes scheduled posts; session-store sweep hourly; rate-limit flush 10 s with a slow companion every 6th tick (~60 s)                                                                                                                                                                                                                                 | `services/scheduler.rs:9-25`; `main.rs:586-660,761-775`                                                                                                                                                    |
| Health — API                                | `/healthz` = DB ping only, **Docker-network only** (Traefik label excludes `/api/healthz`); `/quran/health/ready` = **static `ready:true`** + verse/surah constants + auth-provider flags — **no DB check**                                                                                                                                                                   | `router.rs:212-234`; `quran_v1/controller.rs:1037-1060`; `docker-compose.yml:184`                                                                                                                          |
| Health — web                                | `/health/quran` = build manifest present + cache dir writable; **does not expose cache stats**                                                                                                                                                                                                                                                                                | `web/src/routes/health/quran/+server.ts:20-34`; `readiness.ts`                                                                                                                                             |
| Disk-cache stats                            | `getQuranDiskCacheStats()` exists with `hits`/`misses` — **zero callers** (grep)                                                                                                                                                                                                                                                                                              | `web/src/lib/server/quran-disk-cache.ts:138-142,223`                                                                                                                                                       |
| Admin dashboard                             | `analytics_v1` 8 POST endpoints, `ROLE_ADMIN`; `dashboard_summary` selects from missing `newsletter_subscribers` → errors today; no abuse field anywhere                                                                                                                                                                                                                      | `analytics_v1/mod.rs:8-35`; `controller.rs:875-890`                                                                                                                                                        |
| Ban admin                                   | IP bans: `GET/DELETE /admin/bans` + `/export` (Bearer `BAN_EXPORT_TOKEN`); account bans: `user_bans` model + `create/revoke/admin_list` exist, **zero writers, no route**                                                                                                                                                                                                     | `admin_bans_v1/mod.rs`; `user_ban/actions.rs`; `deploy/README.md:411-436`                                                                                                                                  |
| Audit                                       | `audit_logs` table + model, **zero writers**; `user_id` FK `ON DELETE SET NULL`                                                                                                                                                                                                                                                                                               | `db/sea_models/audit_log/`; grep 0 hits                                                                                                                                                                    |
| Notifications                               | `notify_user` + FCM fan-out works; **zero callers**; no `Enforcement`/alert kind; `devices` (FCM tokens, 20/user cap)                                                                                                                                                                                                                                                         | `services/notification/mod.rs:41-113`; `enums/notification.rs`                                                                                                                                             |
| Mail                                        | Per-recipient caps + provider cap (transactional bypasses the provider cap); suppression list; dedup; sync-bounce suppression; **webhook secret optional and empty-allowed** → `email_suppression` can be structurally 0                                                                                                                                                      | `services/mail/router.rs:20-66,119-135,241-291`; `main.rs:39-42`; E2 §1                                                                                                                                    |
| Flags in use                                | `QURAN_BAN_ESCALATION_ENABLED` (false) + `QURAN_BAN_ALLOWLIST` (required when enabled, boot error otherwise); `QURAN_ACTIVE_BAN_MAX` (2000, read once at flush spawn); `WEB_AUTH_ENABLED`; `MAIL_RATE_LIMIT_ENABLED`; `IMAGE_MODERATION_ENABLED`; `FCM_ENABLED`; `QURAN_DEMAND_COLLECT`; `QURAN_INTERNAL_REQUESTS_PER_MINUTE` (600); `QURAN_HEALTH_REQUESTS_PER_MINUTE` (120) | `config/settings.rs:349-378,462-497`; `main.rs:39,424,499,547`; `rate_limit_store.rs:56-58,134`                                                                                                            |
| Deploy                                      | master push → images → Dokploy `compose.redeploy`; **no `logging:` block** in compose; `restart: unless-stopped`; healthchecks: api `curl /healthz && /quran/health/ready`, web `fetch /health/quran`; no monitoring section in `deploy/README.md`; no backup section                                                                                                         | `docker-compose.yml` (full read); `deploy/README.md:21-28,124-126,180-181`                                                                                                                                 |

One-line summary: **the instrumentation is built and write-only; the ledger is missing; the output is
missing; the heartbeat is missing; the switches are not persistent.**

---

## 1. The 8 numbers a solo operator must see

Design rules that shape every number below:

- **No new request-path DB writes.** `rate_limit_store.rs:1` states the rule; B6 F9 and A7 quantify why
  (1 fsync/commit ≈ ~950 req/s ceiling; the 10 s flush holds the single connection 60–200 ms).
- **Every number is computable from data that exists after the ban contract (E4) and the limiter fix
  (E6 P0.2) — plus two tiny new tables** (`rate_limit_events`, `job_runs`), both written only by
  off-path tasks.
- **Timestamps are mixed formats.** Old tables (`user_bans`, `audit_logs`, `users`, `comment_flags`,
  `email_suppression`) are TEXT ISO-8601; new tables and `rate_limit_state` are INTEGER epoch. A
  cross-format comparison in SQLite is silently false (E1 §3 note). Each query below uses the right
  form.
- **No raw IPs or emails in the output.** Key classes are a fixed 7-value enum; identity keys stay in
  the live `rate_limit_state` snapshot only (the operator can read them there when investigating a
  specific block).

### 1.1 The 8 digest slots (11 required items mapped)

| #   | Slot                       | Sub-numbers                                                                                                                                         | Source / exact query                                                                                                       | Cadence                                                    | Threshold that should worry                                                                                                                          |
| --- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Enforcement**            | account bans active / created 7 d / revoked 7 d; IP bans active (`quran-ban:`); notices undelivered > 24 h                                          | `user_bans`, `enforcement_notices`, `rate_limit_state` — Q1                                                                | daily digest + endpoint + report                           | **any** new ban → read the notice + case; > 5 active bans; any revoked 7 d → check appeals; any undelivered notice → Art 17 breach                   |
| 2   | **Blocks by key class**    | blocks 7 d by class (route / identity-login / identity-comment / identity-forgot / identity-newsletter / escalation-ban), Long share, expired count | new `rate_limit_events` — Q2                                                                                               | daily digest + report (7 d), endpoint (24 h)               | first event after zero = signal; escalation-ban > 0 → read the incident; identity-login blocks on > 3 distinct identities/day → distributed stuffing |
| 3   | **429s by route**          | per-route 429s since boot (atomics); hottest route buckets now (`fixed_count`)                                                                      | new in-process `route_429s{route}` counters (incremented in `blocked_response` closure) + `rate_limit_state` snapshot — Q3 | digest (delta from stored snapshot), endpoint (since boot) | one route > 50/day or > 10× ceiling; `quran-v1` 429s while no CF Pareto anomaly → possible false positive                                            |
| 4   | **Auth pressure**          | failed logins since boot; banned-login attempts since boot; login blocks 7 d                                                                        | new atomics at the existing WARN sites (`auth_v1/controller.rs:155,226`) + `rate_limit_events` — Q4                        | digest (delta), endpoint                                   | > 50 failed/day or > 5 login blocks/day; any `identity-login` Long block                                                                             |
| 5   | **Registration pressure**  | signups/day 7 d + verify rate; top 5 domains 7 d; gmail dot-alias identities 7 d                                                                    | `users` — Q5                                                                                                               | daily digest + report                                      | > 20 signups/day; > 5 from one domain/day; dot-alias > 3 (D3 §7 heuristic)                                                                           |
| 6   | **Comment abuse**          | flags 7 d / total; flagged-visible comments; hidden comments; comments 7 d                                                                          | `comment_flags`, `post_comments` — Q6                                                                                      | daily digest + report                                      | any flag → triage; > 3 flags on one comment → hide; > 5 comments/author/day (report query)                                                           |
| 7   | **Mail health**            | suppression 7 d by reason; sends since boot; `webhook_configured`; bounce proxy = suppressions ÷ verification rows 7 d                              | `email_suppression`, `email_verifications`, mail atomics — Q7                                                              | daily digest + report                                      | any complaint; suppression rate > 5 % of sends; **webhook false → treat 0 as uninformative** (F12)                                                   |
| 8   | **Jobs + storage + cache** | last run age + deletions per job; DB bytes; WAL bytes; disk-cache hit rate; uptime                                                                  | `job_runs` (new) + file stat + web `/health/quran` — Q8/Q9/Q10                                                             | daily digest + endpoint                                    | job age > 2× interval or `eligible>0, deleted=0` ×2 → broken; WAL > 256 MB; DB > 1 GB; cache hits 0 with traffic or writable false                   |

**"Comment flags open" is not computable today** — `comment_flags` has no resolution state (E1 F4).
Slot 6 uses `flags_total` + `flags_7d` + `flagged_visible` (`post_comments.flags_count > 0 AND hidden = 0`)
as the proxy. A true open count needs the `state` column from E1 F4; add it when the report intake ships,
then this number becomes `WHERE state='new'`.

### 1.2 The new tables (the whole schema cost of this design)

```sql
-- Migration m0000NN_observability. Both tables are written ONLY by off-path tasks.
CREATE TABLE IF NOT EXISTS rate_limit_events (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    event      TEXT    NOT NULL,   -- 'block_start' | 'block_upgrade' | 'block_expired'
    key_class  TEXT    NOT NULL,   -- fixed enum, never the raw key
    scope      TEXT    NOT NULL,   -- 'Temp' | 'Long' | 'none'
    issued_at  INTEGER NOT NULL,   -- epoch seconds
    expires_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_rate_limit_events_issued ON rate_limit_events(issued_at);

CREATE TABLE IF NOT EXISTS job_runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    job         TEXT    NOT NULL,  -- 'retention'|'self_heal'|'digest'|'shadow_flush'|'wal_checkpoint'|'invariants'|'alert'
    started_at  INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    ok          INTEGER NOT NULL,
    detail_json TEXT,              -- {"user_sessions":12,...} or {"class":"p0_report","channel":"push","result":"ok"}
    error       TEXT
);
CREATE INDEX IF NOT EXISTS idx_job_runs_job ON job_runs(job, started_at);
```

`rate_limit_events` writer (inside the **existing** flush transaction, `rate_limit_store.rs:190-227`):
keep an in-memory `HashMap<String, i64>` of last-seen `block_until_at` per key in the flush task. For each
snapshot key: if `block_until_at > 0` and the map had 0/absent → `block_start` (or `block_upgrade` when
the scope changed Temp→Long); if the map had a value and the key is now absent from the snapshot →
`block_expired`. One `INSERT` per transition, inside the transaction that already runs every 10 s. The
map is bounded by the live key count and pruned with the snapshot. `key_class` is derived from the key
prefix (`quran-ban:` → `escalation-ban`; `ratelimit:` → `route`; `login:` → `identity-login`;
`comment:create:user:` → `identity-comment`; `forgot_password` → `identity-forgot`;
`newsletter:` → `identity-newsletter`; else `other`). **The raw key is never stored** — identity keys
contain emails, and this table has a 90 d retention; the live snapshot remains the place to see the
identity (existing behavior, existing retention caveat).

`job_runs` writer: every scheduled job wraps its body in one helper that inserts one row
(started/duration/ok/detail/error). `job='alert'` rows record each alert attempt (class, channel,
result) — this is how the digest can report "alerts undelivered: N".

Retention: `rate_limit_events` 90 d, `job_runs` 30 d, both deleted by the retention sweeper in
500-row batches (`DELETE FROM t WHERE id IN (SELECT id FROM t WHERE ts < ? LIMIT 500)` — no
`DELETE … LIMIT` on this SQLite build, A10 #15). Until the sweeper ships, a tiny delete in the flush
task's slow companion (every 6th tick) is acceptable — it is one indexed delete per minute.

### 1.3 Exact queries

**Q1 — enforcement now / 7 d (validated against the dev DB; returns zeros there)**

```sql
SELECT
  (SELECT COUNT(*) FROM user_bans
     WHERE revoked_at IS NULL AND (expires_at IS NULL OR datetime(expires_at) > datetime('now')))
    AS account_bans_active,
  (SELECT COUNT(*) FROM user_bans
     WHERE datetime(created_at) >= datetime('now','-7 days')) AS account_bans_created_7d,
  (SELECT COUNT(*) FROM user_bans
     WHERE revoked_at IS NOT NULL AND datetime(revoked_at) >= datetime('now','-7 days'))
    AS account_bans_revoked_7d,
  (SELECT COUNT(*) FROM enforcement_notices
     WHERE notice_delivery_state IN ('pending','no_contact_details')
       AND decided_at < CAST(strftime('%s','now') AS INTEGER) - 86400)
    AS notices_undelivered_over_24h,
  (SELECT COUNT(*) FROM rate_limit_state
     WHERE key LIKE 'quran-ban:%'
       AND block_until_at > CAST(strftime('%s','now') AS INTEGER)) AS ip_bans_active;
```

**Q2 — blocks by key class, 7 d**

```sql
SELECT key_class,
       COUNT(*) AS n,
       COALESCE(SUM(scope = 'Long'), 0) AS long_n,
       COALESCE(SUM(event = 'block_expired'), 0) AS expired_n
FROM rate_limit_events
WHERE issued_at >= CAST(strftime('%s','now') AS INTEGER) - 7*86400
GROUP BY 1 ORDER BY n DESC;
```

**Q3 — route 429s: current hot buckets (durable history is the digest snapshot)**

```sql
SELECT key, fixed_count, fixed_expires_at
FROM rate_limit_state
WHERE key LIKE 'ratelimit:%' AND fixed_count > 0
ORDER BY fixed_count DESC LIMIT 10;
```

plus in-process `route_429s{label}` atomics (increment in the `on_block` closure of each
`middlewares/rate_limit.rs` layer; labels are the fixed layer names — bounded set, no cardinality
risk). The digest stores yesterday's snapshot JSON in `app_constants` (`OPS_COUNTER_SNAPSHOT`) and
reports the delta.

**Q4 — auth pressure (blocks 7 d)**

```sql
SELECT key_class, COUNT(*) AS n
FROM rate_limit_events
WHERE key_class = 'identity-login'
  AND issued_at >= CAST(strftime('%s','now') AS INTEGER) - 7*86400
GROUP BY 1;
```

Raw failure volume = in-process `auth_login_failed` / `auth_login_banned` atomics added at the existing
WARN sites (`auth_v1/controller.rs:155,226`). **Per-account failure attribution is not available** — the
WARN carries IP only; the per-account signal is the block event (F11).

**Q5 — signups/day + domain + dot-alias**

```sql
SELECT date(created_at) AS day, COUNT(*) AS signups, COALESCE(SUM(is_verified),0) AS verified
FROM users WHERE datetime(created_at) >= datetime('now','-7 days')
GROUP BY 1 ORDER BY 1;

SELECT lower(substr(email, instr(email,'@')+1)) AS domain, COUNT(*) AS n,
       COUNT(DISTINCT CASE
         WHEN lower(substr(email, instr(email,'@')+1)) IN ('gmail.com','googlemail.com')
         THEN replace(substr(email, 1, instr(email,'@')-1), '.', '') END) AS dot_alias_identities
FROM users WHERE datetime(created_at) >= datetime('now','-7 days')
GROUP BY 1 ORDER BY n DESC LIMIT 5;
```

**Q6 — comment abuse**

```sql
SELECT
  (SELECT COUNT(*) FROM comment_flags
     WHERE datetime(created_at) >= datetime('now','-7 days')) AS flags_7d,
  (SELECT COUNT(*) FROM comment_flags) AS flags_total,
  (SELECT COUNT(*) FROM post_comments WHERE hidden = 1) AS hidden_comments,
  (SELECT COUNT(*) FROM post_comments WHERE flags_count > 0 AND hidden = 0) AS flagged_visible,
  (SELECT COUNT(*) FROM post_comments
     WHERE datetime(created_at) >= datetime('now','-7 days')) AS comments_7d;
```

**Q7 — mail**

```sql
SELECT reason, COUNT(*) AS n, COALESCE(SUM(permanent),0) AS permanent_n
FROM email_suppression
WHERE datetime(COALESCE(last_seen, created_at)) >= datetime('now','-7 days')
GROUP BY 1 ORDER BY n DESC;

SELECT COUNT(*) AS verification_mails_7d
FROM email_verifications WHERE datetime(created_at) >= datetime('now','-7 days');
```

plus in-process `mail_sent` / `mail_failed` / `mail_suppressed` / `mail_throttled` atomics and
`webhook_configured = CLOUDFLARE_EMAIL_WEBHOOK_SECRET` non-empty. **If webhook_configured is false, the
suppression numbers are uninformative** — the digest must print that next to them (F12).

**Q8 — jobs (last run per job)**

```sql
SELECT job, started_at, duration_ms, ok, detail_json, error
FROM job_runs
WHERE id IN (SELECT MAX(id) FROM job_runs GROUP BY job)
ORDER BY job;
```

**Q9 — storage**

```sql
SELECT (SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()) AS db_bytes;
```

plus file stats for `easyquran.db`, `-wal`, `-shm` (the API can stat its own `DATABASE_URL` path; the
report script uses `ls -l`). WAL > 256 MB = checkpoint stalled; DB > 1 GB = plan a backup rehearsal.

**Q10 — disk-cache hit rate** — web fetch: extend `/health/quran` with
`cache: {hits, misses, hit_rate}` from `getQuranDiskCacheStats()` (today it only tests writability).
The digest job fetches `http://web:8080/health/quran` (same Docker network, alias `web`) with a 2 s
timeout; failure → the digest prints `cache: unreachable`, not a crash.

---

## 2. Where they are displayed — decision and shape

**Decision: one admin JSON endpoint + one `just` recipe + the daily digest mail. Nothing else.**

Rejected, with reasons:

- **Enabling OTLP + Quickwit** — the backend is **not deployed**: there is no Quickwit service in
  `docker-compose.yml`, the default ingest URL is `http://localhost:7280`, export failures are dropped
  by the batch processors (no error surface), the counters are process-local (reset every redeploy),
  and `Counter` has no read API so the digest could not read them anyway. "Two env lines" produces a
  black hole (F2). Not a dashboard; a second system to operate.
- **The existing `analytics_v1` dashboard** — it is API-only, POST-only, has no abuse field, and
  `dashboard_summary` currently errors (missing `newsletter_subscribers`, D3 F9). Hanging the operator
  view on a 500 is how dashboards die. Fix F8 independently or delete the module.
- **A web admin UI** — cut by A8 §9 and E1 §11 for good reasons; a solo operator with 10 minutes a day
  does not open a console. The 3 a.m. tool is a phone + SSH + `just`.

**Implementation shape.**

```
rust/backend/api/src/modules/admin_abuse_v1/{mod.rs,controller.rs}
  GET /admin/abuse/v1/summary      ROLE_ADMIN, Cache-Control: no-store
  - one SQL statement: Q1..Q8 subselects (single round trip on the pool-of-1 connection)
  - merged in Rust with: in-process atomics (route 429s, auth, mail, saturation),
    fs stats (db/wal/shm), optional web fetch (disk cache), job ages (Q8)
  - output: flat JSON, no PII, key classes only
```

```json
{
  "generated_at": "2026-10-12T03:00:00Z",
  "since_boot": {
    "uptime_secs": 40211,
    "route_429s": { "quran-v1": 0, "auth": 0 },
    "auth": { "login_failed": 3, "login_banned": 0 },
    "mail": { "sent": 4, "failed": 0, "suppressed": 0, "throttled": 0 },
    "saturation": 0
  },
  "bans": {
    "account_active": 0,
    "account_created_7d": 0,
    "account_revoked_7d": 0,
    "ip_active": 0,
    "notices_undelivered_over_24h": 0
  },
  "blocks_7d": { "escalation-ban": 0, "route": 0, "identity-login": 0 },
  "signups": {
    "by_day": { "2026-10-10": 9 },
    "by_domain": { "example.com": 7 },
    "dot_alias_identities": 0
  },
  "comments": { "flags_7d": 0, "flags_total": 0, "flagged_visible": 0, "hidden": 0 },
  "mail": { "suppressions_7d": {}, "webhook_configured": false },
  "jobs": { "digest_age_secs": 3600, "retention_age_secs": null, "last_errors": [] },
  "storage": { "db_bytes": 356352, "wal_bytes": 0 },
  "disk_cache": { "hits": 0, "misses": 0, "hit_rate": null }
}
```

```bash
# justfile
abuse-report host="": _env
    # WAL-safe copy on the host, then the same SQL locally. Never query the live volume.
    bash scripts/abuse-report.sh {{host}}
```

`scripts/abuse-report.sh` (one page, no dependencies): `ssh $host` → `docker run --rm
-v easyquran_api_data:/data:ro -v /tmp:/out debian:trixie-slim sh -c 'sqlite3 "file:/data/easyquran.db?mode=ro" ".backup /out/prod-copy.db"'`
→ `scp` back → `sqlite3 -readonly prod-copy.db < scripts/abuse-report.sql` → print. `PRAGMA integrity_check`

- a freshness check (max `created_at` vs container `StartedAt`) before trusting the copy (D3 F7).

**Digest mail** (daily 03:00 UTC = 08:00 PKT, from the existing scheduler tick + `MailRouter`):
subject `[EasyQuran ops] 2026-10-12 · bans 0 · blocks 0 · flags 0 · jobs OK`; body = the 8 slots as
20–30 plain-text lines, with deltas since the previous digest (previous snapshot stored in
`app_constants` `OPS_COUNTER_SNAPSHOT`), `alerts sent N / failed M`, and the "cannot see" line. The
digest is the durable history: it is mail.

---

## 3. Alerting — minimum viable for one person

**Channels (in order of latency):**

1. **FCM push** to the operator's own device via the existing `notify_user` + `devices` + `rux-fcm`
   stack (works today, zero callers) — the only sub-minute channel.
2. **Mail** to `abuse@easyquran.fyi` (Cloudflare Email Routing → phone mail app) — the fallback and the
   digest.
3. **External uptime monitor** (free tier: Better Stack or UptimeRobot) on three URLs every 3–5 min:
   `https://easyquran.fyi/api/quran/health/ready` (expect `"ready":true` **after F1's DB check**),
   `https://easyquran.fyi/health/quran` (web), and `https://easyquran.fyi/api/quran/health/ops`
   (new; 503 when the last digest/self-heal run is stale — the heartbeat, F8). Alert contacts: mail +
   the monitor's own mobile app push.

**Triggers (the whole list; everything else is digest-only):**

| Trigger                                                        | Channel                              | Dedupe          | What the operator does                                                                              |
| -------------------------------------------------------------- | ------------------------------------ | --------------- | --------------------------------------------------------------------------------------------------- |
| report `kind IN (csam, threat)` submitted (E1 §6)              | push + mail, immediate               | per report id   | E1 runbook; content is auto-quarantined at intake                                                   |
| first account ban created since the last digest                | mail (digest) + push if P0           | per day         | verify notice sent + case exists (invariant check)                                                  |
| first escalation IP ban (`rate_limit_events` `escalation-ban`) | push                                 | per day         | read the incident; check the allowlist + scope                                                      |
| limiter saturation (`saturation_count()` increases)            | push                                 | per day         | raise `QURAN_ACTIVE_BAN_MAX` or review bans — new ban creation is silently declined while saturated |
| mail global cap tripped (E2 F-1)                               | push                                 | per day         | check for mail abuse / provider flagging                                                            |
| any job `ok=0` twice consecutively                             | push + mail                          | per job per day | SSH; `SELECT error FROM job_runs WHERE job=… ORDER BY id DESC LIMIT 1`                              |
| `notices_undelivered_over_24h > 0`                             | push                                 | per day         | DSA Art 17 breach; resend/record state                                                              |
| WAL > 512 MB or DB > 2 GB                                      | push                                 | per day         | checkpoint/backup rehearsal                                                                         |
| disk-cache `writable=false` or hits = 0 with traffic           | mail                                 | per day         | web container                                                                                       |
| a resolved client IP outside Cloudflare ranges (invariant 1)   | push                                 | per day         | ingress rule broken — the whole IP model is forgeable (E5 #1)                                       |
| digest missed > 26 h                                           | external monitor (`/health/ops` 503) | monitor's own   | the alerting itself is dead                                                                         |
| site down / `ready:false`                                      | external monitor                     | monitor's own   | restart/inspect                                                                                     |

**Dedupe.** `alert_key = (class, subject_id_or_day)`; at most one push per class per day; suppressed
duplicates are counted and printed in the digest. Last-alert state persists in `app_constants`
(`OPS_LAST_ALERT_<class>`) so a redeploy does not re-fire a day's alerts.

**Delivery observability.** Every alert attempt writes a `job_runs` row (`job='alert'`,
`detail_json={"class":…,"channel":"push|mail","result":"ok|failed|no_device"}`). The digest prints
`sent N / failed M` and the undelivered rows. Monthly: a deliberate test push (`just ops test-push`)
proves the FCM path still works. `notify_user` returning `Ok` with no push is a real failure mode (F5);
the alert helper must inspect the result, not trust `Ok`.

**3 a.m., one person.** Push arrives → phone → Termius SSH → `just incident queue` / `just incident
action …`. The P0 content quarantine is automatic (E1 §1.4) and does not depend on the operator being
awake. If the push path fails, mail is the fallback; if both fail, the next digest shows the queue and
the external monitor catches an outage. What cannot be automated: the ban, the notice, the NCMEC/LE
step — the runbook's rule applies (submit what you have within 24 h and amend). No 24/7 promise
anywhere.

**External monitor recommendation (concrete).** Better Stack free (10 monitors, 3-min interval, email +
mobile push) or UptimeRobot free (50 monitors, 5-min, email + app push). Create the three monitors
above; verify each alert channel with the provider's "test alert" button once; put the monitor account
credentials in the operator's password manager (not the repo). This is the only component outside the
repo that is load-bearing for alerting — document it in `deploy/README.md` as a named prerequisite.

---

## 4. Kill switches

**Conventions copied from the repo:** `env_bool` accepts `1|true|yes|on` (`config/env.rs:3-15`);
enabled-with-missing-prerequisite is a boot error, fail-closed (`EscalationConfig::from_env`,
`settings.rs:348-363`, `main.rs:867-877` exit(1)); the engine is attached as an `Option<Arc<dyn …>>` so
every other path compiles unchanged; flags are parsed in their own struct, never added to
`HttpSettings`/`RateLimitSettings` (B6 F10); the only no-restart pattern is atomics + `Notify`
(`route_blocker_config.rs`). **New in this design:** the runtime switch state is persisted in
`app_constants` and re-read at boot, because master pushes auto-redeploy (F3).

**The runtime ops switch module** (`services/ops_switches.rs`, modeled on `route_blocker_config.rs`):

- `AtomicBool` per switch + `Notify`; `GET /admin/ops/v1/status` and
  `POST /admin/ops/v1/switch/{name}` (`{"on": bool}`) behind `ROLE_ADMIN` (CSRF on the mutation).
- Every flip writes `app_constants` (`key='OPS_SWITCH_<name>'`, `source='ops'`, `updated_by=admin`);
  boot reads them back. A redeploy cannot silently re-enable enforcement.
- Switches govern **automatic/self-acting subsystems only**. Manual operator actions (ban create/
  revoke, hide, capture) are **never** behind a switch — you must be able to act during an incident.

| #   | Subsystem                          | Flag / switch                                                                    | Default            | Restart?                                                             | Failure mode when off                                                                                                   |
| --- | ---------------------------------- | -------------------------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| 1   | Quran IP escalation (`quran-ban:`) | `QURAN_BAN_ESCALATION_ENABLED` + `QURAN_BAN_ALLOWLIST` (exists)                  | `false`            | boot; enabled without allowlist = exit(1)                            | no automatic IP bans; manual account bans unaffected; abuse visible only in `rate_limit_events`/logs                    |
| 2   | Active-ban capacity                | `QURAN_ACTIVE_BAN_MAX` (exists)                                                  | `2000`             | **boot** — read once at flush-task spawn (`rate_limit_store.rs:134`) | over capacity, new ban creation is silently declined + saturation counter; the alert in §3 makes it visible             |
| 3   | Device collection                  | `DEVICE_COLLECTION_ENABLED`                                                      | `false`            | boot                                                                 | no `device_records` rows; shadow evaluator has no data                                                                  |
| 4   | Device enforcement                 | `DEVICE_ENFORCEMENT_ENABLED`                                                     | `false`            | boot (deliberately not flippable)                                    | shadow only; nothing is blocked                                                                                         |
| 5   | Device evaluator pause             | `ops.pause.shadow_eval` (runtime)                                                | `false`            | **no restart**                                                       | evaluator returns pass; collection continues                                                                            |
| 6   | Device pepper                      | `DEVICE_KEY_PEPPER` / `_PREV`                                                    | unset              | **process-immutable**; rotation = restart                            | changing it orphans records; `_PREV` is the verification fallback                                                       |
| 7   | Registration admission             | `REGISTRATION_ADMISSION_ENABLED` + `REGISTRATION_ADMISSION_MODE=shadow\|enforce` | `false` / `shadow` | boot                                                                 | shadow = counts only, no blocks; enforce requires Turnstile configured (prod gate)                                      |
| 8   | Automatic ban proposals            | `BAN_AUTOPROPOSE_ENABLED`                                                        | `false`            | boot or runtime (`ops.pause.auto_enforce`)                           | no automated proposals; manual ban path always live                                                                     |
| 9   | Manual ban path                    | none — by design                                                                 | —                  | —                                                                    | must always work; a switch here is an outage of the incident-response capability                                        |
| 10  | Mail per-recipient/provider caps   | `MAIL_RATE_LIMIT_ENABLED` (exists)                                               | `true`             | boot                                                                 | caps off; the provider's abuse system becomes the cap (account flagged)                                                 |
| 11  | Mail global/domain caps (E2 F-1)   | `MAIL_GLOBAL_CAP_ENABLED`                                                        | `true`             | boot                                                                 | unbounded transactional spend; 144k mails/day/IP is the current shape                                                   |
| 12  | Web-origin limiter (E6 P0.2)       | `WEB_ORIGIN_LIMITER_ENABLED` (web env)                                           | `true`             | boot (web container; ~30 s redeploy)                                 | origin starvation returns; false positives resolved by flipping + redeploy — document this as the known non-runtime gap |
| 13  | SSR miss budget / internal bucket  | `QURAN_SSR_MISSES_PER_MINUTE` (web), `QURAN_INTERNAL_REQUESTS_PER_MINUTE` (api)  | 600 / 600          | boot                                                                 | raising = soft-off; the translated reader degrades to the internal 429 → empty 200 (E6 F1)                              |
| 14  | Retention sweeper                  | `RETENTION_SWEEPER_ENABLED` + `RETENTION_SWEEPER_DRY_RUN`                        | `false` / `true`   | boot + runtime `ops.pause.retention`                                 | off = nothing deleted (safe direction) but storage grows; **must not ship without the hold probe** (E1 F7)              |
| 15  | Digest / alerting                  | `OPS_DIGEST_ENABLED`                                                             | `true`             | boot                                                                 | operator blind — the absence itself must alert (external monitor on `/health/ops`)                                      |
| 16  | Edge attestation (B6)              | `EDGE_ATTEST_REQUIRED`                                                           | `false`            | boot                                                                 | no edge claims required; device/edge work unaffected                                                                    |

**How the switch is validated (prod-gate style).** `OPS_SWITCH_*` values are parsed with `env_bool`
semantics; an unknown switch name in the admin route → 400; `REGISTRATION_ADMISSION_MODE` outside
`{shadow,enforce}` → exit(1) at boot; `DEVICE_ENFORCEMENT_ENABLED=true` without the collection flag,
pepper, allowlist and rule-pack version → exit(1) before the DB opens (B6 §2). Every flip writes an
`audit_logs` row (`action='ops.switch'`) — the switch itself is auditable.

---

## 5. The "is the invariant holding?" checks

Each row: the invariant, the runtime proof, where it runs, and what failure does.

| #   | Invariant (from the plan)                                 | Proof                                                                                                                                                                                                                                                                                                                                                                                                               | Where                                   | Failure behavior                                                                      |
| --- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------- |
| 1   | **No forged IP** (E5 #1, B6 §8.1)                         | (a) boot: if `IP_SOURCE=CfConnectingIp` and `TRUSTED_PROXY_CIDRS` contains any non-private CIDR → **refuse to boot**; log the CIDR list; (b) hourly: counter of requests whose resolved client IP is outside the Cloudflare published ranges → any non-zero → push alert. The ranges must be a repo-committed file (refreshed quarterly; the current state is an owner-run manual step, `deploy/README.md:377-409`) | boot + hourly (`job_runs` `invariants`) | boot refusal = deploy fails loudly (fail-closed); hourly non-zero = P0 security alert |
| 2   | **No ban without a notice** (E1 F8/F9, DSA Art 17)        | SQL: active `user_bans` rows with no `enforcement_notices` row, or notice `pending`/`no_contact_details` older than 24 h → must be 0                                                                                                                                                                                                                                                                                | boot + hourly + digest                  | > 0 → push alert; the ban stays (revoking it is a separate decision)                  |
| 3   | **No device record without expiry** (DESIGN v0→v1 row 22) | SQL: `device_records WHERE expires_at IS NULL OR expires_at <= created_at` → must be 0; schema `expires_at NOT NULL`                                                                                                                                                                                                                                                                                                | boot + daily                            | > 0 → mail alert; the sweeper cannot delete what has no expiry                        |
| 4   | **No reader-path rate limit on identity** (B6 §3.1, E6)   | (a) guard test (source scan): reader route handlers/limiter attach points contain no identity/device key; (b) runtime counter `device.shadow.reader_route_evals` must stay 0 (increment if the evaluator ever runs on a reader route)                                                                                                                                                                               | guard test in CI; counter checked daily | test fails the build; counter > 0 → push alert (the device axis leaked onto readers)  |
| 5   | **Retention actually deleting** (E1 F7, C4)               | `job_runs`: for `job='retention'`, if `eligible > 0` and `deleted = 0` for 2 consecutive runs → alert; monthly SQL: `user_sessions` older than 90 d with `ip_address IS NOT NULL` must fall to the held-only count                                                                                                                                                                                                  | daily + monthly                         | alert; the sweeper is not working or a hold is silently blocking everything           |
| 6   | **Enforcement actions audited** (E1 F1/F2)                | SQL: every `user_bans` row has an `audit_logs` row (`action='enforcement_action'`, `resource_type='ban'`, `resource_id=CAST(ban.id AS TEXT)`) within 60 s; `enforcement_notices` count == bans count                                                                                                                                                                                                                | hourly + digest                         | > 0 → mail alert; the appeal/LE record is incomplete                                  |
| 7   | **Escalation allowlist respected** (B6 §3.4 G5)           | unit test: an allowlisted unit never escalates; runtime: `rate_limit_events` `escalation-ban` rows whose unit is inside the allowlist → 0 (requires storing the unit for `quran-ban` only — a CIDR, not PII)                                                                                                                                                                                                        | unit test in CI + daily                 | any row → push alert                                                                  |

**The rule that makes these checks honest:** an invariant with no runtime probe is a comment. Rows 2, 3,
5, 6 are pure SQL and must be in the digest even when they are zero — a check that is never printed is a
check that silently stopped running.

---

## 6. The self-heal jobs

One new task (`services/ops_jobs.rs`, spawned after `start_scheduler`, same `tokio::spawn` precedent as
the hourly session sweep `main.rs:761-775`), with its own interval and a runtime pause. Every job body
is wrapped by the `job_runs` writer. Failure behavior for every job: warn + one `job_runs` row;
**two consecutive failures → alert** (§3).

| Job                                        | Cadence                     | What it does                                                                                                                                                                                                  | Failure behavior                                                                                             |
| ------------------------------------------ | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Expired bans                               | hourly                      | `user_bans` with `expires_at <= now AND revoked_at IS NULL` → set `revoked_at`, `revoked_by=NULL`, reason `'expired'`; write `audit_logs`; count in `detail_json`. Never rotates the secret on revoke (E4 §2) | retry next tick; 2 misses → alert; `eligible>0, processed=0` → alert                                         |
| Invariant checks                           | boot + hourly               | §5 rows 1, 2, 6, 7                                                                                                                                                                                            | boot refusal for row 1; alerts otherwise                                                                     |
| Queue aging                                | daily                       | `reports`/`cases` open past SLA (E1 §2), oldest P0/P1 age → digest; P0 > 24 h → push                                                                                                                          | alert; the job never auto-actions                                                                            |
| Holds expiring                             | daily                       | `legal_holds.preserve_until < now + 14 d` → digest. **Never auto-releases** (E1 §5.1)                                                                                                                         | alert; a lapsed hold is an evidence-destruction risk                                                         |
| Orphaned evidence                          | weekly                      | `case_evidence` with no `cases` row, or case closed + past `expires_at` → count; the retention sweeper deletes at retention                                                                                   | count only; anomaly → alert                                                                                  |
| Stale shadow observations                  | hourly (own flush, B6 §3.2) | delete `device_shadow_observations` older than 14 d in ≤500-row batches                                                                                                                                       | 3 consecutive failures → alert                                                                               |
| WAL growth                                 | daily                       | stat `-wal`; if > 256 MB run `PRAGMA wal_checkpoint(PASSIVE)`, then TRUNCATE if still large; report sizes                                                                                                     | size grows 2 days in a row → alert; never blocks the flush (runs on the shared connection in a quiet window) |
| `rate_limit_events` / `job_runs` retention | daily                       | delete > 90 d / > 30 d in ≤500-row batches                                                                                                                                                                    | failure → alert                                                                                              |
| Session expiry sweep                       | hourly (exists)             | tower-session `delete_expired`                                                                                                                                                                                | warn (existing); digest counts                                                                               |
| Digest                                     | daily 03:00 UTC             | §2; stores `OPS_LAST_DIGEST_AT` + `OPS_COUNTER_SNAPSHOT` in `app_constants`                                                                                                                                   | 2 misses → external monitor on `/health/ops`                                                                 |
| Alert retry                                | on failure                  | retry push once, then mail                                                                                                                                                                                    | recorded in `job_runs` `job='alert'`; digest reports undelivered                                             |

**What must NOT self-heal:** bans (only expire on their own timestamp), holds (never auto-release),
cases (never auto-close a P0), and any content (never auto-delete). Self-healing is for bookkeeping,
not for enforcement decisions.

---

## 7. Attack the design

**A. What breaks the observability.**

1. **Log volume.** Promoting the 429 `debug!` to `warn!` (D3 F4) without dedupe turns a scrape into one
   line per request. The code already has the one-event-per-window primitive: `count == max_requests + 1`
   (`layer.rs:291-300`). Log at that equality only, and add a `logging:` block to compose
   (`max-size: 10m`, `max-file: 5`) or the json-file grows unbounded (today: no `logging:` key anywhere,
   F7). The log window remains container-lifetime (redeploy resets it) — never build a durable number on
   logs alone.
2. **DB cost on pool = 1.** The summary is one statement with ~14 subselects; on today's row counts it
   is <5 ms, but `users`/`post_comments` scans grow. Mitigations: single statement, no joins to `users`
   for big aggregates, `LIMIT` on every per-key list, indexes when a scan shows up (`users(created_at)`,
   `comment_flags(created_at)`, `email_suppression(last_seen)`, `user_bans(created_at)`), digest in a
   quiet window, and the heavy report runs on a **copy** (`just abuse-report`), never the live volume.
   The endpoint is admin-only and rate-limit-exempt by role, not by path — do not put it on a
   hot-polling loop.
3. **Cardinality.** The design adds **zero** metric labels: route labels are the fixed layer names,
   `key_class` is a 7-value enum, domains are top-5 only. The existing `http_metrics` template-path
   discipline stays. The failure mode to guard: someone adds `client_ip` or `email` to a counter label —
   document the ban in the module comment, next to the existing cardinality comments.
4. **The alert channel failing silently.** FCM `notify_user` returns `Ok` when FCM is absent or the
   operator has no devices (F5); a suppressed operator mailbox would also drop the mail (the router
   returns `Ok` for suppressed transactional sends — ironic for the alert path). Mitigations: inspect
   the result and record it; mail to a different domain (personal mailbox) as the fallback; monthly
   test push; the `/health/ops` heartbeat watched by the external monitor; the digest prints
   `alerts failed M`.
5. **The operator ignoring it.** The only real defense is volume discipline: push only for P0-class and
   first-of-kind events, everything else once-daily in one mail with deltas. A digest with 40 numbers is
   not read. If the operator ignores a P0 push, nothing else in this design works — state that honestly
   in the runbook.

**B. The 10-minute cut.** If the operator has 10 minutes a day, the digest is **5 numbers + 1 line**:
(1) account/IP bans active + created/revoked 7 d, (2) blocks by class 7 d, (3) signups/day + top domain,
(4) comment flags 7 d, (5) storage (DB/WAL) — plus `jobs: OK/2 failures`. Everything else (auth
pressure, mail suppression detail, dot-alias, disk cache, route 429 detail) moves to the weekly
`just abuse-report`. The push path is unchanged: P0 reports and invariant failures only.

**C. What I would not build even now.** OTLP/Quickwit; a web dashboard; per-request counters; a durable
per-route 429 history table (the in-process counter + digest snapshot + logs are enough at this volume);
alert routing rules (one operator, one channel pair); on-call schedules; a status page.

---

## 8. The honest minimum — required before any enforcement ships vs what can wait

**REQUIRED before the ban contract (or any enforcement) is enabled in production:**

1. **The audit writer** — already inside the ban contract (E1 F7/E4); it is the enforcement ledger.
2. **`rate_limit_events`** (the block ledger) — without it, "is enforcement firing at all" is
   unanswerable after any window closes. Written by the existing flush task, no request-path writes.
3. **The 429 `debug!→warn!` promotion + the `logging:` block** — one word + five lines; without them
   reader-path enforcement is invisible and the log grows unbounded.
4. **The ban/notice invariant check** (§5 row 2) at boot + hourly — a ban without a notice is the Art 17
   breach; the check is four lines of SQL.
5. **One readable output** — `GET /admin/abuse/v1/summary` **or** the digest + `just abuse-report` if the
   endpoint is cut. The operator must be able to answer "what is blocked right now" in one command.
6. **The digest + one working alert channel with recorded delivery** — enforcement without notice to the
   operator is unattended automation.
7. **The external uptime monitor** (3 URLs, free tier) — because every enforcement change can break the
   site, and the site is the only thing the operator currently finds out about by accident.
8. **The persisted runtime pause for automatic enforcement** (`ops.pause.auto_enforce` + the
   `app_constants` persistence) — a false-positive storm must be stoppable in seconds and must survive
   the next auto-redeploy.

**Can wait (real but not blocking):** OTLP/Quickwit (needs a collector first — F2); the full metric
family; disk-cache stats plumbing (cheap, nice, not safety-critical); WAL/DB size alerts (add with the
digest); route-429 durable history; per-account failed-login attribution (F11); the mail webhook
verification (F12 — verify it or mark the number uninformative); retention-sweeper observability (ships
**with** the sweeper and the holds, never before, E1 F7).

**The one-sentence minimum:** _the operator must be able to see, in one place, that enforcement is off
or firing, that no ban exists without a notice, and that the jobs that keep the system honest are still
running — and must be able to stop automatic enforcement without a deploy._

---

## Findings

All findings below are **NEW** relative to C3/D3/E1/E5/A8/B6/C6 (each is an observability or
kill-switch fact, or a correction of a prior claim). Every repo claim is `file:line`; SQL was validated
read-only against the dev DB.

### F1 — The public readiness endpoint is a static `ready:true`; the only DB check is unreachable from outside, so the recommended external monitor cannot see the most likely 3 a.m. failure. **NEW**

**Severity: Critical.**
**Evidence:** `quran_v1/controller.rs:1037-1060` — `health_ready` touches no DB; it returns constants
(`VERSE_COUNT`, `SURA_COUNT`) and auth flags. `router.rs:212-234` — `/healthz` does `sea_db.ping()`, but
`docker-compose.yml:184` excludes `/api/healthz` from the public Traefik router and
`deploy/README.md:124-126` documents it as Docker-network only. The api healthcheck (`:171-176`) curls
both from inside the container, so Docker restarts the container on DB failure — but the **public** edge
keeps serving `{"ready":true}` for a locked, corrupt, or lost DB. The external monitor (the only
3 a.m. signal that exists) therefore proves process liveness, not service readiness.
**Fix:** add `db: ok|error` (existing `sea_db.ping()`) and `quran_db: ok` (open one immutable read) to
the `/quran/health/ready` payload; keep `/healthz` internal. Then the monitor's assertion
(`"ready":true` + db ok) is meaningful. Cost: one ping per health poll (health bucket 120/min,
`settings.rs:463`).

### F2 — "Enable OTLP with two env lines" is wrong: there is no collector, the default ingest URL is localhost, export failures are dropped, and the counters cannot be read back anyway. **NEW**

**Severity: High.**
**Evidence:** `utils/telemetry.rs:247-252` (gate), `:264-268` (default `QUICKWIT_INGEST_URL =
http://localhost:7280`), `:301-308` (`.expect` only at init; a running exporter that cannot connect
drops batches in the batch processor with no error surface), `docker-compose.yml` (no Quickwit/collector
service, no OTEL vars), `.env:39` (`OTEL_SDK_DISABLED=true` — **not read by this code**; the only gate is
`ENABLE_QUICKWIT_OTEL`, so the env var is misleading). `opentelemetry::metrics::Counter` has no read API
(`telemetry.rs:343-537` statics), so even a live exporter would not let the digest read the numbers.
B6 F7 ("set two env vars as an operator prerequisite") and C3 F3 understate this: two env lines produce a
silent black hole plus redeploy-reset counters.
**Fix:** do not enable OTLP until a collector exists with storage/retention and a named owner; make the
digest/admin endpoint the source of truth. If it is ever enabled, require `QUICKWIT_INGEST_URL` to be
non-default and fail the boot when it is unset (fail-closed, matching `EscalationConfig`).

### F3 — The only no-restart switch is in-memory and not persisted, while every master push auto-redeploys: any pause flipped during an incident is silently reverted. **NEW**

**Severity: High.**
**Evidence:** `route_blocker_config.rs:12-20` (`lazy_static` atomics; no persistence), `main.rs:694-756`
(the loop reads them), `deploy/README.md:21-28,180-181` (master push → images → Dokploy
`compose.redeploy`; pull-only stack). So `POST /admin/route/v1/sync_interval/pause` — and any new
runtime switch modeled on it — survives only until the next merge to master. The operator pauses
enforcement because of a false-positive storm; the next push re-enables it with no signal.
**Fix:** persist switch state in `app_constants` (`OPS_SWITCH_<name>`, `source='ops'`, `updated_by`),
read at boot; apply the same to the existing route-blocker pause. Write an `audit_logs` row on every
flip.

### F4 — The disk-cache hit rate is computed nowhere and the web health endpoint only tests writability: the E6 disk-cache-loss failure mode is invisible. **NEW**

**Severity: High.**
**Evidence:** `web/src/lib/server/quran-disk-cache.ts:138-142,223` — `stats()` returns
`{hits, misses}` and `getQuranDiskCacheStats()` is exported with **zero callers** (grep).
`web/src/routes/health/quran/+server.ts:20-34` — readiness checks `buildManifestPresent()` and
`diskCacheWritable()` only; a cache that is writable but never hit (E6 F2: any cookie defeats the disk
cache; GA4 `_ga` makes that the default for returning readers) reports `ready:true` forever.
**Fix:** extend the readiness payload with `cache:{hits,misses,hit_rate}`; the digest fetches
`http://web:8080/health/quran` (same Docker network) and prints the rate; alert when `writable=false`
or `hits=0` with non-zero page traffic. No new endpoint needed.

### F5 — The FCM alert primitive reports success when it delivered nothing; an alert path built on it can silently fail. **NEW**

**Severity: High.**
**Evidence:** `services/notification/mod.rs:63-69` — FCM client `None` → `warn!` + `Ok(model)`;
`:86-89` — no devices → `Ok(model)`; `:102-115` — per-device send failures are logged and the function
still returns `Ok`. E1 F10 recommends this exact primitive for the 3 a.m. push. A digest/alert design
that trusts `Ok` cannot tell "pushed" from "no device registered".
**Fix:** the alert helper inspects the dispatch result (device count, per-device outcome) and records it
in `job_runs` (`job='alert'`); the digest prints undelivered alerts; monthly test push; a `devices` row
for the operator's phone is a named setup step.

### F6 — `limiter.saturation` is written to a dead counter, so capacity saturation (new ban creation silently declined) is invisible. **NEW**

**Severity: High.**
**Evidence:** `rate_limit_store.rs:149-159` — when active blocks exceed `QURAN_ACTIVE_BAN_MAX`, the
flush calls `store.incr_saturation()` and the comment states "New ban creation is declined at the L1
gate"; the only reader is the OTel counter (`telemetry.rs:443-476`, `abuse_limiter.rs:11-31`), whose
export is off (F2). `store.saturation_count()` (`store.rs:220-226`) is readable but nothing reads it.
**Fix:** a saturation transition writes a `rate_limit_events` row (`key_class='saturation'`) and raises
the §3 push alert once per day; expose `saturation` on the admin endpoint.

### F7 — No `logging:` block in compose: the json-file grows unbounded, and the promoted 429 WARN would be its first high-volume line; the log window is container-lifetime. **NEW**

**Severity: Medium-High.**
**Evidence:** `docker-compose.yml` — full read, no `logging`, `max-size`, `max-file`, or `log-opts`
anywhere; `RUST_LOG: ${RUST_LOG:-info}` (`:129`); containers recreated on every master push
(`deploy/README.md:21-28`), so `docker logs` reaches only the current container (D3 F2). The 429
promotion (`layer.rs:323`) is the cheapest abuse sensor (C3 F4) but a scrape produces one line per 429
unless gated on `count == max_requests + 1` (`layer.rs:291-300` — the primitive exists).
**Fix:** `logging: {driver: json-file, options: {max-size: "10m", max-file: "5"}}` on api + web; gate the
429 log on the equality; document that log-derived numbers have a days-long window.

### F8 — Nothing monitors the monitor: the digest and self-heal jobs can die silently and the operator finds out by accident. **NEW**

**Severity: High.**
**Evidence:** `services/scheduler.rs:9-25` — one job, no heartbeat, no failure escalation;
`deploy/README.md` has no monitoring section (grep); E1 §6 recommends a digest but no proof it ran. A
crashed task, a bad migration, or a full disk stops the digest and the queue silently.
**Fix:** a public `GET /quran/health/ops` returning 503 when `now - OPS_LAST_DIGEST_AT > 26 h` or the
last self-heal run is > 2 intervals old (read from `app_constants`/`job_runs`); the external monitor
watches it. This is the only mechanism that detects "the alerting is dead".

### F9 — Every new counter must ride the existing off-path tasks; the repo's own rule forbids request-path writes and the single connection makes violations expensive. **NEW**

**Severity: Medium-High.**
**Evidence:** `rate_limit_store.rs:1` ("Do NOT add a DB write to the request hot path"); B6 F9; A7
(1 fsync/commit ≈ ~950 req/s ceiling; the 10 s flush holds the connection 60–200 ms). A naive
"increment a row per 429" design (the obvious implementation) violates both and degrades the reader.
**Fix:** in-process `AtomicU64`s on the request path (readable — unlike OTel counters), transition
detection inside the existing flush transaction, one `job_runs` row per job run, digest on a quiet
window. This is a design constraint, not a suggestion.

### F10 — The block ledger does not exist and cannot be derived: expired rows are deleted and per-route 429s never set a block, so "blocks by key class" and "429s by route" have no durable source. **NEW**

**Severity: High.**
**Evidence:** `rate_limit_store.rs:190-197` — the 10 s flush opens with `DELETE FROM rate_limit_state
WHERE fixed_expires_at <= ? AND block_until_at <= ?`; `store.rs:176` prunes after. `layer.rs:315-330` —
the per-route layer returns 429 when `count > max_requests` and **never calls `set_block`**, so route
429s leave no `block_until_at` and no block row; only the abuse limiter (`abuse.rs:100-115`) sets
blocks. C3 F1 established the snapshot-not-ledger problem; the new fact is that the transition detector
must be scoped to abuse-limiter blocks, and route 429s need a separate in-process counter.
**Fix:** `rate_limit_events` written on transitions inside the existing flush transaction (§1.2) +
in-process `route_429s` counters incremented in the `on_block` closure of the rate-limit layers
(`middlewares/rate_limit.rs:14-19` — no crate change needed).

### F11 — "Failed logins by identity class" is only partially computable: the WARN carries IP but not account, and the per-account bucket purges ≤ 15 min. **NEW**

**Severity: Medium.**
**Evidence:** `auth_v1/controller.rs:226` — `warn!(client_ip = %secure_ip, "Invalid credentials")`;
`:129` — `info!(client_ip, "Login attempt")`; the `login:{email}` limiter bucket is deleted by the
flush once its window and block expire (`rate_limit_store.rs:190-197`); D3 F3 established the log lines
exist. Per-account targeting (one account, rotating IPs) is invisible in both the DB and the log.
**Fix:** count login blocks into `rate_limit_events` (`key_class='identity-login'`, **no raw email**);
add readable atomics at the two WARN sites for raw volume; accept and document the per-account blind
spot until a hashed-email field is added to the failure line (a one-line change with a privacy review).

### F12 — The mail numbers read 0 when the system is blind: the suppression webhook secret is optional and empty-allowed, so bounce/complaint rates can be structurally zero. **NEW**

**Severity: Medium.**
**Evidence:** E2 §1 — `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` is built with `unwrap_or_default()` and the
webhook is undeployable without it (`main.rs:80-81`); `email_suppression` fills only from the webhook
(`mail_v1/mod.rs:27`) and sync-bounce suppression (`router.rs:206-221`, permanent bounces only).
A digest that prints "bounces: 0" with the webhook unconfigured reports the absence of an instrument,
not the absence of bounces.
**Fix:** the digest prints `mail_webhook_configured: true|false` next to the suppression numbers and
treats 0 as uninformative when false; configure the webhook (or drop the metric) as part of the mail
work (E2 F-1).

---

## Appendix — verification commands (all read-only)

```bash
# the primitives
sed -n '247,268p' rust/backend/api/src/utils/telemetry.rs      # OTLP gate + localhost default
sed -n '1,49p'   rust/backend/api/src/middlewares/http_metrics.rs
sed -n '12,20p'  rust/backend/api/src/services/route_blocker_config.rs
sed -n '279,330p' rust/backend/api/crates/rux-request-gate/src/layer.rs   # 429 without set_block
sed -n '100,115p' rust/backend/api/crates/rux-request-gate/src/abuse.rs   # block WARN
sed -n '180,229p' rust/backend/api/src/services/rate_limit_store.rs       # DELETE-first flush
sed -n '1037,1060p' rust/backend/api/src/modules/quran_v1/controller.rs   # static ready
sed -n '212,234p' rust/backend/api/src/router.rs                          # /healthz DB ping
grep -n "logging" docker-compose.yml                                     # 0 hits
grep -rn "getQuranDiskCacheStats" web/src                                 # 1 hit, 0 callers
grep -rn "notify_user(" rust/backend/api/src                             # 0 callers

# SQL validated against the dev DB (read-only, immutable=1; all zeros/no-ops there)
DB=rust/data/easyquran.db
sqlite3 "file:$DB?immutable=1" "$(sed -n '/^-- Q1/,/^-- Q9/p' <the queries in §1.3>)"
sqlite3 "file:$DB?immutable=1" "SELECT (SELECT page_count*page_size FROM pragma_page_count(), pragma_page_size());"
```

_End of F4._
