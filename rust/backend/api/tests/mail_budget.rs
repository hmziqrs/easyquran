// Mail budget and newsletter unsubscribe (A29) over HTTP: the RFC 8058 one-click
// endpoint answers 200 for every input, and only the right token for a live row
// unsubscribes it. The router-level caps, bounce guard and suppression rules are
// unit-tested in services/mail/router.rs.
mod reading_common;

use axum::http::StatusCode;

use reading_common::{harness, send, Harness};
use ruxlog::db::sea_models::newsletter_subscriber::{
    Entity as Subscriber, NewSubscriber, SubscriberStatus,
};
use ruxlog::modules::newsletter_v1;
use ruxlog::utils::code_hash::hash_code;
use sea_orm::EntityTrait;

async fn confirmed_subscriber(h: &Harness, email: &str) -> i32 {
    let row = Subscriber::create(
        &h.state.sea_db,
        NewSubscriber {
            email: email.to_string(),
            status: SubscriberStatus::Confirmed,
            token: "stored-hash-placeholder".to_string(),
        },
    )
    .await
    .expect("subscriber inserted");
    row.id
}

fn one_click_token(h: &Harness, id: i32) -> String {
    hash_code(&h.state.secret_key, &format!("newsletter-unsubscribe:{id}"))
}

async fn status_of(h: &Harness, id: i32) -> SubscriberStatus {
    Subscriber::find_by_id(id)
        .one(&h.state.sea_db)
        .await
        .unwrap()
        .unwrap()
        .status
}

#[tokio::test]
async fn one_click_unsubscribe_answers_200_for_unknown_and_wrong_tokens() {
    let h = harness(axum::Router::new().nest("/newsletter/v1", newsletter_v1::routes())).await;
    let id = confirmed_subscriber(&h, "reader@one-click.test").await;

    let uri = format!("/newsletter/v1/unsubscribe/{id}/not-the-token");
    let (status, _) = send(&h.app, "POST", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(status_of(&h, id).await, SubscriberStatus::Confirmed);

    let (status, body) = send(
        &h.app,
        "POST",
        "/newsletter/v1/unsubscribe/999999/whatever",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "unknown id still 200");
    assert!(body.get("message").is_some());

    let (status, _) = send(
        &h.app,
        "POST",
        "/newsletter/v1/unsubscribe/abc/x",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "non-numeric id still 200");
}

#[tokio::test]
async fn one_click_unsubscribe_with_right_token_unsubscribes() {
    let h = harness(axum::Router::new().nest("/newsletter/v1", newsletter_v1::routes())).await;
    let id = confirmed_subscriber(&h, "leaving@one-click.test").await;
    let token = one_click_token(&h, id);

    let uri = format!("/newsletter/v1/unsubscribe/{id}/{token}");
    let (status, _) = send(&h.app, "POST", &uri, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(status_of(&h, id).await, SubscriberStatus::Unsubscribed);
}
