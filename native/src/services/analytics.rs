//! The analytics seam: one small trait with an in-memory sink, so the real
//! telemetry library (and the config's telemetry gate) lands in one file.
//! The local sink records everything and sends nothing anywhere.

use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::foundation::time;

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

/// The app's shared sink. Swapping the implementation means editing
/// this file's `SINK` line; call sites never change.
pub fn sink() -> Arc<dyn Analytics> {
    static SINK: std::sync::LazyLock<Arc<dyn Analytics>> =
        std::sync::LazyLock::new(|| Arc::new(MemoryAnalytics::default()));
    SINK.clone()
}

/// Tracks on the shared sink: the one-liner feature code calls. The
/// config's `telemetry_enabled` gate belongs to the real sink, which is
/// the one file this seam swaps.
pub fn track(event: &str, properties: Value) {
    sink().track(event, properties);
}
