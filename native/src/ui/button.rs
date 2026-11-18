//! The app's buttons: one styled wrapper over `nsh_base::button`, with
//! the variants, sizes and the press, hover, focus and disabled states.

use nsh_base::{ButtonElement, button as base_button, theme};
use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

/// What a button means. Primary stays scarce: one commitment per area.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonVariant {
    /// The one emphasized commitment in a decision area.
    #[default]
    Primary,
    /// A quiet filled action (secondary emphasis).
    Secondary,
    /// An action that needs a clear boundary with less emphasis.
    Outline,
    /// Familiar, low-emphasis toolbar and inline actions.
    Ghost,
    /// Destructive commitment: delete, revoke.
    Danger,
}

/// How big a button is: typography, padding and hit target move together.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum ButtonSize {
    /// Toolbars and dense rows.
    Small,
    /// The default.
    #[default]
    Medium,
    /// Prominent decisions, empty states.
    Large,
}

/// The touch floor is a physical bound: 44 points on touch screens. The
/// desktops use the platform's own control heights (AppKit: 22, 28, 34).
fn min_height(size: ButtonSize) -> impl IntoReactive<Length> + use<> {
    let touch = !is_desktop();
    move || {
        px(match (size, touch) {
            (ButtonSize::Small, false) => 24.0,
            (ButtonSize::Small, true) => 40.0,
            (ButtonSize::Medium, false) => 28.0,
            (ButtonSize::Medium, true) => 44.0,
            (ButtonSize::Large, false) => 34.0,
            (ButtonSize::Large, true) => 48.0,
        })
    }
}

/// Desktop controls round less than touch ones, as AppKit's do.
fn corner() -> impl IntoReactive<Length> + use<> {
    let touch = !is_desktop();
    move || {
        if touch { theme().radius } else { px(6.0) }
    }
}

/// The app's primary action, medium: the default button.
pub fn button(
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    button_with(
        label,
        ButtonVariant::Primary,
        ButtonSize::Medium,
        false,
        None,
        on_press,
    )
}

/// The app's button with every choice open: variant, size, disabled, and
/// `name` (what screen readers hear when the label alone is not enough;
/// `None` names it from its label).
pub fn button_with(
    label: impl IntoReactive<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: impl IntoReactive<bool>,
    name: Option<&'static str>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let small = size == ButtonSize::Small;
    let touch = !is_desktop();
    let disabled = disabled.into_reactive();
    // The frame is decided once; the variants only add fill and a label.
    let base = move || {
        let named = base_button().on_press(on_press).disabled(disabled);
        let named = match name {
            Some(name) => named.accessibility_label(name),
            None => named,
        };
        named
            .min_h(min_height(size))
            .px(match (size, touch) {
                (ButtonSize::Small, _) => px(8.0),
                (ButtonSize::Medium, false) => px(12.0),
                _ => px(16.0),
            })
            .gap(if small { px(4.0) } else { px(6.0) })
            .flex_row()
            .items_center()
            .justify_center()
            .rounded(corner())
            .transition_colors()
            .active(move |s| {
                let s = s.opacity(0.85);
                // Desktop buttons darken without moving; touch ones give a
                // small press, unless the user asked for reduced motion.
                if touch && !reduce_motion() {
                    s.scale(0.98)
                } else {
                    s
                }
            })
            .focus(|s| {
                s.outline(px(2.0))
                    .outline_offset(px(2.0))
                    .outline_color(move || theme().ring)
            })
            .disabled_style(|s| s.opacity(0.5))
    };
    // Variants are decided while building (an argument), not per state.
    match variant {
        ButtonVariant::Primary => base()
            .bg(move || theme().primary)
            .shadow_xs()
            .hover(|s| s.bg(move || theme().primary.opacity(0.88)))
            .child(label_text(label, || theme().primary_foreground, size)),
        ButtonVariant::Secondary => base()
            .bg(move || theme().secondary)
            .hover(|s| s.bg(move || theme().accent))
            .child(label_text(label, || theme().secondary_foreground, size)),
        // The desktop push button: a raised surface with a hairline edge.
        ButtonVariant::Outline => base()
            .border_1()
            .border_color(move || theme().border)
            .bg(move || theme().card)
            .shadow_xs()
            .hover(|s| s.bg(move || theme().accent))
            .child(label_text(label, || theme().foreground, size)),
        ButtonVariant::Ghost => base()
            .hover(|s| s.bg(move || theme().accent))
            .child(label_text(label, || theme().foreground, size)),
        ButtonVariant::Danger => base()
            .bg(move || theme().destructive)
            .shadow_xs()
            .hover(|s| s.bg(move || theme().destructive.opacity(0.88)))
            .child(label_text(
                label,
                crate::ui::tokens::destructive_foreground,
                size,
            )),
    }
}

