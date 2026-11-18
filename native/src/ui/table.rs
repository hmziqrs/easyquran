//! Tables: the small static table with a table's semantics, and the data
//! table over any number of rows, both styled here.

use nsh_base::{
    DataTableClasses, DataTableColumn, DataTableRows, table, table_body, table_cell, table_head,
    table_header, table_row, theme,
};
use nsh_core::prelude::*;

/// A small static table: `header` titles over rows of cells. Put it in a
/// horizontal `scroll` when it can overflow.
pub fn table_of(header: &[&'static str], rows: &[Vec<SharedString>]) -> impl IntoElement {
    table()
        .accessibility_label("Table")
        .w_full()
        .flex_col()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .overflow_hidden()
        .child(
            table_header()
                .border_b_1()
                .border_color(move || theme().border)
                .bg(move || theme().muted)
                .child(table_row().children(header.iter().map(|title| {
                    table_head().px_3().py_2().child(
                        text(*title)
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(move || theme().muted_foreground),
                    )
                }))),
        )
        .child(table_body().children(rows.iter().map(|row| {
            table_row()
                .border_b_1()
                .border_color(move || theme().border)
                .children(row.iter().map(|cell| {
                    table_cell().px_3().py_2().child(
                        text(cell.clone())
                            .text_sm()
                            .text_color(move || theme().card_foreground),
                    )
                }))
        })))
}

/// A column of a [`data_table_of`]: its key joins it to the rows' field.
pub fn column(key: &'static str, title: &'static str, width: f32) -> DataTableColumn {
    let mut column = DataTableColumn::default();
    column.key = key.into();
    column.title = Some(title.into());
    column.width = Some(px(width));
    column.sortable = true;
    column
}

/// The app's data table look: the box, its header, its rows and cells.
fn table_classes() -> DataTableClasses {
    let mut classes = DataTableClasses::default();
    classes.root = Some("rounded-lg border border-border bg-card overflow-hidden".into());
    classes.header = Some("border-b border-border bg-muted".into());
    classes.header_cell = Some("px-3 py-2 text-xs font-medium text-muted-foreground".into());
    classes.row = Some("border-b border-border".into());
    classes.row_odd = Some("bg-muted/50".into());
    classes.row_hover = Some("hover:bg-accent".into());
    classes.row_selected = Some("bg-primary/10".into());
    classes.header_cell_sorted = Some("text-foreground font-medium".into());
    classes.sort_indicator = Some("ml-1 text-muted-foreground".into());
    classes.sort_ascending = Some("text-primary".into());
    classes.sort_descending = Some("text-primary".into());
    classes.cell = Some("px-3 py-2 text-sm text-card-foreground".into());
    classes.empty = Some("p-6 text-sm text-muted-foreground".into());
    classes
}

/// A virtual data table over `rows` (by column key) and `columns`, in the
/// app's look. Give it a bounded height, or it fills its box.
pub fn data_table_of(
    columns: Vec<DataTableColumn>,
    rows: DataTableRows,
) -> nsh_base::DataTableElement {
    nsh_base::data_table()
        .columns(columns)
        .data(rows)
        .row_height(px(36.0))
        .stripe(true)
        .classes(table_classes())
        .border_1()
        .border_color(move || theme().border)
        .rounded(move || theme().radius)
}
