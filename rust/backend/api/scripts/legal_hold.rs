//! Legal holds (A25) from the shell. Run through `just legal-hold`. Writes go through
//! `services::legal_hold`, so each place and renew writes its `legal_hold.*` audit row (actor
//! null: operator CLI). Validation is shared with the admin route.
//!
//!   just legal-hold list
//!   just legal-hold place --rule enforcement_12m --authority "..." --reason "..." \
//!       --owner "..." [--case-ref X] --subject user:42 [--subject media:9]
//!   just legal-hold renew --id 3

use ruxlog::db::sea_connect::try_connect;
use ruxlog::modules::admin_legal_hold_v1::dto::{
    validate_place, HoldSubjectPayload, PlaceHoldPayload,
};
use ruxlog::services::legal_hold;

const USAGE: &str = "usage: legal_hold list | place --rule <class> --authority <t> --reason <t> \
--owner <t> [--case-ref <t>] --subject <kind:id>... | renew --id <n>";

fn flag(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn flags(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}

fn parse_subject(raw: &str) -> Result<HoldSubjectPayload, String> {
    let (kind, id) = raw
        .split_once(':')
        .ok_or_else(|| format!("subject must be kind:id, got {raw}"))?;
    Ok(HoldSubjectPayload {
        kind: kind.to_string(),
        id: id.to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    if std::env::var("DATABASE_URL").is_err() {
        let _ = dotenvy::from_filename("../../.env.dev");
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let db = try_connect(true)
        .await
        .map_err(|e| format!("database connect failed: {e}"))?;

    match args.first().map(String::as_str) {
        Some("list") => {
            let views = legal_hold::list(&db, 200)
                .await
                .map_err(|e| format!("{e:?}"))?;
            for view in views {
                let subjects: Vec<String> = view
                    .subjects
                    .iter()
                    .map(|s| format!("{}:{}", s.subject_kind, s.subject_id))
                    .collect();
                println!(
                    "hold {} class={} expires_at={} released_at={:?} renewals={} subjects={}",
                    view.hold.id,
                    view.hold.retention_class,
                    view.hold.expires_at,
                    view.hold.released_at,
                    view.hold.renewals,
                    subjects.join(",")
                );
            }
        }
        Some("place") => {
            let subjects = flags(&args, "--subject")
                .iter()
                .map(|raw| parse_subject(raw))
                .collect::<Result<Vec<_>, _>>()?;
            let payload = PlaceHoldPayload {
                rule: flag(&args, "--rule").unwrap_or_default(),
                authority: flag(&args, "--authority").unwrap_or_default(),
                reason: flag(&args, "--reason").unwrap_or_default(),
                owner: flag(&args, "--owner").unwrap_or_default(),
                case_ref: flag(&args, "--case-ref"),
                subjects,
            };
            let new = validate_place(payload).map_err(|msg| msg.to_string())?;
            let hold = legal_hold::place(&db, None, new)
                .await
                .map_err(|e| format!("{e:?}"))?;
            println!(
                "placed hold {} class={} expires_at={}",
                hold.id, hold.retention_class, hold.expires_at
            );
        }
        Some("renew") => {
            let id: i64 = flag(&args, "--id").ok_or("--id is required")?.parse()?;
            let hold = legal_hold::renew(&db, None, id)
                .await
                .map_err(|e| format!("{e:?}"))?;
            println!(
                "renewed hold {} renewals={} expires_at={}",
                hold.id, hold.renewals, hold.expires_at
            );
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    Ok(())
}
