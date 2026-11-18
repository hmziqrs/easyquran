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
