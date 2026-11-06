//! Media takedown (A30). Removing a media item deletes the original and every derived
//! object from storage, purges CDN copies, then deletes the media and variant rows and
//! writes one `media_deletions` tombstone plus one audit row in a single transaction.
//! A failed storage step leaves the rows in place, so a takedown can be retried.
//!
//! `sweep` (`just media-sweep`) lists `media/` objects that no row references. Objects
//! younger than 24 h are kept, because an upload writes its object before its row. Sweep
//! is a dry run unless `apply` is set.

use std::collections::BTreeSet;
use std::time::Duration;

use aws_sdk_s3::Client as S3Client;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    Statement, TransactionTrait, Value,
};
use serde_json::json;
use tracing::{info, warn};

use crate::db::sea_models::media::url::build_public_file_url;
use crate::db::sea_models::{media, media_variant};
use crate::error::{DbResult, ErrorCode, ErrorResponse};

use super::audit::{self, AuditAction, AuditEvent};

pub const TOMBSTONE_RETENTION_CLASS: &str = "media_tombstone_12m";
const TOMBSTONE_RETENTION_SECS: i64 = 365 * 24 * 60 * 60;
pub const SWEEP_GRACE_SECS: i64 = 24 * 60 * 60;
pub const MEDIA_KEY_PREFIX: &str = "media/";
/// Keys are UUID v4 per upload and never reused, so a cached copy can be immutable.
pub const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";
const DELETE_ATTEMPTS: u32 = 3;
const PURGE_BATCH: usize = 30;
const CLOUDFLARE_API: &str = "https://api.cloudflare.com/client/v4";

/// Base key shared by an original and its derived objects, e.g.
/// `media/2026/11/<uuid>` for `media/2026/11/<uuid>@480w.webp`.
pub fn base_key(object_key: &str) -> &str {
    let file_start = object_key.rfind('/').map_or(0, |idx| idx + 1);
    let cut = object_key[file_start..]
        .find(['.', '@'])
        .map_or(object_key.len(), |idx| file_start + idx);
    &object_key[..cut]
}

/// True when `key` is the original (`base`) or one of its derived objects. A different
/// upload whose UUID merely starts with the same characters does not match.
pub fn belongs_to_base(base: &str, key: &str) -> bool {
    key.strip_prefix(base)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(['.', '@']))
}

/// Object keys of every variant row for one media item.
pub async fn variant_keys(db: &DatabaseConnection, media_id: i32) -> DbResult<Vec<String>> {
    let rows = media_variant::Entity::find()
        .filter(media_variant::Column::MediaId.eq(media_id))
        .all(db)
        .await
        .map_err(ErrorResponse::from)?;
    Ok(rows.into_iter().map(|row| row.object_key).collect())
}

/// Every object a takedown removes: the original, each variant row, and any derived
/// object found by listing under the base key (covers keys with no row left behind).
pub async fn takedown_keys(
    client: &S3Client,
    bucket: &str,
    object_key: &str,
    row_keys: &[String],
) -> Result<BTreeSet<String>, ErrorResponse> {
    let base = base_key(object_key);
    let mut keys: BTreeSet<String> = row_keys.iter().cloned().collect();
    keys.insert(object_key.to_string());
    for (key, _) in list_objects(client, bucket, base).await? {
        if belongs_to_base(base, &key) {
            keys.insert(key);
        }
    }
    Ok(keys)
}

/// Delete each object, retrying transient failures. Stops at the first key that still
/// fails after the last attempt; deletes are idempotent, so a retry is safe.
pub async fn delete_objects(
    client: &S3Client,
    bucket: &str,
    keys: &BTreeSet<String>,
) -> Result<(), ErrorResponse> {
    for key in keys {
        delete_object(client, bucket, key).await?;
    }
    Ok(())
}

async fn delete_object(client: &S3Client, bucket: &str, key: &str) -> Result<(), ErrorResponse> {
    let mut attempt: u32 = 1;
    loop {
        match client.delete_object().bucket(bucket).key(key).send().await {
            Ok(_) => return Ok(()),
            Err(err) if attempt < DELETE_ATTEMPTS => {
                warn!(attempt, key, error = %err, "media object delete failed; retrying");
                tokio::time::sleep(Duration::from_millis(200 * u64::from(attempt))).await;
                attempt += 1;
            }
            Err(err) => {
                return Err(ErrorResponse::new(ErrorCode::FileDeletionError)
                    .with_message("Failed to delete media from storage")
                    .with_details(err.to_string()));
            }
        }
    }
}

