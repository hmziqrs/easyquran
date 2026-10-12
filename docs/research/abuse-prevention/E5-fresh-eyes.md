# E5 — Fresh-eyes independent audit

Auditor: fresh-eyes subagent. No prior audit report read. Input = repository at `/Users/hmziq/os/easyquran`
(HEAD `53d3d421`, Mon Oct 12 2026) + the one-paragraph scope statement.

Every finding below is marked **NEW** (no prior reports were consulted, so no comparison is possible).

## What I read (code first, docs second)

- `rust/backend/api/src/{main.rs,router.rs,state.rs}`, `middlewares/*` (client_ip, rate_limit, static_csrf,
  cors, auth_guard, route_blocker, security_headers), `services/*` (auth, abuse_limiter, password_work,
  session_store, pending_login, passkey_state, fresh_auth, webauthn, turnstile, escalation,
  rate_limit_store, mail/cloudflare, oauth/state), `modules/*` (auth_v1, passkey_v1, admin_bans_v1,
  forgot_password_v1, email_verification_v1, device_v1, user_v1, newsletter_v1, analytics_v1, quran_v1,
  media_v1, csrf_v1), `db/sea_models/{user,user_ban}`, `migration/src/m000001_init.rs`.
- `rust/backend/api/crates/rux-auth/*` (session extractor + guard), `crates/rux-request-gate/*`
  (abuse + store + layer).
- `web/server.ts`, `web/src/hooks.server.ts`, `web/src/lib/server/*`, `web/src/lib/auth/auth-client.ts`,
  `web/src/lib/quran/*`, `web/src/routes/health/quran/*`, `web/src/routes/_quran/[...artifact]/+server.ts`,
  `web/scripts/gen-offline-pack.ts`, `web/static/_headers`.
- `docker-compose.yml`, `deploy/Dockerfile.{web,api}`, `deploy/README.md`, `.github/workflows/images.yml`,
  `.env.example`, `.gitignore`, `justfile`, `AGENTS.MD`.
- `git log` / `git show` for the HEAD commit (evidence, not a prior report).

Not read: anything under `/private/var/folders/.../fp-audit/`. I also deliberately did not read
the repo's security docs so as not to be anchored.

## Verification runs I actually executed (not guesses)

| Gate (removed from CI at HEAD)                                           | Result on HEAD                                                                                                                                                                                                        |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `web`: `bun run check` (svelte-check `--fail-on-warnings`)               | **passes** — "0 errors and 0 warnings"                                                                                                                                                                                |
| `web`: `bun run lint` (`vp lint --deny-warnings`)                        | **passes**                                                                                                                                                                                                            |
| `web`: `bun run test`                                                    | **passes** — 190 files, 2399 tests                                                                                                                                                                                    |
| `web`: `bun audit`                                                       | **passes** — "No vulnerabilities found (checked 464 packages)"                                                                                                                                                        |
| `rust`: `cargo test --workspace --locked -- --test-threads=1`            | **passes** — 39 suites, 0 failures                                                                                                                                                                                    |
| `rust`: `cargo clippy --workspace --all-targets --locked -- -D warnings` | **fails locally**: 16 × `double_must_use` in `rux-auth` + `rux-request-gate`. Local toolchain is rustc **1.99.0**; CI pins **1.98.1**, so this is likely new-lint drift, not a regression. Unverified against 1.98.1. |

So: the gates were removed **while passing**. That matters for the recommendation in Part 6.

---

# Part 1 — Top 10 security problems in SHIPPED code (ranked by real risk)

## 1. **NEW** — The whole IP-trust model rests on an unverifiable, in-repo-absent ingress rule; `CF-Connecting-IP` is trusted from any private-CIDR peer and nothing strips it

**Evidence**

- `docker-compose.yml:136` `IP_SOURCE: CfConnectingIp`; `:143` `TRUSTED_PROXY_CIDRS` defaults to
  `10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fd00::/8`.
- `rust/backend/api/src/middlewares/client_ip.rs:64-74` — the peer gate only requires the TCP peer to be
  inside `TRUSTED_PROXY_CIDRS`; the header is then believed verbatim (`:124-131`).
- `client_ip.rs:92-106` — a valid `X-EasyQuran-Internal-Token` grants `InternalService` identity with **no
  peer check at all**.
- `deploy/README.md:377-409` — "Restrict origin ingress to Cloudflare ranges (required before production)…
  Traefik (or the host firewall) MUST deny public origin ingress…" and "The live CF-range negative test … is
  the owner's sign-off; this script is the scaffolding." The api **cannot** tell a forged header from a real one.
- Repo contains **no** Traefik/Cloudflare configuration: `grep -rn "ipAllowList|ipWhiteList|forwardedHeaders"`
  over all yml/yaml/toml returns nothing. There is no `wrangler.toml`, no cache-rule file, no CF zone config.
