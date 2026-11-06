//! Media sweep (A30). Lists `media/` objects that no `media` or `media_variants` row
//! references, and prints them. Dry run by default. `--apply` deletes orphans older than
//! 24 h and writes one `media.sweep` audit row. Run through `just media-sweep`.

use ruxlog::config::ObjectStorageConfig;
use ruxlog::db::sea_connect::try_connect;
use ruxlog::services::media_takedown;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        let _ = dotenvy::from_filename("../../.env.dev");
    }
    let apply = std::env::args().skip(1).any(|arg| arg == "--apply");

    let storage = ObjectStorageConfig::from_env();
    let s3_config = aws_config::from_env()
        .endpoint_url(&storage.endpoint)
        .credentials_provider(aws_sdk_s3::config::Credentials::new(
            &storage.access_key,
            &storage.secret_key,
            None,
            None,
            "S3Compatible",
        ))
        .region(aws_sdk_s3::config::Region::new(storage.region.clone()))
        .load()
        .await;
    let client = aws_sdk_s3::Client::new(&s3_config);
    let db = try_connect(true)
        .await
        .map_err(|e| format!("database connect failed: {e}"))?;

    let plan = media_takedown::sweep(&db, &client, &storage.bucket, apply)
        .await
        .map_err(|err| format!("media sweep failed: {err:?}"))?;
    for key in &plan.orphans {
        println!("orphan {key}");
    }
    for key in &plan.missing {
        println!("missing-object {key}");
    }
    println!(
        "[media-sweep] mode={} orphans={} missing={}",
        if apply { "apply" } else { "dry-run" },
        plan.orphans.len(),
        plan.missing.len()
    );
    Ok(())
}
