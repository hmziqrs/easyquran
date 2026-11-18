//! The `Dialog` and `AlertDialog` sections: a rename flow in a dialog the
//! app styles, and the system's alerts answering a real destructive flow.

use super::{
    ButtonVariant, caption, overlay_button, overlay_switch, scrim, status_line, text_field,
};
use nsh_base::{alert_dialog, dialog, theme};
use nsh_core::prelude::*;

/// A dialog card's shared geometry: centered over the scrim, above it,
/// leaving the corners to the scrim.
fn dialog_layer(card: impl IntoElement) -> impl IntoElement {
    view()
        .absolute()
        .inset_0()
        .flex_col()
        .items_center()
        .justify_center()
        .p_4()
        .pointer_events(PointerEvents::BoxNone)
        .child(
            view()
                .w_full()
                .max_w(px(360.0))
                .flex_col()
                .gap_4()
                .p_5()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().popover)
                .shadow_lg()
                .child(card),
        )
}

/// The scrim a custom dialog draws itself; pressing it asks to close.
fn dialog_backdrop(on_press: impl Fn() + 'static) -> impl IntoElement {
    view()
        .absolute()
        .inset_0()
        .style_with(scrim)
        .label("Dismiss")
        .focusable(false)
        .on_press(on_press)
}

/// `Dialog`: a rename flow — a trigger, a focused decision with a field,
/// `close_on_escape` as the decision it is, and the focus back on the
/// trigger when it closes.
pub(crate) fn dialog_section() -> impl IntoElement {
    let open = signal(false);
    let escape_closes = signal(true);
    let name = signal("release-notes.md".to_owned());
    let draft = signal(String::new());
    let status = signal("Closed".to_owned());
    let focus_note = signal("Focus: page".to_owned());
    let trigger_ref = element_ref();
    // Focus watches live while the page does, on whatever carries the ref.
    trigger_ref.on_focus_in(move || focus_note.set("Focus: rename trigger".to_owned()));
    trigger_ref.on_focus_out(move || focus_note.set("Focus: elsewhere".to_owned()));
    let open_dialog = move || {
        draft.set(name.get_untracked());
        open.set(true);
    };
    let close = move || {
        open.set(false);
        // The app restores focus to the trigger, where the task continues.
        trigger_ref.focus();
    };
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_3()
                .flex_wrap()
                .child(
                    overlay_button("Rename file…", ButtonVariant::Outline, move || {
                        status.set("Opened".to_owned());
                        open_dialog();
                    })
                    .element_ref(&trigger_ref),
                )
                .child(
                    text(move || name.get())
                        .text_sm()
                        .font_family(crate::ui::text::mono_family)
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(overlay_switch("Escape closes", escape_closes))
        .child(caption(
            "Escape, the back button and the scrim close it (when allowed); the dialog takes the focus while it is up.",
        ))
        .child(status_line({ let s = status; move || s.get() }))
        .child(status_line({ let s = focus_note; move || s.get() }))
        .child(when(open, move || {
                        let save = move || {
                name.set(draft.get_untracked());
                status.set(format!("Renamed to {}", draft.get_untracked()));
                close();
            };
            dialog()
                .animation(ModalAnimation::Fade)
                .close_on_escape(false)
                .close_on_backdrop_press(false)
                .close_on_swipe(false)
                .on_cancel(move || {
                    // Every attempt is reported; the flag decides.
                    if escape_closes.get_untracked() {
                        status.set("Closed with Escape".to_owned());
                        close();
                    } else {
                        status.set("Escape ignored (close_on_escape is off)".to_owned());
                    }
                })
                .child(dialog_backdrop({
                                        move || {
                        status.set("Closed with the scrim".to_owned());
                        close();
                    }
                }))
                .child(dialog_layer(
                    view()
                        .flex_col()
                        .gap_3()
                        .child(
                            text("Rename file")
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || theme().popover_foreground),
                        )
                        .child(
                            text_field(draft, "File name").test_id("rename-field"),
                        )
                        .child(
                            view()
                                .flex_row()
                                .justify_end()
                                .gap_2()
                                .child(
                                    overlay_button("Cancel", ButtonVariant::Ghost, {
                                                                                move || {
                                            status.set("Closed with Cancel".to_owned());
                                            close();
                                        }
                                    })                                    ,
                                )
                                .child(
                                    overlay_button("Save", ButtonVariant::Primary, save)                                        ,
                                ),
                        ),
                ))
        }))
}

/// `AlertDialog`: the system's alert, from a real flow — deleting a local
/// draft is destructive and named, and the answer lands in data.
pub(crate) fn alert_section() -> impl IntoElement {
    let draft_exists = signal(true);
    let last_answer = signal("No alert yet".to_owned());
    let ask = move |is_sheet: bool| {
        let mut options = alert_dialog()
            .title("Delete “Release notes” draft?")
            .description("The draft is kept on this device only. This cannot be undone.")
            .cancel("cancel", "Cancel");
        options = if is_sheet {
            options
                .action("duplicate", "Duplicate first")
                .destructive("delete", "Delete draft")
                .action_sheet()
        } else {
            options.destructive("delete", "Delete draft")
        };
        spawn(async move {
            match options.show().await {
                Ok(result) => match result.action.as_deref() {
                    Some("delete") => {
                        draft_exists.set(false);
                        last_answer.set("Deleted the draft".to_owned());
                    }
                    Some("duplicate") => {
                        last_answer.set("Duplicated instead".to_owned());
                    }
                    Some(_) => last_answer.set("Kept the draft".to_owned()),
                    None => last_answer.set("Dismissed without a choice".to_owned()),
                },
                Err(err) => last_answer.set(format!("Alert failed: {err}")),
            }
        });
    };
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    overlay_button("Delete draft…", ButtonVariant::Destructive, move || {
                        ask(false)
                    })
                    .disabled_style(|s| s.opacity(0.4))
                    .disabled(move || !draft_exists.get()),
                )
                .child(
                    overlay_button("Share options…", ButtonVariant::Outline, move || ask(true)),
                ),
        )
        .child(caption(
            "The platform draws the alert (an action sheet for Share options); the answer reaches the app's data.",
        ))
        .child(status_line({ let s = last_answer; move || s.get() }))
        .child(status_line(move || {
            if draft_exists.get() {
                "Draft: kept".to_owned()
            } else {
                "Draft: deleted".to_owned()
            }
        }))
}
