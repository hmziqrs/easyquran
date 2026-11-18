//! The archive: `Pagination` over a generated set, ten rows a page. The
//! page is the app's; the pager reports the page to go to.

use crate::ui::pagination::pagination_of;
use crate::ui::text::muted;
use nsh_base::theme;
use nsh_core::prelude::*;

use super::heading;

/// How many entries the archive holds.
const ITEMS: usize = 500;

/// Rows on a page.
const PER_PAGE: usize = 10;

/// An archive entry's title.
fn title(n: usize) -> String {
    format!("Archive {n}")
}

/// One row of the current page; its `n` never changes under its key.
fn archive_row(n: usize) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .h(px(44.0))
        .px_3()
        .border_b_1()
        .border_color(move || theme().border.opacity(0.6))
        .child(
            text(title(n))
                .text_sm()
                .tabular_nums()
                .text_color(move || theme().foreground),
        )
        .child(
            text(format!("entry {n}"))
                .text_xs()
                .tabular_nums()
                .text_color(move || theme().muted_foreground),
        )
}

/// The Pagination demo: fifty pages of a generated archive.
#[component]
pub fn PagerDemo() -> impl IntoElement {
    let page = signal(1_u32);
    let pages = (ITEMS / PER_PAGE) as u32;
    // The page's first entry; the pager holds the page to the pages there
    // are, but the rows guard their range anyway.
    let first = move || ((page.get() as usize - 1) * PER_PAGE + 1).min(ITEMS);
    view()
        .flex_1()
        .gap_2()
        .pb_4()
        .child(heading("Pages").child(muted(move || -> SharedString {
            format!("{} entries, {} a page", ITEMS, PER_PAGE).into()
        })))
        .child(
            view()
                .flex_1()
                .rounded_lg()
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().card)
                .overflow_hidden()
                .child(each(
                    move || (first()..(first() + PER_PAGE).min(ITEMS + 1)).collect::<Vec<_>>(),
                    |n: &usize| *n,
                    |n, _| archive_row(n.get()),
                )),
        )
        .child(pagination_of(page, pages))
        .child(
            text(move || format!("Page {} of {}", page.get(), pages))
                .test_id("pager-caption")
                .text_sm()
                .tabular_nums()
                .text_color(move || theme().muted_foreground),
        )
}
