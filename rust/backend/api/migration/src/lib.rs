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
        ]
    }
}
