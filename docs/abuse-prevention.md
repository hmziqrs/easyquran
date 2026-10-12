# Abuse prevention and account enforcement

Status: **draft plan, not implemented.** Baseline `53d3d421` (2026-10-12).

Evidence base: 59 adversarial audit reports in [`docs/research/abuse-prevention/`](research/abuse-prevention/) — 7 external research reports (`R2-*`) and 52 audit/verification reports (`A*`, `B*`, `C*`, `D*`, `E*`, `F*`, `G*`, `H*`). Their factual claims have been reconciled; where they still differ it is on design decisions, and later reports supersede earlier ones. This document is authoritative where a report differs; §16.1 lists claims that must not be repeated. Do not cite a report's number without checking §16.1.

Relations: [`next-features.md`](next-features.md) wins on reader/sync/bookmark feature work; this document wins on abuse, enforcement, device identity, and the legal artifacts attached to them. [`quran-system.md`](quran-system.md) Part 4 is the ingress and rate-limit contract this plan amends. [`history.md`](history.md) is the record of landed work, security included.

---

## 0. Context the implementing agent must load first

| Area       | Load                                                                             |
| ---------- | -------------------------------------------------------------------------------- |
| Any work   | `AGENTS.MD` (hard rules), this document, and the `caveman` skill per house rules |
| Rust API   | `rust/backend/api/AGENTS.md`, `rust/backend/api/readme.md`                       |
| Web        | `web/README.md`, skills `svelte-core-bestpractices`, `svelte-code-writer`        |
| Migrations | `rust/backend/api/migration/README.md`                                           |
| Deploy     | `deploy/README.md`                                                               |
| Evidence   | the relevant reports in `docs/research/abuse-prevention/` for the workstream     |

**Hard-rule compliance statement.** Nothing in this plan modifies, versions, or hashes a Quran database. `db/` stays gitignored; nothing under it is read at runtime, committed, or added with `git add -f`. No SHA-256 is introduced over Quran data in any automated path — the digests this plan adds are keyed HMACs over account/abuse data (comment bodies that a human has already confirmed as violating, never the Quran text or its translations). The sha guards (`rust/backend/api/tests/quran_v1.rs` `scripts_endpoint_carries_no_sha256`, `sources_rows_never_carry_sha256`; `web/src/lib/quran/__tests__/catalogue-sha-guard.test.ts`) must stay green. The reader delivery contract is unchanged: `/t/**` and `/{surah}/t/**` stay SSR with the 7-day disk cache, never SSG, prerender stays Arabic-only, production bytes come from baked id maps, and reader hrefs keep using the `surah*For(ctx, …)` family.

---

## 1. Guarantee and non-goals

**The guarantee this plan delivers, stated so it can be checked:**

> An operator can, in under five minutes and without touching the database by hand, terminate an account, kill its live sessions, deliver a written reason to a real channel, and record the decision so it can be reviewed, appealed, and defended six months later. Every control that decides _who_ to act on is derived from server-side evidence the client cannot author, and every stored identifier has an enforced expiry.

**Non-goals (deliberate, with reasons).**

| Not built                                                                       | Reason                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser component fingerprinting (canvas, WebGL, audio, fonts) as an identifier | Client-authored and cohort-valued: measured mobile fingerprint uniqueness is 18.5% (Gómez-Boix 2018, one French site — population-specific), 45.52% of fingerprints change within a day (FP-Stalker 2018, a larger attribute set), and Safari 26 noises canvas/WebGL/audio for all browsing while Brave's farbling is reversible. It would ban populations, not actors. |
| Device-based account bans                                                       | The device key is a cohort key. A shared phone (family, madrasa, internet café) would inherit one member's ban. See §6.4.                                                                                                                                                                                                                                               |
| Behavioural biometrics (keystroke, mouse, tap dynamics)                         | GDPR Art 9 biometric-for-unique-identification exposure, ~80% F1 in lab studies, and it degrades with assistive tech and IME.                                                                                                                                                                                                                                           |
| Entity-graph clustering / coordinated-inauthentic-behaviour detection           | SQLite pool of 1; cohort edges link strangers; no observed CIB for this app. Deferred with a named trigger (§3).                                                                                                                                                                                                                                                        |
| Buying a bot-management vendor                                                  | Independent measurement: commercial "undetectable" bot services evade DataDome 52.93% and BotD 44.56% of the time.                                                                                                                                                                                                                                                      |
| JA3/JA4/TLS or TCP/IP fingerprinting                                            | The app never sees a ClientHello — Traefik terminates TLS, and JA4 is Enterprise+Bot-Management only at Cloudflare.                                                                                                                                                                                                                                                     |
| Logpush as an enforcement pipeline                                              | 30 s–minutes of batching with no backfill. Dashboards only.                                                                                                                                                                                                                                                                                                             |
| Edge attestation (signed `X-Eq-Edge-*` headers)                                 | No consumer exists; `cf-connecting-ip` is already mandatory and peer-gated. Revisit only with a named field that needs it (§3).                                                                                                                                                                                                                                         |
| Admin web UI                                                                    | Repo convention is API-only admin surfaces; a TUI plus JSON endpoints.                                                                                                                                                                                                                                                                                                  |
| Cross-site tracking, ad-style fingerprinting, consent-banner collection         | Purpose limitation. This is a security control, not a tracking product.                                                                                                                                                                                                                                                                                                 |

---

## 2. Current state (measured)

Everything below was read from the code at `53d3d421`. Numbers marked **M** are measured in this audit; **S** are sourced externally; **E** are estimates and must be labelled as such wherever repeated.

**2.1 Enforcement does not exist.**

| Fact                                                                                                                                              | Evidence                                                                                                                              |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `user_bans` has create/revoke/`get_active_ban` implemented and **zero callers**. No account can be banned today.                                  | `src/db/sea_models/user_ban/actions.rs:11,31`; `src/services/auth.rs:530-547`                                                         |
| No admin route creates or revokes a user ban. `/admin/bans` is IP-only rate-limit ban management.                                                 | `src/modules/admin_bans_v1/mod.rs:17-33`                                                                                              |
| The escalation engine is **default-OFF** and unconfigured in production, so automated bans issued to date are zero by configuration.              | `src/config/settings.rs:349-363`; `docker-compose.yml` (no flag set)                                                                  |
| `audit_logs` has a table, a model, and **zero writers**.                                                                                          | `src/db/sea_models/audit_log/` (mod + model only)                                                                                     |
| The `unverified` guard omits the ban check.                                                                                                       | `src/middlewares/auth_guard.rs:46-55` vs `:31,66,84`                                                                                  |
| A banned user sees "Email or password is incorrect" — `AUTH_006` (403) is classified with the credential errors.                                  | `web/src/lib/auth/auth-copy.ts:80-88,137-139`; `rust/crates/ruxlog-types/src/error.rs:231`                                            |
| The only live report channel is `POST /post/comment/v1/flag/{id}` (verified-only), and deleting a comment cascades its flags — the reports on it. | `src/modules/post_comment_v1/mod.rs:13`; `src/db/sea_models/post_comment/actions.rs:384-387`; `migration/src/m000001_init.rs:304-305` |
| The published contact addresses (`salam@easyquran.app`, `corrections@easyquran.app`) have no MX record; the privacy page has no address at all.   | `web/messages/en.json:152,213`; `web/src/lib/config/site.ts:10`; DNS: `easyquran.app` MX empty                                        |

**2.2 The identity every control depends on is forgeable.**

| Fact                                                                                                                                                                               | Evidence                                                                                                       |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `cf-connecting-ip` is believed from any peer inside `TRUSTED_PROXY_CIDRS`; the default is all of RFC1918 plus `fd00::/8`.                                                          | `rust/backend/api/src/middlewares/client_ip.rs:64-74,113-131`; `docker-compose.yml:143`                        |
| Nothing sets, overwrites, or strips that header. Traefik forwards unknown headers; the api has no strip layer.                                                                     | `docker-compose.yml:181-192`; `src/main.rs:827-858`                                                            |
| The api container publishes no host port but is reachable at `api:8888` by any container on the shared `dokploy-network`; a Cloudflare-range firewall rule cannot close that path. | `docker-compose.yml:113-193,168-170`                                                                           |
| The only documented mitigation is an owner-run manual step, unverified.                                                                                                            | `deploy/README.md:377-409`                                                                                     |
| The internal SSR token is checked **before** the peer gate and has no peer check.                                                                                                  | `client_ip.rs:92-106`                                                                                          |
| Every per-IP limit, IP ban, and Turnstile `remoteip` consumes that value.                                                                                                          | `src/middlewares/rate_limit.rs:57-79`; `src/services/escalation.rs:199-284`; `src/services/turnstile.rs:58-73` |

**2.3 The login limiter is a lockout weapon.**

`login:<email>` counts **every** attempt including successes and is never cleared (`src/modules/auth_v1/controller.rs:133-140`; `crates/rux-request-gate/src/store.rs:305,322`). Three attempts in six minutes block the account for an hour; the long tier (24 h) needs roughly eight timed requests including retries at block expiry (**M**: five one-shot requests reach only the one-hour tier). There is no operator unblock path for that key class.

**2.4 The reader path has an availability risk with no limiter at the web origin.**

| Fact                                                                                                                                                               | Evidence                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| Every translated page renders through one global internal bucket, key `ratelimit:internal-webssr:quran-v1`, 600/60 s.                                              | `src/middlewares/rate_limit.rs:68-79`; `src/config/settings.rs:462`           |
| The web container has no request limiter of any kind.                                                                                                              | `web/server.ts` (no limiter); grep clean                                      |
| Any cookie on a request makes `/t/**` uncacheable (`Cache-Control: private, no-store`), and the 7-day disk cache is skipped. The session cookie is `Path=/` today. | `web/src/hooks.server.ts:346-351,427`; `rust/backend/api/src/main.rs:781-788` |
| The offline pack is 14,158,643 bytes raw / 589,573 bytes brotli, 1,635 entries, unauthenticated, and not edge-cached.                                              | `web/scripts/gen-offline-pack.ts:169-196`; `web/server.ts:508-522` (**M**)    |
| `/quran/v1/sources` fans out 380 HEAD requests in production (382 non-prod), serialized by the memo mutex, when the artifact set is not fully verified.            | `src/modules/quran_v1/controller.rs:893-938` (**M**)                          |

**2.5 Other measured facts the plan depends on.**

