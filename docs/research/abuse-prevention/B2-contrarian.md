# B2 — Contrarian: is browser/device fingerprinting the right tool for this repo at all?

Auditor: contrarian design audit (round-4 target Q2). Date: 2026-10-12.
Inputs: `DESIGN-v1.md` (Q2), `A1-threat-model.md`, `A2-false-positives.md`, `A8-ops-realism.md`,
`R2-evasion-counters.md`, `R2-fingerprint-science.md`, `R2-ban-operations.md`, plus the working tree at
`/Users/hmziq/os/easyquran` (every repo claim below is verified in-tree with `file:line`).

**Verdict up front.** For the stated goal — "secure authentication and increase the banning of user and
illegal actors" — fingerprinting is the wrong primary tool for this repo. The abuse that actually exists here
(scraping, spam, mass account creation, harassment, CSAM in uploads) is stopped or not stopped by
account/credential/content/ops measures; the fingerprint's unique delta over a server-minted device token is
a population dominated by legitimate users who clear storage, and its one durable capability (a stable
identifier that survives account deletion and IP rotation) is delivered by a token at ~1/5 the cost.
Recommended order: **A now, B next, C only under a measured condition (§5).** Note the design already agrees
on the priority (DESIGN-v1 B1: "Primary defense = account + credential, not the device") — this report
quantifies what "not the device" costs and what the device actually adds.

Cost unit: **dev-day** = one experienced dev-day in this repo, tests included. LOC = rough added/modified lines.

---

## 0. Verified repo facts (the ground truth the costing rests on)

| #   | Fact                                                                                                                                                                                                                                                                                     | Evidence                                                                                                                                                                        |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **Per-credential login throttle already exists.** Key `login:<normalized_email>`, config temp 3 attempts/360 s → 1 h block; long 5/900 s → 24 h block. Runs _before_ password verification.                                                                                              | `rust/backend/api/src/modules/auth_v1/controller.rs:31-46,133-140`                                                                                                              |
| 2   | **Other per-identity keys already exist**: `totp:<user_id>`, `totp_setup:<user_id>`, `reauth:<user_id>`, `forgot_password_request:account:<email>`, `forgot_password_verify:account:<email>`, `email_verify:<user_id>`, `email_verification:<user_id>`, `comment:create:user:<user_id>`. | `auth_v1/controller.rs:299,411,443,582,813,918`; `forgot_password_v1/controller.rs:339,505`; `email_verification_v1/controller.rs:48,90`; `post_comment_v1/controller.rs:62-66` |
| 3   | **The limiter counts every attempt, success included** (`abuse_check` appends an attempt event on every call; 3rd call blocks at `temp_block_attempts: 3`). Nothing clears the counter on a successful login.                                                                            | `crates/rux-request-gate/src/store.rs:36-40,46-49`; `abuse.rs:64-125,220-233`                                                                                                   |
| 4   | **Per-route ceilings are per-IP and generous**: auth nest 100/min, post 200/min, comment 100/min, user/billing/oauth 60/min, media/search 30/min, quran content 600/min (fixed bucket, escalation default-OFF).                                                                          | `src/router.rs:55-176`; `src/main.rs:860-912`; `config/settings.rs:306-349`                                                                                                     |
| 5   | **Registration has no per-identity limiter at all.** Handler calls only Turnstile; the only cap is the shared 100/min/IP auth-nest bucket. Turnstile is a no-op unless `TURNSTILE_SECRET_KEY` is set.                                                                                    | `auth_v1/controller.rs:456-530`; `services/turnstile.rs:17-38`; `router.rs:75`                                                                                                  |
| 6   | **No API keys exist for product APIs.** `api_key` appears only in third-party billing/moderation clients and the SSR `INTERNAL_QURAN_API_TOKEN`; the Quran API is unauthenticated public content.                                                                                        | `grep api_key`; `config/settings.rs:461-499`                                                                                                                                    |
| 7   | **`user_bans` is fully dormant: no account can be banned today.** Table + `create/revoke/admin_list/get_active_ban` + `NewUserBan::permanent()` exist; enforcement `check_ban` → `not_banned()` is wired; **zero callers create or revoke**, no admin route.                             | `db/sea_models/user_ban/{slice,actions}.rs`; `services/auth.rs:531`; `middlewares/auth_guard.rs`; grep: no writer                                                               |
| 8   | **The only existing ban surface is IP-only** (`/admin/bans`, `rate_limit_state` keys; `BanUnit` parses CIDR only, no reason/actor/case).                                                                                                                                                 | `modules/admin_bans_v1/{dto,controller}.rs`; `A8 §0`                                                                                                                            |
| 9   | **Session list already exists** (`/sessions/list`, `/sessions/terminate/{id}`) and returns raw UA + IP + `isCurrent`. No terminate-all; no new-session notification.                                                                                                                     | `auth_v1/mod.rs:40-47`; `auth_v1/controller.rs:976-1040`                                                                                                                        |
| 10  | **Writes already require verified email**: comments and media are behind `auth_guard::verified` / `verified_with_role::<ROLE_AUTHOR>`.                                                                                                                                                   | `post_comment_v1/mod.rs:10-17`; `media_v1/mod.rs:14-20`                                                                                                                         |
| 11  | **Fresh-auth step-up already guards 2FA setup/verify/disable**; self-service password change does not exist (reset only, via emailed code; password change is admin-only).                                                                                                               | `auth_v1/controller.rs:411-454,582,817,931`; `user_v1/mod.rs:31-32`; `forgot_password_v1/*`                                                                                     |
| 12  | **`expectedUserId` guards already exist** where cross-account writes happen: push-device registry, bookmarks, all four OAuth modules. **No mail outbox exists anywhere** (mail sends inline in `tokio::spawn`).                                                                          | `device_v1/controller.rs:88-94`; `bookmark_v1/controller.rs:131-132`; `github/discord/facebook_auth_v1`; `grep outbox` → ∅                                                      |
| 13  | **Image moderation is a NoOp by default** (`NoOpModerator` returns `safe: true`); no CSAM hash list, no NCMEC path, no `legal_holds`.                                                                                                                                                    | `services/image_moderation/mod.rs:37-47`                                                                                                                                        |
| 14  | **`audit_logs` has zero writers.**                                                                                                                                                                                                                                                       | `A8 §0`; grep                                                                                                                                                                   |
| 15  | **A second "device" concept already exists**: `device_v1` = FCM push-token registry, client-minted `token TEXT UNIQUE`, per-user cap 20, re-register transfers ownership (shared devices are explicitly normal). Not an abuse token.                                                     | `device_v1/controller.rs:17,35-94,167-193`                                                                                                                                      |
| 16  | **The abuse limiter is skipped entirely outside production** (non-prod returns `Ok(())`).                                                                                                                                                                                                | `services/abuse_limiter.rs:39-49`                                                                                                                                               |
| 17  | **The limiter fails closed**: store error → 503 on the gated endpoint.                                                                                                                                                                                                                   | `services/abuse_limiter.rs:69-73`; `rux-request-gate/src/abuse.rs:215-218`                                                                                                      |

