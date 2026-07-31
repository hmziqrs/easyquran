use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let stmts: &[&str] = &[
            // `users.avatar_id` intentionally has no DB-level FK: it would form a
            // cycle with `media.uploader_id → users` (no valid insert/delete order).
            r#"CREATE TABLE IF NOT EXISTS "users" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "name" TEXT NOT NULL,
                "email" TEXT NOT NULL UNIQUE,
                "password" TEXT,
                "avatar_id" INTEGER,
                "is_verified" INTEGER NOT NULL DEFAULT 0,
                "role" TEXT NOT NULL DEFAULT 'user',
                "two_fa_enabled" INTEGER NOT NULL DEFAULT 0,
                "two_fa_secret" TEXT,
                "two_fa_backup_codes" TEXT,
                "two_fa_last_totp_counter" INTEGER,
                "google_id" TEXT,
                "oauth_provider" TEXT,
                "session_auth_secret" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "media" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "bucket" TEXT,
                "object_key" TEXT NOT NULL,
                "mime_type" TEXT NOT NULL,
                "width" INTEGER,
                "height" INTEGER,
                "size" INTEGER NOT NULL,
                "extension" TEXT,
                "uploader_id" INTEGER,
                "reference_type" TEXT,
                "content_hash" TEXT,
                "is_optimized" INTEGER NOT NULL DEFAULT 0,
                "optimized_at" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("uploader_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "user_sessions" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "device" TEXT,
                "ip_address" TEXT,
                "last_seen" TEXT NOT NULL,
                "revoked_at" TEXT,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "user_bans" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "reason" TEXT,
                "banned_by" INTEGER,
                "expires_at" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "revoked_at" TEXT,
                "revoked_by" INTEGER,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("banned_by") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL,
                FOREIGN KEY ("revoked_by") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "categories" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "name" TEXT NOT NULL,
                "slug" TEXT NOT NULL,
                "parent_id" INTEGER,
                "description" TEXT,
                "cover_id" INTEGER,
                "logo_id" INTEGER,
                "color" TEXT NOT NULL,
                "text_color" TEXT NOT NULL,
                "is_active" INTEGER NOT NULL DEFAULT 1,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("cover_id") REFERENCES "media" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL,
                FOREIGN KEY ("logo_id") REFERENCES "media" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "tags" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "name" TEXT NOT NULL,
                "slug" TEXT NOT NULL,
                "description" TEXT,
                "color" TEXT NOT NULL,
                "text_color" TEXT NOT NULL,
                "is_active" INTEGER NOT NULL DEFAULT 1,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "posts" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "title" TEXT NOT NULL,
                "slug" TEXT NOT NULL UNIQUE,
                "content" TEXT NOT NULL,
                "excerpt" TEXT,
                "featured_image_id" INTEGER,
                "status" TEXT NOT NULL DEFAULT 'draft',
                "published_at" TEXT,
                "author_id" INTEGER NOT NULL,
                "category_id" INTEGER NOT NULL,
                "view_count" INTEGER NOT NULL DEFAULT 0,
                "likes_count" INTEGER NOT NULL DEFAULT 0,
                "tag_ids" TEXT NOT NULL DEFAULT '[]',
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("author_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("category_id") REFERENCES "categories" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("featured_image_id") REFERENCES "media" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_revisions" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "content" TEXT NOT NULL,
                "metadata" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "scheduled_posts" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "publish_at" TEXT NOT NULL,
                "status" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_series" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "name" TEXT NOT NULL,
                "slug" TEXT NOT NULL,
                "description" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_series_posts" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "series_id" INTEGER NOT NULL,
                "post_id" INTEGER NOT NULL,
                "sort_order" INTEGER NOT NULL DEFAULT 1,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("series_id") REFERENCES "post_series" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_views" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "ip_address" TEXT,
                "user_agent" TEXT,
                "user_id" INTEGER,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_access" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "access_type" TEXT NOT NULL,
                "price_cents" INTEGER,
                "currency" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_likes" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "user_id" INTEGER NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "media_variants" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "media_id" INTEGER NOT NULL,
                "object_key" TEXT NOT NULL,
                "mime_type" TEXT NOT NULL,
                "width" INTEGER,
                "height" INTEGER,
                "size" INTEGER NOT NULL,
                "extension" TEXT,
                "quality" INTEGER,
                "variant_type" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("media_id") REFERENCES "media" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "media_usage" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "media_id" INTEGER NOT NULL,
                "entity_type" TEXT NOT NULL,
                "entity_id" INTEGER NOT NULL,
                "field_name" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("media_id") REFERENCES "media" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "email_suppression" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "recipient" TEXT NOT NULL UNIQUE,
                "reason" TEXT NOT NULL,
                "source" TEXT,
                "diagnostic" TEXT,
                "permanent" INTEGER NOT NULL DEFAULT 0,
                "last_seen" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "app_constants" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "key" TEXT NOT NULL UNIQUE,
                "value" TEXT NOT NULL,
                "value_type" TEXT,
                "description" TEXT,
                "is_sensitive" INTEGER NOT NULL DEFAULT 0,
                "source" TEXT NOT NULL DEFAULT 'env',
                "updated_by" INTEGER,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "route_status" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "route_pattern" TEXT NOT NULL UNIQUE,
                "is_blocked" INTEGER NOT NULL DEFAULT 0,
                "reason" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "audit_logs" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER,
                "action" TEXT NOT NULL,
                "resource_type" TEXT NOT NULL,
                "resource_id" TEXT NOT NULL,
                "metadata" TEXT,
                "ip_address" TEXT,
                "user_agent" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE SET NULL
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "forgot_passwords" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "code_hash" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "email_verifications" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "user_id" INTEGER NOT NULL,
                "code_hash" TEXT NOT NULL,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "post_comments" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "post_id" INTEGER NOT NULL,
                "user_id" INTEGER NOT NULL,
                "content" TEXT NOT NULL,
                "likes_count" INTEGER NOT NULL DEFAULT 0,
                "hidden" INTEGER NOT NULL DEFAULT 0,
                "flags_count" INTEGER NOT NULL DEFAULT 0,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                "updated_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("post_id") REFERENCES "posts" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
            r#"CREATE TABLE IF NOT EXISTS "comment_flags" (
                "id" INTEGER PRIMARY KEY AUTOINCREMENT,
                "comment_id" INTEGER NOT NULL,
                "user_id" INTEGER NOT NULL,
                "reason" TEXT,
                "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY ("comment_id") REFERENCES "post_comments" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY ("user_id") REFERENCES "users" ("id")
                    ON UPDATE CASCADE ON DELETE CASCADE
            )"#,
        ];

        for stmt in stmts {
            manager
                .get_connection()
                .execute_unprepared(*stmt)
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let drops: &[&str] = &[
            r#"DROP TABLE IF EXISTS "comment_flags""#,
            r#"DROP TABLE IF EXISTS "post_comments""#,
            r#"DROP TABLE IF EXISTS "email_verifications""#,
            r#"DROP TABLE IF EXISTS "forgot_passwords""#,
            r#"DROP TABLE IF EXISTS "audit_logs""#,
            r#"DROP TABLE IF EXISTS "route_status""#,
            r#"DROP TABLE IF EXISTS "app_constants""#,
            r#"DROP TABLE IF EXISTS "email_suppression""#,
            r#"DROP TABLE IF EXISTS "media_usage""#,
            r#"DROP TABLE IF EXISTS "media_variants""#,
            r#"DROP TABLE IF EXISTS "post_likes""#,
            r#"DROP TABLE IF EXISTS "post_access""#,
            r#"DROP TABLE IF EXISTS "post_views""#,
            r#"DROP TABLE IF EXISTS "post_series_posts""#,
            r#"DROP TABLE IF EXISTS "post_series""#,
            r#"DROP TABLE IF EXISTS "scheduled_posts""#,
            r#"DROP TABLE IF EXISTS "post_revisions""#,
            r#"DROP TABLE IF EXISTS "posts""#,
            r#"DROP TABLE IF EXISTS "tags""#,
            r#"DROP TABLE IF EXISTS "categories""#,
            r#"DROP TABLE IF EXISTS "user_bans""#,
            r#"DROP TABLE IF EXISTS "user_sessions""#,
            r#"DROP TABLE IF EXISTS "media""#,
            r#"DROP TABLE IF EXISTS "users""#,
        ];
        for stmt in drops {
            manager
                .get_connection()
                .execute_unprepared(*stmt)
                .await?;
        }
        Ok(())
    }
}