| Fact                           | Value                                                                                                                                                                                                                               | Evidence                                                                                          |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| DB                             | SQLite, pool of **1** connection, WAL, `synchronous=FULL`, migrations run at boot before the listener binds                                                                                                                         | `src/db/sea_connect.rs:35,46-56,87-93`; `src/main.rs:168,939`                                     |
| SQLite migrations              | **not** wrapped in a transaction; the version row is written after `up()` returns, so a crash mid-migration re-runs it                                                                                                              | `sea-orm-migration-1.1.20/src/migrator.rs:261-272,386-397`                                        |
| Password work                  | global semaphore capacity **2**, fail-fast 429 on saturation, dummy verification burns a slot                                                                                                                                       | `src/services/password_work.rs:8-15,56-58`; `src/services/auth.rs:86-89,115,136`                  |
| Argon2                         | m=19456, t=2, p=1; verify p50 11.6 ms on the audit machine (**M**, not the VPS)                                                                                                                                                     | `password-auth-1.0.0:65,86`                                                                       |
| Process-local state            | fresh-auth grants, pending logins, passkey/OAuth challenge maps, revocation set, rate-limiter L1, escalation history — all in-process; the app is **structurally single-instance**                                                  | `src/services/fresh_auth.rs:60`; `pending_login.rs:44`; `oauth/state.rs:22-23`; `main.rs:181-209` |
| Production flags (probed live) | `WEB_AUTH_ENABLED=true` (register returns 400), seeder routes 404 (not compiled), FCM off, escalation off, billing tables absent, media mounted but 0 rows and no upload UI                                                         | prod probes 2026-10-12                                                                            |
| Test suites                    | web 190 files / 2,399 tests; Rust 39 integration suites. All pass on HEAD; `clippy -D warnings` fails locally with 16 `double_must_use` under rustc 1.99 vs the CI pin 1.98.1                                                       | (**M**)                                                                                           |
| CI                             | the test/lint/audit gates were **deliberately removed** in `53d3d421`; the pipeline now builds and publishes only. The commit hook runs `vp check`, which applies the root lint block only, so the web rule set is enforced nowhere | `.github/workflows/images.yml`; `vite.config.ts:4-6`; `web/vite.config.ts:31-62`                  |

---

## 3. Scope decisions: adopted, dropped, deferred

**Adopted (with the reason the audit settled it).**

1. **Ingress trust is the first change.** Every IP-keyed control consumes a forgeable value, and IP bans are exported to an out-of-repo edge adapter — a forgeable IP is a ban weapon aimed at third parties.
2. **Account/credential-first defense.** The ban primitive, the login limiter, registration admission, and mail budgets are the abuse-reduction work. The device layer is evidence only.
3. **The device token is Phase 4 and gated.** A server-minted opaque token, HttpOnly cookie scoped `Path=/api`, link rows per account, evidence-only semantics, never a sole ban cause. Ships only when Gate B fires (§6.6).
4. **No component fingerprinting.** Deferred behind Gate B plus a measured base rate. If it ever ships it is a server-recomputed, typed-payload, shadow-mode-first subsystem with a class algebra, not a score.
5. **Ladder with human-before-effect and no permanent ban without a second look.** L4 is a queue with a hard expiry, never a restriction. L5 auto-reverses within 72 h unless a human approves. L6 requires a human before it takes effect.
6. **Retention is enforced by `expires_at` on every new table**, written as INTEGER epoch seconds, swept by a batched job registered next to `scheduler::start_scheduler`.
7. **Notices and appeals ship with the ban path**, not after it.
8. **Measurement before enforcement.** The base rate is currently unmeasured; §4 W3 makes it measurable, and §6.5 states the sentence the plan will publish with the number.

**Dropped, and must not be re-proposed without new evidence.**

Client-authored coherence booleans as scored inputs; a component-derived ban key; `UNIQUE(device_key)`; per-request device evaluation; the WebAssembly timing signature; weekly epoch rotation with a mapping table; edge attestation; random enforcement delay; the two-person review requirement as stated (impossible solo — replaced by auto-expiry plus recorded self-review); the entity-graph panel; the `device_bans` table without a defined effect; the `enforcement_cases` state machine in v1; behavioural similarity as a corroborator; VM/devtools/VPN/incognito penalties; ASN or residential-proxy blocking; published thresholds; any client-side risk score.

**Deferred with a named trigger.**

| Deferred                                         | Trigger to reopen                                                                                    |
| ------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| Device token (Option B)                          | Gate B (§6.6)                                                                                        |
| Component fingerprinting + shadow rule evaluator | Gate B **and** a measured base rate **and** one engineer-day per week committed for rule maintenance |
| `device_bans` enforcement                        | a defined action that a device-scoped row produces, plus the shared-device guards shipped            |
| Entity-graph clustering                          | a named CIB case plus a DB that is not a single SQLite file                                          |
| Edge attestation                                 | a Cloudflare field the ladder actually consumes                                                      |
| Billing                                          | the owner confirms payment is intended (§11 O11)                                                     |

---

## 4. Workstreams

Each workstream: **Build** (what changes, with current-state citations), **Acceptance** (checkable), **Touch** (files). Effort is in dev-days and excludes review.

### Phase 0 — foundations

#### W0. Ingress trust (do this first)

**Build.**

1. Terminate origin traffic with a Cloudflare Tunnel (`cloudflared` service, `TUNNEL_TOKEN`, public hostnames pointing at Traefik) — or, if the tunnel is not acceptable, per-zone Authenticated Origin Pulls with your own certificate (Traefik dynamic `tls.options.clientAuth: RequireAndVerifyClientCert` + `caFiles` + a Cloudflare-range `ipallowlist`). Do **not** rely on the global AOP certificate: it is shared across all Cloudflare accounts.
2. Add a Cloudflare Transform Rule that **sets** `x-eq-ingress-marker` (≥32 CSPRNG bytes) on every origin request. Not in Traefik — a rogue container behind Traefik would otherwise be marked.
3. New `IngressSettings::from_env` in `src/config/settings.rs` following the existing prod-gate pattern (`:191-233,456-495`): in production, refuse to boot unless `INGRESS_TRUST_MODE ∈ {cloudflare_tunnel, authenticated_origin_pull}`, the marker is set and non-placeholder and differs from every other secret, `INGRESS_ALLOWED_PEER_CIDRS` is non-empty and rejects `0.0.0.0/0` / `::/0`, and `IP_SOURCE` is exactly `CfConnectingIp`.
4. In `resolve_client_ip` (`src/middlewares/client_ip.rs:76-149`), before the peer gate: require the marker (constant-time compare, `subtle::ct_eq` as the internal token does at `:92-106`) in production, else 400. After resolution: overwrite `cf-connecting-ip` with the verified value and remove `x-forwarded-for`, `x-real-ip`, `true-client-ip`, `forwarded`, and the marker from the request headers so nothing downstream can be confused.
5. Counters `ingress_marker_missing_total`, `ingress_marker_mismatch_total`, `untrusted_proxy_peer_total`; alert on any non-zero. A scheduled non-Cloudflare canary asserts that a direct request with a forged header is rejected.
6. Web tier: `getClientAddress()` currently returns the Traefik peer (`web/server.ts:640-657`). Any future web limiter must read the marker-gated `cf-connecting-ip`; never `ADDRESS_HEADER=x-forwarded-for`.

**Acceptance.** `tests/ingress_trust.rs` (prod-pinned binary, per `tests/quran_prod_gate.rs:1-8`) proves: a forged `cf-connecting-ip` without the marker is rejected and **no rate-limit bucket key contains the forged value**; a wrong marker is rejected; a correct marker yields exactly one bucket keyed on the real IP; 25 forged floods with escalation enabled leave `ban_status("quran-ban:<forged>/32")` empty; a peer outside the CIDRs is rejected; duplicate `cf-connecting-ip` headers fail closed. Boot-gate tests live beside the existing settings tests using `TEST_ENV_MUTEX` (`src/config/settings.rs:56`).

**Touch.** `src/middlewares/client_ip.rs`, `src/config/settings.rs`, `src/main.rs`, `docker-compose.yml`, `deploy/README.md`, new `tests/ingress_trust.rs`.

#### W1. CI gates and documentation truth (timeboxed: 2 days, web-only fallback)

**Build.** Restore the removed gates as **separate required jobs** and make `publish` depend on them: web `bun run check`, `bun run lint`, `bun run test`, `bun audit`, `bun run quran:check`; Rust `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked -- --test-threads=1`, the seed-system `cargo check`, and `python3 scripts/security/cargo-audit-gate.py` (the script still exists and still works). Note the gates have never been observed green in CI — expect one provisioning pass, and fall back to the web trio only if the Rust jobs cannot be provisioned inside the timebox. Do not let W1 block W2–W4.

Fix the commit hook so the web rule set is actually applied: either move the web rules into the root `lint.overrides`/`fmt.overrides`, or change `staged` to `{"web/**": "cd web && vp check --fix"}` (`vite.config.ts:4-6`; root lint block is `:13-17`). Add `--test-threads=1` to the `justfile` `api-test` target so local runs match CI.

**Acceptance.** A pull request that introduces a nested ternary, an unsorted import, or a failing test fails the pipeline. `git log --oneline -- .github/workflows/images.yml` shows the restore commit; the workflow comments no longer describe gates that do not run.

**Touch.** `.github/workflows/images.yml`, `vite.config.ts`, `web/package.json` (if a script is added), `justfile`, `web/README.md`, `deploy/README.md`, `documentation/history.md:36`, `rust/backend/api/AGENTS.md` (its guard claim is currently false).

#### W2. Interim compliance truth (claims ledger)

**Build.** Ship only statements the code honours, before the retention work exists.

1. Turn analytics and crash reporting **off by default** and gate the boot on consent (`web/src/lib/stores/consent.svelte.ts:16`; `web/src/lib/boot/analytics.ts:20-31`; `crash-reporting.ts:2-6`; `routes/+layout.svelte:98`).
2. Force consent false on settings-document import (`web/src/lib/settings/settings-document.ts:226`), remove the second consent surface on the public site, drop the dead `performance` toggle.
3. Fix the contact addresses to a working, MX-verified mailbox (`privacy@easyquran.fyi`, `abuse@easyquran.fyi`, `legal@easyquran.fyi`, `support@easyquran.fyi`), and add an address to the privacy page.
4. Rewrite the privacy and terms pages as a **claims ledger**: for each sentence, either it is true today or it is removed. The five claims that are false today: analytics is opt-in (it is on), deletion is self-service (it is admin-only, `src/modules/user_v1/mod.rs:29`), administrative reads are logged (`audit_logs` has no writers), backups exist (there is no backup), and data is deleted within 30 days (nothing is swept). Remove or mark each.
5. Add the DSA Art 14 moderation ground and an entity line to the terms.

**Acceptance.** A reviewer can point at every sentence in the published policy and find the code that makes it true, or the sentence is not there. `bun run check` and `bun run lint` pass with the new i18n messages (`web/i18n-namespaces.json` + `web/messages/{en,ar}.json`; new keys need `bun run i18n:namespaces`).

**Touch.** `web/src/lib/stores/consent.svelte.ts`, `web/src/lib/boot/{analytics,crash-reporting}.ts`, `web/src/routes/+layout.svelte`, `web/src/lib/settings/settings-document.ts`, the marketing privacy/terms copy files and their `m/` namespaces, `web/messages/{en,ar}.json`, `web/i18n-namespaces.json`.

#### W3. Measurement

**Build.**

