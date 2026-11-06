pub use sea_orm_migration::prelude::*;

mod m000001_init;
mod m000002_rate_limit_state;
mod m000003_translation_popularity;
mod m000004_auth_session_binding;
mod m000005_device_notification_oauth;
mod m000006_email_code_unique_user;
mod m000007_bookmarks;
mod m000008_bookmark_verse_unique;
mod m000009_lowercase_emails;
mod m000010_passkey_credentials;
mod m000011_device_token_ownership;
mod m000012_bookmark_context;
mod m000013_reading_sessions;
mod m000014_user_analytics;
mod m000015_newsletter_subscribers;
mod m000016_abuse_measurement;
mod m000017_sync_protocol;
mod m000018_device_registry;
mod m000019_device_keys;
mod m000021_user_preferences;
mod m000022_verification_identity;
mod m000026_email_change_pending;
mod m000030_user_ban_contract;
mod m000031_reminder_schedules;
mod m000032_user_bans_evidence_safe;
mod m000033_notice_channel;
mod m000034_appeals;
mod m000035_shared_device_check;
mod m000050_audit_logs_evidence;
mod m000060_reports;
mod m000061_case_evidence;
mod m000070_mail_budget;
mod m000100_comment_notice_hardening;
mod m000110_device_token_hygiene;
mod m000120_media_deletions;
mod m000140_media_upload_indexes;
mod m000150_retention_sweep;
mod m000160_erasure_receipts;
mod m000170_data_requests;
mod m000200_user_is_minor;
mod m000210_user_age_declared;
mod m000220_retention_sweep_indexes;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m000001_init::Migration),
            Box::new(m000002_rate_limit_state::Migration),
            Box::new(m000003_translation_popularity::Migration),
            // m000004 ships AFTER m000003; additive (new table only), so an old
            // binary rolled back across this migration simply ignores the table.
            Box::new(m000004_auth_session_binding::Migration),
            // m000005 backfills tables the device/notification/OAuth code paths
            // always wrote but no migration owned; IF NOT EXISTS keeps any
            // out-of-band-provisioned DB that already has them untouched.
            Box::new(m000005_device_notification_oauth::Migration),
            // m000006 adds the missing UNIQUE(user_id) on email_verifications —
            // without it the regenerate() upsert fails at prepare time in SQLite
            // (surfaced as DB_003 "Duplicate entry" on resend) — and enforces the
            // same 1:1 invariant on forgot_passwords.
            Box::new(m000006_email_code_unique_user::Migration),
            // m000007 ships the offline-sync bookmark tables owned by
            // bookmark_v1; IF NOT EXISTS keeps any DB already carrying them
            // untouched.
            Box::new(m000007_bookmarks::Migration),
            // m000008 makes (user_id, surah, ayah) the bookmark identity: it
            // normalizes legacy CURRENT_TIMESTAMP-shaped updated_at rows into
            // the canonical LWW wire format, dedupes existing verse groups
            // (keep MAX(updated_at), tie-break max(rowid)), then enforces
            // UNIQUE (user_id, surah, ayah) so two devices can't mint two live
            // rows for one verse.
            Box::new(m000008_bookmark_verse_unique::Migration),
            // m000009 normalizes stored account emails to the canonical
            // trim+lowercase identity form used by every lookup since the
            // normalization change; collision pairs are skipped, not merged.
            Box::new(m000009_lowercase_emails::Migration),
            // m000010 creates passkey_credentials — the WebAuthn code paths
            // wrote this table but no migration ever created it, so passkey
            // writes and the identities COUNT(*) failed on every real DB.
            Box::new(m000010_passkey_credentials::Migration),
            Box::new(m000011_device_token_ownership::Migration),
            // m000012 rebuilds bookmarks as folder-based:
            // every bookmark lives in a folder, a server-ensured default
            // folder per user, a verse may sit in many folders.
            Box::new(m000012_bookmark_context::Migration),
            // m000013 ships reading sessions: one
            // row per continuous reading thread (cursor inline) plus the
            // account-level MAX-merged reading_progress.
            Box::new(m000013_reading_sessions::Migration),
            // m000014 ships reading insights:
            // idempotent raw reading_events plus the reading_daily and
            // reading_ayah_totals rollups that survive the retention purge.
            Box::new(m000014_user_analytics::Migration),
            // m000015 restores newsletter_subscribers, dropped from the chain by
            // the m000001 squash (4876e8e1); the seeder and newsletter_v1 need it.
            Box::new(m000015_newsletter_subscribers::Migration),
            // m000016 ships abuse measurement: the durable block ledger
            // (rate_limit_events, transition rows only) and job_runs (one row per
            // periodic-job run). Both new tables, IF NOT EXISTS, no backfill.
            Box::new(m000016_abuse_measurement::Migration),
            Box::new(m000017_sync_protocol::Migration),
            // m000018 ships the device registry (devices.md D2): user_devices, plus a
            // nullable device_id on devices and user_sessions. Additive, IF NOT EXISTS.
            Box::new(m000018_device_registry::Migration),
            // m000019 ships device keys and continuity (devices.md D3): key columns on
            // user_devices, the conflict flag, and user_device_salts. Additive, IF NOT EXISTS.
            Box::new(m000019_device_keys::Migration),
            Box::new(m000021_user_preferences::Migration),
            // m000022 ships the verification identity column (A10): a nullable keyed
            // HMAC per account, filled by the boot backfill. Additive, IF NOT EXISTS.
            Box::new(m000022_verification_identity::Migration),
            // m000026 parks a requested email change until its code is proven (A12).
            // m000023 to m000025 are reserved.
            Box::new(m000026_email_change_pending::Migration),
            // m000030 ships the ban contract core (A19): one active ban per account
            // (partial unique index), the enforcement_notices case record, and audit
            // indices. Additive; down() drops only the active-ban index.
            Box::new(m000030_user_ban_contract::Migration),
            // m000031 holds the goal reminder schedule (F9). m000027 to m000030 are reserved.
            Box::new(m000031_reminder_schedules::Migration),
            // m000032 rebuilds user_bans without its ON DELETE CASCADE from users (A19b):
            // account deletion keeps ban history. Dedupe-only on rows, no row is deleted.
            Box::new(m000032_user_bans_evidence_safe::Migration),
            // m000050 rebuilds audit_logs with no FK to users and adds expires_at and
            // retention_class (A23). Numbers m000033 to m000049 are skipped on purpose.
            Box::new(m000050_audit_logs_evidence::Migration),
            // m000033 adds enforcement_notices.notice_channel (A21). Constant default, guarded add.
            Box::new(m000033_notice_channel::Migration),
            // m000034 adds the appeals table (A35): one appeal per notice, 7-day decision
            // clock, token hash index on enforcement_notices. Additive, IF NOT EXISTS.
            Box::new(m000034_appeals::Migration),
            // m000035 adds enforcement_notices.shared_device_checked (A22). Constant default 0.
            Box::new(m000035_shared_device_check::Migration),
            // m000060 ships incident intake reports (A24): no FK to users, expiry columns, 12-month class.
            Box::new(m000060_reports::Migration),
            // m000061 adds case_evidence, legal_holds and legal_hold_subjects (A25). No FK to users.
            Box::new(m000061_case_evidence::Migration),
            // m000070 adds mail budget columns (A29): email_suppression.blocks_transactional
            // (webhook rows default 0 until reviewed) and newsletter retention columns.
            Box::new(m000070_mail_budget::Migration),
            // m000100 hardens comment moderation (A27): post_comments.flags_cleared_at, and
            // enforcement_notices gains subject_kind, nullable ban_id, comment_id and automated.
            Box::new(m000100_comment_notice_hardening::Migration),
            // m000110 adds devices.invalid_strikes and devices.invalid_at (A36). Constant default 0.
            Box::new(m000110_device_token_hygiene::Migration),
            // m000120 adds media_deletions (A30): media takedown tombstones. No FK, 12-month class.
            Box::new(m000120_media_deletions::Migration),
            // m000140 adds media indexes on uploader_id and content_hash (A31). Plain, IF NOT EXISTS.
            Box::new(m000140_media_upload_indexes::Migration),
            // m000150 ships the retention sweeper's guard indexes (A32). No new tables.
            Box::new(m000150_retention_sweep::Migration),
            // m000160 adds erasure_receipts (A33): subject erasure receipts, replay list for boot. No FK, 24-month class.
            Box::new(m000160_erasure_receipts::Migration),
            // m000170 adds data_requests (A38): one-month erasure request clock. No FK, 24-month class.
            Box::new(m000170_data_requests::Migration),
            // m000200 adds users.is_minor (A34): constant-default flag from self-declared age at sign-up.
            Box::new(m000200_user_is_minor::Migration),
            // m000210 adds users.age_declared_at (A34): when the account recorded its age. NULL = never.
            Box::new(m000210_user_age_declared::Migration),
            // m000220 adds expression indexes on the age columns the A32 sweeper filters on.
            Box::new(m000220_retention_sweep_indexes::Migration),
        ]
    }
}
