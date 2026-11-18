//! Lists and data views: the `Table` parts over an order book, a sectioned
//! list with sticky headers, `DataTable` over three thousand quotes with
//! sorting and a fixed column, a `Tree`, a `Calendar`, a grid of cards with
//! viewability, an inverted chat that loads at both ends, a `Carousel` and
//! `Pagination`. Owned by the lists area. The shell mounts `lists_page`;
//! keep its name and signature.

pub mod calendar_view;
pub mod carousel_view;
pub mod chat;
pub mod grid;
pub mod orders;
pub mod pager;
pub mod quotes;
pub mod tree;

use crate::ui::text::{muted, section_title};
use nsh_base::{button, theme};
use nsh_core::prelude::*;

/// The demos the page switches among, one list or data view each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoView {
    Orders,
    Quotes,
    Tree,
    Calendar,
    Grid,
    Chat,
    Carousel,
    Pager,
}

const VIEWS: [(DemoView, &str); 8] = [
    (DemoView::Orders, "Orders"),
    (DemoView::Quotes, "Quotes"),
    (DemoView::Tree, "Tree"),
    (DemoView::Calendar, "Calendar"),
    (DemoView::Grid, "Grid"),
    (DemoView::Chat, "Chat"),
    (DemoView::Carousel, "Carousel"),
    (DemoView::Pager, "Pages"),
];

/// The lists page: the demo switcher over the selected view. The shell
/// owns the page's header and safe areas; this is content only.
pub fn lists_page() -> impl IntoElement {
    let active = signal(DemoView::Orders);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .gap_3()
        .p_4()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(VIEWS.map(|(view, label)| chip(active, view, label))),
        )
        // A view's state is its own; switching remakes it, as the tabs of
        // the gallery's lists demo do.
        .child(dynamic(move || {
            let content = match active.get() {
                DemoView::Orders => orders::orders_demo().into_element(),
                DemoView::Quotes => quotes::quotes_demo().into_element(),
                DemoView::Tree => tree::tree_demo().into_element(),
                DemoView::Calendar => calendar_view::calendar_demo().into_element(),
                DemoView::Grid => grid::grid_demo().into_element(),
                DemoView::Chat => chat::chat_demo().into_element(),
                DemoView::Carousel => carousel_view::carousel_demo().into_element(),
                DemoView::Pager => pager::pager_demo().into_element(),
            };
            view().flex_1().child(content)
        }))
}

/// One switcher chip: the base button with the app's pill look, named
/// for tests so a label collision with the shell's nav cannot miss.
fn chip(active: Signal<DemoView>, me: DemoView, label: &'static str) -> impl IntoElement {
    let mine = move || active.get() == me;
    let chip = button()
        .selected(mine)
        .accessibility_label(label)
        .hit_slop(6.0)
        .h(px(32.0))
        .px(px(14.0))
        .flex_row()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(move || {
            if mine() {
                theme().primary
            } else {
                theme().border
            }
        })
        .style_when(mine, |s| s.bg(move || theme().primary))
        .style_when(move || !mine(), |s| s.bg(move || theme().card))
        .active(|s| s.opacity(0.7))
        .on_press(move || active.set(me))
        .child(
            text(label)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || {
                    if mine() {
                        theme().primary_foreground
                    } else {
                        theme().muted_foreground
                    }
                }),
        );
    view()
        .test_id(&format!("lists-tab-{}", key_of(me)))
        .child(chip)
}

/// The stable key a view names itself by in test ids.
fn key_of(view: DemoView) -> &'static str {
    match view {
        DemoView::Orders => "orders",
        DemoView::Quotes => "quotes",
        DemoView::Tree => "tree",
        DemoView::Calendar => "calendar",
        DemoView::Grid => "grid",
        DemoView::Chat => "chat",
        DemoView::Carousel => "carousel",
        DemoView::Pager => "pager",
    }
}

/// A view's title line in the app's section style; the caller adds its
/// own muted note under it.
pub(crate) fn heading(title: &'static str) -> View {
    view().child(section_title(title))
}

/// A muted line of supporting text under a heading.
pub(crate) fn note(line: &'static str) -> impl IntoElement {
    muted(line)
}
