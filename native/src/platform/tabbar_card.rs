//! The system tab bar card: `nsh/mobile`'s `TabBar`, native on iOS and
//! Android, no room and nothing shown elsewhere — the other platforms use
//! `nsh/base`'s `Tabs`, which the shell's navigation demonstrates.

use nsh_core::{
    mobile::{TabBarItem, tab_bar},
    prelude::*,
};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale,
    widgets::{hint, localized, panel},
};

#[component]
fn TabBarCard() -> impl IntoElement {
    // The app owns the selection: the bar reports the choice, the page shows it.
    let selected = signal(0_u32);
    panel(
        "platform.tabbar.title",
        [
            tab_bar()
                .items(move || {
                    vec![
                        TabBarItem::new(SharedString::from(localized("platform.tabbar.one")())),
                        TabBarItem::new(SharedString::from(localized("platform.tabbar.two")())),
                    ]
                })
                .selected_index(selected)
                .on_change(move |&index: &u32| selected.set(index))
                .into_element(),
            text(move || {
                locale::app_locale().get();
                let which = if selected.get() == 0 {
                    t!("platform.tabbar.one")
                } else {
                    t!("platform.tabbar.two")
                };
                format!("{}: {}", t!("platform.tabbar.selected"), which)
            })
            .text_sm()
            .tabular_nums()
            .test_id("platform-tabbar-selected")
            .into_element(),
            hint("platform.tabbar.hint"),
        ],
    )
}

/// The tab bar card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(tab_bar_card()).into_element()
}
