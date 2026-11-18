//! Selection controls and value controls: switch, checkbox, radio group,
//! slider, progress. Every part is styled here, or it draws nothing.

use nsh_base::{
    SliderValue, checkbox, progress as base_progress, radio, radio_group, slider as base_slider,
    switch, theme,
};
use nsh_core::prelude::*;

/// A labelled switch row: the track and thumb are the app's to draw; the
/// flip writes `checked`.
pub fn switch_row(label: impl Into<SharedString>, checked: Signal<bool>) -> impl IntoElement {
    let label: SharedString = label.into();
    switch_row_with(label.clone(), checked, move |next| checked.set(next), label)
}

/// The switch row with the change taken by `on_change` (an effect, a
/// form field, an optimistic write) and `name` for screen readers.
pub fn switch_row_with(
    label: impl IntoReactive<SharedString>,
    checked: impl IntoReactive<bool>,
    on_change: impl Fn(bool) + 'static,
    name: impl Into<SharedString>,
) -> impl IntoElement {
    switch()
        .checked(checked.into_reactive())
        .accessibility_label(name.into())
        .on_change(move |next: &bool| on_change(*next))
        .flex_row()
        .items_center()
        .gap_3()
        .rounded_sm()
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .disabled_style(|s| s.opacity(0.5))
        .track_style(|s| {
            s.w(px(44.0))
                .h(px(26.0))
                .p(px(3.0))
                .rounded_full()
                // `muted` and `input` vanish on a white card; the off track
                // needs a real edge against both surfaces.
                .bg(move || theme().muted_foreground.opacity(0.3))
                .transition_colors()
                .checked(|s| s.bg(move || theme().primary))
        })
        .thumb_style(|s| {
            s.size(px(20.0))
                .rounded_full()
                .bg(move || theme().background)
                .shadow_sm()
                .transition_transform()
                .checked(|s| s.translate_x(px(18.0)))
        })
        .child(
            text(label.into_reactive())
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

/// A labelled checkbox row with its indicator; the label is what screen
/// readers hear, not the mark next to it.
pub fn checkbox_row(label: impl Into<SharedString>, checked: Signal<bool>) -> impl IntoElement {
    let label: SharedString = label.into();
    checkbox()
        .checked(checked)
        .accessibility_label(label.clone())
        .on_change(move |next: &bool| checked.set(*next))
        .flex_row()
        .items_center()
        .gap_2()
        .rounded_sm()
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .disabled_style(|s| s.opacity(0.5))
        .indicator_style(|s| {
            s.size(px(18.0))
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_2()
                .border_color(move || theme().input)
                .transition_colors()
                .checked(|s| {
                    s.bg(move || theme().primary)
                        .border_color(move || theme().primary)
                })
        })
        .indicator(
            text("✓")
                .text_xs()
                .text_color(move || theme().primary_foreground),
        )
        .child(
            text(label.into_reactive())
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

/// A checkbox's three states for a "select all" over a partial selection.
pub fn checkbox_tri_state(
    label: impl Into<SharedString>,
    selected: Signal<usize>,
    total: usize,
) -> impl IntoElement {
    let label: SharedString = label.into();
    checkbox()
        .checked(move || selected.get() == total)
        .indeterminate(move || selected.get() > 0 && selected.get() < total)
        .accessibility_label(label.clone())
        .on_change(move |all: &bool| selected.set(if *all { total } else { 0 }))
        .flex_row()
        .items_center()
        .gap_2()
        .rounded_sm()
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .indicator_style(|s| {
            s.size(px(18.0))
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_2()
                .border_color(move || theme().input)
                .transition_colors()
                .checked(|s| {
                    s.bg(move || theme().primary)
                        .border_color(move || theme().primary)
                })
                .indeterminate(|s| {
                    s.bg(move || theme().primary)
                        .border_color(move || theme().primary)
                })
        })
        .indicator(
            text(move || {
                if selected.get() == total {
                    "✓".to_owned()
                } else if selected.get() > 0 {
                    "–".to_owned()
                } else {
                    String::new()
                }
            })
            .text_xs()
            .text_color(move || theme().primary_foreground),
        )
        .child(
            text(label.into_reactive())
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

/// A radio choice: pick one of a small, visible set of options.
pub fn radio_row(
    label: impl IntoReactive<SharedString>,
    checked: Signal<bool>,
) -> impl IntoElement {
    radio()
        .checked(checked)
        .on_change(move |next: &bool| checked.set(*next))
        .flex_row()
        .items_center()
        .gap_2()
        .rounded_sm()
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .disabled_style(|s| s.opacity(0.5))
        .child(
            view()
                .size(px(18.0))
                .rounded_full()
                .border_2()
                .border_color(move || theme().input)
                .transition_colors()
                .items_center()
                .justify_center()
                .style_when(checked, |s| s.border_color(move || theme().primary))
                .child(
                    view()
                        .size(px(8.0))
                        .rounded_full()
                        .transition_colors()
                        .style_when(checked, |s| s.bg(move || theme().primary)),
                ),
        )
        .child(
            text(label.into_reactive())
                .text_sm()
                .text_color(move || theme().foreground),
        )
}

/// A labelled radio group over `options`; the value is the chosen label.
pub fn radio_group_control(
    options: &'static [&'static str],
    value: Signal<SharedString>,
) -> impl IntoElement {
    radio_group()
        .flex_col()
        .gap_2()
        .children(options.iter().map(|option| {
            let option = *option;
            radio()
                .checked(computed(move || value.get().as_ref() == option))
                .on_change(move |_: &bool| value.set(SharedString::from(option)))
                .flex_row()
                .items_center()
                .gap_2()
                .rounded_sm()
                .focus(|s| {
                    s.outline(px(2.0))
                        .outline_offset(px(2.0))
                        .outline_color(move || theme().ring)
                })
                .disabled_style(|s| s.opacity(0.5))
                .child(
                    view()
                        .size(px(18.0))
                        .rounded_full()
                        .border_2()
                        .border_color(move || theme().input)
                        .transition_colors()
                        .items_center()
                        .justify_center()
                        .style_when(computed(move || value.get().as_ref() == option), |s| {
                            s.border_color(move || theme().primary)
                        })
                        .child(
                            view()
                                .size(px(8.0))
                                .rounded_full()
                                .transition_colors()
                                .style_when(
                                    computed(move || value.get().as_ref() == option),
                                    |s| s.bg(move || theme().primary),
                                ),
                        ),
                )
                .child(
                    text(option)
                        .text_sm()
                        .text_color(move || theme().foreground),
                )
        }))
}

/// A slider with its track, range, filled indicator and thumb styled.
pub fn slider(value: Signal<f64>, min: f64, max: f64) -> impl IntoElement {
    base_slider()
        .value(move || Some(SliderValue::Single(value.get())))
        .on_change(move |next: &SliderValue| value.set(next.start()))
        .min(min)
        .max(max)
        .step(1.0)
        .accessibility_label("Value")
        .rounded_full()
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .track_style(|s| {
            s.h(px(6.0))
                .rounded_full()
                .bg(move || theme().muted)
                .overflow_hidden()
        })
        .indicator_style(|s| s.h_full().rounded_full().bg(move || theme().primary))
        .range_style(|s| s.h_full().bg(move || theme().primary.opacity(0.35)))
        .thumb_style(|s| {
            s.size(px(18.0))
                .rounded_full()
                .bg(move || theme().background)
                .border_2()
                .border_color(move || theme().primary)
                .shadow_sm()
        })
}

/// A determinate progress bar with its track and fill styled.
pub fn progress(value: impl IntoReactive<f32>) -> impl IntoElement {
    base_progress()
        .value(value)
        .accessibility_label("Progress")
        .track_style(|s| {
            s.w_full()
                .h(px(6.0))
                .rounded_full()
                .bg(move || theme().muted)
                .overflow_hidden()
        })
        .indicator_style(|s| s.h_full().rounded_full().bg(move || theme().primary))
}

/// An indeterminate progress bar, for work whose extent is unknown.
pub fn progress_indeterminate() -> impl IntoElement {
    base_progress()
        .indeterminate(true)
        .accessibility_label("Working")
        .track_style(|s| {
            s.w_full()
                .h(px(6.0))
                .rounded_full()
                .bg(move || theme().muted)
                .overflow_hidden()
        })
        .indicator_style(|s| {
            s.h_full()
                .w(px(64.0))
                .rounded_full()
                .bg(move || theme().primary)
        })
}