Consequence: A8's ranking ("Per-credential rate limits … missing today … v1 — do first") is **stale**. The
per-credential login limit shipped, along with five more account-keyed keys. The real gaps are registration
admission, ban wiring, and the _correctness_ of the existing limiter (§4, findings F1–F3).

---

## 1. Three options, fully costed

### OPTION A — Credential/account-centric only (no fingerprint, no device token)

| #   | Item                                                                                                                                                                                                                                                               | Abuse it stops                                                                                                                                     | Implementation cost (files, size, days)                                                                                                                | Residual evasion                                                                                                                                              |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1  | **Wire `user_bans`**: `POST/GET/DELETE /admin/users/bans` (list/create/revoke), reason + expiry + `banned_by`, audit row                                                                                                                                           | Nothing by itself — it is the **only way to terminate an account**. Today enforcement is impossible (fact 7). Makes every later measure meaningful | New `modules/admin_user_bans_v1/{mod,dto,controller}.rs` (~450 LOC), `modules/mod.rs`, `router.rs`, reuse `user_ban` actions, ~150 test LOC. **2-3 d** | 100%: banned actor re-registers. Ban cache lag ≤5 min.                                                                                                        |
| A2  | **Registration admission control + limiter correctness**: per-IP/day cap, per-email-domain/day cap, lower the register burst (100/min → 5-10/min), make Turnstile fail-closed in prod; fix the login limiter to count _failures_ and add a `(email, IP)` dimension | Mass account creation (the substrate under spam/harassment/bombing); account-lockout DoS from fact 3                                               | `auth_v1/controller.rs`, `services/abuse_limiter.rs` + config, `router.rs`, `services/turnstile.rs`, tests (~150-250 LOC). **1-2 d**                   | Residential proxies rotate IPs; disposable domains not on a list; real inboxes cost nothing                                                                   |
| A3  | **Turnstile on more surfaces**: login (after N failures), forgot-password request, comment create. Generalize `verify_registration` → `verify(action, ip)`; web widget + fallback                                                                                  | Scripted login floods / naive credential stuffing; reset-mail bombing                                                                              | `services/turnstile.rs`, `auth_v1`, `forgot_password_v1`, web widget + `(auth)` routes (~300 LOC). **1.5-2 d**                                         | Solver services (~65% of ATO uses advanced automation, R2 §7.2); challenge blocked on censored networks → audience harm (A2 F8); needs a non-CAPTCHA fallback |
| A4  | **Step-up on credential change**: mostly present (fact 11). Remaining work = make fresh-auth a hard precondition on any future email/phone change, and fix the process-global `MAX_GRANTS = 10_000` cap → per-user caps + durable state                            | ATO-driven credential changes once a session is stolen; grant-map saturation DoS                                                                   | `services/fresh_auth.rs`, tests (~150 LOC). **1-2 d**                                                                                                  | Stolen session + stolen mailbox                                                                                                                               |
| A5  | **Session list improvements**: terminate-all-other-sessions, coarse platform label instead of raw UA (also closes A8 finding 6), new-session email notification + `NotificationKind`                                                                               | Persistent ATO sessions; user-visible remediation; email tripwire is the strongest cheap ATO alarm                                                 | `auth_v1/{controller,mod}.rs`, `services/mail/templates/new_session.html` + router, web `(account)` page (~350 LOC). **2-3 d**                         | Attacker with mailbox access; unverified users have no email                                                                                                  |
| A6  | **`expectedUserId` extension + mail outbox**: audit remaining mutation endpoints (0.5 d). Outbox = new table + worker + idempotency keys                                                                                                                           | expectedUserId: cross-account write confusion (mostly present). Outbox: **abuse reduction ≈ 0** — reliability feature                              | Audit 0.5 d; outbox ~300 LOC, **2-3 d** (optional)                                                                                                     | n/a                                                                                                                                                           |
| A7a | **Comment moderation hardening**: stricter caps for accounts <24 h (e.g. 5/day), link/repetition heuristic → auto-flag, flag-threshold auto-hide, admin bulk actions                                                                                               | Spam comments from fresh accounts; slow-roll spam                                                                                                  | `post_comment_v1/{controller,validator}.rs`, config (~250 LOC). **1-2 d**                                                                              | Determined actor posting slowly; novel spam wording                                                                                                           |
| A7b | **Real content moderation + CSAM path**: replace `NoOpModerator` with a required provider in prod; add hash-match interface (industry lists), report/NCMEC SOP, retention class for CSAM-adjacent evidence                                                         | CSAM/illegal uploads — **the only item that actually detects them**                                                                                | `services/image_moderation/*`, `media_v1/controller.rs`, migration for retention flag, SOP doc (~400 LOC + doc). **3-5 d**                             | Novel content; text illegality needs report + human review                                                                                                    |
| A8  | **Email-verification hardening**: disposable-domain list + refresh job, per-domain/24 h signup cap, catch-all detection optional (keyed code hash, per-user limiter, duplicate masking, verified write-gate already exist)                                         | Disposable-inbox mass creation                                                                                                                     | New `services/email_policy.rs` + register hook + tests (~150 LOC). **1-2 d**                                                                           | Gmail plus-aliases (aliases are rejected in prod by default — `email_aliases_enabled()`), real accounts                                                       |
| A9  | **Invite/allowlist (emergency switch only)**: registration mode flag (open/invite/closed) + invites table + admin create/revoke + mail template + web + i18n                                                                                                       | Mass account creation near-fully _when enabled_                                                                                                    | Migration + `modules/invite_v1/*` or auth extension + web + i18n (~700-900 LOC). **5-8 d**                                                             | Invite resale; growth/mission cost — not a default                                                                                                            |
| A10 | **Abuse logging**: wire `audit_logs` writers (admin mutations, ban lifecycle, auth events, evidence reads), indices + retention                                                                                                                                    | Nothing directly; makes A1/A5/A7 defensible (DSA Art 17/20, LE, disputes)                                                                          | New `services/audit.rs` + call sites + migration (~300 LOC). **2-3 d**                                                                                 | n/a                                                                                                                                                           |

