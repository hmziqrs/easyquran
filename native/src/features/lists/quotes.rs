//! Quotes: `DataTable` over three thousand generated rows, sorted by the
//! app (the table only reports the column's next sort), with the symbol
//! column fixed at the left and the titles staying over the rows.

use std::cmp::Ordering;

use crate::ui::table::{column as ui_column, data_table_of};
use crate::ui::text::muted;
use nsh_base::{
    DataTableAlign, DataTableColumn, DataTableRows, DataTableSelection, DataTableSort,
    DataTableSortDirection,
};
use nsh_core::prelude::*;

use super::heading;

/// How many rows the table holds.
const ROWS: usize = 3000;

/// The tickers the symbols cycle through.
const TICKERS: [&str; 24] = [
    "AAPL", "MSFT", "NVDA", "AMZN", "GOOG", "META", "TSLA", "BRK", "JPM", "V", "MA", "UNH", "XOM",
    "LLY", "AVGO", "HD", "PG", "COST", "MRK", "ABBV", "WMT", "NFLX", "AMD", "CRM",
];

/// The companies the names cycle through.
const COMPANIES: [&str; 8] = [
    "Apple",
    "Microsoft",
    "Nvidia",
    "Amazon",
    "Alphabet",
    "Meta",
    "Tesla",
    "Berkshire",
];

/// One quote, a row of the table's data.
#[derive(Debug, Clone, PartialEq)]
pub struct Quote {
    pub symbol: String,
    pub name: String,
    pub price: f64,
    pub change: f64,
    pub open: bool,
}

/// `count` deterministic quotes: no randomness, so tests know the data,
/// and the first row sits mid-ranked by both numbers, so a sort moves it.
pub fn quote_rows(count: usize) -> Vec<Quote> {
    (0..count)
        .map(|id| Quote {
            price: ((id * 97 + 500) % 997) as f64 / 10.0 + 10.0,
            change: ((id * 53 + 133) % 200) as f64 / 10.0 - 10.0,
            symbol: if id < TICKERS.len() {
                TICKERS[id].to_owned()
            } else {
                format!("{}-{}", TICKERS[id % TICKERS.len()], id)
            },
            name: format!("{} {}", COMPANIES[id % COMPANIES.len()], 100 + id % 900),
            open: id % 3 != 0,
        })
        .collect()
}

/// Sorts `rows` for `sort`, stably, ties broken by symbol so the order is
/// total and repeatable.
pub fn sort_quotes(rows: &mut [Quote], sort: &DataTableSort) {
    let dir = |o: Ordering| {
        if sort.direction == DataTableSortDirection::Descending {
            o.reverse()
        } else {
            o
        }
    };
    let tie = |a: &Quote, b: &Quote| a.symbol.cmp(&b.symbol);
    match sort.key.as_ref() {
        "name" => rows.sort_by(|a, b| dir(a.name.cmp(&b.name)).then(tie(a, b))),
        "price" => rows.sort_by(|a, b| {
            dir(a.price.partial_cmp(&b.price).unwrap_or(Ordering::Equal)).then(tie(a, b))
        }),
        "change" => rows.sort_by(|a, b| {
            dir(a.change.partial_cmp(&b.change).unwrap_or(Ordering::Equal)).then(tie(a, b))
        }),
        _ => {}
    }
}

/// The rows laid out column by column, in `sort`'s order.
fn table_rows(sort: Option<&DataTableSort>) -> DataTableRows {
    let mut rows = quote_rows(ROWS);
    if let Some(sort) = sort {
        sort_quotes(&mut rows, sort);
    }
    DataTableRows::new(rows.len())
        .texts("symbol", rows.iter().map(|q| q.symbol.as_str()))
        .texts("name", rows.iter().map(|q| q.name.as_str()))
        .numbers("price", rows.iter().map(|q| q.price))
        .numbers("change", rows.iter().map(|q| q.change))
        .bools("open", rows.iter().map(|q| q.open))
}

/// What the note under the heading says about the sort.
fn sort_note(sort: Option<&DataTableSort>) -> String {
    match sort {
        None => format!("{ROWS} rows, in table order. Titles sort."),
        Some(sort) => {
            let by = match sort.key.as_ref() {
                "name" => "name",
                "price" => "price",
                "change" => "change",
                _ => "column",
            };
            let direction = match sort.direction {
                DataTableSortDirection::Ascending => "ascending",
                DataTableSortDirection::Descending => "descending",
            };
            format!("{ROWS} rows, sorted by {by} {direction}.")
        }
    }
}

/// A column by `key`, on the ui table's column with an alignment, a
/// width and a fixed flag the shared helper does not take yet.
fn column(
    key: &'static str,
    title: &'static str,
    width: f32,
    align: DataTableAlign,
    sortable: bool,
) -> DataTableColumn {
    let mut column = ui_column(key, title, width);
    column.align = align;
    column.sortable = sortable;
    column.fixed = (key == "symbol").into();
    column
}

/// The Quotes demo: three thousand rows that sort and scroll both ways.
#[component]
pub fn QuotesDemo() -> impl IntoElement {
    // The base data never changes; `rows` holds it in the sort's order.
    // Every write notifies, so the table refills even when the new order
    // equals the old one as far as PartialEq can tell.
    let sort = signal::<Option<DataTableSort>>(None);
    let rows = signal_with(table_rows(None), SignalEquals::Never);
    let selection = signal(None::<DataTableSelection>);
    let on_sort = move |next: &Option<DataTableSort>| {
        sort.set(next.clone());
        rows.set(table_rows(next.as_ref()));
    };
    let initial = table_rows(None);
    view()
        .flex_1()
        .gap_2()
        .pb_4()
        .child(heading("Quotes").child(muted(move || -> SharedString {
            sort_note(sort.get().as_ref()).into()
        })))
        .child(
            data_table_of(
                vec![
                    column("symbol", "Symbol", 100.0, DataTableAlign::Start, false),
                    column("name", "Name", 200.0, DataTableAlign::Start, true),
                    column("price", "Price", 100.0, DataTableAlign::End, true),
                    column("change", "Change", 100.0, DataTableAlign::End, true),
                    column("open", "Open", 80.0, DataTableAlign::Center, false),
                ],
                initial,
            )
            .data(move || rows.get())
            .accessibility_label("Quotes")
            .sort(move || sort.get())
            .selection(move || selection.get())
            .on_sort_change(on_sort)
            .on_selection_change(move |picked: &Option<DataTableSelection>| selection.set(*picked))
            .flex_1(),
        )
}
