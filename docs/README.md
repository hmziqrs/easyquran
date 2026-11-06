# Build queue

The only plan in this repo. One agent implements it top to bottom, one item at a time.

## How to work

1. Take the first item in **Queue** whose `Blocked by` items are all gone from this file. Skip blocked items and come back to them.
2. Build it and its tests. Follow `AGENTS.MD` and load the area docs it names.
3. Get gates green: `bun run check`, `bun run lint`, `bun run test` from `web/`; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` for Rust changes.
4. Delete the finished item from this file in the same commit. One commit per item, on the current branch. Never push.
5. Repeat until only blocked items remain.

`Blocked by` stops an item. `Owner` names a human step for real-world rollout: build anyway, keep the feature behind config or off until that step is done, and never wait for it. Owner steps live in **Owner only**; the owner deletes each one when done. A `Blocked by` naming an `O<n>` clears the same way.

If reality differs from an item, trust the code, adjust, and note it in the commit message. New work found while building goes in as a new item at the right position. Stop and ask only if an item would break a rule in `AGENTS.MD`.

## Standing rules

- Migrations: next free number at build time (`m000015` is the latest today). Every migration: all statements `IF NOT EXISTS`, wrapped in explicit `conn.transaction` (sea-orm does not wrap SQLite migrations), re-runnable, tested with up-twice and a full migrator run. `CREATE UNIQUE INDEX` on a populated table: dedupe first (pattern: `m000008`, `m000011`). No `ALTER TABLE … ADD COLUMN` with a non-constant default.
- Every new table: `expires_at INTEGER NOT NULL` (epoch seconds, never TEXT) and `retention_class TEXT NOT NULL`. No `ON DELETE CASCADE` from `users` on any evidence table. No `DELETE … LIMIT` (not compiled into this SQLite).
- Process: DB pool is 1 connection, WAL, `synchronous=FULL`. App is single instance. Process-local state is acceptable where stated.
- No new DB write on the request path unless an item says so (login already writes).
- Never claim a privacy, legal or security property the code does not enforce. Every user-facing sentence maps to code.
- Product: reduce friction. No dialogs or review screens where a silent safe default with a toast works. shadcn-svelte for UI. Pre-launch, so no legacy shims. Reading heuristics never penalise very fast readers. Notes stay device-local. Personal reading insights are always on.
- State the fix in code and commit messages, not attack recipes. This repo is public.
- Client-minted entity ids are UUID v7 on every client (web, React Native, GPUI). Default folders stay deterministic UUID v5. An id's timestamp sets sort order only: it comes from the device clock, so never trust it for security or use it as `startedAt`.
- Device identity has one purpose: showing a user their own devices and letting them sign those devices out. The device id and key never reach analytics, rate-limit keys, bans, abuse scoring or logs, and never act as an auth factor. Ids are minted per account on each install, so two accounts on one phone share nothing. Every lookup includes `user_id`. No device cookie, and nothing device-related on reader routes or for signed-out readers.
- Enforcement ladder: L0 observe → L1 throttle → L2 challenge → L3 cooldown → L4 queue only (no restriction; hard expiry; auto-downgrade if unreviewed) → L5 interim restriction (human-approved, or auto-reversing within 72 h) → L6 ban (human before effect) → L7 legal referral.
- A19–A22 are one release: nothing separable reaches production alone, or bans become silent lockouts.
- Production Quran bytes, size and path come from baked id maps, never remote metadata. Bulk JSON follows the same rule.
- Keep gstatic, googletagmanager and other CSP origins Firebase Analytics needs. Do not "fix" CSP by removing them.
- Drain the old container before a migration that can take a write lock (`SQLITE_BUSY`).
- Add source-scan guards as the matching items land: no `ON DELETE CASCADE` from `users` on evidence tables, no `DELETE … LIMIT`, route-count guard for new admin nests.
- Shared Rust test harness lives in `tests/` (`src/test_utils` is `cfg(test)`-only; `tests/reading_common/mod.rs` is the pattern). Update existing guards when you change what they pin: `ErrorCode` snapshot (`rust/crates/ruxlog-types/src/error.rs`), CORS route table, ban-export tests, `auth-client.test.ts` fetch counts, the middleware-order mirror `full_app()` in `tests/quran_v1.rs`.
- Rollout abort: any existing test fails; ingress canary reports a forged header; sweeper deletes a row under a hold or live ban; a notice says `sent` with no mail delivered; reader shows empty shells; authenticated post list returns 500.

## Do not build (these decisions are closed; do not re-open in code)

- Component fingerprinting as an identifier or ban key, on any client. Canvas/WebGL/audio/fonts/plugins/battery signals in browsers; IMEI, serial, MAC, `ANDROID_ID`, IDFV or hardware-derived hashes on native. Device identity is a per-install id plus a device-held key (D2, D3).
- Device-based account bans. A device signal may challenge, never ban. Device-triggered actions cap at L2.
- Behavioural biometrics. Entity-graph clustering. Bot-vendor purchase. JA3/JA4/TLS fingerprinting. Edge attestation headers. Logpush as enforcement.
- IP reputation, ASN, residential-proxy, VPN, VM, devtools, or incognito penalties. Hard ASN blocking.
- Randomized enforcement delay. Published thresholds. Any client-side risk score. Scores returned to client.
- Permanent bans with one operator. Longest ban auto-expires and is renewable with a recorded self-review.
- Two-person review as a requirement. Replace with auto-expiry plus recorded self-review.
- `enforcement_cases` state machine. Use `enforcement_notices` plus the audit row as the case record.
- `device_bans` table without a defined effect. `BanUnit::Device` in the existing ban namespace. `UNIQUE(device_key)`. Reusing `rate_limit_state` for device bans.
- Per-request device evaluation. Weekly epoch rotation with mapping table. Fail-closed on missing edge header (a Worker fault would take the site down).
- Admin web UI (repo convention: TUI + JSON API).
- Cross-site tracking, ad-style fingerprinting, consent banner collection, global consent banner.
- Logging, storing, or returning raw UA, screen, or plugins data in the device registry or any abuse device layer (abuse device work is behind Gate B).

## Already shipped, do not rebuild

- Folder cap 200 per account (`rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs:33` `LIVE_FOLDER_QUOTA`, new-id-only). Missing only its amplification test (A6).
- Bookmark cap 10,000 (`bookmark/actions.rs:13`). Reading-session cap 10,000 (`reading_session.rs:17`).
- Clamp of client `updated_at` to now+60 s (`reading_session_v1/controller.rs clamp_ts`). Verse-reference validation per surah (`bookmark_v1/controller.rs ensure_valid_range`). Range span cap 50. `expectedUserId` ownership guards on all three sync routes.
- Newsletter table: migration `m000015_newsletter_subscribers.rs` exists.
- `user_bans` model (`db/sea_models/user_ban/actions.rs`). `get_active_ban` runs at login (`services/auth.rs`); create and revoke have no production callers.
- `not_banned()` requirement helper in `middlewares/auth_guard.rs`; used by `verified`, `verified_with_role`, `authenticated` guards.
- Mail: `lookup_suppressed`, `rcpt_transactional` cap, `suppressed` metric (`services/mail/router.rs`). Mail webhook route mounted at `/mail/v1/webhook/{provider}`.
- `email_suppression` never-downgrade upsert.
- Partial: `utils/telemetry.rs` and `capped` appears in bookmark and reading-session controllers (verify the envelope before reuse; see A7).

## Queue

### Ids

#### D1. UUID v7 for client-minted ids

Reading sessions start offline, so their ids must sort by creation time without the server. They are random UUID v4 today.

Build:

1. `newEntityId()` in `web/src/lib/sync/ids.ts` returns UUID v7 (RFC 9562): 48-bit Unix ms, 12-bit `rand_a` counter that increases within the same ms, random tail. If the clock moves backwards, keep the last ms so ids from one tab never go down. Remove `crypto.randomUUID` and `fallbackUuidV4`. Keep the `getRandomValues` then `Math.random` entropy fallback for insecure origins. No new dependency. This covers threads, bookmarks, folders and insight events. `uuidV5` stays.
2. Reading-session validator requires version 7 and variant `10xx`. Bookmark and analytics validators keep accepting any dashed UUID (default folders are v5). Do not reject an id for its timestamp: a wrong device clock must not lose reading data.
3. One validation failure must not jam the outbox. On store load, give any local thread with a non-v7 id a new v7 id built from its `startedAt`, and drop queued mutations for the old id.
4. Shared test vectors at `contracts/uuid-v7.json` (input ms, counter, random bytes, expected id) so React Native and GPUI prove the same output. Rust clients use the `uuid` crate `v7` feature, added when a Rust client needs it; the server mints no session ids.

Files: `web/src/lib/sync/ids.ts`, `web/src/lib/sessions/store.svelte.ts`, `rust/backend/api/src/modules/reading_session_v1/validator.rs`, new `contracts/uuid-v7.json`.

Done when: ids minted in one ms and across ms sort by creation (test, including backwards clock); vectors pass; server refuses a v4 session id and accepts v7 (test); a stored v4 thread is re-keyed on load and sync recovers (test).

Owner: O30 before deploy.

### Abuse phase 0: truth, measurement, ingress

#### A1. Ingress trust in code

Build:

1. `rust/backend/api/src/config/settings.rs`: add `IngressSettings::from_env` following the existing prod-gate pattern (see the prod checks near lines 191–233 and 456–495). In production refuse to boot unless all hold:
   - `INGRESS_TRUST_MODE` ∈ {`cloudflare_tunnel`, `authenticated_origin_pull`}.
   - `INGRESS_EDGE_MARKER` is set, at least 32 bytes, not a placeholder, and differs from every other configured secret.
   - `INGRESS_ALLOWED_PEER_CIDRS` is non-empty and rejects `0.0.0.0/0` and `::/0`.
   - `IP_SOURCE` is exactly `CfConnectingIp`.
2. `rust/backend/api/src/middlewares/client_ip.rs` `resolve_client_ip` (about lines 76–149). Order:
   a. Before the peer gate: in production require header `x-eq-ingress-marker`. Compare constant-time (`subtle` `ct_eq`, same as the internal token check near lines 92–106). Missing or wrong → 400.
   b. Peer gate: TCP peer must be inside `INGRESS_ALLOWED_PEER_CIDRS`. Outside → reject.
   c. Duplicate `cf-connecting-ip` headers → fail closed (400).
   d. After resolution: overwrite `cf-connecting-ip` with the verified value. Remove from the request: `x-forwarded-for`, `x-real-ip`, `true-client-ip`, `forwarded`, and the marker header. Nothing downstream may read the stripped headers.
3. Fix the internal SSR token path: it is checked before the peer gate and has no peer check (`client_ip.rs` ~92–106). Put the internal token behind the same peer gate.
4. Counters: `ingress_marker_missing_total`, `ingress_marker_mismatch_total`, `untrusted_proxy_peer_total`. Expose through the existing metrics/telemetry module.
5. `docker-compose.yml` and `deploy/README.md`: add the env vars above; document the Traefik/Cloudflare path (owner executes the tunnel; see Owner only O1).
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

1. `rust/backend/api/src/services/paywall.rs` (~115–161): an empty purchase list returns "no purchases" without querying. This stops the 500 on the authenticated published post list (`rust/backend/api/src/modules/post_v1/controller.rs` ~122–126 and ~330).
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
4. `web/src/lib/settings/settings-document.ts` (~226): settings import forces consent flags to false. Remove the second consent surface on the public site. Drop the dead `performance` toggle.
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
2. Migration (next free, m000016): `rate_limit_events(id, key, key_class, scope, issued_at INTEGER, expires_at INTEGER NOT NULL, retention_class)`. Write inside the existing 10 s flush transaction, on state transitions only: `block_until_at` 0 → >0, and once when a block expires. Never per request. `rate_limit_state` deletes a row when window and block both expire (`rust/backend/api/src/services/rate_limit_store.rs` ~22–28, ~190–197), so this ledger is the only durable record. Retention 180 days.
3. Migration (same file or next): `job_runs(id, job, started_at INTEGER, finished_at INTEGER, outcome, detail, expires_at INTEGER NOT NULL)`. Write one row for every periodic job (`rust/backend/api/src/services/scheduler.rs` and others). Retention 90 days.
4. `GET /api/admin/abuse/v1/summary` (`ROLE_ADMIN`, one SQL statement plus in-process counters plus file sizes). New module, mount in `router.rs`.
5. `justfile` recipe `abuse-report`: WAL-safe copy using `sqlite3 mode=ro ".backup"` (never a raw file copy; WAL makes that torn), then print the ten numbers from the copy.
6. Ten numbers, each printed or "not measurable" (never a made-up number): active account bans; bans created and revoked in last 7 days; blocks by key class; 429s by route; signups by day and domain; open comment flags; mail suppression counts; retention sweeper deletions (0 until A32 ships); DB and WAL bytes; disk-cache hit rate.
7. `docker-compose.yml`: add a `logging:` block with rotation so the log window is known.

Files: `rust/backend/api/crates/rux-request-gate/src/layer.rs`, `rust/backend/api/src/services/rate_limit_store.rs`, new `rust/backend/api/src/services/abuse_report.rs`, new migration `m000016_*.rs`, `rust/backend/api/migration/src/lib.rs`, `rust/backend/api/src/router.rs`, `justfile`, `docker-compose.yml`.

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
3. Dual bucket: effective limit = min(IP+route bucket, user+route bucket); both must pass. Unauthenticated requests keep the IP-only layer. The user id is server-derived from the session. Never client-authored.
4. `rate_limit_layer` gains a verified-user variant. Current route layers: reading-session, bookmark, user-analytics nests each at 100/60 in `rust/backend/api/src/router.rs` (~lines 178–186).
5. Key structure: subject bucket keys contain the user id and never the IP.

Files: `rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/router.rs`, new `rust/backend/api/tests/rate_limit_user.rs`, `rust/backend/api/src/db/sea_models/bookmark_folder/actions.rs` (test), `rust/backend/api/tests/bookmarks_v1.rs` (harness reuse).

Done when: `tests/rate_limit_user.rs` proves: a user on two IPs cannot exceed the user bucket; a 429'd user's next request carries `Retry-After`; subject-class bucket keys contain the user id and never an IP. Folder amplification test passes.

#### A7. Daily cross-domain ceiling, sync envelope, snapshot guard, subject_score seam

Build:

1. Daily ceiling: per-user in-memory TTL counter `writes:user:{id}`, 5,000 accepted mutations per day across the three route classes (session sync, analytics, bookmark sync). Exceed → 429 with `Retry-After` to midnight UTC. Write exactly one `rate_limit_events` row per user per day, on the transition only.
2. Envelope: sync endpoints return `{applied, skipped}`. Add `rejected` and `capped` counters. Verify existing `capped` occurrences in `bookmark_v1/controller.rs` and `reading_session_v1` first and reuse. Web client stores (`web/src/lib/bookmarks/*`, `web/src/lib/sync/*`) must surface `rejected` and `capped`, not treat them as soft skips.
3. Snapshot guard: when a user's snapshot would exceed the live quotas (10,000 bookmarks or 200 folders), return 507. Never truncate silently. Reading-session snapshot already caps open threads at 20 (`OPEN_SNAPSHOT_LIMIT`); keep that.
4. `subject_score` seam: optional parameter on the limiter, default 1.0, no consumer. A low score (future use) would lower the user budget floor (×0.5) and raise an operator event; it never blocks. Document in code. Typecheck only.

Files: `rust/backend/api/src/middlewares/rate_limit.rs`, `rust/backend/api/src/db/sea_models/bookmark/*`, `rust/backend/api/src/modules/bookmark_v1/controller.rs`, `rust/backend/api/src/modules/reading_session_v1/controller.rs`, `web/src/lib/bookmarks/*`, `web/src/lib/sync/*`.

Done when: 5,001st mutation in a day → 429; exactly one `rate_limit_events` row per user per day; a capped account gets 507 on snapshot, never silent truncation; client store surfaces `rejected` and `capped` (web test); `subject_score` typechecks with default 1.0.
Blocked by: A5

### Abuse phase 1: reader path and login (measured first)

#### A8. Login limiter split: failures only, no lock

Build:

1. Split the store API in `rust/backend/api/crates/rux-request-gate/src/abuse.rs` and `store.rs`: `abuse_peek` (read-only, before verification) and `abuse_record` (append and evaluate, after a failed verification). Today `abuse_check` appends on every call including success.
2. Do not append on the blocked path. A blocked user's retries must not extend their own block.
3. Credential-keyed failures (`login:<email>`, `totp:`, `reauth:`): progressive delay 1–8 s plus a session challenge. Never a 429, never a lock.
4. IP-keyed failures: 429 with `Retry-After` capped at 900 s. No long tier.
5. Clear the credential key on success.
6. Admin action: clear the limiter for an identity (classes `login:`, `totp:`, `reauth:`). Add to the admin API. Block removed within one flush cycle (10 s).
7. Update the clone sites in `rust/backend/api/src/modules/auth_v1/controller.rs` (search `abuse_limiter::limiter`; around lines 135, 299, 411, 443, 582, 813, 918). Also `email_verification_v1` and `forgot_password_v1`. Current config: `ABUSE_LIMITER_CONFIG` at controller line ~31.

Files: `rust/backend/api/crates/rux-request-gate/src/{abuse,store}.rs`, `rust/backend/api/src/services/abuse_limiter.rs`, `rust/backend/api/src/modules/auth_v1/controller.rs`, `rust/backend/api/src/modules/{email_verification_v1,forgot_password_v1,admin_bans_v1}/*`.

Done when: three successful logins in a row never block; three failures delay the fourth but do not lock; a blocked identity's retry does not extend the block; the admin unblock removes the block within one flush cycle; tests in `tests/login_limiter.rs`.
Blocked by: A5

#### A9. Registration admission, password and OAuth doors

Build:

1. Per-identity creation caps on both doors: password register and the five OAuth callbacks (google, facebook, github, discord, apple auth modules). OAuth callbacks currently have no Turnstile and allow about 86,400 accounts per IP per provider per day; the caps close that.
2. Per-IP soft cap plus a per-day global ceiling. Values: owner supplies after A5 measurement (O20), at ≥4× measured peak, never below. Until then, use a configured value and note "not measured" in code.
3. Disposable-domain list (static list in repo). No hard ASN blocking.
4. Turnstile stays on password registration. Never on login. (A missing site key makes the widget unavailable while the secret makes the API fail closed with 503; that combination kills registration.)
5. Fresh-account limits for first 24 h: one comment, no links, no media.
6. Deploy smoke test: registration works end-to-end with prod keys, or the deploy aborts (`deploy/README.md` runbook).
7. CSRF anonymous-session cap: anonymous `/csrf/v1/generate` sessions are capped per IP (the unbounded anonymous session creation is a storage/DoS lever). Value comes from config, set after A5 measurement (O20); test the cap refuses the next session. Check `rust/backend/api/src/modules/csrf_v1/` first: a cap may already exist.

Files: `rust/backend/api/src/modules/auth_v1/*`, `rust/backend/api/src/modules/{google,facebook,github,discord,apple}_auth_v1/*`, `rust/backend/api/src/services/turnstile.rs`, `rust/backend/api/src/config/settings.rs`, `deploy/README.md`.

Done when: a registration burst test cannot exceed the per-day ceiling; OAuth callbacks share the caps; a fresh account cannot post a link or media; a user with a blocked Turnstile host can still register via the documented fallback or the fallback is documented as unavailable in that region.
Blocked by: A8
Owner: O20 for numeric caps.

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
2. Mount the missing nest limiter (`rust/backend/api/src/router.rs` ~81–83).
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

1. `rust/backend/api/src/main.rs` (~781–788, `SessionManagerLayer`): add `.with_path("/api")` to the session cookie (`tower-sessions` default is `/`). Confirm CSRF cookie and any other cookie still work for their routes; fix any path dependency.
2. Login-only marker cookie `ruxlog.auth=1`, `Path=/`: set on successful login, cleared on logout, in `rust/backend/api/src/modules/auth_v1/controller.rs`.
3. `web/src/hooks.server.ts` (~346–351 and ~427): the cache predicate must use `ruxlog.auth` for "has a session". It must ignore `_ga`, `__cf_bm`, `ruxlog.sid`. The predicate must never become "no cookies at all" (GA4 `_ga` is default-on and would make every returning reader uncacheable).
4. Session cookie name stays `ruxlog.sid` (hardcoded; a configurable name widens the cookie-fixing surface).
5. Web matches the marker by name (`AUTH_COOKIE_RE`), not cookie presence. Keep `responseSetsCookie` as the predicate's second half.

Files: `rust/backend/api/src/main.rs`, `rust/backend/api/src/modules/auth_v1/controller.rs`, `web/src/hooks.server.ts`, `web/src/lib/server/quran-disk-cache.ts`.

Done when: a translated page loaded logged-in and logged-out both return a cache HIT or disk-cache hit (test or manual check via `tests/quran_v1.rs` plus web test); no `private, no-store` on `/t/**` for a logged-in reader; a device cookie never appears on `/t/**` (no device cookie yet; guard test in place); web test covers the predicate for `_ga`, `ruxlog.sid`, `ruxlog.auth`.

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
2. Client fetch: change `cache: "no-store"` to default in `web/src/lib/offline/offline-store.svelte.ts` at lines ~185, ~208, ~226 (manifest and pack).
3. Edge caching: Cloudflare Cache Rule for `/offline/*` is owner-only (O14).

Files: `web/server.ts`, `web/src/lib/offline/offline-store.svelte.ts`.

Done when: pack download #21 from one IP in a day returns 429; byte budget enforced (test); client fetch uses default cache mode (test asserting fetch options).
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

1. Publish the bulk path: `deploy/bulk-data.md`, `/.well-known/quran-bulk.json`, a section in `llms.txt`.
2. `static/robots.txt`: `Disallow: /offline/`.
3. Remove ruxlog residue from the API `robots.txt` (`rust/backend/api/src/router.rs` ~236–246).
4. Wire the collected-but-unread disk-cache stats into `/health/quran`.
5. Warn on swallowed SSR failures (completes A15 item 4 if not already done).

Files: new `deploy/bulk-data.md`, `web/static/*` (well-known file and `llms.txt`), `static/robots.txt`, `rust/backend/api/src/router.rs`, `web/src/lib/server/quran-disk-cache.ts`.

Done when: `/.well-known/quran-bulk.json` returns 200; robots disallows `/offline/`; `/health/quran` includes disk-cache hit rate; the disk-cache hit rate appears in `just abuse-report` (A5 item 6).
Blocked by: A5
Owner: O10 (default: publish).

### Abuse phase 2: ban contract and enforcement

#### A19. Ban contract core: migration, admin API, session kill

Build:

1. Migration (next free after A10): explicit transaction; all statements `IF NOT EXISTS`.
   - Partial unique index: `CREATE UNIQUE INDEX idx_user_bans_one_active ON user_bans (user_id) WHERE revoked_at IS NULL`. Dedupe active bans first (pattern `m000011`).
   - Index `user_bans(user_id)`.
   - Table `enforcement_notices`: `id`, `user_id` (INTEGER, no FK to users), `ban_id`, `reason_code`, `reason_text`, `decided_by`, `decided_at INTEGER`, `effective_at INTEGER`, `expires_at INTEGER NOT NULL CHECK (expires_at >= decided_at)`, `delivery_state`, `sent_at`, `appeal_token_hash`, `created_at`, `retention_class`. No FKs to `users`. The notice outlives the account.
   - `audit_logs` indices.
   - `down()` drops the index only and keeps the notices table and its rows.
2. Module `rust/backend/api/src/modules/admin_user_bans_v1/`:
   - `GET /admin/users/bans`, `POST /admin/users/bans`, `POST /admin/users/bans/{id}/revoke`, `POST /admin/users/bans/{id}/notice/resend`.
   - `ROLE_ADMIN`, global CSRF, no export token.
   - Create order in one transaction: revoke expired bans → insert ban → rotate `users.session_auth_secret` → insert notice row (`delivery_state` set later) → insert audit row → commit → send mail outside the transaction.
   - Duplicate active ban → 409 with the existing ban id.
   - Required fields on create: `corroborator` (closed enum: `credential_reuse | content_match | ip | none`) and an `expires_at`. No permanent bans: `expires_at` required. Pick the max length and record it in code.
   - Refuse self-ban. Refuse ban of equal or higher role.
3. Session kill: the single `UPDATE users SET session_auth_secret = …` in the ban transaction. The extractor already rejects sessions whose hash mismatches (`rust/backend/api/crates/rux-auth/src/session/extractor.rs` ~53–56). Do not delete `sessions` rows.
4. Ban check and cache: the existing 5-minute ban cache stays. UI copy makes no timing promise.
5. Renewal: longest ban auto-expires. `POST /admin/users/bans/{id}/renew` extends it only with a recorded self-review (`single_operator_control`).

Files: new migration `m000016+`, `rust/backend/api/src/db/sea_models/user_ban/actions.rs`, new `rust/backend/api/src/db/sea_models/enforcement_notice/*`, `rust/backend/api/src/db/sea_models/user/actions.rs`, new `rust/backend/api/src/modules/admin_user_bans_v1/*`, `rust/backend/api/src/modules/mod.rs`, `rust/backend/api/src/router.rs`.

Done when: `tests/ban_contract.rs` proves: second active ban → 409; `EXPLAIN QUERY PLAN` for `get_active_ban` uses the partial index; migration re-runnable; `down()` keeps notice rows; a notice row survives account deletion; self-ban refused; live session gets 401 on next request after ban.
Owner: O2 before this migration reaches a live DB; O7 (default: no permanent bans).

#### A20. Enforcement correctness and banned UX

Build:

1. `rust/backend/api/src/middlewares/auth_guard.rs`: add `.not_banned()` to the `unverified` guard (~lines 46–55). The `verified`, `verified_with_role`, and `authenticated` guards already call `not_banned()`.
2. Login (`auth_v1/controller.rs`): ban check runs after the password check, not before (hoisting creates a ban/existence oracle). In the ban branch clear the `login:<email>` key. A banned login returns the banned response, never a 429.
3. Web `web/src/lib/auth/auth-copy.ts`: add a `banned` branch before the credential-status set (~lines 80–88 and 137–139). Removing `AUTH_006` from the set alone is not enough: a generic 403 fallback wins. `AUTH_006` is classified at `rust/crates/ruxlog-types/src/error.rs:231`.
4. Banned screen: shows reason code and appeal link. i18n keys in `web/messages/auth/{en,ar}.json`.

Files: `rust/backend/api/src/middlewares/auth_guard.rs`, `rust/backend/api/src/modules/auth_v1/controller.rs`, `rust/crates/ruxlog-types/src/error.rs`, `web/src/lib/auth/{auth-copy.ts,flows.svelte.ts}`, `web/messages/auth/{en,ar}.json`.

Done when: every guard class rejects a banned user (test); a banned login returns the banned response, never a 429 or credential error (test); web test renders the banned copy with reason and appeal link; `bun run check` and `bun run lint` green with new i18n keys.
Blocked by: A19

#### A21. Notice delivery, suppression result, appeal clock

Build:

1. Template `rust/backend/api/src/services/mail/templates/enforcement_notice.html`. Classify as transactional. Enforcement-scoped rule: sends despite `Complaint` suppression. Verification and reset mail keep today's drop. Add `TEMPLATE_ENFORCEMENT_NOTICE` to `is_transactional`.
2. `NotificationKind::Enforcement` for in-app copy (`rust/crates/ruxlog-types/src/enums/notification.rs`). The in-app inbox is not a delivery channel; it is unreachable while banned.
3. Write the notice row before send. `delivery_state` ∈ {`sent`, `suppressed`, `no_contact_details`, `send_failed`}. `sent` only when the provider accepts the message; `Ok` alone is not delivery.
4. Mail router (`rust/backend/api/src/services/mail/router.rs` ~241–252): return a distinguishable `suppressed` result instead of `Ok(default)`. Fail closed on DB error with that same result. Bounce or manual suppression records `suppressed` and shows in the admin list.
5. Appeal clock: decision due within 7 days of appeal. Queue-aging alert when open appeals exceed the clock.
6. `notice_channel = 'email'`. The in-app row is post-unban copy only, never counted as delivered.

Files: `rust/backend/api/src/services/mail/{router,provider}.rs`, `rust/backend/api/src/services/mail/templates/enforcement_notice.html`, `rust/crates/ruxlog-types/src/enums/notification.rs`, `rust/backend/api/src/modules/admin_user_bans_v1/*`.

Done when: `tests/ban_contract.rs` proves each `delivery_state` value; a suppressed address produces a row with `suppressed`; verification mail still dropped for a complained address (test); appeal alert fires in test.
Blocked by: A19
Owner: O3 for the real delivery check.

#### A35. Appeal intake

Build:

1. `POST /appeal/v1/submit`: public, CSRF, rate-limited, uniform response with no case oracle.
2. `/appeal/<token>` web page outside the auth `ready` gate. `X-Robots-Tag: noindex`, `Referrer-Policy: no-referrer`, no third-party scripts.
3. `appeals` table. Token ≥128 bits, stored hashed (keyed HMAC).

Files: new `rust/backend/api/src/modules/appeal_v1/`, new migration, `rust/backend/api/src/router.rs`, new `web/src/routes/appeal/[token]/`.

Done when: a notice's token opens the page and submits an appeal; unknown and known tokens get an identical response; page headers tested; the A21 appeal clock reads from this table.

Blocked by: A21

#### A22. Shared-device precedence and ban-decision fields

Build:

1. Precedence: account ban > grant > challenge > nothing.
2. A device may challenge, never ban. A device-triggered action caps at L2.
3. Multi-account decision: stored checklist in new `deploy/moderation-sop.md` (reference: `docs/research/abuse-prevention/R2-ban-operations.md`): prior case link → functional test (second account can do what first cannot) → causal test (created after ban, on retained identifier) → admissible corroborators (credential reuse, identical violating content, declared link) → mandatory shared-device check → continuity → closed reason codes.
4. Shared-device check forces a step-down and bars L6.
5. Two fields on the ban decision: `corroborator` (already required by A19) and `shared_device_checked` (boolean, required on create).

Files: `rust/backend/api/src/modules/admin_user_bans_v1/*`, new `deploy/moderation-sop.md`, migration (only if `shared_device_checked` is a new column; add in A19 migration if not yet shipped).

Done when: ban create without `shared_device_checked` → 400; SOP checklist committed; precedence rule tested (device signal alone cannot create a ban).
Blocked by: A19

#### A23. Audit writer and call sites

Build:

1. `rust/backend/api/src/services/audit.rs` (new): one writer function; writes `audit_logs` rows. Model exists at `rust/backend/api/src/db/sea_models/audit_log/` (no writers today).
2. Call it from every destructive or enforcement action: route block and unblock; comment hide, unhide, delete; flag clear; ban create and revoke; user delete; media delete; newsletter send; suppression add and delete; seed run.
3. Source-scan guard test: the capability list above is the list of call sites (house pattern: a test that fails the build, with regex self-tests).

Files: new `rust/backend/api/src/services/audit.rs`, `rust/backend/api/src/db/sea_models/audit_log/*`, `rust/backend/api/src/modules/{post_comment_v1,user_v1,media_v1,admin_route_v1,mail_v1,newsletter_v1,admin_user_bans_v1}/*`, `rust/backend/api/src/services/{route_blocker_service,notification}/*`, new `rust/backend/api/tests/audit_writer.rs`.

Done when: every action listed writes one `audit_logs` row (test per action); source-scan guard green.
Blocked by: A19

#### A24. Incident intake: reports

Build:

1. Table `reports` (no FK to users on reporter). Expiry columns per retention rules; class 12 months.
2. `POST /report/v1/submit`: CSRF; 5 per hour and 20 per day per IP. New module `rust/backend/api/src/modules/report_v1/`.
3. `/report` page in web, rendered outside the auth `ready` gate.
4. Report control on posts and comments (web components).
5. Capture-at-intake for P0 classes (CSAM, credible threat): store the reported item snapshot at intake.
6. Mail aliases: four working aliases on an MX-verified domain are an owner step (O3). Code reads them from settings.

Files: new `rust/backend/api/src/modules/report_v1/*`, new `rust/backend/api/src/db/sea_models/report/*`, migration, `rust/backend/api/src/router.rs`, web `web/src/routes/report/**` (new), comment and post components.

Done when: submitting a report creates a row and an audit row; rate limit enforced (test); `/report` renders without an account; P0 capture writes snapshot (test).
Blocked by: A23
Owner: O3.

#### A25. Case evidence, legal holds, incident capture, delete guard

Build:

1. `case_evidence`: append-only, self-contained, no FK that can null or cascade. Columns include `case_ref`, snapshot JSON, `captured_at INTEGER`, `retention_class`, `expires_at INTEGER NOT NULL`.
2. `legal_holds` and `legal_hold_subjects`: subject kind and id copied at hold time; `authority`, `reason`, `owner`, `expires_at`. Retention classes: CSAM hold 1 year, never auto-released; law-enforcement preservation 90 + 90 days (`preserve_until`, renewable); enforcement records 12 months.
3. `just incident capture <ref>`: copies the comment, its flags, the user, sessions, views, media metadata, and an audit row into the case, before any removal.
4. Comment delete with open flags is refused unless `confirm_evidence` is passed (A27 item 1 or here; implement here since evidence needs it). Also `flags_clear` must not destroy evidence.

Files: new migration, new `rust/backend/api/src/db/sea_models/{case_evidence,legal_hold}/*`, `justfile`, `rust/backend/api/src/modules/post_comment_v1/*`, `rust/backend/api/src/db/sea_models/comment_flag/actions.rs`.

Done when: deleting a comment with open flags is refused without `confirm_evidence` (test); `just incident capture` writes a case row with all listed objects (test or run); legal holds table enforced by sweeper later (A32).
Blocked by: A23

#### A26. Alerting, runbook, drill

Build:

1. P0 intake alert: mail and FCM push to operator. Requires `FCM_ENABLED` and a registered operator device (O12). Record the dispatch result. `Ok` from the FCM call is not delivery.
2. Daily 08:00 digest of open reports and appeals.
3. External uptime monitor on a public health endpoint (owner executes: O19).
4. Docs: new `deploy/incident-response.md` (runbook plus drill log section), new `deploy/legal-debt.md` (legal-debt list with trigger conditions).
5. The drill itself is owner-executed (O13).

Files: `rust/backend/api/src/modules/report_v1/`, `rust/backend/api/src/services/notification/`, new `deploy/incident-response.md`, new `deploy/legal-debt.md`.

Done when: a P0 report triggers the mail and push path in a test env with dispatch result recorded; runbook and legal-debt list committed. Drill acceptance is owner's.
Blocked by: A24, A25
Owner: O12, O13, O19.

#### A27. Moderation notice and comment hardening

Build:

1. Comment hide and delete produce an enforcement notice and appeal token, reusing A19/A21. Notice is Art 17-shaped: facts, ground, whether automated, how to contest.
2. Refuse comment edit while hidden or with ≥3 flags. Author cannot edit flagged text.
3. Add `flags_cleared_at`. Flag count counts only rows created after it. Clearing flags stays cleared after a new flag.
4. Flag rate limit: 20 per hour and 50 per day per account. Refuse self-flagging.
5. Sanitization: comment content is a raw `String`. Choose sanitize-on-write using `rust/backend/api/src/utils/sanitize.rs` (ammonia), extended to all block types, or document that consumers must escape. Record choice in the commit message. The current sanitizer skips header, quote, embed, table blocks.

Files: `rust/backend/api/src/modules/post_comment_v1/*`, `rust/backend/api/src/db/sea_models/{post_comment,comment_flag}/*`, `rust/backend/api/src/utils/sanitize.rs`, migration, web comment components, `web/messages/*`.

Done when: hiding a comment creates a notice row and an appeal token (test); author edit refused on hidden or flagged comment (test); cleared flag count stays cleared after a new flag (test); self-flag refused; rate limit test.
Blocked by: A21, A25

#### A28. Admin safety

Build:

1. Comment delete refuses when `flags_count > 0` unless `confirm_evidence` is passed. Delete the dead helper `comment_flag::clear_flags` (`rust/backend/api/src/db/sea_models/comment_flag/actions.rs` ~179–186; zero callers).
2. User delete requires a typed email confirmation. Writes a pre-state snapshot into `audit_logs` before deleting (the cascade currently destroys bans, sessions, and reports; see `m000001_init.rs` ~48–73 and ~283–308).
3. Category delete requires confirmation (cascades every post in the category).
4. Media delete: restrict to Admin. Moderator keeps read. Current media routes use `verified_with_role::<ROLE_AUTHOR>` (`rust/backend/api/src/modules/media_v1/mod.rs` ~20). Verify each delete route.
5. Add rate-limit layers to the unmetered nests, one line each in `rust/backend/api/src/router.rs`: `/feed/v1` (~135), `/category/v1` (~128), `/tag/v1` (~129), `/mail/v1` (~130), `/email_verification/v1` (~84), `/forgot_password/v1` (~85), `/analytics/v1` (~146), `/admin/route/v1` (~148), `/admin/acl/v1` (~150), `/admin/bans` (verify its mount). Use the existing `rate_limit_layer` pattern.
6. Route blocker: refuse to block its own unblock route. Audit every block and unblock (A23).
7. `app_constants` import filter: `R2_ACCESS_KEY_ID` slips past the denylist in `rust/backend/api/src/services/acl_service.rs` (~85–135) because it ends `_ID`, and is stored in plaintext. Add the `_ID` needle, or invert to an allowlist. Unit test: known secret env names are refused.
8. Seeder: refuse to run when `is_production()` at the four call sites. Drop the `password == email` pattern. Stop minting Admin-role accounts. Stop the TUI undo deleting ID ranges that include `users` (`rust/backend/api/src/services/seed/undo.rs` ~34–105).
9. Narrow moderator inline powers: `is_moderator()` must not grant reading any user's email, paywall bypass, hidden-post comments, attaching any media, or seeing all drafts.
10. Persist the route-blocker runtime pause (`route_blocker_config::is_paused` is in-memory, a redeploy reverts it): store in `app_constants`, rehydrate at boot.

Files: `rust/backend/api/src/modules/{user_v1,category_v1,media_v1,admin_route_v1}/*`, `rust/backend/api/src/db/sea_models/{comment_flag,post_comment}/actions.rs`, `rust/backend/api/src/services/seed/*`, `rust/backend/api/src/services/acl_service.rs`, `rust/backend/api/src/tui/*`, `rust/backend/api/src/router.rs`.

Done when: every destructive action in the capability list has an audit row; category delete without confirmation refused (test); a Moderator cannot delete media (test); unmetered-nest test list is empty (guard test); `R2_ACCESS_KEY_ID` refused by filter (test); seeder refuses in production (test).
Blocked by: A23
Owner: O8 (default: no moderator role until this lands).

#### A29. Mail budget, bounce guard, suppression governance, unsubscribe

Build:

1. Global transactional caps in `MailRouter::check_rate` (`rust/backend/api/src/services/mail/router.rs` ~137–159; provider cap currently applies only when not transactional):
   - `mail:send:global:reset` 500 per hour.
   - `mail:send:global:verify` 500 per hour (separate keys so verification cannot starve recovery).
   - `mail:send:domain:{domain}` 100 per hour, all classes.
2. Bounce guard: if hourly bounce rate > 5%, pause sends 15 minutes (`mail:guard:paused`).
3. Suppression: fail closed on DB error (same `suppressed` result from A21). Manual suppression requires rank and reason and writes an audit row. Webhook-sourced suppressions get `blocks_transactional = 0` and go through review before they block transactional mail. A webhook-triggered complaint must not permanently suppress a third party.
4. Webhook `/mail/v1/webhook/{provider}` (`rust/backend/api/src/modules/mail_v1/mod.rs` ~26): add a limiter. Document `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` as a production prerequisite. Ship the secret only together with item 3's review rule (the secret alone arms a chain where any address can be suppressed by its owner's spam click). Owner provides the secret (O14).
5. Newsletter: table already exists (`m000015`). Add `List-Unsubscribe` and `List-Unsubscribe-Post` headers on newsletter mail. One-click unsubscribe endpoint: no login, always 200 (including unknown token). Store HMAC of token instead of plaintext (`rust/backend/api/src/db/sea_models/newsletter_subscriber/actions.rs` token handling). Retention class 30 days for unsubscribed/pending, 12 months for confirmed. Fix the web confirm link if it targets a route that does not exist.
6. Per-recipient cooldown: `mail:send:cooldown:{template}:{recipient}` at 60 s, using the dedup primitive, not the abuse limiter (the abuse limiter's long tier would become a 24-hour lockout lever).

Files: `rust/backend/api/src/services/mail/{router,provider,cloudflare,smtp}.rs`, `rust/backend/api/src/modules/{mail_v1,newsletter_v1,email_verification_v1}/*`, `rust/backend/api/src/router.rs`, `.env.example`, web newsletter routes.

Done when: a 10,000-address registration burst cannot exceed the global cap and pauses on bounce rate (test); one-click unsubscribe returns 200 for an unknown token (test); `grep -r List-Unsubscribe rust/backend/api/src` non-empty; webhook suppression does not block transactional mail until reviewed (test).
Blocked by: A21, A23
Owner: O14 for the webhook secret.

#### A36. Device token hygiene

Build:

1. Treat every terminal FCM class as invalid: `INVALID_ARGUMENT` (400), `SENDER_ID_MISMATCH` (403), `UNREGISTERED`. Mark `invalid_at`; prune after 2 consecutive failures. `rux-fcm` already recognises `UNREGISTERED`; check what the caller does with it.
2. Validate token shape on device registration. Write a `device.transfer` audit row when a token moves accounts.

Files: `rust/backend/api/crates/rux-fcm/src/client.rs`, `rust/backend/api/src/modules/device_v1/`, new migration if `invalid_at` is absent.

Done when: each terminal class marks the token and two in a row prune it (test); malformed tokens are refused; transfer writes an audit row.

Blocked by: A23

#### A30. Media takedown: variants, tombstones, sweep

Build:

1. Delete: read the variant keys first (`@480w`, `@768w`, `@1024w`, `@1600w`, `@2048w`, `@lqip`, and any others the optimizer writes). Delete the original and every variant object from R2. Then delete the rows. Write a tombstone row (`media_deletions`) in the same transaction as the row delete. Retries on object-delete failure. Retention 12 months.
2. Variant upload failures are currently logged and ignored (`rust/backend/api/src/modules/media_v1/controller.rs` ~471–486). Make a failed variant upload fail the upload.
3. `just media-sweep`: dry-run by default; 24-hour grace; cross-checks `media` and `media_variant` tables against R2 listing; prints orphans; deletes only with explicit flag.
4. `Cache-Control` on PUT and a purge step for the bucket's CDN (owner config for the CDN domain).
5. Delete restricted to Admin (A28 item 4).
6. Before trusting media URLs, pin the shape with a unit test: media is `{public_url}/{bucket}/{key}`, Quran has no bucket.

Files: `rust/backend/api/src/modules/media_v1/*`, `rust/backend/api/src/services/media.rs`, `rust/backend/api/src/services/image_optimizer.rs`, `rust/backend/api/src/db/sea_models/{media,media_variant}/*`, new migration (`media_deletions`), `rust/backend/api/src/router.rs`, `justfile`.

Done when: deleting a media item leaves no reachable variant URL (test requests each derived key → 404 on a test bucket); sweeper finds zero orphans after a full cycle (test); tombstone written in same transaction (test).
Blocked by: A23, A28

#### A37. Image moderation and CSAM path

Build:

1. `IMAGE_MODERATION_ENABLED` defaults false (`rust/backend/api/src/main.rs`), and `NoOpModerator` is dead. In production, refuse to boot with media upload enabled and no moderator.
2. Dedup runs after moderation, never before.
3. Hash-match interface for known CSAM, behind a trait, no vendor bound.
4. Triage path in `deploy/incident-response.md`: remove, preserve (A25 hold), report to NCMEC, freeze the account. Human-confirmed violating content only; never Quran text.

Files: `rust/backend/api/src/main.rs`, `rust/backend/api/src/services/image_moderation/mod.rs`, `rust/backend/api/src/modules/media_v1/controller.rs`, `deploy/incident-response.md`.

Done when: prod boot refuses the unsafe combination (test); a rejected image never enters dedup (test).

Blocked by: A26, A30
Owner: O29 for the hash-match provider and NCMEC registration.

#### A31. Media upload hardening and gate

Build:

1. Magic-byte validation after the declared-MIME check and before dedup (`rust/backend/api/src/modules/media_v1/validator.rs` ~46–88 trusts declared type and extension today). Reconcile the GIF allowlist with the missing decoder (drop GIF or add decoder; record choice).
2. Strip metadata by default (EXIF and GPS are preserved today) and apply orientation before strip.
3. Bound decode cost: width and height caps plus a decode semaphore, in addition to the existing 12 Mpx ceiling.
4. Per-uploader quota and daily cap. Indices on `media.uploader_id` and `media.content_hash` (migration).
5. Gate: do not grant the Author role for media upload and do not ship any media UI until items 1–5 are done. Existing limits: 2 MiB, 12 Mpx, 30 per minute.

Files: `rust/backend/api/src/modules/media_v1/validator.rs`, `rust/backend/api/src/modules/media_v1/controller.rs`, `rust/backend/api/src/services/image_optimizer.rs`, migration.

Done when: a polyglot upload (valid image header with script payload) is rejected (test); EXIF absent from served object (test); decode bomb (over cap) rejected before decode (test); quota enforced (test).
Blocked by: A30

#### A32. Retention sweeper with ship gates

Build:

1. Every new table: `expires_at INTEGER NOT NULL` and `retention_class TEXT NOT NULL`. Source-scan guard over the migration directory (house pattern).
2. `rust/backend/api/src/services/retention.rs` (new). Own task, registered next to `scheduler::start_scheduler` (`rust/backend/api/src/services/scheduler.rs` line ~11) but not on the flush task. 6-hour interval with 0–300 s jitter. ≤500 rows per statement. 50 ms pause between batches. ≤20 batches per table per run. No `DELETE … LIMIT`. Non-fatal per table. No `PERSISTENCE_LOCK`.
3. Every delete adds: `NOT EXISTS (SELECT 1 FROM legal_holds … )` and `NOT EXISTS (SELECT 1 FROM user_bans WHERE … AND revoked_at IS NULL)`.
4. Retention classes:
   - `user_sessions` 90 days (raw UA and IP stored forever today); UA and IP NULLed on revoke.
   - `devices` (FCM) 90 days idle.
   - `post_views` 90 days.
   - `rate_limit_events` 180 days.
   - `job_runs` 90 days.
   - bans 12 months from max(`expires_at`, `revoked_at`, `created_at`); `user_bans.expires_at` stays TEXT (existing table). Notice, evidence, `audit_logs`, hidden comments, `comment_flags`, notifications 12 months.
   - CSAM hold 1 year, never auto-released. Law-enforcement preservation 90 + 90 days.
   - `media_deletions` 12 months.
   - erasure receipts 24 months.
   - reports 12 months.
   - newsletter per A29.
5. Ship gates (all required before deletes are enabled):
   - Each run writes a `job_runs` row.
   - A canary row inserted at boot is deleted by the first run.
   - A run deleting 0 rows while rows past expiry exist raises an alert.
   - Dry-run count is logged and checked before deletes are enabled. Behind a feature flag, default off.
6. Physical deletion wording: `DELETE` leaves bytes in freelist and WAL on this build. Do not claim physical erasure (see A33).
7. `POST /post/v1/track_view/{id}` is a public writer of raw IP and UA: hash or drop both, and rate-limit it.

Files: new `rust/backend/api/src/services/retention.rs`, new migration, `rust/backend/api/src/main.rs`, `rust/backend/api/src/services/scheduler.rs`, `docker-compose.yml` (flag), `deploy/README.md`, `justfile`.

Done when: seeded canary deleted by first sweep and a `job_runs` row written (test); row under legal hold survives (test); row under live ban survives (test); dry-run count logged; flag default off in production config.
Blocked by: A5, A19, A25
Owner: O2 before deletes are enabled.

#### A33. Erasure receipts, physical deletion, backups

Build:

1. `erasure_receipts`: subject ref only (no digest), scope, `retained[]` (basis and expiry per item), timestamp. Retention 24 months.
2. Boot and post-restore replay re-deletes anything a restore resurrected (replay list from receipts).
3. `PRAGMA secure_delete = ON` at connect. Scheduled `wal_checkpoint(TRUNCATE)`.
4. Physical swap via `VACUUM INTO` copy only in a maintenance window. Default: do not implement the swap. Publish the claim as "removed from the live database; removed from backups within 30 days" (O15 decides whether the physical claim is needed).
5. Nightly `VACUUM INTO` to a dated file, plus off-host encrypted copy (O2 provides destination). Hard 30-day cap on backup retention. Restore rehearsal documented in `deploy/README.md`.

Files: new migration, `rust/backend/api/src/services/retention.rs`, `rust/backend/api/src/db/sea_connect.rs` (PRAGMA), `rust/backend/api/src/main.rs`, `deploy/README.md`, `justfile`.

Done when: nightly backup completes without blocking writes (run test or manual with timestamp); restore followed by boot leaves erased subjects absent (test with a fixture backup); `PRAGMA secure_delete` reads back ON (test).
Blocked by: A32
Owner: O2, O15.

#### A38. Erasure and data-request procedure

Build: admin erase path writing an erasure receipt (A33), one-month response clock with queue-aging alert, runbook section in `deploy/legal-debt.md`. Self-service deletion is F7 and writes the same receipt.

Files: `rust/backend/api/src/modules/user_v1/`, `rust/backend/api/src/services/retention.rs`, `deploy/legal-debt.md`.

Done when: an admin erase leaves evidence under hold intact, deletes the rest, and writes a receipt (test); overdue requests alert.

Blocked by: A33

#### A34. Policy text sentence ledger, minors, code side of legal artifacts

Build:

1. Sentence ledger: for each sentence in `web/src/routes/(marketing)/**` privacy and terms, record the code reference that makes it true (table in `deploy/legal-debt.md`, or a comment map). Remove sentences without a reference. After A32 is green, publish the full privacy policy. Abuse device-identity paragraph only if it ships (not in this queue); D2 adds the account-device sentence.
2. Minors: self-declared date of birth → `users.is_minor` flag (migration). For minors: no device records, no push, no analytics, no comments. India: no accounts under 18 and no device processing (default; O6 confirms). Test the minor flag path end to end.
3. No global consent banner. Consent is jurisdiction-aware. If client collection ever ships, just-in-time consent at login, equal-weight decline, reader never gated.
4. Art 17 statement-of-reasons template and appeal SOP text: in `deploy/moderation-sop.md` (A22 creates it).
5. Legal artifacts (DPIA, LIA, ROPA, retention schedule): owner authors and signs (O11). Do not write them here. Code side only: `deploy/legal/` is the owner's.

Files: `web/src/routes/(marketing)/**`, `web/messages/{en,ar}.json`, `rust/backend/api/src/db/sea_models/user/*`, `rust/backend/api/src/modules/user_v1/*`, new migration for `is_minor`, `deploy/legal-debt.md`.

Done when: sentence ledger has no unreferenced sentences (reviewer check); minor flag test blocks device write, push, analytics, and comments for minors; `bun run check`, `bun run lint`, `bun run test` green.
Blocked by: A4, A32
Owner: O11, O6.

### Account UX

#### U1. Account page chrome, primitives, localization

Build: (1) Add `AuthTopBar` to the `(account)` layout exactly as `auth/+layout.svelte` does: `Brand` → `marketingHomeHref(locale)` left, one "Read Quran" exit → `marketingReaderHomeHref(locale)` right, `min-h-11` targets, logical properties only, loading gate inside the shell. (2) Replace hand-rolled session rows and the Passkeys card with `Card`; keep inner markup and the "This session" pill; no new primitives or elevation. (3) Anonymous view uses one CTA model: both actions through the auth modal; if it has no register tab, `returnTo=/account` links for both. (4) New `account` namespace: `web/messages/account/{en,ar}.json`, `account-copy.ts` following `auth-copy.ts`, layout sets `lang`/`dir`, claim `account_` in `web/i18n-namespaces.json`, regenerate the barrel. (5) Heading scale matches `(auth)` pages. No `Nav`/`Footer`, no behaviour or data changes.
Files: `web/src/routes/(account)/+layout.svelte`, `web/src/routes/(account)/account/+page.svelte`, `web/i18n-namespaces.json`, new: `web/messages/account/{en,ar}.json`, new: `web/src/lib/i18n/account-copy.ts`
Done when: bar + exit show on authenticated, anonymous and loading views, and the exit is in prerendered HTML without JS; no inline `border-line-2 bg-bg-2` left; `en` and `ar` fully localized with correct `lang`/`dir`.

### Devices

Account device management for web, React Native and GPUI desktop. Not an abuse signal; Gate B still governs that.

#### D2. Device registry

Build:

1. New `user_devices` table, primary key `(user_id, id)`: `id` (client-minted UUID v7), `display_name`, `platform` (`web|ios|android|macos|windows|linux`), `client` (`web|react_native|gpui`), `form_factor` (`phone|tablet|desktop`, same values as `reading_sessions.device`), `browser` (closed enum, web only), `os_major`, `app_version`, `created_at`, `last_seen_at`, `revoked_at`, `expires_at` (last seen + 90 days), `retention_class`. Closed enums validated on the server. No raw UA, screen, model string or IP.
2. The client mints the id at first sign-in for that account and keeps it across sign-out. Signed-out readers have no device id. Storage per platform is in D3.
3. Nullable `device_id` on `devices` (existing FCM push table) and `user_sessions`. A device owns its push tokens and sessions.
4. `POST /device/v1/register`: upsert, bind the current session and push token, refresh `last_seen_at`. Client calls it after sign-in and at most once per 24 h after that, never per request. Limit 10 per hour per user.
5. `GET /device/v1/list`, `PATCH /device/v1/{id}` (rename, 1–40 characters, rendered as plain text), `DELETE /device/v1/{id}`. Delete revokes the device's sessions through the A19 session kill, deletes its push tokens, sets `revoked_at` and writes an audit row. Registering a revoked id returns 409; the client mints a new id. Existing token routes move to the new shape, no shim.
6. Cap 20 live devices per user. Evict the oldest idle one, never the calling device. This replaces `MAX_DEVICES_PER_USER` eviction by token.
7. Minors get no device row (A34). `register` answers `{registered: false}` and sign-in still works.
8. Erasure: A38 and F7 delete these rows. A32 sweeps them on `expires_at`. Add the account-device sentence to the privacy policy and the A34 ledger.
9. Guard: source scan fails if `device_id` appears in analytics modules, rate-limit keys or ban tables.

Files: new migration, `rust/backend/api/src/modules/device_v1/*`, `rust/backend/api/src/db/sea_models/device/*`, new `rust/backend/api/src/db/sea_models/user_device/`, `rust/backend/api/src/services/auth.rs`, `web/src/lib/firebase/messaging.ts`, new `web/src/lib/device/`.

Done when: API tests cover register, rename, delete, cap and 409; deleting a device kills its session on the next request and removes its push token; the same id under two accounts yields two unrelated rows; a minor gets no row; `/t/**` responses and signed-out requests carry nothing device-related (test).

Blocked by: A19, A23, A34, A36
Owner: O6.

#### D3. Device key: the real device fingerprint

The fingerprint is a key only that install holds, not traits read off the device. It cannot be copied by another device claiming the id, stays stable across browser and OS updates, and works the same on all three clients.

Build:

1. The client creates one P-256 key pair per account per install. The fingerprint is its RFC 7638 JWK thumbprint (SHA-256, base64url).
2. Storage:

   | Client                | Key and id stored in                                                     | `key_store`             | New device after                             |
   | --------------------- | ------------------------------------------------------------------------ | ----------------------- | -------------------------------------------- |
   | Web                   | Non-extractable WebCrypto key in IndexedDB                               | `browser`               | Site data cleared, new browser profile       |
   | React Native, iOS     | Secure Enclave key; id in Keychain, `ThisDeviceOnly`                     | `secure_hardware`       | Restore onto a new phone; reinstall keeps it |
   | React Native, Android | Android Keystore key (StrongBox when present); excluded from Auto Backup | `secure_hardware`       | Reinstall, restore onto a new phone          |
   | GPUI, macOS           | Keychain                                                                 | `os_keystore`           | Keychain item removed                        |
   | GPUI, Windows         | Credential Manager / DPAPI                                               | `os_keystore`           | Windows profile change                       |
   | GPUI, Linux           | Secret Service; else a `0600` file under `$XDG_DATA_HOME`                | `os_keystore` or `file` | Keyring or file removed                      |

3. `POST /device/v1/challenge` returns a 32-byte nonce bound to the session, single use, 5-minute TTL (process-local, A13's no-eviction pattern).
4. `register` sends the public key and a signature over `easyquran-device-v1`, nonce, device id and user id. Server verifies and stores `public_key`, `key_thumbprint`, `key_store`. Reuse the P-256 code `passkey_v1` already pulls in if it fits. A known id with a different key gets 409 and the client mints a new id. Signing again after session expiry updates the same row, so re-sign-in makes no duplicate.
5. `key_store` is a self-reported label for display. App Attest and Play Integrity are out of scope, and the key never unlocks anything (standing rule).

Files: `rust/backend/api/src/modules/device_v1/*`, new migration, `web/src/lib/device/`, new `contracts/devices.md`.

Done when: wrong signature, reused nonce, expired nonce and wrong key all fail (tests); re-sign-in reuses the row; thumbprint vectors in `contracts/devices.md` pass in web and Rust.

Blocked by: D2

#### D4. Devices on the account page

Build: replace the Sessions card on `/account` with Devices: name, platform icon, browser or OS, last seen, "This device". Rename inline. "Sign out" on another device acts at once with a toast, no dialog. On this device it runs normal sign-out and keeps the id. Sessions with no device show as "Unknown device" using the existing fixed-table label. Strings go in the `account` namespace (`en`, `ar`).

Files: `web/src/routes/(account)/account/+page.svelte`, `web/src/routes/(account)/account/account-view.ts`, `web/src/lib/auth/account-client.ts`, `web/messages/account/{en,ar}.json`.

Done when: list, rename and remote sign-out work in both locales; labels never echo unvalidated input (test); `bun run check` stays warning-free.

Blocked by: D2, U1

#### D5. Handoff by device, and two-device sync test

Build:

1. Nullable `device_id` on `reading_sessions`. Sync accepts it. An unknown or revoked id is stored as NULL, never a 422, so threads started offline still land. Signed-out threads get the id when they fold in at sign-in.
2. Snapshot and archive return `deviceId` and `deviceName` from the user's own live row.
3. `ContinueThreads` compares device ids instead of device kinds, so two phones are no longer "the same". Label: "on Pixel · 5 min ago". Falls back to the kind label when the id or row is gone.
4. F6 export leaves out `device_id`.
5. Automated two-device test: device A mints a thread offline, device B writes a newer one, both sync, both see the same order; tombstones and progress MAX converge; a revoked device stops syncing and keeps its outbox until sign-in.

Files: new migration, `rust/backend/api/src/modules/reading_session_v1/*`, `rust/backend/api/src/db/sea_models/reading_session.rs`, `web/src/lib/sessions/{schema.ts,store.svelte.ts}`, `web/src/routes/(application)/yours/_components/ContinueThreads.svelte`, new `rust/backend/api/tests/reading_two_devices.rs`.

Done when: the two-device test passes; a thread from another phone is labelled with that phone's name; removing that device falls back to "on your phone".

Blocked by: D1, D2

#### D6. Native client contract

Build:

1. `contracts/devices.md`: sign-in, cookie storage in the OS secure store, `csrf-token` from `/csrf/v1/generate`, no `Origin` header, register flow, offline id and outbox rules, 401 and 409 handling. Native provider sign-in already exists (`/auth/{provider}/v1/token`, `ensure_native_token_request`).
2. Integration test acting as a native client, no `Origin` header: sign in, fetch CSRF, register with key proof, sync a v7 thread, list devices, delete self, get 401.
3. Check every synced route works without browser-only behaviour. Fix what doesn't.

Files: new `contracts/devices.md`, new `rust/backend/api/tests/native_client.rs`, `rust/backend/api/src/middlewares/*` only if the test exposes a gap.

Done when: the test passes on `cargo test`; a React Native or GPUI developer can implement sign-in, device registration and sync from the contract alone.

Blocked by: D3, D5

### Product

#### F1. Remove dead tafsir placeholder

Build: delete the unreferenced `tafsirFor` sample string.
Files: `web/src/lib/data/quran.ts`
Done when: no `tafsirFor` in `web/src`.

#### F2. Settings and widget-layout sync

Build: sync the settings document and home widget layout across devices using the same whole-row LWW + clamp + `expectedUserId` shape as the other sync domains. New migration (next free number), new module, wire `settings-document.ts`. Contract detail: `documentation/settings-system.md` §7. Notes stay device-local by decision; never add them to sync.
Files: `web/src/lib/home/layout.svelte.ts`, `web/src/lib/sync/`, `rust/backend/api/src/modules/`, `rust/backend/api/migration/src/`
Done when: changing a setting or widget layout on one signed-in device shows on another after sync; signed-out behaviour unchanged; `cargo test` covers LWW and owner mismatch.

#### F3. Notes library

Build: notes tab on `/yours`: list, search, sort, edit, delete. Add per-note timestamp and source-aware verse context to the note model. Device-local only.
Files: `web/src/lib/stores/annotations.svelte.ts`, `web/src/lib/home/notes-source.svelte.ts`, `web/src/routes/(application)/yours/`
Done when: a note written in the reader appears on `/yours` with verse, source and date, and can be found, edited and deleted there.

#### F4. Thread history clear-all

Build: clear-all on thread history. Tombstones every ended thread, with an Undo toast and no dialog.
Files: `web/src/routes/(application)/yours/_components/ThreadHistory.svelte`, `web/src/lib/sessions/store.svelte.ts`
Done when: clear-all empties history, Undo restores it, and the result syncs to other devices.

#### F5. Explicit reading bookmark

Build: user-set page/verse reading bookmark, separate from automatic resume. Set, replace with Undo toast, timestamp, source, card on `/yours`, synced with the account.
Files: `web/src/lib/stores/reader.svelte.ts`, `web/src/routes/(application)/_reader/VerseTools.svelte`, `web/src/routes/(application)/yours/`
Done when: automatic resume never overwrites the user-set bookmark; set, replace and undo are tested.

#### F6. Personal data export and import

Build: versioned JSON export of bookmarks, folders, notes, threads, cards and settings. Import validates, merges silently with a toast, and rejects newer schema versions. Lives in Settings.
Files: `web/src/routes/(application)/settings/`, new: `web/src/lib/storage/data-export.ts`
Done when: export then import on a clean browser round-trips everything; malformed and future-schema files are rejected with a clear message.

#### F7. Self-service account deletion

Build: authenticated deletion using `FreshAuthControls` re-auth. Revokes sessions, deletes Quran-state rows, device rows and device tokens, offers export first, clears local account data. Immediate, no grace period (default until the owner says otherwise). Update the privacy and terms deletion wording to match.
Files: `rust/backend/api/src/modules/user_v1/`, `web/src/routes/(account)/account/+page.svelte`, `web/messages/en.json`, `web/messages/ar.json`
Done when: API test proves rows and tokens are gone and the session is dead; legal copy no longer says "ask us to delete".
Blocked by: F6, A38

#### F8. Reading goals and streaks

Build: goal presets (time per day, finish in 30 days, finish in a year) plus custom range, built on `reading_daily` and `localDay`. Pause and completion. Streak calendar on `/yours`. Must never penalise very fast readers: time goals count verse reads too.
Files: `web/src/routes/(application)/yours/_components/ReadingInsights.svelte`, `web/src/lib/analytics/reading-analytics.ts`, `rust/backend/api/src/modules/user_analytics_v1/`
Done when: goal progress and streak derive from recorded reads, respect the user's local day, and are covered by tests including a fast-reader case.

#### F9. Goal reminders

Build: goal-aware reminder schedules on the current FCM substrate. Timezone, quiet hours, per-kind preferences, unsubscribe.
Files: `rust/backend/api/src/modules/notification_v1/`, `web/src/lib/firebase/`, `web/src/routes/(application)/settings/`
Done when: a scheduled reminder fires at the user's local time, respects quiet hours, and stops on unsubscribe.
Blocked by: F8

#### F10. Pin and compare verses

Build: ordered pin set with add, remove and reorder; compare drawer with composed Arabic and translation selection; mobile and overflow handling; convert pins into a bookmark folder.
Files: `web/src/routes/(application)/_reader/VerseTools.svelte`, new: `web/src/lib/stores/pins.svelte.ts`
Done when: pins survive navigation, compare renders on phone and desktop, conversion creates a folder.

#### F11. Parallel translation layout

Build: side-by-side presentation for stacked translations (extras stack vertically today, verse mode only); per-extra font controls. Stays client-only, no change to cache keys.
Files: `web/src/routes/(application)/_reader/StackedLanes.svelte`, `web/src/routes/(application)/_reader/stacked-translations.svelte.ts`
Done when: side-by-side works with up to 5 extras, collapses on narrow screens, and SSR/cache keys are untouched.

#### F12. Deeper navigation

Build: revelation-order sort in the sidebar; `surah:ayah` and range jump from anywhere in the palette; browse by ruku, manzil and sajda from existing metadata.
Files: `web/src/routes/(application)/_reader/Sidebar.svelte`, `web/src/routes/(application)/_reader/IndexTabs.svelte`, `web/src/lib/components/search/GlobalSearchPalette.svelte`
Done when: `2:255` and `2:255-2:286` jump correctly from any route with translation context preserved; revelation sort and new browse lists are tested.

#### F13. Share dialog and image card

Build: dialog for range, source and format with attribution options and the canonical link (`…/{slug}?ayah=2:255-2:286`); branded accessible image card with download. Plain copy/share stays the fast path. Video waits for audio.
Files: `web/src/lib/stores/reader-share.svelte.ts`, `web/src/routes/(application)/_reader/VerseTools.svelte`
Done when: card renders Arabic correctly and downloads; shared links open the right range.

#### F14. Bookmark bulk actions

Build: multi-select on `/bookmarks`: move and delete with an Undo toast.
Files: `web/src/routes/(application)/bookmarks/+page.svelte`, `web/src/lib/bookmarks/store.svelte.ts`
Done when: bulk move and delete work offline and sync; Undo restores them.

#### F15. Voice search

Build: voice input in the palette and `/search` with unsupported, permission and error states. Preserve the Arabic normalization/parity contract.
Files: `web/src/lib/components/search/GlobalSearchPalette.svelte`, `web/src/routes/(application)/search/+page.svelte`
Done when: control is hidden where unsupported; denied and failed states are handled and tested.

#### F16. Public daily ayah route

Build: indexable daily route from `/quran/random`, source-aware link, share metadata. Deterministic per UTC date, cached to UTC midnight.
Files: `web/src/lib/home/daily-ayah.ts`, `web/src/routes/(marketing)/`
Done when: every visitor sees the same ayah on a UTC day; link preserves translation context.

#### F17. Translation correction report

Build: "Report an issue" on a verse, prefilled with immutable source id and verse key, sent to the contact address behind Turnstile. Never edits a Quran DB.
Files: `web/src/routes/(application)/_reader/VerseTools.svelte`, `web/src/routes/(marketing)/contact/`
Done when: report arrives with source id and verse key; spam control is in place.

#### F18. RTL polish and localized text variants

Build: fix remaining physical utilities and reader arrows in RTL; localized `.md`/`.txt` representations. Keep UI locale out of reader server cache keys.
Files: `web/src/routes/[slug=marketingText].md/`, `web/src/routes/[slug].txt/`, `web/src/routes/(application)/_reader/`
Done when: logical-properties guard green; Arabic text variants serve Arabic.

#### F19. Recitation audio and player

Build: reciter/recording catalogue, verse timing, surah and ayah queue, play/pause/seek/next/previous, persistent mini and full player, playback speed, media-session controls, failure/retry, optional offline audio with explicit storage limits, attribution.
Files: `web/src/lib/stores/reader-core.svelte.ts`, `web/src/lib/stores/reader.svelte.ts`, new: `web/src/lib/audio/`
Done when: continuous surah playback survives navigation and backgrounding; failures recover.
Blocked by: O22

#### F20. Memorization repeat engine

Build: verse/range selection, repeat counters, inter-ayah delay, boundaries, cancellation, screen-lock behaviour, saved presets.
Files: new: `web/src/lib/audio/repeat.ts`
Done when: a 3-verse range repeats the set count with the delay and stops cleanly.
Blocked by: F19

#### F21. Word-by-word

Build: canonical word coordinates, Arabic token display, per-word meaning and transliteration, popovers with keyboard and touch, offline delivery, attribution. Tests must prove Quran text is never rewritten.
Files: `web/src/routes/(application)/_reader/VerseRow.svelte`, new: `web/src/lib/quran/words/`
Done when: every token maps to its source word; reader text bytes are unchanged.
Blocked by: O23

#### F22. More tafsir sources

Build: add licensed tafsir beyond `ar.jalalayn` and `ar.muyassar`: verse coverage, source switching, attribution, footnotes, citation links, offline rules, missing-source states.
Files: `web/src/lib/data/translations.json`, `web/src/lib/components/translation/TranslationModal.svelte`
Done when: each new source is credited and missing verses show a clear state.
Blocked by: O24

### Copy

#### C1. Copy truth pass

Build: drop or defer recitation claims in `legal_terms_service_body`, `legal_terms_third_party_body` and `faq_offline_answer` unless F19 shipped. Then sweep landing, reader empty/loading/settings labels, and PWA manifest descriptions: every capability is available now, beta with an entry point, or planned. Remove the unused `auth_merge_*` strings. Recheck translation-source counts against the baked catalogue.
Files: `web/messages/en.json`, `web/messages/ar.json`, `web/messages/auth/en.json`, `web/messages/auth/ar.json`, `web/static/manifest.webmanifest`
Done when: no shipped copy promises a feature that doesn't exist; `en` and `ar` match.

## Owner only

Human steps an agent cannot do. Delete a step when done.

- **O1. Ingress: Cloudflare Tunnel (or per-zone Authenticated Origin Pulls).** Create `cloudflared` service with `TUNNEL_TOKEN`, public hostnames to Traefik. Add a Cloudflare Transform Rule that sets `x-eq-ingress-marker` (≥32 CSPRNG bytes, same value as `INGRESS_EDGE_MARKER`) on every origin request. Verify the origin is not reachable except through the edge (scan from outside: no 443/80 response). Unblocks: A1 real deploy; A15 (marker identity).
- **O2. Backup rehearsal and destination.** WAL-safe copy (`sqlite3 mode=ro ".backup"`), restore rehearsed. Off-host encrypted destination. 30-day cap. Unblocks: A19 (ban migration), A32 (sweeper deletes), A33. Rule: backup before first live schema change in A5 too.
- **O3. Mailboxes (MX verified).** Provision `privacy@`, `abuse@`, `legal@`, `support@` on `easyquran.fyi`, plus four intake aliases for reports. Verify MX records. Unblocks: A4 contact copy; A24 intake; A21 real delivery check.
- **O4. Gate −1: publish the base rate.** Run `just abuse-report` (A5) on a production DB copy. Write the sentence with the date: "Measured abuse base rate as of <date>: <n> active account bans, <n> escalation bans, <n> comment flags, <n> mail suppressions, <n> audit rows; production per-IP/ASN request distribution flat (from Cloudflare; include it in the quarterly report). Device identity is deferred; it reopens if any of these counters moves, or if the Gate B condition is observed." Write `deploy/abuse/2026-Q4.md` (quarterly report, within 10 days of quarter end). Until counters exist, write "not measurable". Unblocks: A15 and A16 ceiling values; A9 numeric caps.
- **O5. Reader-path ship window.** Ship A15–A18 (reader ceilings) outside the six weeks before Ramadan. Owner sets the date. Unblocks: A15–A18 release.
- **O6. Decision O1 (India).** Confirm: no abuse device processing for India (default). Confirm no accounts under 18 in India (default). Decide whether that rule also covers the account device list (D2); default no, since it is user-visible account function with no abuse use. Unblocks: A34 minors path.
- **O7. Decision O3 (permanent bans).** Confirm: no permanent bans; longest ban auto-expires; renewal requires recorded self-review (default). Unblocks: A19 validation rule.
- **O8. Decision O10 (moderator role).** Confirm: moderator role not granted until A28 lands (default). Unblocks: A28 release.
- **O9. Decision O11 (billing intent).** Confirm: billing unmounted until payments are intended (default). Unblocks: A3 final.
- **O10. Decision O14 (public bulk data).** Confirm: publish bulk path (default yes). Unblocks: A18.
- **O11. Legal artifacts and sign-off.** Name the owner for DPIA, LIA, ROPA, retention schedule, and quarterly measurement (decision O5/O13). Write and sign privacy and terms text. Record dates in `deploy/legal/`. Input: `docs/research/abuse-prevention/R2-legal-privacy.md`. If unowned, A34 does not ship. Unblocks: A34 publication; A26 legal-debt list.
- **O12. FCM operator device.** Register operator device for P0 push; set `FCM_ENABLED`. Unblocks: A26 push path.
- **O13. Incident drill.** Run one drill end to end (report → case → capture → ban → notice with `delivery_state` → appeal → decision). Keep written log in `deploy/incident-response.md`. Set next drill date. Unblocks: A26 acceptance.
- **O14. Cloudflare rules and secrets.** (a) Cache Rule for `/offline/*` (A16 edge part). (b) `CLOUDFLARE_EMAIL_WEBHOOK_SECRET` provisioned only with A29 item 3 review rule live. (c) CDN purge config for media (A30).
- **O15. Physical erasure claim (decision O17).** Confirm the published erasure wording (default: "removed from the live database; removed from backups within 30 days"). Physical swap only if the owner requires that claim. Unblocks: A33 scope.
- **O16. Custody document (decision O15).** Write where the pepper and other secrets live, who answers DSARs, what happens on restore or change of control. Unblocks: A34 legal.
- **O17. Scheduled external canary.** Non-Cloudflare probe that sends a forged header and asserts 400. Unblocks: A1 canary acceptance.
- **O18. Synthetic translated-page check.** External probe on a translated page with alert. Unblocks: A15 acceptance.
- **O19. Public uptime monitor.** External monitor on public health endpoint. Unblocks: A26 item 3.
- **O20. Registration numbers (decision O18).** Provide per-IP per-day cap, per-domain per-day cap, burst floor after A5 data. Unblocks: A9 values.
- **O21. Documentation owned by owner.** The following are owner-only edits (AGENTS hard rule): `documentation/history.md` (landed-control record; line 36 CI claim is false until A2 lands; falsification sentences: "The central bet is wrong if within 90 days there are two or more evasion cases where the returning account shares no credential, no verification identity, no content signature, and no actionable IP/ASN. It is right if there are zero such cases, or every evasion is linked by credential, identity, or content reuse, and no legitimate user was blocked by the per-credential limits."), `documentation/quran-system.md` Part 4 (ingress and rate-limit contract amended by A1, A14, A15). Unblocks: record only.
- **O22. Recitation licence and source.** Unblocks: F19, F20, then reciter catalogue and radio.
- **O23. Licensed word/token source.** Unblocks: F21.
- **O24. Licensed tafsir sources.** Unblocks: F22.
- **O25. QA.** ⌘K go-to-verse; reading threads, folder bookmarks and reading insights end to end; offline handoff and remote sign-out across a real phone and desktop.
- **O26. Editorial content, decide first.** Study panels, surah info, learning plans, Quran-in-a-Year, Ramadan hubs, topics, full duas catalogue and review. No item until you approve one.
- **O27. Product calls.** Standalone ayah/range routes (`?ayah=` grammar is settled); discovery home beyond cards; Mushaf layouts; production IndoPak review; public newsletter signup; more UI locales; fluent Arabic review of `about`/`faq`/`contact`/`privacy`/`terms`.
- **O28. Strategy, no item unless approved.** Partner directory, cross-product identity, public developer platform, feedback portal, donations.
- **O29. Image safety.** Hash-match provider and NCMEC registration. Unblocks: A37 rollout.
- **O30. Reading-session ids.** Confirm production has no `reading_sessions` rows with v4 ids before deploying D1 (`m000013` was undeployed as of 2026-10-14). If any exist, tell the agent; D1's validator would refuse their updates.
- **GATED: Gate B (abuse device identity).** Account device management (D2–D6) is not gated and never feeds this. Do not start any abuse device work (W18 device token, W19 component fingerprinting) unless: ≥90 days or ≥1,000 signups of A5 data; and the operator has a recorded ban-evasion case where the returning account shares no credential, no verification identity, no content signature, and no actionable IP/ASN, and the operator can name the case. Corroborator recorded as enum `credential_reuse | content_match | ip | none`, never derived from the device token. If it fires, the design of record is `docs/research/abuse-prevention/DESIGN-v1.md` and `B4-rules.md` (keep those two files until then). Device cookie scope if it ever ships: `ruxlog.device`, `Path=/api`, HttpOnly, 30 days, 1-year cap, HMAC'd token, link table per (device, account) with no FK to users. Otherwise: record "not measurable", re-run quarterly, do not build.
- **GATED: Gate −1** is item O4 above. A9 numeric caps and A15–A16 ceilings wait on it.