**Option A totals.** Core (A1+A2+A3+A7a+A10): **~8-12 dev-days**. Full list (incl. A4-A9): **~22-34 dev-days**.
No new privacy surface, no DPIA, no collector, no new permanent maintenance.

### OPTION B — Option A + light device (server-minted opaque token, **no component fingerprint**)

| #   | Item                                                                                                                                                                                                                                 | Detail       | Cost    |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------ | ------- |
| B1  | Migration: `device_tokens` (id ULID, `token_hash` UNIQUE, first_seen, last_seen, revoked_at) + `device_account_links` (token_id, user_id, first_seen, last_seen, source)                                                             | ~130 LOC     | 0.5-1 d |
| B2  | `services/device_token.rs`: mint ≥128-bit random, store HMAC-SHA256 with a purpose subkey, verify, bind, rotate, revoke; **HttpOnly; Secure; SameSite=Lax; Path=/api** cookie (path-scoped → `/t/**` cache untouched, A1 F4)         | ~300 LOC     | 2-3 d   |
| B3  | Hooks: bind token at login/register; on register, if the token is linked to an active-banned account → flag for review (never auto-ban); token-keyed limiter keys (`login:device:<id>` alongside `login:<email>`, comment per token) | ~200 LOC     | 1-2 d   |
| B4  | Admin evidence: token → linked accounts (one hop, cap 20) on the A1 case view                                                                                                                                                        | ~150 LOC     | 1 d     |
| B5  | Web: no collector. Device list on the account page (or extend the session list), new-device email template                                                                                                                           | ~200 LOC     | 1 d     |
| B6  | Guard tests: cookie flags, token never on reader routes, retention job, no component collection                                                                                                                                      | ~200 LOC     | 1 d     |
| B7  | Policy: privacy-policy paragraph + retention decision (token 12 mo, links 12 mo, delete on request) + ePrivacy note; DSA Art 17 template only if it feeds enforcement                                                                | 1-2 d policy | 1-2 d   |

