//! SVG two ways: the `svg` element showing markup (a native drawing
//! through resvg on iOS, Android, macOS, Windows and Linux; an image the
//! browser draws on the web), and SVG path data drawn through a canvas
//! painter as `Path2D` fills and strokes.

use nsh_base::theme;
use nsh_core::{
    canvas::{FillRule, LineCap, LineJoin, svg_path},
    element::ImageSrc,
    prelude::*,
};

use super::{Palette, caption, section};

/// The glyphs: a name, SVG path data and whether it strokes.
const GLYPHS: [(&str, &str, bool); 3] = [
    (
        "Heart",
        "M12 21s-7-4.35-9.5-8.5C.5 8.5 3 4 7 4c2 0 3.5 1 5 3 1.5-2 3-3 5-3 4 0 6.5 4.5 4.5 8.5C19 16.65 12 21 12 21z",
        false,
    ),
    (
        "Star",
        "M12 2l3.09 6.26L22 9.27l-5 4.87L18.18 21 12 17.27 5.82 21 7 14.14l-5-4.87 6.91-1.01z",
        false,
    ),
    ("Check", "M20 6L9 17l-5-5", true),
];

/// An inline SVG badge. Its colors are the document's raster content,
/// not interface colors. `r##` : the markup holds `"#` sequences.
const BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48" width="48" height="48">
  <rect x="2" y="2" width="44" height="44" rx="12" fill="#2e1040"/>
  <path d="M10 30 L20 18 L28 26 L38 12" fill="none" stroke="#ff9e5e" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="38" cy="12" r="4" fill="#5ed0b2"/>
</svg>"##;

/// A glyph tile: the path parsed once, drawn again per appearance.
fn glyph(name: &str, d: &str, stroked: bool) -> impl IntoElement {
    let path = svg_path(d);
    let drawing = move || {
        let c = Palette::now();
        let p = Painter::new().scale(30.0 / 24.0, 30.0 / 24.0);
        if stroked {
            p.stroke_color(c.series[1])
                .line_width(2.5)
                .line_cap(LineCap::Round)
                .line_join(LineJoin::Round)
                .stroke_path(&path)
        } else {
            p.fill_color(c.series[1])
                .fill_path(&path, FillRule::NonZero)
        }
    };
    view()
        .flex_1()
        .items_center()
        .gap_2()
        .child(
            view()
                .size(px(56.0))
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().muted)
                .items_center()
                .justify_center()
                .child(canvas(drawing).size(px(30.0))),
        )
        .child(
            text(name.to_owned())
                .text_xs()
                .text_color(move || theme().muted_foreground),
        )
}

/// The SVG section.
#[component]
pub(crate) fn SvgDemo() -> impl IntoElement {
    let tiles = GLYPHS.map(|(name, d, stroked)| glyph(name, d, stroked));
    view().child(section(
        "SVG",
        "svg markup · Path2D from path data",
        view()
            .gap_3()
            .child(
                view()
                    .flex_row()
                    .gap_3()
                    .items_center()
                    .child(
                        view()
                            .size(px(56.0))
                            .rounded(move || theme().radius)
                            .overflow_hidden()
                            .border_1()
                            .border_color(move || theme().border)
                            .bg(move || theme().muted)
                            .items_center()
                            .justify_center()
                            .child(svg(ImageSrc::svg_markup(BADGE)).size(px(40.0))),
                    )
                    .children(tiles),
            )
            .child(caption("svg-note", move || {
                if cfg!(target_arch = "wasm32") {
                    "markup: the browser draws it · paths: parsed natively".to_owned()
                } else {
                    "markup: resvg draws it natively · paths: parsed natively".to_owned()
                }
            })),
    ))
}
