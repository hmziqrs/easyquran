//! Resizable panels: a split the user drags, its handle drawn here; the
//! sizes the drag reports come back to the panels.

use nsh_base::{h_resizable, resizable_panel, theme, v_resizable};
use nsh_core::prelude::*;

/// A row of panels with draggable handles between them; `sizes` holds
/// what the last drag reported, passed back so a rebuilt layout keeps.
pub fn h_resizable_panels(
    panels: Vec<impl IntoElement>,
    sizes: Signal<Vec<nsh_core::Points>>,
) -> nsh_base::ResizablePanelGroupElement {
    h_resizable()
        .on_resize(move |next: &Vec<nsh_core::Points>| sizes.set(next.clone()))
        .size_full()
        .handle_class("w-1.5 bg-border group-hover:bg-ring group-active:bg-ring")
        .handle_accessibility_label("Resize")
        .handle_thumb_class(
            "h-8 w-1 rounded-full bg-muted-foreground opacity-0 group-hover:opacity-60 \
             transition-opacity",
        )
        .children(panels.into_iter().map(|panel| {
            resizable_panel()
                .min_size(px(80.0))
                .flex_col()
                .min_w(px(0.0))
                .overflow_hidden()
                .child(panel)
        }))
}

/// A column of panels with draggable handles between them.
pub fn v_resizable_panels(
    panels: Vec<impl IntoElement>,
    sizes: Signal<Vec<nsh_core::Points>>,
) -> nsh_base::ResizablePanelGroupElement {
    v_resizable()
        .on_resize(move |next: &Vec<nsh_core::Points>| sizes.set(next.clone()))
        .size_full()
        .handle_class("h-1.5 bg-border group-hover:bg-ring group-active:bg-ring")
        .handle_accessibility_label("Resize")
        .handle_thumb_class(
            "w-8 h-1 rounded-full bg-muted-foreground opacity-0 group-hover:opacity-60 \
             transition-opacity",
        )
        .children(panels.into_iter().map(|panel| {
            resizable_panel()
                .min_size(px(80.0))
                .flex_col()
                .min_h(px(0.0))
                .overflow_hidden()
                .child(panel)
        }))
}

/// A fixed panel that keeps its width while the others share the rest.
pub fn fixed_panel(content: impl IntoElement, width: f32) -> impl IntoElement {
    resizable_panel()
        .fixed(true)
        .size(px(width))
        .flex_col()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .overflow_hidden()
        .child(content)
}
