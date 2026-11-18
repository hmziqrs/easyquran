//! Product analytics (PARITY N2), the web's `lib/boot/analytics.ts` +
//! `lib/firebase/analytics.ts` over `nsh-firebase-analytics`.
//!
//! - Off until consent: nothing leaves the device unless
//!   [`ConsentStore::analytics_allowed`] (preference on, not a minor, auth
//!   resolved). The check runs on every event and before every send; a
//!   revoked consent stops queued events too (the client's consent gate).
//! - Reader paths never reach analytics: every reader and zikr route is
//!   reported as the coarse screen `reader`, a card detail as
//!   `yours/cards` ([`screen_view`], the web's `scrubReaderPath`).
//! - No device id, continuity hint, device key, session or CSRF value in a
//!   payload (docs/plans/native-shared.md N5): [`scrub`] drops such keys.
//!   The client id GA4 needs is the library's own random analytics id.
//! - Transport: the pure tier's GA4 Measurement Protocol, built only when
//!   the app is compiled with `EQ_GA_MEASUREMENT_ID` and `EQ_GA_API_SECRET`
//!   (`native/README.md`). Without them events stay in the local sink.
//!
//! Every event is also kept in the in-memory sink ([`Analytics::events`]),
//! which never sends anything.

use std::sync::{Arc, LazyLock, Mutex};

use nsh_core::{prelude::*, storage::Storage};
use nsh_firebase_analytics::{
    Analytics as Remote, AnalyticsConfig, Consent, ConsentStatus, KeyValueStorage,
};
use nsh_firebase_core::{FirebaseOptions, app::initialize_app};
use serde_json::{Map, Value};

use crate::foundation::time;
use crate::stores::consent::ConsentStore;

/// One tracked event, as the sink keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalyticsEvent {
    /// The event's name: `"sign_in"`, `"like_toggled"`.
    pub name: String,
    /// Its properties, a JSON object (empty when the event carries none).
    pub properties: Value,
    /// When it was tracked (`foundation::time::now_ms`).
    pub at_ms: f64,
}

/// Where tracked events go.
pub trait Analytics: Send + Sync {
    /// Tracks one event. Never fails and never blocks the caller: losing
    /// telemetry is fine, losing a tap is not.
    fn track(&self, event: &str, properties: Value);

    /// The events tracked so far, oldest first.
    fn events(&self) -> Vec<AnalyticsEvent>;

    /// Forgets every event (a test's reset, a screen's "clear").
    fn clear(&self);
}

/// The in-memory sink: bounded, thread-safe, entirely local.
#[derive(Debug, Default)]
pub struct MemoryAnalytics {
    events: Mutex<Vec<AnalyticsEvent>>,
}

/// The most events the sink keeps; older ones drop from the front.
const MEMORY_MAX: usize = 500;

impl Analytics for MemoryAnalytics {
    fn track(&self, event: &str, properties: Value) {
        let tracked = AnalyticsEvent {
            name: event.to_owned(),
            properties,
            at_ms: time::now_ms(),
        };
        let mut events = self.events.lock().expect("analytics events");
        events.push(tracked);
        if events.len() > MEMORY_MAX {
            events.remove(0);
        }
    }

    fn events(&self) -> Vec<AnalyticsEvent> {
        self.events.lock().expect("analytics events").clone()
    }

    fn clear(&self) {
        self.events.lock().expect("analytics events").clear();
    }
}

/// The app's shared local sink.
pub fn sink() -> Arc<dyn Analytics> {
    static SINK: LazyLock<Arc<dyn Analytics>> =
        LazyLock::new(|| Arc::new(MemoryAnalytics::default()));
    SINK.clone()
}

const MEASUREMENT_ID: Option<&str> = option_env!("EQ_GA_MEASUREMENT_ID");
const API_SECRET: Option<&str> = option_env!("EQ_GA_API_SECRET");
const PROJECT_ID: Option<&str> = option_env!("EQ_FIREBASE_PROJECT_ID");
const MP_BASE: &str = "https://www.google-analytics.com";

/// The GA4 client id persists in NSH storage, so one install counts once.
struct NshStorage;

impl KeyValueStorage for NshStorage {
    fn get(&self, key: &str) -> Option<String> {
        Storage::get_item(key)
    }

    fn set(&self, key: &str, value: &str) {
        let _ = Storage::set_item(key, value);
    }
}