**Option B totals.** ~1,200 LOC, **6-10 dev-days** engineering + **1-2 d** policy, on top of A. Running cost
≈ 0.5 d/month (cleanup + rotation; no rule maintenance). Residual: cookie clearing (32% of browser instances
in 8 months, Li & Cao), incognito, anti-detect profiles, iOS storage isolation between Safari and home-screen
apps — all documented in A2 B1-B4.

### OPTION C — Full DESIGN-v1

| #   | Artifact (per DESIGN-v1)                                                                                                                                                                                          | Cost                                                  |
| --- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| C1  | Client collector + provider seam + lazy worker + closed typed payload + no-reader-route guard                                                                                                                     | 5-8 d                                                 |
| C2  | Server tables + models + migration: `device_records`, `device_sessions`, `device_evidence`, `enforcement_cases`, `device_bans`, `legal_holds`                                                                     | 5-8 d                                                 |
| C3  | Crypto: `DEVICE_KEY_PEPPER`, HKDF subkeys, `Zeroizing`, token mint/revoke                                                                                                                                         | 2-3 d                                                 |
| C4  | Edge signing: Ed25519 in the Worker, canonical header list, `kid` rotation, origin verification, fail-open reads                                                                                                  | 5-10 d (+ a new site-wide outage failure mode, A1 F6) |
| C5  | Risk engine + L0-L7 ladder + hysteresis + versioned rules + shadow mode                                                                                                                                           | 5-8 d                                                 |
| C6  | Admin API + bun TUI/CLI + role seat (`ROLE_MODERATOR`)                                                                                                                                                            | 4-6 d                                                 |
| C7  | Appeals: table + public `POST /appeal/v1/submit` + tokenized page + transactional mail template + `NotificationKind::Enforcement`                                                                                 | 4-6 d                                                 |
| C8  | Retention sweeper + auto-close/aging jobs + boot invariants                                                                                                                                                       | 3-4 d                                                 |
| C9  | Machine guards (no device × reader-data join; no payload on reader routes) + guard tests                                                                                                                          | 2-3 d                                                 |
| C10 | Metrics endpoint (precision, overturn, queue aging, TTD)                                                                                                                                                          | 1-2 d                                                 |
| C11 | i18n: notices + appeal page in readers' languages                                                                                                                                                                 | 2-3 d + ongoing translation                           |
| C12 | Legal/policy artifacts: DPIA, LIA, ROPA, retention schedule, privacy-policy rewrite + device paragraph, Art 17 statement-of-reasons template, appeal SOP, LE preservation SOP + `legal_holds`, DSA Art 17 posture | 5-10 d (likely external review)                       |

**Option C totals.** C-only engineering: **38-61 dev-days**; plus A+B prerequisites (27-44) → **65-105 dev-days
(3-5 months solo)** + **5-10 policy days** + **1-2 dev-days/month forever** (rule rebuild, browser-feature DB,
FP triage, appeals, retention job). Every artifact above is required by the design itself (B2-B7); none is
optional.

**Cost per unit of unique abuse reduction.** A+B ≈ 27-44 dev-days buys ~85-90% of the _enforceable_ reduction
(§2). C adds 38-61 dev-days + a permanent maintenance bill for the delta in §3, which is one narrow
population: evaders who clear the token but do not spoof components.

---

## 2. Scenario matrix: what actually stops each abuse

Legend: ✅ stops (or stops to an accepted residual) · ◐ partial · ✗ theatre (no measurable effect).

