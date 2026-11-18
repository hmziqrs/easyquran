//! Platform and system: safe areas, keyboard, the device and environment
//! page, clipboard, picker, speech, file dialogs, `nsh/mobile` and
//! `nsh/desktop`, i18n, accessibility, capabilities in `nsh.json` (owned
//! by this area), icons and app metadata. Owned by the platform area. The
//! shell mounts `platform_page` and `device_page`; keep their names and
//! signatures.

mod clipboard_card;
mod desktop_card;
mod environment;
mod files;
mod i18n_card;
mod keyboard;
pub mod locale;
mod picker_card;
mod safe_area;
mod speech_card;
mod tabbar_card;
mod widgets;

use nsh_base::theme;
use nsh_core::prelude::*;

use widgets::localized;

/// One page of the platform area: its content in a scroll view that sits
/// inside the safe area on every edge, and lifts above the keyboard. The
/// keyboard and the safe area both own the bottom edge, so one computed
/// padding decides it (the keyboard covers the bottom inset while it shows).
/// The scroll carries `scroll_id` so captures and tests can scroll the page.
fn page(title: &'static str, scroll_id: &'static str, cards: Vec<AnyElement>) -> AnyElement {
    scroll()
        .flex_1()
        .content_inset(crate::ui::layout::page_insets(0.0))
        .test_id(scroll_id)
        .child(
            view()
                // The gallery's frame (`ui::layout::page_body`), with the
                // sides kept clear of a landscape notch. UIKit insets the
                // scroll under the header already, so the top is no inset.
                .w_full()
                .flex_col()
                .pt(px(16.0))
                .pr_safe_area_plus(px(16.0))
                .pl_safe_area_plus(px(16.0))
                .pb(move || {
                    let above = if keyboard_height().0 > 0.0 {
                        keyboard_height().0
                    } else {
                        safe_area().bottom.0
                    };
                    px(above + 16.0)
                })
                .gap_6()
                .text_color(move || theme().foreground)
                .child(crate::ui::text::page_heading(localized(title)))
                .children(cards),
        )
        .into_element()
}

/// The platform page: safe areas, keyboard, clipboard, picker, speech, file
/// dialogs, language and RTL, the desktop's windows and menus, the system tab
/// bar.
pub fn platform_page() -> impl IntoElement {
    page(
        "platform.title",
        "platform-page-scroll",
        vec![
            safe_area::card(),
            clipboard_card::card(),
            picker_card::card(),
            speech_card::card(),
            files::card(),
            i18n_card::card(),
            desktop_card::card(),
            tabbar_card::card(),
            // The keyboard card last: its field sits at the bottom, where the
            // keyboard reaches.
            keyboard::card(),
        ],
    )
}

/// The device and environment page: what the platform exposes about itself.
pub fn device_page() -> impl IntoElement {
    page(
        "platform.device.page_title",
        "device-page-scroll",
        environment::cards(),
    )
}
