//! The keyboard card: a field near the bottom that stays visible with the
//! keyboard up (the page pads above it), the keyboard's height live, and a
//! way to dismiss it.

use nsh_core::prelude::*;

use super::widgets::{action_button, field, hint, info_row, localized, panel};

#[component]
fn KeyboardCard() -> impl IntoElement {
    let draft = signal(String::new());
    panel(
        "platform.keyboard.title",
        [
            info_row("platform.keyboard.height", || {
                format!("{} pt", keyboard_height().0.round())
            }),
            hint("platform.keyboard.hint"),
            field(
                "platform-keyboard-draft",
                "platform.keyboard.placeholder",
                draft,
            )
            .into_element(),
            view()
                .flex_row()
                .justify_end()
                .child(action_button(
                    "platform-keyboard-dismiss",
                    localized("platform.keyboard.dismiss"),
                    dismiss_keyboard,
                ))
                .into_element(),
        ],
    )
}

/// The keyboard card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(keyboard_card()).into_element()
}
