# D1 — Independent verification of round-5 claims

Auditor: D1, verification pass. Date: 2026-10-12.
Method: every claim re-derived from source at HEAD (`/Users/hmziq/os/easyquran`), not from the
C1/C3/C4 reports. Paths below are repo-relative. Where the reports are wrong, the correction is
stated. DNS checked with the system resolver and `@1.1.1.1`. Dev DB read read-only
(`immutable=1`, no writes).

**Score: 12 TRUE (several with wording corrections), 2 PARTLY TRUE (claim 1's second half FALSE).** The two load-bearing overstatements are
claim 1's "5 POSTs → 24 h" and claim 12's "no user-facing revoke UI"; both come from compressing
the source reports past what the code supports.

---

## 1. Login abuse limiter — PARTLY TRUE (second half FALSE)

**Counts every attempt including successes:** TRUE.
`rust/backend/api/crates/rux-request-gate/src/store.rs:322` — `b.attempts.push_back(now_e)` runs
unconditionally on the allowed path. `:305` — the blocked path also appends before returning.
`rust/backend/api/src/modules/auth_v1/controller.rs:135-140` — the limiter is called before
`authenticate_password` (`:142-145`), so a success is indistinguishable from a failure to the
counter. Config `:31-38`: `temp_block_attempts: 3, temp_block_range: 360, temp_block_duration: 3600,
block_retry_limit: 5, block_range: 900, block_duration: 86400`.

**Never clears:** TRUE. `clear_limit` callers: only
`rust/backend/api/src/modules/admin_bans_v1/controller.rs:152-153,448-449,545-546` (IP-ban keys).
No `del`/`clear_limit` on `login:*` keys; no clear-on-success exists.

**3 successful logins in 6 minutes → 1 h lock:** TRUE in substance, wording wrong.
The 3rd attempt is itself blocked — it returns 429 before the password is checked. So it is
"3 _attempts_ in 6 min → 1 h", with only 2 successes; the third never succeeds.
(`abuse.rs:64-125` maps `Blocked` → `TooManyAttempts`; `services/abuse_limiter.rs:60-68`.)

**5 anonymous POSTs → 24 h lock:** **FALSE as stated.** Five one-shot requests produce only the
1 h temp block. While a temp block is live, every call returns early at `store.rs:303-315` and the
long threshold at `:335-343` is never evaluated. The 24 h tier is reachable only when, at the
moment the temp block expires, ≥5 attempts survive in the trailing 900 s window _and_ the trailing
360 s count is <3 (else Temp fires first). Practical minimum: 3 requests → 1 h block; then ≥4
blocked retries inside the final 900 s of the block (they are appended at `:305`); then 1 request
after expiry → `long_count ≥ 5` → 24 h. **≈8 timed requests, not 5** — or the victim's own ≥4
retries in the final 15 min plus one request after expiry.
Note: a 3 + 3 + 1 sequence is one short — it yields `long_count = 4`, below the threshold of 5. C1 F2
says the same ("not 5 requests"); the claim as given to this audit is a wrong compression of C1.

**Reachable?** Yes — the 24 h weapon is real via the blocked-path append; only the "5 requests"
framing is wrong.

## 2. Gmail dot-alias bypass — TRUE (wording correction)

- `rust/backend/api/src/utils/email.rs:7-9` — `normalize_email` = trim + lowercase only. No dot
  handling anywhere.
- `rust/backend/api/src/utils/email_alias.rs:8-18` — rejects only `+` in the local part, only when
  the flag is off.
- `rust/backend/api/src/config/settings.rs:48-53` — `email_aliases_enabled()` defaults to
  `!production`, i.e. **false in production**.
- `rust/backend/api/src/modules/auth_v1/validator.rs:44-47` — `into_new_user` stores
  `normalize_email`; `:6-14` applies the alias check to the register payload.
- `rust/backend/api/src/db/sea_models/user/actions.rs:280-289` — `find_by_email` is an exact
  `Column::Email.eq(...)` filter; callers normalize (`services/auth.rs:103`).

So `a.b@gmail.com` and `ab@gmail.com` are distinct rows that deliver to one inbox, and no
per-inbox/per-identity cap exists (registration has no `abuse_limiter` call;
`auth_v1/controller.rs:459-535`). **Correction to wording:** the production default does not
"normalize `+` off" — it _rejects_ plus-tagged addresses. Dots are never canonicalized in any
environment; that is the bypass. TRUE on substance.

## 3. `newsletter_subscribers` missing — TRUE (failure point correction)

- No migration: `rust/backend/api/migration/src/lib.rs:3-13` lists m000001–m000011; no newsletter
  table in any file (`grep newsletter migration/src/` → 0).
- No runtime DDL: only four `CREATE TABLE IF NOT EXISTS` in `src/` —
  `services/oauth/login.rs:400`, `services/translation_popularity_store.rs:192`,
  `services/rate_limit_store.rs:22`, `services/session_store.rs:15`.
- Model: `db/sea_models/newsletter_subscriber/model.rs:7` declares the table.
- Router: `src/router.rs:140-141` mounts `/newsletter/v1`.
- Dev DB (`rust/data/easyquran.db`, read-only): no `newsletter_subscribers` in `sqlite_master`.
- 500: `db/sea_models/newsletter_subscriber/actions.rs:18-21` fails on the initial SELECT against
  the missing table → `DbErr::Exec` → `classify_db_error` → `QueryError` →
  `error/database.rs:45,58-61` → status 500 (`rust/crates/ruxlog-types/src/error.rs:247`).
  **Correction:** the failure is at the duplicate-check SELECT, not the INSERT (`:49`), though the
  user-visible result is the same 500.

## 4. Undeliverable contact address — TRUE (scope precision)

Addresses found: `salam@easyquran.app` (Terms; `web/messages/en.json:213`, `ar.json:213`),
`corrections@easyquran.app` (FAQ; `en.json:152`, `ar.json:152`). `web/src/lib/config/site.ts:3,10`
— domain `easyquran.fyi`, contact email `hmziqrs@gmail.com`.

DNS (system resolver and 1.1.1.1, both agree):

- `easyquran.app` MX → **empty**. A → 185.199.108-111.153 (GitHub Pages).
- `easyquran.fyi` MX → `route1/2/3.mx.cloudflare.net`.

So the `.app` address has no MX. Delivery then falls back to the A record (RFC 5321 implicit MX) —
GitHub Pages runs no SMTP listener, so mail to that domain bounces; "undeliverable" is practically
correct. **Correction:** the privacy policy contains **no** contact address at all (no `@` in any
`legal_privacy_*` body, `web/messages/en.json:173-192`); the undeliverable addresses are the Terms
and FAQ ones. That is two separate gaps, not one.

## 5. Analytics/crash reporting before consent — TRUE

- `web/src/lib/stores/consent.svelte.ts:16` — `DEFAULT_FLAGS = { analytics: true, performance:
true, advertising: false }`; `decodeConsent` (`:19-27`) returns true for any flag never set.
- `web/src/lib/boot/analytics.ts:20-31` — unconditional `initAnalytics()` → `applyConsent()` →
  `pageView(location.pathname)`; consent is applied **after** init (`:25`, `:12-16`).
- `web/src/lib/firebase/analytics.ts:20` — `getAnalytics(core)` is the call that injects the gtag
  script (Firebase SDK behaviour); nothing gates it on consent.
- `web/src/lib/boot/crash-reporting.ts:2-6` — exceptions go `logException` → `track("exception")`
  (`firebase/analytics.ts:94-96`) with no consent check.
- `web/src/routes/+layout.svelte:90,98` — `consent.hydrate()` then
  `const cleanups = [startAnalytics(), startCrashReporting()]` unconditionally in `onMount`; the
  root layout covers logged-out visitors too.

## 6. `user_sessions` UA/IP never swept — TRUE

- Schema: `migration/src/m000001_init.rs:48-57` — `device TEXT`, `ip_address TEXT`, `last_seen`,
  `revoked_at`; no expiry column.
- Writer: `modules/auth_v1/controller.rs:182-186` (UA + IP captured at login) → `:203`
  `create_bound_session` → `:1122-1125` → `db/sea_models/user_session/actions.rs:11-27`.
- Revoke does not clear them: `user_session/actions.rs:48-67` sets only `last_seen` + `revoked_at`.
- Only sweeper: `src/main.rs:764-775` hourly `sweep_store.delete_expired()` →
  `services/session_store.rs:52-62` — `DELETE FROM sessions WHERE expiry_date <= ?` (tower table
  only). No `DELETE FROM user_sessions` exists anywhere (`grep` → only SELECTs in tests).

## 7. `unverified` guard omits ban check — TRUE

- `rust/backend/api/src/middlewares/auth_guard.rs:46-55` — `unverified` uses
  `auth_requirements().authenticated().unverified()` with **no** `.not_banned()`; `authenticated`
  (`:31`), `verified` (`:64-67`) and `verified_with_role` (`:79-86`) all include `.not_banned()`.
- `crates/rux-auth/src/requirements/builder.rs:8,16,36-44` — `not_banned` defaults false and
  `unverified()` never sets it; `crates/rux-auth/src/middleware/guard.rs:46-56` runs the ban check
  only when `requirements.not_banned`.
- Scope: the only routes behind it are `/email_verification/v1/verify` and `/resend`
  (`modules/email_verification_v1/mod.rs:8-12`). TRUE; impact is narrow (verify/resend only).

## 8. Banned-user UX + limiter-before-ban — TRUE

- `rust/crates/ruxlog-types/src/error.rs:20-21` (AUTH_006 = `AccountLocked`), `:231` (403).
- `web/src/lib/auth/auth-copy.ts:80-88` — 403 in `CREDENTIAL_STATUSES`, AUTH_006 in
  `CREDENTIAL_TYPES`; `:137-139` → `CREDENTIAL_FAILURE` ("Email or password is incorrect.",
  `:3`); consumed at `web/src/lib/auth/flows.svelte.ts:118-134`.
- Ordering: `auth_v1/controller.rs:135` (limiter) before `:153` (ban check). A banned user with a
  correct password gets 403 on the first two attempts, then the 3rd is a 429 with
  `Retry-After: 3600` — and their attempts feed the same counter.

## 9. `rate_limit_state` snapshot, not ledger — TRUE (with a log-surface caveat)

- Schema: `src/services/rate_limit_store.rs:22-28` — `key, fixed_count, fixed_expires_at,
block_until_at, block_scope`; no `blocked_at`, no history.
- Purge: `:194` — `DELETE FROM rate_limit_state WHERE fixed_expires_at <= ? AND block_until_at <= ?`
  on every 10 s flush; `InMemoryStore::snapshot()` filters both-expired buckets
  (`crates/rux-request-gate/src/store.rs:142-144`); `prune()` at `rate_limit_store.rs:165`.
- "Blocks issued in 90 days" is unanswerable **from the table**. Caveat: the per-identity block is
  a WARN (`crates/rux-request-gate/src/abuse.rs:109`, "Request blocked by abuse limiter"), so
  retained container logs can answer part of it — route-level 429s cannot (claim 11). The DB claim
  stands; "unanswerable" is true only if logs are not retained.

## 10. Escalation default-OFF / unconfigured — TRUE (repo-level)

- `src/config/settings.rs:349` — `env_bool("QURAN_BAN_ESCALATION_ENABLED", false)`; `:350-363` —
  enabling without `QURAN_BAN_ALLOWLIST` is a boot error.
- `docker-compose.yml` — no `QURAN_BAN_*` (grep 0 hits); `.env.example` — none; `deploy/` — none.
- `src/main.rs:867-895` — engine constructed only when `cfg.enabled`; attached at `:912`
  (`quran_content_layer(...).with_escalation(escalation)`).
- Caveat: "provably zero in production" assumes the production env equals the repo defaults;
  production env values are not readable from the tree (C1's own confidence note). The engine
  issues IP blocks (`quran-ban:`), not `user_bans`; user bans have zero writers (claim 14 context,
  C3 F7).

## 11. Route 429s DEBUG-only, RUST_LOG=info — TRUE (default-value caveat)

- `crates/rux-request-gate/src/layer.rs:323-330` — `debug!(..., "Rate limit exceeded")` for every
  over-limit request.
- `docker-compose.yml:129` — `RUST_LOG: ${RUST_LOG:-info}` (default info; an operator-set
  `RUST_LOG=debug` would change this).
- The Quran content ceiling is a route-level layer: `middlewares/rate_limit.rs` `quran_content_layer`
  = `RateLimitLayer::builder(..., 600, 60)`, wired at `main.rs:912`.
- Nuance: when escalation is enabled it logs `info!` on ban creation
  (`services/escalation.rs:271,280`) — but it is default-off, and the 429s themselves stay
  invisible. The per-identity abuse block is WARN (`abuse.rs:109`).

## 12. FCM push token — PARTLY TRUE ("no revoke UI" is FALSE)

- No expiry: TRUE. `migration/src/m000005_device_notification_oauth.rs:13-23` — no expiry column;
  prune only on FCM `Unregistered` (`services/notification/mod.rs:111-122`) or cap eviction
  (`modules/device_v1/controller.rs:16-31`, cap 20).
- **No user-facing revoke UI: FALSE as stated.** The app shell renders `Tweaks` with
  `showReaderTools` defaulting true (`routes/(application)/+layout.svelte:219`;
  `lib/components/tweaks/Tweaks.svelte:19,333-336`), which renders the Notifications toggle
  (`lib/components/notifications/Notifications.svelte:15-18`) →
  `notifications.unsubscribe()` (`lib/stores/notifications.svelte.ts:316-321`) →
  `unregisterTokenFromServer` (`:151-155`) → `POST /device/v1/delete` + FCM `deleteToken`.
  What _is_ true: there is **no device list**, so a user cannot see or revoke tokens for other or
  stale browsers. The accurate statement is C4's own wording ("no device list in web/src
  settings"), not "no revoke UI".

## 13. Comment hide/delete: no reasons/notice/appeal — TRUE

- `modules/post_comment_v1/mod.rs:17-37` — admin hide/unhide/delete routes; `controller.rs:233-313`
  take only `comment_id` and return `{"message": ...}`; no reason parameter, no notification call
  (no `notification`/`notify` reference in the module), no `audit_logs` write.
- `grep -rniE "appeal|statement of reasons" rust/backend/api/src web/src` → **0 matches**.

## 14. `audit_logs` zero writers — TRUE

- Table: `migration/src/m000001_init.rs:252-264` (FK to users `ON DELETE SET NULL`).
- Model dir: `db/sea_models/audit_log/` contains only `mod.rs` + `model.rs` — no `actions.rs`/
  `slice.rs`.
- Repo-wide grep for `audit_log` outside that dir + the migration → only `db/sea_models/mod.rs:11`
  (`pub mod audit_log;`). No `INSERT INTO audit_logs` anywhere. Zero writers confirmed.

---

## Compact table

| #   | Verdict                          | Evidence (file:line)                                                                                                                                                                                                      | Correction                                                                                                                                                                                                                                                                                                                                    |
| --- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **PARTLY TRUE** (2nd half FALSE) | `rust/backend/api/crates/rux-request-gate/src/store.rs:305,322`; `auth_v1/controller.rs:31-38,135-145,153`; `abuse.rs:64-125`                                                                                             | Counts every attempt + never clears: TRUE. "3 successful logins" → the 3rd is itself 429'd (2 succeed). **"5 POSTs → 24 h" is FALSE**: 5 one-shot requests give only 1 h; 24 h needs ≥5 attempts in the trailing 900 s at block expiry (≥4 blocked retries in the final 15 min + 1 after → ≈8 timed requests). A 3+3+1 sequence is one short. |
| 2   | TRUE (wording)                   | `utils/email.rs:7-9`; `utils/email_alias.rs:8-18`; `config/settings.rs:48-53`; `auth_v1/validator.rs:44-47`; `user/actions.rs:280`                                                                                        | Not "+ normalization off" — production _rejects_ plus-tags; dots are never canonicalized anywhere. Bypass real.                                                                                                                                                                                                                               |
| 3   | TRUE                             | `migration/src/lib.rs:3-13`; `router.rs:140-141`; `newsletter_subscriber/model.rs:7`; `newsletter_subscriber/actions.rs:18-21`; `error/database.rs:45,58-61`; `ruxlog-types/error.rs:247`; dev DB `sqlite_master`         | 500 occurs at the duplicate-check SELECT, not the INSERT; same user-visible result.                                                                                                                                                                                                                                                           |
| 4   | TRUE (scope)                     | `web/messages/en.json:213,152`; `ar.json:213,152`; `site.ts:3,10`; DNS: app MX empty / A 185.199.x (GitHub Pages), fyi MX Cloudflare                                                                                      | Privacy policy has **no** contact address at all; the dead addresses are Terms + FAQ. No-MX → A-record fallback → GitHub Pages has no SMTP → bounce.                                                                                                                                                                                          |
| 5   | TRUE                             | `consent.svelte.ts:16,19-27`; `boot/analytics.ts:12-16,20-31`; `boot/crash-reporting.ts:2-6`; `+layout.svelte:90,98`; `firebase/analytics.ts:20,94-96`                                                                    | None needed.                                                                                                                                                                                                                                                                                                                                  |
| 6   | TRUE                             | `m000001_init.rs:48-57`; `auth_v1/controller.rs:182-186,203,1122-1125`; `user_session/actions.rs:11-27,48-67`; `main.rs:764-775`; `session_store.rs:52-62`                                                                | None. Only `sessions` is swept; no `DELETE FROM user_sessions` exists.                                                                                                                                                                                                                                                                        |
| 7   | TRUE                             | `middlewares/auth_guard.rs:46-55`; `rux-auth/requirements/builder.rs:8,16,36-44`; `rux-auth/middleware/guard.rs:46-56`; `email_verification_v1/mod.rs:8-12`                                                               | Scope: only `/email_verification/v1/verify\|resend`.                                                                                                                                                                                                                                                                                          |
| 8   | TRUE                             | `ruxlog-types/error.rs:20-21,231`; `auth-copy.ts:3,80-88,137-139`; `flows.svelte.ts:118-134`; `auth_v1/controller.rs:135,153`                                                                                             | First two attempts return 403; the 3rd returns 429 (limiter before ban check).                                                                                                                                                                                                                                                                |
| 9   | TRUE                             | `rate_limit_store.rs:22-28,165,194`; `store.rs:142-144`                                                                                                                                                                   | Unanswerable _from the table_; per-identity blocks are a WARN (`abuse.rs:109`) recoverable from retained logs.                                                                                                                                                                                                                                |
| 10  | TRUE (repo-level)                | `config/settings.rs:349-363`; `docker-compose.yml` (0 hits); `.env.example` (0 hits); `main.rs:867-895,912`                                                                                                               | "Provably zero" assumes prod env = repo defaults; prod env unreadable from tree. Engine issues IP blocks, not user bans.                                                                                                                                                                                                                      |
| 11  | TRUE                             | `rux-request-gate/src/layer.rs:323-330`; `docker-compose.yml:129`; `middlewares/rate_limit.rs` `quran_content_layer`                                                                                                      | Default `RUST_LOG=info`; an operator override would change it. Escalation (default-off) logs info on ban creation only.                                                                                                                                                                                                                       |
| 12  | **PARTLY TRUE**                  | `m000005_device_notification_oauth.rs:13-23`; `notification/mod.rs:111-122`; `device_v1/controller.rs:16-31`; `Notifications.svelte:15-18`; `notifications.svelte.ts:151-155,316-321`; `(application)/+layout.svelte:219` | No expiry: TRUE. **"No user-facing revoke UI" is FALSE** — the in-app Notifications toggle revokes the current device's token via `/device/v1/delete`. What is missing is a device **list** (revoke for other/stale devices).                                                                                                                 |
| 13  | TRUE                             | `post_comment_v1/mod.rs:17-37`; `controller.rs:233-313`; grep `appeal` → 0                                                                                                                                                | None.                                                                                                                                                                                                                                                                                                                                         |
| 14  | TRUE                             | `m000001_init.rs:252-264`; `db/sea_models/audit_log/` (mod+model only); repo grep → `sea_models/mod.rs:11` only                                                                                                           | None.                                                                                                                                                                                                                                                                                                                                         |

**Flags for the plan (most valuable output):**

1. Do not write "5 POSTs → 24 h lockout" — it is false and falsifiable; write the timed sequence
   (3 → 1 h; ≥4 blocked retries in the final 900 s; 1 after expiry) or just "the 24 h tier is
   reachable via the blocked-path append".
2. Do not write "no user-facing revoke" for FCM tokens — write "no device list; the current
   device's token can be removed via the notifications toggle".
3. Claim 4's privacy-address framing should be split: Terms/FAQ address is dead; the privacy
   policy has no address at all.
