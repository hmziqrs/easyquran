//! Platform helpers: the UI locale signal moves with `rust-i18n`'s global
//! locale, so string bindings that read it re-run on a switch. The locale is
//! process-global; this file holds the only test that changes it.

use std::time::SystemTime;

use easyquran::{
    platform::locale::{CHOICES, app_locale, set_app_locale},
    ui::button::button,
};
use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_testing::Headless;

/// The locale code, bound to the signal, and a switch to Arabic.
#[component]
fn LocaleRoot() -> impl IntoElement {
    view()
        .flex_1()
        .test_id("locale-root")
        .child(text(move || format!("locale: {}", app_locale().get())))
        .child(button("Arabic", || set_app_locale("ar")))
}

#[test]
fn locale_switch_rebinds_strings() -> Result<(), String> {
    let mut config = EngineConfig::new(Root::of::<LocaleRoot>());
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    let mut app = Headless::start(config).map_err(|e| e.to_string())?;
    app.wait_for("id:locale-root")?;
    assert!(CHOICES.iter().any(|(code, _)| *code == "ar"));
    app.press("role:button[Arabic]")?;
    app.wait_for("text:locale: ar")?;
    assert_eq!(rust_i18n::locale().to_string(), "ar");
    rust_i18n::set_locale("en");
    Ok(())
}
