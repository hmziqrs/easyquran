
use ruxlog::db::sea_connect::try_connect;
use ruxlog::services::seed;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        let _ = dotenvy::from_filename("../../.env.dev");
    }

    println!("[seed_now] connecting to database …");
    let db = try_connect(true)
        .await
        .map_err(|e| format!("database connect failed: {e}"))?;

    println!("[seed_now] running seed_all (this may take a moment) …");
    match seed::seed_all(&db).await {
        Ok(_) => println!("[seed_now] seed_all completed successfully."),
        Err(e) => {
            eprintln!("[seed_now] seed_all FAILED: {e:?}");
            std::process::exit(1);
        }
    }
    Ok(())
}
