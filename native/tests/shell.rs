//! The shell's headless tests: the deep-link table, the back order and
//! the palette on a manual clock, undo and redo, the two layouts and the
//! lifecycle watch. Pure tables first, then the real engine.
//!
//! Settings opens on its Storage section, whose description line
//! ([`SETTINGS_MARK`]) is what these tests look for to tell the page apart
//! from Home.

use std::time::{Duration, SystemTime};

/// The Storage section's description: on screen only while Settings is.
const SETTINGS_MARK: &str = "text:Downloads and cached content on this device.";

use easyquran::shell::{
    back::{BackStep, back_step},
    lifecycle::{AppEvent, AppPhase, transition},
    routes::{Route, SettingsSection, route_from_deep_link},
    undo::UndoHistory,
};
use nsh_core::{
    geometry::{Points, Size},
    platform::Lifecycle,
    time::Clock,
};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;

/// A phone-sized app on a manual clock, as a headless run behaves (the
/// engine reports its build's mobile OS).
fn phone() -> Headless {
    let mut config = EngineConfig::new(Root::of::<easyquran::App>());
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    Headless::start(config).unwrap()
}

/// A desktop-width app on a manual clock: the sidebar shell.
fn desktop() -> Headless {
    let mut config = EngineConfig::new(Root::of::<easyquran::App>());
    config.viewport = Size::new(Points(1280.0), Points(800.0));
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    Headless::start(config).unwrap()
}

#[test]
fn deep_links_parse_into_routes() {
    assert_eq!(route_from_deep_link(&[]), Some(Route::Home));
    assert_eq!(route_from_deep_link(&["surah"]), Some(Route::SurahIndex));
    assert_eq!(route_from_deep_link(&["juz"]), Some(Route::JuzIndex));
    assert_eq!(
        route_from_deep_link(&["settings"]),
        Some(Route::Settings(SettingsSection::Storage))
    );
    assert_eq!(
        route_from_deep_link(&["settings", "privacy"]),
        Some(Route::Settings(SettingsSection::Privacy))
    );
    assert_eq!(
        route_from_deep_link(&["yours", "cards", "c1"]),
        Some(Route::YoursCard("c1".into()))
    );
    assert_eq!(
        route_from_deep_link(&["azkar", "morning"]),
        Some(Route::Zikr {
            slug: "morning".into(),
            translation: None
        })
    );
    assert_eq!(
        route_from_deep_link(&["diagnostics"]),
        Some(Route::Diagnostics)
    );
    assert_eq!(route_from_deep_link(&["nope"]), None);
    assert_eq!(route_from_deep_link(&["settings", "nope"]), None);
}

#[test]
fn the_back_order_is_overlays_pages_then_the_exit_ask() {
    // The engine asks the newest modal before this listener runs; what
    // this table fixes is what the listener itself decides.
    assert_eq!(back_step(2, false), BackStep::PopPage);
    assert_eq!(back_step(1, false), BackStep::PopPage);
    assert_eq!(back_step(0, false), BackStep::AskExit);
    assert_eq!(back_step(0, true), BackStep::Exit);
    // A page popped disarms the exit ask: it is asked for again.
    assert_eq!(back_step(1, true), BackStep::PopPage);
}

#[test]
fn the_lifecycle_machine_follows_the_engine_events() {
    assert_eq!(
        transition(AppPhase::Starting, AppEvent::BecameActive),
        AppPhase::Active
    );
    assert_eq!(
        transition(AppPhase::Active, AppEvent::ResignedActive),
        AppPhase::Inactive
    );
    assert_eq!(
        transition(AppPhase::Inactive, AppEvent::EnteredBackground),
        AppPhase::Backgrounded
    );
    assert_eq!(
        transition(AppPhase::Backgrounded, AppEvent::BecameActive),
        AppPhase::Active
    );
    // A phase reported before any other is still the truth.
    assert_eq!(
        transition(AppPhase::Starting, AppEvent::EnteredBackground),
        AppPhase::Backgrounded
    );
    // The engine phases map onto the events.
    assert_eq!(
        AppEvent::of(Lifecycle::Background),
        AppEvent::EnteredBackground
    );
}

#[test]
fn undo_history_is_bounded() {
    let mut history = UndoHistory::default();
    assert_eq!(history.undo(Vec::new()), None, "nothing recorded yet");
    for depth in 0..60 {
        history.record(vec![Route::YoursCard(depth.to_string())]);
    }
    // The oldest snapshots went; the limit is the newest fifty.
    assert_eq!(
        history.undo(vec![Route::Home]),
        Some(vec![Route::YoursCard("59".into())])
    );
    assert_eq!(history.redo(vec![Route::Home]), Some(vec![Route::Home]));
    assert_eq!(
        history.redo(vec![Route::Home]),
        None,
        "the redo tail went on record"
    );
}

