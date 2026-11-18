//! The monitoring seam: errors and breadcrumbs in an in-memory sink
//! (local only), plus crash reporting (PARITY N3), the web's
//! `lib/boot/crash-reporting.ts`.
//!
//! Crash reporting mirrors the web: an uncaught failure (a Rust panic, the
//! web's `error` / `unhandledrejection`) becomes the analytics `exception`
//! event (`description`, `fatal: true`), sent only with analytics consent
//! (`services::analytics`). Handled errors stay local, as on the web. The
//! description never carries a session, CSRF or token value: one that
//! mentions such a word is replaced whole ([`crash_description`]).
//!
//! `nsh-firebase-crashlytics` is native-tier only (an embedding host's
//! `firebase.crashlytics.*` handlers); an `nsh build` app has none, so the
//! analytics event is the report, exactly as on the web.

use std::sync::{Arc, Mutex};

use crate::foundation::error::AppError;
use crate::foundation::time;

/// One recorded error, as the sink keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedError {
    /// The error as it was raised.
    pub error: AppError,
    /// When it was recorded (`foundation::time::now_ms`).
    pub at_ms: f64,
}

/// One breadcrumb: what the app did just before something went wrong.
#[derive(Clone, Debug, PartialEq)]
pub struct Breadcrumb {
    /// One line: `"feed:page-loaded"`.
    pub message: String,
    /// When it was dropped (`foundation::time::now_ms`).
    pub at_ms: f64,
}

/// Where errors and breadcrumbs go.
pub trait Monitoring: Send + Sync {
    /// Records a raised error. Never fails and never blocks the caller.
    fn record_error(&self, error: &AppError);

    /// Drops a breadcrumb (the newest [`MEMORY_MAX`] survive).
    fn breadcrumb(&self, message: &str);

    /// The errors recorded so far, oldest first.
    fn errors(&self) -> Vec<RecordedError>;

    /// The breadcrumbs, oldest first.
    fn breadcrumbs(&self) -> Vec<Breadcrumb>;

    /// Forgets everything (a test's reset).
    fn clear(&self);
}

/// The most records of each kind the sink keeps.
const MEMORY_MAX: usize = 200;

/// The in-memory sink: bounded, thread-safe, entirely local.
#[derive(Debug, Default)]
pub struct MemoryMonitoring {
    errors: Mutex<Vec<RecordedError>>,
    breadcrumbs: Mutex<Vec<Breadcrumb>>,
}

impl Monitoring for MemoryMonitoring {
    fn record_error(&self, error: &AppError) {
        let recorded = RecordedError {
            error: error.clone(),
            at_ms: time::now_ms(),
        };
        let mut errors = self.errors.lock().expect("monitoring errors");
        errors.push(recorded);
        if errors.len() > MEMORY_MAX {
            errors.remove(0);
        }
    }

    fn breadcrumb(&self, message: &str) {
        let crumb = Breadcrumb {
            message: message.to_owned(),
            at_ms: time::now_ms(),
        };
        let mut breadcrumbs = self.breadcrumbs.lock().expect("monitoring breadcrumbs");
        breadcrumbs.push(crumb);
        if breadcrumbs.len() > MEMORY_MAX {
            breadcrumbs.remove(0);
        }
    }

    fn errors(&self) -> Vec<RecordedError> {
        self.errors.lock().expect("monitoring errors").clone()
    }

    fn breadcrumbs(&self) -> Vec<Breadcrumb> {
        self.breadcrumbs
            .lock()
            .expect("monitoring breadcrumbs")
            .clone()
    }

    fn clear(&self) {
        *self.errors.lock().expect("monitoring errors") = Vec::new();
        *self.breadcrumbs.lock().expect("monitoring breadcrumbs") = Vec::new();
    }
}

/// The app's shared sink. Swapping the implementation means editing
/// this file's `SINK` line; call sites never change.
pub fn sink() -> Arc<dyn Monitoring> {
    static SINK: std::sync::LazyLock<Arc<dyn Monitoring>> = std::sync::LazyLock::new(|| {
        install();
        Arc::new(MemoryMonitoring::default())
    });
    SINK.clone()
}

/// Panics caught by the hook, waiting for the engine thread to report them.
static CRASHES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The longest description GA4 keeps (parameter values: 100 characters).
const DESCRIPTION_MAX: usize = 100;

/// A crash description safe to send: `Uncaught: <message> @ <file>:<line>`,
/// cut to [`DESCRIPTION_MAX`], and redacted whole when it names a session,
/// CSRF, token, key or secret.
pub fn crash_description(message: &str, location: Option<(&str, u32)>) -> String {
    let lower = message.to_ascii_lowercase();
    let sensitive = ["session", "csrf", "token", "secret", "key", "cookie"]
        .iter()
        .any(|word| lower.contains(word));
    let message = if sensitive { "[redacted]" } else { message };
    let full = match location {
        Some((file, line)) => format!("Uncaught: {message} @ {file}:{line}"),
        None => format!("Uncaught: {message}"),
    };
    full.chars().take(DESCRIPTION_MAX).collect()
}

/// Installs the panic hook (once; the previous hook still runs). The app
/// root calls it at start; [`report_pending_crashes`] sends what it caught.
pub fn install() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = info.payload();
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_owned());
            let location = info.location().map(|l| (l.file(), l.line()));
            if let Ok(mut crashes) = CRASHES.lock() {
                crashes.push(crash_description(&message, location));
            }
            previous(info);
        }));
    });
}

/// Sends the panics caught so far as `exception` events (consent-gated in
/// `services::analytics`). Engine thread; [`record_error`] calls it.
pub fn report_pending_crashes() {
    let pending = match CRASHES.lock() {
        Ok(mut crashes) => std::mem::take(&mut *crashes),
        Err(_) => return,
    };
    for description in pending {
        crate::services::analytics::exception(&description, true);
    }
}

/// Records a handled error locally and sends any caught panic: the
/// one-liner `foundation::error` calls.
pub fn record_error(error: &AppError) {
    install();
    sink().record_error(error);
    report_pending_crashes();
}

/// Drops a breadcrumb on the shared sink: the one-liner feature code
/// calls.
pub fn breadcrumb(message: &str) {
    sink().breadcrumb(message);
}

#[cfg(test)]
mod tests {
    use super::crash_description;

    #[test]
    fn crash_text_never_carries_secrets() {
        let d = crash_description("bad x-eq-session abc", Some(("src/a.rs", 3)));
        assert_eq!(d, "Uncaught: [redacted] @ src/a.rs:3");
        assert!(crash_description(&"x".repeat(300), None).chars().count() <= 100);
    }
}