1. Route-level 429s are logged at `debug!` and production runs `RUST_LOG=info` — promote to `warn!` with the identity, path, count, and limit (`crates/rux-request-gate/src/layer.rs:323-330`). One word; the only durable signal the content ceiling currently produces.
2. Add `rate_limit_events(key, key_class, scope, issued_at, expires_at)` written **inside the existing 10 s flush transaction** on state transitions only (`block_until_at` 0→>0, and once on expiry) — never per request. `rate_limit_state` deletes a row as soon as its window and block both expire, so "blocks issued in 90 days" is currently unanswerable (`src/services/rate_limit_store.rs:22-28,190-197`).
3. Add `job_runs(job, started_at, finished_at, outcome, detail)` for every periodic job.
4. Add `GET /api/admin/abuse/v1/summary` (`ROLE_ADMIN`, one statement + in-process counters + file sizes) and `just abuse-report` (WAL-safe copy: `sqlite3 mode=ro ".backup"`, never a raw file copy — WAL makes that torn).
5. Add the ten numbers the plan's gate needs: active account bans; bans created/revoked in 7 days; blocks by key class; 429s by route; signups by day and domain; open comment flags; mail suppression counts; retention sweeper deletions; DB and WAL bytes; disk-cache hit rate.
6. Compose gets a `logging:` block (rotation) so the log window is known, and the operator writes the base-rate sentence (§6.5) with the measurement date.

**Acceptance.** `just abuse-report` prints all ten numbers on a copy of the production DB without touching the live file. A 429 on the content path appears in the log at `warn`. A block created by the limiter produces exactly one `rate_limit_events` row.

**Touch.** `crates/rux-request-gate/src/layer.rs`, `src/services/rate_limit_store.rs`, new `src/services/abuse_report.rs`, new migration, `src/router.rs`, `justfile`, `docker-compose.yml`, new `docs/abuse/2026-Q4.md`.

#### W4. Reader-path cost and availability

**Measure first** (W3 gives the numbers); set every ceiling at ≥4× the measured peak, and ship outside the pre-Ramadan window.

**Build.**

1. Session cookie gets `.with_path("/api")` so the session no longer poisons the reader cache (`src/main.rs:781-788`; tower-sessions default path is `/`). Then a login-only marker cookie (`ruxlog.auth`, set on successful login, cleared on logout) lets `hooks.server.ts:427` distinguish "has a session" from "has any cookie" — and ignore `_ga`, `__cf_bm`, `ruxlog.sid`. The cache predicate must never become "no cookies at all": GA4's `_ga` is default-on and currently makes every returning reader uncacheable.
2. A web-origin limiter in `web/server.ts` for SSR misses (identity = marker-gated `cf-connecting-ip`; W0 must land first), with a global miss budget below the API's 600/min, and health exempt.
3. Split the internal SSR bucket by render class: plumb an `x-eq-render-class: html|md` header from the translation page renderer and add `quran-v1-md` / `quran-v1-range` layers plus a global backstop (`src/middlewares/rate_limit.rs:68-79`).
4. Boot error when `INTERNAL_QURAN_API_BASE`/`INTERNAL_QURAN_API_TOKEN` are missing in production (`web/src/lib/server/quran-translation-page.ts:33-41`) — otherwise SSR silently falls back into the **escalatable external** bucket and one IP can earn a 7-day ban for everyone.
5. Offline pack: per-IP byte budget (20 downloads/day and 100 MB/IP/day), change the client fetch from `cache: "no-store"` to default (`web/src/lib/offline/offline-store.svelte.ts:226`), and add a Cloudflare Cache Rule for `/offline/*`.
6. Canonical 308 redirects for `from`/`to` permutations and sorted `keys`; drop `Vary: Accept` from HTML; add `Cache-Control: no-store` to every error response (`src/error/response.rs:88-120`).
7. Per-artifact verified set for `/quran/v1/sources` and `/scripts` so the 380-HEAD sweep is not repeated per request (`src/modules/quran_v1/controller.rs:893-938`).
8. Publish the bulk-data path: `docs/bulk-data.md`, `/.well-known/quran-bulk.json`, a section in `llms.txt`, `Disallow: /offline/` in robots, and remove the ruxlog residue from the API `robots.txt` (`src/router.rs:236-246`).
9. Observability: wire the collected-but-unread disk-cache stats to `/health/quran`, warn on swallowed SSR failures, and keep `warn!` on 429s.

**Acceptance.** A synthetic reader pass (a real browser on a fresh profile plus a logged-in profile) shows `cf-cache-status: HIT` or a disk-cache hit for `/t/**` in both cases. One IP hammering translated pages cannot push any other IP's renders into the shared internal bucket. The pack download is edge-cached and byte-capped. A 3× peak load test on the reader path passes with no empty shells.

**Touch.** `web/src/hooks.server.ts`, `web/server.ts`, `web/src/lib/server/quran-disk-cache.ts`, `web/src/lib/server/quran-translation-page.ts`, `web/src/lib/offline/offline-store.svelte.ts`, `rust/backend/api/src/main.rs`, `src/middlewares/rate_limit.rs`, `src/modules/quran_v1/controller.rs`, `src/error/response.rs`, `docker-compose.yml`, `deploy/README.md`, `docs/bulk-data.md`, `static/robots.txt`.

### Phase 1 — enforcement and response

#### W5. The ban contract

One release. Nothing here is separable without producing a silent lockout.

**Build.**

1. **Migration** (next free number at implementation time; `next-features.md` reserves `m000012` for reading sessions — verify, do not hard-code). All statements `IF NOT EXISTS`, wrapped in an explicit `conn.transaction` (sea-orm does not wrap SQLite migrations). Contents: a **partial unique index** `CREATE UNIQUE INDEX idx_user_bans_one_active ON user_bans (user_id) WHERE revoked_at IS NULL` (dedupe first, following `m000011`); the `enforcement_notices` table (`id`, `user_id`, `ban_id`, `reason_code`, `reason_text`, `decided_by`, `decided_at`, `effective_at`, `expires_at INTEGER NOT NULL CHECK (expires_at >= decided_at)`, `delivery_state`, `sent_at`, `appeal_token_hash`, `created_at`; no FKs to `users` — the notice must outlive the account); `audit_logs` indices. `down()` drops the index and the table but must not be run while notices exist.
2. **API** `src/modules/admin_user_bans_v1/`: `GET/POST /admin/users/bans`, `POST /admin/users/bans/{id}/revoke`, `POST /admin/users/bans/{id}/notice/resend`. `ROLE_ADMIN`, global CSRF, no export token. Create order inside one transaction: revoke-expired → insert ban → rotate `session_auth_secret` → notice row → audit row → commit → send mail outside the transaction. Duplicate create returns 409 with the existing ban id.
3. **Session kill**: one `UPDATE users SET session_auth_secret = …` in the ban transaction. The extractor already deletes a session whose auth hash no longer matches (`crates/rux-auth/src/session/extractor.rs:53-56`). Do **not** delete `sessions` rows — that needs a binding lookup and N deletes on a pool of 1, and the hash mismatch already kills them. Refuse self-bans and equal-or-higher-role bans so an admin cannot lock themselves out mid-incident.
4. **Enforcement correctness**: add `.not_banned()` to the `unverified` guard (`src/middlewares/auth_guard.rs:53`); clear `login:<email>` in the ban branch instead of hoisting the ban check above the password check (hoisting creates a ban/existence oracle). The 5-minute ban cache stays; UI copy promises no timing.
5. **Banned UX**: a dedicated `banned` branch **before** the credential-status set in `web/src/lib/auth/auth-copy.ts` (removing `AUTH_006` from the set alone is not enough — a generic 403 fallback wins), a banned screen with the reason code and an appeal link, and i18n keys in `web/messages/auth/{en,ar}.json`.
6. **Notice delivery**: an `enforcement_notice` template classified transactional and sent despite a `Complaint` suppression — an enforcement-scoped rule, so verification and reset mail keep today's drop (CAN-SPAM transactional exemption); `NotificationKind::Enforcement` for the in-app copy; row written **before** send with `delivery_state` ∈ `sent | suppressed | no_contact_details | send_failed`; the mail router must return a distinguishable `suppressed` result (see W12); on `Bounce` or `Manual` suppression record `suppressed` and surface it in the admin list. The in-app inbox is not a channel — it is unreachable while banned.
7. **Shared-device rules ship with this release** (0.5 d, they are policy text plus two fields): precedence P1–P6 (account ban > grant > challenge > nothing); a device may challenge, never ban; a device-triggered action caps at L2; the multi-account decision procedure is a stored checklist with closed reason codes; the shared-device check forces a step-down and bars L6. Without these the first ban is a family-ban machine.
8. **OAuth callback admission** (D2 F7): the password door gets Turnstile but the five OAuth modules have none, so an attacker can create 86,400 accounts/day/IP/provider. Fold the OAuth callbacks into the same per-day creation caps in this release.

**Acceptance.** `tests/ban_contract.rs` proves: every guard class rejects a banned user; a requirements-level test proves `not_banned` is consulted; a banned login returns the banned response and **never** a 429; the partial unique index makes a second active ban fail with a 409; `EXPLAIN QUERY PLAN` shows the index is used by `get_active_ban`; the migration is re-runnable and `down()` keeps notices; a notice row survives account deletion; suppression produces the right `delivery_state`. Web: the banned copy renders, and a banned user sees it rather than the credential error.

**Touch.** new `migration/src/m0000NN_ban_contract.rs` (next free number) + `migration/src/lib.rs`, `src/db/sea_models/user_ban/actions.rs`, new `src/db/sea_models/enforcement_notice/*`, `src/db/sea_models/user/actions.rs`, new `src/services/audit.rs`, new `src/modules/admin_user_bans_v1/*`, `src/modules/mod.rs`, `src/router.rs`, `src/modules/auth_v1/controller.rs`, `src/middlewares/auth_guard.rs`, `src/services/mail/{provider,router}.rs`, `src/services/mail/templates/enforcement_notice.html`, `rust/crates/ruxlog-types/src/enums/notification.rs`, the OAuth modules, `web/src/lib/auth/{auth-copy.ts,flows.svelte.ts}`, `web/messages/auth/{en,ar}.json`, new `tests/ban_contract.rs`, new `docs/moderation-sop.md`.

#### W6. Incident response capability

**Build.** The irreducible core is 4–5 days and is the precondition for W16 (a sweeper without holds deletes law-enforcement evidence).

1. **Intake**: four working mail aliases on a MX-verified domain; a `reports` table; `POST /report/v1/submit` (CSRF, 5/h and 20/day per IP); a `/report` page that renders outside the auth `ready` gate; a report control on posts and comments. Capture-at-intake for P0 classes.
2. **Preserve-first**: `case_evidence` (append-only, self-contained, no FK that can null or cascade). A `just incident capture <ref>` command that copies the comment, its flags, the user, sessions, views, media metadata, and an audit row into the case before any removal. Guard `admin_delete` and `flags_clear` so evidence cannot be destroyed by the action that responds to it (today deleting a comment cascades its flags — `m000001_init.rs:304-305`).
3. **Act**: one endpoint that writes ban + notice + appeal token + audit in a single transaction (W5).
4. **Escalate**: `legal_holds` + `legal_hold_subjects` (subject kind, authority, reason, owner, expiry). Retention classes: CSAM 1 year, law-enforcement preservation 90 + 90 days, enforcement records 12 months.
5. **Alert**: P0 intake sends mail and an FCM push to the operator (needs `FCM_ENABLED` and a registered operator device; record the dispatch result, because `Ok` is not delivery); a daily 08:00 digest; an external uptime monitor on a public health endpoint; the 429 `warn!` from W3.
6. **Drill**: `docs/incident-response.md` plus one executed drill with a written log, and a calendar date for the next.
7. Write the audit writer (`src/services/audit.rs`) and call it from every destructive and enforcement action: route block/unblock, comment hide/unhide/delete, flag clear, ban create/revoke, user delete, media delete, newsletter send, suppression add/delete, seed run.

