# D3 — Attack on the C3 measurement plan

Auditor: adversarial measurement pass. Date: 2026-10-12. Repo read-only during this pass.
Target: `C3-base-rate.md` §4 (instrumentation) and §5 (7-day measurement plan), its §6 conclusion,
and the C1/C2/C3 conditions as carried by `C5-decision-ledger.md` (O14/O18) and `C1-optionAB-audit.md`
(Gate 0/Gate 1). Inputs read: C3, C5, C1, B10; every repo claim below re-verified at HEAD this session.
Marking: **NEW** = not stated in C1–C5/B10; _confirms_ = re-derivation of an existing finding.

**Verdict up front.** The plan's instinct is right — measure before building, use what already exists —
and the four-step shape is close to the cheapest possible. But read naively it is **not runnable and not
trustworthy**, in five ways:

1. The log window is not 720 h. Every master push recreates the containers; `docker logs` reaches
   back only to the current container's start. Failed logins (WARN + client IP) are the one log signal
   that already exists, and a greedy block grep extracts the wrong field.
2. A CF requests ÷ GA4 sessions ratio ("> ~50") has no baseline, mismatched units, an
   undercounted denominator (ad blockers, deniers, JS-less bots) and an inflated numerator (SSR-adjacent
   asset churn, SW/offline pack, prefetch). It cannot support a build/stop decision.
3. C1 and C2 are **not computable at all before WS2 and WS8**, and the C1 instrument C3 recommends
   (`audit_logs` reuse) is on C5's _rejected-by-evidence_ list (#20) and contradicts the adopted case
   model (D26). C1 also has a selection-bias defect that survives WS8: the case population is defined by
   the detector being evaluated.
4. Zero automated bans is a default-value inference, not a proof — the deployed `.env` is not
   in the repo — and the DB cross-check only looks back ≤7 days.
5. The "stop if zero" conclusion is directionally right but must be time-boxed and re-worded: the
   measurement is structurally blind to scraping, distributed credential stuffing, and the one documented
   threat (app-store takedowns / review-bombing), which no step queries at all.

The corrected plan (§9) keeps the five sources but fixes the reads; the corrected gate (§10) replaces
"C1/C2/C3 after 90 days" with a three-stage gate that does not arm an unmeasurable instrument.

---

## 1. Attack each step: what can actually be read, what is missing, what the numbers cannot show

### 1.1 Access assumptions the plan never states

| Surface         | What the plan assumes                              | Reality / what must be verified first                                                                                                                                                                                                                                                                                                     |
| --------------- | -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cloudflare zone | Dashboard → Traffic + Security → Events, "~10 min" | Requires the operator's CF account (2FA) for the zone. Top-IP/ASN groupings are best obtained from the **GraphQL Analytics API** (`httpRequestsAdaptiveGroups` with `clientIP`/`clientAsn` dimensions), which needs an API token with **Zone Analytics: Read** — a new secret, 15 min to create, and it must not be pasted into the repo. |
| GA4             | Property `G-ZYL6HY24W6`, "~5 min"                  | Requires Google-account access to the `easyquran-fyi` Firebase project. The measurement ID in the bundle (`web/src/lib/firebase/index.ts:12`) does not imply property access.                                                                                                                                                             |
| Prod SQLite     | `docker run … cp /data/easyquran.db`               | Requires deploy-host access. The copy method is wrong for a WAL database (§1.3).                                                                                                                                                                                                                                                          |
| Container logs  | `docker logs easyquran_api --since 720h`           | Window is bounded by container recreation, not by `--since` (§1.2).                                                                                                                                                                                                                                                                       |

The plan's "under one hour" is an underestimate: access setup (CF token, GA4 property, host access,
copy verification) is realistically 2–3 hours, and the plan does not name an owner. **NEW**

### 1.2 Step 4 — the log window is days, not 30 days; the grep is broken

**The window.** `docker-compose.yml` has **no `logging:` block on any service** (full file read: no
`logging`, `max-size`, `max-file`, or `log-opts` anywhere; grep over `deploy/` and the justfile → 0 hits).
So Docker's default `json-file` driver applies, with whatever daemon-level defaults the host has — the
repo cannot tell you. The binding constraint is different and worse: **the containers are recreated on
every master push.** CI publishes `:latest` and calls Dokploy's `POST /api/compose.redeploy`
(`deploy/README.md:21-28`, `:180-181`); the compose stack is pull-only (`docker-compose.yml:41-50`,
`114-117`), so each redeploy pulls a new digest and replaces the container. `docker logs <name>` reads
the _current_ container's log file; the previous container's log file is reachable only by its container
id, until Dokploy/`docker compose` removes it.

Consequences:

- `--since 720h` does not fail — it **silently returns less**. The realistic window is
  `now − container StartedAt` (verify: `docker inspect -f '{{.State.StartedAt}}' easyquran_api`).
- If the operator pushed code this week, the window is ~days. If Dokploy prunes old containers, earlier
  logs are gone entirely.
- The plan must enumerate old containers (`docker ps -a --filter name=easyquran_api`) and grep each by id,
  or state the window honestly and start retaining logs (one `logging:` block with `max-size`/`max-file`
  is the fix, and it also stops unbounded growth).

**The grep.** `crates/rux-request-gate/src/abuse.rs:57-63` records the _span_ field `scope = %key_prefix`
(the full key, e.g. `login:user@example.com`), while the block event at `:109-115` re-uses the field name
`scope` for the `BlockScope` enum (`Temp`/`Long`). The line therefore contains two `scope=` fields. A
greedy `sed -E 's/.*scope=([^ ]+).*/\1/'` extracts the **last** one — `Temp`/`Long` — so a
"per-identity blocks" count becomes a count by scope, not by identity. Fix: take the first occurrence
(`grep -o 'scope=[^ ]*' | head -1` per line, or grep the span prefix `request{scope=`). Also note the
block line is emitted once per block window, so it counts blocks, not requests. **NEW**

