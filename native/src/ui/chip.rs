//! Chips (`lib/components/chip`): the 24-point pill badge (§19) and the
//! interactive filter chip (§33), whose active state is a primary fill and
//! is announced as selected, never shown by color alone.

use nsh_base::{ButtonElement, button as base_button, theme};
use nsh_core::prelude::*;

use crate::ui::tokens;

/// A static chip's tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ChipTone {
    /// The surface with a hairline.
    #[default]
    Default,
    /// The secondary hue's soft fill (a Makki badge).
    Accent,
    /// No fill.
    Ghost,
}

fn frame(view: View) -> View {
    view.flex_row()
        .items_center()
        .gap_1p5()
        .min_h_6()
        .px_2()
        .rounded_full()
        .border_1()
}

fn label_text(label: impl IntoReactive<SharedString>) -> Text {
    text(label.into_reactive())
        .text_size(px(11.0))
        .line_height(px(16.0))
        .font_weight(FontWeight::MEDIUM)
        .number_of_lines(1)
}

/// The editorial marker dot a chip may lead with.
fn dot() -> View {
    view()
        .size_1p5()
        .rounded_full()
        .bg(tokens::gold)
        .accessible(false)
}

/// A static chip: a short classification, `dot` leading it.
pub fn chip(label: impl IntoReactive<SharedString>, tone: ChipTone, with_dot: bool) -> View {
    let base = frame(view().self_start()).child(with_dot.then(dot));
    let styled = match tone {
        ChipTone::Default => base
            .border_color(move || theme().border)
            .bg(tokens::surface),
        ChipTone::Accent => base
            .border_color(AdaptiveColor::TRANSPARENT)
            .bg(tokens::gold_soft),
        ChipTone::Ghost => base.border_color(AdaptiveColor::TRANSPARENT),
    };
    let ink: fn() -> AdaptiveColor = match tone {
        ChipTone::Accent => tokens::gold_strong,
        _ => tokens::foreground_secondary,
    };
    styled.child(label_text(label).text_color(ink))
}

/// A filter chip: pressing it toggles `active`; the active chip carries
/// the primary fill and is selected for screen readers.
pub fn filter_chip(
    label: impl IntoReactive<SharedString> + Clone,
    name: &'static str,
    active: impl Fn() -> bool + Copy + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    base_button()
        .on_press(on_press)
        .accessibility_label(name)
        .selected(active)
        .flex_row()
        .items_center()
        .gap_1p5()
        .min_h(move || px(if is_desktop() { 24.0 } else { 32.0 }))
        .px_2()
        .rounded_full()
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
        .transition_colors()
        .hover(|s| s.bg(tokens::surface_hover))
        .active(|s| s.opacity(0.85))
        .focus(crate::ui::button::focus_ring)
        .disabled_style(|s| s.opacity(0.5))
        .selected_style(|s| {
            s.bg(move || theme().primary)
                .border_color(AdaptiveColor::TRANSPARENT)
        })
        .child(label_text(label).text_color(move || {
            if active() {
                theme().primary_foreground
            } else {
                tokens::foreground_secondary()
            }
        }))
}
