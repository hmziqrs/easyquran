//! The foundation's headless tests, on a manual clock, through the UI the
//! foundation ships: the first-run gate, the index, Diagnostics and the
//! central error surface, all mounted by `foundation::DemoRoot`.
//!
//! Each test was seen to fail once by breaking the code on purpose; the
//! break used is named in the test's doc comment here for the integration
//! tests, and beside each inline `#[test]` in `src/foundation` and
//! `src/state` for the unit tests.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use nsh_engine::{EngineConfig, Root};
use nsh_testing::{Headless, MemoryStorage};

/// The wall time every test's clock starts at: the Unix epoch, so ids and
/// timestamps are reproducible.
const START: SystemTime = SystemTime::UNIX_EPOCH;

/// Starts `DemoRoot` with `entries` preloaded into the app's storage and
/// the clock standing at [`START`].
fn app_with(entries: Arc<MemoryStorage>) -> Headless {
    let config = EngineConfig {
        clock: nsh_core::time::Clock::manual(START),
        storage: Some(entries.clone()),
        ..EngineConfig::new(Root::of::<nsh_starter::foundation::DemoRoot>())
    };
    Headless::start(config).unwrap()
}

/// Storage with the first-run flow already completed, so tests land on the
/// index directly.
fn returning_storage() -> Arc<MemoryStorage> {
    MemoryStorage::new([(
        "app.first_run_done".to_owned(),
        START.elapsed().unwrap().as_millis().to_string(),
    )])
}

/// Opens Diagnostics from the demo index.
fn open_diagnostics(app: &mut Headless) {
    app.press("role:button[Open Diagnostics]").unwrap();
    app.wait_for("text:/Capabilities/").unwrap();
}

