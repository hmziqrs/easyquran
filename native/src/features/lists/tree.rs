//! The workspace tree: `Tree` with the app owning the expanded and
//! selected items, each row drawn by `renderItem` around the label the
//! tree would have shown.

use nsh_base::{TreeEntry, TreeItem, theme, tree};
use nsh_core::prelude::*;

use super::{heading, note};

/// A file of the workspace, named by its last path part.
fn file(path: &str) -> TreeItem {
    let name = path.rsplit('/').next().unwrap_or(path);
    TreeItem::new(path.to_owned(), name.to_owned())
}

/// The files of the workspace, folders with children.
fn workspace() -> Vec<TreeItem> {
    vec![
        TreeItem::new("src", "src").children([
            TreeItem::new("src/ui", "ui").children([
                file("src/ui/button.rs"),
                file("src/ui/card.rs"),
                file("src/ui/theme.rs"),
            ]),
            TreeItem::new("src/features", "features")
                .children([file("src/features/lists.rs"), file("src/features/forms.rs")]),
            file("src/lib.rs"),
        ]),
        TreeItem::new("docs", "docs")
            .children([file("docs/architecture.md"), file("docs/coverage.md")]),
        file("README.md"),
    ]
}

/// One row: indented by its depth, a chevron while it holds items (its
/// direction follows the app's expanded set), a dot for files. Owned data,
/// so the renderer's borrow ends here.
fn workspace_row(entry: TreeEntry, expanded: Signal<Vec<SharedString>>) -> impl IntoElement {
    let id = entry.id;
    let label = entry.label;
    let folder = entry.folder;
    let depth = entry.depth;
    view()
        .flex_row()
        .items_center()
        .flex_1()
        .child(view().w(px(depth as f32 * 16.0)))
        .child(when(
            move || folder,
            move || {
                let id = id.clone();
                let open = move || expanded.get().iter().any(|e| e == &id);
                text(move || if open() { "▾" } else { "▸" })
                    .text_xs()
                    .w(px(14.0))
                    .text_center()
                    .text_color(move || theme().muted_foreground)
            },
        ))
        .child(when(
            move || !folder,
            move || {
                text("·")
                    .text_xs()
                    .w(px(14.0))
                    .text_center()
                    .text_color(move || theme().muted_foreground)
            },
        ))
        .child(text(label).text_sm().flex_1())
}

/// The Tree demo: expand, collapse and select, the app owning the state.
#[component]
pub fn TreeDemo() -> impl IntoElement {
    let items = workspace();
    let expanded = signal(vec![SharedString::from("src")]);
    let selected = signal(None::<SharedString>);
    let status = move || match selected.get() {
        Some(id) => format!("Selected {id}"),
        None => "Nothing selected".to_owned(),
    };
    view()
        .flex_1()
        .gap_2()
        .pb_4()
        .child(heading("Tree").child(note("Items that hold items; expansion is the app's state")))
        .child(
            tree()
                .items(items)
                .expanded(move || Some(expanded.get()))
                .selected(move || selected.get())
                .accessibility_label("Workspace")
                .list_class("p-2")
                .item_class(
                    "flex-row items-center h-9 pr-2 rounded-md text-sm \
                     hover:bg-accent/50 selected:bg-accent selected:text-accent-foreground",
                )
                .on_expanded_change(move |ids: &Vec<SharedString>| expanded.set(ids.clone()))
                .on_selected_change(move |id: &SharedString| selected.set(Some(id.clone())))
                .render_item(move |entry: &TreeEntry| workspace_row(entry.clone(), expanded))
                .h(px(360.0))
                .rounded_lg()
                .border_1()
                .border_color(move || theme().border)
                .bg(move || theme().card)
                .overflow_hidden(),
        )
        .child(
            text(status)
                .test_id("tree-status")
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}
