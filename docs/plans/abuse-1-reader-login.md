# Abuse phase 1: reader path and login

[Back to index](../README.md)

Measured first.

#### A10. Verification identity canonicalization

Build:

1. Gmail-family normalization: strip dots in local part for `gmail.com` and `googlemail.com`. Current alias logic (`rust/backend/api/src/utils/email.rs` ~7–9, `email_alias.rs`) handles `+` only; production rejects plus-tags.
2. Migration (next free): `users.verification_identity_hash` (nullable, indexed). Value: keyed HMAC (new setting `VERIFICATION_IDENTITY_KEY`, required in production, not SHA) of the canonical address. Backfill: lazy on next login or a boot job that computes it. Do not compute over Quran data (not applicable here, but keep the hard rule).
3. Caps: 3 pending and 5 lifetime accounts per verification identity.

Files: `rust/backend/api/src/utils/email.rs`, `rust/backend/api/src/utils/email_alias.rs`, new migration, `rust/backend/api/src/db/sea_models/user/*`, `rust/backend/api/src/modules/auth_v1/*`, `rust/backend/api/src/config/settings.rs`.

Done when: `a.b@gmail.com` and `ab@gmail.com` map to the same identity; a dot-alias cannot exceed the lifetime cap; migration re-runnable; test in `tests/registration_admission.rs`.
Blocked by: A9

#### A11. Password-work queue and reserved lanes

Build:

1. `rust/backend/api/src/services/password_work.rs` (capacity 2, fail-fast 429 at ~lines 8–15 and 56–58). Replace fail-fast with a bounded queue: 1.5 s wait, waiter cap. Waiter cap: pick a small bound, record it in a code comment and the commit message.
2. Separate lane for the dummy verification (currently burns a slot in `services/auth.rs` ~86–89, ~115, ~136).
3. Reserved lane for already-authenticated operations (reauth, fresh-auth).
4. Telemetry: counter for password-work saturation.

Files: `rust/backend/api/src/services/password_work.rs`, `rust/backend/api/src/services/auth.rs`, `rust/backend/api/src/modules/auth_v1/controller.rs`.

Done when: 20 parallel logins do not deny a 21st legitimate login beyond the queue wait; saturation counter increments in test; test in `tests/auth_availability.rs`.
Blocked by: A8

#### A12. Recovery masking, reset grants, email change

Build:

1. `rust/backend/api/src/modules/forgot_password_v1/controller.rs` (~343–349): the unknown-email response is an account-existence oracle. Make status, body, and timing identical for known and unknown addresses. Update the tests that pin the old behavior.
2. Mount the missing nest limiter (`rust/backend/api/src/router.rs` ~85).
3. Count failures only on verify, not on request.
4. Move reset grants into the session payload (process-local today in `services/fresh_auth.rs` ~60; lost on restart).
5. Email change: `rust/backend/api/src/db/sea_models/user/actions.rs` (~142–166) sets `is_verified = false`, rotates the session secret, deletes verification rows, and sends nothing. Fix: verify the new address before switching, or send a code on change and return an explicit marker. A typo must be recoverable without operator help.

Files: `rust/backend/api/src/modules/forgot_password_v1/*`, `rust/backend/api/src/modules/user_v1/*`, `rust/backend/api/src/db/sea_models/user/actions.rs`, `rust/backend/api/src/services/fresh_auth.rs`, `rust/backend/api/src/router.rs`.

Done when: known vs unknown email response identical in status, body, and timing (test); email-change typo recoverable with a code (test); reset grant survives a process restart (test with session payload).
Blocked by: A8

#### A13. Challenge maps: no eviction, per-session quota, session payload state

Build:

1. `rust/backend/api/src/services/passkey_state.rs` (~104–112) and `rust/backend/api/src/services/oauth/state.rs` (~71–79, ~142–150): at the cap, refuse instead of evicting the oldest entry. Add a per-session quota for passkey and OAuth maps.
2. Move fresh-auth (`services/fresh_auth.rs` ~60) and pending-login (`services/pending_login.rs` ~44) state into the session payload.
3. `AuthSessionState` change: add new fields with `#[serde(default)]`, append-only ordering. Test a legacy payload decodes (a missing field must not turn every live session into anonymous or a 500).
4. Failure-only counters on passkey and OAuth finish paths.

