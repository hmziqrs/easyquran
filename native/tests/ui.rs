//! The UI library's headless tests: the wrappers feature screens build on
//! (the stub page frame, the dialog card, toasts) react through the UI only,
//! on a manual clock.

use std::time::{Duration, SystemTime};

use easyquran::{
    features::placeholder,
    ui::{
        button::button,
        dialog::dialog_card,
        text::muted,
        toast::{notify, toast_host},
    },
};
use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Points, Root, Size};
use nsh_testing::Headless;

/// A stub page, a dialog card behind a button, and the toast host.
#[component]
fn KitRoot() -> impl IntoElement {
    let open = signal(false);
    view()
        .flex_1()
        .child(button("Open dialog", move || open.set(true)))
        .child(button("Notify", || notify("Saved")))
        .child(placeholder("Kit", Some("detail line".into())))
        .child(dialog_card(
            open,
            "Discard the note?".into(),
            move || open.set(false),
            || muted("It cannot be restored.").into_element(),
            move || button("Cancel", move || open.set(false)).into_element(),
        ))
        .child(toast_host())
}

fn start(viewport: Option<Size<Points>>) -> Headless {
    let mut config = EngineConfig::new(Root::of::<KitRoot>());
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    if let Some(viewport) = viewport {
        config.viewport = viewport;
    }
    Headless::start(config).unwrap()
}

#[test]
fn the_stub_page_says_it_is_not_built() -> Result<(), String> {
    // Desktop width: the page draws its own heading (phones leave it to
    // the system header).
    let mut app = start(Some(Size::new(Points(1200.0), Points(800.0))));
    app.wait_for("text:Not built yet")?;
    app.wait_for("text:detail line")?;
    Ok(())
}

#[test]
fn the_dialog_card_opens_and_cancels() -> Result<(), String> {
    let mut app = start(None);
    app.wait_for("text:Not built yet")?;
    app.press("role:button[Open dialog]")?;
    app.wait_for("text:Discard the note?")?;
    app.press("role:button[Cancel]")?;
    app.wait_for_gone("text:Discard the note?", Duration::from_secs(1))?;
    Ok(())
}

#[test]
fn a_notify_shows_a_toast_that_times_out() -> Result<(), String> {
    let mut app = start(None);
    app.wait_for("text:Not built yet")?;
    app.press("role:button[Notify]")?;
    app.run_for(Duration::from_millis(300))?;
    app.wait_for("text:Saved")?;
    // The toast lives 5 s on the manual clock; nothing moves but the test.
    app.run_for(Duration::from_secs(6))?;
    app.wait_for_gone("text:Saved", Duration::from_secs(1))?;
    Ok(())
}

/// The appearance store switches the whole theme at run time: a token a
/// binding resolved changes with the palette and with the mode.
#[component]
fn AppearanceRoot() -> impl IntoElement {
    use easyquran::stores::appearance::{AppearanceMode, AppearanceStore};
    use easyquran::ui::tokens::{Palette, resolved};
    let store = AppearanceStore::install();
    view()
        .flex_1()
        .child(text(move || {
            format!("primary {:?}", resolved(nsh_base::theme().primary))
        }))
        .child(button("Sepia", move || store.set_palette(Palette::Sepia)))
        .child(button("Dark", move || store.set_mode(AppearanceMode::Dark)))
        .child(button("Light", move || {
            store.set_mode(AppearanceMode::Light)
        }))
}

fn primary_text(app: &Headless) -> String {
    app.screen()
        .texts()
        .into_iter()
        .find(|t| t.starts_with("primary "))
        .map(str::to_owned)
        .unwrap_or_default()
}

#[test]
fn palette_and_mode_switch_the_resolved_tokens() -> Result<(), String> {
    let mut app = Headless::start(EngineConfig::new(Root::of::<AppearanceRoot>())).unwrap();
    app.wait_for("text:Sepia")?;
    app.press("role:button[Light]")?;
    app.run_for(Duration::from_millis(100))?;
    let sacred_light = primary_text(&app);
    app.press("role:button[Sepia]")?;
    app.run_for(Duration::from_millis(100))?;
    let sepia_light = primary_text(&app);
    assert_ne!(sacred_light, sepia_light, "the palette changes the primary");
    app.press("role:button[Dark]")?;
    app.run_for(Duration::from_millis(100))?;
    let sepia_dark = primary_text(&app);
    assert_ne!(sepia_light, sepia_dark, "the mode changes the primary");
    Ok(())
}

/// Quran text reaches the screen byte for byte: the verse, then the
/// ornament span, nothing trimmed or normalised; tajweed runs join back
/// into the string the segments carried.
#[component]
fn ArabicRoot() -> impl IntoElement {
    use easyquran::ui::arabic::{ArabicStyle, QuranScript, ayah_text, plain_ayah};
    use easyquran::ui::specimen::{INDOPAK, UTHMANI, tajweed_rows};
    let (_, ayah, verse) = UTHMANI[7];
    let (_, indopak_ayah, indopak) = INDOPAK[7];
    let (_, tajweed_ayah, runs) = tajweed_rows().remove(7);
    view()
        .flex_1()
        .child(plain_ayah(verse, Some(ayah), || {
            ArabicStyle::of(QuranScript::Uthmani)
        }))
        .child(plain_ayah(indopak, Some(indopak_ayah), || {
            ArabicStyle::of(QuranScript::IndoPak)
        }))
        .child(ayah_text(runs, Some(tajweed_ayah), || {
            ArabicStyle::of(QuranScript::Tajweed)
        }))
}

#[test]
fn arabic_text_mounts_verbatim() -> Result<(), String> {
    use easyquran::ui::arabic::{Digits, ornament_text};
    use easyquran::ui::specimen::{INDOPAK, UTHMANI, tajweed_rows};
    let mut app = Headless::start(EngineConfig::new(Root::of::<ArabicRoot>())).unwrap();
    app.run_for(Duration::from_millis(100))?;
    let texts: Vec<String> = app
        .screen()
        .texts()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let uthmani = format!("{}{}", UTHMANI[7].2, ornament_text(255, Digits::Arabic));
    let indopak = format!("{}{}", INDOPAK[7].2, ornament_text(255, Digits::Urdu));
    let joined: String = tajweed_rows()[7]
        .2
        .iter()
        .map(|s| s.text.to_string())
        .collect();
    let tajweed = format!("{joined}{}", ornament_text(255, Digits::Arabic));
    for expected in [uthmani, indopak, tajweed] {
        assert!(
            texts.iter().any(|t| t.as_bytes() == expected.as_bytes()),
            "not on screen byte for byte: {expected}\n{texts:?}"
        );
    }
    Ok(())
}
