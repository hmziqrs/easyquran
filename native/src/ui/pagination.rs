//! Pagination: the pages of a set, its items drawn here so the row reads
//! as one control.

use nsh_base::{PaginationItemState, pagination, theme};
use nsh_core::prelude::*;

/// A pagination row over `total_pages`; the chosen page is in `page`.
pub fn pagination_of(page: Signal<u32>, total_pages: u32) -> impl IntoElement {
    pagination()
        .page(page)
        .total_pages(total_pages)
        .accessibility_label("Pages")
        .flex_row()
        .items_center()
        .gap_1()
        .item_class(
            "min-w-9 min-h-9 px-2 items-center justify-center rounded-md text-sm \
             text-foreground hover:bg-accent active:bg-accent focus:outline-2 \
             focus:outline-ring selected:bg-primary selected:text-primary-foreground \
             disabled:opacity-50",
        )
        .render_item(|item: &PaginationItemState| {
            view().child(
                text(item_label(item))
                    .text_sm()
                    .font_weight(if item.current {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::MEDIUM
                    })
                    .text_color(move || theme().foreground),
            )
        })
        .on_change(move |next: &u32| page.set(*next))
}

fn item_label(item: &PaginationItemState) -> String {
    match item.kind {
        nsh_base::PaginationItemKind::Previous => "‹".to_owned(),
        nsh_base::PaginationItemKind::Next => "›".to_owned(),
        nsh_base::PaginationItemKind::Ellipsis => "…".to_owned(),
        nsh_base::PaginationItemKind::Page => item.page.to_string(),
    }
}