/// Purge CDN copies of the removed keys. Needs `CDN_PURGE_ZONE_ID` and
/// `CDN_PURGE_API_TOKEN` (owner config). Without them the purge is skipped and logged,
/// so edge copies may outlive the takedown until the owner configures it.
pub async fn purge_cdn(
    http: &reqwest::Client,
    public_url: &str,
    bucket: &str,
    keys: &BTreeSet<String>,
) -> Result<(), ErrorResponse> {
    let zone = env_value("CDN_PURGE_ZONE_ID");
    let token = env_value("CDN_PURGE_API_TOKEN");
    let (Some(zone), Some(token)) = (zone, token) else {
        warn!("CDN purge not configured (CDN_PURGE_ZONE_ID, CDN_PURGE_API_TOKEN); edge copies may outlive the takedown");
        return Ok(());
    };
    let urls: Vec<String> = keys
        .iter()
        .map(|key| build_public_file_url(public_url, Some(bucket), key))
        .collect();
    for batch in urls.chunks(PURGE_BATCH) {
        let response = http
            .post(format!("{CLOUDFLARE_API}/zones/{zone}/purge_cache"))
            .bearer_auth(&token)
            .json(&json!({ "files": batch }))
            .send()
            .await
            .map_err(|err| {
                ErrorResponse::new(ErrorCode::ServiceUnavailable)
                    .with_message("CDN purge failed")
                    .with_details(err.to_string())
            })?;
        if !response.status().is_success() {
            return Err(ErrorResponse::new(ErrorCode::ServiceUnavailable)
                .with_message("CDN purge was refused")
                .with_details(response.status().to_string()));
        }
    }
    Ok(())
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Delete the media and variant rows, insert the tombstone and the audit row, in one
/// transaction. Any failure rolls all of it back.
pub async fn remove_rows(
    db: &DatabaseConnection,
    media: &media::Model,
    object_keys: &BTreeSet<String>,
    actor_id: i32,
) -> DbResult<()> {
    let txn = db.begin().await.map_err(ErrorResponse::from)?;
    let now = Utc::now().timestamp();
    let keys_json = json!(object_keys).to_string();

    txn.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "DELETE FROM media_variants WHERE media_id = ?",
        vec![Value::from(media.id)],
    ))
    .await
    .map_err(ErrorResponse::from)?;

    let removed = txn
        .execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM media WHERE id = ?",
            vec![Value::from(media.id)],
        ))
        .await
        .map_err(ErrorResponse::from)?;
    if removed.rows_affected() != 1 {
        return Err(
            ErrorResponse::new(ErrorCode::FileNotFound).with_message("Media record not found")
        );
    }

    let values: Vec<Value> = vec![
        Value::from(media.id),
        Value::from(media.bucket.clone()),
        Value::from(keys_json),
        Value::from(media.uploader_id),
        Value::from(actor_id),
        Value::from(now),
        Value::from(now + TOMBSTONE_RETENTION_SECS),
        Value::from(TOMBSTONE_RETENTION_CLASS.to_string()),
    ];
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO media_deletions (media_id, bucket, object_keys, uploader_id, deleted_by, deleted_at, expires_at, retention_class) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        values,
    ))
    .await
    .map_err(ErrorResponse::from)?;

    audit::record(
        &txn,
        AuditEvent {
            actor_id: Some(actor_id),
            action: AuditAction::MediaDelete,
            resource_id: media.id.to_string(),
            metadata: json!({ "uploaderId": media.uploader_id, "objects": object_keys.len() }),
        },
    )
    .await
    .map_err(ErrorResponse::from)?;

    txn.commit().await.map_err(ErrorResponse::from)?;
    info!(
        media_id = media.id,
        objects = object_keys.len(),
        "Media taken down"
    );
    Ok(())
}

/// Result of a sweep: stored objects no row references (past the grace window), and
/// row keys with no stored object. `missing` is reported only; it is never auto-fixed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SweepPlan {
    pub orphans: Vec<String>,
    pub missing: Vec<String>,
}

/// Pure sweep rule. `listed` is `(key, last_modified_epoch_secs)`. An object with no
/// timestamp is kept (`i64::MAX` never passes the grace check).
pub fn plan_sweep(listed: &[(String, i64)], referenced: &BTreeSet<String>, now: i64) -> SweepPlan {
    let stored: BTreeSet<&str> = listed.iter().map(|(key, _)| key.as_str()).collect();
    let cutoff = now - SWEEP_GRACE_SECS;
    let orphans = listed
        .iter()
        .filter(|(key, modified)| !referenced.contains(key) && *modified <= cutoff)
        .map(|(key, _)| key.clone())
        .collect();
    let missing = referenced
        .iter()
        .filter(|key| !stored.contains(key.as_str()))
        .cloned()
        .collect();
    SweepPlan { orphans, missing }
}

