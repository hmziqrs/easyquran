use std::error::Error;
use std::str::FromStr;

use clap::Parser;

use ruxlog::tui::{app::run_tui, theme::ThemeKind};

#[derive(Parser, Debug)]
#[command(name = "ruxlog_tui", about = "Ruxlog TUI (auth + tags)")]
struct Args {
    #[arg(long, default_value = "dracula")]
    theme: String,
    #[arg()]
    theme_positional: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        let _ = dotenvy::from_filename("../../.env.dev");
    }

    let args = Args::parse();
    let theme_name = args
        .theme_positional
        .first()
        .map(String::as_str)
        .unwrap_or(&args.theme);
    let theme = ThemeKind::from_str(theme_name).unwrap_or(ThemeKind::Dracula);

    if let Err(err) = run_tui(theme).await {
        eprintln!("Error: {}", err);
    }

    Ok(())
}
