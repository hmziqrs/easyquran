//! The popup family: menu, context menu, popover, hover card, tooltip.
//! One popup surface for all of them, so the anchored overlays do not
//! drift apart.

use nsh_base::{
    OpenChangeReason, PopoverEvent, button as base_button, context_menu, hover_card, menu,
    menu_item, menu_separator, popover, submenu, theme, tooltip,
};
use nsh_core::element::StyleRefinement;
use nsh_core::prelude::*;

/// The app's popup surface, shared by its menus, popovers and hover
/// cards: the popover tokens, one radius, one shadow.
pub fn popup_surface(s: StyleRefinement) -> StyleRefinement {
    s.min_w(px(192.0))
        .p_1()
        .rounded_lg()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().popover)
        .shadow_lg()
}

/// What a menu, popover or hover card sits on, on phones and desktops.
pub fn popup_backdrop(s: StyleRefinement) -> StyleRefinement {
    s.bg(crate::ui::tokens::backdrop())
}

/// The quiet entry/exit the popup parts animate: scale from 95%, fade.
fn popup_motion_class() -> &'static str {
    "transition-all duration-150 ease-out"
}

/// The closed control of a select, combobox or date picker: the framed
/// summary row with its chevron, which the base opens and places from.
pub fn popup_trigger_frame(
    summary: impl Fn() -> String + 'static,
    chevron: bool,
) -> impl IntoElement {
    let touch = !is_desktop();
    view()
        .min_h(move || px(if touch { 44.0 } else { 36.0 }))
        .px_3()
        .py_2()
        .flex_row()
        .items_center()
        .gap_2()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().input)
        .bg(move || theme().background)
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .focus(|s| {
            s.border_color(move || theme().ring)
                .outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .child(
            text(summary)
                .flex_1()
                .text_sm()
                .number_of_lines(1)
                .text_color(move || theme().foreground),
        )
        .when(chevron, |v| {
            v.child(
                text("▾")
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
            )
        })
}

/// One selectable command of a [`dropdown_menu`], a [`submenu_of`] or
/// [`context_menu_of`].
pub fn menu_entry(
    label: impl IntoReactive<SharedString>,
    on_select: impl Fn() + 'static,
) -> impl IntoElement {
    menu_item()
        .on_select(on_select)
        .px_2()
        .py_1p5()
        .rounded_sm()
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .focus(|s| s.bg(move || theme().accent))
        .disabled_style(|s| s.opacity(0.5))
        .child(
            text(label)
                .text_sm()
                .text_color(move || theme().popover_foreground),
        )
}

/// A command that destroys something: red text, still a normal item.
pub fn destructive_menu_entry(
    label: impl IntoReactive<SharedString>,
    on_select: impl Fn() + 'static,
) -> impl IntoElement {
    menu_item()
        .on_select(on_select)
        .px_2()
        .py_1p5()
        .rounded_sm()
        .transition_colors()
        .hover(|s| s.bg(move || theme().destructive))
        .active(|s| s.bg(move || theme().destructive))
        .focus(|s| s.bg(move || theme().destructive))
        .disabled_style(|s| s.opacity(0.5))
        .child(
            text(label)
                .text_sm()
                .text_color(crate::ui::tokens::destructive_foreground),
        )
}

/// A checkable command: `checked` is the app's; the mark is the app's.
pub fn checkable_menu_entry(
    label: impl IntoReactive<SharedString>,
    checked: Signal<bool>,
    on_change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    menu_item()
        .checked(move || Some(checked.get()))
        .close_on_select(false)
        .on_select(move || on_change(!checked.get_untracked()))
        .px_2()
        .py_1p5()
        .rounded_sm()
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .focus(|s| s.bg(move || theme().accent))
        .indicator(
            text("✓")
                .text_xs()
                .text_color(move || theme().popover_foreground),
        )
        .child(
            text(label)
                .text_sm()
                .text_color(move || theme().popover_foreground),
        )
}

/// A hairline between command groups of a menu.
pub fn menu_divider() -> impl IntoElement {
    menu_separator()
        .h(px(1.0))
        .my_1()
        .bg(move || theme().border)
}

/// A submenu opening off [`menu_entry`]-like rows of [`dropdown_menu`].
pub fn submenu_of(
    label: &'static str,
    items: impl IntoIterator<Item = SubmenuEntry>,
) -> impl IntoElement {
    submenu()
        .trigger(menu_entry(label, || ()))
        .children(items.into_iter().map(|entry| {
            let select = entry.on_select;
            let label = entry.label;
            if entry.destructive {
                destructive_menu_entry(label, select).into_element()
            } else {
                menu_entry(label, select).into_element()
            }
        }))
}

/// One command of a [`submenu_of`].
pub struct SubmenuEntry {
    /// What the row says.
    pub label: &'static str,
    /// What choosing it does.
    pub on_select: fn(),
    /// Destructive commands draw in the destructive color.
    pub destructive: bool,
}

/// A menu off a trigger: the overflow for secondary commands.
pub fn dropdown_menu(
    trigger_label: impl IntoReactive<SharedString>,
    items: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    menu()
        .offset(px(4.0))
        .popup_style(popup_surface)
        .popup_class(popup_motion_class())
        .popup_starting_class("opacity-0 scale-95")
        .popup_ending_class("opacity-0 scale-95")
        .backdrop_style(popup_backdrop)
        .trigger(
            base_button()
                .on_press(|| ())
                // The outline button's medium frame, so the trigger lines
                // up with the buttons beside it.
                .min_h(move || px(if is_desktop() { 36.0 } else { 44.0 }))
                .px_4()
                .flex_row()
                .items_center()
                .justify_center()
                .gap_1()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().input)
                .bg(move || theme().background)
                .transition_colors()
                .hover(|s| s.bg(move || theme().accent))
                .active(|s| s.opacity(0.8))
                .focus(|s| {
                    s.outline(px(2.0))
                        .outline_offset(px(2.0))
                        .outline_color(move || theme().ring)
                })
                .child(
                    text(trigger_label.into_reactive())
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().foreground),
                ),
        )
        .children(items)
}

