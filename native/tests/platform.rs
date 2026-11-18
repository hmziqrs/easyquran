//! The platform pages, driven through the UI on a manual clock: the locale
//! switch re-renders the strings, the clipboard round-trips, the device page
//! renders its sections, and the picker, speech and keyboard surfaces answer.
//! No real network, no real time, no real dialogs: the driver answers the
//! platform's part. Tests run in parallel in one process and the locale is
//! process-global, so every test but the locale one presses by test id and
//! asserts on locale-independent values only.

use std::time::{Duration, SystemTime};

use nsh_core::{ImageData, picker::PickedFile, prelude::*};
use nsh_engine::{Clock, Edges, EngineConfig, EnvironmentPins, Points, Root};
use nsh_testing::Headless;

/// The platform page as the shell mounts it.
#[component]
fn PlatformRoot() -> impl IntoElement {
    view()
        .flex_1()
        .child(nsh_starter::platform::platform_page())
}

/// The device page as the shell mounts it.
#[component]
fn DeviceRoot() -> impl IntoElement {
    view().flex_1().child(nsh_starter::platform::device_page())
}

fn start<C: Component<Props = ()>>() -> Headless {
    let config = EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<C>())
    };
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

/// `root`'s page with pinned safe-area insets, as a phone reports them.
fn pinned<C: Component<Props = ()>>() -> Headless {
    let safe = Edges {
        top: Points(47.0),
        right: Points(0.0),
        bottom: Points(34.0),
        left: Points(0.0),
    };
    // `Environment` and its pins are non-exhaustive: build, then set fields.
    let mut pins = EnvironmentPins::default();
    pins.safe_area = Some(safe);
    let mut config = EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        pinned: pins,
        ..EngineConfig::new(Root::of::<C>())
    };
    config.environment.safe_area = safe;
    let mut app = Headless::start(config).unwrap();
    app.first_render().unwrap();
    app
}

#[test]
fn locale_switch_re_renders_strings() -> Result<(), String> {
    let mut app = start::<PlatformRoot>();
    // A card's heading: the page's own title is the iOS header's large
    // title on phones, which the headless phone does not draw.
    app.wait_for("Clipboard")?;
    app.press("中文")?;
    app.wait_for("剪贴板")?;
    // The current-locale line followed the switch too.
    app.wait_for("当前: zh-CN")?;
    // `rust_i18n::set_locale` is process-global: put it back for the other tests.
    rust_i18n::set_locale("en");
    Ok(())
}

#[test]
fn clipboard_roundtrips_through_the_ui() -> Result<(), String> {
    let mut app = start::<PlatformRoot>();
    app.wait_for("id:platform-clipboard-copy")?;
    app.type_text("id:platform-clipboard-draft", "Buy milk")?;
    app.press("id:platform-clipboard-copy")?;
    // The driver holds what the app copied.
    assert_eq!(app.screen().clipboard(), Some("Buy milk"));
    app.clear("id:platform-clipboard-draft")?;
    app.press("id:platform-clipboard-paste")?;
    // The paste answered through the UI: the field holds the text again.
    app.wait_until(
        "the field pastes the copied text back",
        Duration::from_secs(2),
        |screen| {
            screen
                .input_text("id:platform-clipboard-draft")
                .ok()
                .flatten()
                == Some("Buy milk")
        },
    )?;
    Ok(())
}

#[test]
fn device_page_renders_its_sections() -> Result<(), String> {
    let mut app = pinned::<DeviceRoot>();
    // The pinned insets reach the page as values, whatever the language.
    app.wait_for("47 pt")?;
    assert!(app.shows("34 pt"));
    // The not-exposed row rendered (its label translates, so find it by id).
    assert_eq!(app.screen().count("id:platform-device-model")?, 1);
    Ok(())
}

#[test]
fn picker_shows_the_picked_file_name() -> Result<(), String> {
    let mut app = start::<PlatformRoot>();
    app.wait_for("id:platform-picker-pick")?;
    // The user picks one file; nothing uploads.
    app.on_pick(move |_| {
        vec![PickedFile::new(
            "trail-photo.jpg",
            "image/jpeg",
            ImageData::new(vec![0xFF, 0xD8, 0xFF, 0xE0]),
        )]
    });
    app.press("id:platform-picker-pick")?;
    app.wait_for("trail-photo.jpg · image/jpeg")?;
    Ok(())
}

#[test]
fn speech_transcribes_a_heard_phrase() -> Result<(), String> {
    let mut app = start::<PlatformRoot>();
    app.wait_for("id:platform-speech-start")?;
    app.press("id:platform-speech-start")?;
    // Only a listening session accepts words, so this doubles as its assertion.
    app.hear("hello nsh")?;
    app.end_speech(None)?;
    app.wait_for("hello nsh")?;
    Ok(())
}

#[test]
fn keyboard_height_updates_the_page() -> Result<(), String> {
    let mut app = start::<PlatformRoot>();
    app.wait_for("id:platform-keyboard-draft")?;
    app.set_environment("keyboard", |environment| {
        environment.keyboard_height = Points(320.0)
    })?;
    // Asserted here: the card's live readout follows the keyboard environment,
    // and the field stays editable while it is up. The page's padding reads the
    // same environment value, but headless exposes no geometry: the lift
    // itself is what the macOS capture (`vision/platform.test.tsx`) shows.
    app.wait_for("320 pt")?;
    app.type_text("id:platform-keyboard-draft", "still visible")?;
    app.wait_for("still visible")?;
    Ok(())
}