| Scenario                                          | A            | B   | C   | What actually stops it                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------- | ------------ | --- | --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Credential stuffing**                           | ✅           | ◐   | ✗   | Already: `login:<email>` throttle + 100/min/IP. Add: login Turnstile, new-session email, per-ASN aggregate. C adds nothing — DESIGN-v1 B1 itself says per-credential is the fix. Coherence detection catches _bot frameworks_, not stuffing (stuffers use real browsers via OpenBullet).                                                                               |
| **Brute force on one account**                    | ✅ (already) | ✗   | ✗   | `login:<email>`: 3/6 min → 1 h, 5/15 min → 24 h. **But it currently counts successful logins and locks the victim out (F2)** — fix it, don't extend it. B/C add nothing; C's "new device at login" step-up is attacker-triggerable (A1 F7).                                                                                                                            |
| **Mass account creation**                         | ◐            | ◐   | ◐   | Today 100 signups/min/IP = **144 k accounts/day/IP**, no per-identity cap, Turnstile optional (fact 5). A2/A8 fix the substrate. B adds a real per-token cap (cookie clear resets). C's cohort detection is FP-dense (18.5% mobile uniqueness → 81.5% collisions). Winner: A then B.                                                                                   |
| **Scraping the Quran text**                       | ◐            | ✗   | ✗   | Public-domain text, mission says distribute it. Only IP cost-shaping (600/min, escalation OFF) + a cheap published bulk path (R2 §9 row 17, §10 item 5). B has no token for anonymous readers. **C explicitly excludes reader routes (DESIGN-v1 B2) — C is not a scraping defense.**                                                                                   |
| **Spam comments**                                 | ✅           | ◐   | ◐   | Verified-email gate + `comment:create:user:<id>` (20/min→10 min, 100/h→1 h) + moderation queue + flags exist. A7a adds fresh-account caps + heuristics. B adds per-token limits + fresh-account-on-known-token flagging. C's device evidence is corroboration, never the signal — content is.                                                                          |
| **Review-bombing / harassment from new accounts** | ◐            | ✅  | ◐   | Repo has **no ratings** (harassment = comments/profile). A: verified gate, moderation, bans. B: **one token → N fresh accounts is the strongest cheap signal** — exactly the KUNAI pattern (R2 §8.1: 8.1→1.7 in a day by one operator). C adds the same edge with more shared-device FP risk (A2 F3).                                                                  |
| **Ban evasion, one determined actor**             | ✗            | ✅  | ◐   | A: nothing (new account = new user). B: recaptures the evader who keeps their browser and clears nothing — the naive majority (R2-ban: "recapture naive evasion"). C adds only the token-clearing non-spoofer (mostly legitimate privacy clearers) + sloppy component spoofers; anti-detect browser/VM defeats B _and_ C.                                              |
| **Coordinated inauthentic behavior**              | ✗            | ◐   | ✗   | Neither fingerprint option stops CIB. B's token graph gives one high-precision edge; the literature's signal is **co-action timing + content**, not hardware (R2 §7.3: "coordination is the very thing you are measuring"). C's component cohort edges are the exact edge R2 §6.2 warns "links thousands of unrelated users"; DESIGN-v1 has **no co-action pipeline**. |
| **CSAM / illegal content posting**                | ✅           | ◐   | ✗   | Detection is a **content** problem: real image moderation (NoOp today, fact 13) + industry hash list + NCMEC SOP + preservation. B contributes recapture after a CSAM ban (real but small). C contributes nothing to detection and makes the retention class harder (A8 §7: 30 d key vs §2703 90+90 vs §2258A 1 yr).                                                   |
| **Impersonation**                                 | ✅           | ◐   | ✗   | Reserved/official names, report flow, moderation, bans, off-platform DMCA (R2 §8.2). B catches repeat actors who reuse a token. C adds nothing unique.                                                                                                                                                                                                                 |

---

## 3. Where the fingerprint adds real, measurable value over A/B (honest accounting)

1. **Pre-auth → post-auth linkage when no token exists.** Catches the storage-clearing non-spoofer at
   registration. Population size: 32% of browser instances clear cookies in 8 months (Li & Cao, IMC 2020) and
   Safari ITP deletes script-writable storage after 7 days — i.e. the set is dominated by _legitimate_
   privacy-protecting users, not evaders. An attacker who wants to evade simply clears the token; an attacker
   who does not clear it is caught by B.
2. **Cohort detection without any account or cookie** (N accounts sharing one component profile). Real, and
   exactly the false-positive-dense edge: mobile fingerprints are 18.5% unique (Gómez-Boix et al., WWW 2018,
   _with_ canvas/WebGL/audio, which this design excludes), so >81.5% of mobile component keys are shared.
3. **Fuzzy re-link after updates** (FP-Stalker-style evolution-aware matching). Measured F1 **75.9% at 1M
   fingerprints** and dropping as the DB grows; the learning variant cannot scale past ~300 K (Li & Cao).
   This app is far below 1 M; a small team would run a worse model with permanent maintenance.
