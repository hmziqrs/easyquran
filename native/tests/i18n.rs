//! The locale store against the real engine: a mounted `t(..)` text follows
//! a locale switch, the choice is stored, and the root's direction flips a
//! row.

use std::sync::Arc;

use easyquran::{
    stores::locale::{install, layout_direction, set_locale, t},
    ui::button::button,
};
use eq_core::i18n::{Locale, keys};
use nsh_core::prelude::*;
use nsh_engine::{EngineConfig, Root};
use nsh_testing::{Headless, MemoryStorage};

#[component]
fn LocaleProbe() -> impl IntoElement {
    install();
    view()
        .flex_1()
        .direction(layout_direction)
        .child(text(t(keys::READER_SHARE_CARD_BRAND)))
        .child(button("Switch", || set_locale(Locale::Ar)))
        .child(
            view()
                .flex_row()
                .child(view().test_id("first").child(text("1")))
                .child(view().test_id("second").child(text("2"))),
        )
}

fn x_of(app: &Headless, id: &str) -> f32 {
    app.screen()
        .layout_nodes()
        .into_iter()
        .find(|n| n.test_id.as_deref() == Some(id))
        .map(|n| n.bounds.origin.x.0)
        .unwrap()
}

#[test]
fn switching_locale_updates_mounted_text_and_direction() -> Result<(), String> {
    let storage = MemoryStorage::new([("easyquran.locale".to_owned(), "en".to_owned())]);
    let config = EngineConfig {
        storage: Some(storage.clone() as Arc<dyn nsh_core::storage::StorageBackend>),
        ..EngineConfig::new(Root::of::<LocaleProbe>())
    };
    let mut app = Headless::start(config).map_err(|e| e.to_string())?;
    app.wait_for("text:EasyQuran")?;
    assert!(x_of(&app, "first") < x_of(&app, "second"), "ltr row");

    app.press("role:button[Switch]")?;
    app.wait_for("text:إيزي قرآن")?;
    assert!(x_of(&app, "first") > x_of(&app, "second"), "rtl row");
    Ok(())
}
