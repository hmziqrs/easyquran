//! The window-and-menus card of the desktops: the window title, the
//! application menu (set while the page is open, re-set per locale), a
//! context menu and the window's maximize. Mobile has none of these, so the
//! card says so instead of showing dead buttons.

use nsh_base::theme;
use nsh_core::{
    desktop::{MenuRole, WindowFrame},
    element::desktop::{MenuItem, main_window, popup_menu, set_application_menu},
    prelude::*,
};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use crate::ui::desktop::{set_window_title, toggle_maximize};

use super::{
    locale,
    widgets::{hint, localized, panel, quiet_button},
};

/// The application menu of the page, localized on every effect run: the menu
/// follows the signals its effect reads.
fn application_menu(picks: Signal<u32>) -> Vec<MenuItem> {
    vec![
        MenuItem::submenu(
            t!("platform.desktop.file").to_string(),
            vec![
                MenuItem::new(t!("platform.desktop.new_note").to_string())
                    .shortcut("mod+n")
                    .on_select(move || picks.update(|count| *count += 1)),
                MenuItem::separator(),
                MenuItem::role(MenuRole::Close),
            ],
        ),
        MenuItem::submenu(
            t!("platform.desktop.help").to_string(),
            vec![
                MenuItem::new(t!("platform.desktop.about").to_string())
                    .on_select(move || picks.update(|count| *count += 1)),
            ],
        ),
    ]
}

/// The context menu of the button, built at press time in the current locale.
fn context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::new(t!("platform.desktop.copy_path").to_string()),
        MenuItem::separator(),
        MenuItem::new(t!("platform.desktop.remove").to_string()),
    ]
}

#[component]
fn WindowCard() -> impl IntoElement {
    // The application-menu items chosen since the page opened.
    let picks = signal(0_u32);
    // The menu lives while this card does; the effect re-sets it when the
    // locale changes and drops the handlers when the card goes.
    effect(move || {
        locale::app_locale().get();
        set_application_menu(application_menu(picks));
    });
    panel(
        "platform.desktop.title",
        [
            hint("platform.desktop.menu_note"),
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(quiet_button(
                    "platform-desktop-title",
                    localized("platform.desktop.title_button"),
                    {
                        move || {
                            set_window_title(format!(
                                "{} — {}",
                                t!("app.name"),
                                t!("platform.title")
                            ));
                            picks.update(|count| *count += 1);
                        }
                    },
                ))
                .child(quiet_button(
                    "platform-desktop-context",
                    localized("platform.desktop.context"),
                    {
                        move || {
                            // The answer only says when the menu closed.
                            drop(popup_menu(context_menu()));
                            picks.update(|count| *count += 1);
                        }
                    },
                ))
                .child(quiet_button(
                    "platform-desktop-maximize",
                    localized("platform.desktop.maximize"),
                    {
                        move || {
                            toggle_maximize();
                            picks.update(|count| *count += 1);
                        }
                    },
                ))
                .child(quiet_button(
                    "platform-desktop-frame",
                    localized("platform.desktop.frame"),
                    {
                        move || {
                            main_window().set_bounds(WindowFrame {
                                width: Some(px(480.0)),
                                height: Some(px(720.0)),
                                ..WindowFrame::default()
                            });
                            picks.update(|count| *count += 1);
                        }
                    },
                ))
                .into_element(),
            // What the last window or menu action was, as a count: the window
            // functions have no answer to show.
            text(move || {
                locale::app_locale().get();
                format!("{}: {}", t!("platform.desktop.picks"), picks.get())
            })
            .text_sm()
            .tabular_nums()
            .test_id("platform-desktop-picks")
            .into_element(),
        ],
    )
}

/// What the platform page shows for windows and menus: the card on a desktop,
/// the boundary note elsewhere (decided once; a platform never changes).
pub(crate) fn card() -> AnyElement {
    when(is_desktop(), || view().child(window_card()).into_element())
        .otherwise(|| {
            view()
                .child(
                    panel(
                        "platform.desktop.title",
                        [text(move || {
                            locale::app_locale().get();
                            t!("platform.desktop.unavailable").to_string()
                        })
                        .text_sm()
                        .text_color(move || theme().muted_foreground)
                        .into_element()],
                    )
                    .into_element(),
                )
                .into_element()
        })
        .into_element()
}
