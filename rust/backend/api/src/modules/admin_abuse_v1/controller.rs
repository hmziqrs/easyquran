use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use tracing::instrument;

use crate::db::sea_connect::get_db_url;
use crate::services::abuse_report::{self, InProcessCounters};
use crate::utils::telemetry::ingress_rejection_totals;
use crate::AppState;

/// `GET /admin/abuse/v1/summary`. Reads the live database plus this process's
/// counters. Responses are never cached.
#[instrument(skip(state, _auth))]
pub async fn get_summary(
    State(state): State<AppState>,
    _auth: crate::services::auth::AuthSession,
) -> impl IntoResponse {
    let db_path = get_db_url()
        .ok()
        .and_then(|url| abuse_report::sqlite_file_path(&url));
    let counters = InProcessCounters {
        ingress_rejections: ingress_rejection_totals(),
        rate_limited_by_path: rux_request_gate::blocked_by_path(),
    };
    let body = abuse_report::collect(
        &state.sea_db,
        db_path.as_deref(),
        Some(&counters),
        chrono::Utc::now(),
    )
    .await;
    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(body),
    )
}