**Acceptance.** A drill report exists: report submitted → case opened → evidence captured → account banned → notice delivered (with a recorded `delivery_state`) → appeal submitted → decision recorded. `audit_logs` has rows for every action in the drill. A test proves that deleting a comment with open flags is refused.

**Touch.** new `src/modules/report_v1/*`, new `src/db/sea_models/{report,case_evidence,legal_hold}/*`, new `src/services/audit.rs`, `src/modules/{post_comment_v1,user_v1,media_v1,admin_route_v1,mail_v1,newsletter_v1}/*`, `src/services/{route_blocker_service,notification}/*`, `justfile`, new `docs/incident-response.md`, new `docs/legal-debt.md`.

#### W7. Content moderation notice and comment hardening

**Build.** Comment hide and delete already happen with no reason, no notice, and no appeal (DSA Art 17 is Section 2 and is **not** exempted for micro/small platforms; only Section 3 is).

1. Wire the same notice/appeal mechanism as W5 to comment hide/delete, with an Art 17-shaped statement: the facts, the ground, whether it was automated, and how to contest.
2. Refuse a comment edit while it is hidden or has ≥3 flags (an author currently edits flagged text and destroys the evidence).
3. Add `flags_cleared_at` and make the flag count only count rows created after it (today clearing flags silently reverts on the next flag).
4. Rate-limit flagging (20/h, 50/day per account) and handle self-flagging.
5. Output sanitization: comment content is a raw `String` with no output sanitizer, and `src/utils/sanitize.rs` cleans only paragraph, list and raw-HTML blocks. No in-repo HTML consumer exists, so sanitize on write or document that consumers must escape.

**Acceptance.** Hiding a comment produces a notice row and an appeal token. An author cannot edit a hidden or heavily-flagged comment. A cleared flag count stays cleared after a new flag.

**Touch.** `src/modules/post_comment_v1/*`, `src/db/sea_models/{post_comment,comment_flag}/*`, `src/utils/sanitize.rs`, migration, web comment components, `web/messages/*`.

#### W8. Admin safety

**Build.** `audit_logs` covers the destructive verbs (W6.7). Then:

1. Comment delete refuses when `flags_count > 0` unless `confirm_evidence` is passed; add the `cleared_at` marker; delete the dead `comment_flag::clear_flags` helper (`src/db/sea_models/comment_flag/actions.rs:179-186`, zero callers).
2. User delete requires a typed email confirmation and writes a pre-state snapshot into `audit_logs` **before** deleting (the cascade currently destroys bans, sessions, and reports — `m000001_init.rs:48-73,283-308`).
3. Category delete requires confirmation (it cascades every post in the category).
4. Media delete: restrict to Admin (Moderator keeps read) — round 8 verified Moderator can delete any media, including system media, with no audit.
5. Add rate-limit layers to the ten unmetered nests: `/feed`, `/category`, `/tag`, `/mail`, `/email_verification`, `/forgot_password`, `/analytics`, `/admin/route`, `/admin/acl`, `/admin/bans` (one line each in `src/router.rs`).
6. Route blocker: refuse to block its own unblock route, and audit every block/unblock.
7. `app_constants` import filter: `R2_ACCESS_KEY_ID` misses every needle in `src/services/acl_service.rs:85-135` (it ends `_ID`) and is stored in plaintext. Add the needle or invert to an allowlist, with a unit test that the known secret env names are refused.
8. Seeder: refuse to run when `is_production()` at the four call sites, drop the `password == email` pattern, and stop minting Admin-role accounts; stop the TUI undo deleting ID ranges that include `users` (`src/services/seed/undo.rs:34-105`). The seeder is not in the shipped image and production routes are 404, so this is a hardening item, not an incident.

**Acceptance.** Every destructive action in the capability matrix has an audit row. Deleting a category requires confirmation. A Moderator cannot delete media. The unmetered-nest test list is empty.

**Touch.** `src/modules/{user_v1,category_v1,media_v1,admin_route_v1}/*`, `src/db/sea_models/{comment_flag,post_comment}/actions.rs`, `src/services/seed/*`, `src/services/acl_service.rs`, `src/tui/*`, `src/router.rs`.

### Phase 2 — abuse reduction

#### W9. Login limiter redesign

**Build.** The counter must count **failures** and must never lock an account.