Files: `rust/backend/api/src/services/{passkey_state,pending_login,fresh_auth}.rs`, `rust/backend/api/src/services/oauth/state.rs`, `rust/backend/api/src/modules/{auth_v1,passkey_v1}/*`, session payload types.

Done when: an unused passkey challenge survives an API restart (test); cap refusal tested; legacy session payload test green; session-payload tests updated and named in the commit message.
Blocked by: A12

#### A14. Reader-path cookie and cache predicate

Build:

1. `rust/backend/api/src/main.rs` (~781–788, `SessionManagerLayer`): set the session cookie path from a new `SESSION_COOKIE_PATH` setting, `/api` in production and `/` by default. Local compose serves the API at `http://localhost:8888` with no `/api` prefix, so a hard-coded path would break local sign-in. Confirm CSRF and any other cookie still work for their routes; fix any path dependency. Native clients (D6) use the same base path.
2. Login-only marker cookie `ruxlog.auth=1`, `Path=/`: set wherever a login completes (password and TOTP in `auth_v1/controller.rs`, `passkey_v1::login_finish`, `oauth::finish_oauth_login` for all five providers), cleared on logout. A stale marker after expiry only costs a cache miss.
3. `web/src/hooks.server.ts` (~138 and ~474, `requestHasCookie`): the cache predicate must use `ruxlog.auth` for "has a session". It must ignore `_ga`, `__cf_bm`, `ruxlog.sid`. The predicate must never become "no cookies at all" (GA4 `_ga` is default-on and would make every returning reader uncacheable).
4. Session cookie name stays `ruxlog.sid` (hardcoded; a configurable name widens the cookie-fixing surface).
5. Web matches the marker by name (`AUTH_COOKIE_RE`), not cookie presence. Keep `responseSetsCookie` as the predicate's second half.

Files: `rust/backend/api/src/main.rs`, `rust/backend/api/src/config/settings.rs`, `docker-compose.yml`, `rust/backend/api/src/modules/auth_v1/controller.rs`, `rust/backend/api/src/modules/passkey_v1/controller.rs`, `rust/backend/api/src/services/oauth/mod.rs`, `web/src/hooks.server.ts`, `web/src/lib/server/quran-disk-cache.ts`.

Done when: a translated page loaded logged-in and logged-out both return a cache HIT or disk-cache hit (test or manual check via `tests/quran_v1.rs` plus web test); no `private, no-store` on `/t/**` for a logged-in reader; a device cookie never appears on `/t/**` (no device cookie yet; guard test in place); web test covers the predicate for `_ga`, `ruxlog.sid`, `ruxlog.auth`; every login path sets the marker (test); local compose sign-in still works.

#### A15. Web-origin limiter, internal bucket split, prod boot error

Build:

1. `web/server.ts`: limiter for SSR misses. Identity: marker-gated `cf-connecting-ip` (needs A1). Global miss budget below the API's 600/min. Health exempt. Logged-in readers exempt. Add a shared-IP step-down (a shared address gets the lower class). Ceiling values come from env, at ≥4× measured peak (O4). Limiter stays off until they are set.
2. Split the internal SSR bucket by render class. Renderer sends `x-eq-render-class: html|md` (`web/src/lib/server/quran-translation-page.ts`). Add `quran-v1-md` and `quran-v1-range` layers and a global backstop. Current bucket: `ratelimit:internal-webssr:quran-v1`, 600/60 s, in `rust/backend/api/src/middlewares/rate_limit.rs` ~68–79.
3. Boot error: production refuses to boot the web server when `INTERNAL_QURAN_API_BASE` or `INTERNAL_QURAN_API_TOKEN` is missing. Today `web/src/lib/server/quran-translation-page.ts` (~33–41) falls back to `QURAN.apiBase`, which lands in the escalatable external bucket, so one IP can earn a 7-day ban for everyone.
4. Warn on swallowed SSR failures (log at `warn`).

