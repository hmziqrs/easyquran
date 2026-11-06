# Abuse phase 2: ban contract and enforcement

[Back to index](../README.md)

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

1. The `confirm_evidence` guard is built in A25; do not build it twice. Delete the dead helper `comment_flag::clear_flags` (`rust/backend/api/src/db/sea_models/comment_flag/actions.rs` ~179–186; zero callers).
2. User delete requires a typed email confirmation. Writes a pre-state snapshot into `audit_logs` before deleting (the cascade currently destroys bans, sessions, and reports; see `m000001_init.rs` ~48–73 and ~283–308).
3. Category delete requires confirmation (cascades every post in the category).
4. Media delete: restrict to Admin. Moderator keeps read. Current media routes use `verified_with_role::<ROLE_AUTHOR>` (`rust/backend/api/src/modules/media_v1/mod.rs` ~20). Verify each delete route.
5. Add rate-limit layers to the unmetered nests, one line each in `rust/backend/api/src/router.rs`: `/feed/v1` (~135), `/category/v1` (~128), `/tag/v1` (~129), `/mail/v1` (~130), `/email_verification/v1` (~84), `/forgot_password/v1` (~85), `/analytics/v1` (~146), `/admin/route/v1` (~148), `/admin/acl/v1` (~150), `/admin/bans` (verify its mount). Use the existing `rate_limit_layer` pattern.
6. Route blocker: refuse to block its own unblock route. Audit every block and unblock (A23).
7. `app_constants` import filter: `R2_ACCESS_KEY_ID` slips past the denylist in `rust/backend/api/src/services/acl_service.rs` (~85–135) because it ends `_ID`, and is stored in plaintext. Add an `ACCESS_KEY` needle; a blanket `_ID` needle would refuse harmless values such as `GOOGLE_CLIENT_ID`. Unit test: known secret env names are refused.
8. Seeder: refuse to run when `is_production()` at every entry point (`seed_v1`, the TUI, and CLI paths; none check it today, and only the `seed-system` feature gates the routes). Drop the `password == email` pattern. Stop minting Admin-role accounts. Stop the TUI undo deleting ID ranges that include `users` (`rust/backend/api/src/services/seed/undo.rs` ~34–105).
9. Narrow moderator inline powers: `is_moderator()` must not grant reading any user's email, paywall bypass, hidden-post comments, attaching any media, or seeing all drafts.
10. Persist the route-blocker runtime pause (`route_blocker_config::is_paused` is in-memory, a redeploy reverts it): store in `app_constants`, rehydrate at boot.

Files: `rust/backend/api/src/modules/{user_v1,category_v1,media_v1,admin_route_v1}/*`, `rust/backend/api/src/db/sea_models/{comment_flag,post_comment}/actions.rs`, `rust/backend/api/src/services/seed/*`, `rust/backend/api/src/services/acl_service.rs`, `rust/backend/api/src/tui/*`, `rust/backend/api/src/router.rs`.

Done when: every destructive action in the A23 list has an audit row; category delete without confirmation refused (test); a Moderator cannot delete media (test); unmetered-nest test list is empty (guard test); `R2_ACCESS_KEY_ID` refused by filter (test); seeder refuses in production (test).
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

Files: `rust/backend/api/src/services/mail/{router,provider,cloudflare,smtp}.rs`, `rust/backend/api/src/modules/{mail_v1,newsletter_v1,email_verification_v1}/*`, `rust/backend/api/src/router.rs`, `.env.example`, new migration (`blocks_transactional`, token hash), web newsletter routes.

Done when: a 10,000-address registration burst cannot exceed the global cap and pauses on bounce rate (test); one-click unsubscribe returns 200 for an unknown token (test); `grep -r List-Unsubscribe rust/backend/api/src` non-empty; webhook suppression does not block transactional mail until reviewed (test).
Blocked by: A21, A23
Owner: O14 for the webhook secret.

#### A36. Device token hygiene

Build:

1. `UNREGISTERED` (404) already prunes at once in `services/notification/mod.rs`; keep that, as Firebase advises. `SENDER_ID_MISMATCH` (403) and `INVALID_ARGUMENT` (400) are new error classes in `rux-fcm`. A 400 can also mean a bad payload, so mark `invalid_at` and prune only after 2 consecutive failures on that token.
2. Validate token shape on device registration. Write a `device.transfer` audit row when a token moves accounts.

Files: `rust/backend/api/crates/rux-fcm/src/{client,error}.rs`, `rust/backend/api/src/services/notification/mod.rs`, `rust/backend/api/src/modules/device_v1/`, new migration if `invalid_at` is absent.

Done when: `UNREGISTERED` prunes at once and the other two classes prune on the second consecutive failure (tests); malformed tokens are refused; transfer writes an audit row.

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
2. Strip metadata by default and apply orientation first. Check first which served objects keep EXIF and GPS today; the test must cover the original as well as each variant.
3. Bound decode cost: width and height caps plus a decode semaphore, in addition to the existing 12 Mpx ceiling.
4. Per-uploader quota and daily cap. Indices on `media.uploader_id` and `media.content_hash` (migration).
5. Gate: do not grant the Author role for media upload and do not ship any media UI until items 1–4 are done. Existing limits: 2 MiB, 12 Mpx, 30 per minute.

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
