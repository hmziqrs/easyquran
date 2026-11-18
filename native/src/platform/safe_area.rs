//! The safe-area demo of the platform page: the page's own content sits
//! inside `p_safe_area`, and this card shows the insets as bars so the
//! covered edges are visible, not just numbers.

use nsh_base::theme;
use nsh_core::prelude::*;

use super::widgets::{hint, localized, panel, rule};

/// One inset as a bar of its height, the visible shape of the covered edge.
fn edge_band(edge: impl Fn() -> f32 + Copy + 'static, color: AdaptiveColor) -> AnyElement {
    let height = move || px(edge().max(2.0));
    let label = move || format!("{} pt", edge().round());
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            view()
                .flex_1()
                .h(height)
                .rounded(move || theme().radius)
                .bg(move || color.opacity(0.35)),
        )
        .child(
            text(label)
                .w(px(56.0))
                .text_sm()
                .tabular_nums()
                .text_right(),
        )
        .into_element()
}

/// The safe-area card of the platform page.
pub(crate) fn card() -> AnyElement {
    panel(
        "platform.safe_area.title",
        [
            edge_band(|| safe_area().top.0, theme().accent),
            edge_band(|| safe_area().bottom.0, theme().accent),
            edge_band(|| safe_area().left.0, theme().primary),
            edge_band(|| safe_area().right.0, theme().primary),
            rule(),
            view()
                .p_2()
                .rounded(move || theme().radius)
                .border_1()
                .border_dashed()
                .border_color(move || theme().border)
                .child(
                    text(localized("platform.safe_area.demo"))
                        .text_sm()
                        .text_center(),
                )
                .into_element(),
            hint("platform.safe_area.hint"),
        ],
    )
}
