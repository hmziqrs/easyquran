//! The tree: items that hold items, its rows drawn here.

use nsh_base::{TreeEntry, TreeItem, theme, tree};
use nsh_core::prelude::*;

/// A row of the tree: its id, label and children.
pub fn tree_row(id: &'static str, label: &'static str, children: Vec<TreeItem>) -> TreeItem {
    let mut item = TreeItem::new(id, label);
    item.children = children;
    item
}

/// A tree over `items`; which rows are open is the app's (`expanded`),
/// the chosen row's id is reported.
pub fn tree_of(
    items: Vec<TreeItem>,
    expanded: Signal<Vec<SharedString>>,
    selected: Signal<SharedString>,
) -> impl IntoElement {
    tree()
        .items(items)
        .expanded(move || Some(expanded.get()))
        .on_expanded_change(move |next: &Vec<SharedString>| expanded.set(next.clone()))
        .selected(move || Some(selected.get()))
        .on_selected_change(move |next: &SharedString| selected.set(next.clone()))
        .accessibility_label("Tree")
        .list_style(|s| s.p_1().gap(px(1.0)))
        .item_class(
            "min-h-9 px-2 py-1.5 rounded-md text-sm text-foreground hover:bg-accent \
             focus:bg-accent selected:bg-primary selected:text-primary-foreground \
             disabled:opacity-50",
        )
        .render_item(|entry: &TreeEntry| {
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(if entry.folder { "▸" } else { "·" })
                        .text_xs()
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    text(entry.label.clone())
                        .text_sm()
                        .text_color(move || theme().foreground),
                )
        })
}
