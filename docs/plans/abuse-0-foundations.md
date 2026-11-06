# Abuse phase 0: truth, measurement, ingress

[Back to index](../README.md)

#### A1. Ingress trust in code

Build:

1. `rust/backend/api/src/config/settings.rs`: add `IngressSettings::from_env` following the existing prod-gate pattern (see the prod checks near lines 191–233 and 456–495). In production refuse to boot unless all hold:
   - `INGRESS_TRUST_MODE` ∈ {`cloudflare_tunnel`, `authenticated_origin_pull`}.
   - `INGRESS_EDGE_MARKER` is set, at least 32 bytes, not a placeholder, and differs from every other configured secret.
   - `TRUSTED_PROXY_CIDRS` (already required in production, `settings.rs` ~217) also rejects `0.0.0.0/0` and `::/0`. Do not add a second peer list.
   - `IP_SOURCE` is exactly `CfConnectingIp`.
2. `rust/backend/api/src/middlewares/client_ip.rs` `resolve_client_ip` (about lines 76–149). Order:
   a. Before the peer gate: in production require header `x-eq-ingress-marker`. Compare constant-time (`subtle` `ct_eq`, same as the internal token check near lines 92–106). Missing or wrong → 400.
   b. Peer gate: TCP peer must be inside `TRUSTED_PROXY_CIDRS`. Outside → reject, instead of today's fall-through. Health routes stay exempt (`is_health_route` runs first).
   c. Duplicate `cf-connecting-ip` headers → fail closed (400).
   d. After resolution: overwrite `cf-connecting-ip` with the verified value. Remove from the request: `x-forwarded-for`, `x-real-ip`, `true-client-ip`, `forwarded`, and the marker header. Nothing downstream may read the stripped headers.
3. Fix the internal SSR token path: it is checked before the peer gate and has no peer check (`client_ip.rs` ~92–106). Put the internal token behind the same peer gate.
4. Counters: `ingress_marker_missing_total`, `ingress_marker_mismatch_total`, `untrusted_proxy_peer_total`. Expose through the existing metrics/telemetry module.
5. `docker-compose.yml` and `deploy/README.md`: add the env vars above; document the Traefik/Cloudflare path (owner executes the tunnel; see O1).
6. Web tier (for later items): any web limiter must read the marker-gated `cf-connecting-ip`. Never `ADDRESS_HEADER=x-forwarded-for`.
7. Alert on any non-zero ingress counter.

Files: `rust/backend/api/src/middlewares/client_ip.rs`, `rust/backend/api/src/config/settings.rs`, `rust/backend/api/src/main.rs`, `docker-compose.yml`, `deploy/README.md`, new `rust/backend/api/tests/ingress_trust.rs`.

Done when: `tests/ingress_trust.rs` (prod-pinned binary, same pattern as `tests/quran_prod_gate.rs`) proves: forged `cf-connecting-ip` without marker → rejected and no rate-limit bucket key contains the forged value; wrong marker → rejected; correct marker → exactly one bucket keyed on the real IP; 25 forged-IP floods with escalation enabled leave `ban_status("quran-ban:<forged>/32")` empty; peer outside CIDRs → rejected; duplicate `cf-connecting-ip` → fail closed; prod boot refuses each missing/invalid setting (boot-gate tests in `settings.rs` tests using `TEST_ENV_MUTEX`).
Owner: O1 for real deploy.

#### A2. Restore CI gates and fix the commit hook

Build:

1. `.github/workflows/images.yml`: add gate jobs and make `publish` `needs:` them. Existing `quran:check` step in the web build job can be reused.
   - Web (`web/` working dir, bun install --frozen-lockfile): `bun run check`, `bun run lint`, `bun run test`, `bun audit`, `bun run quran:check`.
   - Rust: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo test --workspace --locked -- --test-threads=1`; seed-system `cargo check`; `python3 scripts/security/cargo-audit-gate.py`.
   - Timebox 2 days. If the Rust jobs cannot be provisioned in that time, ship the web jobs only and record the fallback in the commit message. Do not block A3–A6 on this item.
   - Known local failure: `clippy -D warnings` fails with 16 `double_must_use` under rustc 1.99; CI pins 1.98.1. Use the pinned toolchain in CI.
2. Commit hook: root `vite.config.ts` currently runs `staged: {"*": "vp check --fix"}`, which applies only the root lint block. Change to `staged: {"web/**": "cd web && vp check --fix"}`.
3. `justfile` `api-test` target: add `--test-threads=1`.
4. Fix the false claim in `rust/backend/api/AGENTS.md` about guard coverage. Fix the stale comments in `images.yml`.
5. `documentation/history.md:36` claims CI runs the suites. That file is owner-only: add the exact replacement sentence under O21.
6. `DEPLOYMENT.md`: fix the false gates claim (line ~74) and replace pnpm references with bun.

Files: `.github/workflows/images.yml`, `vite.config.ts`, `justfile`, `rust/backend/api/AGENTS.md`, `web/README.md`, `deploy/README.md`, `DEPLOYMENT.md`.

Done when: every gate job is a `needs:` of `publish`, so a nested ternary, an unsorted import, or a failing test stops publication. Owner confirms with a failing PR.

#### A3. Billing: degrade the read path and unmount

Build:

1. `rust/backend/api/src/services/paywall.rs` (~115–161): `user_purchased_post_ids` only skips its query when `post_ids` is empty, then reads `post_purchase`, a table that does not exist. While billing is unmounted, return an empty set without querying. This stops the 500 on the authenticated published post list (`rust/backend/api/src/modules/post_v1/controller.rs` ~122–126 and ~330).
2. `rust/backend/api/src/router.rs`: remove the `billing_v1` import (~line 38) and the `/billing/v1` nest (~lines 163–164). Keep the module code.
3. Do not add the missing billing migrations. Adding them would activate an open redirect (client `success_url`/`cancel_url`), no card-testing control, and two providers with wrong signature verification (Paddle uses HMAC `h1` over `ts:body`; Revolut needs the `v1.` prefix).
4. Revival is out of scope. If the owner revives billing, that needs: allowlisted redirect URLs, per-user checkout velocity, durable webhook dedup with unique constraints, per-provider dispute record, trimmed webhook JSON in `metadata`. Create a new item then.

Files: `rust/backend/api/src/router.rs`, `rust/backend/api/src/services/paywall.rs`, `rust/backend/api/src/modules/post_v1/controller.rs`.

Done when: authenticated published post list returns 200 with no billing tables present; `/billing/v1/*` returns 404; `grep -c billing rust/backend/api/src/router.rs` is 0 or one commented line; `tests/billing_providers.rs` still compiles or is updated to not mount the router.
Owner: O9 (default: unmount).

#### A4. Interim compliance truth: consent default and claims ledger

Build:

1. `web/src/lib/stores/consent.svelte.ts`: `DEFAULT_FLAGS.analytics` is `true` (line ~18). Change to `false`. Also `stored.analytics === false ? false : DEFAULT_FLAGS.analytics` (line ~28) must default to off for new visitors.
2. Gate boot on consent: `web/src/lib/boot/analytics.ts` (~20–31), `web/src/lib/boot/crash-reporting.ts` (~2–6), `web/src/routes/+layout.svelte` (~98). Analytics and crash reporting load only after consent is true.
3. Owner decision (2026-10-14): personal reading insights are always on. Do not gate them behind consent. The policy must say so.
4. `web/src/lib/settings/settings-document.ts` (`applySettingsDoc`, ~226): a settings import must never turn consent on, so force consent flags to false on import. Remove the second consent surface on the public site. Drop the dead `performance` toggle.
5. Contact addresses. Current code uses `.app` addresses in `web/messages/en.json` (line ~156 `corrections@easyquran.app`; line ~217 `salam@easyquran.app`). `web/src/lib/config/site.ts` already uses `easyquran.fyi`. Change message copy only after owner confirms MX (O3). Targets: `privacy@`, `abuse@`, `legal@`, `support@` on `easyquran.fyi`. Add a contact address to the privacy page.
6. Claims ledger: rewrite privacy and terms copy (`legal_privacy_*`, `legal_terms_*`) so each sentence is true today or removed. Check in particular: "anonymous" analytics alongside a device identifier, retention periods, administrative access, backups. Deletion stays "by request" until F7 ships.
7. Add DSA Art 14 moderation ground and an entity line to terms.
8. i18n: new keys go in `web/messages/{en,ar}.json` and `web/i18n-namespaces.json`, then run `bun run i18n:namespaces`.
9. `/contact` mails the owner's personal address (`contactEmail` in `web/src/lib/config/site.ts`). Switch to `abuse@`/`support@` once O3 is done.

Files: `web/src/lib/stores/consent.svelte.ts`, `web/src/lib/boot/analytics.ts`, `web/src/lib/boot/crash-reporting.ts`, `web/src/routes/+layout.svelte`, `web/src/lib/settings/settings-document.ts`, marketing privacy/terms route files under `web/src/routes/(marketing)/**` and their `m/` namespaces, `web/messages/{en,ar}.json`, `web/i18n-namespaces.json`.

Done when: every sentence in published privacy/terms maps to code that makes it true, or is gone (reviewer check); default consent test asserts analytics false for a new visitor; import test asserts consent forced false; `bun run check`, `bun run lint`, `bun run test` green.
Owner: O3 for the new contact addresses only.

#### A5. Measurement: promote 429 logs, durable block ledger, job log, abuse report

Build:

1. 429 log: the route-level 429 path in `rust/backend/api/crates/rux-request-gate/src/layer.rs` (~line 323) logs at `debug!`. Change to `warn!` with identity, path, count, and limit. Line ~288 already uses `warn!` for a different case; do not change it.
2. Migration (next free): `rate_limit_events(id, key, key_class, scope, issued_at INTEGER, expires_at INTEGER NOT NULL, retention_class)`. Write inside the existing 10 s flush transaction, on state transitions only: `block_until_at` 0 → >0, and once when a block expires. Never per request. `rate_limit_state` deletes a row when window and block both expire (`rust/backend/api/src/services/rate_limit_store.rs` ~22–28, ~190–197), so this ledger is the only durable record. Retention 180 days.
3. Migration (same file or next): `job_runs(id, job, started_at INTEGER, finished_at INTEGER, outcome, detail, expires_at INTEGER NOT NULL)`. Write one row for every periodic job (`rust/backend/api/src/services/scheduler.rs` and others). Retention 90 days.
4. `GET /admin/abuse/v1/summary`, public as `/api/admin/abuse/v1/summary` because the proxy strips `/api` (`ROLE_ADMIN`, one SQL statement plus in-process counters plus file sizes). New module, mount in `router.rs`.
5. `justfile` recipe `abuse-report`: WAL-safe copy using `sqlite3 mode=ro ".backup"` (never a raw file copy; WAL makes that torn), then print the ten numbers from the copy.
6. Ten numbers, each printed or "not measurable" (never a made-up number): active account bans; bans created and revoked in last 7 days; blocks by key class; 429s by route; signups by day and domain; open comment flags; mail suppression counts; retention sweeper deletions (0 until A32 ships); DB and WAL bytes; disk-cache hit rate.
7. `docker-compose.yml`: add a `logging:` block with rotation so the log window is known.

Files: `rust/backend/api/crates/rux-request-gate/src/layer.rs`, `rust/backend/api/src/services/rate_limit_store.rs`, new `rust/backend/api/src/services/abuse_report.rs`, new migration, `rust/backend/api/migration/src/lib.rs`, `rust/backend/api/src/router.rs`, `justfile`, `docker-compose.yml`.

Done when: `just abuse-report` prints all ten items on a copy of the DB and does not touch the live file; a 429 on the content path appears in logs at `warn`; a block created by the limiter writes exactly one `rate_limit_events` row (transition only); a periodic job writes a `job_runs` row.
Owner: O2 before any migration on a live DB.

#### A6. Subject-keyed limiters for authenticated write routes

Build:

1. Amplification test for the folder cap: write a test that creates 200 folders and asserts the 201st new-id create is refused and reported in `skipped`. Same for a rename at the cap (renames must still work). Location: near `bookmark_folder/actions.rs` tests or in `tests/bookmarks_v1.rs`.
2. Key classes (new) in `rust/backend/api/src/middlewares/rate_limit.rs`:
   - `session:sync:user:{id}` 60 per 60 s (reading-session sync).
   - `analytics:events:user:{id}` 30 per 60 s (user analytics events).
   - `bookmark:sync:user:{id}` 60 per 60 s (bookmark sync).
     No long tier. A 429 carries `Retry-After`, capped at 900 s.
3. Dual bucket: effective limit = min(IP+route bucket, user+route bucket); both must pass. Set the IP bucket well above the user bucket on these routes (at least 10×), because a mobile carrier can put many users on one IP and IP penalties are closed. Unauthenticated requests keep the IP-only layer. The user id is server-derived from the session. Never client-authored.
4. `rate_limit_layer` gains a verified-user variant. Current route layers: reading-session, bookmark, user-analytics nests each at 100/60 in `rust/backend/api/src/router.rs` (~lines 178–186).
5. Key structure: subject bucket keys contain the user id and never the IP.

Files: `rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/router.rs`, new `rust/backend/api/tests/rate_limit_user.rs`, `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs` (test), `rust/backend/api/tests/bookmarks_v1.rs` (harness reuse).

Done when: `tests/rate_limit_user.rs` proves: a user on two IPs cannot exceed the user bucket; a 429'd user's next request carries `Retry-After`; subject-class bucket keys contain the user id and never an IP. Folder amplification test passes.

#### A7. Daily cross-domain ceiling, sync envelope, snapshot guard, subject_score seam

Build:

1. Daily ceiling: per-user in-memory TTL counter `writes:user:{id}`, 5,000 accepted mutations per day across the three route classes (session sync, analytics, bookmark sync). Exceed → 429 with `Retry-After` to midnight UTC. Write exactly one `rate_limit_events` row per user per day, on the transition only.
2. Envelope: S5's per-row `results` carry `rejected` and `capped`. Add summary counters from them. Verify existing `capped` occurrences in `bookmark_v1/controller.rs` and `reading_session_v1` first and reuse. Web client stores (`web/src/lib/bookmarks/*`, `web/src/lib/sync/*`) must surface `rejected` and `capped`, not treat them as soft skips.
3. Snapshot guard: when a user's snapshot would exceed the live quotas (10,000 bookmarks or 200 folders), return 507. Never truncate silently. After S6, pulls page instead of truncating, so `OPEN_SNAPSHOT_LIMIT` goes away and this guard covers only a full pull's total.
4. `subject_score` seam: optional parameter on the limiter, default 1.0, no consumer. A low score (future use) would lower the user budget floor (×0.5) and raise an operator event; it never blocks. Document in code. Typecheck only.

Files: `rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/db/sea_models/bookmark/*`, `rust/backend/api/src/modules/bookmark_v1/controller.rs`, `rust/backend/api/src/modules/reading_session_v1/controller.rs`, `web/src/lib/bookmarks/*`, `web/src/lib/sync/*`.

Done when: 5,001st mutation in a day → 429; exactly one `rate_limit_events` row per user per day; a capped account gets 507 on snapshot, never silent truncation; client store surfaces `rejected` and `capped` (web test); `subject_score` typechecks with default 1.0.
Blocked by: A5, S6
