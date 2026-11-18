//! The monitoring seam: one small trait with an in-memory sink, so the
//! real error-tracking library lands in one file. The local sink records
//! errors and breadcrumbs and sends nothing anywhere.

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
    static SINK: std::sync::LazyLock<Arc<dyn Monitoring>> =
        std::sync::LazyLock::new(|| Arc::new(MemoryMonitoring::default()));
    SINK.clone()
}

/// Drops a breadcrumb on the shared sink: the one-liner feature code
/// calls.
pub fn breadcrumb(message: &str) {
    sink().breadcrumb(message);
}
