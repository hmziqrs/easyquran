# Abuse phase 2: ban contract and enforcement

[Back to index](../README.md)

#### A32. Retention sweeper with ship gates

Build:

1. Every new table: `expires_at INTEGER NOT NULL` and `retention_class TEXT NOT NULL`. Source-scan guard over the migration directory (house pattern).
2. `rust/backend/api/src/services/retention.rs` (new). Own task, registered next to `scheduler::start_scheduler` (`rust/backend/api/src/services/scheduler.rs` line ~11) but not on the flush task. 6-hour interval with 0–300 s jitter. ≤500 rows per statement. 50 ms pause between batches. ≤20 batches per table per run. No `DELETE … LIMIT`. Non-fatal per table. No `PERSISTENCE_LOCK`.
3. Every delete adds: `NOT EXISTS (SELECT 1 FROM legal_holds … )` and `NOT EXISTS (SELECT 1 FROM user_bans WHERE … AND revoked_at IS NULL)`.
4. Retention classes:
   - `user_sessions` 90 days (raw UA and IP stored forever today); UA and IP NULLed on revoke.
   - `devices` (FCM) and `user_devices` 90 days idle.
   - Deleted thread and folder content scrubbed after 24 h (S6). Sync tombstones 180 days. Advance `tombstone_floor` before deleting.
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

Blocked by: A26, A33

#### A34. Policy text sentence ledger, minors, code side of legal artifacts

Build:

1. Sentence ledger: for each sentence in `web/src/routes/(marketing)/**` privacy and terms, record the code reference that makes it true (table in `deploy/legal-debt.md`, or a comment map). Remove sentences without a reference. After A32 is green, publish the full privacy policy. Abuse device-identity paragraph only if it ships (not in this queue); D2 adds the account-device sentence.
2. Minors: self-declared date of birth → `users.is_minor` flag (migration). For minors: no device records, no push, no analytics, no comments. India: no accounts under 18 and no device processing (default; O6 confirms). Test the minor flag path end to end.
3. No global consent banner. Consent is jurisdiction-aware. If client collection ever ships, just-in-time consent at login, equal-weight decline, reader never gated.
4. Art 17 statement-of-reasons template and appeal SOP text: in `deploy/moderation-sop.md` (A22 creates it).
5. Legal artifacts (DPIA, LIA, ROPA, retention schedule): owner authors and signs (O11). Do not write them here. Code side only: `deploy/legal/` is the owner's.

Files: `web/src/routes/(marketing)/**`, `web/messages/{en,ar}.json`, `rust/backend/api/src/db/sea_models/user/*`, `rust/backend/api/src/modules/user_v1/*`, new migration for `is_minor`, `deploy/legal-debt.md`.

Done when: sentence ledger has no unreferenced sentences (reviewer check); minor flag test blocks device write, push, analytics, and comments for minors; `bun run check`, `bun run lint`, `bun run test` green.
Blocked by: A4, A26, A32
Owner: O11, O6.
