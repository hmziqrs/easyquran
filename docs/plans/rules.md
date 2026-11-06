# Rules for every item

[Back to index](../README.md)

Standing rules, closed decisions and what already exists.

## Standing rules

- Migrations: next free number at build time (`m000015` is the latest today). Every migration: all statements `IF NOT EXISTS`, wrapped in explicit `conn.transaction` (sea-orm does not wrap SQLite migrations), re-runnable, tested with up-twice and a full migrator run. `CREATE UNIQUE INDEX` on a populated table: dedupe first (pattern: `m000008`, `m000011`). No `ALTER TABLE … ADD COLUMN` with a non-constant default.
- Every new table: `expires_at INTEGER NOT NULL` (epoch seconds, never TEXT) and `retention_class TEXT NOT NULL`. Data that lives as long as the account (settings, sync counters, device salts) uses `retention_class = 'account_lifetime'` with `expires_at = 253402300799`; the sweeper skips it and erasure deletes it. No `ON DELETE CASCADE` from `users` on any evidence table. No `DELETE … LIMIT` (not compiled into this SQLite).
- Process: DB pool is 1 connection, WAL, `synchronous=FULL`. App is single instance. Process-local state is acceptable where stated.
- No new DB write on the request path unless an item says so (login already writes).
- Never claim a privacy, legal or security property the code does not enforce. Every user-facing sentence maps to code.
- Product: reduce friction. No dialogs or review screens where a silent safe default with a toast works. shadcn-svelte for UI. Pre-launch, so no legacy shims. Reading heuristics never penalise very fast readers. Notes stay device-local. Personal reading insights are always on.
- State the fix in code and commit messages, not attack recipes. This repo is public.
- Native apps have no screen-reader requirement (owner decision, 2026-11-06). Web keeps its existing accessibility gates. Desktop renders all Arabic and Urdu through HarfRust. Text is verbatim from database to screen on every client.
- Client-minted entity ids are UUID v7 on every client (web, React Native, GPUI). Default folders stay deterministic UUID v5. An id's timestamp sets sort order only: it comes from the device clock, so never trust it for security or use it as `startedAt`.
- Device identity has two purposes only: keeping one account's device list consistent, and attributing sync writes. The device id, key and continuity hint never reach analytics, rate-limit keys, bans, abuse scoring or logs, and never act as an auth factor. Ids, keys and hints are per account on each install, so two accounts on one phone share nothing. Every lookup includes `user_id`. No device cookie, and nothing device-related on reader routes or for signed-out readers.
- Every synced domain follows `contracts/sync.md` (S1): stamps decide conflicts, wall clocks never do, the merge is the same pure function on server and clients, and it passes the shared vectors. A new domain adds its rule to the contract and vectors before any code.
- Enforcement ladder: L0 observe → L1 throttle → L2 challenge → L3 cooldown → L4 queue only (no restriction; hard expiry; auto-downgrade if unreviewed) → L5 interim restriction (human-approved, or auto-reversing within 72 h) → L6 ban (human before effect) → L7 legal referral.
- A19–A22 and A35 are one release: nothing separable reaches production alone, or bans become silent lockouts and the banned screen links to an appeal page that does not exist.
- Production Quran bytes, size and path come from baked id maps, never remote metadata. Bulk JSON follows the same rule.
- Keep gstatic, googletagmanager and other CSP origins Firebase Analytics needs. Do not "fix" CSP by removing them.
- Drain the old container before a migration that can take a write lock (`SQLITE_BUSY`).
- Add source-scan guards as the matching items land: no `ON DELETE CASCADE` from `users` on evidence tables, no `DELETE … LIMIT`, route-count guard for new admin nests.
- Shared Rust test harness lives in `tests/` (`src/test_utils` is `cfg(test)`-only; `tests/reading_common/mod.rs` is the pattern). Update existing guards when you change what they pin: `ErrorCode` snapshot (`rust/crates/ruxlog-types/src/error.rs`), CORS route table, ban-export tests, `auth-client.test.ts` fetch counts, the middleware-order mirror `full_app()` in `tests/quran_v1.rs`.
- Rollout abort: any existing test fails; ingress canary reports a forged header; sweeper deletes a row under a hold or live ban; a notice says `sent` with no mail delivered; reader shows empty shells; authenticated post list returns 500.

## Do not build (these decisions are closed; do not re-open in code)

- Component fingerprinting as an identifier or ban key, on any client. Canvas/WebGL/audio/fonts/plugins/battery signals in browsers; IMEI, serial, MAC or IDFV on native. Device identity is a per-install id, a device-held key, and the salted per-account continuity hint in D3, the only use of an OS machine id. No hint on Apple platforms: Apple's license (section 3.3.9) forbids deriving device data to uniquely identify a device.
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
- Clamp of client `updated_at` to now+60 s (`reading_session_v1/controller.rs clamp_ts`); S3 and S4 replace it for conflict decisions. Verse-reference validation per surah (`bookmark_v1/controller.rs ensure_valid_range`). Range span cap 50. `expectedUserId` ownership guards on all three sync routes.
- Newsletter table: migration `m000015_newsletter_subscribers.rs` exists.
- `user_bans` model (`db/sea_models/user_ban/actions.rs`). `get_active_ban` runs at login (`services/auth.rs`); create and revoke have no production callers.
- `not_banned()` requirement helper in `middlewares/auth_guard.rs`; used by `verified`, `verified_with_role`, `authenticated` guards.
- Mail: `lookup_suppressed`, `rcpt_transactional` cap, `suppressed` metric (`services/mail/router.rs`). Mail webhook route mounted at `/mail/v1/webhook/{provider}`.
- `email_suppression` never-downgrade upsert.
- Partial: `utils/telemetry.rs` and `capped` appears in bookmark and reading-session controllers (verify the envelope before reuse; see A7).
