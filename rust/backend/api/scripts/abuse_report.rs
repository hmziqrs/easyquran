//! Offline abuse report (A5). `abuse_report <db-file>` prints the summary JSON for
//! one SQLite file. Pass a WAL-safe copy made with `sqlite3 .backup`, never the
//! live file: `just abuse-report` does this for you. The file is opened read-only.
//! Process-local figures (429s by route, ingress rejections, disk cache) print as
//! "not measurable" because no api process runs here.

use std::path::PathBuf;

use ruxlog::services::abuse_report;
use sea_orm::{ConnectOptions, Database};

#[tokio::main]
async fn main() {
    let Some(path) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: abuse_report <db-file>");
        std::process::exit(2);
    };

    let mut opt = ConnectOptions::new(format!("sqlite:{}?mode=ro", path.display()));
    opt.max_connections(1);
    let db = match Database::connect(opt).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let report = abuse_report::collect(&db, Some(&path), None, chrono::Utc::now()).await;
    match serde_json::to_string_pretty(&report) {
        Ok(text) => println!("{text}"),
        Err(e) => {
            eprintln!("cannot render report: {e}");
            std::process::exit(1);
        }
    }
}
