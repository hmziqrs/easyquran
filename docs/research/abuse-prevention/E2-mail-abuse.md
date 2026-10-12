# E2 — Mail-system abuse audit and fix design (round 6)

Auditor: dedicated mail-system auditor (subagent). Date: 2026-10-12. Repo: `/Users/hmziq/os/easyquran`
(working tree, read-only). Method: **read the actual code**, not the reports; every `file:line` below was
read. Cross-checked against `D5-final-gaps.md` (round 5) and the 35 prior reports; each finding is marked
**NEW** (no prior report states the mechanism) or **EXTENDS** (D5 stated the outline; this report adds the
verified detail, the arithmetic, and the fix).

Scope: `rust/backend/api/src/services/mail/**`, `src/modules/{forgot_password_v1,email_verification_v1,
newsletter_v1,mail_v1,auth_v1}/**`, `src/router.rs`, `src/main.rs`, `src/services/{abuse_limiter,rate_limit,
turnstile,turnstile}`, `src/db/sea_models/email_suppression/**`, `migration/m000001_init.rs`, `web/src/lib/
auth/**`, `.env.example`, `deploy/README.md`, and the Cloudflare Email Service docs
(REST API, limits, pricing) as of 2026-10.

---

## 1. The five claims, verified against the code

### 1.1 Forgot-password account-existence oracle — **CONFIRMED, and it is worse than one oracle**

`POST /forgot_password/v1/request` (public, `auth_guard::unauthenticated`, `forgot_password_v1/mod.rs:8-13`)
has **three** distinguishable outcomes for the same email address:

| Case                                   | Status           | Body                                                                               | Evidence                                                                                       |
| -------------------------------------- | ---------------- | ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Account does not exist                 | **404** `DB_002` | `"No account exists for that email"`                                               | `controller.rs:345-349`; `RecordNotFound` → 404 in `rust/crates/ruxlog-types/src/error.rs:245` |
| Account exists, no recent request      | **200**          | `"A password reset code has been sent to your email."`                             | `controller.rs:414-419`                                                                        |
| Account exists, code issued < 60 s ago | **429**          | `"A reset code was already sent. Please wait a minute before requesting another."` | `controller.rs:359-365` (`TooManyAttempts` → 429, `error.rs:232`)                              |

It is a deliberate product decision, not a bug: the doc comment at `controller.rs:314-318` says so, the
commit is `3b2b375f` (2026-08-26, "Replaces the SC-006 uniform envelope … with accurate feedback"), and
**three tests now pin the oracle** (`controller.rs:542-552` asserts `RecordNotFound` +
`"No account exists for that email"`; `:572-582` asserts the affirmative success envelope and forbids
`"if an account exists"`). The web client is wired to it (`web/src/lib/auth/flows.svelte.ts:562-564` special-
cases 404; `web/src/lib/auth/auth-copy.ts:5` `NO_ACCOUNT_EXISTS`; test `forgot-password-flow.test.ts:70-79`).
No committed doc records the decision (`documentation/history.md` does not mention it), so the fix finishes an
undocumented decision rather than reverting a recorded one.

Timing differs too, and the difference is large: the exists-path `await`s the provider HTTP call inside the
request (`controller.rs:403-411`), the unknown-path returns after one indexed lookup. There is also a
second-order oracle: on provider failure the exists-path returns 502/500/503/422
(`mail_error_to_response`, `error_map.rs:5-27`; `ExternalServiceError` → 502, `error.rs:263`) — an outcome
the unknown-path can never produce.

**Rate limiting is the only brake and it does not make the oracle expensive**: per IP `forgot_password:{ip}`
3/360 s → 1 h block, 5/900 s → 24 h block; per account `forgot_password_request:account:{email}` same config
(`controller.rs:21-28, 326-342`). Enumeration costs ~3 requests per IP; IP rotation is the standard tool.
Also note: the per-account limiter runs **before** the user lookup, so the 429 path itself is uniform
(same key for exist/not-exist) — only the 200/404/429-delay split leaks.

**Same question for the other paths:**

