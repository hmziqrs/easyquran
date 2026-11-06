# Abuse phase 1: reader path and login

[Back to index](../README.md)

Measured first.

#### A14. Reader-path cookie and cache predicate

Build:

1. `rust/backend/api/src/main.rs` (~781–788, `SessionManagerLayer`): set the session cookie path from a new `SESSION_COOKIE_PATH` setting, `/api` in production and `/` by default. Local compose serves the API at `http://localhost:8888` with no `/api` prefix, so a hard-coded path would break local sign-in. Confirm CSRF and any other cookie still work for their routes; fix any path dependency. Native clients (D6) use the same base path.
2. Login-only marker cookie `ruxlog.auth=1`, `Path=/`: set wherever a login completes (password and TOTP in `auth_v1/controller.rs`, `passkey_v1::login_finish`, `oauth::finish_oauth_login` for all five providers), cleared on logout. A stale marker after expiry only costs a cache miss.
3. `web/src/hooks.server.ts` (~138 and ~474, `requestHasCookie`): the cache predicate must use `ruxlog.auth` for "has a session". It must ignore `_ga`, `__cf_bm`, `ruxlog.sid`. The predicate must never become "no cookies at all" (GA4 `_ga` is default-on and would make every returning reader uncacheable).
4. Session cookie name stays `ruxlog.sid` (hardcoded; a configurable name widens the cookie-fixing surface).
5. Web matches the marker by name (`AUTH_COOKIE_RE`), not cookie presence. Keep `responseSetsCookie` as the predicate's second half.

Files: `rust/backend/api/src/main.rs`, `rust/backend/api/src/config/settings.rs`, `docker-compose.yml`, `rust/backend/api/src/modules/auth_v1/controller.rs`, `rust/backend/api/src/modules/passkey_v1/controller.rs`, `rust/backend/api/src/services/oauth/mod.rs`, `web/src/hooks.server.ts`, `web/src/lib/server/quran-disk-cache.ts`.

Done when: a translated page loaded logged-in and logged-out both return a cache HIT or disk-cache hit (test or manual check via `tests/quran_v1.rs` plus web test); no `private, no-store` on `/t/**` for a logged-in reader; a device cookie never appears on `/t/**` (no device cookie yet; guard test in place); web test covers the predicate for `_ga`, `ruxlog.sid`, `ruxlog.auth`; every login path sets the marker (test); local compose sign-in still works.

#### A17. Canonical redirects, no-store errors, per-artifact verified set

Build:

1. Canonical 308 redirects for `from`/`to` permutations and sorted `keys` on the quran translation routes. Drop `Vary: Accept` from HTML responses.
2. `Cache-Control: no-store` on every error response: `rust/backend/api/src/error/response.rs` (~88–120).
3. Per-artifact verified set for `/quran/v1/sources` and `/scripts`: `rust/backend/api/src/modules/quran_v1/controller.rs` (~893–938). Today it fans out 380 HEAD requests in production (382 non-prod) per request when the artifact set is not fully verified, serialized by the memo mutex. Cache verification per artifact set. Second request must make 0 HEAD requests.
4. `/quran/health/ready` returns a hard-coded `ready: true`. Add a real DB check so uptime monitoring (O19) means something.

Files: `rust/backend/api/src/modules/quran_v1/controller.rs`, `rust/backend/api/src/error/response.rs`, `rust/backend/api/src/router.rs`, `web/server.ts` (if redirect lives there).

Done when: second `/quran/v1/sources` request makes zero HEADs (test counts); error responses carry `no-store` (test); permutations 308 to canonical (test). Quran byte identity and baked id maps unchanged.
