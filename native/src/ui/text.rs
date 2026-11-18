//! The app's text styles: one function per role of the type scale (12-20
//! points around a 16-point body), so screens never set sizes directly,
//! plus the web's §11 ramp ([`TextRole`]: display, h1-h3, body, caption,
//! micro) for surfaces that mirror a web board. One Latin face, Onest
//! (the theme's `font-sans`); JetBrains Mono for code only.

use nsh_base::theme;
use nsh_core::prelude::*;

/// A page or screen title: 20pt semibold.
pub fn title(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_xl()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(move || theme().foreground)
}

/// A section header inside a page: 16pt semibold.
pub fn section_title(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_base()
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(move || theme().foreground)
}

/// Body copy: 14pt, the reading size for lists and cards.
pub fn body(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_sm()
        .text_color(move || theme().foreground)
}

/// Secondary text: hints, captions, metadata.
pub fn muted(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_sm()
        .text_color(move || theme().muted_foreground)
}

/// The small label above a form field: 12pt medium.
pub fn label(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(move || theme().foreground)
}

/// Inline code and identifiers: monospaced, in an inset surface.
pub fn code(label: impl IntoReactive<SharedString>) -> impl IntoElement {
    text(label.into_reactive())
        .text_xs()
        .font_family(mono_family)
        .px_1()
        .rounded(move || theme().radius)
        .bg(move || theme().muted)
        .text_color(move || theme().foreground)
}

/// The theme's monospaced family (JetBrains Mono, shipped in
/// `assets/fonts`), or the system's when a theme leaves it out. Code,
/// identifiers and figures that must line up use it; the rest of the app
/// is the theme's `font-sans` (Onest), which every text inherits.
pub fn mono_family() -> SharedString {
    theme()
        .font_mono
        .clone()
        .unwrap_or_else(|| "monospace".into())
}

/// Whether a page draws its own heading: not on phones, where the system
/// header already shows the page's title (iOS's large title, Android's
/// toolbar), so the two never repeat each other.
pub fn page_heading_shown() -> bool {
    use nsh_core::platform::Os;
    is_desktop() || !matches!(os(), Os::Ios | Os::Android)
}

/// A page's heading where it draws one (`page_heading_shown`).
pub fn page_heading(heading: impl IntoReactive<SharedString> + 'static) -> impl IntoElement {
    let heading = heading.into_reactive();
    when(page_heading_shown, move || {
        title(heading.clone()).into_element()
    })
}

/// A page's opening: its heading (desktops and the web) over one line of
/// what the page is, kept tight together so the pair reads as one block
/// above the sections.
pub fn page_intro(
    heading: impl IntoReactive<SharedString> + 'static,
    summary: impl IntoReactive<SharedString>,
) -> impl IntoElement {
    view()
        .gap_1()
        .child(page_heading(heading))
        .child(muted(summary))
}

/// A role of the web's §11 type ramp (`layout.css` `--text-*`): size,
/// line height, weight and tracking move together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    /// 76/1.06, 500, -0.04em: the landing headline.
    DisplayXl,
    /// 40/1.1, 500, -0.035em.
    H1,
    /// 26/1.2, 500, -0.03em.
    H2,
    /// 20/1.25, 500, -0.025em.
    H3,
    /// 20/1.55, 400: lead paragraphs.
    BodyXl,
    /// 17.5/1.6, 400.
    BodyL,
    /// 15/1.5, 400: the body.
    Body,
    /// 13.5/1.45, 400: captions, secondary lines.
    Caption,
    /// 13/1.4, 500: the smallest label.
    Micro,
}

impl TextRole {
    /// `(size, line height multiple, weight, tracking in em)`.
    pub fn metrics(self) -> (f32, f32, FontWeight, f32) {
        match self {
            TextRole::DisplayXl => (76.0, 1.06, FontWeight::MEDIUM, -0.04),
            TextRole::H1 => (40.0, 1.1, FontWeight::MEDIUM, -0.035),
            TextRole::H2 => (26.0, 1.2, FontWeight::MEDIUM, -0.03),
            TextRole::H3 => (20.0, 1.25, FontWeight::MEDIUM, -0.025),
            TextRole::BodyXl => (20.0, 1.55, FontWeight::NORMAL, 0.0),
            TextRole::BodyL => (17.5, 1.6, FontWeight::NORMAL, 0.0),
            TextRole::Body => (15.0, 1.5, FontWeight::NORMAL, 0.0),
            TextRole::Caption => (13.5, 1.45, FontWeight::NORMAL, 0.0),
            TextRole::Micro => (13.0, 1.4, FontWeight::MEDIUM, 0.0),
        }
    }
}

/// `label` set in `role` of the ramp, in the foreground color (override
/// with `.text_color(..)`).
pub fn role(label: impl IntoReactive<SharedString>, role: TextRole) -> Text {
    let (size, line, weight, tracking) = role.metrics();
    text(label.into_reactive())
        .text_size(px(size))
        .line_height(nsh_core::LineHeight::Multiple(line))
        .font_weight(weight)
        .letter_spacing(px(size * tracking))
        .text_color(move || theme().foreground)
}