/// The config migration roundtrip, through the Diagnostics page: a schema-0
/// document ("Ada", dark) migrates to the current schema, shows migrated on
/// screen, and is saved back so the stored form converges.
///
/// Break seen: `config::migrate` mapped `v0.dark` to `"system"` always —
/// the test failed at `wait_for("text:dark")`.
#[test]
fn config_migration_roundtrip() {
    let storage = MemoryStorage::new([
        (
            "app.config".to_owned(),
            r#"{"name":"Ada","dark":true}"#.to_owned(),
        ),
        ("app.first_run_done".to_owned(), "1".to_owned()),
    ]);
    let mut app = app_with(storage.clone());
    // First frame: the index is up, so the pre-seeded flow stays done.
    app.wait_for("text:Foundation and state").unwrap();
    // The logging ring is process-global and parallel tests share it.
    // Clearing now bounds the read to this test's window, and the line
    // itself names this engine's saved config ("Ada") — no other test's
    // engine saves a config with that name, so only this engine's
    // DemoRoot subscription can produce the asserted string.
    nsh_starter::foundation::logging::clear_log();
    open_diagnostics(&mut app);

    // The migrated fields the config card shows.
    app.wait_for("text:Ada").unwrap();
    app.wait_for("text:dark").unwrap();
    app.wait_for("text:v1").unwrap();

    // The save-back travelled the bus: the events card shows the
    // `app.config` publish (its effect runs after the page's build, so the
    // log card's snapshot cannot hold the line).
    app.wait_for("text:app.config").unwrap();
    assert!(
        nsh_starter::foundation::logging::log_tail(200)
            .iter()
            .any(|line| line.message == "config saved (app.config) for Ada"),
        "the subscription did not log: {:?}",
        nsh_starter::foundation::logging::log_tail(200)
    );

    // The migration was saved back in the current schema.
    let stored = storage
        .entries()
        .get("app.config")
        .expect("config saved")
        .clone();
    assert!(stored.contains(r#""schema_version":1"#), "stored: {stored}");
    assert!(
        stored.contains(r#""display_name":"Ada""#),
        "stored: {stored}"
    );
    assert!(stored.contains(r#""theme":"dark""#), "stored: {stored}");
}

/// The schema-0 migrator is pure: every field lands where the v1 config
/// keeps it. (No UI, no clock; the UI roundtrip above proves load and
/// save.)
///
/// Break seen: `config::migrate` mapped `v0.dark` to `"dark"` always (the
/// opposite break of the roundtrip's) — the test failed on the
/// `theme == "system"` assertion.
#[test]
fn migrate_maps_schema_zero_fields() {
    let migrated =
        nsh_starter::foundation::config::migrate(r#"{"name":"Bo","dark":false}"#).unwrap();
    assert_eq!(migrated.schema_version, 1);
    assert_eq!(migrated.display_name, "Bo");
    assert_eq!(migrated.theme, "system");
    assert!(migrated.telemetry_enabled);
}

/// The event bus keeps publish order: three events published in one batch
/// appear on the Diagnostics page first, second, third.
///
/// Break seen: `publish_test_events` published `"third"`, `"second"`,
/// `"first"` — the test failed on the order assertion at `first < second`.
#[test]
fn event_bus_ordering() {
    let mut app = app_with(returning_storage());
    app.wait_for("text:Foundation and state").unwrap();
    open_diagnostics(&mut app);

    app.press("role:button[Publish test events]").unwrap();
    app.wait_for("text:first").unwrap();

    let texts = app.screen().texts();
    let at = |needle: &str| {
        texts
            .iter()
            .position(|text| *text == needle)
            .unwrap_or_else(|| panic!("{needle} not on screen: {texts:?}"))
    };
    let (first, second, third) = (at("first"), at("second"), at("third"));
    assert!(first < second && second < third, "order: {texts:?}");
}

/// The task manager cancels a running task: its future stops, the state
/// reads Cancelled, and moving the clock afterwards brings no "done".
///
/// Break seen: `tasks::cancel` recorded Cancelled but
/// `std::mem::forget(spawned)` kept the future alive — the task ran to the
/// end, and the test failed on "task finished despite cancel".
#[test]
fn task_manager_cancel() {
    let mut app = app_with(returning_storage());
    app.wait_for("text:Foundation and state").unwrap();
    open_diagnostics(&mut app);

    app.press("role:button[Start sample task]").unwrap();
    app.wait_for("text:/running/").unwrap();

    // Cancel while the first tick (one second out) is still pending.
    app.press("role:button[Cancel]").unwrap();
    app.wait_for("text:/cancelled/").unwrap();

    // The cancelled task stays cancelled no matter how far time moves.
    app.run_for(Duration::from_secs(6)).unwrap();
    // Task rows read "#<id> <state>"; event rows are history and are
    // allowed to mention anything.
    let done_rows = app.screen().count("text:/#[0-9]+ (done|running)/").unwrap();
    assert_eq!(done_rows, 0, "task finished despite cancel");
    let cancelled_rows = app.screen().count("text:/#[0-9]+ cancelled/").unwrap();
    assert!(cancelled_rows >= 1, "no cancelled row on screen");
}

/// A task that runs to its end reads Done on the bus's task events, and
/// Cancel all empties the running set (the drain-on-quit path).
///
/// Break seen: `tasks::cancel_all` took the live handles but
/// `std::mem::forget(spawned)` kept them running — the test failed on
/// "drain did not cancel both: 0".
#[test]
fn task_manager_runs_to_done_and_drains() {
    let mut app = app_with(returning_storage());
    app.wait_for("text:Foundation and state").unwrap();
    open_diagnostics(&mut app);

    app.press("role:button[Start sample task]").unwrap();
    // Five ticks, one second apart, then the task ends by itself.
    app.run_for(Duration::from_secs(6)).unwrap();
    app.wait_for("text:done sample").unwrap();

    // Two samples running; Cancel all (drain on quit) ends both.
    app.press("role:button[Start sample task]").unwrap();
    app.press("role:button[Start sample task]").unwrap();
    app.press("role:button[Cancel all]").unwrap();
    app.run_for(Duration::from_secs(6)).unwrap();
    let running = app.screen().count("text:/#[0-9]+ running/").unwrap();
    assert_eq!(running, 0, "drain left a task running");
    let cancelled = app.screen().count("text:/#[0-9]+ cancelled/").unwrap();
    assert!(cancelled >= 2, "drain did not cancel both: {cancelled}");
}

/// The first-run flow shows once, persists completion, and never shows
/// again on the next start.
///
/// Break seen: `first_run::complete` published the bus event but skipped
/// `Storage::set_item` (`let _ = marker`) — the restarted app showed the
/// flow again, and the test failed at the marker assertion.
#[test]
fn first_run_once_only() {
    let storage = MemoryStorage::new([]);
    let mut app = app_with(storage.clone());
    app.wait_for("text:Welcome to nsh-starter").unwrap();

    // "Bo", not the roundtrip test's "Ada": each test's engine logs its
    // own saved name, so the process-global ring never cross-matches.
    app.type_text("id:first-run-name", "Bo").unwrap();
    app.press("role:button[Get started]").unwrap();
    app.wait_for("text:Foundation and state").unwrap();

    let stored = storage.entries();
    // The completion marker is an id (`foundation::id::new_id`): on the
    // manual clock its timestamp part is the clock's, so it reads back
    // exactly.
    let marker = stored
        .get("app.first_run_done")
        .expect("first-run marker saved");
    assert!(marker.starts_with("first-run-"), "marker: {marker}");
    let config = stored.get("app.config").expect("config saved").clone();
    assert!(
        config.contains(r#""display_name":"Bo""#),
        "stored: {config}"
    );

    // The next start skips the flow.
    let mut again = app_with(storage.clone());
    again.wait_for("text:Foundation and state").unwrap();
    assert!(
        !again.screen().shows("Welcome to nsh-starter"),
        "first-run came back"
    );
}

/// The central error surface: a raised error shows with its detail, and
/// Dismiss clears it.
///
/// Break seen: `error_surface`'s `when` condition was hardcoded to
/// `false` — the banner never appeared, and the test failed at
/// `wait_for` of the message.
#[test]
fn error_surface_shows_and_dismisses() {
    let mut app = app_with(returning_storage());
    app.wait_for("text:Foundation and state").unwrap();
    open_diagnostics(&mut app);

    app.press("role:button[Send test error]").unwrap();
    app.wait_for("text:Test error from Diagnostics. Dismiss it when read.")
        .unwrap();
    // The detail line rides along.
    app.wait_for("text:/raised to verify the central surface/")
        .unwrap();

    app.press("role:button[Dismiss]").unwrap();
    // The banner's detail line is unique to it: the events card keeps the
    // payload as history, so it cannot say whether the banner went.
    app.wait_for_gone(
        "text:/raised to verify the central surface/",
        Duration::from_secs(2),
    )
    .unwrap();

    // The same raise landed in the monitoring sink, so the real sink
    // receives everything a page raises.
    let recorded = nsh_starter::services::monitoring::sink().errors();
    assert_eq!(recorded.len(), 1, "monitoring saw the raise: {recorded:?}");
    assert_eq!(recorded[0].error.code, "diagnostic");
}