- `docker-compose.yml:104-110, 181-192` — the only Traefik middlewares are `easyquran-web-compress` and
  `strip-api`. Nothing overwrites or clears `CF-Connecting-IP` on the way in.

**Consequence.** Anyone who can reach the origin hostname/IP (bypassing Cloudflare, or via any other DNS
record on that box) speaks to Traefik, which forwards to the api with a **private-range peer**, so
`CF-Connecting-IP` is trusted. That caller can then: (a) mint unlimited rate-limit buckets → the 600/min
content ceiling, the 100/min auth ceiling, and every per-IP abuse limiter become no-ops; (b) once W3a
escalation is enabled, pin _any_ IP they like into a durable ban (an attacker can ban a competitor, a
customer range, or a whole Cloudflare range); (c) defeat any Cloudflare WAF/rate rule that was assumed to
be in front. `TRUSTED_PROXY_CIDRS` does not help: Traefik's peer _is_ in that list, and Traefik is not
configured to sanitize the header.

**Fix.** Make it structural, not documented: terminate at a Cloudflare Tunnel (`cloudflared`) so no public
origin ingress exists, or ship the allowlist as code — Traefik `ipAllowList` from the CF ranges plus a
`forwardedHeaders`/header-rewrite middleware that _sets_ `CF-Connecting-IP` from the real peer — and add a
boot-time refusal in production when the ingress invariant cannot be demonstrated (e.g. require a
per-deployment `EDGE_SHARED_HEADER` that only Traefik can add, and treat a request without it as external).
Also add a scheduled job that re-fetches CF ranges, since `deploy/README.md:392` admits they change.

## 2. **NEW** — There is no automated gate between `master` and production: the security gates were deleted at HEAD and the same push auto-redeploys

**Evidence**

- `git show 53d3d421` (HEAD, "fix(ci): remove security publication gates") deletes from
  `.github/workflows/images.yml`: `bun run check`, `bun run lint`, `bun run test`, `bun audit`
  ("Web security gates" / "JavaScript dependency audit") and `cargo fmt --all --check`,
  `cargo clippy … -D warnings`, `cargo test --workspace …`, `cargo check … seed-system`,
  `python3 scripts/security/cargo-audit-gate.py` ("Rust security gates" / "Rust dependency audit").
- `.github/workflows/images.yml:104-112` — the web job now runs only `quran:check` + `vite build`;
  `:187-192` — the api job now runs only `cargo build --release`.
- `:239-266` `publish` (registry write) needs `[meta, web, api]`; `:269-292` `deploy` calls Dokploy
  `compose.redeploy` on every `master` push. So master push → build → push `latest` → redeploy.
- `deploy/README.md` (HEAD) now states the gates "remain local checks"; `web/README.md` same.
- `AGENTS.MD` still asserts the three gates "are green and must stay green" — policy now enforced only by
  a human remembering to run them.

