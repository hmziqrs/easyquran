//! The brand (PARITY A5): the open-pages mark, the wordmark beside it and
//! the bismillah calligraphy, from the web's `lib/components/brand`. The
//! SVGs are the web's own paths (`lib/assets/open-pages.svg`,
//! `Bismillah.svelte`), filled with `currentColor` so the caller's text
//! color tints them.

use nsh_base::theme;
use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

// nsh_core's prelude has a `t!` of its own; the app translates with rust-i18n.

/// The open-pages mark (64×64).
pub const BRAND_MARK_SVG: &str = include_str!("../../assets/brand/open-pages.svg");
/// The bismillah calligraphy (176×36).
pub const BISMILLAH_SVG: &str = include_str!("../../assets/brand/bismillah.svg");

/// The mark alone, `side` points square, in the inherited text color
/// (set `.text_color(..)` to tint it). Decorative: no accessible name.
pub fn brand_mark(side: f32) -> Svg {
    svg(ImageSrc::svg_markup(BRAND_MARK_SVG))
        .size(px(side))
        .accessible(false)
}

/// The brand lockup (`Brand.svelte`): the mark in its 32-point holder
/// (primary fill, on-primary mark) and the lowercase wordmark, its second
/// half in the primary color. `on_accent` inverts it for a primary
/// surface. Named for screen readers by the app's name.
pub fn brand(on_accent: bool) -> View {
    let name = crate::APP_NAME.to_lowercase();
    let split = name.char_indices().nth(4).map_or(name.len(), |(at, _)| at);
    let (head, tail) = name.split_at(split);
    let (head, tail) = (head.to_owned(), tail.to_owned());
    let holder_bg = move || {
        if on_accent {
            theme().primary_foreground
        } else {
            theme().primary
        }
    };
    let mark_color = move || {
        if on_accent {
            theme().primary
        } else {
            theme().primary_foreground
        }
    };
    let word_color = move || {
        if on_accent {
            theme().primary_foreground
        } else {
            theme().foreground
        }
    };
    let tail_color = move || {
        if on_accent {
            theme().primary_foreground
        } else {
            theme().primary
        }
    };
    view()
        .flex_row()
        .items_center()
        .gap(px(10.0))
        .accessible(true)
        .label(t!("app.name").to_string())
        .child(
            view()
                .size_8()
                .items_center()
                .justify_center()
                .rounded(crate::ui::tokens::radius_sm)
                .bg(holder_bg)
                .child(brand_mark(28.0).text_color(mark_color)),
        )
        .child(
            text(head)
                .text_size(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .letter_spacing(px(-0.5))
                .text_color(word_color)
                .child(text(tail).text_color(tail_color)),
        )
}

/// The bismillah calligraphy, `width` points wide at its 176:36 ratio, in
/// the inherited text color. Decorative unless `label` names it.
pub fn bismillah(width: f32, label: Option<&'static str>) -> Svg {
    let art = svg(ImageSrc::svg_markup(BISMILLAH_SVG))
        .w(px(width))
        .h(px(width * 36.0 / 176.0));
    match label {
        Some(label) => art.accessible(true).label(label),
        None => art.accessible(false),
    }
}