1. Split the store API: `abuse_peek` (read-only, used before verification) and `abuse_record` (append + evaluate, used after a failed verification). Today `abuse_check` appends on every call including the success path (`crates/rux-request-gate/src/store.rs:305,322`).
2. Do not append on the blocked path (a victim's own retries currently escalate them).
3. Credential-keyed failures produce a progressive delay (1–8 s) plus a session challenge — never a 429, never a lock. IP-keyed failures produce a 429 with `Retry-After` capped at 15 minutes, with no long tier.
4. Clear the credential key on success. The admin ban path also clears it (W5.4).
5. Add an operator-visible "clear limiter for this identity" action to the admin API (the key classes `login:`, `totp:`, `reauth:` have no unblock path today).

**Acceptance.** Three successful logins in a row never block. Three failed logins delay the fourth but do not lock. A blocked identity's retry does not extend its own block. The unblock action removes the block within one flush cycle.

**Touch.** `crates/rux-request-gate/src/{abuse,store}.rs`, `src/services/abuse_limiter.rs`, `src/modules/auth_v1/controller.rs`, the five clone sites (`:299,411,582,813,918`), `src/modules/{email_verification_v1,forgot_password_v1}/*`, `src/modules/admin_bans_v1/*`.

#### W10. Registration admission

**Build.**

1. Per-identity creation caps on both doors: password registration and the five OAuth callbacks. Per-IP soft cap plus a per-day global ceiling; a disposable-domain list; no hard ASN blocking.
2. Fresh-account privilege limits for the first 24 h: one comment, no links, no media.
3. Canonicalize the verification identity: `a.b@gmail.com` and `ab@gmail.com` are distinct rows today but one inbox (`src/utils/email.rs:7-9`; alias logic handles `+` only, and production rejects plus-tags). Strip dots for Gmail-family domains and store a keyed `verification_identity_hash`; cap 3 pending and 5 lifetime accounts per verification identity.
4. Turnstile stays on registration; **never** on login (a missing site key makes the widget unavailable while the secret makes the API fail closed with 503 — that combination kills registration).
5. Deploy smoke test: registration works end-to-end with the production keys, or the deploy aborts.

**Acceptance.** The registration burst test cannot exceed the per-day ceiling. A Gmail dot-alias cannot create more than the lifetime cap. A user with no JS or a blocked Turnstile host can still register via the documented fallback (email verification), or the fallback is documented as unavailable in that region.

**Touch.** `src/modules/auth_v1/*`, the OAuth modules, `src/utils/{email,email_alias}.rs`, `src/services/turnstile.rs`, `src/config/settings.rs`, `deploy/README.md`.

#### W11. Auth availability bundle

**Build.**

1. Password work: replace the fail-fast semaphore with a bounded queue (1.5 s wait, waiter cap), a separate lane for the dummy verification, and a reserved lane for already-authenticated operations. Saturation today returns an immediate 429 and denies login/register/reauth globally at ~2 concurrent operations (`src/services/password_work.rs:8-15,56-58`).
2. Recovery: mask the unknown-email response (`src/modules/forgot_password_v1/controller.rs:343-349` is an explicit account-existence oracle, pinned by its own tests), mount the missing nest limiter (`src/router.rs:81-83`), count failures only on verify, and move the reset grants into the session payload (they are process-local and lost on restart).
3. Email change: verify before switching (or send a code on change and return an explicit marker). Today the change sets `is_verified = false`, rotates the session secret, deletes the verification rows, and sends nothing — a typo locks the user out of every verified route with no code to fix it (`src/db/sea_models/user/actions.rs:142-166`).
4. Challenge maps: refuse at the cap instead of evicting the oldest entry, and add a per-session quota for the passkey and OAuth maps (`src/services/passkey_state.rs:104-112`; `src/services/oauth/state.rs:71-79,142-150`). Move fresh-auth and pending-login state into the session payload.
5. Add failure-only counters on the passkey and OAuth finish paths, and telemetry on password-work saturation.

**Acceptance.** A script that opens 20 parallel logins does not deny a 21st legitimate login beyond the queue wait. The unknown-email response is indistinguishable in status, body, and timing from the known-email response. An email-change typo is recoverable without operator help. Restarting the API does not invalidate an in-flight passkey challenge that has not yet been used.

**Touch.** `src/services/password_work.rs`, `src/services/{auth,passkey_state,pending_login,fresh_auth}.rs`, `src/services/oauth/state.rs`, `src/modules/{auth_v1,forgot_password_v1,passkey_v1,user_v1}/*`, `src/router.rs`.

#### W12. Mail budget, suppression governance, newsletter

**Build.**

1. Global transactional caps by class in `MailRouter::check_rate` (`src/services/mail/router.rs:137-159` currently enforces provider config only when `!transactional`): `mail:send:global:reset` and `mail:send:global:verify` at 500/h each (separate so a verification flood cannot starve password recovery), plus `mail:send:domain:{domain}` at 100/h for all classes.
2. Bounce-rate guard: if the hourly bounce rate exceeds 5%, pause sends for 15 minutes (`mail:guard:paused`). A registration burst to 10k invalid addresses currently produces ~100% hard bounces and can collapse the sending domain's reputation, which locks users out of recovery.
3. Suppression: return a distinguishable `suppressed` result instead of `Ok(default)` (`router.rs:243-250`), fail closed on a DB error with that same result, require a rank and a reason for manual suppression, audit it, and route webhook-sourced suppressions through review before they block transactional mail (a `blocks_transactional` flag, 0 for webhook-sourced rows). A webhook-triggered complaint can currently suppress a third party permanently.
4. Webhook: mount a limiter on `/mail/v1/webhook/{provider}`, document `CLOUDFLARE_EMAIL_WEBHOOK_SECRET`, and make it a production prerequisite (it is empty today, so the webhook rejects everything). Ship the secret together with item 3's review rule: the secret alone arms a chain where any address typed into registration can be suppressed permanently by its owner's spam click.
5. Newsletter: add the missing migration (the table does not exist, and the mounted route 500s), `List-Unsubscribe` + `List-Unsubscribe-Post` headers, a one-click unsubscribe endpoint that always returns 200, an HMAC token hash instead of a plaintext token, and a 30-day/12-month retention class.
6. Per-recipient cooldown: `mail:send:cooldown:{template}:{recipient}` at 60 s using the dedup primitive, not the abuse limiter (the abuse limiter's long tier would become a 24-hour lockout lever).

**Acceptance.** A 10k-address registration burst cannot exceed the global cap and pauses on bounce rate. A suppressed address still receives an enforcement notice when the class allows it, and the notice row records which state applied. One-click unsubscribe works without a login and returns 200 for an unknown token. `grep List-Unsubscribe` is non-empty.

**Touch.** `src/services/mail/{router,provider,cloudflare,smtp}.rs`, `src/modules/{mail_v1,newsletter_v1,email_verification_v1}/*`, new migration for `newsletter_subscribers`, `src/router.rs`, `.env.example`, `web` newsletter routes (the confirm link currently targets a route that does not exist).

#### W13. Sync and UGC bounds (reachable today)

**Build.** Bookmarks are live (`/bookmark` renders for any verified user) and the folder upsert has no per-user cap: 200 mutations/round × 100 rounds/min = 1,200,000 folder rows/hour (**E** ~340 MB/h) and an unbounded snapshot response (~216 MB after an hour, **E**).

1. Caps: 500 folders, 10,000 bookmarks per account, checked on **new ids only** so renames still work at the cap; the response reports `rejected` and `capped`.
2. Clamp client-supplied `updated_at`: values more than 24 h in the future clamp to now; more than a year out are rejected. Today a client can forward-date a row and lock every other device out of it.
3. Snapshot guard: return 507 over the cap; never truncate silently.
4. Per-account sync limiter: `bookmark:sync:user:{id}` at 60/60 s with no long tier (keep the IP layer).
5. Integrity uniques in one migration: `(comment_id, user_id)` on flags, `(post_id, user_id)` on likes, `(post_id)` on access, `(series_id, post_id)`, `series.slug`, plus the missing indices on `post_comments(post_id, hidden, created_at)`, `comment_flags(comment_id)`, `post_views(post_id, created_at)`, `post_revisions(post_id, created_at)`, `posts(status, updated_at)`, `posts(author_id, created_at)`, `posts(category_id)`. Dedupe before creating each unique (the `m000008` pattern).
6. Validate the verse reference range (bookmarks currently accept surah 1–114 × ayah 1–286 = 32,604 combinations including impossible verses).
7. One publish path: either `/schedule` writes `posts.published_at` and the scheduler reads it, or the scheduler reads `scheduled_posts` — today `/schedule` writes a table nothing reads, and a published post can be created through two paths with no review.

**Acceptance.** The amplification test cannot create more than the cap. A forward-dated mutation does not lock a second device. A duplicate like/flag/access row is rejected by the DB, not by application logic. Every new index is used by its query in `EXPLAIN QUERY PLAN`.

**Touch.** `src/modules/bookmark_v1/*`, `src/modules/post_comment_v1/*`, `src/modules/post_v1/*`, `src/db/sea_models/{bookmark,bookmark_folder,post,post_comment,comment_flag,post_like,post_view,post_revision,scheduled_post}/*`, migration, `web/src/lib/bookmarks/*`, `web/src/lib/sync/*`.

#### W14. Media takedown (and gate the media rollout)

**Build.** The pipeline is mounted and one Admin role grant turns it live with no other prerequisite. Deleting an image removes only the original R2 object; the derived variants (`@480w`, `@768w`, `@1024w`, `@1600w`, `@2048w`, `@lqip`) stay public on the bucket's custom domain, and the row delete cascades away the only record of their keys.

1. Delete: read the variant keys first, delete the original and every variant, and only then delete the rows — with retries and a tombstone row (`media_deletions`) written in the same transaction as the row delete.
2. Orphan sweeper: `just media-sweep` (dry-run by default, 24 h grace, cross-checks both tables) — the only way to clean what is already orphaned.
3. Magic-byte validation after the declared-MIME check and before dedup (`src/modules/media_v1/validator.rs:46-88` currently trusts the declared type and extension); reconcile the GIF allowlist with the missing decoder.
4. Strip metadata by default (EXIF/GPS are preserved today) and apply orientation.
5. Bound decode cost: width/height caps plus a decode semaphore, not only the 12 Mpx ceiling.
6. Per-uploader quota and daily cap; indices on `uploader_id` and `content_hash`.
7. Restrict delete to Admin (see W8.4) and add explicit `Cache-Control` on PUT plus a purge step for the bucket's CDN.
8. **Gate**: do not grant the Author role, and do not ship any media UI, until 1–5 are done.

**Acceptance.** Deleting a media item leaves no reachable variant URL (verified by requesting each derived key). The sweeper finds zero orphans after a full cycle on a test bucket. A polyglot upload is rejected. EXIF is absent from the served object.

**Touch.** `src/modules/media_v1/*`, `src/services/image_optimizer.rs`, `src/services/media.rs`, `src/db/sea_models/{media,media_variant}/*`, migration, `src/router.rs`, `justfile`.

#### W15. Billing: degrade the read path and unmount

**Build.** Eight of nine billing tables have no migration, `GET /billing/v1/plans` 500s anonymously (live-confirmed), and the authenticated published-post list 500s because the paywall queries missing tables without an empty-input early return (`src/services/paywall.rs:115-161`; `src/modules/post_v1/controller.rs:122-126,330`).

1. Degrade the paywall read path: an empty purchase list returns "no purchases" instead of querying.
2. Unmount `billing_v1` from the router until the owner confirms payments are intended. Adding the missing migrations instead would activate an open redirect (client-supplied `success_url`/`cancel_url`), no card-testing control, and two providers whose signature verification is wrong (Paddle uses HMAC `h1` over `ts:body`, not Ed25519; Revolut needs the `v1.` prefix — both currently reject every genuine event).
3. When billing is revived: allowlist the redirect URLs, add per-user checkout velocity, add durable webhook dedup with unique constraints, store a per-provider dispute record, and trim the raw webhook JSON now persisted in `metadata`.

**Acceptance.** The authenticated post list returns 200 with no billing tables. No billing route is reachable. `grep -c "billing" src/router.rs` is zero or a single commented line.

**Touch.** `src/router.rs`, `src/services/paywall.rs`, `src/modules/post_v1/controller.rs`.

### Phase 3 — retention and legal

#### W16. Retention execution

**Build.** `expires_at` is meaningless without a job, and a job without holds deletes law-enforcement evidence.

1. Every new table carries `expires_at INTEGER NOT NULL` and a `retention_class`; retention columns are INTEGER epoch seconds — never TEXT, because mixed TEXT/INTEGER comparisons are silently always-false.
2. A batched sweeper (`src/services/device_retention.rs` or `retention.rs`) registered next to `scheduler::start_scheduler`: own task, 6 h interval with 0–300 s jitter, ≤500 rows per statement, 50 ms pause between batches, ≤20 batches per table per run, no `DELETE … LIMIT` (not compiled into this SQLite build), no `PERSISTENCE_LOCK`, non-fatal per table.
3. Every delete carries `NOT EXISTS (SELECT 1 FROM legal_holds …)` and `NOT EXISTS (SELECT 1 FROM user_bans WHERE … AND revoked_at IS NULL)`.
4. Classes: `user_sessions` 90 days (raw UA and IP are stored forever today); `devices` (FCM) 90 days idle; `post_views` 90 days; `rate_limit_events` 180 days; ban + notice + evidence 12 months; CSAM-hold 1 year; law-enforcement preservation 90 + 90 days; erasure receipts 24 months.
5. Ship-gate invariant: the sweeper run must be observable (a `job_runs` row), and a canary row inserted at boot must be deleted by the first run — a 0-row sweep alerts. Then:
6. Backups: nightly `VACUUM INTO` to a dated file plus an off-host encrypted copy, a hard 30-day cap on backup retention, and a documented restore rehearsal. There is no backup today, so every retention claim is currently unbacked.
7. Physical erasure wording: `DELETE` leaves bytes in the freelist and WAL on this build (no `SQLITE_SECURE_DELETE`). Enable `PRAGMA secure_delete=ON` and a scheduled `wal_checkpoint(TRUNCATE)`. Swapping the live file for a `VACUUM INTO` copy needs a maintenance window (it cannot happen under the live connection), so do it only if the physical claim is required. Qualify the published claim as "removed from the live database; removed from backups within 30 days".
8. Erasure receipts: `erasure_receipts` (subject ref, scope, `retained[]` with basis and expiry, timestamp). Boot and post-restore replay re-deletes anything a restore resurrected.

**Acceptance.** A seeded canary row is deleted by the first sweep and a `job_runs` row records it. A row under a legal hold or a live ban is not deleted. A restore from backup followed by boot leaves previously-erased subjects erased. The nightly `VACUUM INTO` backup completes without blocking writes.

**Touch.** new `src/services/retention.rs`, new migration, `src/main.rs`, `src/services/scheduler.rs`, `docker-compose.yml`, `deploy/README.md`, `justfile`.

#### W17. Policy rewrite and legal artifacts

**Build.** The artifacts the plan must produce, with the trigger that makes each necessary:

1. **DPIA** (Art 35): triggered by the combination of special-category-adjacent data (a Quran app's user base) and innovative processing. Owner must be named.
2. **LIA** (Art 6(1)(f)) for security processing: necessity and balancing, plus the per-Member-State ePrivacy analysis for any client-side collection (none in v1, which simplifies this).
3. **ROPA** entry (Art 30) and a retention schedule table matching W16's classes.
4. **Full privacy policy** (replacing W2's claims ledger) once W16 is green, including the device-data paragraph if W18 ships.
5. **Art 17 statement-of-reasons template** and the appeal SOP (W5/W7).
6. **Law-enforcement preservation SOP** (W6) plus a public reporting page.
7. **Minors**: self-declared date of birth → minor flag → no device records, no push, no analytics, no comments; India: no accounts under 18, and no device processing at all (India's DPDP has no legitimate-interest basis and s.9(3) bans tracking under-18s with no security exemption) until the owner decides otherwise (§11 O1).
8. **Backup and custody**: document where the pepper and other secrets live, and what happens on restore or change of control.
9. **Consent**: jurisdiction-aware. No global banner. If any client-side collection ever ships, just-in-time consent at the point of login, with equal-weight decline, and the reader never gated.

**Acceptance.** Each artifact exists with a named owner and a done-when statement. The privacy policy's sentences are all backed by code (the W2 test). The minor flag path is exercised by a test.

**Touch.** `web/src/routes/(marketing)/**`, `web/messages/{en,ar}.json`, `src/db/sea_models/user/*`, `src/modules/user_v1/*`, new `docs/legal/{dpia,lia,ropa,retention-schedule}.md`.

### Phase 4 — gated device identity

#### W18. Device token (evidence only)

**Ship only when Gate B fires (§6.6).** Design is settled by the lifecycle audit; the following are the normative rules.

1. **Mint** inside `create_bound_session` callers only — never on register (no session exists), never on `/csrf/v1/generate` or `/user/v1/get`. 128-bit random token, server-minted, stored as HMAC only.
2. **Transport**: HttpOnly cookie `ruxlog.device`, explicit `Path=/api`, `Secure` gated on `cookie_secure`, `SameSite=Lax`, 30-day `Max-Age`. Never `__Host-` (incompatible with `Path=/api`). Path scoping keeps it off reader routes, so the 7-day disk cache is untouched. The client never reads it; the API sets it.
3. **Bind**: a link row per `(device_record, account)` pair, `UNIQUE` on the pair, cascade from the record, **no FK to `users`**. A second account on the same browser **adds** a link row — it never reassigns an owner column. (The repo's only shipped device token, the FCM push token, does the opposite and reassigns ownership; its tests pin that behaviour. Do not inherit it.)
4. **Use**: evidence and linkage only. It answers "which record", never "who". It is never an authentication factor, never gates a privilege, never causes a ban, and never triggers a challenge on its own. Missing payload or missing token means "unknown", never "new".
5. **Rotate**: no rotation on login (it races and adds nothing). One 1-year cap, in place, with `token_hash` rotated in place. The cookie value is re-set with a fresh `Max-Age` on each login so the client clock tracks the sliding record.
6. **Revoke**: logout and account switch do nothing to the record. "Remove from my account" revokes the caller's link only; "this is not my device" revokes the record for all accounts. A ban never revokes the token — revoking on ban hands the evader a clean slate. A live ban pins the record (excluded from the sweep and from re-mint) with `expires_at = ban expiry + 30 days`.
7. **Delete**: explicit link cleanup on account deletion (there is no FK to lean on); keep the record with `user_id` nulled when a ban or case is live, with the case id recorded; write a receipt. Pepper destruction is a global invalidation, never a per-user erasure — say so in the SOP.
8. **UI**: a Devices section in account settings listing the caller's own links only, with two honest verbs (remove from my account / this is not my device), no fan-out count, no other accounts' information. Wire or delete the dormant `/device/v1/list`.
9. **Guards**: no client-side device lifecycle code (the notifications store tears down on every account change — the device layer must have none); no middleware may name the device cookie or table; `trust_state` may only feed an L2 challenge and must always be self-clearable via the shared-device claim; a rule may not read `trust_state`.
10. **Shared device**: kiosk registration is admin-issued (a one-time HMAC'd enrol code, bound to the device record, 12 months, revocable, bounded quota); the self-service claim clears device reputation for 30 days and never touches `user_bans`.

**Acceptance.** A shared phone with three accounts has one record and three link rows; banning one account leaves the other two working with no prompt. A second account on the same browser does not orphan the first link. Clearing storage produces a new record with zero friction and no challenge. Deleting an account removes only its links. A live ban pins the record through the sweep. The device cookie never appears on a `/t/**` request.

**Touch.** new `src/modules/device_identity_v1/*`, new `src/db/sea_models/{device_record,device_account_link,shared_device_grant}/*`, migration, `src/services/auth.rs`, `src/modules/auth_v1/controller.rs`, `web/src/lib/auth/auth-client.ts` (attach allowlist), `web/src/routes/(account)/account/*`, `web/messages/*`, new `web/src/lib/device/**` (if a collector ever ships).

#### W19. Optional: component fingerprinting

**Do not build unless Gate B fires, the base rate is non-zero, and one engineer-day per week is committed indefinitely.** If it is ever built, the normative constraints are: a closed typed payload (enums and small integers, no free strings, no raw UA/screen/plugins), every predicate recomputed server-side, a class algebra (header-anchored / server-state / request-shape / page-payload) with a per-class cap so one attacker-controlled input can never escalate, a cohort non-actionability guard (a key with ≥4 accounts or ≥6 tokens is never actioned), shadow mode with a promotion gate measured on actioned proposals only, and no canvas/audio/plugins/battery/WASM-timing signals. The full design and its rejected variants are in `docs/research/abuse-prevention/DESIGN-v1.md` and `B4-rules.md`.

---

## 5. Data model and retention

**New tables (all with `expires_at INTEGER NOT NULL` and `retention_class TEXT NOT NULL`).**

| Table                                                            | Purpose                                       | Key constraints                                                         | Retention                                                    |
| ---------------------------------------------------------------- | --------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------ |
| `enforcement_notices`                                            | the written reason, survives account deletion | no FK to `users`; `ban_id`                                              | 12 months                                                    |
| `reports`                                                        | abuse intake from the public form and mail    | no FK to `users` on the reporter                                        | 12 months                                                    |
| `case_evidence`                                                  | append-only, self-contained evidence per case | no FK that can null or cascade                                          | per hold class                                               |
| `legal_holds`, `legal_hold_subjects`                             | stop deletion                                 | subject kind + id copied at hold time                                   | until released                                               |
| `erasure_receipts`                                               | proof of erasure, replayable after restore    | subject ref only, no digest                                             | 24 months                                                    |
| `rate_limit_events`                                              | durable block ledger (transitions only)       | written inside the 10 s flush txn                                       | 180 days                                                     |
| `job_runs`                                                       | every periodic job's outcome                  | —                                                                       | 90 days                                                      |
| `media_deletions`                                                | tombstone + retry list for object deletion    | —                                                                       | 12 months                                                    |
| `device_records`, `device_account_links`, `shared_device_grants` | Phase 4 only                                  | `UNIQUE(token_hash)`; link `UNIQUE(record, user)`; **no** FK to `users` | record 30 days sliding, ban pins it; links follow the record |

**Changed tables.** `user_bans` gains a partial unique index on active rows and an index on `user_id`. `user_sessions`, `devices`, `post_views` gain retention classes and enter the sweeper. `users` gains `is_minor` (self-declared) and a `verification_identity_hash` for alias canonicalization.

**Rules that must hold in every new table.** No `ON DELETE CASCADE` from `users` on anything that carries evidence (a user delete currently destroys bans, sessions, and reports). No mixed timestamp types. No `DELETE … LIMIT`. Every migration idempotent, transactional, and re-runnable, with a tested `down()` that does not destroy notices.

---

## 6. Enforcement rules and gates

**6.1 Ladder.** L0 observe → L1 throttle → L2 challenge → L3 cooldown → L4 **queue only** (no restriction; hard expiry; auto-downgrade if unreviewed) → L5 interim restriction (human-approved, or auto-reversing within 72 h) → L6 ban (human before effect; permanent requires a recorded second look) → L7 legal referral.

**6.2 Deterministic vs scored.** Deterministic (safe to auto-act): CSAM hash match, a prior ban plus credential reuse, identical human-confirmed violating content, a court order. Everything else is scored, versioned, shadow-mode first, and never the sole cause of an action.

**6.3 Thresholds.** Ban ≥ 80, restore ≤ 40, with hysteresis and a fixed grace period — **no randomized delay** (it breaks reproducibility, DSAR answers, and Art 22(3) contestation). Thresholds are never exposed to the client. Scores are never returned to the client.

**6.4 Device and shared-device rules.** Precedence: account ban > grant > challenge > nothing. A device signal may challenge, never ban; device-triggered actions cap at L2. A cohort key (≥4 accounts or ≥6 tokens in 30 days) is never actioned. A shared-device claim clears device reputation and never touches an account ban. The multi-account-vs-evasion procedure is a stored checklist: prior case link → the functional test (the second account can do what the first cannot) → causal test (created after the ban, on a retained identifier) → admissible corroborators (credential reuse, identical violating content, declared link) → mandatory shared-device check → continuity → closed reason codes.

**6.5 Base rate.** The plan publishes this sentence, with the measurement date filled in: _"Measured abuse base rate as of <date>: <n> active account bans, <n> escalation bans, <n> comment flags, <n> mail suppressions, <n> audit rows; production per-IP/ASN request distribution flat. Device identity is deferred; it reopens if any of these counters moves, or if the Gate B condition is observed."_ Until the counters exist, write "not measurable" rather than a number.

**6.6 Gate B (the only gate that authorises Phase 4).** Build the device token only if, over ≥90 days or ≥1,000 signups, the operator has recorded **at least one** ban-evasion case where the returning account shares no credential, no verification identity, no content signature, and no actionable IP/ASN — and the operator can name the case. The corroborator is recorded as an operator-selected enum (`credential_reuse | content_match | ip | none`), never derived from the device token itself (that would be circular). Otherwise record "not measurable", re-run the measurement quarterly, and do not build.

**6.7 Promotion gate for any scored rule.** 14 days of shadow evaluation, ≥1,000 evaluations per rule, a human-labelled sample of ≥100 cases (solo review), false positives ≤5% for a challenge and ≤1% for a throttle-or-ban action, unknown-device rate ≤15%, p99 request overhead ≤+1 ms, and zero request-path DB writes.

---

## 7. Rollout and migration safety

**7.1 Deploy mechanics (verified).** A push to master builds both images, publishes, and triggers a Dokploy compose redeploy; migrations run at boot before the listener binds; a migration failure panics and the container enters a restart loop rather than failing the deploy; rollback is pinning a `VERSION` sha. There is no staging environment and no DB backup step anywhere.

**7.2 Migration rules.** Every migration: all statements `IF NOT EXISTS`, wrapped in an explicit `conn.transaction` (SQLite migrations are not transactional in sea-orm and the version row is written after `up()` returns), re-runnable, with a `down()` that is safe, and tested with up-twice and a full-migrator run. No `ALTER TABLE … ADD COLUMN` with a non-constant default. A `CREATE UNIQUE INDEX` on a populated table dedupes first. Drain the old container before a migration that can take a write lock (`SQLITE_BUSY`).

**7.3 Backup before the first schema change.** Take a WAL-safe copy (`sqlite3 mode=ro ".backup"`, never a raw file copy) and rehearse a restore. This is a hard prerequisite for W5 and W16.

**7.4 Rollback per step.** W0 is revertible by config. W1 is revertible by reverting the workflow. W5's migration is additive; `down()` drops the index and the notice table — never run it while notices exist, and never run it at all on a live ban. W16's sweeper is behind a flag and its deletions are not recoverable except from backup, which is why backups land first. W18's tables are additive and the cookie is inert if the server stops setting it.

**7.5 Abort criteria.** Abort and roll back if: any existing test fails; the ingress canary reports a non-zero forged-header count; the sweeper deletes a row under a hold or a live ban; a notice is sent with `delivery_state = sent` and no mail was actually delivered; the reader path shows empty shells at peak; the authenticated post list still 500s.

---

## 8. Tests

**New Rust integration suites** (`rust/backend/api/tests/`, in-memory SQLite + `Migrator::up` + `tower::ServiceExt::oneshot`, matching the existing pattern; note `src/test_utils` is `#[cfg(test)]`-only and cannot be used from `tests/*.rs`, so each suite builds its own app — add a shared `tests/common/mod.rs` rather than a fifth copy):

`ingress_trust.rs` (W0, prod-pinned binary), `ban_contract.rs` (W5), `incident_capture.rs` (W6), `moderation_notice.rs` (W7), `login_limiter.rs` (W9), `registration_admission.rs` (W10), `auth_availability.rs` (W11), `mail_budget.rs` (W12), `sync_bounds.rs` (W13), `media_takedown.rs` (W14), `billing_degraded.rs` (W15), `retention_sweep.rs` (W16), `device_lifecycle.rs` (W18).

**New web suites** (`web/src/**/__tests__/`, vitest via `vite-plus/test`, happy-dom; browser-only APIs need an injected seam — happy-dom has no canvas, WebGL, audio, UA-CH, or IndexedDB, and `navigator.webdriver` reads true): banned copy and appeal rendering; consent default and boot gating; device-cookie path scoping (assert no cookie write on a reader route and no device read in `hooks.server.ts`); cache-predicate behaviour for `_ga`/`ruxlog.sid`/`ruxlog.auth`; reader href and nav guard extension if any new link appears.

**New machine guards** (the house pattern: a test that fails the build, with its own regex self-tests):

1. No SHA-256 over Quran data — extend the existing guard if any new digest module lands near Quran paths.
2. No device data in the disk-cache key, and no cookie written by a device module.
3. No reader route may carry a device payload or a device cookie.
4. No rule may read `trust_state` except an L2 challenge.
5. Every new table must declare `expires_at` and `retention_class` (a source scan over the migration directory).
6. No `ON DELETE CASCADE` from `users` on evidence tables (a source scan).
7. No `DELETE … LIMIT` in the codebase.
8. The audit writer is called from every destructive action (a source scan over the capability list).
9. The migration's index presence and the partial-unique behaviour.
10. The route-count guard for any new admin nest (there is a precedent that derives the expected count from the source).

**Existing tests that must be updated** (name them in the PR): the `ErrorCode` snapshot (`rust/crates/ruxlog-types/src/error.rs:436-507` asserts 55 rows and a total mapping), the session-payload tests if `AuthSessionState` gains a field (add `#[serde(default)]` and append-only ordering, and test a legacy payload — a missing field currently decodes to anonymous or 500s every live session), the CORS route-table test, the ban-export tests, `web/src/lib/auth/__tests__/auth-client.test.ts` (it asserts fetch call counts and indices), and the middleware-order mirror in `tests/quran_v1.rs` `full_app()`.

---

## 9. Manual verification matrix

| #   | Check                                           | How                                                         | Expected                                                                          |
| --- | ----------------------------------------------- | ----------------------------------------------------------- | --------------------------------------------------------------------------------- |
| 1   | Forged header cannot influence limits or bans   | run the W0 canary from a non-Cloudflare IP                  | 400, and no bucket key contains the forged IP                                     |
| 2   | Origin is not reachable except through the edge | scan the origin IP from outside                             | no 443/80 response                                                                |
| 3   | A banned user sees the ban                      | ban a test account, attempt login                           | banned screen with the reason and appeal link, never the credential error         |
| 4   | Sessions die on ban                             | with a live session, ban the account, make a request        | 401 on the next request                                                           |
| 5   | Notice is really delivered                      | ban with a real mailbox                                     | mail received; `delivery_state = sent`; a suppressed address records `suppressed` |
| 6   | Appeal works without a session                  | open the appeal URL in a private window                     | page renders, submission accepted, 200 for an unknown token                       |
| 7   | Reader cache survives a logged-in reader        | load a translated page logged in                            | `cf-cache-status: HIT` or a disk-cache hit; no `private, no-store`                |
| 8   | One IP cannot starve the translated reader      | hammer translated pages from one IP, then load from another | the second IP renders normally                                                    |
| 9   | Pack is edge-cached and capped                  | download the pack twice; exceed the cap                     | cache HIT; 429/403 at the cap                                                     |
| 10  | Registration cannot burst                       | run the burst script                                        | capped at the configured ceiling; Turnstile enforced                              |
| 11  | Login limiter cannot lock a real user           | three successful logins, then three failures                | no lock; delay on the fourth failure; recovery works                              |
| 12  | Unknown email is indistinguishable              | request a reset for a known and an unknown address          | identical status, body, and timing                                                |
| 13  | Email-change typo is recoverable                | change the email, attempt recovery                          | a code arrives and the change completes                                           |
| 14  | Deleted media leaves nothing public             | delete an item, request every derived variant key           | 404 for all                                                                       |
| 15  | Comment evidence survives moderation            | flag a comment, hide it, inspect the case                   | evidence present; notice row present                                              |
| 16  | Retention actually deletes                      | seed a canary, wait for the sweep                           | canary deleted; `job_runs` row present; held rows survive                         |
| 17  | Restore does not resurrect erased data          | restore a backup, boot, re-run the sweep                    | erased subjects absent                                                            |
| 18  | Billing is unmounted                            | request the billing routes                                  | 404, and the authenticated post list is 200                                       |
| 19  | Audit trail is real                             | perform every destructive action in the matrix              | an `audit_logs` row per action                                                    |
| 20  | Shared device is not punished                   | three accounts on one browser, ban one                      | the other two work with no prompt                                                 |

---

## 10. Docs to amend in the same change

| Doc                                                                                                                     | Reason                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `docs/abuse-prevention.md` (this file)                                                                                  | the plan                                                                                         |
| `docs/README.md`                                                                                                        | index entry                                                                                      |
| `documentation/quran-system.md`                                                                                         | Part 4 (ingress, rate limits, bans) — Part 1 only if reader delivery changes, which it must not  |
| `documentation/history.md`                                                                                              | the record: add each landed control; line 36 currently claims CI runs the suites, which is false |
| `docs/next-features.md`                                                                                                 | boundary reference only — EC-19, §9.5, R2-08, P10, P09 own their areas                           |
| `docs/remaining/copy-corrections.md`, `docs/remaining/feature-gap-catalogue.md`                                         | P10 gains deletion evidence; copy corrections superseded by the policy rewrite                   |
| privacy and terms copy (`web/src/routes/(marketing)/**`, their `m/` namespaces, `web/messages/{en,ar}.json`)            | W2 then W17                                                                                      |
| `web/README.md`, `deploy/README.md`                                                                                     | env vars, ingress, backup runbook, the export feed's IP-only rule                                |
| `AGENTS.MD`                                                                                                             | only if a new hard rule is added (device/reader separation)                                      |
| `rust/backend/api/AGENTS.md`                                                                                            | its guard claim is false today                                                                   |
| `rust/backend/api/migration/README.md`                                                                                  | migration idempotency and numbering                                                              |
| `.github/workflows/images.yml`                                                                                          | W1                                                                                               |
| `web/src/env.ts`, `web/server.ts`, `server-headers.test.ts`                                                             | only if a new `PUBLIC_*` var is added                                                            |
| new `docs/bulk-data.md`, `docs/incident-response.md`, `docs/moderation-sop.md`, `docs/legal/*`, `docs/abuse/YYYY-QN.md` | the artifacts above                                                                              |

**Never touch:** `documentation/my-plan-raw.md` (read-only by rule) and anything under `db/`.

---

## 11. Open decisions for the owner (with recommended defaults)

| #   | Decision                                                                              | Default if the owner does not answer                                                                            |
| --- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| O1  | India: no device processing at all, or consent-based                                  | **No device processing for India** (DPDP has no legitimate-interest basis and s.9(3) has no security exemption) |
| O2  | Whether to restore the CI gates at all (they were removed deliberately in `53d3d421`) | **Restore as separate required jobs**, web-only fallback inside the 2-day timebox                               |
| O3  | Permanent bans with one operator                                                      | **No permanent bans**; longest ban auto-expires and can be renewed with a recorded self-review                  |
| O4  | Shadow-mode schema if Phase 4 ever ships                                              | **B6's two tables**, single 60 s off-path flush                                                                 |
| O5  | Who owns the DPIA/LIA/ROPA, the quarterly measurement, and the drill                  | **The owner**, recorded in `docs/legal/` with dates; if unowned, W17 does not ship                              |
| O6  | Ingress mechanism                                                                     | **Cloudflare Tunnel**; per-zone AOP is the alternative                                                          |
| O7  | Registration ceiling values                                                           | Set from the W3 measurement at ≥4× the observed peak, never below                                               |
| O8  | Whether the web-origin limiter ships before the measurement                           | **No** — measure first, then set ceilings                                                                       |
| O9  | Backup destination                                                                    | Off-host encrypted, 30-day cap, restore rehearsed before W5                                                     |
| O10 | Whether the moderator role is ever granted                                            | **Not yet** — narrow its inline powers (W8.4) before promoting anyone                                           |
| O11 | Billing intent                                                                        | **Unmount until confirmed**; if payments are intended, W15.3 is a prerequisite                                  |
| O12 | India minors and age assurance                                                        | **Self-declared DOB; no accounts under 18 in India; no device processing**                                      |
| O13 | The 30-day key retention versus the 12-month ban record                               | **A live ban pins the record**; the ban row is the retention claim; one rule shared by the SOP and the schedule |
| O14 | Whether a public "bulk data" path is published                                        | **Yes** — it is cheaper than scraping and makes the honest posture explicit                                     |

---

## 12. Sequencing and commit plan

| #   | Workstream                                                | Depends on                               | Effort             | Reversible                            |
| --- | --------------------------------------------------------- | ---------------------------------------- | ------------------ | ------------------------------------- |
| 1   | W0 ingress trust                                          | —                                        | 2–3 d              | yes (config)                          |
| 2   | W1 CI gates (timeboxed)                                   | —                                        | 1.5–2.5 d          | yes                                   |
| 3   | W2 interim compliance truth                               | —                                        | 2–3 d              | yes                                   |
| 4   | W3 measurement                                            | —                                        | 0.5–1 d            | yes                                   |
| 5   | W4 reader-path cost                                       | W0 (for the limiter), W3 (for ceilings)  | 3–4 d              | yes (flags)                           |
| —   | **Gate −1: publish the base rate**                        | W3                                       | —                  | —                                     |
| 6   | W5 ban contract (+ shared-device rules + OAuth admission) | backup (7.3)                             | 7–10.5 d           | additive                              |
| 7   | W6 incident response + audit writer                       | W5                                       | 4–5 d              | yes                                   |
| 8   | W7 moderation notice + comment hardening                  | W5, W6                                   | 2.5–3.5 d          | yes                                   |
| 9   | W8 admin safety                                           | W6                                       | 1–2 d              | yes                                   |
| 10  | W9 login limiter redesign                                 | W3                                       | 2–3 d              | yes                                   |
| 11  | W10 registration admission                                | W9                                       | 2–3 d              | yes                                   |
| 12  | W11 auth availability bundle                              | W9                                       | 3–4 d              | yes                                   |
| 13  | W12 mail budget + suppression + newsletter                | W5 (notice)                              | 3–4 d              | yes                                   |
| 14  | W13 sync/UGC bounds                                       | —                                        | 2–3 d              | yes                                   |
| 15  | W14 media takedown                                        | W6                                       | 2–3 d              | yes                                   |
| 16  | W15 billing degrade + unmount                             | —                                        | 0.5 d              | yes                                   |
| 17  | W16 retention execution + backups                         | W6 (holds)                               | 4–6 d              | deletions irreversible; backups first |
| 18  | W17 policy + legal artifacts                              | W16, W2                                  | 2–3 d + legal time | yes                                   |
| —   | **Gate B: decide Phase 4**                                | ≥90 d of W3 data                         | —                  | —                                     |
| 19  | W18 device token                                          | Gate B                                   | 9–14 d             | additive                              |
| 20  | W19 optional fingerprinting                               | Gate B + base rate + a weekly commitment | 38–61 d            | additive                              |

**Total for v1 (steps 1–18): ≈28–42 dev-days plus 1–2 days of policy work (**E**).** The single workstream that matters most if only one can ship: **W5 with W6** — the enforcement-and-notice contract. If only one _day_ is available: W3's `debug!` → `warn!` plus `just abuse-report`, which ends the blindness.

**First commit.** W0's boot gate and header overwrite, with `tests/ingress_trust.rs`. Nothing else in this plan is safe to build on a forgeable identity.

---

## 13. Gate commands and their enforcement status

| Gate                | Command (from)                                                                                                        | Status today                                                                                          |
| ------------------- | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Web check           | `bun run check` (`web/`)                                                                                              | passes (**M**); not run in CI                                                                         |
| Web lint            | `bun run lint` (`web/`)                                                                                               | passes (**M**); not run in CI; the commit hook does not apply the web rule set                        |
| Web tests           | `bun run test` (`web/`)                                                                                               | 2,399 pass (**M**); not run in CI                                                                     |
| Rust fmt            | `cargo fmt --all --check` (`rust/`)                                                                                   | passes (**M**); not run in CI                                                                         |
| Rust clippy         | `cargo clippy --workspace --all-targets --locked -- -D warnings`                                                      | fails locally with 16 `double_must_use` under rustc 1.99 vs the pin 1.98.1; not run in CI             |
| Rust tests          | `cargo test --workspace --locked -- --test-threads=1` (`rust/`)                                                       | 39 suites pass (**M**); not run in CI; serial execution preserves the global password-admission limit |
| Dependency policy   | `python3 scripts/security/cargo-audit-gate.py`                                                                        | the script works and still displays the unreachable `rsa` finding; not run in CI                      |
| Quran guards        | `tests/quran_v1.rs`, `catalogue-sha-guard.test.ts`                                                                    | pass as part of the suites; not run in CI                                                             |
| Manual Quran audits | `bun run audit:arabic` (`web/`), `bun run verify` (`db/quran/translations`), `just quran-audit` (`rust/backend/api/`) | unchanged; manual only                                                                                |

W1 restores the CI rows. Until it lands, every gate above is a local-only invariant and the plan must say so rather than claim enforcement.

---

## 14. KPIs and falsification

**What this plan is judged on in six months.**

| #   | Criterion              | Threshold                                                                                                                                                                                                        |
| --- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Can respond end to end | one real case: ban → session kill → notice row with a truthful `delivery_state` → appeal → recorded decision; 100% of rows present; zero unwired hide/delete actions                                             |
| 2   | Base rate measured     | 2 of 2 quarterly reports in `docs/abuse/`, each within 10 days of quarter end, with all ten W3 numbers and the Cloudflare per-IP/ASN distribution; the 429 `warn!` verified live; the base-rate sentence updated |
| 3   | No harm                | zero overturned enforcement actions with `corroborator = none`; zero reader 429s for logged-in users; zero empty-shell incidents; zero merges in the last 30 days without the restored CI gates                  |

**Falsification.** The central bet — account/credential first, device identity deferred — is **wrong** if within 90 days there are two or more evasion cases where the returning account shares no credential, no verification identity, no content signature, and no actionable IP/ASN. It is **right** if there are zero such cases, or every evasion is linked by credential/identity/content reuse, and no legitimate user was blocked by the per-credential limits.

**Governance minimum that survives one person.** A staleness row in `app_constants` for the rule set and the base rate, checked at boot with a warning and shown on the admin summary; a quarterly one-page report in `docs/abuse/`; a legal-debt list in `docs/legal-debt.md`; a drill log in `docs/incident-response.md`; and the falsification sentences above copied into `documentation/history.md` so they are seen.

---

## 15. Rejected by evidence (do not re-propose)

| Rejected                                         | Killing evidence                                                                                            |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| Component fingerprint as a ban key               | 18.5% mobile uniqueness (population-specific), 45.52% one-day churn, vendor noising                         |
| `BanUnit::Device` in the existing ban namespace  | breaks the canonical-unit invariant, the admin filter, the export contract consumed by an out-of-repo proxy |
| `UNIQUE(device_key)`                             | makes an attacker-chosen cohort the ban unit                                                                |
| Client-authored coherence booleans               | one line of JavaScript defeats it                                                                           |
| Per-request device evaluation                    | violates the repo's own "no DB write on the request path" rule; pool of 1                                   |
| WASM/JS timing signature                         | 2–4 bits, 50–400 ms on low-end devices, unstable across sessions                                            |
| Weekly epoch rotation + mapping table            | the mapping's stable pseudonym **is** the join rotation exists to break                                     |
| Edge attestation (signed headers)                | no consumer; Cloudflare is the only party that can see the connection                                       |
| Fail-closed on a missing edge header             | a Worker fault would take the site down                                                                     |
| Randomized enforcement delay                     | destroys reproducibility, DSAR answers, and Art 22(3) contestation                                          |
| Two-person permanent bans as stated              | impossible with one operator; replaced by auto-expiry plus a recorded self-review                           |
| Entity-graph clustering in v1                    | pool of 1; cohort edges link strangers; no observed CIB                                                     |
| Behavioural biometrics                           | Art 9 exposure, ~80% F1, accessibility harm                                                                 |
| Bot-vendor purchase                              | measured 52.93% / 44.56% evasion against the best vendors                                                   |
| JA3/JA4/TLS fingerprinting                       | the app never sees a ClientHello; Enterprise-only at Cloudflare                                             |
| Logpush as enforcement                           | 30 s–minutes, no backfill                                                                                   |
| IP reputation / ASN / residential-proxy blocking | four of five requests from residential-proxy IPs are legitimate household traffic                           |
| VPN, VM, devtools, incognito penalties           | base rates 13% VM, ~20% VPN, ~6% devtools, 4.4% tampering                                                   |
| Canvas/audio/plugins/battery signals             | noised by Safari for all browsing; plugins is a fixed stub in 2026; battery removed                         |
| Publishing thresholds                            | turns the engine into a calibration oracle                                                                  |
| Component-based duplicate-content matching       | the Quran text is byte-identical by design; restricted to human-confirmed violating bodies only             |
| `enforcement_cases` state machine in v1          | zero cases exist; the notice row plus the audit row carry the record                                        |
| Reusing `rate_limit_state` for device bans       | invisible to the admin filter, pollutes the IP counters and the capacity caps                               |

---

## 16. Evidence appendix

**16.1 Claims that must not be repeated (corrected during the loop).** `950 req/s` (estimated, from an unsourced 0.5 ms fsync). `18.5% uniqueness` / `45.52% churn` as this audience's numbers (population-specific, attribute-specific). `5 POSTs = 24 h` (needs ~8 timed requests). `2 requests = auth outage` (the semaphore is 2, but the arrival rate matters). `NoOpModerator returns safe:true` (the default is no moderator at all). `no revoke UI` (there is an FCM revoke toggle; the missing thing is a device list). `0 DB ops on the request path` (login writes). `device_sessions CASCADE` (it must not cascade). `migrations are transactional` (they are not). `the privacy policy has a contact address` (it has none). `any cookie kills the cache` (the predicate is any cookie — that part is true — but a `Path=/api` cookie never reaches `/t/**`). `the CSRF cookie is the default state` (it is not minted on a first reader visit; GA4's `_ga` is the default-state cookie). `rotate the token on login`. `new-ULID re-mint`. `device_key is the ban key`. `just quran-audit` is missing (it is a recipe in `rust/backend/api/justfile`; run it from there). `OTLP is two env lines` (no backend is deployed; it is a black hole). `billing works`. `media delete removes content`. `the in-app notice channel works` (no web inbox, and it is unreachable while banned). `FCM alerts deliver` (dispatch returns Ok with no push when FCM is off). `m000012` hard-coded (verify the next free number). `the device token ships in v1`. `CI enforces the gates`. `India: 30 days applies`. `the moderator role is dead`.

**16.2 Numbers the plan may state, with provenance.** **M** (measured in this audit): pack 14,158,643 B / 589,573 B brotli / 1,635 entries; 380 HEADs (prod); 378 translation ids; internal bucket 600/60 s; disk cache 7 d / 256 MB; range cache 512 entries; translation pool 8 entries / 48 MiB / 1,800 s; web 190 files / 2,399 tests; Rust 39 suites; 16 clippy findings; Argon2 m=19456 t=2 p=1 with p50 11.6 ms verify; password work 2; mail ≤29/h/recipient (the 30th attempt trips the bucket); FCM cap 20; `MAX_GRANTS` 10,000; bookmarks 200 mutations/round, 100 rounds/min, 32,604 possible; media 2 MiB / 12 Mpx / 30 per minute; pool of 1 with FULL sync. **S** (sourced, with a population caveat): 18.5% / 33.6% mobile uniqueness; 45.52% one-day fingerprint churn; 32% of browser instances clear cookies in 8 months; 52.93% / 44.56% vendor evasion; 4 of 5 residential-proxy IPs are legitimate; ~$8.85 per coordinated account; StatCounter 90.62% Indian mobile share; Pew/IAMAI shared-device rates; 13% VM, ~20% VPN, ~6% devtools, 4.4% tampering; 18 USC 2258A(h) and 18 USC 2703(f); Paddle HMAC `h1`; Revolut `v1.`. **E** (estimates — label them): 0.5 ms fsync, ~950 req/s ceiling, 60–200 ms flush lock hold, ~216 MB snapshot / ~340 MB per hour bookmark amplification, ~86 GB/day media storage, ~5 min FCM fan-out for 20 devices, all dev-day figures, and any traffic assumption.

**16.3 Report inventory.** `R2-*` (7): ban operations, evasion countermeasures, fingerprint science, commercial products, network/transport fingerprinting, legal/privacy, session binding and RBA. `A1–A10`: device-design threat model, false positives, repo integration, web collisions, privacy/legal, testability, performance/cost, ops realism, crypto/keys, data model. `B1–B10`: token storage, contrarian option comparison, anonymous abuse, detection rules, shared devices, rollout, repo drift, retention execution, second-order attack, research falsification. `C1–C6`: Option A+B audit, contradiction resolution, base-rate measurement, shipped legal debt, decision ledger, plan shape. `D1–D5`: claim verification, v1 attack, measurement attack, hard-rule compliance, final gap sweep. `E1–E6`: incident response, mail abuse, billing, ban contract, fresh-eyes audit, reader-path cost. `F1–F6`: ingress trust, auth DoS, admin surface, observability and kill switches, device-token lifecycle, remaining-gap enumeration. `G1–G4`: media pipeline, sync and UGC bounds, device endpoints and fan-out, pre-mortem. `H1–H3`: exhaustion verdict, reachability verdicts, final consistency sweep.
