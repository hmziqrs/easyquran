//! Incident capture (A25). `incident_capture <report-id>` copies the comment behind
//! an intake report into `case_evidence` and writes one `incident.capture` audit
//! row. It writes the live database, so run it through `just incident capture`,
//! which uses the same connection settings as `seed_now`.

use chrono::Utc;
use ruxlog::db::sea_connect::try_connect;
use ruxlog::services::incident_capture;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        let _ = dotenvy::from_filename("../../.env.dev");
    }

    let Some(report_id) = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<i64>().ok())
    else {
        eprintln!("usage: incident_capture <report-id>");
        std::process::exit(2);
    };

    let db = try_connect(true)
        .await
        .map_err(|e| format!("database connect failed: {e}"))?;
    match incident_capture::capture_report(&db, report_id, Utc::now()).await {
        Ok(outcome) => {
            println!(
                "[incident] case {} captured {} rows",
                outcome.case_ref, outcome.rows
            );
            Ok(())
        }
        Err(err) => {
            eprintln!("[incident] capture failed: {err:?}");
            std::process::exit(1);
        }
    }
}
