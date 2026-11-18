//! The picker card: the system's media picker answers with the picked image
//! as a local file; its name and type show, nothing uploads.

use nsh_base::theme;
use nsh_core::{picker::PickRequest, prelude::*};

// nsh_core 0.5.0's prelude has a `t!` of its own; the app translates with rust-i18n.
use rust_i18n::t;

use super::{
    locale,
    widgets::{action_button, hint, localized, panel},
};

#[component]
fn PickerCard() -> impl IntoElement {
    // The first file of the last pick; none when it was cancelled.
    let picked = signal(None::<nsh_core::picker::PickedFile>);
    let owner = Owner::current();
    panel(
        "platform.picker.title",
        [
            action_button(
                "platform-picker-pick",
                localized("platform.picker.pick"),
                move || {
                    spawn(async move {
                        let files = pick_images(PickRequest::new(false, 1)).await;
                        if owner.is_live() {
                            picked.set(files.into_iter().next());
                        }
                    });
                },
            )
            .into_element(),
            // One binding for both states: the placeholder translates with
            // the locale, the name arrives when the pick answers.
            text(move || {
                locale::app_locale().get();
                picked
                    .with(|file| {
                        file.as_ref()
                            .map(|file| format!("{} · {}", file.name, file.mime))
                    })
                    .unwrap_or_else(|| t!("platform.picker.none").to_string())
            })
            .text_sm()
            .tabular_nums()
            .text_color(move || theme().muted_foreground)
            .into_element(),
            hint("platform.picker.hint"),
        ],
    )
}

/// The picker card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(picker_card()).into_element()
}
