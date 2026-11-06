//! Operator alerts (A26): a P0 dispatch sends mail to the operator, tries push, and
//! writes one `alert.dispatch` audit row with each channel's result. The daily digest
//! counts open rows and writes one `operator_digest` job row. Live FCM is not
//! exercised here: the test passes no FCM client.
mod reading_common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use reading_common::{harness, Harness};
use ruxlog::db::sea_models::audit_log;
use ruxlog::db::sea_models::report::{self, NewReport, ReportReason, ReportTargetKind};
use ruxlog::services::audit::AuditAction;
use ruxlog::services::mail::provider::{MailError, MailProvider, OutboundEmail, SendReceipt};
use ruxlog::services::mail::router::MailRouterLimits;
use ruxlog::services::mail::MailRouter;
use ruxlog::services::operator_alert::{self, OperatorConfig};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    Statement,
};

/// Accepts every message and keeps a copy for assertions.
#[derive(Default)]
struct AcceptingMail {
    sent: Mutex<Vec<OutboundEmail>>,
}

#[async_trait]
impl MailProvider for AcceptingMail {
    fn provider_name(&self) -> &'static str {
        "fake"
    }

    async fn send(&self, msg: OutboundEmail) -> Result<SendReceipt, MailError> {
        self.sent.lock().unwrap().push(msg);
        Ok(SendReceipt {
            delivered: 1,
            ..SendReceipt::default()
        })
    }
}

fn router_with(provider: Arc<AcceptingMail>, db: &DatabaseConnection) -> MailRouter {
    let mut providers: HashMap<String, Arc<dyn MailProvider>> = HashMap::new();
    providers.insert("fake".into(), provider);
    MailRouter::new(
        providers,
        "fake".into(),
        Arc::new(rux_request_gate::InMemoryStore::default()),
        db.clone(),
        MailRouterLimits::default(),
        false,
    )
}

async fn alert_rows(db: &DatabaseConnection) -> Vec<audit_log::model::Model> {
    audit_log::Entity::find()
        .filter(audit_log::Column::Action.eq(AuditAction::AlertDispatch.as_str()))
        .all(db)
        .await
        .unwrap()
}

async fn seed_report(db: &DatabaseConnection, reason: ReportReason) -> i64 {
    report::Entity::insert_report(
        db,
        NewReport {
            reporter_user_id: None,
            target_kind: ReportTargetKind::Comment,
            target_id: "1".into(),
            reason,
            details: None,
            contact_email: None,
            snapshot: None,
        },
    )
    .await
    .expect("report seeded")
    .id
}

async fn digest_detail(db: &DatabaseConnection) -> Option<String> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT detail FROM job_runs WHERE job = ? ORDER BY id DESC LIMIT 1",
            vec!["operator_digest".into()],
        ))
        .await
        .unwrap()?;
    row.try_get::<Option<String>>("", "detail").unwrap()
}

#[tokio::test]
async fn p0_alert_sends_mail_records_both_channels() {
    let h: Harness = harness(axum::Router::new()).await;
    let db = &h.state.sea_db;
    let provider = Arc::new(AcceptingMail::default());
    let mailer = router_with(provider.clone(), db);
    let cfg = OperatorConfig {
        alert_email: Some("abuse@easyquran.fyi".into()),
        user_id: None,
    };
    let report_id = seed_report(db, ReportReason::Csam).await;

    let (mail, push) = operator_alert::dispatch_p0(db, &mailer, None, &cfg, report_id).await;

    assert_eq!(mail, operator_alert::MailStatus::Accepted);
    assert_eq!(push, operator_alert::PushStatus::NotConfigured);

    {
        let sent = provider.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, "abuse@easyquran.fyi");
        let body = sent[0].text.clone().unwrap_or_default();
        assert!(body.contains(&report_id.to_string()));
    }

    let rows = alert_rows(db).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].resource_id, report_id.to_string());
    let meta = rows[0].metadata.clone().expect("metadata");
    assert_eq!(meta["priority"], "p0");
    assert_eq!(meta["mail"]["status"], "accepted");
    assert_eq!(meta["push"]["status"], "not_configured");
}

#[tokio::test]
async fn p0_alert_without_operator_config_records_not_configured() {
    let h: Harness = harness(axum::Router::new()).await;
    let db = &h.state.sea_db;
    let provider = Arc::new(AcceptingMail::default());
    let mailer = router_with(provider.clone(), db);
    let report_id = seed_report(db, ReportReason::Threat).await;

    let (mail, push) =
        operator_alert::dispatch_p0(db, &mailer, None, &OperatorConfig::default(), report_id).await;

    assert_eq!(mail, operator_alert::MailStatus::NotConfigured);
    assert_eq!(push, operator_alert::PushStatus::NotConfigured);
    assert!(provider.sent.lock().unwrap().is_empty());

    let rows = alert_rows(db).await;
    assert_eq!(rows.len(), 1);
    let meta = rows[0].metadata.clone().expect("metadata");
    assert_eq!(meta["mail"]["status"], "not_configured");
}

#[tokio::test]
async fn digest_counts_open_rows_and_records_job_run() {
    let h: Harness = harness(axum::Router::new()).await;
    let db = &h.state.sea_db;
    seed_report(db, ReportReason::Csam).await;
    seed_report(db, ReportReason::Spam).await;
    seed_report(db, ReportReason::Harassment).await;

    operator_alert::send_digest(&h.state).await;

    let detail = digest_detail(db).await.expect("digest job row");
    assert!(
        detail.starts_with("open_p0=1 open_p1=2 open_appeals=0 mail="),
        "detail was {detail}"
    );
}

#[test]
fn alert_action_is_registered_with_report_resource() {
    assert!(AuditAction::ALL.contains(&AuditAction::AlertDispatch));
    assert_eq!(AuditAction::AlertDispatch.as_str(), "alert.dispatch");
    assert_eq!(AuditAction::AlertDispatch.resource_type(), "report");
}
