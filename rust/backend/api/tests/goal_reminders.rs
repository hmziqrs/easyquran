// Goal reminders (F9): the schedule routes, the refusals, and the due run. A
// reminder fires at local time, once a day, holds through quiet hours, goes to the
// newest device only, and stops when switched off.
mod reading_common;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use chrono::{DateTime, TimeZone, Utc};
use migration::{Migrator, MigratorTrait};
use rux_auth::AuthSession as GenAuthSession;
use sea_orm::{ConnectionTrait, EntityTrait};
use serde_json::{json, Value};

use reading_common::{router_with_state, seed_user, send, state};
use ruxlog::db::sea_models::device::{self, NewDevice};
use ruxlog::db::sea_models::reminder_schedule::{self, NewSchedule};
use ruxlog::db::sea_models::user::Model as User;
use ruxlog::db::sea_models::user_session::{self, NewUserSession};
use ruxlog::modules::notification_v1;
use ruxlog::services::auth::AuthBackend;
use ruxlog::services::reminders::{self, SendError};
use ruxlog::state::AppState;

fn at(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 6, h, m, 0).unwrap()
}

/// Same login path as the device registry tests: a real session bound to an audit row.
async fn bound_login(state: &AppState, u: &User) -> String {
    let backend = AuthBackend::new(
        &state.sea_db,
        state.session_store.clone(),
        state.revoked_sessions.clone(),
    );
    let session = tower_sessions::Session::new(None, state.session_store.clone(), None);
    let mut auth = GenAuthSession::new(backend, session).await;
    auth.login(u).await.expect("login succeeds");
    auth.session().save().await.expect("session persisted");
    let tower_sid = auth
        .session()
        .id()
        .expect("session id after save")
        .to_string();
    let audit = user_session::Entity::create(&state.sea_db, NewUserSession::new(u.id, None, None))
        .await
        .expect("audit row");
    auth.backend()
        .record_session_mapping(&state.sea_db, audit.id, &tower_sid)
        .await
        .expect("binding recorded");
    drop(auth);

    let cookie_key = tower_sessions::cookie::Key::derive_from(state.settings.cookie_key.as_bytes());
    let mut jar = tower_sessions::cookie::CookieJar::new();
    jar.private_mut(&cookie_key)
        .add(tower_sessions::cookie::Cookie::new("ruxlog.sid", tower_sid));
    jar.get("ruxlog.sid")
        .expect("forged session cookie present")
        .to_string()
}

async fn setup() -> (AppState, axum::Router, User, String) {
    let state = state().await;
    Migrator::up(&state.sea_db, None).await.expect("migrations");
    let u = seed_user(&state, "reminder").await;
    let cookie = bound_login(&state, &u).await;
    let app = router_with_state(state.clone(), notification_v1::routes());
    (state, app, u, cookie)
}

async fn add_device(state: &AppState, user_id: i32, token: &str, last_seen: &str) {
    device::Entity::upsert(
        &state.sea_db,
        NewDevice {
            user_id,
            token: token.into(),
            platform: "web".into(),
            device_id: None,
        },
    )
    .await
    .expect("device registered");
    // Fixed last_seen values so "newest device" is deterministic.
    state
        .sea_db
        .execute_unprepared(&format!(
            "UPDATE devices SET last_seen_at = '{last_seen}' WHERE token = '{token}'"
        ))
        .await
        .expect("last_seen set");
}

async fn store(
    state: &AppState,
    user_id: i32,
    enabled: bool,
    zone: &str,
    reminder: i32,
    quiet: Option<(i32, i32)>,
) {
    reminder_schedule::Entity::upsert(
        &state.sea_db,
        NewSchedule {
            user_id,
            kind: "goal".into(),
            enabled,
            time_zone: zone.into(),
            reminder_minute: reminder,
            quiet_start_minute: quiet.map(|q| q.0),
            quiet_end_minute: quiet.map(|q| q.1),
        },
    )
    .await
    .expect("schedule stored");
}

/// A send stub that records each token it was asked to push to, and answers with
/// `outcome` for every call.
fn recorder(
    sent: Arc<Mutex<Vec<String>>>,
    outcome: Result<(), SendError>,
) -> impl Fn(String) -> std::future::Ready<Result<(), SendError>> {
    move |token: String| {
        sent.lock().unwrap().push(token);
        std::future::ready(outcome)
    }
}

