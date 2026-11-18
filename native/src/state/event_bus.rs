//! The app's event bus: publish and subscribe over a signal, so any part
//! of the app announces what happened and any owner listens while it
//! lives.

use std::cell::Cell;
use std::rc::Rc;

use nsh_core::element::{SharedString, Signal};
use nsh_core::prelude::*;

/// The most events the bus keeps for subscribers that read late and for
/// the diagnostics page. Older events drop from the front.
pub const EVENT_LOG_MAX: usize = 100;

/// An error entered the central error surface (`foundation::report_error`).
/// Payload: the error's message, then its code in parentheses:
/// `"Feed refused (feed.offline)"`.
pub const TOPIC_APP_ERROR: &str = "app.error";
/// The typed config was saved (`foundation::config::save`). Payload: the
/// config's storage key (`foundation::config::CONFIG_KEY`).
pub const TOPIC_CONFIG_SAVED: &str = "app.config";
/// The first-run flow completed (`foundation::first_run::complete`).
/// Payload: `"completed"`.
pub const TOPIC_FIRST_RUN: &str = "app.first-run";
/// A background task changed state (`state::tasks`). Payload: the task's
/// new state (`"running"`, `"done"` or `"cancelled"`), then a space and
/// the task's name: `"done sample"`.
pub const TOPIC_TASK: &str = "app.task";

/// One published event: what happened (`topic`), a short `payload`, and
/// when (`at_ms`, on the engine's clock).
#[derive(Clone, Debug, PartialEq)]
pub struct BusEvent {
    /// Monotonic across the app's run; subscribers follow it, so a trimmed
    /// log never replays or skips.
    pub id: u64,
    /// A documented topic (`TOPIC_APP_ERROR` and friends) or a feature's
    /// own, named `"feature.thing"`.
    pub topic: SharedString,
    /// Short and human-readable; structured data goes in the app's stores.
    pub payload: SharedString,
    /// When it was published (`foundation::time::now_ms`).
    pub at_ms: f64,
}

impl BusEvent {
    /// An event of `topic` published now.
    fn new(topic: SharedString, payload: SharedString) -> Self {
        Self {
            id: crate::foundation::id::next_id(),
            topic,
            payload,
            at_ms: crate::foundation::time::now_ms(),
        }
    }
}

/// The published events, oldest first. One per surface: `global` keeps one
/// value per type.
fn log() -> Signal<Vec<BusEvent>> {
    global::<Vec<BusEvent>>()
}

/// Publishes `payload` under `topic`. Safe from components, handlers and
/// tasks on the engine thread; each publish notifies what reads the log.
pub fn publish(topic: impl Into<SharedString>, payload: impl Into<SharedString>) {
    let event = BusEvent::new(topic.into(), payload.into());
    log().update(|log| {
        log.push(event);
        if log.len() > EVENT_LOG_MAX {
            log.remove(0);
        }
    });
}

/// The published events, oldest first (the diagnostics page shows these).
pub fn events() -> Signal<Vec<BusEvent>> {
    log()
}

/// Calls `on_event` for every event of `topic` from now on, including ones
/// published before it in the same handler. Ends with the current owner,
/// as every listener does: subscribe where an owner lives (a component's
/// body, an effect), not inside a timer callback.
pub fn subscribe(topic: &str, on_event: impl Fn(&BusEvent) + 'static) {
    let topic = SharedString::from(topic.to_owned());
    let log = log();
    let seen = Rc::new(Cell::new(
        log.with_untracked(|log| log.last().map_or(0, |event| event.id)),
    ));
    let handler = Rc::new(on_event);
    effect(move || {
        // New events are those after the last id this subscription saw;
        // ids survive the log's front trim, where indices would not.
        let from = seen.get();
        let fresh = log.with(|log| {
            log.iter()
                .filter(|event| event.id > from)
                .cloned()
                .collect::<Vec<_>>()
        });
        if let Some(last) = fresh.last() {
            seen.set(last.id);
        }
        for event in &fresh {
            if event.topic == topic {
                handler(event);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `TOPIC_APP_ERROR` renamed to `"app.errors"` — failed on its `assert_eq!`.
    #[test]
    fn topics_are_stable_names() {
        // The topics are a cross-area contract: a rename would silently
        // unsubscribe every listener.
        assert_eq!(TOPIC_APP_ERROR, "app.error");
        assert_eq!(TOPIC_CONFIG_SAVED, "app.config");
        assert_eq!(TOPIC_FIRST_RUN, "app.first-run");
        assert_eq!(TOPIC_TASK, "app.task");
    }
}
