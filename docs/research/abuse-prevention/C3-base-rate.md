# C3 — The measured abuse base rate for this app

Auditor: base-rate measurement pass. Date: 2026-10-12.
Inputs: `B10-falsification.md` §6 Q1 ("what is the measured abuse base rate for _this_ app? No report
produced one"), `B2-contrarian.md` §5 (the C1/C2/C3 falsifiable condition).
Method: read every abuse-relevant code path in `/Users/hmziq/os/easyquran`; read the live dev SQLite
database read-only (`immutable=1`, aggregate counts only, no writes); probed the deployed origin with
read-only requests to establish deployment state. Every repo claim below is a `file:line`.

**Answer up front.** There is **no measured abuse base rate**, and the reason is not that nobody
looked — it is that **three of the four required numbers are structurally absent**, not merely
unrecorded:

1. **Production exists and is live** (`easyquran.fyi`, Cloudflare → Traefik → web+api, health endpoint
   reports `auth.enabled=true` with 3 ready OAuth providers). **None of its data is in this repo.**
2. **The only abuse ledger is a moving snapshot.** `rate_limit_state` deletes a row the moment both its
   fixed window and its block expire (`rate_limit_store.rs:190-197`). It has no `blocked_at`, no
   cumulative block counter, no history table. A block that ended 11 seconds ago is gone.
3. **The one automated abuse response is switched off in production.**
   `QURAN_BAN_ESCALATION_ENABLED` defaults to `false` (`config/settings.rs:349`), is **absent from
   `docker-compose.yml`**, and requires `QURAN_BAN_ALLOWLIST` to be set before it can be enabled at all
   (`:356`). So the number of bans _automatically_ issued in production is **zero unless the deployed `.env` enables
   the engine** (the server `.env` is not in the repo), and the
   only way a ban exists is a human `DELETE`-free admin call that **has no writer**: `user_bans` has
   zero create/revoke callers and no admin route (`B2 F7`, re-verified: the only `user_ban` reference
   outside `db/sea_models/user_ban/` is a read at `services/auth.rs:531`).
4. **C1 and C2 cannot be computed today because their denominators do not exist.** There is no
   "confirmed ban-evasion case" record anywhere, and no device token at all. C1's denominator is 0 and
   C2's numerator requires a link table that has not been created.
5. **The local database is dev/test data and contains zero abuse events** — and it is _not_ seed data
   either: the seed generator was never run against it (see §2).

The cheapest path to a real number is **not new code**. It is: Cloudflare zone analytics (the
denominator) + GA4 (`G-ZYL6HY24W6`, already live) + a read-only copy of the production SQLite volume +
one `grep` over the container stdout log. **A few hours the first time (access setup included), no deploy, no schema change.** §5 gives the
exact queries.

---

## 1. Inventory of every data surface that could measure abuse

Legend — **Kind**: REAL = production traffic · DEV = local development traffic only · FABRICATED =
seed generator output (never run in this DB) · DERIVED = computed from another surface · NONE = the
surface cannot measure abuse at all.
**Retained**: how long the row survives. **Queryable today**: can an operator get the number now,
and how.

### 1a. Request-path limiters (the only thing that sees abuse _as it happens_)

| #   | Surface                                                                                                                                                 | What it records                                                                                                                                                                                                                                      | Retained                                                                                                                                                    | Queryable today                                                                                                                                                                                                                                                      | Kind                                                                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1   | `rate_limit_state` (L2 mirror of the in-memory gate store)                                                                                              | `key`, `fixed_count`, `fixed_expires_at`, `block_until_at`, `block_scope` for **currently active** buckets only                                                                                                                                      | **≤15 min for a plain fixed window; ≤ the block duration (1 h Temp / 24 h Long / 7 d quran-ban) for a block. Then DELETEd** (`rate_limit_store.rs:190-197`) | Yes: `SELECT key, fixed_count, block_until_at, block_scope FROM rate_limit_state`. **But it is a snapshot, not a ledger.**                                                                                                                                           | DEV rows present (3), REAL rows exist in prod                                                  |
| 2   | `rate_limit_state` key namespace                                                                                                                        | Three families: `ratelimit:{ip\|internal-webssr\|unknown}:{route-or-label}` (per-route ceiling), `quran-ban:{ip/32\|/64}` (escalation ban), `login:{email}`/`comment:create:user:{id}`/`forgot_password*`/`newsletter:*` (per-identity abuse limits) | as above                                                                                                                                                    | Yes — the key namespace _is_ the taxonomy. `LIKE 'ratelimit:%'` vs `LIKE 'quran-ban:%'` vs the rest                                                                                                                                                                  | DEV: all 3 rows are `ratelimit:*`, **0 blocks**, one key is the unit-test IP `203.0.113.10/32` |
| 3   | `limiter.checks` / `limiter.allowed` / `limiter.blocked` / `limiter.blocked.temp` / `limiter.blocked.long` OTel counters (`utils/telemetry.rs:443-476`) | Cumulative in-process block counts, split Temp/Long                                                                                                                                                                                                  | **Process lifetime only.** In-process counters, reset on every restart/deploy                                                                               | **No.** `ENABLE_QUICKWIT_OTEL` appears **only** in `utils/telemetry.rs` — grep across `*.yml`, `*.env*`, `*.md`, `*.toml`, `*.sh` returns **0 hits**; `.env` has `OTEL_SDK_DISABLED=true`; `docker-compose.yml` sets no OTEL/Quickwit var. Incremented, then dropped | NONE (dead code in practice)                                                                   |
| 4   | `"Request blocked by abuse limiter"` WARN (`crates/rux-request-gate/src/abuse.rs:109`)                                                                  | One line per per-identity block, with the key in the span (`scope = %key_prefix`), `retry_after`, `short_count`, `long_count`                                                                                                                        | Container stdout → whatever Dokploy retains                                                                                                                 | **Yes, by grep** — this is a real, working, already-emitted signal. `RUST_LOG=info` includes WARN                                                                                                                                                                    | NONE locally (abuse limiter is skipped in non-prod); REAL in prod                              |
| 5   | `"Rate limit exceeded"` DEBUG (`rux-request-gate/src/layer.rs:323`)                                                                                     | One line per per-route 429, with `identity`, `path`, `count`, `max_requests`                                                                                                                                                                         | Container stdout                                                                                                                                            | **No at the default log level.** `docker-compose.yml` sets `RUST_LOG=info`; DEBUG is dropped. **The 600/min Quran ceiling and every per-route 429 are invisible in production logs.**                                                                                | NONE                                                                                           |
| 6   | `middlewares/route_blocker.rs`                                                                                                                          | Nothing recorded; reads `route_status` and 4xxes if `is_blocked`                                                                                                                                                                                     | n/a                                                                                                                                                         | n/a                                                                                                                                                                                                                                                                  | NONE                                                                                           |
| 7   | `track_view` dedup key `trackview:{post_id}:{ip}` (`post_v1/controller.rs:357`)                                                                         | 5-minute dedup marker per (post, IP)                                                                                                                                                                                                                 | 300 s                                                                                                                                                       | No (in-memory only)                                                                                                                                                                                                                                                  | NONE                                                                                           |

### 1b. Durable tables that could carry an abuse signal

| #   | Table                                                    | What it records                                                                                                                          | Retained                      | Queryable today                                                                                                                                                                                                                        | Kind                                                                                                                                                                                                                                                                                                           |
| --- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 8   | `route_status` (`db/sea_models/route_status/`)           | One row per **matched route pattern ever seen** + a manual `is_blocked` toggle + `reason`                                                | Forever (one row per pattern) | Yes: `GET /api/admin/route/v1/*`, or direct SQL                                                                                                                                                                                        | DEV: 6 patterns, **all `is_blocked = 0`**. This is a **route inventory, not a traffic counter** — `record_route_pattern` is snapshot-gated to one write per pattern ever (`route_blocker_service.rs:79-93`)                                                                                                    |
| 9   | `user_bans` (`db/sea_models/user_ban/`)                  | `user_id`, `reason`, `banned_by`, `expires_at`, `revoked_at`, `revoked_by`                                                               | Forever                       | Table exists; `get_active_ban` / `is_banned` / `admin_list` exist — **but `create`/`revoke` have zero callers and there is no admin route**                                                                                            | DEV: **0 rows**. REAL: **0 by construction** (nothing can write)                                                                                                                                                                                                                                               |
| 10  | `comment_flags` (`db/sea_models/comment_flag/`)          | `comment_id`, `user_id`, `reason` (free text), `created_at`; upsert per (comment, user); `flags_count` denormalized onto `post_comments` | Forever                       | **Yes, fully wired**: `POST /post/comment/v1/flag/{id}` (public, auth-only, **no abuse limiter**) → `admin_flags_list` / `admin_flags_summary` / `admin_flags_details` / `admin_flags_clear` (`post_comment_v1/controller.rs:341-468`) | DEV: **0 rows**. **This is the only real user-generated abuse report in the system.**                                                                                                                                                                                                                          |
| 11  | `post_comments.hidden` + `flags_count`                   | Admin hide/unhide/delete state                                                                                                           | Forever                       | Yes (`admin_hide`/`admin_unhide`/`admin_delete`)                                                                                                                                                                                       | DEV: 0 rows                                                                                                                                                                                                                                                                                                    |
| 12  | `post_views` (`db/sea_models/post_view/`)                | **Raw `ip_address` + raw `user_agent` + `user_id` + `post_id`** per view, 5-min dedup per (post, IP)                                     | Forever, no retention policy  | Yes: `post_views` SQL + the `analytics_v1` `/engagement/page-views` endpoint (unique-visitor = `DISTINCT COALESCE(user_id, 'ip:' \|\| ip_address)`)                                                                                    | DEV: **0 rows**. **The best existing per-IP distribution source in the schema** — and a raw-PII store `A8 F6`/`A10 F10` already flagged                                                                                                                                                                        |
| 13  | `audit_logs` (`db/sea_models/audit_log/`)                | Designed for `action`, `resource_type`, `resource_id`, `metadata`, `ip_address`, `user_agent`                                            | Forever                       | Table + model exist                                                                                                                                                                                                                    | DEV: **0 rows**, and grep for `audit_log::` / `AuditLog` outside `db/sea_models/audit_log/` returns **0 hits → zero writers**                                                                                                                                                                                  |
| 14  | `user_sessions`                                          | `user_id`, raw `device` (UA), `ip_address`, `last_seen`, `revoked_at` — written **on successful login only**                             | Forever, no retention         | Yes: `POST /sessions/list` returns raw UA + IP + `isCurrent`                                                                                                                                                                           | DEV: 42 rows, 33 from `127.0.0.1`, 9 NULL (OAuth). **No failed-login row is ever written**; failures appear only as the `Invalid credentials` WARN log line with client IP (`auth_v1/controller.rs:226`)                                                                                                       |
| 15  | `sessions` (tower-session SQLite store)                  | Session id + blob + `expiry_date`                                                                                                        | Until expiry                  | Yes                                                                                                                                                                                                                                    | DEV: 103 rows, all unexpired                                                                                                                                                                                                                                                                                   |
| 16  | `email_suppression` (`db/sea_models/email_suppression/`) | `recipient` (UNIQUE), `reason` (Bounce/Complaint/Manual), `source`, `diagnostic`, `permanent`, `last_seen`                               | Forever                       | **Yes**: `GET/POST/DELETE /mail/v1/suppression` (admin)                                                                                                                                                                                | DEV: **0 rows**. **The best existing mail-abuse signal**: forgot-password / verification flooding of invalid addresses surfaces here as hard bounces; recipient spam-marking surfaces as complaints                                                                                                            |
| 17  | `email_verifications`                                    | `user_id`, `code_hash`, timestamps                                                                                                       | Forever                       | Direct SQL                                                                                                                                                                                                                             | DEV: 8 rows (signup→verify funnel for the dev accounts)                                                                                                                                                                                                                                                        |
| 18  | `forgot_passwords`                                       | `user_id`, `code_hash`, timestamps — **one row per user** (regenerated, not appended)                                                    | Forever                       | Direct SQL                                                                                                                                                                                                                             | DEV: 1 row. **Appends nothing → cannot measure reset-flood volume**                                                                                                                                                                                                                                            |
| 19  | `newsletter_subscribers`                                 | `email`, `status` (Pending/Confirmed/Unsubscribed), `token`                                                                              | Forever                       | Model + `GET /newsletter/v1/subscribers/list` exist                                                                                                                                                                                    | **THE TABLE DOES NOT EXIST.** No migration creates it (`grep newsletter migration/src/` → 0 hits), no runtime `CREATE TABLE IF NOT EXISTS` for it, and it is absent from the DB's table list. `POST /newsletter/v1/subscribe` is mounted (405 on GET) and will 500 on insert. **Zero data, zero measurement.** |
| 20  | `devices` (FCM push registry, `device_v1`)               | `user_id`, client-minted `token` (UNIQUE), `platform`, `last_seen_at`; cap 20/user; re-register transfers ownership                      | Forever                       | Yes                                                                                                                                                                                                                                    | DEV: 0 rows. **Not an abuse token** (B2 fact 15) — but it is the _only_ existing device↔account link table, and its ownership-transfer semantics are the exact shape a device-token recapture metric needs                                                                                                     |
| 21  | `users` + `is_verified`                                  | Signup + verification state                                                                                                              | Forever                       | Yes                                                                                                                                                                                                                                    | DEV: 13 users, 6 verified (**46.2 %**)                                                                                                                                                                                                                                                                         |
| 22  | `app_constants`                                          | 195 env-snapshot rows with `source`, `updated_by`, `updated_at`                                                                          | Forever                       | Yes: `db/sea_models/app_constant/actions.rs`                                                                                                                                                                                           | DEV: 195 rows, all `source='env'`. **The cheapest place to record a C3 commitment without a new table**                                                                                                                                                                                                        |
| 23  | `translation_popularity`                                 | `hits_total` per translation id (cumulative), `score`, `last_hit_at`                                                                     | Forever                       | Yes                                                                                                                                                                                                                                    | DEV: 41 rows / **414 total hits** — **dev reader traffic, not production**. The only real _volume_ counter for the Quran product                                                                                                                                                                               |
| 24  | `seed_runs` (`db/sea_models/seed_run/model.rs:5`)        | Declared as `key` + `ranges` JSON + `ran_at` — the seed provenance ledger                                                                | n/a                           | **No.** No migration creates it; absent from the DB; `SeedOutcome.seed_run_id` is hardcoded `None` (`services/seed/custom.rs:111`)                                                                                                     | **NONE — the provenance surface is non-functional**                                                                                                                                                                                                                                                            |

### 1c. Analytics / dashboards

| #   | Surface                                                   | What it records                                                                                                                                 | Queryable today                                                                                  | Kind                                                                                                                                                                                                                                               |
| --- | --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 25  | `analytics_v1` (8 endpoints, `ROLE_ADMIN`, `mod.rs:8-35`) | registration-trends, verification-rates, publishing-trends, page-views, comment-rate, newsletter-growth, media-upload-trends, dashboard-summary | Yes                                                                                              | **None of the 9 fields in `dashboard_summary` is an abuse metric** (`controller.rs:875-890`: users, posts, views, comments, newsletter, media). No bans, no blocks, no flags, no suppressions anywhere in the module                               |
| 26  | `admin_bans_v1`                                           | `GET /admin/bans` (list **active** bans, `Cache-Control: no-store`), `DELETE` (lift), `GET /admin/bans/export` (Bearer `BAN_EXPORT_TOKEN`)      | Yes, admin session. **Live confirmed**: `GET /api/admin/bans` → **401** (mounted, auth required) | **Active bans only.** `collect_active_ban_rows` reads `ban_status` off the in-memory store (`controller.rs:72`), which was restored from the L2 snapshot. An expired ban leaves no trace. **No history, no reason, no actor, no case** (B2 fact 8) |
| 27  | `limiter.saturation` / `store.incr_saturation()`          | Escalation capacity-saturation signal                                                                                                           | No (same dead OTel path)                                                                         | NONE                                                                                                                                                                                                                                               |

### 1d. Off-app surfaces (real, reachable, not in the repo)

| #   | Surface                                                                                                                                                                                                                                                                 | What it records                                                                                                                           | Queryable today                                                                                                                                                                                                        | Kind                                                     |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| 28  | **Cloudflare zone analytics** (confirmed in front: `server: cloudflare`, `cf-ray: …-KHI`)                                                                                                                                                                               | Every request: path, status, country, ASN, top client IPs, cache status, bot score if Bot Fight Mode is on                                | Yes, dashboard                                                                                                                                                                                                         | **REAL. The only true request denominator that exists.** |
| 29  | **Firebase Analytics / GA4** — live, `measurementId: "G-ZYL6HY24W6"` (`web/src/lib/firebase/index.ts:12`), consent defaults `analytics: true` (`web/src/lib/stores/consent.svelte.ts:16`), and the CSP in the live response allows `googletagmanager.com`/`gstatic.com` | Real human sessions, geography, engagement                                                                                                | Yes, GA4 dashboard                                                                                                                                                                                                     | **REAL. The only human-user denominator.**               |
| 30  | **Container stdout logs** (Dokploy, `RUST_LOG=info`)                                                                                                                                                                                                                    | The WARN block lines (#4), route-block WARNs, `Comment flagged`, newsletter subscribe (domain only), comment create/delete, admin actions | Yes, Dokploy log view                                                                                                                                                                                                  | REAL                                                     |
| 31  | Quickwit / OTLP                                                                                                                                                                                                                                                         | Would hold traces + metrics + logs                                                                                                        | **No — never enabled** (#3)                                                                                                                                                                                            | NONE                                                     |
| 32  | `docs/`                                                                                                                                                                                                                                                                 | Any traffic or abuse number                                                                                                               | **None.** `grep -rniE "abuse\|spam\|scrap\|stuffing\|ban evas\|429\|attack" docs/*.md` → 3 hits, all about a redirect bug and the rate-limit _design_, not observed volume. No doc records an observed incident number | NONE                                                     |

---

## 2. Measured numbers

### 2a. The local database — readable, and it is DEV data

`DATABASE_URL=sqlite:./data/easyquran.db?mode=rwc` (`.env:15`), run from `rust/` → the file is
`rust/data/easyquran.db` (356 KB, mtime 2026-10-11 21:36 local). Migrations applied: `m000001`–`m000010`
(**`m000011` not applied**). Queried read-only with `file:…?immutable=1`; **no writes, aggregate counts
only.** (`./data/easyquran.db` at the repo root is a separate **0-byte** file — not the DB in use.)

| Metric                                                   | Value                                                                                                                       |
| -------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `users`                                                  | **13**                                                                                                                      |
| — verified                                               | **6 (46.2 %)**                                                                                                              |
| — email domains                                          | `example.com` **7**, `gmail.com` **6**                                                                                      |
| `sessions` (tower store)                                 | **103**, all unexpired                                                                                                      |
| `user_sessions`                                          | **42** (6 revoked)                                                                                                          |
| — distinct source IPs                                    | **`127.0.0.1` only** (33 rows); 9 rows NULL (OAuth)                                                                         |
| — distinct IPs per user                                  | **max 2** (user 2); every other user has 1                                                                                  |
| — UA families                                            | HeadlessChrome **20**, curl **11**, Electron/OpenCode **1**, Chrome **1**, other **9**                                      |
| `auth_session_binding`                                   | 36                                                                                                                          |
| `user_bans`                                              | **0**                                                                                                                       |
| `comment_flags`                                          | **0**                                                                                                                       |
| `post_comments`                                          | **0**                                                                                                                       |
| `posts` / `post_views` / `post_likes` / `post_revisions` | **0 / 0 / 0 / 0**                                                                                                           |
| `audit_logs`                                             | **0**                                                                                                                       |
| `email_suppression`                                      | **0**                                                                                                                       |
| `devices` (FCM)                                          | **0**                                                                                                                       |
| `notifications` / `passkey_credentials` / `media`        | **0 / 0 / 0**                                                                                                               |
| `rate_limit_state`                                       | **3 rows, 0 blocks** (`SUM(block_until_at>0) = 0`, `SUM(block_scope='Long') = 0`)                                           |
| — keys                                                   | `ratelimit:unknown:quran-v1` (count 4), `ratelimit:127.0.0.1/32:quran-health` (2), `ratelimit:203.0.113.10/32:quran-v1` (2) |
| `route_status`                                           | **6 rows, 0 blocked**                                                                                                       |
| `email_verifications` / `forgot_passwords`               | 8 / 1                                                                                                                       |
| `user_oauth_identities`                                  | 2                                                                                                                           |
| `translation_popularity`                                 | 41 rows, **414 total hits**, last hit 2026-10-11 16:36 UTC                                                                  |
| `newsletter_subscribers`                                 | **table does not exist**                                                                                                    |

**Statement of provenance: this is development/test data, unambiguously.** Every row is attributable to
a test fixture or the developer's own browser:

- The 13 accounts are `UX Audit <ux-audit.test+1@example.com>`, three `Sandbox *@example.com` OAuth
  sandboxes, and six `hmziqrs+…@gmail.com` / `hmziqxyz@gmail.com` aliases belonging to the operator,
  plus `e2e-link@example.com` / `e2e-link-y@example.com` written 0.9 s apart by an E2E run.
- Every `user_sessions.ip_address` is `127.0.0.1` or NULL. The 20 HeadlessChrome rows are the audit
  harness; the 11 curl rows are test scripts; one row's UA is literally `OpenCode/2.0.26 … Electron`.
- `rate_limit_state` contains `203.0.113.10/32` — a TEST-NET-3 documentation address, i.e. a unit-test
  artifact.
- The 414 `translation_popularity` hits are the local reader + SSR prerender hitting the local API.

**None of this is seed data.** The seed generator would have produced 50 faker-named users with
`FreeEmail()` addresses (`services/seed/base.rs:171-203`) plus 100 faker newsletter subscribers and 10
faker comment flags; none of those rows exist. **The seed generator was never run against this DB.**

**There is no production data in this repo.** The production SQLite lives in the Docker volume
`api_data` → `/app/data/easyquran.db` (`docker-compose.yml:159-160`), reachable only from the deploy
host.

### 2b. What is measurable about production from here, without host access

| Probe (read-only)                                                                                         | Result                                                                                                                        | What it proves                                                                                           |
| --------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `GET https://easyquran.fyi/`                                                                              | **200**, `server: cloudflare`, `cf-ray: …-KHI`, `cf-cache-status: DYNAMIC`, CSP allows `googletagmanager.com` + `gstatic.com` | The web app is live, behind Cloudflare, with GA4 permitted                                               |
| `GET https://easyquran.fyi/api/quran/health/ready`                                                        | **200** `{"ready":true,"verseCount":6236,"surahCount":114,"auth":{"enabled":true,"providers":[{…ready:true}×3]}}`             | The API is live, `WEB_AUTH_ENABLED=true` in production, 3 OAuth providers configured                     |
| `GET /api/auth/v1/log_in`, `/register`, `/api/forgot_password/v1/request`, `/api/newsletter/v1/subscribe` | **405** on each                                                                                                               | Mounted (POST-only routes) — the abuse-relevant auth + newsletter surfaces are exposed publicly          |
| `GET /api/admin/bans`                                                                                     | **401**                                                                                                                       | The ban admin API is mounted and auth-guarded — so the ban count is readable by the operator in one call |

**Conclusion:** production is real, public and instrumented _only_ by Cloudflare + GA4 + container
stdout. **No abuse number is obtainable without the operator's host or dashboard access.** That is the
finding, not a gap in this audit.

---

## 3. Is there ANY current evidence of abuse?

| Abuse class                           | Evidence found                                                                                                                                                                                                                                                                                                                                                        | Verdict                                                   |
| ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| **Account-creation abuse**            | None. DEV: 13 accounts, all attributable to fixtures/operator, signup→verify 46.2 %. Production: no rate data obtainable; the per-IP ceiling is 100/min on the auth nest (`router.rs:75`) and Turnstile is mandatory for password registration in production (`services/turnstile.rs:42-47`).                                                                         | **NO EVIDENCE**                                           |
| **Credential stuffing**               | None. `user_sessions` has 42 rows from a single IP. **Failed logins are not recorded in the DB**; the only trace is the `Invalid credentials` WARN (client IP, no email) in container logs, kept only since the container started. The `login:{email}` limiter (`auth_v1/controller.rs:135-140`) would count them, but its bucket is purged ≤15 min after the window. | **NO EVIDENCE — observable only per IP, in logs**         |
| **Scraping**                          | None. `translation_popularity` (the only Quran volume counter) shows 414 dev hits. Production: per-route 429s are **not logged** (`layer.rs:323` is DEBUG; `RUST_LOG=info`), so a scrape against the 600/min ceiling would leave **no trace at all**. Cloudflare sees it; the app does not.                                                                           | **NO EVIDENCE — and the app is structurally blind to it** |
| **Comment spam**                      | None. `post_comments = 0`, `comment_flags = 0`. The report mechanism is complete and unused.                                                                                                                                                                                                                                                                          | **NO EVIDENCE**                                           |
| **Ban evasion**                       | None. `user_bans = 0`; and no account can be banned at all (zero writers, no admin route). `QURAN_BAN_ESCALATION_ENABLED` is off in production, so **automated IP bans issued to date = 0 by configuration (verify the running container's env)**. `rate_limit_state` shows 0 `quran-ban:*` rows.                                                                     | **NO EVIDENCE — and the class is uninstantiable today**   |
| **Coordinated inauthentic behaviour** | None. No co-action pipeline, no device graph, no session-IP diversity (DEV max 2 IPs/user).                                                                                                                                                                                                                                                                           | **NO EVIDENCE**                                           |

**Two honest caveats, stated plainly.**

1. **Absence of evidence here is weak evidence of absence — but it is not nothing.** The instrumentation
   sees credential stuffing only as per-IP WARN log lines, cannot see scraping (429s unlogged), and
   cannot see ban evasion (no bans possible). What _is_ hard is: **zero comment flags, zero suppression
   rows, zero active bans, zero escalation history, and a zero-by-configuration automated ban count.** Those
   four are genuine zeros, not blind spots.
2. **The observation window is short.** The deploy README still documents `WEB_AUTH_ENABLED` as _off_
   for production (`deploy/README.md:478`), while the live health endpoint now reports it **on**, so
   accounts went live recently. The window during which any account abuse could have been _recorded_ is
   likely days, not months.

**Bottom line for §3: there is no evidence of any of the six abuse classes, in either the dev database
or any production surface reachable from this repo — and for three of the six, the app could not have
produced evidence even if the abuse were happening.**

---

## 4. Minimum instrumentation for B2's C1 / C2 / C3

B2's condition (§5): build C (the full fingerprint design) **only if, after 90 days of A then B, all
three hold**: C1 ≥30 % of _confirmed_ ban-evasion cases involve a missing/cleared device token; C2
device-token recapture of suspected evaders <40 %; C3 ≥1 engineer-day/week committed indefinitely.

**Today all three are uncomputable, not merely unmeasured.** C1's denominator is 0 because no "case"
record type exists. C2's denominator is 0 because no device token exists. C3 is not a code measurement
at all. Below is the minimum that makes each computable, with its cost on a pool of 1.

### C1 — share of confirmed ban-evasion cases with a missing/cleared device token

**Missing prerequisite:** a _case_ object. Nothing in the schema records "an enforcement decision and
why". The minimum is one table, written **once per enforcement decision**, never on the request path:

```sql
CREATE TABLE IF NOT EXISTS enforcement_cases (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  case_kind       TEXT NOT NULL,          -- 'ban_evasion' | 'spam' | 'scraping' | 'other'
  subject_user_id INTEGER,                -- no FK (A10 F1 / A3 F10: an FK erases the evidence)
  device_token_id TEXT,                   -- NULL == no token presented at the decisive event
  token_state     TEXT NOT NULL,          -- 'bound' | 'minted_new' | 'absent'
  decided_by      INTEGER,
  decided_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

- **Counter / query** (one aggregate, no join to `users`):
  ```sql
  SELECT CASE WHEN token_state = 'minted_new' THEN 'cleared_or_new' ELSE 'had_token' END AS bucket,
         COUNT(*) AS n
  FROM enforcement_cases
  WHERE case_kind = 'ban_evasion';
  ```
  C1 = `cleared_or_new / (cleared_or_new + had_token)`.
- **Where computed:** async, admin-triggered or a nightly job. **Never the request path.** The row is
  written at the moment an operator confirms an evasion case — i.e. ~once per incident, not per request.
- **Cost on a pool of 1:** one `INSERT` per human decision (≈0 per day) + one indexed `COUNT` over a
  table with single-digit rows per year → **<1 ms, one connection acquisition, off the hot path.**
- **Cheapest interim version, no new table:** add `enforcement_cases` to the **existing `audit_logs`**
  shape instead (`action='enforcement.confirm'`, `resource_type='user'`, `metadata` JSON carrying
  `case_kind` + `token_state`). `audit_logs` already exists, has the right columns, and has zero
  writers — so this doubles as the F7 fix. Then:
  ```sql
  SELECT json_extract(metadata,'$.token_state') AS s, COUNT(*)
  FROM audit_logs WHERE action='enforcement.confirm'
    AND json_extract(metadata,'$.case_kind')='ban_evasion' GROUP BY 1;
  ```
  **This is the recommendation** — one writer + one query, zero migrations.

### C2 — device-token recapture rate of suspected evaders

**Missing prerequisite:** the token itself, and a link table with **no FK to `users`** (an FK to
`user_sessions`/`users` cascades away exactly when the linkage is needed — A10 F1, A3 F10, adopted at
`DESIGN-v1.md:43`). B2's Option B already specifies both (`device_tokens`, `device_account_links`).
The measurement needs only the second:

```sql
-- denominator: tokens linked to a banned user (the banned user is kept explicitly)
WITH banned_links AS (
    SELECT l.token_id, b.user_id AS banned_user_id,
           CAST(strftime('%s', b.created_at) AS INTEGER) AS banned_at  -- TEXT -> epoch (B8 F7)
      FROM device_account_links l
      JOIN user_bans b ON b.user_id = l.user_id AND b.revoked_at IS NULL
),
-- numerator: those tokens re-linked to a DIFFERENT account after that ban was created
recaptured AS (
    SELECT DISTINCT bl.token_id
      FROM banned_links bl
      JOIN device_account_links l2 ON l2.token_id = bl.token_id
     WHERE l2.user_id <> bl.banned_user_id
       AND l2.first_seen > bl.banned_at  -- epoch vs epoch
)
SELECT (SELECT COUNT(DISTINCT token_id) FROM banned_links) AS evaders,
       (SELECT COUNT(*) FROM recaptured) AS recaptured,
       CASE WHEN (SELECT COUNT(*) FROM banned_links) = 0 THEN NULL
            ELSE 1.0 * (SELECT COUNT(*) FROM recaptured)
                 / (SELECT COUNT(DISTINCT token_id) FROM banned_links) END AS recapture_rate;
```

- **Where computed:** async only. **Cost on a pool of 1:** two indexed lookups + one small join over a
  table whose row count is `accounts × devices` (single digits here) → **~1 ms, one connection, off the
  request path.** The link row itself is written at login/register — that is B2's B3 hook, and it is the
  only C2 cost that touches the request path (one `INSERT ... ON CONFLICT` per login).
- **Note:** C2 needs `user_bans` to be _writable_ first. Until B2 F7 is fixed, `banned_tokens` is empty
  and C2 returns `NULL`, which is the honest answer.

### C3 — engineer-time availability (≥1 engineer-day/week, indefinitely)

**Not a code measurement — do not instrument it as one.** The honest minimum is a _commitment record
with a staleness alarm_, and the cheapest possible version uses **zero new tables**:

- **Counter / row:** one `app_constants` row — `key='ABUSE_RULES_REVIEWED_AT'`, `value=<ISO date>`,
  `source='manual'`, `updated_by=<admin id>`. The table already exists, already has `updated_by` and
  `updated_at`, and is already read at boot.
- **Query:**
  ```sql
  SELECT value,
         CAST(julianday('now') - julianday(value) AS INTEGER) AS days_stale
  FROM app_constants WHERE key = 'ABUSE_RULES_REVIEWED_AT';
  ```
- **Where computed:** a boot-time check plus one line on the existing `analytics_v1`
  `/dashboard/summary` payload (a `warn!` if `days_stale > 30`). Async, zero request-path cost.
- **Cost on a pool of 1:** one primary-key `SELECT` at boot. **0 per request.**
- **Cross-check with the repo, no instrumentation at all:**
  `git log --since=30.days -- <the rule/threshold files>` — if the count is 0 and the system is
  supposedly tuned, **C3 is false.** Today there are no rule files, so the answer is already "no
  commitment exists"; the metric would exist to keep that honest _after_ C is built.

**Net C1/C2/C3 cost: one `audit_logs` writer + B2's Option B token tables + one `app_constants` row.
Zero new request-path DB work beyond the login-time link insert that Option B already budgets.**

---

## 5. The cheapest "do we even have a problem?" measurement — runnable in 7 days, no new infrastructure

Five sources, all already live. Operator time: **2–3 hours the first time** (Cloudflare API token, GA4
property and deploy-host access), 10–15 min per repeat. Nothing here needs a deploy,
a migration, or a code change.

### Step 1 — Cloudflare (the denominator, ~10 min)

Dashboard → the `easyquran.fyi` zone → **Analytics → Traffic** and **Security → Events**:

1. Total requests / unique visitors for the last 30 days. **This is the number nobody has.**
2. **Top 10 client IPs by request count** and **top 10 ASNs** — a scraper is a single IP or a single
   ASN with a request count 2–4 orders of magnitude above the median.
3. Requests by path: `/api/quran/*` vs `/` vs `/t/*`. If the API share is high relative to page views,
   that is machine traffic.
4. **Security → Events**: any WAF/Bot-Fight-Mode/Bot-Management actions. If the zone has Bot Fight Mode
   on, this is the closest thing to a bot count that exists.

### Step 2 — GA4 (the human denominator, ~5 min)

Property `G-ZYL6HY24W6` → Reports → **User acquisition** / **Tech → Tech details**:

5. Real users + sessions, last 30 days.
6. **The ratio (Cloudflare requests ÷ GA4 sessions)** — a sanity check only, with no fixed threshold:
   GA4 misses blockers, deniers and JS-less bots, and Cloudflare counts every asset, prefetch and pack
   download. The indicator is the Step 1 per-IP/ASN distribution (D3 §2).

### Step 3 — a read-only copy of the production SQLite (~15 min)

The prod DB is in the Docker volume `api_data`. **Copy it out; never query the live file in place.**

```bash
# 1) copy the DB together with its WAL/SHM files (recent commits live in -wal), read-only mount
docker run --rm -v easyquran_api_data:/data:ro -v "$PWD:/out" debian:trixie-slim \
  sh -c 'cp /data/easyquran.db /data/easyquran.db-wal /data/easyquran.db-shm /out/ 2>/dev/null; ls -l /out/easyquran.db*'

# 2) query the COPY (never the volume) — read-only, aggregate counts only
DB="$PWD/easyquran.db"
sqlite3 -readonly "$DB" <<'SQL'
.mode column
.headers on
PRAGMA integrity_check;  -- must print ok; then check freshness: max(created_at) vs container start

-- (a) THE ABUSE HEADLINE: is anything blocked right now, and why?
SELECT key, fixed_count, block_until_at, block_scope
FROM rate_limit_state
ORDER BY (block_until_at > 0) DESC, fixed_count DESC
LIMIT 40;

-- (b) blocks only, counted (0 rows == zero active enforcement anywhere)
SELECT COUNT(*) AS active_blocks,
       SUM(block_scope = 'Long') AS long_blocks,
       SUM(key LIKE 'quran-ban:%') AS escalation_bans
FROM rate_limit_state WHERE block_until_at > strftime('%s','now');

-- (c) THE SCRAPING SIGNAL: hottest identities per route right now
SELECT key, fixed_count FROM rate_limit_state
WHERE key LIKE 'ratelimit:%' ORDER BY fixed_count DESC LIMIT 20;

-- (d) registration admission: signups per day, and the verify rate
SELECT date(created_at) AS day, COUNT(*) AS signups, SUM(is_verified) AS verified
FROM users GROUP BY 1 ORDER BY 1 DESC LIMIT 30;

-- (e) account-creation abuse: signups per email domain (disposable-domain farming)
SELECT substr(email, instr(email,'@')+1) AS domain, COUNT(*) n
FROM users GROUP BY 1 ORDER BY n DESC LIMIT 20;

-- (f) THE SESSION/IP SIGNAL: sessions per IP (ban evasion + shared-account farming)
SELECT ip_address, COUNT(*) sessions, COUNT(DISTINCT user_id) users
FROM user_sessions WHERE ip_address IS NOT NULL
GROUP BY 1 HAVING COUNT(*) > 1 ORDER BY sessions DESC LIMIT 20;

-- (g) THE VIEW/IP SIGNAL: views per IP (scraping; post_views is the only per-IP store)
SELECT ip_address, COUNT(*) views, COUNT(DISTINCT post_id) posts, COUNT(DISTINCT user_id) users
FROM post_views GROUP BY 1 ORDER BY views DESC LIMIT 20;

-- (h) comment abuse: reports + hidden comments
SELECT (SELECT COUNT(*) FROM comment_flags) AS flags,
       (SELECT COUNT(*) FROM post_comments WHERE hidden = 1) AS hidden_comments,
       (SELECT COUNT(*) FROM post_comments) AS comments_total;

-- (i) MAIL ABUSE: bounces/complaints (verification + reset flooding of bad addresses)
SELECT reason, permanent, COUNT(*) n FROM email_suppression GROUP BY 1,2 ORDER BY n DESC;

-- (j) ENFORCEMENT LEDGER (expected: all zeros)
SELECT (SELECT COUNT(*) FROM user_bans) AS user_bans,
       (SELECT COUNT(*) FROM user_bans WHERE revoked_at IS NULL) AS active_user_bans,
       (SELECT COUNT(*) FROM audit_logs) AS audit_rows,
       (SELECT COUNT(*) FROM route_status WHERE is_blocked = 1) AS blocked_routes;

-- (k) total Quran read volume (the real traffic figure, prod only)
SELECT COUNT(*) AS translations, SUM(hits_total) AS total_hits,
       datetime(MAX(last_hit_at),'unixepoch') AS last_hit
FROM translation_popularity;
SQL
```

### Step 4 — log greps over the container stdout (~10 min)

```bash
# the log window is the current container's lifetime: every master push recreates it
docker inspect -f '{{.State.StartedAt}}' easyquran_api

# per-identity blocks (WARN, emitted at RUST_LOG=info); the FIRST scope= is the key,
# the second is the Temp/Long block scope
docker logs easyquran_api 2>&1 \
  | grep -F 'Request blocked by abuse limiter' \
  | awk 'match($0, /scope=[^ ]+/) { print substr($0, RSTART + 6, RLENGTH - 6) }' \
  | sort | uniq -c | sort -rn | head -40

# failed logins per client IP (the only credential-stuffing trace; WARN, IP only)
docker logs easyquran_api 2>&1 \
  | grep -F 'Invalid credentials' \
  | awk 'match($0, /client_ip=[^ ]+/) { print substr($0, RSTART + 10, RLENGTH - 10) }' \
  | sort | uniq -c | sort -rn | head -20

# route blocks, comment flags, newsletter signups (domains only)
docker logs easyquran_api 2>&1 \
  | grep -E 'Route blocked by dynamic route_blocker|Comment flagged|Newsletter subscription created' \
  | head -50
```

**Read the results like this:** if Step 1 shows a flat per-IP/ASN distribution, Step 3
(b)/(c) shows zero active blocks with a flat `fixed_count` distribution, Step 3 (h) shows zero flags,
Step 3 (i) shows zero suppressions, Step 3 (j) shows all zeros, and Step 4 returns nothing — **the
abuse base rate is indistinguishable from zero, and that is a measurement, not an absence of one.**

**Note on what this plan cannot see, stated honestly:** targeted credential stuffing (the failure log
line carries IP, not email), scraping against the Quran ceiling (429s are DEBUG-only, #5), and ban
evasion (no bans possible). If Step 1's distribution is hot, that anomaly is the _only_ evidence that
would exist — and the fix for its blindness is finding #4, a one-word log-level change.

---

## 6. The honest conclusion if the measurement shows near-zero — and what the plan should do

**If the 7-day measurement returns zeros — which is the most likely outcome for a small Quran app
deployed days ago behind Cloudflare with no automation on — then:**

1. **Stop.** Do not build DESIGN-v1's Option C. Its own falsifiable condition (B2 §5) has failed at
   C1 (denominator 0), C2 (denominator 0), and C3 (no commitment exists). A 65–105 dev-day, 3–5 month
   build whose three gating measurements all read "no target" is **net-negative by its own stated
   rule**, not by this audit's opinion.
2. **Write the number down and re-measure quarterly.** The base rate is not a permanent fact; it is a
   reading. Put the five Step-3 queries in a `just abuse-report` recipe and run it on a schedule. A
   measurement that exists is worth more than a control that doesn't.
3. **Close the three blind spots that make the number _unfalsifiable_ rather than _zero_** — these are
   the only abuse work that is justified at zero base rate, because without them the next measurement
   is equally blind:
   - **Log the 429.** `layer.rs:323` `debug!` → `warn!` (finding #4). One word. Makes scraping and
     every per-route ceiling visible in logs forever.
   - **Wire `audit_logs`** (finding #7). The table, the columns and the JSON `metadata` already exist.
     It is the cheapest possible C1 instrument (§4) and the only way any future enforcement decision
     is reviewable.
   - **Add the `seed_runs` migration** (finding #6) _before_ the first seed run against production, or
     the provenance question becomes permanently unanswerable.
4. **Keep the abuse work that is justified at zero base rate — because it is not abuse work.**
   `user_bans` has zero writers and no admin route, so **no account can be terminated today**. That is
   an _incident-response_ gap, not an abuse-reduction gap: if abuse appears tomorrow, there is no
   button. Wiring it (B2's A1, 2–3 days) is justified by "we cannot respond", not by "we measured
   abuse".
5. **Do not spend the budget on the threat the evidence shows and the instrument can't see.** The one
   documented threat class for Quran apps is app-store takedowns and review-bombing, not account abuse
   (R2-evasion §8.1–8.2, unchanged by this audit). And the app is **structurally blind to the abuse it
   does name**: the device axis is excluded from reader routes (`DESIGN-v1.md:72`, B2 F4), so the one
   abuse that this app could plausibly suffer at scale — scraping a public-domain text — would not be
   detected by the proposed system even if it were built.
6. **Say it in the design doc.** `DESIGN-v1.md` §Q2 asks whether fingerprinting is the right tool. The
   honest answer at a zero base rate is: _not yet, and not for this threat, and the measurement that
   would change the answer has not been taken._ Record the measurement, the date, and the condition
   that would reopen the decision. That converts an unfalsifiable design into a falsifiable one.

**The one sentence the plan should contain:** _"Measured abuse base rate as of 2026-10-\__: zero active
bans, zero escalation history, zero comment flags, zero mail suppressions, zero audit rows; production
per-IP/ASN request distribution flat. The device-identity design is deferred; it reopens
if any of the five counters moves, or if the C1/C2 instrumentation (one `audit_logs` writer + one
login-time link insert) produces a non-zero numerator over 90 days."_

---

## Findings

### F1 — No abuse event is retained past its own expiry: `rate_limit_state` is a live snapshot, not a ledger. **NEW**

**Severity: Critical.**
**Evidence:** `services/rate_limit_store.rs:190-197` — the 10-second flush opens with
`DELETE FROM rate_limit_state WHERE fixed_expires_at <= ? AND block_until_at <= ?`. The table's columns
are `key, fixed_count, fixed_expires_at, block_until_at, block_scope` (`:22-28`): **no `blocked_at`, no
cumulative block counter, no history table.** `InMemoryStore::prune()` runs immediately after every
flush (`:165`). A `quran-ban:` row survives only until its 7-day Long expiry; a `login:{email}` counter
row is gone ≤15 minutes after the last attempt. Consequence: "how many blocks were issued in the last
90 days" is **not a hard query, it is an unanswerable one** — and it is the exact query every threshold
in the fingerprint design (80/40, the precision/overturn KPIs, the C1/C2 conditions) is calibrated
against. Dev DB confirms the shape: 3 rows, all `block_until_at = 0`.
**Fix:** add an append-only `rate_limit_events(key, scope, issued_at, expires_at)` written **inside the
existing flush transaction** and only on a **state transition** (first time a key goes from
`block_until_at = 0` to `> 0`, and once on expiry) — one row per block, not per request. This is the
smallest change that makes the base rate measurable at all, and it costs one `INSERT` per block inside
a transaction that already runs every 10 s. Cheapest alternative if a migration is unacceptable:
reuse `audit_logs` (already exists, zero writers — F7) with `action='ratelimit.block'`.

### F2 — The only automated abuse response is default-OFF in production, so the automated ban count is zero by configuration. **NEW**

**Severity: High.**
**Evidence:** `config/settings.rs:349` — `env_bool("QURAN_BAN_ESCALATION_ENABLED", false)`; `:356` —
enabling it _requires_ `QURAN_BAN_ALLOWLIST` or boot fails. `docker-compose.yml` sets no
`QURAN_BAN_ESCALATION_ENABLED`, no `QURAN_BAN_ALLOWLIST`, no `QURAN_ACTIVE_BAN_MAX` (grep → 0 hits for
all three). The escalation engine is attached to the quran content limiter only when enabled
(`main.rs:860-892`, `:912`). `services/escalation.rs:1-3` states "Default-OFF; a no-op until
`QURAN_BAN_ESCALATION_ENABLED=true`". Combined with F7 (`user_bans` has no writer), **no ban of any
kind can be created in production except by a human calling `DELETE /admin/bans`'s sibling — which does
not exist.** The dev DB agrees: `rate_limit_state` has zero `quran-ban:*` rows and zero
`block_until_at > 0`.
**Fix:** this is a deliberate decision that has never been stated as one. Either (a) enable it after the
allowlist is set, or (b) record explicitly that production runs **observe-only** and stop costing the
ban-escalation ladder, the `GET /admin/bans` surface, and the `quran-ban:` export contract as live
controls. Either way, state it in `docs/` so the next audit does not re-derive it.

### F3 — Abuse counters exist in code and are dropped on the floor: the OTLP export has never been enabled anywhere. **NEW**

**Severity: High.**
**Evidence:** `utils/telemetry.rs:443-476` defines `LimiterMetrics { checks, allowed, blocked,
temp_blocks, long_blocks }`, incremented by `TelemetryHooks` in `services/abuse_limiter.rs:11-31`
(`on_blocked` splits Temp/Long). Export requires `ENABLE_QUICKWIT_OTEL` truthy (`telemetry.rs`,
`quickwit_otel_enabled()`); **that variable appears nowhere except that function** — grep over
`*.yml`, `*.yaml`, `*.env*`, `*.md`, `*.toml`, `*.sh` returns **0 hits** outside the code. `.env:39` sets
`OTEL_SDK_DISABLED=true`; `docker-compose.yml` sets no OTEL or Quickwit variable. So the counters are
incremented into a global meter with no reader and no exporter. `Metrics` are also process-local, so
even a live exporter would reset them on every deploy — and `deploy/README.md:21` states that every
master push redeploys.
**Fix:** the cheapest fix is **not** to stand up Quickwit. It is finding F4: promote the block log line
to a durable level and grep the container stdout that already exists. If real metrics are wanted, first deploy
a collector with storage and an owner: the default ingest URL is `http://localhost:7280`, export failures
are dropped silently, and the counters reset on every redeploy (F4 F2) — the env flag alone produces nothing
readable.

### F4 — Route-level 429s are logged at DEBUG only, so the Quran content ceiling is invisible in production. **NEW**

**Severity: High.**
**Evidence:** `crates/rux-request-gate/src/layer.rs:323` — `debug!("Rate limit exceeded", identity,
path, count, max_requests, window_secs)`. `docker-compose.yml` sets `RUST_LOG: ${RUST_LOG:-info}`, which
drops DEBUG. The per-identity abuse limiter's block _is_ a WARN
(`crates/rux-request-gate/src/abuse.rs:109`, `"Request blocked by abuse limiter"`), so per-account
limits are observable and per-route ceilings are not. The 600/min external content ceiling
(`middlewares/rate_limit.rs:57-63`) is the **only layer that can enter escalation** and the only defence
on the public Quran API (`quran_v1/mod.rs:12-50`, all public, unauthenticated) — and a scraper hitting
it produces **no log line and no retained row**. B2 F4 already established that the design excludes
reader routes; this finding is that the app cannot even _see_ the reader-route abuse it does not claim
to stop.
**Fix:** change `debug!` → `warn!` at `layer.rs:323`. One word. Add `count` and `max_requests` to the
message (already in the macro args). Optionally gate it on `count == max_requests + 1` so a sustained
flood logs once per window per identity instead of once per 429 — preserving the existing
one-event-per-window discipline (`layer.rs:293-300`).

### F5 — C1 and C2 are structurally uncomputable: their denominators do not exist, so the falsifiable condition cannot fire in either direction. **NEW**

**Severity: Critical.**
**Evidence:** B2 §5 requires "≥30 % of _confirmed_ ban-evasion cases involve a missing/cleared device
token" and "device-token recapture of suspected evaders <40 %". For C1: no table records an enforcement
decision — grep `enforcement_case`/`case_kind` → 0 hits; `audit_logs` (the natural home) has zero
writers. For C2: there is no device token and no token→account link — `device_v1`'s `devices` table is
an FCM push registry with `token TEXT UNIQUE` and ownership transfer (`modules/device_v1/controller.rs`,
B2 fact 15), not an abuse token. **Denominator for C1 = 0. Denominator for C2 = 0.** A condition whose
denominators are zero can never be evaluated, so "we will decide after 90 days" is not a decision
procedure — it is an indefinite deferral that reads as a plan. `B10 §6.5` asked for exactly this
falsification criterion and did not get one; this finding is that the criterion B2 wrote cannot run.
**Fix:** the minimum that makes both computable is in §4 and is deliberately tiny: (a) one
`audit_logs` writer recording `case_kind` + `token_state` at each human enforcement decision, and
(b) B2's Option B `device_account_links` table (no FK to `users`, per A10 F1/A3 F10). Both are async
aggregates over single-digit row counts; neither adds request-path DB work beyond Option B's existing
login-time link insert. **Without these two, C1/C2 should be recorded as "not measurable" rather than
"pending" — and a condition that cannot be measured cannot gate a build.**

### F6 — Seeded rows are indistinguishable from real rows, and the provenance ledger does not exist. **NEW**

**Severity: High.**
**Evidence:** `db/sea_models/seed_run/model.rs:5` declares `#[sea_orm(table_name = "seed_runs")]` with a
`key` + `ranges` JSON + `ran_at` — but **no migration creates it** (`grep newsletter|seed_run
migration/src/` → 0 hits), it is absent from the live DB's table list, and `SeedOutcome.seed_run_id` is
hardcoded `None` (`services/seed/custom.rs:111`). Meanwhile: seed users get `faker::internet::en::FreeEmail()`
addresses (`services/seed/base.rs:29-34`, `:171-203`) — **real domains, indistinguishable from real
signups**; seeded comment flags use realistic reasons `["spam","inappropriate","off-topic","harassment"]`
(`base.rs:1055`) attached to real `post_comments` rows; seeded newsletter subscribers are 100 faker
emails with `Confirmed`/`Pending` statuses (`base.rs:1085-1100`); seeded `post_views` write
`ip_address` + `user_agent`. **There is no `is_seed` column, no marker, no run id, and no ledger.** The
seed routes are gated behind `ROLE_SUPER_ADMIN` + the `seed-system` feature (`modules/seed_v1/mod.rs:8`),
which is a _call_ gate, not a _row_ marker — once run, the rows are permanent and unattributable.
**Fix:** add the `seed_runs` migration and write one row per seed run **before** the first seed is ever
run against production. If a stronger guarantee is wanted, prefix seeded emails with a reserved
subdomain (`seed+…@seed.invalid`) and add `AND email NOT LIKE '%@seed.invalid'` to every abuse
aggregate. The critical property is that the marker must exist **before** the first write, because
retrofitting provenance onto an unmarked table is impossible.

### F7 — `audit_logs` has a table, a model and zero writers: no enforcement action taken today is recoverable. **NEW**

**Severity: High.**
**Evidence:** `db/sea_models/audit_log/` contains only `mod.rs` + `model.rs` — no `actions.rs`, no
`slice.rs`. Grep for `audit_log::` / `AuditLog` / `audit_logs` across `src/` excluding that directory →
**0 hits**. The table exists with the right columns (`user_id, action, resource_type, resource_id,
metadata, ip_address, user_agent, created_at`, FK to `users` `ON DELETE SET NULL`), and the dev DB shows
`audit_logs = 0`. Meanwhile there are **five** enforcement/mutation actions that produce no record
anywhere: `RouteBlockerService::block_route`/`unblock_route` (`services/route_blocker_service.rs:128-163`,
`info!` only), `admin_flags_clear` (`post_comment_v1/controller.rs:317-337`, `info!` only),
`admin_hide`/`admin_unhide`/`admin_delete` (`:233-313`, `info!` only), the ban admin path, and
`admin_route_v1`'s block/unblock (`:39,63`). Container stdout is the _only_ record, and it is
log-rotation-dependent and unqueryable. This is simultaneously the DSA Art 17 / dispute / LE problem
(A8, B2 A10) **and** the C1 instrumentation gap (F5).
**Fix:** one `services/audit.rs` writer, called from those five sites with a JSON `metadata` payload.
~150 LOC. It is the single change that converts both "we cannot defend a moderation decision" and "C1
cannot be computed" into solved problems, and it needs no migration.

### F8 — `newsletter_subscribers` has no migration and no runtime DDL: the newsletter abuse surface has zero data and its insert path fails. **NEW**

**Severity: Medium.**
**Evidence:** `db/sea_models/newsletter_subscriber/model.rs:7` declares
`#[sea_orm(table_name = "newsletter_subscribers")]`. `grep -rn newsletter migration/src/` → **0 hits**
across all 11 migrations. `grep "CREATE TABLE IF NOT EXISTS" src/` → only four runtime tables:
`user_oauth_identities` (`services/oauth/login.rs:400`), `translation_popularity`
(`services/translation_popularity_store.rs:192`), `rate_limit_state` (`services/rate_limit_store.rs:22`),
`sessions` (`services/session_store.rs:15`). The live DB's table list contains **no
`newsletter_subscribers`**. `POST /api/newsletter/v1/subscribe` is mounted — `GET` returns **405** on
the live origin — so the route exists and the `INSERT` (`newsletter_v1/controller.rs:95`) will fail
against a missing table. Consequence: `newsletter_v1`'s two abuse limiters
(`newsletter:subscribe:{email}` 5/60s→1h + 20/24h→24h, and `newsletter:subscribe:ip:{ip}`
20/60s→1h + 200/24h→24h, `:63-87`) guard a write that cannot succeed, and §5's signup-to-confirm abuse
signal is **unavailable**: there are no Pending rows to compare against Confirmed rows.
**Fix:** either add the migration (next free number, `m0000NN_newsletter_subscribers`; the model's columns are already
fixed) or remove the module from the router. Do not leave a mounted public write endpoint whose table
does not exist — it is an unhandled-500 surface and a false sense of coverage in the abuse inventory.

---

## Appendix — verification commands used (all read-only)

```bash
# dev DB, read-only, never modified
DB=/Users/hmziq/os/easyquran/rust/data/easyquran.db
sqlite3 "file:$DB?immutable=1" ".tables"
sqlite3 "file:$DB?immutable=1" "SELECT COUNT(*) FROM users;"
# (note: mode=ro fails on this file; immutable=1 works and is equally read-only)

# repo greps that produced the findings
rg -n 'ENABLE_QUICKWIT_OTEL' --glob '!node_modules'
rg -n 'audit_log::|AuditLog' rust/backend/api/src | grep -v db/sea_models/audit_log
rg -rn 'newsletter' rust/backend/api/migration/src/
rg -n 'CREATE TABLE IF NOT EXISTS' rust/backend/api/src
rg -n 'QURAN_BAN_ESCALATION_ENABLED|QURAN_BAN_ALLOWLIST' docker-compose.yml

# deployment state, read-only probes
curl -sSI https://easyquran.fyi/ | head
curl -sS  https://easyquran.fyi/api/quran/health/ready
curl -sS -o /dev/null -w '%{http_code}\n' https://easyquran.fyi/api/admin/bans   # 401
```

_End of C3._
