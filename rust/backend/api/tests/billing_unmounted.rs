// Billing stays dark: no routes, and readers never touch its missing tables.
mod reading_common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use reading_common::{harness, send};
use ruxlog::modules::post_v1;

#[tokio::test]
async fn authenticated_published_list_works_without_billing_tables() {
    let h = harness(post_v1::routes()).await;
    let body = json!({});
    let (status, _) = send(
        &h.app,
        "POST",
        "/list/published",
        Some(&h.cookie_a),
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn billing_routes_are_not_mounted() {
    let state = reading_common::state().await;
    let app = ruxlog::router::router(state.clone()).with_state(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/billing/v1/plans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn router_source_never_mentions_billing() {
    let src = include_str!("../src/router.rs");
    assert_eq!(src.matches("billing").count(), 0);
}
