//! Disclosure: the collapsible region and the accordion of sections. The
//! trigger is the app's (a styled button); the base owns the behavior.

use nsh_base::{
    accordion, accordion_header, accordion_item, accordion_panel, accordion_trigger, collapsible,
    theme,
};
use nsh_core::prelude::*;

use crate::ui::button;

/// A disclosure region: `trigger` flips `open`, `content` expands under it
/// with a springing reveal.
pub fn collapsible_section(
    trigger: impl IntoReactive<SharedString>,
    open: Signal<bool>,
    content: impl IntoElement,
) -> impl IntoElement {
    let flip = open;
    collapsible()
        .open(move || open.get())
        .reveal(true)
        .flex_col()
        .gap_2()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .px_4()
        .py_3()
        .child(button::button_with(
            trigger,
            button::ButtonVariant::Ghost,
            button::ButtonSize::Small,
            false,
            None,
            move || flip.set(!flip.get()),
        ))
        .child(content)
}

/// An entry of an accordion: its id, title and revealed content.
pub struct AccordionEntry {
    /// The value in the accordion's selection.
    pub id: &'static str,
    /// The section's title.
    pub title: &'static str,
    /// What opens under the title.
    pub content: SharedString,
}

/// An accordion of entries: `value` holds the open ids (one unless
/// `multiple`).
pub fn accordion_of(
    entries: &[AccordionEntry],
    value: Signal<Vec<SharedString>>,
    multiple: bool,
) -> impl IntoElement {
    accordion()
        .value(value)
        .multiple(multiple)
        .flex_col()
        .gap_2()
        .on_change(move |next: &Vec<SharedString>| value.set(next.clone()))
        .children(entries.iter().map(|entry| {
            accordion_item()
                .value(entry.id)
                .flex_col()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().card)
                .child(
                    accordion_header().level(3).child(
                        accordion_trigger()
                            .min_h(px(44.0))
                            .px_4()
                            .py_2()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .transition_colors()
                            .hover(|s| s.bg(move || theme().accent))
                            .active(|s| s.opacity(0.8))
                            .focus(|s| {
                                s.outline(px(2.0))
                                    .outline_offset(px(-2.0))
                                    .outline_color(move || theme().ring)
                            })
                            .expanded_style(|s| s.bg(move || theme().accent))
                            .child(
                                text(entry.title)
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(move || theme().card_foreground),
                            )
                            .child(
                                text("▾")
                                    .text_xs()
                                    .text_color(move || theme().muted_foreground),
                            ),
                    ),
                )
                .child(
                    accordion_panel()
                        .reveal(true)
                        .keep_mounted(true)
                        .accessibility_label(entry.title)
                        .child(
                            view().px_4().pb_3().child(
                                text(entry.content.clone())
                                    .text_sm()
                                    .text_color(move || theme().muted_foreground),
                            ),
                        ),
                )
        }))
}