/// Run the sweep over the `media/` prefix. Deletes orphans only when `apply` is set, and
/// writes one `media.sweep` audit row per applied run.
pub async fn sweep(
    db: &DatabaseConnection,
    client: &S3Client,
    bucket: &str,
    apply: bool,
) -> Result<SweepPlan, ErrorResponse> {
    let listed = list_objects(client, bucket, MEDIA_KEY_PREFIX).await?;
    let referenced = referenced_keys(db, bucket).await?;
    let plan = plan_sweep(&listed, &referenced, Utc::now().timestamp());
    if apply && !plan.orphans.is_empty() {
        let orphans: BTreeSet<String> = plan.orphans.iter().cloned().collect();
        delete_objects(client, bucket, &orphans).await?;
        audit::record(
            db,
            AuditEvent {
                actor_id: None,
                action: AuditAction::MediaSweep,
                resource_id: MEDIA_KEY_PREFIX.to_string(),
                metadata: json!({ "deleted": orphans.len(), "missing": plan.missing.len() }),
            },
        )
        .await
        .map_err(ErrorResponse::from)?;
    }
    Ok(plan)
}

async fn referenced_keys(db: &DatabaseConnection, bucket: &str) -> DbResult<BTreeSet<String>> {
    let sql = "SELECT object_key AS key FROM media WHERE COALESCE(bucket, ?) = ? \
               UNION \
               SELECT v.object_key AS key FROM media_variants v \
               JOIN media m ON m.id = v.media_id WHERE COALESCE(m.bucket, ?) = ?";
    let values: Vec<Value> = vec![
        Value::from(bucket.to_string()),
        Value::from(bucket.to_string()),
        Value::from(bucket.to_string()),
        Value::from(bucket.to_string()),
    ];
    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            sql,
            values,
        ))
        .await
        .map_err(ErrorResponse::from)?;
    rows.iter()
        .map(|row| {
            row.try_get::<String>("", "key")
                .map_err(ErrorResponse::from)
        })
        .collect()
}

