//! Image on canvas: one committed asset drawn through `draw_image` at its
//! natural size and again at two scales, with a `draw_image_part` crop —
//! the port of the showcase's raster demo. Static: it draws, it takes no
//! input.

use nsh_base::theme;
use nsh_core::{
    Bounds, Points,
    element::{ImageAsset, asset},
    geometry::ScaledPixels,
    prelude::*,
};

use super::{Palette, caption, section};

/// The photo the media section already ships (128 × 64).
const PHOTO: ImageAsset = asset!("canvas/photo.png");

/// The specimen: the asset at 1×, 2× and a cropped detail. `width` is 0
/// before the layout.
fn draw(width: f32, height: f32) -> Painter {
    let c = Palette::now();
    let (w, h) = (PHOTO.size.width.0, PHOTO.size.height.0);
    let mut p = Painter::new()
        // A hairline frame per drawing, so the bounds read against the card.
        .stroke_color(c.border)
        .line_width(1.0)
        .stroke_rect(Bounds::new(point(px(12.0), px(12.0)), size(px(w), px(h))))
        .draw_image(
            PHOTO.url,
            Bounds::new(point(px(12.0), px(12.0)), size(px(w), px(h))),
        );
    if width >= w * 2.0 + 24.0 {
        p = p
            .stroke_rect(Bounds::new(
                point(px(12.0), px(12.0 + h + 14.0)),
                size(px(w * 2.0), px(h * 2.0)),
            ))
            .draw_image(
                PHOTO.url,
                Bounds::new(
                    point(px(12.0), px(12.0 + h + 14.0)),
                    size(px(w * 2.0), px(h * 2.0)),
                ),
            );
    }
    // The crop: the right half of the asset, drawn at 2× — the same idea
    // as the showcase's two-scale raster, one step further.
    if width >= w + 24.0 && height >= 12.0 + h * 3.0 + 28.0 + h {
        let top = 12.0 + h * 3.0 + 28.0;
        p = p
            .stroke_rect(Bounds::new(point(px(12.0), px(top)), size(px(w), px(h))))
            .draw_image_part(
                PHOTO.url,
                // The source crop is in image pixels: the right half of
                // the 128 × 64 asset.
                Some(Bounds::new(
                    point(ScaledPixels(w / 2.0), ScaledPixels(0.0)),
                    size(ScaledPixels(w / 2.0), ScaledPixels(h)),
                )),
                Bounds::new(point(px(12.0), px(top)), size(px(w), px(h))),
            );
    }
    p
}

/// The image specimen section.
#[component]
pub(crate) fn ImageOnCanvas() -> impl IntoElement {
    let canvas_width = signal(0.0_f32);
    let canvas_height = signal(0.0_f32);
    let height = 280.0;
    let canvas = canvas(move || draw(canvas_width.get(), height))
        .h(px(height))
        .test_id("canvas-image")
        .label("Image specimen")
        .on_layout(move |frame: &Bounds<Points>| {
            canvas_width.set(frame.size.width.0);
            canvas_height.set(frame.size.height.0);
        })
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().muted);
    view().child(section(
        "Image on canvas",
        "draw_image · one asset at two scales · draw_image_part crop",
        view()
            .gap_3()
            .child(canvas)
            .child(caption("canvas-image-note", move || {
                format!(
                    "128 × 64 drawn at 1×, 2× and as a right-half crop · canvas {} × {}",
                    canvas_width.get().round() as i32,
                    canvas_height.get().round() as i32
                )
            })),
    ))
}