4. **Anti-detect / automation coherence detection.** FP-Inconsistent (IMC'25): **52.93% evasion vs DataDome,
   44.56% vs BotD**; space+time coherence rules cut evasion by **48.11% / 44.95%**. Strong result — but it is
   _bot detection at request time_, requires a maintained per-version browser knowledge base, and R2's own
   advice is "rebuild the rule set quarterly". It does not transfer to ban evasion (a human evader with a
   normal browser is coherent by construction).
5. **Nothing for**: scraping (excluded by design), CSAM detection, impersonation, CIB (no co-action pipeline).
6. **No measurable device-ban precision exists to justify the cost.** No platform and no vendor publishes a
   device-fingerprint ban false-positive rate (R2-ban §4); the design's ladder would be tuned blind. The
   strongest published _positive_ result for device enforcement is a vendor case study (Schoolhouse) —
   marketing, not measurement.

Net: the fingerprint's marginal detection over B is "evaders who clear storage but don't spoof hardware,"
whose real-world composition is dominated by legitimate users; its marginal false-positive surface is
cohort bans on shared hardware, which for this app's audience (family phones, madrasas, CGNAT — India: 30% of
adults do not own a phone, 56% of non-owners rely on a family member's phone, Pew 2019) is the core
population, not the edge.

---

## 4. Cheapest intervention with the largest abuse reduction — verifying A8's claim

**A8 claimed**: "Per-credential rate limits (limiter keys are IP/route today) … research #1; also fixes
credential stuffing … v1 — do first."

**Verified false for the auth surface.** Exact keys that exist today:

| Key                                                                                          | Where                                 | Config                             |
| -------------------------------------------------------------------------------------------- | ------------------------------------- | ---------------------------------- |
| `login:<normalized_email>`                                                                   | `auth_v1/controller.rs:133-140`       | 3/360 s → 1 h; 5/900 s → 24 h      |
| `totp:<user_id>`, `totp_setup:<user_id>`                                                     | `:299,443,582,813,918`                | same cfg                           |
| `reauth:<user_id>`                                                                           | `:411`                                | same cfg                           |
| `forgot_password_request:account:<email>`, `forgot_password_verify:account:<email>`          | `forgot_password_v1:339,505`          | same cfg                           |
| `forgot_password:<ip>`, `forgot_password_verify:<ip>`, `forgot_password:reset_token:<token>` | `:326,429,454,128`                    | same cfg                           |
| `email_verify:<user_id>`, `email_verification:<user_id>`                                     | `email_verification_v1:48,90`         | same cfg                           |
| `comment:create:user:<user_id>`                                                              | `post_comment_v1:62-66`               | 20/60 s → 10 min; 100/3600 s → 1 h |
| Per-IP per-route (every nest)                                                                | `router.rs:55-176`, `main.rs:860-912` | 30-600/min, matched-route buckets  |

So per-credential limits exist for every auth surface **except registration**. There are **no per-API-key
limits because there are no API keys** (fact 6).

**The actual cheapest high-impact intervention, in order:**

1. **Fix the existing login limiter (0.5-1 d).** It counts successful logins (fact 3) and keys only on the
   email, so (a) three logins in 6 minutes locks a legitimate user out for an hour — a family sharing one
   account trips it; (b) **5 unauthenticated requests to any known email lock that account for 24 hours** —
   a free lockout weapon. Change to: count _failures_ only, add a `(email, IP)` soft dimension, cap the first
   block at minutes, notify the account owner, and never let a successful auth consume budget.
2. **Registration admission control (1-2 d).** 100/min/IP with no per-identity cap and an optional no-op
   Turnstile is the substrate for spam, harassment, and bombing. Per-IP/day + per-domain/day caps + mandatory
   Turnstile in prod removes it. This is the single largest abuse-reduction-per-day item in the repo.
3. **Wire `user_bans` + `/admin/users/bans` (2-3 d).** Today no account can be banned at all (fact 7). Until
   this exists, every other enforcement measure is theoretical; after it, a moderator can actually terminate
   an abuser.

Ranking: **#2 (registration) ≥ #3 (ban wiring) > #1 (limiter fix, safety-critical) > every fingerprint item.**

---

## 5. Recommendation + falsifiable condition

**Build A now** (core subset, ~8-12 dev-days): A1 ban wiring, A2 registration admission + limiter fix, A3
login/reset Turnstile, A7a comment hardening, A10 audit writers. Add A7b (real image moderation + CSAM path)
immediately if the app accepts uploads from untrusted users — it is the only CSAM control that exists.

**Then B** (~6-10 dev-days): server-minted HttpOnly token, bound at login/registration; token-keyed rate
limits (this is what makes IP rotation stop working — R2 §2.3); new-device email; admin evidence panel;
privacy paragraph. Do **not** build the collector, `device_key`, edge signing, or the risk engine.

**Defer C** under a measured condition. **Build C only if, after 90 days of running A then B, all three hold:**

- **C1 (population):** ≥30% of _confirmed_ ban-evasion cases involve a missing/cleared device token — i.e. the
  fingerprint-only population is real, not hypothetical. (Measure: of recapture cases, how many had no token?)
- **C2 (headroom):** device-token recapture of suspected evaders is <40% (R2-ban's target). If the token
  already catches most, the fingerprint adds little.
- **C3 (capacity):** ≥1 engineer-day/week is committed indefinitely to rule maintenance, the per-version
  browser-feature database, FP triage, and appeal handling. No commitment → rules rot, false positives stay.

**If any is false, C is net-negative:** C1 false → nothing to catch; C2 false → token suffices; C3 false →
maintenance debt. A second, independent justification for C would be a documented funded campaign against
_this_ app (R2 §8.1 found **no** public incident of CIB/scraping campaigns against Quran apps) or a new
high-value gated surface (payments, private groups, certificates). Absent either, C is a 3-5 month build
against a threat the research says has never been documented for this class of app.

**Rollout order:** registration caps + limiter fix (week 1) → ban wiring + audit (week 2) → Turnstile + session
notifications + comment hardening (week 3) → B token (weeks 4-6) → measure C1/C2 for 90 days → decide on C.

---

## 6. What a fingerprint does that NO account-centric measure can do

Plainly: the list is short and weak.

1. **Re-identify a user after they explicitly cleared site data.** This is the only capability that is
   genuinely unique to fingerprinting — and it is a _privacy harm_, not a security win, for an app whose users
   may clear storage precisely because the phone is shared or the jurisdiction is hostile (A2 §4.2).
2. **Detect cohort sharing without any account or cookie** (N accounts, one component profile). Real, but the
   FP-dense edge: 18.5% mobile uniqueness means most collisions are strangers, and client-chosen components
   make the key attacker-weaponizable (A1 F2, A2 F2).
3. **Nothing else.** It cannot distinguish "same person, new account" from "same device, different person"
   (R2-ban §2: "the fingerprint proves _device reuse_, not _person reuse_"), cannot see scrapers (excluded by
   design), cannot detect CSAM, cannot stop impersonation, and cannot detect CIB (no co-action pipeline).

The one durable capability that matters — a stable, server-controlled identifier that survives account
deletion, IP rotation, and cookie-level rotation _by the attacker_ — is delivered by a **token**, not a
fingerprint. The fingerprint's unique delta is defeating the user's own privacy choice, and this app's
audience is the population with the most reason to make that choice.

---

## Findings

### F1 — A8's #1 recommendation is already implemented; the roadmap ranking is stale. **NEW**

**Severity: High.**
**Evidence:** `login:<normalized_email>` throttle exists (`auth_v1/controller.rs:133-140`) plus five more
account-keyed keys (fact 2); per-route IP layers exist everywhere (`router.rs:55-176`). A8 §9 and R2 §10 item 1
both rank "per-credential limits — missing today" as the top v1 item.
**Implication:** following the stale ranking spends the first week re-implementing what exists while the real
gaps (registration admission, ban wiring) stay open. Re-rank: registration cap > ban wiring > limiter fix.

### F2 — The existing per-credential login limiter counts successful logins and is a free 24-hour account-lockout weapon. **NEW**

**Severity: High.**
**Evidence:** `abuse_check` appends an attempt event on every call, success or failure
(`rux-request-gate/src/store.rs:36-49`; `abuse.rs:64-125,220-233`); config 3 attempts/360 s → 1 h, 5/900 s →
24 h (`auth_v1/controller.rs:31-38`); key is the email alone and runs before password verification
(`:133-144`); nothing clears the counter on success. So: a user logging in on three devices in 6 minutes gets
a 1-hour block; 5 anonymous POSTs to `/auth/v1/log_in` with any known email lock that account for 24 hours.
**Implication:** this is an availability bug and a DoS primitive that exists _because_ of the per-credential
limit. Fix before adding any more account-keyed limits: failures only, `(email, IP)` soft dimension, short
first block, owner notification, and a recovery path.

### F3 — Registration is the least-defended chokepoint: 100 signups/min/IP, no per-identity cap, Turnstile optional and a no-op by default. **NEW**

**Severity: High.**
**Evidence:** the register handler calls only `turnstile::verify_registration` and creates the user
(`auth_v1/controller.rs:456-530`); the only cap is the shared auth-nest layer of 100/min
(`router.rs:75`); Turnstile is disabled unless `TURNSTILE_SECRET_KEY` is configured
(`services/turnstile.rs:17-38`). 100/min × 60 × 24 = 144,000 accounts/day/IP. The verified-email write gate
(fact 10) raises the cost to "one disposable inbox per account", which is free.
**Implication:** one to two days here removes the substrate under spam, harassment, and rating/review abuse —
more abuse reduction than every fingerprint item in DESIGN-v1 combined.

### F4 — The device axis is excluded from exactly the traffic that generates the abuse the design exists to stop. **NEW**

**Severity: High.**
**Evidence:** DESIGN-v1 B2: "Never on reader routes: no payload on `/t/**`, `/{surah}/t/**`, quran content API";
`quran_v1` routes are public and unauthenticated (`quran_v1/mod.rs:13-50`); scrapers do not run JS. A1 F5
reached the same conclusion for v0 and v1 did not change it.
**Implication:** C cannot defend the reader path — and its own rules forbid the one fix that would (an
anonymous beacon). For scraping, the honest posture is cost-shaping plus a fast legitimate bulk path, which is
Option A, not C.

### F5 — The fingerprint's marginal detection over a device token is a population dominated by legitimate users. **NEW**

**Severity: High.**
**Evidence:** 45.52% of fingerprints change in one day (FP-Stalker); mobile uniqueness 18.5% (Gómez-Boix);
32% of browser instances clear cookies in 8 months (Li & Cao); FP-Stalker F1 75.9% at 1M DBs and the learning
variant cannot scale past ~300 K. The only actor C catches that B does not is one who clears the token but
does not spoof components — which describes Safari ITP users and privacy clearers, not evaders (evaders who
care clear or spoof deliberately; evaders who do not care are caught by B).
**Implication:** C's incremental coverage is small and its incremental FP surface is large; B captures the
evader that matters at ~1/5 the cost.

### F6 — No published device-ban false-positive rate exists, and the FP burden lands on the shared-device core audience. **NEW**

**Severity: Critical.**
**Evidence:** R2-ban §4: no platform and no vendor publishes a device-fingerprint ban FP rate; R2-ban §1: the
only device-enforcement evidence is a vendor case study (Schoolhouse). A2 Part 1 base rates: India 30% of
adults do not own a phone; median 48% (India 56%) of non-owners rely on a family member's phone (Pew 2019);
76% of rural shared-device users are under 19 (IAMAI/Kantar 2024). A2's inheritance lemma: every device-side
corroborator is inherited by co-users.
**Implication:** device enforcement is a blind bet whose downside is concentrated on minors, families, and
madrasas — the app's core readers. This is a values/legal risk as much as an FP risk, and it is why the design
correctly demotes device to evidence-only — which then collapses C to B plus noise.

### F7 — The account-ban primitive is dormant: today no account can be banned. **NEW (confirms A8 F1; verified)**

**Severity: High.**
**Evidence:** `user_bans` table + `create/revoke/admin_list/get_active_ban` + `NewUserBan::permanent()` exist
(`db/sea_models/user_ban/{slice,actions}.rs`); enforcement `check_ban` → `not_banned()` is wired
(`services/auth.rs:531`); **zero callers create or revoke**, no admin route (grep). The only live ban surface
is IP-only (`admin_bans_v1`, `rate_limit_state` keys).
**Implication:** enforcement is impossible today. Wiring `user_bans` is the highest-value _enforcement_ item
in any option, and it is a prerequisite for measuring evasion at all.

### F8 — Coherence/bot detection is a quarterly-maintenance product, not a feature. **NEW**

**Severity: Medium.**
**Evidence:** R2 §3.2 ranks space/time consistency #1-2 and §9 rows 3-4 say the fix "moves the leak, it does
not remove it" and "rebuild the rule set quarterly"; FP-Inconsistent's ~48% evasion reduction was measured for
commercial vendors on a honey site with 20 bot services, not for ban evasion at this app's scale.
**Implication:** any C plan must budget permanent maintenance (≥1 d/week) or the rules rot while the FP cost
stays. This is the C3 condition in §5.

### F9 — For this app's actual abuse (spam, harassment, CIB), the token graph plus co-action timing beats component matching. **NEW**

**Severity: Medium-High.**
**Evidence:** R2 §6.2: exact-match components over shared IPs "link thousands of unrelated users" and the
"ant trap" lets an attacker poison the graph; R2 §7.3: CIB detection is co-action edges in 2-minute buckets,
backbone-filtered, cluster-then-review — none of which needs hardware; A8 §6 recommends a bounded 2-hop
linked-accounts panel and calls automated clustering a no-go at this size.
**Implication:** build B's token edges + a 2-minute co-action window (a few indexed queries); skip component
edges. C's device graph is the one edge type the research says will produce the worst false positives.

### F10 — "Outbox" does not exist and is not an abuse control; `expectedUserId` guards already exist where they matter. **NEW**

**Severity: Low.**
**Evidence:** `grep outbox` across the API → no results; mail sends inline in `tokio::spawn`
(`auth_v1/controller.rs:502-517`). `expected_user_id` guards exist in `device_v1/controller.rs:88-94`,
`bookmark_v1/controller.rs:131-132`, and all four OAuth modules.
**Implication:** classify the outbox as a reliability feature (idempotent queued mail), not an abuse measure;
it should not compete for abuse-reduction budget. `expectedUserId` work is an audit, not a build.

---

## What I did not verify (confidence notes)

- **Cost estimates** are engineering judgment calibrated to repo module sizes (`admin_bans_v1` 698 LOC,
  `post_comment_v1` 608 LOC, `turnstile.rs` 210 LOC, `device_v1` ~200 LOC), not measured builds.
- **Production config values** (whether `TURNSTILE_SECRET_KEY`, `EMAIL_ALIASES_ENABLED`,
  `QURAN_BAN_ESCALATION_ENABLED` are set) were not readable from the tree; the defaults are as stated.
- **The test suite was not run**; all claims are source reads with line references.
- **Traffic base rates for this app** (share of token-clearing users, actual evasion rate) do not exist yet —
  which is precisely why C1/C2 are measured conditions, not assumptions.