/// Opens Settings from the palette and waits for its page.
fn open_settings(app: &mut Headless) {
    app.press_key("mod+k").unwrap();
    app.wait_for("text:Go to").unwrap();
    // The headless keyboard does not focus the palette's search field, so
    // the run chooses the row as a tap does.
    app.press("role:option[Settings]").unwrap();
    app.wait_until(
        "Settings opened and the palette closed",
        Duration::from_secs(5),
        |screen| screen.count("text:Go to") == Ok(0) && screen.count(SETTINGS_MARK) >= Ok(1),
    )
    .unwrap();
}

#[test]
fn back_pops_a_page_then_asks_to_exit() {
    let mut app = phone();
    app.wait_for("text:Home").unwrap();
    open_settings(&mut app);

    // The palette goes up over the page: back closes it first and the
    // page beneath stays.
    app.press_key("mod+k").unwrap();
    app.wait_for("text:Go to").unwrap();
    app.back().unwrap();
    app.wait_until(
        "the palette closed over the intact page",
        Duration::from_secs(5),
        |screen| screen.count("text:Go to") == Ok(0) && screen.count(SETTINGS_MARK) >= Ok(1),
    )
    .unwrap();

    // The next back pops the page.
    app.back().unwrap();
    app.wait_until("Settings popped", Duration::from_secs(5), |screen| {
        screen.count(SETTINGS_MARK) == Ok(0)
    })
    .unwrap();

    // Back at Home asks before the platform could exit the app.
    app.back().unwrap();
    app.wait_for("text:Exit EasyQuran?").unwrap();
    app.press("role:button[Cancel]").unwrap();
    app.wait_for_gone("text:Exit EasyQuran?", Duration::from_secs(5))
        .unwrap();

    // The ask comes again: canceling did not arm the exit.
    app.back().unwrap();
    app.wait_for("text:Exit EasyQuran?").unwrap();
}

#[test]
fn undo_and_redo_retrace_navigation() {
    let mut app = phone();
    app.wait_for("text:Home").unwrap();
    open_settings(&mut app);

    app.press_key("mod+z").unwrap();
    app.wait_until("undo returned Home", Duration::from_secs(5), |screen| {
        screen.count(SETTINGS_MARK) == Ok(0)
    })
    .unwrap();

    app.press_key("mod+shift+z").unwrap();
    app.wait_until(
        "redo opened Settings again",
        Duration::from_secs(5),
        |screen| screen.count(SETTINGS_MARK) >= Ok(1),
    )
    .unwrap();
}

#[test]
fn the_desktop_width_takes_the_sidebar() {
    let mut app = desktop();
    app.wait_for("label:Hide sidebar").unwrap();
    // The pane beside it still shows Home, the status bar under both.
    app.wait_for("text:Home").unwrap();
    app.wait_for("text:Active").unwrap();

    // A sidebar row opens its page into the pane.
    app.press("id:nav-settings").unwrap();
    app.wait_for(SETTINGS_MARK).unwrap();

    let mut narrow = phone();
    narrow.wait_for("text:Home").unwrap();
    assert_eq!(
        narrow.screen().count("label:Hide sidebar"),
        Ok(0),
        "the phone shell has no sidebar"
    );
}

#[test]
fn the_sidebar_collapses_and_comes_back() {
    let mut app = desktop();
    app.wait_for("id:nav-settings").unwrap();
    app.press("label:Hide sidebar").unwrap();
    app.wait_for_gone("id:nav-settings", Duration::from_secs(1))
        .unwrap();
    // The toolbar takes the toggle while the sidebar is away.
    app.press("label:Show sidebar").unwrap();
    app.wait_for("id:nav-settings").unwrap();
    // The platform shortcut does the same.
    app.press_key("ctrl+mod+s").unwrap();
    app.wait_for_gone("id:nav-settings", Duration::from_secs(1))
        .unwrap();
}

#[test]
fn the_lifecycle_watch_follows_the_environment() {
    // The status bar that shows the phase is the desktop shell's.
    let mut app = desktop();
    app.wait_for("text:Active").unwrap();
    // The environment handed in is the platform default's: the desktop
    // width is restated so the shell (and its status bar) stays.
    app.set_environment("the app backgrounds", |environment| {
        environment.window = Size::new(Points(1280.0), Points(800.0));
        environment.lifecycle = Lifecycle::Background;
    })
    .unwrap();
    app.wait_for("text:Backgrounded").unwrap();
    app.set_environment("the app activates", |environment| {
        environment.window = Size::new(Points(1280.0), Points(800.0));
        environment.lifecycle = Lifecycle::Active;
    })
    .unwrap();
    app.wait_for("text:Active").unwrap();
}