async fn list_objects(
    client: &S3Client,
    bucket: &str,
    prefix: &str,
) -> Result<Vec<(String, i64)>, ErrorResponse> {
    let mut found = Vec::new();
    let mut token: Option<String> = None;
    loop {
        let mut request = client.list_objects_v2().bucket(bucket).prefix(prefix);
        if let Some(next) = token.take() {
            request = request.continuation_token(next);
        }
        let page = request.send().await.map_err(|err| {
            ErrorResponse::new(ErrorCode::StorageError)
                .with_message("Failed to list media in storage")
                .with_details(err.to_string())
        })?;
        for object in page.contents() {
            if let Some(key) = object.key() {
                let modified = object.last_modified().map_or(i64::MAX, |time| time.secs());
                found.push((key.to_string(), modified));
            }
        }
        token = page.next_continuation_token().map(str::to_string);
        if token.is_none() {
            break;
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sea_models::media::NewMedia;
    use crate::db::sea_models::media_variant::NewMediaVariant;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{Database, DatabaseConnection};

    const ORIGINAL: &str = "media/2026/11/0b1f0c3e-5a6d-4e7f-8a9b-0c1d2e3f4a5b.png";
    const BASE: &str = "media/2026/11/0b1f0c3e-5a6d-4e7f-8a9b-0c1d2e3f4a5b";

    fn derived(suffix: &str) -> String {
        format!("{BASE}{suffix}")
    }

    #[test]
    fn base_key_strips_extension_and_derived_suffix() {
        assert_eq!(base_key(ORIGINAL), BASE);
        assert_eq!(base_key(&derived("@480w.webp")), BASE);
        assert_eq!(base_key(&derived("@lqip.webp")), BASE);
        assert_eq!(base_key(BASE), BASE);
    }

    #[test]
    fn takedown_plan_covers_every_derived_key_and_skips_other_uploads() {
        // Every derived label the optimizer writes (@480w, @768w, @1024w, @1600w,
        // @2048w, @lqip), and the original, must belong to the base.
        let labels = ["@480w", "@768w", "@1024w", "@1600w", "@2048w", "@lqip"];
        for label in labels {
            assert!(belongs_to_base(BASE, &derived(&format!("{label}.webp"))));
        }
        assert!(belongs_to_base(BASE, ORIGINAL));
        // Another upload whose key starts with the same characters is not this base.
        assert!(!belongs_to_base(
            BASE,
            "media/2026/11/0b1f0c3e-5a6d-4e7f-8a9b-0c1d2e3f4a5b9.png"
        ));
    }

    #[test]
    fn sweep_keeps_referenced_and_young_objects_and_flags_old_orphans() {
        let now = 1_800_000_000;
        let old = now - SWEEP_GRACE_SECS - 1;
        let young = now - 60;
        let listed = vec![
            ("media/a.png".to_string(), old),
            ("media/b.png".to_string(), old),
            ("media/c.png".to_string(), young),
            ("media/unknown.png".to_string(), i64::MAX),
        ];
        let referenced: BTreeSet<String> = ["media/b.png", "media/gone.png"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let plan = plan_sweep(&listed, &referenced, now);
        assert_eq!(plan.orphans, vec!["media/a.png".to_string()]);
        assert_eq!(plan.missing, vec!["media/gone.png".to_string()]);
    }

    #[test]
    fn sweep_after_takedown_finds_no_orphans_for_removed_item() {
        let now = 1_800_000_000;
        let old = now - SWEEP_GRACE_SECS - 1;
        let removed: Vec<(String, i64)> = [ORIGINAL.to_string(), derived("@480w.webp")]
            .into_iter()
            .map(|key| (key, old))
            .collect();
        // After the row delete nothing references the removed keys, so they are the
        // only orphans left. A full cycle sweeps them; a second cycle finds none.
        let plan = plan_sweep(&removed, &BTreeSet::new(), now);
        assert_eq!(plan.orphans.len(), 2);
        let after = plan_sweep(&[], &BTreeSet::new(), now);
        assert!(after.orphans.is_empty() && after.missing.is_empty());
    }

    async fn memory_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        Migrator::up(&db, None).await.unwrap();
        db
    }

    async fn count(db: &DatabaseConnection, sql: &str) -> i64 {
        use sea_orm::DbBackend;
        let row = db
            .query_one(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .unwrap()
            .unwrap();
        row.try_get_by_index(0).unwrap()
    }

    async fn seed_media(db: &DatabaseConnection) -> media::Model {
        let stored = media::Entity::create(
            db,
            NewMedia {
                bucket: "bucket".to_string(),
                object_key: ORIGINAL.to_string(),
                mime_type: "image/png".to_string(),
                width: Some(10),
                height: Some(10),
                size: 100,
                extension: Some("png".to_string()),
                uploader_id: None,
                reference_type: None,
                content_hash: None,
                is_optimized: true,
                optimized_at: None,
            },
        )
        .await
        .unwrap();
        media_variant::Entity::create_many(
            db,
            vec![NewMediaVariant {
                media_id: stored.id,
                object_key: derived("@480w.webp"),
                mime_type: "image/webp".to_string(),
                width: Some(5),
                height: Some(5),
                size: 10,
                extension: Some("webp".to_string()),
                quality: None,
                variant_type: "480w".to_string(),
            }],
        )
        .await
        .unwrap();
        stored
    }

    fn keys() -> BTreeSet<String> {
        [ORIGINAL.to_string(), derived("@480w.webp")]
            .into_iter()
            .collect()
    }

    #[tokio::test]
    async fn takedown_writes_tombstone_and_audit_in_the_same_transaction() {
        let db = memory_db().await;
        let stored = seed_media(&db).await;

        remove_rows(&db, &stored, &keys(), 1).await.unwrap();

        assert_eq!(count(&db, "SELECT COUNT(*) FROM media").await, 0);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM media_variants").await, 0);
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM media_deletions WHERE media_id = 1 AND retention_class = 'media_tombstone_12m' AND expires_at > deleted_at"
            )
            .await,
            1
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM audit_logs WHERE action = 'media.delete' AND resource_id = '1'"
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn failed_tombstone_rolls_back_the_row_delete() {
        let db = memory_db().await;
        let stored = seed_media(&db).await;
        db.execute_unprepared(
            "CREATE TRIGGER fail_tombstone BEFORE INSERT ON media_deletions \
             BEGIN SELECT RAISE(ABORT, 'forced'); END",
        )
        .await
        .unwrap();

        assert!(remove_rows(&db, &stored, &keys(), 1).await.is_err());

        assert_eq!(count(&db, "SELECT COUNT(*) FROM media").await, 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM media_variants").await, 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM audit_logs").await, 0);
    }
}
