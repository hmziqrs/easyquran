//! Surfaces and small marks: the card, the section, the badge, the
//! skeleton, the divider. Screens compose these instead of drawing boxes.

use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui::text;

/// A card: the app's raised content surface, with its foreground set so
/// children inherit readable text.
pub fn card() -> nsh_core::element::View {
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .rounded(crate::ui::tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .p_4()
        .text_color(move || theme().card_foreground)
}

/// A card that can be pressed (`Card interactive`): the hairline
/// strengthens under the pointer; `name` is what screen readers hear.
pub fn pressable_card(
    name: impl Into<SharedString>,
    on_press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(name.into())
        .w_full()
        .flex_col()
        .gap_3()
        .rounded(crate::ui::tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .p_4()
        .text_color(move || theme().card_foreground)
        .transition_colors()
        .hover(|s| s.border_color(crate::ui::tokens::border_strong))
        .active(|s| s.bg(crate::ui::tokens::surface_hover))
        .focus(crate::ui::button::focus_ring)
}

/// A card's header (`CardHeader`): its title and, under it, a
/// description, tight together.
pub fn card_header(
    title: impl IntoReactive<SharedString>,
    description: Option<SharedString>,
) -> impl IntoElement {
    view()
        .flex_col()
        .gap_1()
        .child(
            text(title.into_reactive())
                .text_base()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().card_foreground),
        )
        .child(description.map(|d| {
            text(d)
                .text_sm()
                .text_color(move || theme().muted_foreground)
        }))
}

/// A card's footer (`CardFooter`): actions in a row at the end, under a
/// hairline.
pub fn card_footer(actions: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_end()
        .gap_2()
        .pt_3()
        .border_t_1()
        .border_color(move || theme().border)
        .children(actions)
}

/// The edge-to-edge metric card (`MetricCard`): brand hue `hue` (1-4) as
/// the fill, an icon in its 46-point holder, a large value, its label and
/// an optional caption. Display only (no states).
pub fn metric_card(
    hue: u8,
    icon_svg: &'static str,
    value: impl IntoReactive<SharedString>,
    label: impl IntoReactive<SharedString>,
    caption: Option<SharedString>,
) -> impl IntoElement {
    use crate::ui::text::{TextRole, role};
    use crate::ui::tokens;
    let on = move || tokens::on_hue(hue);
    view()
        .flex_col()
        .justify_between()
        .gap_8()
        .px_8()
        .py_10()
        .bg(move || tokens::hue(hue))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    view()
                        .size(px(46.0))
                        .items_center()
                        .justify_center()
                        .rounded(tokens::radius_sm)
                        .bg(move || tokens::on_hue(hue).opacity(0.2))
                        .child(
                            svg(nsh_core::element::ImageSrc::svg_markup(icon_svg))
                                .size(px(23.0))
                                .text_color(on),
                        ),
                )
                .child(
                    text(value.into_reactive())
                        .text_size(px(44.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(on),
                ),
        )
        .child(
            view()
                .flex_col()
                .gap(px(5.0))
                .child(role(label, TextRole::H3).text_color(on))
                .child(caption.map(|c| {
                    role(c, TextRole::Caption).text_color(move || tokens::on_hue(hue).opacity(0.88))
                })),
        )
}

/// A callout panel's fill (`Panel`).
#[derive(Clone, Copy, PartialEq, Default)]
pub enum PanelVariant {
    /// The primary fill (a call to action).
    #[default]
    Accent,
    /// The soft primary tint with a hairline (a quiet callout).
    Soft,
    /// Inverted: foreground fill, background text.
    Ink,
}

/// The large callout panel: the 14-point large-panel radius, generous
/// padding, its text color set for the children.
pub fn panel(variant: PanelVariant) -> nsh_core::element::View {
    use crate::ui::tokens;
    let base = view()
        .w_full()
        .flex_col()
        .gap_4()
        .overflow_hidden()
        .rounded(tokens::radius_xl)
        .p_8();
    match variant {
        PanelVariant::Accent => base
            .bg(move || theme().primary)
            .text_color(move || theme().primary_foreground),
        PanelVariant::Soft => base
            .bg(tokens::primary_soft)
            .border_1()
            .border_color(move || theme().border)
            .text_color(move || theme().foreground),
        PanelVariant::Ink => base
            .bg(move || theme().foreground)
            .text_color(move || theme().background),
    }
}

/// An empty state (`Empty`): an icon in a muted holder, a title, what to
/// do next, and the action that does it, centered with room around it.
pub fn empty_state(
    icon_svg: Option<&'static str>,
    title: impl IntoReactive<SharedString>,
    description: impl IntoReactive<SharedString>,
    action: Option<AnyElement>,
) -> impl IntoElement {
    view()
        .w_full()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_4()
        .p_6()
        .rounded(crate::ui::tokens::radius_xl)
        .border_1()
        .border_dashed()
        .border_color(move || theme().border)
        .child(icon_svg.map(|icon| {
            view()
                .size_8()
                .items_center()
                .justify_center()
                .rounded(crate::ui::tokens::radius_sm)
                .bg(move || theme().muted)
                .child(
                    svg(nsh_core::element::ImageSrc::svg_markup(icon))
                        .size_4()
                        .text_color(move || theme().foreground),
                )
        }))
        .child(
            view()
                .flex_col()
                .items_center()
                .gap_2()
                .max_w(px(384.0))
                .child(
                    text(title.into_reactive())
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_center()
                        .text_color(move || theme().foreground),
                )
                .child(
                    text(description.into_reactive())
                        .text_sm()
                        .text_center()
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(action)
}

/// A small live dot in the success color with its soft halo
/// (`StatusDot`): online, synced. Decorative; say the state in text too.
pub fn status_dot() -> impl IntoElement {
    view()
        .size(px(14.0))
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(move || crate::ui::tokens::success().opacity(0.22))
        .accessible(false)
        .child(
            view()
                .size_2()
                .rounded_full()
                .bg(crate::ui::tokens::success),
        )
}

/// The primary pulse (`Pulse`): a 6-point dot that breathes, still under
/// reduced motion. Decorative.
pub fn pulse() -> nsh_core::element::View {
    let dot = view()
        .size(px(6.0))
        .rounded_full()
        .bg(move || theme().primary)
        .accessible(false);
    if reduce_motion() {
        return dot;
    }
    dot.animate_pulse()
}

/// A section of a page: a title, then the children, separated from the
/// next section by space alone (no nested cards).
pub fn section(
    title: impl IntoReactive<SharedString>,
    content: impl IntoElement,
) -> impl IntoElement {
    view()
        .flex_col()
        .gap_2()
        .child(text::section_title(title))
        .child(content)
}

/// A badge's variant (`badgeVariants`): keep most badges neutral.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum BadgeVariant {
    /// The primary fill.
    #[default]
    Default,
    /// The quiet surface.
    Secondary,
    /// An error or destructive state: a danger tint, danger text.
    Destructive,
    /// A hairline, no fill.
    Outline,
    /// No frame.
    Ghost,
    /// Primary text only.
    Link,
}

fn foreground() -> AdaptiveColor {
    theme().foreground
}

fn destructive_tint() -> AdaptiveColor {
    theme().destructive.opacity(0.1)
}

fn no_fill() -> AdaptiveColor {
    AdaptiveColor::TRANSPARENT
}

fn border() -> AdaptiveColor {
    theme().border
}

/// A badge of the web's variants: a 20-point pill of 12-point medium text.
pub fn badge_of(label: impl IntoReactive<SharedString>, variant: BadgeVariant) -> impl IntoElement {
    let (bg, edge, fg): (ToneColor, ToneColor, ToneColor) = match variant {
        BadgeVariant::Default => (primary_bg, no_fill, primary_fg),
        BadgeVariant::Secondary => (crate::ui::tokens::surface_hover, no_fill, foreground),
        BadgeVariant::Destructive => (destructive_tint, no_fill, danger_bg),
        BadgeVariant::Outline => (no_fill, border, foreground),
        BadgeVariant::Ghost => (no_fill, no_fill, foreground),
        BadgeVariant::Link => (no_fill, no_fill, primary_bg),
    };
    view()
        .self_start()
        .flex_row()
        .items_center()
        .gap_1()
        .min_h(px(20.0))
        .px_2()
        .py_0p5()
        .rounded_full()
        .border_1()
        .border_color(edge)
        .bg(bg)
        .child(
            text(label.into_reactive())
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .number_of_lines(1)
                .text_color(fg),
        )
}

/// A short status or count, tinted by its tone; keep most badges neutral.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum BadgeTone {
    /// No meaning: the muted surface.
    #[default]
    Neutral,
    /// The primary action's color.
    Primary,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Information.
    Info,
    /// Danger and errors.
    Danger,
}

/// A tone's color as a plain function, so a match can hand it around.
type ToneColor = fn() -> AdaptiveColor;

fn neutral_bg() -> AdaptiveColor {
    theme().muted
}

fn neutral_fg() -> AdaptiveColor {
    theme().muted_foreground
}

fn primary_bg() -> AdaptiveColor {
    theme().primary
}

fn primary_fg() -> AdaptiveColor {
    theme().primary_foreground
}

fn danger_bg() -> AdaptiveColor {
    theme().destructive
}

/// A badge: text on a tinted pill, one row high.
pub fn badge(label: impl IntoReactive<SharedString>, tone: BadgeTone) -> impl IntoElement {
    let (bg, fg): (ToneColor, ToneColor) = match tone {
        BadgeTone::Neutral => (neutral_bg, neutral_fg),
        BadgeTone::Primary => (primary_bg, primary_fg),
        BadgeTone::Success => (
            crate::ui::tokens::success,
            crate::ui::tokens::success_foreground,
        ),
        BadgeTone::Warning => (
            crate::ui::tokens::warning,
            crate::ui::tokens::warning_foreground,
        ),
        BadgeTone::Info => (crate::ui::tokens::info, crate::ui::tokens::info_foreground),
        BadgeTone::Danger => (danger_bg, crate::ui::tokens::destructive_foreground),
    };
    view()
        .self_start()
        .px_2()
        .py_0p5()
        .rounded_full()
        .bg(bg)
        .child(
            text(label.into_reactive())
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(fg),
        )
}

/// The placeholder box for content that is loading (`Skeleton`): drawn in
/// the known shape of what will replace it, the hover fill pulsing (still
/// under reduced motion), and only while content really is loading.
pub fn skeleton() -> impl IntoElement {
    skeleton_of(16.0)
}

/// A skeleton of another height, in points (a physical stand-in, not
/// rhythm): match the row it replaces.
pub fn skeleton_of(height: f32) -> nsh_core::element::View {
    let bar = view()
        .w_full()
        .h(px(height))
        .rounded(crate::ui::tokens::radius_md)
        .bg(crate::ui::tokens::surface_hover)
        .accessible(false);
    if reduce_motion() {
        return bar;
    }
    bar.animate_pulse()
}

/// A hairline between two regions (`Separator`, horizontal): the boundary
/// owner draws it.
pub fn divider() -> impl IntoElement {
    view()
        .w_full()
        .h(px(1.0))
        .flex_shrink_0()
        .bg(move || theme().border)
}

/// A vertical hairline (`Separator orientation="vertical"`) between the
/// items of a row; it takes the row's height.
pub fn divider_vertical() -> impl IntoElement {
    view()
        .w(px(1.0))
        .self_stretch()
        .flex_shrink_0()
        .bg(move || theme().border)
}

/// A small section label (`Eyebrow`): the caption role, medium, muted,
/// sentence case (no spaced-out uppercase).
pub fn eyebrow(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    crate::ui::text::role(label, crate::ui::text::TextRole::Caption)
        .font_weight(FontWeight::MEDIUM)
        .text_color(crate::ui::tokens::muted_text)
}

/// A page's content column, the frame shared by every page: 16pt
/// around, 24pt between sections, room under the last one. Put it inside the
/// page's scroll and give it `text::page_intro` first.
pub fn page_body() -> nsh_core::element::View {
    view()
        .w_full()
        .flex_col()
        .gap_6()
        .p_4()
        .pb_safe_area_plus(px(24.0))
        .text_color(move || theme().foreground)
}

/// The height the phone tab bar floats over at the bottom of a page, above
/// the safe area: iOS's Liquid Glass bar and Android's themed one. Zero on
/// the desktops and the web, which show no tab bar.
pub fn tab_bar_room() -> f32 {
    use nsh_core::platform::Os;
    if is_desktop() {
        return 0.0;
    }
    match os() {
        Os::Ios => 64.0,
        Os::Android => 72.0,
        _ => 0.0,
    }
}

/// A page scroll's content inset on phones: `top` above the content, and
/// room at the end so the last row scrolls clear of the floating tab bar
/// (the content passes under the bar on its way there, as under the
/// header). Read where the scroll is made: `.content_inset(page_insets(0.0))`.
pub fn page_insets(top: f32) -> impl Fn() -> nsh_core::EdgeInsets + 'static {
    move || {
        let room = tab_bar_room();
        let bottom = if room > 0.0 {
            room + safe_area().bottom.0
        } else {
            0.0
        };
        nsh_core::EdgeInsets {
            top: px(top),
            bottom: px(bottom),
            ..nsh_core::EdgeInsets::default()
        }
    }
}
