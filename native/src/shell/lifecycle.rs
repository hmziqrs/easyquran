//! The app's lifecycle state machine, driven by the engine's `lifecycle()`
//! (NSH exposes the engine's phases; `Starting` is the shell's own,
//! until the first phase arrives).

use nsh_core::{platform::Lifecycle, prelude::*};

/// Where the app is, from the shell's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    /// Mounted, before the engine reported its first phase.
    #[default]
    Starting,
    Active,
    Inactive,
    Backgrounded,
}

impl AppPhase {
    /// Its name, as the status bar shows it.
    pub fn name(self) -> &'static str {
        match self {
            AppPhase::Starting => "Starting",
            AppPhase::Active => "Active",
            AppPhase::Inactive => "Inactive",
            AppPhase::Backgrounded => "Backgrounded",
        }
    }
}

/// The engine events the shell maps onto the phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEvent {
    BecameActive,
    ResignedActive,
    EnteredBackground,
}

impl AppEvent {
    /// The event the engine's phase reports.
    pub fn of(lifecycle: Lifecycle) -> Self {
        match lifecycle {
            Lifecycle::Active => AppEvent::BecameActive,
            Lifecycle::Inactive => AppEvent::ResignedActive,
            Lifecycle::Background => AppEvent::EnteredBackground,
        }
    }
}

/// The phase `event` moves the machine to: the engine's report is the
/// truth, so every event wins over the phase it lands on.
pub fn transition(phase: AppPhase, event: AppEvent) -> AppPhase {
    let _ = phase;
    match event {
        AppEvent::BecameActive => AppPhase::Active,
        AppEvent::ResignedActive => AppPhase::Inactive,
        AppEvent::EnteredBackground => AppPhase::Backgrounded,
    }
}

/// The app's phase, a signal of the surface's.
pub fn phase() -> Signal<AppPhase> {
    global::<AppPhase>()
}

/// The root's watch: follows the engine's lifecycle for the app's run.
pub fn watch() {
    effect(move || {
        let event = AppEvent::of(lifecycle());
        phase().update(|phase| *phase = transition(*phase, event));
    });
}
