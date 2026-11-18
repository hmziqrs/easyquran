//! Canvas and graphics: the drawing app on `canvas` with responder input
//! and undo, a signal-driven frame loop, `Plot` charts, `shader` with
//! WGSL, SVG, images with placeholder and blur-up, video and `web_view`
//! with messaging, plus the ports of the gpui-starter canvas pages: the
//! shape editor, the bézier editor, the node graph, the particle field,
//! the text and image specimens, and the shared board with demo peers.
//! Owned by the canvas area. The shell mounts `canvas_page`; keep its
//! name and signature.

mod bezier;
mod canvas_image;
mod canvas_text;
mod charts;
mod collab;
mod media;
mod node_graph;
mod particles;
mod shader;
mod shape_edit;
mod simulation;
mod sketch;
mod svg;
mod web;
mod world;

use crate::ui;
use nsh_base::theme;
use nsh_core::prelude::*;

/// The colors a canvas paints with, resolved for the appearance now. Read
/// it inside a canvas's binding so a change of appearance redraws it.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub background: Hsla,
    pub foreground: Hsla,
    pub border: Hsla,
    pub muted_foreground: Hsla,
    pub series: [Hsla; 3],
}

impl Palette {
    pub(crate) fn now() -> Self {
        let t = theme();
        Self {
            background: color(t.background),
            foreground: color(t.foreground),
            border: color(t.border),
            muted_foreground: color(t.muted_foreground),
            series: [color(t.chart_1), color(t.chart_2), color(t.chart_3)],
        }
    }
}

/// The `series`-th chart token, as a style color: swatch fills and legend
/// entries share it with the marks.
pub(crate) fn series_tint(series: usize) -> AdaptiveColor {
    let t = theme();
    [t.chart_1, t.chart_2, t.chart_3][series % 3]
}

/// A section: the ui section header over a card around one demo's body,
/// with the demo's elements as its subtitle (the pages' shared shape).
pub(crate) fn section(
    title: &'static str,
    subtitle: &'static str,
    body: impl IntoElement,
) -> impl IntoElement {
    ui::layout::section(
        title,
        view()
            .flex_col()
            .gap_2()
            .child(ui::text::muted(subtitle))
            .child(
                view()
                    .flex_col()
                    .gap(px(12.0))
                    .p(px(14.0))
                    .rounded(move || theme().radius)
                    .border_1()
                    .border_color(move || theme().border)
                    .bg(move || theme().card)
                    .child(body),
            ),
    )
}

/// A quiet caption bound to its state. The id rides a wrapper view: the
/// ui text is opaque, so a `test_id` cannot land on it.
pub(crate) fn caption(
    id: &'static str,
    content: impl IntoReactive<SharedString>,
) -> impl IntoElement {
    view().test_id(id).child(ui::text::muted(content))
}

/// A small press control for Undo, Clear and the like: the ui outline
/// button, small. The label is reactive, so toggles read the state they
/// flip; returns the base element so callers can hang a `test_id` on it.
pub(crate) fn tool_button(
    label: impl IntoReactive<SharedString>,
    disabled: impl IntoReactive<bool>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    ui::button::button_with(
        label,
        ui::button::ButtonVariant::Outline,
        ui::button::ButtonSize::Small,
        disabled,
        None,
        on_press,
    )
}

/// The canvas and graphics page, one section per element the area covers.
pub fn canvas_page() -> impl IntoElement {
    view().flex_1().bg(move || theme().background).child(
        scroll()
            .flex_1()
            .content_inset(crate::ui::layout::page_insets(0.0))
            .child(
                ui::layout::page_body()
                    .child(ui::text::page_intro(
                        "Canvas and graphics",
                        "Drawing, animation, charts, shaders, SVG, media, the web view, and the gpui-starter canvas pages ported: editors, graph, particles, text and image specimens, shared board",
                    ))
                    .child(sketch::sketch())
                    .child(simulation::frame_loop())
                    .child(charts::charts())
                    .child(shader::shader_demo())
                    .child(svg::svg_demo())
                    .child(media::media())
                    .child(web::messaging())
                    .child(world::world_board())
                    .child(shape_edit::shape_editing())
                    .child(bezier::bezier_editor())
                    .child(node_graph::node_graph())
                    .child(particles::particle_field())
                    .child(canvas_text::text_on_canvas())
                    .child(canvas_image::image_on_canvas())
                    .child(collab::shared_board()),
            ),
    )
}