/// Commands on the object under the pointer or finger: right click on the
/// desktops, a held finger on touch screens.
pub fn context_menu_of(
    anchored: impl IntoElement,
    items: impl IntoIterator<Item = impl IntoElement>,
) -> impl IntoElement {
    context_menu()
        .popup_style(popup_surface)
        .popup_class(popup_motion_class())
        .popup_starting_class("opacity-0 scale-95")
        .popup_ending_class("opacity-0 scale-95")
        .backdrop_style(popup_backdrop)
        .trigger(anchored)
        .children(items)
}

/// Contextual controls that do not interrupt the task, anchored to
/// `trigger`.
pub fn popover_of(
    trigger: impl IntoElement,
    content: impl IntoElement,
    on_open_change: impl Fn(bool) + 'static,
) -> impl IntoElement {
    popover()
        .offset(px(6.0))
        .margin(px(8.0))
        .popup_style(popup_surface)
        .popup_class(popup_motion_class())
        .popup_starting_class("opacity-0 scale-95")
        .popup_ending_class("opacity-0 scale-95")
        .on_open_change(move |e: &PopoverEvent| {
            if let PopoverEvent::OpenChange(open, _) = e {
                on_open_change(*open);
            }
        })
        .trigger(trigger)
        .child(content)
}

/// A non-blocking preview that follows the pointer's rest on `trigger`.
pub fn hover_card_of(trigger: impl IntoElement, content: impl IntoElement) -> impl IntoElement {
    hover_card()
        .offset(px(6.0))
        .margin(px(8.0))
        .popup_style(popup_surface)
        .popup_class(popup_motion_class())
        .popup_starting_class("opacity-0 scale-95")
        .popup_ending_class("opacity-0 scale-95")
        .trigger(trigger)
        .child(content)
}

/// A short hint on hover: an unfamiliar symbol, a hidden shortcut. Attach
/// it to the element with `.tooltip(..)`; it never carries what a touch
/// user needs.
pub fn tooltip_of(hint: impl IntoReactive<SharedString>) -> nsh_base::TooltipElement {
    tooltip()
        .offset(px(6.0))
        .starting_class("opacity-0 scale-95")
        .ending_class("opacity-0 scale-95")
        .child(
            view()
                .max_w(px(240.0))
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(move || theme().popover)
                .border_1()
                .border_color(move || theme().border)
                .shadow_md()
                .child(
                    text(hint)
                        .text_xs()
                        .text_color(move || theme().popover_foreground),
                ),
        )
}

/// Whether a dismissal was a deliberate cancel (Escape, back, outside).
pub fn was_cancelled(reason: &OpenChangeReason) -> bool {
    matches!(
        reason,
        OpenChangeReason::Cancel | OpenChangeReason::PressOutside | OpenChangeReason::BackdropPress
    )
}