#[tokio::test]
async fn schedule_routes_store_read_and_unsubscribe() {
    let (_state, app, _u, cookie) = setup().await;

    let (status, body) = send(
        &app,
        "POST",
        "/reminder/get",
        Some(&cookie),
        Some(&json!({"kind": "goal"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["reminder"].is_null(), "no schedule yet");

    let set = json!({
        "kind": "goal",
        "enabled": true,
        "time_zone": "Asia/Karachi",
        "reminder_minute": 20 * 60,
        "quiet_start_minute": 22 * 60,
        "quiet_end_minute": 7 * 60,
    });
    let (status, body) = send(&app, "POST", "/reminder/set", Some(&cookie), Some(&set)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reminder"]["enabled"], true);
    assert_eq!(body["reminder"]["time_zone"], "Asia/Karachi");

    // Unsubscribe is enabled=false. The row and its times stay.
    let off = json!({ "kind": "goal", "enabled": false, "time_zone": "Asia/Karachi", "reminder_minute": 20 * 60 });
    let (status, _) = send(&app, "POST", "/reminder/set", Some(&cookie), Some(&off)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(
        &app,
        "POST",
        "/reminder/get",
        Some(&cookie),
        Some(&json!({"kind": "goal"})),
    )
    .await;
    assert_eq!(body["reminder"]["enabled"], false);
    assert_eq!(body["reminder"]["reminder_minute"], 20 * 60);
}

#[tokio::test]
async fn schedule_routes_refuse_bad_input_and_signed_out_callers() {
    let (_state, app, _u, cookie) = setup().await;
    let base = |zone: &str, reminder: i64, qs: Value, qe: Value| {
        json!({
            "kind": "goal",
            "enabled": true,
            "time_zone": zone,
            "reminder_minute": reminder,
            "quiet_start_minute": qs,
            "quiet_end_minute": qe,
        })
    };
    let refused = [
        base("Nope/Zone", 60, Value::Null, Value::Null),
        base("UTC", 1440, Value::Null, Value::Null),
        base("UTC", 60, json!(0), Value::Null),
        base("UTC", 60, json!(30), json!(30)),
        // Reminder 08:00 inside quiet 07:00 to 09:00.
        base("UTC", 480, json!(420), json!(540)),
        base("UTC", 60, json!(-1), json!(120)),
    ];
    for body in refused {
        let (status, _) = send(&app, "POST", "/reminder/set", Some(&cookie), Some(&body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "refused: {body}");
    }
    // Only the goal kind exists.
    let other =
        json!({"kind": "streak", "enabled": true, "time_zone": "UTC", "reminder_minute": 60});
    let (status, _) = send(&app, "POST", "/reminder/set", Some(&cookie), Some(&other)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let signed_out =
        json!({"kind": "goal", "enabled": true, "time_zone": "UTC", "reminder_minute": 60});
    let (status, _) = send(&app, "POST", "/reminder/set", None, Some(&signed_out)).await;
    assert!(
        status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN,
        "signed-out refused, got {status}"
    );
}

#[tokio::test]
async fn fires_at_local_time_once_a_day() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-a", "2026-11-06 10:00:00.000000+00:00").await;
    // 20:00 in Karachi (UTC+5) is 15:00 UTC.
    store(&state, u.id, true, "Asia/Karachi", 20 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let s = reminders::run_due(&state.sea_db, at(14, 59), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!((s.sent, s.skipped), (0, 1), "not yet at 14:59 UTC");

    let s = reminders::run_due(&state.sea_db, at(15, 0), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.sent, 1, "fires at 15:00 UTC, which is 20:00 local");

    let s = reminders::run_due(&state.sea_db, at(16, 0), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.sent, 0, "already fired today");

    let next_day = Utc.with_ymd_and_hms(2026, 11, 7, 15, 0, 0).unwrap();
    let s = reminders::run_due(&state.sea_db, next_day, recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.sent, 1, "fires again the next local day");
    assert_eq!(*sent.lock().unwrap(), vec!["tok-a", "tok-a"]);
}

#[tokio::test]
async fn quiet_hours_hold_the_reminder_until_they_end() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-q", "2026-11-06 10:00:00.000000+00:00").await;
    // Reminder 06:00, quiet 07:00 to 09:00, all UTC.
    store(&state, u.id, true, "UTC", 6 * 60, Some((7 * 60, 9 * 60))).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let s = reminders::run_due(&state.sea_db, at(7, 30), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.sent, 0, "inside quiet hours");

    let s = reminders::run_due(&state.sea_db, at(9, 5), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.sent, 1, "quiet hours ended, the held reminder goes out");
}

#[tokio::test]
async fn unsubscribe_stops_the_reminder() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-u", "2026-11-06 10:00:00.000000+00:00").await;
    store(&state, u.id, false, "UTC", 9 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let s = reminders::run_due(&state.sea_db, at(12, 0), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(s.considered, 0, "disabled schedules are not read");
    assert!(sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn only_the_most_recently_seen_device_is_pushed() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-old", "2026-11-05 10:00:00.000000+00:00").await;
    add_device(&state, u.id, "tok-new", "2026-11-06 08:00:00.000000+00:00").await;
    store(&state, u.id, true, "UTC", 9 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    reminders::run_due(&state.sea_db, at(9, 0), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(*sent.lock().unwrap(), vec!["tok-new"]);
}

#[tokio::test]
async fn unregistered_token_is_pruned_and_the_next_device_gets_it() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-gone", "2026-11-06 08:00:00.000000+00:00").await;
    add_device(
        &state,
        u.id,
        "tok-alive",
        "2026-11-05 08:00:00.000000+00:00",
    )
    .await;
    store(&state, u.id, true, "UTC", 9 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let first = reminders::run_due(
        &state.sea_db,
        at(9, 0),
        recorder(sent.clone(), Err(SendError::Unregistered)),
    )
    .await
    .unwrap();
    assert_eq!(first.pruned, 1);
    let remaining = device::Entity::find().all(&state.sea_db).await.unwrap();
    assert_eq!(remaining.len(), 1, "stale device row removed");
    assert_eq!(remaining[0].token, "tok-alive");

    // The claim was released, so the same day still gets a reminder.
    let second = reminders::run_due(&state.sea_db, at(9, 1), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(second.sent, 1);
    assert_eq!(*sent.lock().unwrap(), vec!["tok-gone", "tok-alive"]);
}

#[tokio::test]
async fn failed_push_is_retried_the_same_day() {
    let (state, _app, u, _cookie) = setup().await;
    add_device(&state, u.id, "tok-f", "2026-11-06 08:00:00.000000+00:00").await;
    store(&state, u.id, true, "UTC", 9 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let first = reminders::run_due(
        &state.sea_db,
        at(9, 0),
        recorder(sent.clone(), Err(SendError::Failed)),
    )
    .await
    .unwrap();
    assert_eq!(first.failed, 1);

    let second = reminders::run_due(&state.sea_db, at(9, 1), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(second.sent, 1, "claim released after a failed send");
}

#[tokio::test]
async fn no_device_means_no_claim_so_a_later_device_still_gets_today() {
    let (state, _app, u, _cookie) = setup().await;
    store(&state, u.id, true, "UTC", 9 * 60, None).await;
    let sent = Arc::new(Mutex::new(Vec::new()));

    let first = reminders::run_due(&state.sea_db, at(9, 0), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(first.no_device, 1);

    add_device(&state, u.id, "tok-late", "2026-11-06 09:30:00.000000+00:00").await;
    let second = reminders::run_due(&state.sea_db, at(9, 45), recorder(sent.clone(), Ok(())))
        .await
        .unwrap();
    assert_eq!(second.sent, 1);
}

#[tokio::test]
async fn schedule_row_keeps_account_lifetime_retention() {
    let (state, _app, u, _cookie) = setup().await;
    store(&state, u.id, true, "UTC", 9 * 60, None).await;
    let row = reminder_schedule::Entity::find_for(&state.sea_db, u.id, "goal")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.expires_at, reminder_schedule::NEVER_EXPIRES);
    assert_eq!(row.retention_class, "account_lifetime");
    assert!(row.last_fired_day.is_none());
}
