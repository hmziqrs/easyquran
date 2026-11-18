//! Modal dialogs: the app's dialog wrapper (a full native layer) and the
//! system alert for consequential confirmations.

use nsh_base::{alert_dialog, dialog, theme};
use nsh_core::prelude::*;

/// A native modal over everything, shown while `open` is set: Escape, the
/// Android back, a backdrop press or a swipe call `on_cancel`.
pub fn dialog_of(
    open: Signal<bool>,
    on_cancel: impl Fn() + Clone + 'static,
    content: impl Fn() -> nsh_core::element::AnyElement + 'static,
) -> impl IntoElement {
    when(open, move || {
        dialog()
            .on_cancel(on_cancel.clone())
            .flex_1()
            .bg(move || theme().background)
            .text_color(move || theme().foreground)
            .child(view().flex_1().p_safe_area().child(content()))
    })
}

/// A modal that reads as a focused form: a page sheet that leaves the
/// presenting page visible.
pub fn dialog_sheet(
    open: Signal<bool>,
    on_cancel: impl Fn() + Clone + 'static,
    content: impl Fn() -> nsh_core::element::AnyElement + 'static,
) -> impl IntoElement {
    when(open, move || {
        dialog()
            .presentation(ModalPresentation::PageSheet)
            .on_cancel(on_cancel.clone())
            .flex_1()
            .bg(move || theme().background)
            .text_color(move || theme().foreground)
            .child(view().flex_1().p_safe_area().child(content()))
    })
}

/// A short, focused decision on a surface of the app's own (a card): the
/// dialog shows it while `open`, and `on_cancel` owns the dismissal.
pub fn dialog_card(
    open: Signal<bool>,
    title: SharedString,
    on_cancel: impl Fn() + Clone + 'static,
    body: impl Fn() -> nsh_core::element::AnyElement + 'static,
    actions: impl Fn() -> nsh_core::element::AnyElement + 'static,
) -> impl IntoElement {
    when(open, move || {
        dialog()
            .on_cancel(on_cancel.clone())
            .flex_1()
            .items_center()
            .justify_center()
            .p_4()
            .bg(crate::ui::tokens::backdrop())
            .child(
                view()
                    .w_full()
                    .max_w(px(340.0))
                    .flex_col()
                    .gap_3()
                    .rounded_lg()
                    .bg(move || theme().card)
                    .border_1()
                    .border_color(move || theme().border)
                    .p_4()
                    .shadow_lg()
                    .child(
                        text(title.clone())
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(move || theme().card_foreground),
                    )
                    .child(body())
                    .child(actions()),
            )
    })
}

/// The system's confirmation for a consequential action. `on_answer`
/// gets whether the user chose the destructive action (id `confirm`).
pub fn confirm_destructive(
    title: &'static str,
    description: &'static str,
    confirm: &'static str,
    on_answer: impl Fn(bool) + 'static,
) {
    spawn(async move {
        let answer = alert_dialog()
            .title(title)
            .description(description)
            .cancel("cancel", "Cancel")
            .destructive("confirm", confirm)
            .show()
            .await;
        let confirmed = matches!(answer, Ok(result) if result.action.as_deref() == Some("confirm"));
        on_answer(confirmed);
    });
}