**The missing signal.** Failed logins are not recorded in the database, but they are in the logs:

- `warn!(client_ip = %secure_ip, "Invalid credentials")` — `auth_v1/controller.rs:226` (WARN ≥ INFO, so
  emitted at `RUST_LOG=info`).
- `info!(client_ip = %secure_ip, "Login attempt")` — `:129` (every attempt, success or failure).
- `warn!(user_id, "Banned user attempted login")` — `:155`.
- `info!("User registration attempt")` — `:466`; `warn!("Registration on an existing email answered with
the generic success envelope")` — `:523`; `warn!(error, "Registration failed")` — `:527`.

So the cheapest credential-stuffing / registration-pressure instrument in the whole stack is a `grep` over
logs that already exist — and a block-only Step 4 misses it. (Limits: the failure line carries IP, not
email; per-account targeting is invisible; and the window is §1.2's window.) **NEW**

**What the numbers still cannot show even when collected.** Per-account attempt volume; per-email
targeting; any 429 on per-route ceilings (DEBUG-only, `layer.rs:323`); anything older than the current
container; anything the app never logs (reads served from the disk cache, SW-served assets, SSR fetches —
the web→API SSR hop bypasses Cloudflare entirely, `docker-compose.yml:63-66`).

### 1.3 Step 3 — the DB copy can be stale or torn

The prod DB runs WAL (`sea_connect.rs:48`), so copying only `easyquran.db` is unsafe. In WAL mode, recent committed data lives in `easyquran.db-wal`. Copying the main file alone yields a
snapshot that can be **behind by an unbounded number of recent transactions** (or, if the WAL is
mid-checkpoint, inconsistent), and `immutable=1` makes it worse: it tells SQLite to ignore the WAL, so the
copy silently reads an older state. The fix is `sqlite3 ".backup"` against a read-only connection (or
copying all three files together), plus `PRAGMA integrity_check` and a freshness check (max
`created_at` vs `docker inspect` container start). **NEW**

Also: C3's Step 3(g) queries `post_views` as "the only per-IP store" — true, but it is written by the
**blog-post** view path (`post_v1/controller.rs:357`, `trackview:{post_id}:{ip}`), and `posts = 0` in the
dev DB. The Quran reader routes never write it. So the plan's scraping query is structurally empty for the
product that actually serves traffic. `translation_popularity` is cumulative (`hits_total`, `last_hit_at`)
with no per-day buckets, so one read gives a total, not a rate; it needs two snapshots to become a rate,
and it cannot separate human from bot. **NEW**

### 1.4 Step 1 — Cloudflare facts the plan does not carry

- Adaptive datasets now retain **at least 31 days with a 30-day query window on all plans** (Cloudflare
  changelog, 2026-10-02). That makes the 30-day ask feasible — but only via datasets the zone's plan can
  read; **verify per zone with the `settings` node** (`httpRequestsAdaptive { notOlderThan maxDuration }`,
  same for `firewallEventsAdaptive`). Cloudflare's own docs disagree between pages for Free-plan Security
  Analytics (one page says 7-day retention / 1-day query window), so do not assume.
- **Security Events on Free is "sampled logs only"** (WAF docs). Small event volumes (this zone) may be
  invisible or heavily estimated. Counts from Free-plan analytics are **estimates with unpublished
  sampling rates**.
- **Bot score is not on Free** (`cf.bot_management.score` is Enterprise); Free gives `cf.client.bot`
  (verified-bot flag) in rules and Bot Fight Mode events if the toggle is on. C3's "closest thing to a bot
  count" is weaker than stated: on this plan there is no bot score to read.
- Free-plan fine-grained magnification is 1-hour scope for the last 24 h; 30-day data is coarse.

### 1.5 What none of the steps can show, even after fixes

No intent (heavy legitimate use — a bulk-download user, a school NAT, an offline-pack mirror — looks like
"machine traffic"); no evasion (no bans can exist); no per-account abuse; no harm (CF counts requests, not
origin cost — `cacheStatus HIT` vs `MISS` is the difference and the plan does not separate them); no
distributed low-and-slow campaigns below per-IP thresholds; nothing long-horizon (30-day windows, a
days-old deployment, log windows measured in days).

---

## 2. Why `CF requests ÷ GA4 sessions > ~50` cannot be the indicator

**What inflates the numerator (legitimate):**

- Browser asset churn per visit (JS/CSS chunks; SvelteKit code-splitting) — high on cold visits, near zero
  after SW install.
- Service worker and offline pack: the SW bypasses `/api/` (C5 constraint 37) and the pack is ~14.1 MB raw
  / 590 KB brotli, unauthenticated — a pack download is one or a few CF requests but represents one user.
- Prefetch (`data-sveltekit-preload-data`) multiplies document/data requests per hover/touch.
- Bots that CF counts and GA4 does not (Googlebot and most crawlers do not execute JS → 0 GA4 sessions).
- Health/monitoring probes and any non-browser client (the zone's own tooling).

**What deflates the denominator (GA4):**

- Ad blockers: uBlock Origin/EasyPrivacy block `googletagmanager.com`/`gstatic.com` by default, so those
  users are invisible. The app's CSP deliberately allows both (`hooks.server.ts:82-96`).
- Explicit deniers: consent defaults to **granted** (`consent.svelte.ts:16`), but `setAnalyticsCollectionEnabled(false)`
  stops collection for anyone who denies; consent is applied after analytics init (`boot/analytics.ts:24-25`).
- JS-disabled and privacy browsers.
- No consent banner on the reader (C5 D42), so there is no way to know the denial rate — it is unmeasured.

**Structurally:** the ratio divides requests (all clients, all resource types,
including cache HITs) by sessions (JS users only, 30-min windows). The two quantities do not share a
denominator, a time base, or a client population. The threshold "~50" has no measured baseline for this
app — an SSR app with a service worker, a disk cache, and a 14 MB pack can plausibly sit anywhere in
5–200 depending on visit mix.

**Corrected computation.** Replace the single ratio with three CF-internal readings plus one GA4
calibration:

1. `R_doc` — origin-cost ratio, **entirely inside Cloudflare**:
   `count(httpRequests where clientRequestPath LIKE '/api/quran/%' AND cacheStatus IN ('DYNAMIC','MISS')) ÷ count(httpRequests where clientRequestPath IN (document paths: '/', '/{surah}*', '/t/*'))`.
   Both terms come from the same dataset with the same sampling, so the sampling bias largely cancels in
   the ratio; report both raw counts anyway.
2. `P` — Pareto test: top-1 client IP share of `/api/quran/*` requests, and top-10 share. Machine traffic
   is a distribution property (heavy tail), not a ratio: flag if top-1 > 5 % of content requests or if any
   IP exceeds 10× the median IP's count.
3. `A` — ASN concentration: share of `/api/quran/*` from the top ASN. Flag > 50 %.
4. `U` — GA4 undercount factor, measured once: `count(CF document requests in window) ÷ GA4 sessions in the
same window`. This is the honest error bar on the human denominator; report it instead of pretending
   GA4 is ground truth.

**Error bars, stated:** CF Free-plan counts are adaptively sampled estimates (rate unpublished) → report
counts as "± sampling, unknown"; GA4 undercounts by `U` (report `U` and its window); and `R_doc` is only
meaningful against the app's own baseline (two consecutive weeks), not a magic constant. Never gate a build
on a single ratio.

---

## 3. Attack the instrumentation plan: does `audit_logs` give C1?

**No — and the plan's own §4 recommendation contradicts the adopted decision.**

1. **C1 needs a case object, and C3's cheap version is already rejected.** C3 §4 recommends recording
   `case_kind` + `token_state` in `audit_logs.metadata`. `C5` §4 ("Rejected-by-evidence") item 20 kills
   exactly that: "Evidence in `device_records.evidence` / reuse of `audit_logs` — A8 F4/F5, A10 F14 (no
   case id, no retention, SET NULL)". D26 (adopted) requires a dedicated `enforcement_cases` + append-only
   `case_events` with a state machine. Verified here: `audit_logs.user_id` is `ON DELETE SET NULL`
   (`audit_log/model.rs:20-27`) — the subject link disappears exactly when a deleted-account evasion case
   is investigated; and `audit_logs` has no case id, no retention class, no state.
2. **Decision-time ≠ event-time.** A `token_state` written when an operator confirms a case records the
   state _at confirmation_, not at the decisive event (the request that established evasion). An evader can
   clear storage in between; the field then reads `absent` and C1 counts it as "missing token" when the
   token was present at the event. C1 needs (a) the case row and (b) an immutable observation of what was
   actually seen at detection (session/token id, or the absence), i.e. `case_events`.
3. **The population does not exist, and its existence depends on the system being evaluated.**
   "Confirmed ban-evasion cases" requires (i) bans to be possible (`user_bans` has zero writers, no admin
   route), and (ii) evasion to be _detectable_. Pre-WS8, detection is IP/credential/content based; a case
   detectable _only_ by the device token is invisible. Post-WS8, the case set is still conditional on the
   detection channel — so C1 is a **lower bound biased toward the channels that already work**. C1 as
   defined can never be measured cleanly; it must be reported with the detection-channel breakdown.
4. **C2's denominator is worse than C3 states.** `user_bans.user_id` is `ON DELETE CASCADE`
   (`user_ban/model.rs:34-41`): an evader who deletes the banned account deletes the ban row, and C2's
   `banned_tokens` CTE (which joins live `user_bans`) then returns nothing — the metric _improves_ when the
   evader destroys the evidence. `get_active_ban` also returns one row, so stacked bans are ambiguous.
   C2 must be keyed on the case/token link (no-FK evidence rows per D23), not on live ban rows.
5. **A subtle null instrument.** If C1 were computed after WS2 but before WS8 (no token exists), every case
   would record `token_state = 'absent'` and C1 would read 100 % — a number that means "the token system
   is not built", not "evaders clear tokens". The instrument must refuse to compute C1 until WS8 is live.

**Computable before WS2/WS8? No.** C1: denominator 0 (no case object) — and after WS2 only, it is a null
instrument. C2: denominator 0 (no token), and its numerator also needs a writable ban record that survives
account deletion. C3's honest status is **"not measurable"**, not "pending".

**The honest gate.** Three stages, §10 below: Gate −1 (observability + base rate, runnable now), Gate 0
(after WS2, before WS8/B — the C1-optionAB §8.2 gate, honestly scoped), Gate 1 (after WS8 + 90 days —
C1/C2/C3/C4 with the case table and a detection-channel breakdown).

---

## 4. Attack "stop, don't build C if the measurement shows near-zero"

The conclusion is directionally right (C3 §6) but the phrasing "measured abuse base rate … zero" over-claims,
because the measurement is blind to:

| Blind spot                                                 | Why invisible to the 7-day plan                                                                                                                                                                         | Cheap close                                                                                              | Verdict                  |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------ |
| Reader/API scraping                                        | 429s DEBUG-only; `post_views` is blog-only; `translation_popularity` has no rate                                                                                                                        | CF Pareto (`P`, `A` in §2) now; the one-word `debug!→warn!` at `layer.rs:323` on the next deploy         | **Close now via CF**     |
| Distributed credential stuffing                            | Per-email attempts never logged with the email; DB keeps nothing                                                                                                                                        | Add the existing `Invalid credentials` + `Login attempt` greps (per-IP counts) — §1.2                    | **Close now, zero code** |
| Targeted credential stuffing (one account, rotating IPs)   | Failure line has IP, not email; limiter bucket purged ≤15 min                                                                                                                                           | Accept for now; add a hashed-email field to the failure line if the login WARN ever shows a spike        | **Accept, watch**        |
| Ban evasion                                                | Uninstantiable (no bans, no writer)                                                                                                                                                                     | WS2 (`user_bans` + case table) is the _incident-response_ fix, not a measurement                         | **Build for response**   |
| Gmail dot-alias account farming                            | Signups-by-domain groups everything under `gmail.com`; alias policy strips only `+` (C1 F3)                                                                                                             | SQL heuristic today (dot-stripped local part per day, §7 #7); real fix is the verification-identity hash | **Close now via SQL**    |
| Comment spam without reports                               | `comment_flags = 0` is not `comments = 0`                                                                                                                                                               | comments-per-author concentration query (§7 #5)                                                          | **Close now via SQL**    |
| Mail abuse                                                 | `email_suppression` fills only from provider sync bounces or the `/mail/v1/webhook/cloudflare` webhook (mail_v1/mod.rs:27; controller.rs:120-145) — if the webhook is not configured, 0 is a blind spot | Verify the webhook is configured at the provider; otherwise treat 0 as uninformative                     | **Verify, then accept**  |
| Newsletter abuse                                           | Table does not exist (C3 F8); endpoint 500s                                                                                                                                                             | Fix F8 or drop the surface                                                                               | **Fix or drop**          |
| **Takedowns / review-bombing — the one documented threat** | **No step queries it at all**                                                                                                                                                                           | A monthly manual field in the report: app-store reviews, mailbox reports, Search Console messages        | **Close now (manual)**   |
| Coordinated inauthentic behaviour                          | No graph, no co-action pipeline                                                                                                                                                                         | Accept at this size (B10 §7)                                                                             | **Accept**               |

So the honest conclusion is: _"no abuse is visible in the classes the instrument can see, over a window
measured in days, with the response engine off"_ — which is a reason to defer the **fingerprint**, not a
reason to believe the app is un-abused.

---

## 5. Cadence and triggers

**Quarterly is wrong at the start and right later.** The correct cadence follows the base rate and the
instrument state, not the calendar:

- **Weekly for the first 4 weeks** (10–15 min each): the numbers are all zero; you are watching for
  _onset_, and the observability fixes land during this period, resetting the baseline.
- **Monthly** once two consecutive readings are zero and traffic is stable (and after any instrument
  change, e.g. WS2/WS8 deploy).
- **Quarterly** once the gate's base-rate questions are settled (two consecutive zero quarters).

**Out-of-cycle triggers (any one forces a re-measure within a week):**

1. First user ban created or revoked (post-WS2).
2. First `quran-ban:*` row or `QURAN_BAN_ESCALATION_ENABLED` enabled.
3. First `Request blocked by abuse limiter` line after the 429 log fix (any per-route block at all).
4. First `comment_flags` row, or comments-per-author crossing the §7 threshold.
5. First `email_suppression` row (bounce or complaint).
6. A signup spike: > 20 signups/day, or > 5 from one ASN/domain/day, or a dot-alias cluster > 3.
7. A takedown/abuse report: mail to the operator address, GitHub issue, app-store review, Search Console
   message.
8. CF Pareto shift: top-1 IP share of `/api/quran/*` doubles week-over-week, or one ASN crosses 50 %.
9. Any deploy that changes the measurement instrument (WS2, WS8, log-level change) — reset baseline.
10. A DSAR/erasure request touching enforcement data.

Triggers 1–5 are cheap to watch: they are dashboard numbers 1–6 in §7 and they are all zero today, so any
move is signal.

---

## 6. What "zero automated bans" proves

**What it proves.** That the _response mechanism_ is disabled: `QURAN_BAN_ESCALATION_ENABLED` defaults to
false (`settings.rs:349`) and is absent from `docker-compose.yml`; `user_bans` has no writers. Therefore no
automated ban was issued and no manual ban was possible. **NEW**

**What it does not prove.** Anything about abuse. Zero bans means zero _enforcement history_ — an abuse
event that would have been banned is instead an unenforced request, invisible for 429s, and (for
credential abuse) visible only as an unlogged-in-DB WARN. The two statements are different in kind:
"the engine is off" is a code/config fact; "there is no abuse" is a population fact.

**And it is not a proof.** It rests on a code default plus absence from the
_repo's_ compose file. The deployed `.env` is written by Dokploy on the server and is not in the repo
(`docker-compose.yml` header + `env_file: [.env]` at `:35,119`). `QURAN_BAN_ESCALATION_ENABLED=true` plus
`QURAN_BAN_ALLOWLIST` in the server `.env` would enable the engine with zero repo changes. The DB
cross-check is weaker than it looks: the L2 flush DELETEs expired rows every 10 s
(`rate_limit_store.rs:190-197`), so a prod copy proves "no escalation row active or expired in the last
≤ 7 days" (the Long TTL), not "never". **NEW**

**Required phrasing (use verbatim or close):**

> "Zero bans of any kind were issued, because the escalation engine is disabled in the deployed
> configuration (verified against the running container's environment) and no writer for `user_bans`
> exists. This bounds the _enforcement history_, not the abuse rate: abuse that would have been banned
> appears here as an unenforced request, and per-route 429s are not logged at the production log level.
> The absence of cases is by construction, not by observation."

---

## 7. The cheapest dashboard a solo operator will actually use

**Design constraints (repo facts):** no metrics backend; `analytics_v1` is API-only with no web UI
(C5 D30); `/dashboard/summary` currently **errors** because it selects from the missing
`newsletter_subscribers` table (`analytics_v1/controller.rs:875-890`, C3 F8) — do not hang anything on it
until F8 is fixed. So: **one `just` recipe + one small admin endpoint**, nothing else.

**Shape.**

- `just abuse-report` (root justfile) runs `abuse-report.sql` against a **read-only copy** (never the
  live volume) and prints the numbers. Run it weekly for 4 weeks, then monthly.
- `GET /api/admin/abuse/v1/summary` — new, `ROLE_ADMIN`, read-only, **one** SQL statement (subselects),
  returning the same numbers as JSON for on-demand checks. Independent of `analytics_v1` (broken).
- Optional: a boot `warn!` if `ABUSE_RULES_REVIEWED_AT` is > 30 days stale — only after WS2 writes that
  row; until then the row is absent and the warning would be noise. Cross-check with
  `git log --since=30.days -- <rule files>` instead.

**The 5–8 numbers** (all validated against the dev DB schema this session; timestamps are TEXT ISO-8601,
`rate_limit_state` is INTEGER epoch):

1. **Enforcement now** — active bans + created/revoked in 30 d (0 until WS2; the incident-response counter).
2. **Enforcement ledger** — `audit_logs` rows by action, 30 d (0 until the writer lands).
3. **Blocks now** — active blocks / Long blocks / `quran-ban:*` rows (snapshot, not history — label it).
4. **Comment abuse** — flags 7 d, flagged comments, hidden comments, comments 7 d.
5. **Comment concentration** — authors with ≥ 5 comments in 7 d (spam without reports).
6. **Mail abuse** — `email_suppression` by reason, 30 d.
7. **Registration pressure** — signups/day 30 d + verify rate + top domains with dot-alias identity count.
8. **Review staleness** — `ABUSE_RULES_REVIEWED_AT` age (display "unset" until WS2).
   _(Context-only, not a number to act on: multi-account IPs 30 d — CGNAT makes it noisy.)_

```sql
-- abuse-report.sql — read-only, run against a verified copy
.mode column
.headers on

-- 1. enforcement now
SELECT
  (SELECT COUNT(*) FROM user_bans
     WHERE revoked_at IS NULL AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))) AS bans_active,
  (SELECT COUNT(*) FROM user_bans WHERE datetime(created_at) >= datetime('now','-30 days')) AS bans_created_30d,
  (SELECT COUNT(*) FROM user_bans
     WHERE revoked_at IS NOT NULL AND datetime(revoked_at) >= datetime('now','-30 days')) AS bans_revoked_30d;

-- 2. enforcement ledger
SELECT action, COUNT(*) AS n FROM audit_logs
WHERE datetime(created_at) >= datetime('now','-30 days')
GROUP BY 1 ORDER BY n DESC LIMIT 20;

-- 3. blocks right now (snapshot — not history)
SELECT COUNT(*) AS active_blocks,
       COALESCE(SUM(block_scope = 'Long'), 0) AS long_blocks,
       COALESCE(SUM(key LIKE 'quran-ban:%'), 0) AS escalation_rows
FROM rate_limit_state
WHERE block_until_at > CAST(strftime('%s','now') AS INTEGER);

-- 4. comment abuse
SELECT
  (SELECT COUNT(*) FROM comment_flags WHERE datetime(created_at) >= datetime('now','-7 days')) AS flags_7d,
  (SELECT COUNT(DISTINCT comment_id) FROM comment_flags
     WHERE datetime(created_at) >= datetime('now','-7 days')) AS flagged_comments_7d,
  (SELECT COUNT(*) FROM post_comments WHERE hidden = 1) AS hidden_comments,
  (SELECT COUNT(*) FROM post_comments WHERE datetime(created_at) >= datetime('now','-7 days')) AS comments_7d;

-- 5. comment concentration (spam without flags)
SELECT user_id, COUNT(*) AS comments_7d FROM post_comments
WHERE datetime(created_at) >= datetime('now','-7 days')
GROUP BY 1 HAVING COUNT(*) >= 5 ORDER BY comments_7d DESC LIMIT 20;

-- 6. mail abuse
SELECT reason, COUNT(*) AS n, COALESCE(SUM(permanent), 0) AS permanent_n
FROM email_suppression
WHERE datetime(COALESCE(last_seen, created_at)) >= datetime('now','-30 days')
GROUP BY 1 ORDER BY n DESC;

-- 7. registration pressure + gmail dot-alias farming heuristic
SELECT date(created_at) AS day, COUNT(*) AS signups, COALESCE(SUM(is_verified), 0) AS verified
FROM users WHERE datetime(created_at) >= datetime('now','-30 days')
GROUP BY 1 ORDER BY 1 DESC;

SELECT lower(substr(email, instr(email,'@') + 1)) AS domain, COUNT(*) AS n,
       COUNT(DISTINCT CASE
         WHEN lower(substr(email, instr(email,'@') + 1)) IN ('gmail.com','googlemail.com')
         THEN replace(substr(email, 1, instr(email,'@') - 1), '.', '') END) AS dot_alias_identities
FROM users WHERE datetime(created_at) >= datetime('now','-30 days')
GROUP BY 1 ORDER BY n DESC LIMIT 10;

-- 8. review staleness (prints nothing until WS2 writes the row)
SELECT value AS last_reviewed,
       CAST(julianday('now') - julianday(value) AS INTEGER) AS days_stale
FROM app_constants WHERE key = 'ABUSE_RULES_REVIEWED_AT';
```

**Refresh/display:** `just abuse-report` output pasted into the operator's monthly note (date + window +
instrument list); the admin endpoint for on-demand. No chart, no cron service, no metrics backend. The
dashboard exists to make triggers 1–6 in §5 visible, not to be a product.

---

## 8. The single measurement that would most change the plan, and the cheapest way to get it this week

**The measurement:** the **origin-side request distribution of the public Quran content API** — does any
single client identity (IP or ASN) exceed a plausible human rate by 2+ orders of magnitude, and what share
of `/api/quran/*` traffic is cacheable (HIT) vs origin-costing (MISS/DYNAMIC)?

**Why it is the one that matters:** it is the only abuse class that (a) can be happening at scale today
(the content API is public, unauthenticated, and unlimited except the in-process 600/min ceiling that is
invisible in logs), (b) has a real cost (bandwidth/origin work, not accounts), and (c) has a
non-identity fix already scoped (WS7: per-IP limiter, byte budget, edge-cache contract, bulk path). If the
distribution is hot, the plan's priority order changes (cost-shaping before identity); if it is flat, the
honest answer is "no target" and the plan stops cleanly.

**Cheapest path this week:** one Cloudflare GraphQL Analytics API call (Zone Analytics: Read token,
~15 min to create): `httpRequestsAdaptiveGroups` for 30 days, filtered to `/api/quran/%`, grouped by
`clientIP` (top 20) and `clientAsn` (top 10), with `cacheStatus` and `edgeResponseStatus`; plus the total
for the same filter. Read it as a Pareto, not a ratio. Second cheapest (no token): snapshot
`translation_popularity.hits_total` twice, 24 h apart — a rate, but no per-IP attribution.

---

## 9. Corrected measurement plan (replaces C3 §5)

**Budget: 2–3 hours once, 10–15 min per repeat. No deploy, no migration, no code change.**
**Output: one page — the 8 dashboard numbers, the CF Pareto, and the explicit "cannot see" list.**

**Step 0 — Access + instrument check (20 min).**

- CF API token (Zone Analytics: Read, one zone) → store outside the repo. Query the `settings` node for
  `httpRequestsAdaptive` / `firewallEventsAdaptive` `notOlderThan` + `maxDuration` so the retention
  claim is verified for _this_ zone, not read from docs.
- `docker inspect -f '{{.State.StartedAt}}' easyquran_api` → the real log window.
- `docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' easyquran_api | grep -E 'RUST_LOG|QURAN_BAN|TURNSTILE'`
  → the log level and whether the escalation engine is actually enabled in the deployed env (§6).
- Verify the mail provider webhook is configured (else `email_suppression` = 0 is uninformative).

**Step 1 — Cloudflare (30 min), GraphQL not dashboard.**

1. Total requests / document requests / `/api/quran/*` requests, 30 d.
2. `httpRequestsAdaptiveGroups` by `clientIP` (top 20) for `/api/quran/*` → the Pareto.
3. Same by `clientAsn` (top 10).
4. By path class: `/`, `/{surah}*`, `/t/*`, `/api/quran/*`, `/api/*` other, `/offline/*`, static.
5. By `cacheStatus` + `edgeResponseStatus` (HIT/MISS/DYNAMIC; 2xx/4xx/5xx).
6. `firewallEventsAdaptiveGroups` by action/source — **label as sampled**.

**Step 2 — GA4 (10 min), sanity only.**

7. Sessions + users, 30 d. Compute `U = CF document requests ÷ GA4 sessions` once, and report `U` as
   the undercount factor. Do **not** use the ratio as the indicator.

**Step 3 — prod DB copy (30 min), read-only, WAL-safe.**

8. Copy with `sqlite3 'file:/data/easyquran.db?mode=ro' ".backup /out/prod-copy.db"` (or copy `.db`,
   `-wal`, `-shm` together); `PRAGMA integrity_check`; check freshness (max `created_at` vs container
   start). Then run C3's queries (a)–(k) with these fixes: (i) label `rate_limit_state` a snapshot with
   a ≤ 7-day lookback; (ii) keep (e) but add the dot-alias grouping; (iii) keep (f) as context only;
   (iv) drop (g) or mark it blog-only; (v) run (k) twice, 24 h apart, to get a rate.
9. Add the §7 dashboard numbers.

**Step 4 — logs (15 min), with the corrected window and greps.**

10. Window = container start (enumerate old containers if the operator wants more).
11. Greps: `Invalid credentials` and `Login attempt` (per-IP counts — the stuffing signal); `Request
blocked by abuse limiter` with the **first** `scope=` extracted (block counts by key); `Banned user
attempted login`; `Registration failed` / `existing email`; `mail bounce event` / `mail complaint
event` (domain only).

**Step 5 — write it down (10 min).**

12. One paragraph: date, window, instrument list, the 8 numbers, the CF Pareto, `U`, and the
    cannot-see list. Put the CF Pareto and the numbers in the plan doc; put the triggers (§5) next to
    them.

---

## 10. Corrected gate (replaces B2 §5 as the decision procedure; annotates C1 §8.2)

**Gate −1 — now, before WS2/WS8. Observability + base rate. (Runnable this week.)**

- Record **C1/C2/C3 = NOT MEASURABLE** (C1: no case object; C2: no token; C3: no commitment). Do not write
  "pending"; do not arm the instrument.
- **G−1.1 base rate:** the §7 numbers all zero (bans, ledger, blocks, flags, suppressions, staleness) AND
  the CF Pareto flat (no IP > 10× median; no ASN > 50 % of `/api/quran/*`; `R_doc` within 2× of the
  two-week baseline) → **no measured target. Do not build B or C.** Land the three observability fixes
  (429→warn; audit writer; case table with `detection_channel`), justified by "we cannot see", not by
  "we measured abuse". Fix `user_bans` wiring as incident response, not as abuse reduction.
- **G−1.2:** if the CF Pareto is hot → reorder: WS7 (reader cost-shaping + the 429 log line) before any
  identity work; re-measure weekly until flat.
- **G−1.3:** if any §5 trigger fires → out-of-cycle re-measure within a week; the trigger becomes the new
  baseline.

**Gate 0 — after WS2 (bans + case table), before WS8/B. (C1-optionAB §8.2, honestly scoped.)**

- G0.1 ≥ 5 ban-worthy events/quarter in the case record (bans + spam actioned + scrape incidents).
- G0.2 ≥ 10 confirmed evasion cases detected by any **non-token** channel (IP / credential / content).
  (Pre-WS8, this is the only channel that can produce cases — record the channel per case.)
- G0.3 in those cases, per-IP linkage produced ≥ 2 unrelated-account collisions (CGNAT) or failed to link.
- G0.4 < 20 % of linked-account investigations resolve as shared-device.
- Any false → stop; neither B nor C is justified.

**Gate 1 — after WS8 (token live) + 90 days. (B2 §5 + C4, computed from the case table, not `audit_logs`.)**

- **C1** = share of confirmed evasion cases whose decisive event had no linked token, **computed only on
  cases from the WS8-live period**, reported with N, a 95 % Wilson interval, and the detection-channel
  breakdown. (N = 5 gives ±30 points; if N < 10, report "insufficient N", do not gate.)
- **C2** = token recapture rate, keyed on the case/token link (no-FK evidence rows), never on live
  `user_bans` rows (they cascade).
- **C3** = ≥ 1 engineer-day/week committed indefinitely, with `ABUSE_RULES_REVIEWED_AT` fresh and
  `git log` corroboration.
- **C4** (C1-optionAB) = B's linkage edge produced < 5 overturned/human-rejected linkage cases/quarter.
- Any false → C is net-negative by the plan's own rule; the decision to defer stands, and it is recorded
  as a _measured_ deferral, not a deferral.

---

## Findings

### F1 — C1 and C2 cannot be armed before WS2/WS8, and the recommended C1 instrument is already rejected. **NEW**

**Severity: Critical.**
**Evidence:** C3 §4 recommends `audit_logs` reuse for `case_kind`/`token_state`; C5 §4 #20 rejects exactly
that ("no case id, no retention, SET NULL" — A8 F4/F5, A10 F14) and D26 requires `enforcement_cases` +
`case_events`. Verified: `audit_logs.user_id` FK `ON DELETE SET NULL` (`audit_log/model.rs:20-27`);
`user_bans` has zero writers and no admin route; `devices` is an FCM registry, not an abuse token. After
WS2 but before WS8, `token_state` is always `absent` → C1 = 100 % trivially (null instrument).
**Fix:** mark C1/C2 "not measurable"; instrument the D26 case table with `token_state` +
`detection_channel` + the observed token/session id at detection; arm C1 only post-WS8, with N + Wilson
interval + channel breakdown (§10 Gate 1).

### F2 — The 720 h log window does not exist, and the block grep extracts the wrong field. **NEW**

**Severity: High.**
**Evidence:** no `logging:` block in `docker-compose.yml` (full read); containers are recreated on every
master push via Dokploy redeploy (`deploy/README.md:21-28`, `:180-181`; pull-only stack,
`docker-compose.yml:41-50,114-117`), so `docker logs` reaches only the current container; `--since 720h`
silently returns less. `abuse.rs:57-63` sets span `scope = %key_prefix`; `:109-115` re-uses `scope` for
`BlockScope`; a greedy `sed` captures `Temp`/`Long`, not the key.
**Fix:** window = `docker inspect -f '{{.State.StartedAt}}'`; enumerate old containers by id if more is
wanted; add a `logging:` block with `max-size`/`max-file` on the next deploy; grep the **first**
`scope=` (`grep -o 'scope=[^ ]*' | head -1`).

### F3 — Failed logins ARE logged (WARN, with client IP); the plan omits the only existing stuffing signal. **NEW**

**Severity: High.**
**Evidence:** `warn!(client_ip = %secure_ip, "Invalid credentials")` (`auth_v1/controller.rs:226`);
`info!(client_ip, "Login attempt")` (`:129`); `warn!(user_id, "Banned user attempted login")` (`:155`);
registration at `:466,523,527`.
**Fix:** add these greps to Step 4 and count per IP; record that per-email targeting remains invisible
(the line carries IP only).

### F4 — A CF ÷ GA4 ratio is not a trustworthy indicator; use a CF-internal Pareto + ratio and a measured GA4 undercount. **NEW**

**Severity: High.**
**Evidence:** GA4 consent defaults granted (`consent.svelte.ts:16`) but blockers (CSP allows gtag/gstatic,
`hooks.server.ts:82-96`), deniers (`setAnalyticsCollectionEnabled`, `boot/analytics.ts:15`), and JS-less
bots are invisible; the numerator includes SW/offline-pack/prefetch/asset traffic (C5 constraints 37,
`docker-compose.yml:63-66` — SSR bypasses the edge); CF Free counts are adaptively sampled estimates
(Security Events = "sampled logs only"; bot score absent on Free).
**Fix:** §2 corrected computation (`R_doc` inside CF, top-1/top-10 IP share, ASN concentration, GA4
undercount factor `U`), with the error-bar statement.

### F5 — C2's denominator is destroyed by `ON DELETE CASCADE`; the metric improves when the evader deletes the account. **NEW**

**Severity: High.**
**Evidence:** `user_ban/model.rs:34-41` (`user_id … ON DELETE CASCADE`); `get_active_ban` returns one row;
C3's C2 CTE joins live `user_bans`; `audit_logs` subject is `SET NULL`.
**Fix:** evidence rows with no FK / denormalized subject id (D23); key C2 on the case↔token link; record
the ban snapshot inside the case row at decision time.

### F6 — Zero automated bans is a default inference; the deployed `.env` is not in the repo and the DB lookback is ≤ 7 days. **NEW**

**Severity: High.**
**Evidence:** `settings.rs:349` default false; absence from `docker-compose.yml`; but `env_file: [.env]`
(`:35,119`) is Dokploy-written server-side (compose header); L2 flush DELETEs expired rows
(`rate_limit_store.rs:190-197`), so a copy proves only a ≤ 7-day lookback.
**Fix:** verify the running container env; phrase as "zero by configuration (verified)", and state the
enforcement-history vs abuse-rate distinction (§6).

### F7 — The DB copy can be stale or torn on a WAL database. **NEW**

**Severity: Medium-High.**
**Evidence:** `PRAGMA journal_mode=WAL` (`sea_connect.rs:48`); copying only `easyquran.db`, or querying
with `immutable=1` (which ignores the WAL), reads an older state.
**Fix:** `sqlite3 'file:/data/easyquran.db?mode=ro' ".backup /out/prod-copy.db"` (or copy `.db`/`-wal`/`-shm`
together), then `PRAGMA integrity_check` + a freshness check.

### F8 — Step 3's scraping queries are structurally empty for the product that serves traffic. **NEW**

**Severity: Medium.**
**Evidence:** `post_views` is written by blog-post views (`post_v1/controller.rs:357`; posts = 0 in dev);
`translation_popularity` is cumulative with no buckets (`hits_total`, `last_hit_at`); the reader routes
write neither.
**Fix:** the CF Pareto is the scraping instrument (§8); snapshot `translation_popularity` twice for a rate;
land the one-word 429 log fix on the next deploy.

### F9 — The proposed home for the C3 staleness line (`/dashboard/summary`) is currently broken. **NEW**

**Severity: Medium.**
**Evidence:** `dashboard_summary` selects from `newsletter_subscribers`
(`analytics_v1/controller.rs:875-890`); the table does not exist (dev DB table list; C3 F8).
**Fix:** fix F8 first, or add a standalone `/api/admin/abuse/v1/summary` (recommended) — do not hang
measurement on a 500.

### F10 — Quarterly cadence is too slow pre-baseline, and no trigger forces an out-of-cycle re-measure. **NEW**

**Severity: Medium.**
**Evidence:** all counters are zero and the deployment is days old (C3 §3 caveat 2); the plan's own §6.2
says "re-measure quarterly" with no trigger list.
**Fix:** weekly × 4 → monthly → quarterly; the 10 triggers in §5, watched via dashboard numbers 1–6.

### F11 — "Stop if the measurement shows zero" over-claims, and the one documented threat is not measured at all. **NEW**

**Severity: Medium.**
**Evidence:** §4 table (scraping, distributed stuffing, dot-alias farming, unconfigured mail webhook,
takedowns/review-bombing — no step queries any of them); B10 §7/R2-evasion §8.1 (documented threat =
takedowns/review-bombing).
**Fix:** adopt the §4 cheap closes (CF Pareto, log greps, dot-alias SQL, comments-per-author, webhook
check, manual takedown field); phrase the conclusion as §4's honest sentence.

### F12 — The plan has no owner, no instrument list, and an unrealistic time budget. **NEW**

**Severity: Low-Medium.**
**Evidence:** a one-hour budget excludes CF token creation, GA4 property access, host access, and copy
verification; no named owner; no output format.
**Fix:** budget 2–3 h once / 10–15 min per repeat; name the owner; write the one-page output (date, window,
instrument list, numbers, cannot-see list) into the plan doc; put the triggers next to it.

---

## Appendix — repo facts verified this session

- `docker-compose.yml`: no `logging:` block anywhere; `RUST_LOG: ${RUST_LOG:-info}` (`:129`);
  `env_file: [.env]` (`:35,119`); internal SSR base `http://api:8888/quran` (`:63-66`); api container
  `${PROJECT:-easyquran}_api` (`:118`); volume `api_data` (`:160,201`).
- `deploy/README.md:21-28`, `:180-181`: master push → images → Dokploy `compose.redeploy`; no schedule job.
- `auth_v1/controller.rs:129,155,226,466,523,527`: login/registration log lines (failed logins at WARN with IP).
- `crates/rux-request-gate/src/abuse.rs:57-63` vs `:109-115`: span `scope=key` vs event `scope=BlockScope`.
- `services/rate_limit_store.rs:190-197`: DELETE-then-upsert flush (snapshot, not ledger).
- `db/sea_models/user_ban/model.rs:34-41`: `user_id` FK ON DELETE CASCADE; `:24-28` `is_active`.
- `db/sea_models/audit_log/model.rs:20-27`: `user_id` FK ON DELETE SET NULL.
- `db/sea_models/app_constant/model.rs`: `key` UNIQUE, `value`, `source`, `updated_by`, `updated_at`.
- `db/sea_connect.rs:48`: `PRAGMA journal_mode=WAL`.
- `modules/analytics_v1/controller.rs:875-890`: `dashboard_summary` references `newsletter_subscribers`.
- `modules/mail_v1/mod.rs:27` + `controller.rs:120-145`: `/mail/v1/webhook/cloudflare` → suppression;
  `services/mail/router.rs:206-221`: sync-bounce suppression.
- `web/src/lib/firebase/index.ts:12`: `G-ZYL6HY24W6`; `stores/consent.svelte.ts:16`: analytics default true;
  `lib/boot/analytics.ts:24-25`: init then applyConsent.
- Dev DB (read-only, `immutable=1`): table list lacks `newsletter_subscribers`; timestamps are TEXT
  ISO-8601 with `T` + offset; `rate_limit_state` is INTEGER epoch; §7 SQL validated against it.
- Cloudflare docs (fetched 2026-10-12): 2026-10-02 changelog — adaptive datasets retain ≥ 31 days with a
  30-day query window on all plans; Security Events on Free = "sampled logs only"; `firewallEventsAdaptive`
  availability table inconsistent across pages → verify the zone's `settings` node; bot score is not a
  Free-plan field.

_End of D3._