| Endpoint                                   | Oracle?                                                                                                                                                                                                                                                                                                                                                                                              | Evidence                                                                                                                            |
| ------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `POST /auth/v1/register` (duplicate email) | **No.** Unique violation is classified (`DuplicateEmail`) and answered with the byte-identical 201 `{"message":"Registration accepted"}`; no verification mail is sent for the existing address; tests pin status+body identity (`auth_v1/controller.rs:519-525, 540-568`; tests `:1395, 1435-1455`). Residual: a small timing delta (failed INSERT vs INSERT+spawn) — not a reliable remote oracle. |                                                                                                                                     |
| `POST /email_verification/v1/resend`       | **No existence oracle** (requires an authenticated session; recipient is the session user's own email, `email_verification_v1/mod.rs:9-12`; `controller.rs:65-71`). It _does_ leak mail-send outcomes (429/502/422) and can be used to burn the user's own bucket.                                                                                                                                   |                                                                                                                                     |
| `POST /newsletter/v1/subscribe`            | **Yes, once the table exists** — an arbitrary-recipient send with no account concept. Today it 500s before the send (table missing).                                                                                                                                                                                                                                                                 | `newsletter_v1/controller.rs:55-124`; migration has zero `newsletter` hits; dev DB `.tables` lacks `newsletter_subscribers` (C3 F8) |

### 1.2 No global/provider cap for transactional mail — **CONFIRMED**

`MailRouter::check_rate` (`services/mail/router.rs:137-159`):

```rust
self.enforce(&rcpt_key, rcpt_cfg).await?;
// Provider-wide circuit breaker is marketing-only; transactional sends are per-user limited at the controller.
if !transactional {
    self.enforce(&format!("mail:send:provider:{}", self.registry.default_provider()), self.limits.provider).await?;
}
```

Exact enforcement condition: **the provider bucket (`MAIL_PROVIDER_CFG`, 50/min → 5 min pause; 500/h → 1 h
pause, `router.rs:42-49`) is skipped whenever `is_transactional(msg.template)` is true** — i.e. for
`TEMPLATE_VERIFICATION` and `TEMPLATE_PASSWORD_RESET` (`provider.rs:13-22`). Transactional mail keeps only
the per-recipient bucket `mail:send:rcpt:txn:{recipient}` (30 attempts/h → 1 h block; 90 attempts/24 h →
24 h block, `router.rs:33-40`), which does nothing against **N distinct recipients**.

So the classification "transactional" bypasses: (a) the provider-wide per-minute bucket, (b) the
provider-wide hourly bucket, (c) the dedup path (only `TEMPLATE_NEWSLETTER` is deduped,
`router.rs:262-274`), and (d) any notion of a global daily ceiling — **none exists anywhere in the code**
(`MAIL_PROVIDER_CFG` is the only provider-level limiter and it is bypassed; `MAIL_RATE_LIMIT_ENABLED`
only toggles all of it, `main.rs:39`, `router.rs:277`).

### 1.3 `/mail/v1` has no rate-limit layer — **CONFIRMED**

`router.rs:128`: `.nest("/mail/v1", mail_v1::routes())` — no `.layer(rate_limit_layer(...))`, unlike every
other nest (`:75, :79, :86, :91-104, :110-142, :160-177`). The private router has **no global limiter**
either; `main.rs:827-930` layers origin/CSRF/session/route-blocker only.

What it serves (`mail_v1/mod.rs:13-33`):

- `GET/POST/DELETE /mail/v1/suppression` — admin-guarded (`verified_with_role::<ROLE_ADMIN>`); create/delete
  can add/remove suppression rows (no mail sent).
- `POST /mail/v1/webhook/{provider}` — **public, unauthenticated**. It does HMAC-SHA256 over the raw body
  (up to 64 KiB, `mod.rs:32` + `config::body_limits::DEFAULT`), parses JSON, then a DB upsert and a
  SHA-256 dedup key (`mail_v1/controller.rs:86-114`). Cost per request on a **single-connection SQLite**
  API (`sea_connect.rs:34-35`, `opt.max_connections(1)`): one HMAC + one SHA-256 + JSON parse; only the
  rare verified-and-fresh request touches the DB. The HMAC is over an attacker-supplied body with a secret
  the attacker does not have, so this is a CPU/flood surface, not an auth bypass — Low severity, but it is
  the only unauthenticated POST in the private router with no limiter at all.

### 1.4 Suppression fails open on a DB error — **CONFIRMED**

`lookup_suppressed` returns `Err(())` on DB failure and logs `"suppression lookup failed (fail-open)"`
(`router.rs:119-135`). The caller (`router.rs:243-250`) converts `Err(())` to `suppressed = false` and
proceeds to send, incrementing `suppression_check_failed`. Consequence: **during a DB blip the router mails
exactly the address class the provider punishes** (hard-bounced / complained / complaint-marked) — the one
send that must never happen. The `Err(())` path is also the only place the metric is incremented, and no
alert consumes it (telemetry counters only).

### 1.5 Newsletter subscribe can send to an arbitrary recipient — **CONFIRMED, dead today**

`subscribe` accepts any validated email (validator: `#[validate(email)]` only, no alias check —
`newsletter_v1/validator.rs:7-10`), inserts a pending subscriber, then sends a confirmation mail to that
address (`controller.rs:89-113`). Caps: per-email `newsletter:subscribe:{email}` 5/60 s (4 sends, then 1 h
block) and per-IP `newsletter:subscribe:ip:{ip}` 20/60 s (19 sends) (`controller.rs:63-87`), nest 100/min/IP
(`router.rs:139-142`), plus the **marketing** provider bucket (50/min, 500/h) because the template is
`TEMPLATE_NEWSLETTER`.

Today it cannot fire: no migration creates `newsletter_subscribers` (`grep -rn newsletter
migration/src/` → 0 hits; the dev DB `.tables` has no such table), so `SubscriberEntity::create` fails at
`controller.rs:95` and returns a 500 **before** the send. The moment C3 F8's fix lands (the table), the
endpoint becomes a live arbitrary-recipient mailer. Two activation bugs to fix with the table:

1. the send error is discarded (`let _ = send_mail(...)`, `controller.rs:113`), so a suppressed/bounced
   recipient still gets `201 "Please check your email"`;
2. a duplicate subscribe on a UNIQUE email column will surface as a DB error → 500 → a "already
   subscribed" oracle; the response must be the same 201 for new/duplicate/non-deliverable.

---

## 2. Every endpoint that can trigger outbound mail

Complete inventory: every `OutboundEmail` construction in the repo is in `services/mail/mod.rs` (2
helpers), `newsletter_v1/controller.rs` (1 helper), plus the two admin/registration call sites that use the
helpers. Grep proof: only `services/mail/mod.rs:34-68`, `newsletter_v1/controller.rs:34-51`, and the callers
below. The tera templates (`welcome`, `payment_receipt`, `subscription_confirmation`) are **dead code** —
`templates::render` has zero production callers — so **billing receipts, welcome mail, comment
notifications, and any contact form do not send mail at all.**

| #   | Route                                                             | Auth                                  | Recipient chosen by                                                                  | Volume cap today                                                                                 | Abuse it enables                                                                                                                                          |
| --- | ----------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `POST /forgot_password/v1/request`                                | public                                | attacker (any address that has an account; mail goes to that account's real mailbox) | per-IP 3/360 s→1 h, 5/900 s→24 h; per-account same; router per-recipient 29/h→1 h, ~87/24 h→24 h | existence oracle (§1.1); targeted reset-mail harassment; victim lockout (429) for 1 h with 3 requests, 24 h with ≈8 timed requests (D1 §1); no global cap |
| 2   | `POST /auth/v1/register`                                          | public + Turnstile (required in prod) | attacker (arbitrary submitted address)                                               | nest 100/min/IP; per-recipient 29/h; **no global/provider cap**                                  | N unique addresses → N verification emails; bounce farming at 100/min/IP → 144 k/day/IP; account-existence masked (good)                                  |
| 3   | `POST /email_verification/v1/resend`                              | authenticated, unverified             | own email only                                                                       | per-user 3/360 s→1 h, 5/900 s→24 h; router per-recipient 29/h                                    | self-bucket burning; no external recipient control; leaks send outcome (429/502)                                                                          |
| 4   | `POST /email_verification/v1/admin/issue_code`                    | ROLE_ADMIN                            | target user's email                                                                  | **no controller limiter**; router per-recipient 29/h                                             | admin-only; a compromised admin can mail any user's address (29/h each, no global cap)                                                                    |
| 5   | `POST /newsletter/v1/subscribe`                                   | public                                | attacker (arbitrary)                                                                 | per-email 5/60 s, per-IP 20/60 s, nest 100/min/IP, marketing provider 50/min+500/h               | **dead today** (no table); once the table lands: arbitrary-recipient confirmation mailer (≤4/min/address, ≤19/min/IP)                                     |
| 6   | `POST /newsletter/v1/send`                                        | ROLE_ADMIN                            | all `Confirmed` subscribers                                                          | marketing provider 50/min+500/h; 1.3 s pacing; per-recipient content dedup 300 s                 | admin-only; body/subject arbitrary; no `List-Unsubscribe` (CAN-SPAM/Gmail bulk)                                                                           |
| 7   | `POST /mail/v1/webhook/{provider}`                                | public (HMAC)                         | n/a — inbound only                                                                   | none (§1.3)                                                                                      | unmetered HMAC/JSON CPU on a pool-of-1 host                                                                                                               |
| 8   | billing receipts / welcome / comment notifications / contact form | —                                     | —                                                                                    | —                                                                                                | **no sender exists** (templates unused; contact page is a `mailto:` link)                                                                                 |

No open relay: `canonicalize_recipient` rejects CR/LF, comma, semicolon, `<`, `>` and display names
(`router.rs:319-351`); the CF request is JSON; the SMTP path parses a `Mailbox`. There is no user-supplied
header anywhere (§4).

---

## 3. Provider integration, webhook, suppression sync (claim 7)

**Send path.** `.env.example:10-14` selects `MAIL_PROVIDER=cloudflare`, `MAIL_FROM_ADDRESS=
no-reply@send.easyquran.fyi`, `MAIL_REPLY_TO=support@easyquran.fyi`, `MAIL_FROM_NAME=EasyQuran`. `main.rs:66-106`
builds `CloudflareMailProvider` from `CLOUDFLARE_ACCOUNT_ID`/`CLOUDFLARE_EMAIL_ACCOUNT_ID` +
`CLOUDFLARE_EMAIL_API_TOKEN` + optional `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` (**`unwrap_or_default()` → empty
allowed**, `main.rs:80-81`) + optional `CLOUDFLARE_EMAIL_ALLOWED_ADDRESSES` sandbox allowlist. Send is
`POST /accounts/{id}/email/sending/send` with a bearer token and JSON
(`cloudflare.rs:53-56, 108-123, 228-239`); response `result.permanent_bounces` feeds
`record_sync_bounces` (`cloudflare.rs:150-154`, `router.rs:302-305`). No SMTP fallback for Cloudflare; if
`MAIL_PROVIDER` is unset the catch-all builds SMTP (`main.rs:107-112`), which has **no webhook at all**
(default trait impl returns "provider has no inbound webhook", `provider.rs:59-65`).

**Webhook.** `POST /mail/v1/webhook/{provider}` → `verify_webhook` (`mail_v1/controller.rs:80-118`):
timestamp header required and fresh within ±5 min (`cloudflare.rs:165-177`; `MAX_SKEW_SECS = 5*60`,
`rux-webhook-crypto/src/lib.rs:6`), HMAC-SHA256 over `"{ts}.{body}"` compared hex-constant-time
(`cloudflare.rs:184-192`), event types `bounced|complained|delivered` (`:199-209`), recipient re-canonicalized
(`:212-214`), `permanent = env.permanent || complained` (`:217`). Then `apply_mail_event`
(`controller.rs:120-165`): bounce → `email_suppression` upsert with `permanent = ev.permanent`; complaint →
upsert `permanent: true`; delivered → debug only. Dedup: after apply, `mail:webhook:{provider}:{sha256(body)}`
claim, 24 h (`controller.rs:104-114`) — replay re-applies the idempotent upsert and then reports
`duplicate:true`; correct ordering (apply before claim).

**Three real gaps in this pipeline:**

1. **The webhook is undeployable by default.** `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` appears in **no** env
   example and in no `deploy/README.md` step (grep: `.env.example` mail block has no such key; deploy's mail
   section, `deploy/README.md:463-506`, never mentions the webhook). Boot accepts an empty secret
   (`main.rs:80-81`); `verify_webhook` then fails closed with `"no webhook secret configured"`
   (`cloudflare.rs:159-163`) → 401 (`error_map.rs:19-21`). So a deploy that follows the repo's own docs
   records **zero** bounces/complaints beyond the synchronous API result; the only suppression path left is
   `permanent_bounces` returned inline at send time. There is no boot warning, no metric, no test that the
   secret is set. (The boot gate in `settings.rs:548-600` only checks `MAIL_PROVIDER` ≠ none + from-address/
   from-name + provider credential keys; the webhook secret is not in `mail_provider_required_keys`.)
2. **No bounce-rate guard, no alerting.** Bounce/complaint handling is per-message only. Nothing aggregates
   "share of sends bouncing this hour"; the suppression table can grow unbounded (rows never expire —
   `m000001_init.rs:221-231`) and `record_sync_bounces` marks every sync bounce `permanent: true`
   (`router.rs:214`). The API's list is named `permanent_bounces`, so the classification is the provider's;
   the real problem is invisibility: an operator cannot see a 100 %-bounce burst until users complain.
3. **No sync to the provider's suppression list.** Cloudflare Email Service exposes a suppression list API
   (bulk import ≤1 000/request, ≤10 req/min; blocked sends do **not** count toward quota — docs
   "Limits"/"Suppression list limits", read 2026-10). The app keeps its own table and never pushes to it,
   so every bounce is paid for and counted against the daily quota before the provider's own list would
   have refused it.

**Suppression semantics** (`router.rs:119-135`): a recipient is suppressed if `permanent == true`, or if
`reason == Bounce` and `last_seen` is within `MAIL_SOFT_BOUNCE_COOLDOWN_SECS` (default 86 400 s,
`main.rs:42`). Complaint rows are forced permanent by the upsert (`actions.rs:36`, `email_suppression/actions.rs:41-48`
never downgrades). Correct, except for the fail-open path (§1.4).

---

## 4. Deliverability and header/template security (claim 8)

**What the code assumes.** SPF/DKIM/DMARC live in Cloudflare DNS, outside this repo. The code assumes:
`From: {MAIL_FROM_NAME} <{MAIL_FROM_ADDRESS}>` on the `send.` subdomain; `Reply-To: MAIL_REPLY_TO` on every
message (`cloudflare.rs:80-85, 107-115`); no `Message-ID`/`Date` control (the provider adds them);
`Return-Path` is the provider's. Misconfiguration consequences: (a) missing/misaligned DKIM for
`send.easyquran.fyi` + DMARC `p=reject` at the apex → **all** verification/reset mail rejected, including
for real users; (b) using the apex instead of the `send.` subdomain mixes the app's reputation with the
marketing/root domain — one bounce burst then junks both; (c) the repo's own smoke test
(`deploy/README.md:463-506`) is the only check, and it is manual and pre-`WEB_AUTH_ENABLED`. There is no
`Authentication-Results` monitoring and no postmaster-feedback ingestion.

**Header injection: no live vector found.**

- Recipient: CR/LF, `,`, `;`, `<`, `>` rejected (`router.rs:319-351`, tests `:433-443`).
- The CF path is JSON (`serde_json`), so CRLF in any field cannot become a new header.
- SMTP path: lettre 0.11.22 parses the `From`/`To` mailboxes and encodes `Subject`
  (`smtp.rs:86-104`); a CRLF in `MAIL_FROM_NAME` fails the `.parse()` and becomes a `Config` error.
- Templates: `templates/mod.rs` tera templates have **no production callers**; the live OTP template
  interpolates a server-generated 6-digit code (`html_templates.rs:1-57`); the newsletter body is inline
  HTML with `urlencoding::encode` on the email and token (`newsletter_v1/controller.rs:100-112`).
- **Residual:** the newsletter `subject` is admin-authored and only length-validated
  (`newsletter_v1/validator.rs:29-34`). Admin is trusted, but the subject is the only user-influenced
  header value in the system; strip CR/LF explicitly at the validator as defense-in-depth.

**Open relay: no.** Recipient control is per-endpoint and the router enforces a single canonical mailbox.
**Template-variable injection: no** (tera unused; the inline template encodes its interpolations).

---

## 5. Queue, retry, backoff, and provider-outage behavior (claim 9)

There is **no outbox, no queue, no retry, no backoff** anywhere in the mail path. Behavior per producer:

| Producer                | Failure behavior                                                                                                                                                       | Evidence                                                        |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| Register                | `tokio::spawn` fire-and-forget; failure only `error!`-logged; the API still returns 201                                                                                | `auth_v1/controller.rs:501-517`                                 |
| Forgot-password request | synchronous `await`; provider error → request fails 502/500/503/422                                                                                                    | `forgot_password_v1/controller.rs:403-411`; `error_map.rs:5-27` |
| Verification resend     | synchronous; provider error → request fails                                                                                                                            | `email_verification_v1/controller.rs:126-133`                   |
| Admin issue-code        | synchronous; provider error → request fails                                                                                                                            | `email_verification_v1/controller.rs:272-279`                   |
| Newsletter subscribe    | `let _ =` — send failure **silently ignored**, still 201                                                                                                               | `newsletter_v1/controller.rs:113`                               |
| Newsletter send         | background task, retries `Throttled` up to 2× with the provider's `retry_after`, 1.3 s pacing, counts the rest as failed; task dies on process restart; no persistence | `newsletter_v1/controller.rs:164-231`                           |

Provider outage consequences: login is unaffected; registration "succeeds" with no code (user must guess to
hit resend); reset returns a visible error; newsletter is lost silently. No mail metric is wired to an alert,
and the scheduler (`services/scheduler`) publishes posts only. The only mitigation is the manual smoke test
in `deploy/README.md`.

---

## 6. The arithmetic of the abuse (claim 2, quantified)

Provider facts (Cloudflare Email Service docs, read 2026-10-12): Workers Paid **$5/mo**; **3 000 emails/mo
included**, then **$0.35/1 000**; **hard bounces count toward the quota**; daily sending limits start
conservative and scale with reputation/account standing; `429 email.sending.error.throttled` and
`403 …sending_disabled` exist; the suppression-list API blocks sends that then don't count.

**Vector A — verification blast (registration), the mass-mail path.**

- Cap per IP: 100/min (`router.rs:75`, matched route) = 6 000/h = **144 000/day/IP**; per-recipient cap is
  irrelevant (each address gets one email); **no global/provider cap** for transactional (§1.2).
- Cost at 144 k: `$5 + (144 000 − 3 000)/1 000 × $0.35 = **$49.35**` for one IP-day — _if_ the provider
  allows it. It will not: the daily limit for a new account is conservative and reputation-scaled, so the
  realistic failure mode is `429`/`403 sending_disabled`, not a huge bill. That is precisely the self-DoS:
  **the app has no cap, so Cloudflare's abuse system becomes the cap, and the account gets flagged.**
- Bounce rate for fabricated addresses: ~100 %. Gmail/Yahoo bulk-sender guidance: keep spam complaint rate
  <0.3 % (ideally <0.1 %) and hard bounces <2 %. A 10 000-address burst against a domain that normally sends
  <3 000/month is a >100× spike at ~100 % bounce — domain reputation collapses, and verification/reset mail
  starts landing in spam. Each bounce also writes a permanent `email_suppression` row (`router.rs:206-221`),
  and the quota is burned by bounces.

**Vector B — reset-mail harassment/lockout (targeted, not mass).**

- Controller: request 1 sends; request 2 within 60 s → 429 (delay); request 3 within 360 s → 1 h block;
  ≈8 timed requests → 24 h block (≥4 blocked retries in the block's last 15 min, then 1 after expiry — D1 §1).
  So **3 requests = 1 h of no recovery for a victim; ≈8 = 24 h**.
- Router: 29 sends/h then 1 h block; paced 29/h × 3 h → the 24 h Long block.
- Mass cost: each attacker IP yields ~1 send/day before its own block, so per victim/day ≈ 87 emails
  (87 IPs). Mail cost ≈ 87 × $0.00035 = **$0.03/victim/day**; the attacker's real cost is proxy IPs. This
  vector is harassment + lockout, not a bill.

**Vector C — newsletter confirmation (post-table).** ≤4/min/address, ≤19/min/IP, marketing provider bucket
50/min → 499/h ⇒ ≤ ~11 976/day globally (the provider bucket is the only global brake).

**Comparison.** Vector A is the one with unbounded app-side volume; B is cheap and targeted; C is capped by
the marketing bucket.

---

## 7. Fix design (claim 10) — exact enforcement points and key scheme

### F-1 Global transactional cap + per-domain cap (enforcement: `MailRouter::check_rate`, `router.rs:137-159`)

Add to `MailRouterLimits` (`router.rs:51-58`) and constants beside `MAIL_PROVIDER_CFG`:

```rust
pub const MAIL_GLOBAL_RESET_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 500, temp_block_range: 3600, temp_block_duration: 900,
    block_retry_limit: 2000, block_range: 86_400, block_duration: 86_400,
};
pub const MAIL_GLOBAL_VERIFY_CFG: AbuseLimiterConfig = MAIL_GLOBAL_RESET_CFG; // separate key, same shape
pub const MAIL_DOMAIN_CFG: AbuseLimiterConfig = AbuseLimiterConfig {
    temp_block_attempts: 100, temp_block_range: 3600, temp_block_duration: 3600,
    block_retry_limit: 300, block_range: 86_400, block_duration: 86_400,
};
```

Change `check_rate(&self, recipient: &str, transactional: bool)` to take the template
(`check_rate(&self, recipient: &str, template: Option<&'static str>)`) — `send()` already has `msg.template`
— and enforce, in order:

1. per-recipient bucket (existing, unchanged);
2. **class bucket** for transactional: `mail:send:global:reset` (password reset) or
   `mail:send:global:verify` (verification) — **separate keys on purpose** so a verification flood cannot
   starve account recovery (this is the self-attack answer, §8);
3. marketing keeps the existing `mail:send:provider:{name}` bucket;
4. **domain bucket** for every class: `mail:send:domain:{recipient-domain}` (kills single-domain bounce
   farming: an attacker registering `x1@a.com, x2@a.com, …` hits 100/h, not 144 000/day).

Values: 500/h → 15 min pause per class (a small app sends far less; make them env-overridable:
`MAIL_GLOBAL_TXN_PER_HOUR`, `MAIL_DOMAIN_PER_HOUR`). Reset is deliberately **not** capped tighter than
verification, because recovery is the availability-critical path.

### F-2 Oracle fix (enforcement: `forgot_password_v1::generate`, `controller.rs:320-420`)

1. Unknown email (`:345-349`): run the same work as the exists path — `forgot_password::Entity::generate_code()` +
   `hash_code()` (discard) — then fall through to the **same** 200 response. Do not return 404.
2. In-delay (`:359-365`): return the **same** 200 without sending (the previous code is still valid; the
   user's 6-digit code has a 3-hour expiry, `html_templates.rs:42`).
3. Success: `tokio::spawn` the provider send (mirroring register, `auth_v1/controller.rs:506-517`), log
   `mail_error_kind` on failure, increment a new `mail_send_failures` metric, return 200 immediately. This
   removes both the message/status oracle and the send-latency timing delta (the unknown path now does the
   same DB lookup + hash).
4. Uniform body: `{"message":"If an account exists for that email, a reset code has been sent."}`, 200.
   The rate limiter still returns 429 with the same key for exist/not-exist (already uniform).
5. Machine guards: replace `unknown_email_answers_explicit_record_not_found` and
   `success_envelope_confirms_the_send` (`controller.rs:542-582`) with tests asserting: (a) the uniform body
   appears for both branches, (b) no `RecordNotFound` in the handler, (c) the send is inside `tokio::spawn`.
6. Web: delete the 404 special-case (`flows.svelte.ts:562-564`), neutralize `NO_ACCOUNT_EXISTS`
   (`auth-copy.ts:5`), rewrite `forgot-password-flow.test.ts:70-79` to assert the request step advances to
   the code step on 200 regardless of account existence.
7. Record the decision in `documentation/history.md`.

### F-3 Suppression fail-closed (enforcement: `MailRouter::send`, `router.rs:243-250`)

```rust
let suppressed = match self.lookup_suppressed(&msg.to).await {
    Ok(v) => v,
    Err(()) => {
        router_metrics.suppression_check_failed.add(1, &[]);
        // Fail closed: never mail an address we cannot prove is unsuppressed.
        if transactional { return Ok(SendReceipt { suppressed: true, ..Default::default() }); } // same envelope to the caller; the flag lets the notice path record `suppressed` (G3 P0.1)
        return Err(MailError::Suppressed);
    }
};
```

Plus an alert on `suppression_check_failed > 0` — silent drops of reset mail are otherwise invisible.

### F-4 Webhook processing hardening (enforcement: `router.rs:128`, `mail_v1/controller.rs`, `cloudflare.rs`)

1. Document and require `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` in `.env.example` (mail block) and
   `deploy/README.md`'s mail smoke test; add it to `mail_provider_required_keys("cloudflare")`
   (`settings.rs:714-730`) so production boot fails closed without it, and add a boot `warn!` when the
   provider is `cloudflare` and the secret is empty.
2. Rate-limit the mail nest with a **fixed-label** branch bucket so Cloudflare's webhook source IPs share
   one bucket and cannot fan out:
   `.nest("/mail/v1", mail_v1::routes().layer(rate_limit::rate_limit_layer_branch(&state, 600, 60, "mail-webhook")))`
   (`rate_limit.rs:30-40`), plus a fail-closed `abuse_limiter` brake on
   `mail:webhook-flood:{provider}` (e.g. 3 000/min) inside the handler.
3. Bounce-rate guard (new): increment `mail:stats:sent:{hour}` on every successful send and
   `mail:stats:bounce:{hour}` in `apply_mail_event` (bounce branch) and in `record_sync_bounces`. Before any
   non-reset send, if `sent >= 20` and `bounce/sent > 0.05`, set `mail:guard:paused` (claim, 900 s) and drop
   marketing/verification (transactional → `Ok(default)`, marketing → `Err(Throttled)`), emit a metric +
   `error!`. Add a read-only `RateLimitStore::peek_count(key)` so the guard can read counters without
   incrementing.
4. Sync permanent suppressions to Cloudflare's suppression list API (bulk import, ≤1 000/request, ≤10/min)
   so blocked sends stop counting toward quota; best-effort, idempotent, admin-visible.
5. Keep sync bounces as the provider classifies them (`permanent_bounces`) but add an admin-visible count and
   a test; do not re-derive permanence in the app.

### F-5 Newsletter: one-click unsubscribe + `List-Unsubscribe` (C4 D10) + table + activation guards

1. **Table (C3 F8) in the same migration as D10**: `newsletter_subscribers` with `email UNIQUE`,
   `status`, `token_hash` (HMAC-SHA256 with the server secret — replace the plaintext `token`,
   `db/sea_models/newsletter_subscriber/model.rs:13`), `created_at`, `updated_at`, index on `status`.
2. **Headers**: add `headers: Vec<(String, String)>` to `OutboundEmail` (`provider.rs:4-11`); add
   `headers: Option<HashMap<String, String>>` to `CfSendRequest` (`cloudflare.rs:228-239`) — the CF REST API
   supports `headers` and its docs show `List-Unsubscribe`/`List-Unsubscribe-Post` verbatim; for SMTP use
   `Message::builder().raw_header(HeaderValue::new(HeaderName::new_from_ascii_str("List-Unsubscribe"), v))`
   (lettre 0.11.22 has `raw_header`, `message/mod.rs:373`; no built-in `ListUnsubscribe` type).
3. **Every newsletter send** (`newsletter_v1/controller.rs:154-239`) sets
   `List-Unsubscribe: <{site_url}/newsletter/unsubscribe-one-click?token={token}>` and
   `List-Unsubscribe-Post: List-Unsubscribe=One-Click`; the confirmation mail gets the same plus a footer.
4. **New route** `POST /newsletter/v1/unsubscribe/one-click` (public, inside the existing nest): token from
   the query string (RFC 8058 posts to the header URL), body ignored, **always 200** (no token oracle),
   idempotent, sets `status = Unsubscribed`.
5. **Activation guards** (from §1.5): keep the send after the insert; propagate send errors to the log +
   metric (not `let _ =`); return the same 201 for new/duplicate/unconfirmed; add the global/domain caps and
   the per-recipient cooldown; normalize the rate-limit key by stripping the plus tag (key-only — storage
   keeps the full address) because the subscribe validator does not check `EMAIL_ALIASES_ENABLED`
   (`newsletter_v1/validator.rs:7-10`).
6. 30-day expiry for unconfirmed rows (depends on C4 D14's retention module).

### F-6 Per-recipient cooldown (enforcement: `MailRouter::send`, before `check_rate`)

Use a **claim**, not the abuse limiter, so blocked attempts cannot accumulate into a 24 h lockout:

```rust
let cooldown_key = format!("mail:send:cooldown:{}:{recipient}", msg.template.unwrap_or("default"));
if !rux_request_gate::dedup_nx(&self.gate_store, &cooldown_key, self.limits.cooldown_secs).await {
    return if transactional { Ok(SendReceipt::default()) } else { Err(MailError::Throttled { retry_after_secs: 60 }) };
}
```

`cooldown_secs = 60`, keyed per template **and** recipient, so register→reset and reset→verification never
collide (the legit user case). The existing controller delay (`forgot_password` 1 min, `email_verification`
1 min) stays as the first layer.

### F-7 Registration admission (enforcement: `auth_v1::register`, before `user::Entity::create`, `controller.rs:482`)

Add `register:ip:{ip}:day` and `register:domain:{domain}:day` abuse buckets (the D19/B2 A2 plan; the app
currently has only the 100/min nest). Suggested defaults: 50/day/IP, 100/day/domain, env-overridable, and
loosened for known school/NAT ranges only by the per-IP value. This is the real ceiling for vector A.

### Rate-limit key scheme (new keys; collision-checked against existing)

| New key                                                | Map    | Where                     | Window/behavior      |
| ------------------------------------------------------ | ------ | ------------------------- | -------------------- |
| `mail:send:global:reset`                               | limits | `router.rs` check_rate    | 500/h → 15 min pause |
| `mail:send:global:verify`                              | limits | `router.rs` check_rate    | 500/h → 15 min pause |
| `mail:send:domain:{domain}`                            | limits | `router.rs` check_rate    | 100/h → 1 h pause    |
| `mail:send:cooldown:{template}:{recipient}`            | claims | `router.rs` send          | 60 s single-shot     |
| `mail:stats:sent:{hour}` / `mail:stats:bounce:{hour}`  | limits | router + webhook          | 3 600 s counters     |
| `mail:guard:paused`                                    | claims | router                    | 900 s guard trip     |
| `mail:webhook-flood:{provider}`                        | limits | `mail_v1` handler         | 3 000/min brake      |
| `ratelimit:{identity}:mail-webhook`                    | limits | `rate_limit_layer_branch` | 600/min, fixed label |
| `register:ip:{ip}:day`, `register:domain:{domain}:day` | limits | `auth_v1` register        | 24 h admission       |

Existing keys (not reused, not renamed): `ratelimit:{unit}:{path}` (all nest layers), `login:{email}`,
`totp:{user_id}`, `totp_setup:{user_id}`, `reauth:{user_id}`, `forgot_password:{ip}`,
`forgot_password_request:account:{email}`, `forgot_password_verify:{ip}`,
`forgot_password_verify:account:{email}`, `email_verify:{user_id}`, `email_verification:{user_id}`,
`newsletter:subscribe:{email}`, `newsletter:subscribe:ip:{ip}`, `mail:send:rcpt:{recipient}`,
`mail:send:rcpt:txn:{recipient}`, `mail:send:provider:{name}`, `mail:dedup:{template}:{digest}`,
`mail:webhook:{provider}:{digest}` (the flood brake is named `mail:webhook-flood:` to stay visually distinct
from the dedup claim even though limits and claims are separate maps).

### F-8 Mail outbox (minimum viable, optional but recommended)

One table `mail_outbox(id, template, recipient, payload_json, attempts, next_attempt_at, status)` + a
scheduler tick (there is already a scheduler) with backoff 1 m/5 m/30 m/2 h/6 h, drop after 24 h, and the
per-send rate-limit check on each attempt. Apply to `verification` + `password_reset` only; marketing keeps
the current in-task loop. Without it, register still loses the code silently on an outage.

---

## 8. Attacking the fixes (claim 11)

1. **Global cap as a DoS lever.** An attacker who can _send_ 500 verification emails/h holds
   `mail:send:global:verify` at its cap, pausing all verification mail for 15 min at a time. Answer: reset
   has its own bucket (`mail:send:global:reset`) with a 15-min pause, so account recovery never depends on
   the verification bucket; the attacker also has to pass the registration admission caps (F-7) to _send_
   that much, and the guard alerts. Residual: a determined attacker can still intermittently pause
   verification mail; that is strictly better than an unbounded blast, and verification is resumable.
2. **"A user requesting 3 resets in 5 minutes."** Today: request 2 within 60 s gets a distinct 429; request
   3 gets the limiter's 429. After the fix: request 2/3 get the uniform 200 with no new mail (delay +
   cooldown), and the code from request 1 remains valid for 3 h. The user experience is "the page says a code
   was sent, I use the code I already have". No support escalation path needed beyond the existing
   `resend`/re-request delay.
3. **"A school with many students on one IP."** This is **already broken today**: `forgot_password:{ip}`
   3/360 s → 1 h block means the 3rd student's reset from the NAT fails for an hour. The global cap does not
   make it worse; the fix should _raise_ the per-IP reset cap (e.g. 20/360 s) now that the per-account,
   per-recipient, per-domain and global buckets exist. Verification resend is per-user, so it is unaffected.
4. **"A user with a plus-alias."** Production rejects plus-tagged account emails
   (`EMAIL_ALIASES_ENABLED` default off, `settings.rs:45-50`; test `:1190`), so reset/verification recipients
   cannot be aliases of each other. The newsletter validator does **not** check aliases, so an attacker can
   rotate `victim+1@…`, `victim+2@…` past the per-email bucket; the per-domain cap + key normalization
   (F-5.5) close that.
5. **Residual abuse after all fixes.** With Turnstile solved at scale and IP rotation, an attacker can still
   push up to the global cap per hour indefinitely (each hour refills): ≤500 verification + ≤500 reset
   emails/h, each also limited to 100/h/domain. If every one bounces, the guard trips after the first ~20–500
   and pauses non-reset mail for 15 min; reset keeps flowing until its own bucket fills. Worst case cost
   ≈ `$0.18/h` of mail; the real damage is reputation, which the guard bounds but cannot zero. Honest
   statement: **no design caps reputation damage to zero without a provider-side pre-send verification**
   (double opt-in / address verification before the first send), which is out of scope for a
   reset/verification flow.

---

## 9. The honest risk if nothing is fixed (claim 12)

Sender reputation collapse is the real risk, and it is asymmetric: the app's legitimate mail is
**account-recovery-critical** — a user who cannot receive a reset code is locked out permanently, because
there is no alternate recovery path (no enforcement/notice channel, no support tooling, D5 §2).

Quantified: a single 10 000-address registration burst from one IP (100/min → under 2 hours) produces
10 000 hard bounces at ~100 % rate, burns 10 000 quota units (bounces count), and writes 10 000 permanent
suppression rows. Against a domain that sends <3 000/month, that is a >100× reputation event at a bounce
rate ~50× the 2 % industry tolerance and a complaint-rate risk far above Gmail's 0.3 % ceiling. The likely
sequence: Gmail/Yahoo/Outlook junk the `send.easyquran.fyi` stream → verification and reset mails land in
spam → users cannot recover accounts → the operator finds out from complaints, because **no metric or alert
watches bounce rate** (the counters exist only as OTel counters; `suppression_check_failed` has no consumer).
Cloudflare's own daily limits and `sending_disabled` are the only backstop, and hitting them means the app
cannot send _anything_, including resets. Recovery from domain-reputation damage is weeks to months of clean
sending, and may require a new sending subdomain. Cost is secondary ($49/IP-day worst case, bounded by the
provider); the irreversible part is the recovery path for existing users and the domain's standing.

---

## 10. Findings

Severity: Critical > High > Med > Low. Every finding is **NEW** unless marked EXTENDS.

**M1 — Forgot-password account-existence oracle (explicit 404 + message + timing + send-outcome).**
**NEW** · **Severity: High** (privacy; enables targeted phishing; documented anti-enumeration regression).
Evidence: `modules/forgot_password_v1/controller.rs:314-318` (decision), `:343-349` (404), `:359-365`
(in-delay 429), `:403-411` (sync send → timing/502), `:542-552, 572-582` (tests pin it);
`rust/crates/ruxlog-types/src/error.rs:232,245`; web `flows.svelte.ts:562-564`, `auth-copy.ts:5`,
`forgot-password-flow.test.ts:70-79`; commit `3b2b375f`.
Fix: §7 F-2 — uniform 200 + background send + dummy hash work; rewrite the three source-pinning tests; update
the web flow; record the decision in `documentation/history.md`.

**M2 — No global/provider cap for transactional mail; the "transactional" classification bypasses the only
provider limiter.**
**NEW** · **Severity: High**. Evidence: `services/mail/router.rs:137-159` (the `if !transactional` gate),
`:42-49` (`MAIL_PROVIDER_CFG`), `provider.rs:17-22` (`is_transactional` = verification+reset), `main.rs:39`
(`MAIL_RATE_LIMIT_ENABLED` toggles all of it).
Fix: §7 F-1 — global reset/verify buckets + per-domain bucket; class-separated keys.

**M3 — Bounce-farm reputation DoS: arbitrary-address registration → 100 %-bounce bursts; no bounce-rate
guard; permanent suppression rows grow unbounded.**
**NEW** · **Severity: High**. Evidence: `auth_v1/controller.rs:459-517` (arbitrary recipient, 1 mail per new
address, 100/min/IP nest at `router.rs:75`), `mail/router.rs:206-221` (permanent rows), `mail_v1/controller.rs:129-142`
(bounce upsert), `migration/m000001_init.rs:221-231` (rows never expire), no bounce-rate metric anywhere.
Fix: §7 F-1 + F-4.3 + F-7; provider suppression sync (F-4.4).

**M4 — Webhook is undeployable by default: the secret is absent from `.env.example`/deploy docs and boot
accepts it empty, so bounce/complaint processing 401s forever.**
**NEW** · **Severity: High** (silent loss of the entire suppression pipeline; only inline sync bounces
remain). Evidence: `.env.example:9-14` (no `CLOUDFLARE_EMAIL_WEBHOOK_SECRET`), `main.rs:80-81`
(`unwrap_or_default()`), `cloudflare.rs:159-163` (fail closed), `error_map.rs:19-21` (401),
`settings.rs:548-600, 714-730` (prod gate omits it), `deploy/README.md:463-506` (no webhook step).
Fix: §7 F-4.1.

**M5 — `/mail/v1` has no rate-limit layer (unmetered unauthenticated HMAC/JSON webhook on a pool-of-1 host).**
**NEW** · **Severity: Low-Med**. Evidence: `router.rs:128` vs every other nest; `mail_v1/controller.rs:80-114`;
`sea_connect.rs:34-35` (`max_connections(1)`).
Fix: §7 F-4.2.

**M6 — Suppression fails open on a DB error: a DB blip mails the exact bounced/complained addresses the
provider punishes.**
**NEW** · **Severity: Med**. Evidence: `router.rs:119-135` (Err(())), `:243-250` (Err → not suppressed),
`:247` (metric with no alert consumer).
Fix: §7 F-3 — fail closed; `Ok(default)` for transactional (anti-enumeration), `Err(Suppressed)` for
marketing; alert.

**M7 — Newsletter subscribe is an arbitrary-recipient mailer, dead today but armed by its own table fix;
send errors are silently swallowed; duplicate subscribe will be a 500 oracle.**
**EXTENDS (C3 F8 / D5 1e)** · **Severity: Med (on activation)**. Evidence:
`newsletter_v1/controller.rs:55-124` (caps `:63-87`, send `:113` `let _ =`), missing migration
(C3 F8), dev DB table list, `newsletter_v1/validator.rs:7-10` (no alias check).
Fix: §7 F-5.1/5.5 + F-1 + F-6.

**M8 — No `List-Unsubscribe`/`List-Unsubscribe-Post`; unsubscribe requires the token; token stored
plaintext.**
**NEW** (D5 listed the header gap in its mail row; this is the verified mechanism + the provider capability)
· **Severity: Med** (CAN-SPAM/GDPR/ePrivacy; Gmail bulk requirements). Evidence: grep `List-Unsubscribe` in
`services/mail/` → 0 hits; `cloudflare.rs:228-239` (`CfSendRequest` has no `headers` field) though the CF
REST API supports it; `newsletter_v1/controller.rs:128-150` (token-bound unsubscribe);
`db/sea_models/newsletter_subscriber/model.rs:13` (plaintext token); C4 D10.
Fix: §7 F-5.2-5.4.

**M9 — The per-recipient transactional bucket is a lockout lever: blocked attempts accumulate, so 3 requests
block a victim's reset for 1 h and ≈8 timed requests for 24 h; a paced sender reaches a 24 h recipient block
after ~87 sends.**
**NEW** · **Severity: Med**. Evidence: `rux-request-gate/src/store.rs:303-320` (blocked path appends
attempts), `:326-349` (temp branch wins; Long only when short < 30), `router.rs:33-40`;
`forgot_password_v1/controller.rs:21-28` (3/360 s → 1 h; 5/900 s → 24 h).
Fix: §7 F-6 (claim-based cooldown for recipient keys) + F-1; document the residual lockout; alert.

**M10 — No queue/retry/backoff; outage behavior is inconsistent (register loses the code silently, reset
502s, newsletter send dies with the process).**
**NEW** · **Severity: Med**. Evidence: `auth_v1/controller.rs:501-517` (spawn, log-only),
`forgot_password_v1/controller.rs:403-411`, `email_verification_v1/controller.rs:126-133`,
`newsletter_v1/controller.rs:113` (`let _ =`), `:164-231` (in-process loop).
Fix: §7 F-8 (minimal outbox for verification+reset), or at minimum send-failure metrics + alerts.

**M11 — Header injection / template-variable injection: no live vector; residual is the admin-authored
newsletter subject (length-only validation) as the sole user-influenced header value.**
**NEW (negative finding + one hardening)** · **Severity: Low/Info**. Evidence:
`router.rs:319-351, 433-443` (recipient canonicalization + tests); `cloudflare.rs:228-239` (JSON);
`smtp.rs:86-104` (lettre encodes Subject/parses mailboxes); `templates/mod.rs` (tera unused);
`newsletter_v1/validator.rs:29-34` (subject length only).
Fix: strip CR/LF from the newsletter subject in the validator; keep the canonicalization tests.

**M12 — `MAIL_RATE_LIMIT_ENABLED=false` disables every mail cap with no production guard; and the mail
send-outcome is exposed to clients (429/422/502) on the reset path.**
**NEW** · **Severity: Low-Med**. Evidence: `main.rs:39` (env default true), `router.rs:277-282` (all caps
gated), `error_map.rs:5-27`, `forgot_password_v1/controller.rs:403-411`.
Fix: gate the kill switch on non-production (or log a loud warning + metric), and map all reset send
outcomes to the uniform 200 (F-2.3).

---

## Appendix — verification commands (all read-only)

```bash
# claims 1-5
sed -n '314,420p' rust/backend/api/src/modules/forgot_password_v1/controller.rs
sed -n '137,159p' rust/backend/api/src/services/mail/router.rs      # the !transactional gate
sed -n '42,49p'   rust/backend/api/src/services/mail/router.rs      # MAIL_PROVIDER_CFG
sed -n '119,135p;243,250p' rust/backend/api/src/services/mail/router.rs   # fail-open suppression
grep -n 'mail_v1' rust/backend/api/src/router.rs                    # line 128, no layer
grep -rn newsletter rust/backend/api/migration/src/                 # 0 hits (C3 F8)
sqlite3 'file:rust/data/easyquran.db?immutable=1' ".tables"         # no newsletter_subscribers

# claims 6-9
grep -rn "OutboundEmail" rust/backend/api/src --type rust | grep -v services/mail/
sed -n '80,165p' rust/backend/api/src/modules/mail_v1/controller.rs
sed -n '157,225p' rust/backend/api/src/services/mail/cloudflare.rs
sed -n '228,239p' rust/backend/api/src/services/mail/cloudflare.rs  # CfSendRequest, no headers
grep -n 'CLOUDFLARE_EMAIL_WEBHOOK_SECRET' .env.example deploy/README.md   # 0 hits
grep -rn 'List-Unsubscribe' rust/backend/api/src web/src            # 0 hits

# limiter semantics / keys
sed -n '287,350p' rust/backend/api/crates/rux-request-gate/src/store.rs
grep -rn 'abuse_limiter::limiter(' rust/backend/api/src --type rust | grep -v services/
grep -rn 'format!("login:' rust/backend/api/src/modules/auth_v1/controller.rs
```
