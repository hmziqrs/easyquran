//! Tabs, toggles and the toolbar: the app's segmented selection row, its
//! on/off buttons, and the bar that groups quiet actions.

use nsh_base::{
    tab, tabs, theme, toggle as base_toggle, toggle_group, toolbar, toolbar_group,
    toolbar_separator,
};
use nsh_core::prelude::*;

/// The app's tab row: a segmented control whose indicator the base moves.
/// The selection is the caller's; panels are shown beside it.
pub fn tab_bar(
    selected: impl Fn() -> usize + Copy + 'static,
    labels: &'static [&'static str],
    on_select: impl Fn(usize) + Copy + 'static,
) -> impl IntoElement {
    tabs()
        .flex_row()
        .self_start()
        .gap_1()
        .p_1()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .indicator_style(|s| {
            s.top(px(4.0))
                .bottom(px(4.0))
                .rounded_sm()
                .bg(move || theme().background)
                .shadow_sm()
        })
        .children(labels.iter().enumerate().map(move |(at, label)| {
            let is_selected = move || selected() == at;
            tab()
                .selected(is_selected)
                .on_press(move |_: &()| on_select(at))
                .px_3()
                .py_1p5()
                .rounded_sm()
                .transition_colors()
                .active(|s| s.opacity(0.8))
                .focus(|s| {
                    s.outline(px(2.0))
                        .outline_offset(px(1.0))
                        .outline_color(move || theme().ring)
                })
                .disabled_style(|s| s.opacity(0.5))
                .child(
                    text(*label)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || {
                            if is_selected() {
                                theme().foreground
                            } else {
                                theme().muted_foreground
                            }
                        }),
                )
        }))
}

/// An on/off button, alone or in [`toggle_bar`]: bold, a favorite star.
pub fn toggle(
    label: impl IntoReactive<SharedString>,
    pressed: Signal<bool>,
    on_change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    base_toggle()
        .pressed(pressed)
        .on_change(move |next: &bool| on_change(*next))
        .px_2()
        .py_1p5()
        .rounded_sm()
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.opacity(0.8))
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .disabled_style(|s| s.opacity(0.5))
        .pressed_style(|s| {
            s.bg(move || theme().accent)
                .text_color(move || theme().accent_foreground)
        })
        .child(text(label).text_sm().text_color(move || theme().foreground))
}

/// A toolbar of toggles: one keyboard stop, arrows move between them.
pub fn toggle_bar(items: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    toggle_group()
        .flex_row()
        .gap_1()
        .p_1()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .children(items)
}

/// The app's toolbar: a quiet row of buttons, toggles and separators.
pub fn toolbar_bar(items: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    toolbar().flex_row().items_center().gap_1().children(items)
}

/// A group inside a toolbar, its items kept together.
pub fn toolbar_group_of(items: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    toolbar_group()
        .flex_row()
        .items_center()
        .gap_1()
        .children(items)
}

/// A hairline between toolbar groups.
pub fn toolbar_divider() -> impl IntoElement {
    toolbar_separator()
        .self_stretch()
        .w(px(1.0))
        .bg(move || theme().border)
}