Files: `web/server.ts`, `web/src/lib/server/quran-translation-page.ts`, `web/src/lib/server/quran-disk-cache.ts`, `rust/backend/api/src/middlewares/rate_limit.rs`, `web/src/env.ts` if a new `PUBLIC_*` var is added.

Done when: one IP hammering translated pages cannot push another IP's renders into the shared internal bucket (Rust test on the layers and web test); prod boot fails without internal vars (web test); the web limiter exempts health.
Blocked by: A1, A5
Owner: O4 for ceiling values, O5 for ship window.

#### A16. Offline pack budget and cache semantics

Build:

1. Per-IP budget: 20 downloads per day, burst 5, and 100 MB per IP per day for the offline pack. Enforce in `web/server.ts` (`packPattern` ~line 158; `/offline/` handling ~line 508–522). Over budget → 429.
2. Client fetch in `web/src/lib/offline/offline-store.svelte.ts`: the pack (~226) has a hashed name, so use the default cache mode. `manifest.json` (~185, ~208) has a fixed name, so use `no-cache` to revalidate; the default could hide a new pack from readers.
3. Edge caching: Cloudflare Cache Rule for `/offline/*` is owner-only (O14).

Files: `web/server.ts`, `web/src/lib/offline/offline-store.svelte.ts`.

Done when: pack download #21 from one IP in a day returns 429; byte budget enforced (test); client fetch uses default mode for the pack and `no-cache` for the manifest (test asserting fetch options).
Blocked by: A15
Owner: O14 for the edge cache part.

#### A17. Canonical redirects, no-store errors, per-artifact verified set

Build:

1. Canonical 308 redirects for `from`/`to` permutations and sorted `keys` on the quran translation routes. Drop `Vary: Accept` from HTML responses.
2. `Cache-Control: no-store` on every error response: `rust/backend/api/src/error/response.rs` (~88–120).
3. Per-artifact verified set for `/quran/v1/sources` and `/scripts`: `rust/backend/api/src/modules/quran_v1/controller.rs` (~893–938). Today it fans out 380 HEAD requests in production (382 non-prod) per request when the artifact set is not fully verified, serialized by the memo mutex. Cache verification per artifact set. Second request must make 0 HEAD requests.
4. `/quran/health/ready` returns a hard-coded `ready: true`. Add a real DB check so uptime monitoring (O19) means something.

Files: `rust/backend/api/src/modules/quran_v1/controller.rs`, `rust/backend/api/src/error/response.rs`, `rust/backend/api/src/router.rs`, `web/server.ts` (if redirect lives there).

Done when: second `/quran/v1/sources` request makes zero HEADs (test counts); error responses carry `no-store` (test); permutations 308 to canonical (test). Quran byte identity and baked id maps unchanged.

#### A18. Bulk-data path and observability

Build:

1. Publish the bulk path: `deploy/bulk-data.md`, `/.well-known/quran-bulk.json`, a section in `llms.txt` (served by `web/src/routes/llms.txt`).
2. `web/static/robots.txt`: `Disallow: /offline/`.
3. Remove ruxlog residue from the API `robots.txt` (`robots_txt` in `rust/backend/api/src/router.rs`, ~246).
4. Wire the collected-but-unread disk-cache stats into `/health/quran`.
5. Warn on swallowed SSR failures (completes A15 item 4 if not already done).

Files: new `deploy/bulk-data.md`, `web/static/*` (well-known file), `web/src/routes/llms.txt`, `web/static/robots.txt`, `rust/backend/api/src/router.rs`, `web/src/lib/server/quran-disk-cache.ts`.

Done when: `/.well-known/quran-bulk.json` returns 200; robots disallows `/offline/`; `/health/quran` includes disk-cache hit rate; the disk-cache hit rate appears in `just abuse-report` (A5 item 6).
Blocked by: A5
Owner: O10 (default: publish).
