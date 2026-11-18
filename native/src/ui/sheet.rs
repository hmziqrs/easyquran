//! The sheet: supplementary work on an edge panel, over a backdrop; on
//! phones the same structure is the bottom sheet.

use nsh_base::{SheetEvent, sheet, theme};
use nsh_core::prelude::*;

/// A panel sliding from `side`, shown while `open`; its dismissal (a
/// backdrop press, Escape, back, a swipe) writes `open` back.
pub fn sheet_of(
    open: Signal<bool>,
    side: Placement,
    content: impl Fn() -> nsh_core::element::AnyElement + 'static,
) -> impl IntoElement {
    sheet()
        .open(move || Some(open.get()))
        .on_open_change(move |e: &SheetEvent| {
            if let SheetEvent::OpenChange(next, _) = e {
                open.set(*next);
            }
        })
        .side(side)
        .popup_style(move |s| {
            let mut s = s
                .w(px(320.0))
                .max_w(px(320.0))
                .h_full()
                .bg(move || theme().popover)
                .text_color(move || theme().popover_foreground)
                .shadow_lg();
            // The corners open toward the screen edge the panel slides
            // from; the radius is a constant of the sheet family.
            if side == Placement::Right {
                s = s.rounded_l(crate::ui::tokens::radius_xl());
            }
            if side == Placement::Left {
                s = s.rounded_r(crate::ui::tokens::radius_xl());
            }
            if side == Placement::Top {
                s = s.rounded_b(crate::ui::tokens::radius_xl());
            }
            if side == Placement::Bottom {
                s = s.rounded_t(crate::ui::tokens::radius_xl());
            }
            s
        })
        .backdrop_style(|s| s.bg(crate::ui::tokens::backdrop()))
        .child(
            view()
                .size_full()
                .p_safe_area()
                .flex_col()
                .gap_3()
                .p_4()
                .child(content()),
        )
}
