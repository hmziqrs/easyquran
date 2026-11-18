//! The clipboard card: copy the field to the system clipboard and paste it
//! back, through the UI only (the headless clipboard answers both).

use nsh_core::prelude::*;

use super::widgets::{action_button, field, hint, localized, panel, quiet_button};

#[component]
fn ClipboardCard() -> impl IntoElement {
    let draft = signal(String::new());
    let owner = Owner::current();
    panel(
        "platform.clipboard.title",
        [
            field(
                "platform-clipboard-draft",
                "platform.clipboard.placeholder",
                draft,
            )
            .into_element(),
            hint("platform.clipboard.hint"),
            view()
                .flex_row()
                .gap_2()
                .child(action_button(
                    "platform-clipboard-copy",
                    localized("platform.clipboard.copy"),
                    move || {
                        set_clipboard(draft.get_untracked());
                    },
                ))
                .child(quiet_button(
                    "platform-clipboard-paste",
                    localized("platform.clipboard.paste"),
                    move || {
                        spawn(async move {
                            let text = clipboard_string().await;
                            if owner.is_live() {
                                draft.set(text);
                            }
                        });
                    },
                ))
                .into_element(),
        ],
    )
}

/// The clipboard card, as the platform page places it.
pub(crate) fn card() -> AnyElement {
    view().child(clipboard_card()).into_element()
}
