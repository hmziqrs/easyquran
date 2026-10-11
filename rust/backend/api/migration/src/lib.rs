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
        ]
    }
}