fn configured(value: Option<&'static str>) -> Option<&'static str> {
    value.map(str::trim).filter(|v| !v.is_empty())
}

fn build_remote() -> Option<Remote> {
    let measurement_id = configured(MEASUREMENT_ID)?;
    let api_secret = configured(API_SECRET)?;
    let options = FirebaseOptions {
        project_id: configured(PROJECT_ID).unwrap_or("easyquran").to_owned(),
        ..FirebaseOptions::default()
    };
    let app = initialize_app(options, "[DEFAULT]").ok()?;
    let transport = Arc::new(nsh_firebase_analytics::MeasurementProtocolTransport::new(
        MP_BASE,
        api_secret,
        measurement_id,
    ));
    let remote = Remote::with_parts(
        &app,
        transport,
        Arc::new(nsh_firebase_analytics::SystemClock),
        Arc::new(NshStorage),
        AnalyticsConfig::default(),
    )
    .ok()?;
    // Denied until the first consented event says otherwise.
    remote.set_consent(consent(false));
    Some(remote)
}

thread_local! {
    /// Engine thread only: the client reads NSH storage when built and
    /// sends through the engine's `fetch`.
    static REMOTE: Option<Arc<Remote>> = build_remote().map(Arc::new);
}

fn remote() -> Option<Arc<Remote>> {
    REMOTE.with(Clone::clone)
}

fn consent(granted: bool) -> Consent {
    let status = if granted {
        ConsentStatus::Granted
    } else {
        ConsentStatus::Denied
    };
    Consent {
        analytics_storage: status,
        ..Consent::default()
    }
}

/// Whether analytics may leave the device now (untracked read).
fn allowed() -> bool {
    untracked(|| ConsentStore::get().analytics_allowed())
}

/// Parameter keys that could carry a device, account or session secret.
fn forbidden_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["device", "hint", "session", "csrf", "token", "secret"]
        .iter()
        .any(|word| key.contains(word))
        || key == "key"
        || key.ends_with("_key")
        || key.ends_with("key_id")
}

/// The properties without any forbidden key (N5).
pub fn scrub(properties: Value) -> Value {
    match properties {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(k, _)| !forbidden_key(k))
                .collect::<Map<String, Value>>(),
        ),
        _ => Value::Object(Map::new()),
    }
}

/// Sends what is due, consent permitting. Engine thread.
fn pump(remote: Arc<Remote>) {
    spawn(async move {
        remote.set_consent(consent(allowed()));
        let now = u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0);
        let _ = remote.poll(now).await;
    });
}

/// Tracks one event: kept locally, and sent only with consent. Call on the
/// engine thread (feature code does).
pub fn track(event: &str, properties: Value) {
    let properties = scrub(properties);
    sink().track(event, properties.clone());
    let Some(remote) = remote() else {
        return;
    };
    let granted = allowed();
    remote.set_consent(consent(granted));
    if !granted {
        return;
    }
    let _ = remote.log_event(event, Some(properties));
    pump(remote);
    crate::services::monitoring::report_pending_crashes();
}

/// The web's `scrubReaderPath`: reader and zikr routes are the coarse
/// `/reader`; a card detail is `/yours/cards`.
pub fn scrub_path(path: &str) -> String {
    if matches!(
        eq_core::routes::parse_path(path),
        Some(eq_core::routes::Link::Reader(_) | eq_core::routes::Link::Zikr { .. })
    ) {
        return "/reader".to_owned();
    }
    let trimmed = path.trim_end_matches('/');
    if let Some(rest) = trimmed.strip_prefix("/yours/cards/")
        && !rest.is_empty()
        && !rest.contains('/')
    {
        return "/yours/cards".to_owned();
    }
    path.to_owned()
}

/// A screen view (`pageView`): the shell calls it on navigation with the
/// route's path.
pub fn screen_view(path: &str) {
    let reported = scrub_path(path);
    let screen = match reported.trim_matches('/') {
        "" => "home".to_owned(),
        other => other.to_owned(),
    };
    sink().track("screen_view", serde_json::json!({ "screen_name": screen }));
    let Some(remote) = remote() else {
        return;
    };
    let granted = allowed();
    remote.set_consent(consent(granted));
    if !granted {
        return;
    }
    let _ = remote.log_screen_view(&screen, Some(&screen));
    pump(remote);
    crate::services::monitoring::report_pending_crashes();
}

/// `logException`: crash reporting's event (`description`, `fatal`).
pub fn exception(description: &str, fatal: bool) {
    track(
        "exception",
        serde_json::json!({ "description": description, "fatal": fatal }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_session_keys_never_leave() {
        let out = scrub(serde_json::json!({
            "device_id": "d", "deviceHint": "h", "x-eq-session": "s", "csrf": "c",
            "device_key": "k", "fcm_token": "t", "screen": "home", "fatal": true,
        }));
        assert_eq!(out, serde_json::json!({ "screen": "home", "fatal": true }));
    }
}