**Consequence.** Any auth/rate-limit/CSRF/path-traversal regression merges and ships with zero machine
checks. The removed `cargo-audit-gate.py` specifically gated _publication_ on the dependency graph
(`rust/deny.toml` comment: "blocks publication if that crate becomes reachable in the shipped arm64
feature graph") — that control no longer exists anywhere.

**Fix.** Restore the four steps verbatim as a required job that `publish` needs (all of them pass today
except clippy on a newer-than-pinned toolchain — see the verification table). Keep `cargo test` out of the
hot path if it's too slow, but keep `cargo check` + `clippy` + `audit` + `bun run test`. This is cheap: the
scripts still exist (`scripts/security/cargo-audit-gate.py`).

## 3. **NEW** — You cannot ban anyone today: `user_bans` has no writer, the only automatic ban engine is default-OFF, and no operator route can create a ban

**Evidence**

- `services/auth.rs:530-547` reads `user_ban::Entity::get_active_ban`; the only other references to
  `user_ban` in the whole tree are the model and a test fixture (`grep -rn user_ban rust/backend/api/src`).
  There is **no** insert/upsert path — no admin endpoint, no service, no seed.
- `services/escalation.rs:267-277` is the only caller of `store.set_block(...)` in the tree; it is
  constructed only when `QURAN_BAN_ESCALATION_ENABLED=true`
  (`config/settings.rs:349` default `false`; `main.rs:867-895` attaches it to the public quran content
  limiter only).
- `modules/admin_bans_v1/mod.rs:17-33` exposes only `GET /admin/bans`, `DELETE /admin/bans`,
  `GET /admin/bans/export`. `controller.rs:123-153` deletes exactly two key shapes:
  `quran-ban:{unit}` and `ratelimit:{unit}:quran-v1` — and `BanUnit::parse` only accepts `/32`/`/64`
  (`dto.rs`, pinned by `controller.rs:299-307`).
- `deploy/README.md:411-437`: the edge enforcement adapter is explicitly "outside this repo… none lives in
  this repository, and an existing export row is **not** proof that an edge block is in place."
- `web/server.ts` has no ban/deny hook at all (no call to the api, no IP deny list).

**Consequence.** Goal (2) of the plan — "make banning abusive/illegal actors effective" — fails at step one.
A logged-in abuser cannot be banned through the product; the ban table is dead weight that only _reads_.
Even if a row were inserted by hand, it only gates the authenticated API surface (5-minute cache,
`rux-auth/src/requirements/builder.rs:16` + `middleware/guard.rs:46-56`); the web tier (reader SSR, SSG,
offline pack, artifact proxy) ignores bans entirely, so a banned IP can keep costing you bandwidth.

**Fix.** Add a write path (admin `POST /admin/bans` for IP units **plus** a user-level ban keyed by
`user_id` that flips `not_banned()` for every authenticated route), turn on W3a with a measured allowlist
after Part 1's fix, expose `login:`/`totp:`/user-id key classes in the admin list, and add a web-tier
enforcement hook (read the same export, or a shared deny list) so a ban covers the origin the attacker
actually hits.

## 4. **NEW** — The per-account login throttle is an unauthenticated, self-extending, unrecoverable lockout weapon

**Evidence**

- `modules/auth_v1/controller.rs:133-140` — the throttle runs **before** password verification, keyed on
  `login:{email.trim().to_lowercase()}`, i.e. per _account_, IP-independent.
- `controller.rs:31-38` + `services/abuse_limiter.rs:34-52` + `crates/rux-request-gate/src/store.rs:287-350`:
  `abuse_check` pushes a timestamp on **every** call (including allowed ones), then
  `short_count >= 3` within `temp_block_range=360s` → `Temp` block `3600s`;
  `long_count >= 5` within `block_range=900s` → `Long` block `86400s`. Blocked calls keep appending.
- `admin_bans_v1/controller.rs:128-153` — the only `clear_limit` callers target `quran-ban:` / `ratelimit:`
  keys; `login:{email}` is unreachable from any operator route, and `rate_limit_state` persists it across
  restarts (`services/rate_limit_store.rs` flush/load).

**Consequence.** Anyone who knows (or guesses, or enumerates — see #5) a victim's email can lock that
account out of password login with 3 requests, and escalate to a 24-hour block with ≈8 timed requests
(≥4 blocked retries in the block's last 15 min, then one after expiry — D1 §1), re-triggered at each expiry. There is no user remedy and no operator remedy. It also fires on
_successful_ logins: a normal user's third login inside six minutes is 429'd.

**Fix.** Count failures only (decrement/clear on success), make the block short and bounded
(e.g. 5 min doubling to ≤1 h), keep the per-IP ceiling as the real brake, and add an operator clear path
for every key class the limiter can create (or key account throttling on `(email, ip)` so the victim is
never punished for the attacker's IP).

## 5. **NEW** — `POST /forgot_password/v1/request` is an explicit, deliberate account-existence oracle

**Evidence**

- `modules/forgot_password_v1/controller.rs:343-349` — unknown email returns
  `ErrorCode::RecordNotFound` + "No account exists for that email".
- The intent is pinned by tests: `controller.rs:314-317` ("product decision: accurate feedback over the old
  SC-006 anti-enumeration mask") and `controller.rs:541-552` ("unknown_email_answers_explicit_record_not_found").
- Contrast: registration is carefully masked (`auth_v1/controller.rs:519-535`, "any distinguishable
  status/shape is an account-existence oracle") and password login uses a dummy Argon2 verify
  (`services/auth.rs:81-89,115`). The enumeration defence exists everywhere except here.

**Consequence.** Anyone can turn a list of emails into a list of confirmed accounts, at the per-IP rate of
the route (and unlimited with #1 unfixed — note the `forgot_password` nest has **no** nest-level limiter at
all, `router.rs:82-83`; only the in-handler abuse limiter protects it). That list is exactly the input for
#4 (targeted lockout) and for credential-stuffing prioritisation, and it leaks user data (who is a user of
this app) which is a privacy issue in its own right.

**Fix.** Restore the uniform response for `request` (the verify step can stay honest), or gate the honest
answer behind Turnstile + a stricter per-IP cap. Also mount `rate_limit_layer` on the
`/forgot_password/v1` and `/email_verification/v1` nests (`router.rs:82-83`), matching every other nest.

## 6. **NEW** — Registration is captcha-only, and each registration sends attacker-addressed mail; there is no per-recipient or global mail budget on that path

**Evidence**

- `modules/auth_v1/controller.rs:470-474` — the only admission control on register is
  `turnstile::verify_registration`; `services/turnstile.rs:17-56` is fail-closed in production when the
  secret is unset (good) but is per-attempt and has no per-IP/global registration quota.
- `controller.rs:506-517` — a successful registration always spawns `send_email_verification_code` to the
  **attacker-chosen** address.
- `router.rs:72-76` — the whole auth nest is 100 req/min/IP, keyed per matched route, so
  `/auth/v1/register` alone gets 100 sends/min/IP.
- The newsletter path shows the intended pattern and this path lacks it: `newsletter_v1/controller.rs:65-96`
  has both a per-email and a per-IP abuse limiter before sending.

**Consequence.** With a captcha-solving service (or one IP and 100 solves/min) an attacker can (a) bomb a
victim's inbox with your brand, (b) burn your mail provider's quota and reputation until SPF/DKIM-signed
verification and password-reset mail is throttled or blocked — which is a full auth outage for real users,
(c) grow `users`/`email_verification` rows without bound. `deploy/README.md:463-506` already warns that
deliverability failure is a silent auth outage; nothing in code prevents it.

**Fix.** Per-recipient + per-IP + global send budgets for verification mail (reuse the newsletter limiter
shape and `MailRouter`'s dedup/cooldown), and a global cap on new accounts per hour with an alert.

## 7. **NEW** — `PASSWORD_WORK` is a global fail-fast semaphore (2 slots) that any unauthenticated caller can saturate to 429 every login/registration/reauth

**Evidence**

- `services/password_work.rs:8-15` — `PASSWORD_WORK_CONCURRENCY`, default **2**, range 1..=32.
- `:56-58` — `budget.try_acquire_owned().map_err(|_| PasswordWorkError::Busy)` → maps to 429
  (`:25-32`, pinned by `:108-114`).
- Every consumer goes through it: password verify (`services/auth.rs:143`), the **dummy** verify for
  unknown emails (`auth.rs:86-89,115,136`), password hashing on register
  (`db/sea_models/user/actions.rs:88-91`), reauth (`auth_v1/controller.rs:416-428`), backup-code consume
  (`auth_v1/controller.rs:730-734`).

**Consequence.** ~2 concurrent Argon2 verifications is the entire login capacity. A handful of parallel
login attempts (each one takes a slot, even for a non-existent email — no account needed) makes every
other login, registration and reauth return 429 immediately. With #1 unfixed, there is no rate limit at
all to slow the flood down. This is a complete, trivial denial of the auth surface.

**Fix.** Replace `try_acquire` with a bounded wait (`acquire` + timeout) so bursts queue instead of
failing, raise the default, and/or give authenticated/reauth work reserved capacity. Keep the fail-closed
behavior only as the last resort, and alert on saturation (`telemetry::limiter_metrics` already exists).

## 8. **NEW** — Security-critical state lives in process-global, 10k-capped maps that evict the _oldest_ entry, so an unauthenticated flood can evict other users' in-flight challenges

**Evidence**

- `services/passkey_state.rs:88-115` — `MAX_STATES = 10_000`; when full it deletes the **oldest** entry
  (`:104-112`). Issued by the public `passkey login/begin` / `register/begin` routes
  (`modules/passkey_v1/mod.rs`).
- `services/oauth/state.rs:77-90` — same evict-oldest policy for `MAX_OAUTH_STATES`/`MAX_NATIVE_NONCES`.
- `services/pending_login.rs:59-104` — `MAX_GRANTS = 10_000`, returns 429 when full (fails closed for the
  attacker _and_ for every real 2FA user).
- `services/fresh_auth` (proof-of-fresh-identity) and `forgot_password_v1::reset_token` are in-process too;
  `deploy/README.md:225-227` admits "Sessions carrying in-memory proof must reach the same API process".

**Consequence.** A single client (60/min/IP per matched route; unlimited under #1) can fill these maps and
evict legitimate users' pending WebAuthn challenges and OAuth states — breaking their logins mid-flow — and
can 429 all new TOTP logins. It also hard-wires the deployment to exactly one api process: any future
second replica silently breaks fresh-auth (a security control) and every pending-login flow.

**Fix.** Scope these maps per session (quota per session id, no cross-session eviction), and move
fresh-auth from process memory to a signed short-lived token (HMAC over user id + session + expiry) so it
survives a replica count > 1 and a restart without weakening the guarantee.

## 9. **NEW** — The API's default CSP (used on every API response) carries `'unsafe-eval'`, and the origin allowlist is a single env string that is never validated against the deployed host

**Evidence**

- `middlewares/security_headers.rs:16-26` — `DEFAULT_CSP` includes `script-src 'self' 'wasm-unsafe-eval'
'unsafe-eval'` and `connect-src 'self'`; applied to every private-router response (`main.rs:856-858`),
  and the same for the public quran router (`main.rs:925-927`).
- `utils/cors.rs` → `allowed_origins_from_env` (`main.rs:156-163`); production fails closed if the list is
  missing/insecure, but nothing ties the list to `DOMAIN`. `.env.example:6` ships four origins
  (`easyquran.fyi`, `hmziq.rs`, `hzmiqrs.com`, `blog.hmziq.rs`) and `docker-compose.yml` never derives it.
- `web/src/hooks.server.ts:59-101` builds the _real_ browser CSP (no `unsafe-eval`), so the API's copy is
  effectively dead weight — but it is the one that would apply if any API path ever returns HTML
  (an error page, a Swagger UI, a future file-serving route).

**Consequence.** Today: low direct impact (the API serves JSON). But it is a latent footgun: any future
HTML/error surface served by the api inherits a policy that permits `eval`, and `connect-src 'self'` would
break legitimate callers. The four-origin allowlist also means a compromise/XSS of _any_ of the owner's
other domains inherits credentialed CORS to the API (mitigated by the CSRF header, so not a CSRF hole).

**Fix.** Drop `'unsafe-eval'` from the API policy (or stop emitting a CSP from the API at all — the web
tier owns it), and derive/assert the origin list against `DOMAIN` at boot.

## 10. **NEW** — Unauthenticated, unlimited, un-edge-cached bulk transfer on the web origin (offline pack + R2 artifact proxy) — a cost/abuse hole, not just a design smell

**Evidence**

- `web/server.ts:508-522` — `lateStaticResponse` serves any `GET /offline/*` from disk with
  `packPattern` → `IMMUTABLE` (`:296-310`), **no auth, no rate limit, no concurrency cap** anywhere in
  `server.ts`.
- `web/scripts/gen-offline-pack.ts:169-196` — the pack is `pack.<appVersion>.json` + `.br`/`.gz`; the local
  build I measured is `14,158,643` bytes raw / `589,573` bytes brotli.
- `web/src/routes/_quran/[...artifact]/+server.ts:17-80` — proxies R2 artifacts (translation SQLite, ~0.8–1.2 MB
  each per `web/src/lib/data/translations.json`) through the web container with
  `cache-control: public, max-age=31536000, immutable` and no limiter.
- `web/static/_headers:29-35` is a _static-host_ declaration (Cloudflare Pages/Netlify format) and does not
  create a Cloudflare cache rule; there is no cache-rule/Worker config in the repo. Cloudflare's default
  cacheable-extension set does not include `.json`, `.sqlite`, or `.db` — so unless a zone-level Cache Rule
  exists (unverifiable from here), these bodies hit origin every time.

**Consequence.** An anonymous client can loop `/offline/pack.<id>.json` and `/_quran/tanzil/translations/*`
to burn the VPS's bandwidth/CPU (each request streams from disk or through the R2 proxy), and each offline
install transfers ~590 KB–14 MB. Nothing in the repo bounds it, and (per Part 1 #1) nothing stops the
attacker from rotating IPs. This is the scope statement's "no edge cache contract" item — I confirm it, and
I rank it as a security/cost issue rather than a pure perf issue.

**Fix.** Commit the edge contract as code (a documented, testable Cache Rule / Worker: cache everything for
`/offline/*` and `/_quran/*`, immutable, plus a per-IP origin limiter as backstop), and add an origin-side
concurrency/rate cap in `web/server.ts` for those two prefixes.

---

# Part 2 — Top 5 abuse/availability problems (attacker with no account)

## A1. **NEW** — Complete auth outage by saturating 2 password-work slots

`services/password_work.rs:8-15,56-58`; reachable pre-auth via `POST /auth/v1/log_in` with any email
(the unknown-email path still burns a slot: `services/auth.rs:115,136`). Consequence: all logins,
registrations, reauths and 2FA backup-code checks return 429. Fix: queue instead of fail-fast; add reserved
capacity.

## A2. **NEW** — Global translated-reader outage by draining the single internal SSR bucket

`middlewares/rate_limit.rs:68-79` — one bucket `ratelimit:internal-webssr:quran-v1`, default
**600/min** (`config/settings.rs:462`), shared by the entire web container, not per user. The translated
reader SSR calls the api per page (`web/src/lib/server/quran-translation-page.ts:82-111` →
`web/src/lib/quran/range-fetch.ts:82-96`), and the web tier has **no rate limit of its own**
(`web/server.ts`, 881 lines, no limiter). A single client hammering e.g. `/t/en/<translator>/page/<n>`
(cookie-less → disk cache misses only once per key, but the 604 keys × translators are all reachable)
consumes the shared budget; when it is gone every translated page degrades to an empty shell
(`web/src/lib/server/reader-route-guards.ts:39-45` marks it pending/no-store, so at least it is not pinned
to disk). Consequence: global degradation of the translated reader by one attacker. Fix: per-user/per-IP
budgets on the web tier, a per-page SSR budget, and a much higher internal ceiling with a separate lane per
concern.

## A3. **NEW** — Unauthenticated session-row inflation via `POST /csrf/v1/generate`

`modules/csrf_v1/controller.rs:10-30` inserts a `sessions` row for every cookie-less call
(`services/session_store.rs:15-19,140-164`), rate-limited only per IP at 60/min
(`router.rs:57-60`), swept only when expired (`main.rs:764-775`, hourly; 14-day inactivity TTL,
`main.rs:781-788`). Consequence: unbounded SQLite growth on the `api_data` volume (disk exhaustion →
api crashloop) from a handful of IPs. Fix: don't persist anonymous sessions for CSRF alone (or cap the
anonymous session table / add a global admission budget).

## A4. **NEW** — Cost amplification on the public Quran surface

`modules/quran_v1/controller.rs:893-938` — a cold `/quran/v1/sources` fans out **380 HEAD requests** to R2 in
production (2 scripts + 378 catalogue entries; 382 non-prod; serialized by the memo mutex — E6 F4)
(`:995-1009`), and `/scripts` 2 (`:846-873`); the result is memoized only when _fully_ verified
(`:915-917`), so a partial failure re-fans on every request. `quran_v1/controller.rs:1216-1256,1258-1351`
serves translations through a pool capped at 8 resident corpora / 48 MB
(`config/settings.rs:299-301`), so cycling through the 378 catalogue ids forces continuous eviction +
SQLite re-reads. Consequence: an attacker on the 600/min content bucket can keep the api hot on R2 HEADs
and disk I/O without ever using a valid account. Fix: single-flight the cold resolve, cache partial results
with a short TTL, and add a per-source cost budget.

## A5. **NEW** — Mail as an attack surface (registration + newsletter)

Registration: 100 sends/min/IP to arbitrary addresses (#6 above). Newsletter:
`newsletter_v1/controller.rs:55-120` sends a confirmation mail per new address, limited to 5/address/min
and 20/IP/min (then 1 h blocks) — but with #1 unfixed, per-IP limits are nominal. Consequence: inbox bombing + provider
quota/reputation burn, which is an auth outage (`deploy/README.md:463-506`). Fix: global send budget +
domain allowlist/reputation check + alerting.

---

# Part 3 — Top 5 authentication weaknesses

## B1. **NEW** — No second factor on the paths that matter most, and the strongest paths are unthrottled

The password path is throttled per account (#4, badly) and reauth is throttled
(`auth_v1/controller.rs:411,443`), but the **passkey** login path has no abuse limiter at all
(`modules/passkey_v1/controller.rs:243-260` → `complete_passkey_login`) and neither do the OAuth callbacks
(`modules/google_auth_v1/controller.rs`). That is defensible (crypto-bound) — but it means the only
defence on those paths is the 60/min per-IP nest limiter, i.e. none once #1 is in play. Fix: add a
per-account/per-session failure throttle to passkey/OAuth finish, and alert on failure spikes.

## B2. **NEW** — `forgot_password` is the weakest link in the recovery chain: honest oracle + in-memory

single-use grant + no nest limiter
`forgot_password_v1/controller.rs:30-172` (reset tokens are process-global, 600s, single-use, bound to
`session_auth_secret` — good design) but the _enumeration_ step is honest (#5) and the nest has no
`rate_limit_layer` (`router.rs:82-83`). Reset itself rotates `session_auth_secret`
(`db/sea_models/user/actions.rs:223-228`), killing all sessions — good. Fix: close the oracle, mount the
nest limiter, and persist reset grants (or move to a signed token) so a restart doesn't silently invalidate
an in-flight reset.

## B3. **NEW** — Account-recovery and verification mail share one transport with billing/marketing, with no

priority separation
`main.rs:29-124` builds one `MailRouter` for everything; `main.rs:648-652` rides the mail dedup/cooldown on
the same gate store. A mail-bombing run (#6/A5) degrades verification and reset mail first. Fix: separate
queues/budgets for transactional auth mail and an explicit "auth mail never dropped" budget.

## B4. **NEW** — `fresh_auth` (the proof gate for security-changing operations) is process-memory only,

so a restart or a second replica silently changes its meaning
`auth_v1/controller.rs:404-455` + `deploy/README.md:225-227` ("a restart safely requires confirmation
again"). Confirmation again is safe-fail; but a second replica would make _some_ requests see "not fresh"
and others see "fresh" unpredictably. Fix: HMAC-signed freshness token in the session.

## B5. **NEW** — Email-change path is sound but silently locks the account out of `verified` routes with no

in-flow re-verification send
`db/sea_models/user/actions.rs:142-166` sets `is_verified=false`, rotates `session_auth_secret` (all
sessions die) and deletes pending verification rows; `user_v1/controller.rs:222-236` requires fresh auth
first. But nothing in the update flow issues a new verification code, so the user must discover
`email_verification/v1/resend` (whose own delay gate is `email_verification_v1/controller.rs:73-88`).
Consequence: self-inflicted lockout from `verified` features; fix by triggering the send in the same
transaction (queued, not inline).

---

# Part 4 — Scope critique: what is obviously missing, and what I would insist on before any of it ships

## Missing from the plan

1. **The ingress invariant is not in the plan at all, yet every other control depends on it.** The plan
   talks about account-centric defences and a device token while the ban/rate-limit substrate trusts a
   forgeable header (`client_ip.rs:64-131`, `deploy/README.md:377-409`). A device token whose fingerprint is
   keyed to a spoofable IP is not evidence. **Insist:** the origin-ingress restriction becomes an in-repo,
   testable artifact (Tunnel or Traefik allowlist + header rewrite) _before_ anything else is built, and a
   production boot check fails when the invariant is absent.
2. **No ban-write path is in scope.** "Wire the dormant `user_bans`" is stated, but the plan should state
   the _operator_ verbs: create/expire/revoke for both IP units and user ids, plus the key classes the
   limiters create (`login:`, `totp:`, `forgot_password*`). Without that, "effective banning" is still
   impossible after the work.
3. **No web-tier enforcement.** A ban that only 429s `/api` leaves the reader, SSG, pack and artifact proxy
   open (`web/server.ts`). Any ban design must name where the web origin learns about it.
4. **No abuse budget for outbound mail / no global spend circuit-breaker.** #6 and A5 are the cheapest
   attacks on this app and they are not mentioned.
5. **No test/observability contract for the ban system.** Nothing in the plan says how you will _know_ a ban
   is in effect (a counter for "ban hit", a synthetic check that a banned unit gets 403/429 at the edge).
   `deploy/README.md:429-436` already admits an export row is not proof of an edge block.
6. **No data-minimisation answer for the device token.** The plan says "evidence only, never a sole ban
   cause" — good — but says nothing about the token's own storage, rotation, revocation-on-logout, or its
   interaction with the "retention for stored identifiers" item. It also does not say what happens to a
   device token when the account is deleted (the `devices` table already has `ON DELETE`? verify) or when a
   shared device changes hands (the FCM `device` table explicitly transfers ownership on re-register —
   `migration/src/m000011_device_token_ownership.rs`, `modules/device_v1/controller.rs:42-50` — the same
   hazard applies to a device token: two accounts on one browser).
7. **Nothing about the _deletion_ of illegal content.** "Illegal actors" implies content takedown
   (comments, posts, uploads) — the plan covers actors, not artifacts, and `media` uploads go to R2
   (`services/media.rs`, `modules/media_v1`) where a takedown needs the object removed, not just a row.
8. **No mention of who can act.** Admin routes exist (`verified_with_role::<ROLE_ADMIN>`) but there is no
   audit trail requirement for ban/unban decisions in the plan (`audit_log` model exists but is not wired
   into admin_bans_v1 — verify before relying on it).
9. **No rollback/kill switch for each new control.** If the new admission control or device token
   misfires, how is it disabled without a deploy? The existing code uses env flags
   (`QURAN_BAN_ESCALATION_ENABLED` etc.) — the plan should reuse that pattern explicitly.

## I would insist on before any of it ships

1. Part 1 #1 fixed and _proven_ (an automated negative test that a non-Cloudflare client cannot set
   `CF-Connecting-IP` and get a distinct bucket), because every downstream control is decorative otherwise.
2. Part 1 #2 restored (gates green in CI before publish) — this is the mechanism that stops the rest of the
   plan from being undone by the next commit.
3. A ban-write path + a web-tier enforcement point + an observability counter, demonstrated end-to-end with
   a banned test identity on both origins.
4. The login throttle redesign (#4) shipped **before** the new device token, because the current throttle is
   itself the most effective abuse tool in the app.
5. A mail-spend circuit breaker.

---

# Part 5 — What the scope gets wrong or over-engineers (blunt)

- **Wrong ordering.** It puts "account/credential-centric defences first" and defers fingerprints, but it
  omits the one control that makes all of them meaningful: authenticated ingress identity. Building an
  evidence-grade device token on top of a spoofable client IP produces evidence an attacker can fabricate
  (they choose the IP that gets stored with the token).
- **"Fix the login rate limiter" understates it.** This is not a tuning bug; the current design is an
  unauthenticated, unrecoverable account-lockout weapon (#4). It needs a redesign (failure-only counting,
  bounded block, operator clear path), not a parameter change.
- **"Add notices + appeals" is premature and unenforceable.** You cannot ban anyone yet (#3). Appeals for a
  ban system that cannot issue bans is building the courtroom before the police. Ship the write path and
  the enforcement point first; notices/appeals are a UI layer on top.
- **"Restore the deliberately-removed CI gates" is correct but framed as optional cleanup.** It is the
  highest-leverage single item in the plan and it costs almost nothing (all gates pass today; only clippy
  needs the toolchain pin decision).
- **Over-engineering risk: the server-minted device token.** As described ("evidence only, never a sole ban
  cause") it cannot by itself justify a ban, so its value is correlation — and the plan has no stated
  correlation consumer (no analyst workflow, no review queue). Meanwhile the FCM `devices` table already
  demonstrates the failure mode: token ownership transfers between accounts on re-registration
  (`modules/device_v1/controller.rs:42-50`, migration `m000011`). A device token with the same
  transfer semantics is worse than useless as evidence. If it ships, it must be per-account-scoped,
  rotate-on-logout, and _never_ re-bindable across accounts without an explicit, logged transfer.
- **Over-engineering risk: the "measured gate" for browser fingerprinting.** This is a good instinct, but
  the plan should say what the _measurement_ is. Without a defined metric (e.g. "ban evasion rate on
  accounts with ≥2 confirmed abuse events, measured against a holdout"), the gate is unfalsifiable and will
  be argued about forever. Fingerprinting is also the item most likely to create GDPR/UK-GDPR exposure for a
  religious-content app, which is a real risk the plan should name.
- **Under-specified: "retention for stored identifiers."** Which identifiers, what retention, enforced how?
  The app currently stores `user_sessions.device` (User-Agent), `user_sessions.ip_address`
  (`migration/src/m000001_init.rs:47-57`), `devices.token`, `forgot_password`/`email_verification` rows, and
  `rate_limit_state` keys that embed emails and IPs (`login:{email}`, `quran-ban:{cidr}`). Retention without
  a per-table schedule and a deletion job is a document, not a control.
- **Reader-path cost work is right but mis-framed as availability-only.** The 600/min global internal
  bucket, the 14 MB unauthenticated pack, and the absent edge cache are also _abuse_ surfaces (Part 2 A2/A4,
  Part 1 #10). Frame them with a spend/abuse budget, not just "make the reader fast".

---

# Part 6 — The single first change

**Restore the CI gates (and make `publish` depend on them).**

Justification:

- It is the only change that protects every other change in the plan. Right now a master push builds,
  publishes `latest`, and redeploys production with zero tests (`images.yml:239-292`), and the security
  gates were deleted at HEAD (`53d3d421`) _while all of them pass locally_ — web check/lint/test (0
  errors/warnings; 2399 tests), rust workspace tests (39 suites), `bun audit` clean. This is the cheapest
  high-leverage fix available, and the `cargo-audit-gate.py` script still exists
  (`scripts/security/cargo-audit-gate.py`) so nothing has to be rewritten.
- The alternative "first change" (fix the ingress invariant, Part 1 #1) is _more important_ but is
  partly outside the repo and cannot be verified from here; the CI gate is fully in-repo and blocks
  regressions in the meantime. If the team can do both in one PR, do both — but the CI gate is the one
  that cannot fail for environmental reasons.

(If the question is "which _code_ change first", the answer is the login-throttle redesign, Part 1 #4:
it removes the most effective abuse tool currently in the product.)

---

# Part 7 — The one thing that makes this app most vulnerable today

**Its ingress identity is unauthenticated and unverifiable from the repository.** `CF-Connecting-IP` is
believed from any peer inside `TRUSTED_PROXY_CIDRS` (`client_ip.rs:64-131`), Traefik is not configured to
set or strip it, no allowlist exists in the repo, and the required firewall rule lives only in
`deploy/README.md:377-409` as an owner-run manual step with an owner-run negative test. Everything the plan
wants to build — rate limits, IP bans, escalation, "evidence" device tokens — is keyed on that value. If the
rule is not in place (and nothing in the repo can tell you it is), then today: rate limits are advisory,
bans are attacker-directed, and the WAF is bypassable by anyone who knows the origin address.

---

# Appendix — things I could not determine (stated rather than speculated)

1. Whether the Cloudflare-range ingress restriction and the edge cache rules are actually configured on the
   live deployment. No config exists in the repo; `deploy/README.md:377-409` says it is manual and
   owner-signed.
2. Whether any repo doc records an accepted-risk decision for the login throttle, the enumeration oracle,
   or the ingress trust model. I deliberately did not read the security docs, to keep this audit unanchored.
3. Whether clippy passes on the CI-pinned toolchain (1.98.1). It fails on the locally installed 1.99.0 with
   16 `double_must_use` errors, which looks like new-lint drift.
4. Whether Cloudflare actually caches `/offline/*` and `/_quran/*` (extension-based defaults say `.json`
   and `.sqlite` are not in the default set; a zone-level Cache Rule could override — unverifiable here).
5. Whether the `audit_log` model is wired to admin ban actions (`db/sea_models/audit_log` exists; I did not
   trace every writer).
6. Whether a second api replica is ever intended; several controls (`fresh_auth`, pending-login grants,
   passkey/oauth state) are correct only at replica count 1, and `deploy/README.md:225-227` acknowledges
   this in prose rather than in code.