/// A choice or toggle button: a pressed-in selected state (the accent
/// fill, the ring border) over the outline button; `name` is what screen
/// readers hear.
pub fn choice_button(
    label: impl IntoReactive<SharedString>,
    name: &'static str,
    selected: impl IntoReactive<bool>,
    disabled: impl IntoReactive<bool>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    button_with(
        label,
        ButtonVariant::Outline,
        ButtonSize::Small,
        disabled,
        Some(name),
        on_press,
    )
    .selected(selected.into_reactive())
    .selected_style(|s| {
        s.bg(move || theme().accent)
            .border_color(move || theme().primary)
    })
}

/// A compact glyph-only action, for well-known symbols: `name` is what
/// screen readers hear and tests press.
pub fn icon_button(
    glyph: &'static str,
    name: &'static str,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    icon_frame(name, on_press).child(
        text(glyph)
            .text_base()
            .text_color(move || theme().foreground),
    )
}

/// A toolbar action drawn from an SVG icon (stroked in `currentColor`),
/// muted until hovered, as the desktops' toolbar buttons are.
pub fn icon_svg_button(
    svg_markup: &'static str,
    name: &'static str,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    icon_frame(name, on_press).child(
        svg(ImageSrc::svg_markup(svg_markup))
            .size(px(16.0))
            .text_color(move || theme().muted_foreground),
    )
}

fn icon_frame(name: &'static str, on_press: impl Fn() + 'static) -> ButtonElement {
    let side = move || {
        if is_desktop() { px(28.0) } else { px(40.0) }
    };
    base_button()
        .on_press(on_press)
        .accessibility_label(name)
        .min_h(side)
        .min_w(side)
        .flex_row()
        .items_center()
        .justify_center()
        .rounded(corner())
        .transition_colors()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.opacity(0.7))
        .focus(|s| {
            s.outline(px(2.0))
                .outline_offset(px(2.0))
                .outline_color(move || theme().ring)
        })
        .disabled_style(|s| s.opacity(0.5))
}

fn label_text(
    label: impl IntoReactive<SharedString>,
    color: impl Fn() -> AdaptiveColor + 'static,
    size: ButtonSize,
) -> impl IntoElement {
    let base = text(label.into_reactive())
        .text_color(color)
        .font_weight(FontWeight::MEDIUM)
        .number_of_lines(1);
    // AppKit sets control labels at 13 points (11 small); touch keeps the
    // type scale.
    match (size, is_desktop()) {
        (ButtonSize::Small, true) => base.text_size(px(12.0)).into_element(),
        (ButtonSize::Medium, true) => base.text_size(px(13.0)).into_element(),
        (ButtonSize::Large, true) => base.text_size(px(14.0)).into_element(),
        (ButtonSize::Small, false) => base.text_xs().into_element(),
        (ButtonSize::Medium, false) => base.text_sm().into_element(),
        (ButtonSize::Large, false) => base.text_base().into_element(),
    }
}
