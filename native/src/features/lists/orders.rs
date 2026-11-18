//! The order book: a static `Table` of the app's own rows drawn with the
//! table's semantics parts, and a sectioned list whose letter headers
//! stick while the page scrolls.

use std::rc::Rc;

use nsh_base::{
    TableHeaderElement, TableRowElement, table, table_body, table_caption, table_cell,
    table_footer, table_head, table_header, table_row, theme,
};
use nsh_core::prelude::*;

use super::{heading, note};

/// A line of the order book.
#[derive(Clone, PartialEq)]
struct Order {
    id: u32,
    item: &'static str,
    status: Status,
    total: f64,
}

/// Where an order stands; the pill's look follows it.
#[derive(Clone, Copy, PartialEq)]
enum Status {
    Pending,
    Shipped,
    Cancelled,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Pending => "Pending",
            Status::Shipped => "Shipped",
            Status::Cancelled => "Cancelled",
        }
    }
}

/// Eight orders the table shows.
const ORDERS: [Order; 8] = [
    Order {
        id: 1042,
        item: "Desk lamp",
        status: Status::Shipped,
        total: 89.0,
    },
    Order {
        id: 1041,
        item: "Standing desk",
        status: Status::Pending,
        total: 549.0,
    },
    Order {
        id: 1040,
        item: "Monitor arm",
        status: Status::Shipped,
        total: 129.5,
    },
    Order {
        id: 1039,
        item: "Cable set",
        status: Status::Cancelled,
        total: 24.0,
    },
    Order {
        id: 1038,
        item: "Herman chair",
        status: Status::Shipped,
        total: 312.0,
    },
    Order {
        id: 1037,
        item: "Footrest",
        status: Status::Pending,
        total: 45.0,
    },
    Order {
        id: 1036,
        item: "Desk mat",
        status: Status::Shipped,
        total: 39.0,
    },
    Order {
        id: 1035,
        item: "Herman desk shelf",
        status: Status::Shipped,
        total: 78.0,
    },
];

/// The orders' total, the footer row's.
fn total() -> f64 {
    ORDERS.iter().map(|o| o.total).sum()
}

/// A status pill: muted for settled states, accent while it waits,
/// destructive when it died.
fn status_pill(status: Status) -> impl IntoElement {
    view()
        .self_start()
        .px_2()
        .py(px(2.0))
        .rounded_full()
        .bg(move || match status {
            Status::Pending => theme().accent,
            _ => theme().muted,
        })
        .child(
            text(status.label())
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || match status {
                    Status::Pending => theme().accent_foreground,
                    Status::Cancelled => theme().destructive,
                    Status::Shipped => theme().muted_foreground,
                }),
        )
}

/// The header row of column titles.
fn head_row(label: &'static str, amounts: &'static str) -> TableHeaderElement {
    table_header().child(
        table_row()
            .border_b_1()
            .border_color(move || theme().border)
            .child(
                table_head().flex_1().px_3().py_2().child(
                    text(label)
                        .text_xs()
                        .uppercase()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().muted_foreground),
                ),
            )
            .child(
                table_head().w(px(110.0)).px_3().py_2().child(
                    text("Status")
                        .text_xs()
                        .uppercase()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().muted_foreground),
                ),
            )
            .child(
                table_head().w(px(90.0)).px_3().py_2().child(
                    text(amounts)
                        .text_xs()
                        .uppercase()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().muted_foreground),
                ),
            ),
    )
}

/// One order's row.
fn order_row(order: &Order) -> TableRowElement {
    let (id, item, status, total) = (order.id, order.item, order.status, order.total);
    table_row()
        .border_b_1()
        .border_color(move || theme().border)
        .hover(|s| s.bg(move || theme().accent.opacity(0.4)))
        .child(
            table_cell().flex_1().px_3().py_2().child(
                view()
                    .gap(px(2.0))
                    .child(
                        text(item)
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(move || theme().foreground),
                    )
                    .child(
                        text(format!("Order {id}"))
                            .text_xs()
                            .tabular_nums()
                            .text_color(move || theme().muted_foreground),
                    ),
            ),
        )
        .child(
            table_cell()
                .w(px(110.0))
                .px_3()
                .py_2()
                .child(status_pill(status)),
        )
        .child(
            table_cell().w(px(90.0)).px_3().py_2().child(
                text(format!("${total:.2}"))
                    .text_sm()
                    .tabular_nums()
                    .text_right()
                    .text_color(move || theme().foreground),
            ),
        )
}

/// The order table: caption, header, body, footer, each a part with its
/// own role for screen readers.
fn orders_table() -> impl IntoElement {
    let rows: Vec<TableRowElement> = ORDERS.iter().map(order_row).collect();
    table()
        .accessibility_label("Recent orders")
        .rounded_lg()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .overflow_hidden()
        .child(
            table_caption()
                .px_3()
                .py_3()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    text("Recent orders")
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().card_foreground),
                ),
        )
        .child(head_row("Item", "Total"))
        .child(table_body().children(rows))
        .child(
            table_footer().bg(move || theme().muted.opacity(0.5)).child(
                table_row()
                    .child(
                        table_cell().flex_1().px_3().py_2().child(
                            text("8 orders")
                                .text_xs()
                                .text_color(move || theme().muted_foreground),
                        ),
                    )
                    .child(table_cell().w(px(110.0)))
                    .child(
                        table_cell().w(px(90.0)).px_3().py_2().child(
                            text(format!("${:.2}", total()))
                                .text_sm()
                                .tabular_nums()
                                .text_right()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(move || theme().foreground),
                        ),
                    ),
            ),
        )
}

/// A person of the directory, under a letter.
#[derive(Clone, Copy, PartialEq)]
struct Person {
    name: &'static str,
    team: &'static str,
}

/// The directory's rows: each letter's header, then its people.
#[derive(Clone, Copy, PartialEq)]
enum Row {
    Header(char),
    Person(Person),
}

impl Row {
    fn person(self) -> Option<Person> {
        match self {
            Row::Person(person) => Some(person),
            Row::Header(_) => None,
        }
    }
}

const PEOPLE: [(&str, &str, char); 12] = [
    ("Ada Lovelace", "Compilers", 'A'),
    ("Alan Kay", "Dynabook", 'A'),
    ("Barbara Liskov", "Abstractions", 'B'),
    ("Brian Kernighan", "Tooling", 'B'),
    ("Grace Hopper", "Compilers", 'G'),
    ("Linus Torvalds", "Kernels", 'L'),
    ("Margaret Hamilton", "Flight", 'M'),
    ("Niklaus Wirth", "Languages", 'N'),
    ("Radia Perlman", "Networking", 'R'),
    ("Tim Berners-Lee", "The web", 'T'),
    ("Whitfield Diffie", "Crypto", 'W'),
    ("Yukihiro Matsumoto", "Languages", 'Y'),
];

/// The directory, one row per person, a header row per letter.
fn directory_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    for letter in ['A', 'B', 'G', 'L', 'M', 'N', 'R', 'T', 'W', 'Y'] {
        let group: Vec<_> = PEOPLE.iter().filter(|(.., l)| *l == letter).collect();
        if group.is_empty() {
            continue;
        }
        rows.push(Row::Header(letter));
        rows.extend(
            group
                .into_iter()
                .map(|&(name, team, _)| Row::Person(Person { name, team })),
        );
    }
    rows
}

/// A letter's header row, of the cell's current row.
fn directory_header(row: Memo<Row>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .h(px(32.0))
        .px_2()
        // Opaque: rows scroll under it while it sticks.
        .bg(move || theme().background)
        .child(
            text(move || match row.get() {
                Row::Header(letter) => letter.to_string(),
                Row::Person(_) => String::new(),
            })
            .text_xs()
            .uppercase()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || theme().muted_foreground),
        )
}

/// A person's row, of the cell's current row.
fn directory_person(row: Memo<Row>) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .h(px(52.0))
        .px_2()
        .border_b_1()
        .border_color(move || theme().border.opacity(0.6))
        .child(
            view()
                .size(px(34.0))
                .rounded_full()
                .items_center()
                .justify_center()
                .bg(move || theme().secondary)
                .child(
                    text(move || {
                        row.get().person().map_or(String::new(), |p| {
                            p.name.chars().next().map(String::from).unwrap_or_default()
                        })
                    })
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(move || theme().secondary_foreground),
                ),
        )
        .child(
            view().flex_1().child(
                text(move || {
                    row.get()
                        .person()
                        .map_or(String::new(), |p| p.name.to_owned())
                })
                .text_sm()
                .text_color(move || theme().foreground),
            ),
        )
        .child(
            text(move || {
                row.get()
                    .person()
                    .map_or(String::new(), |p| p.team.to_owned())
            })
            .text_xs()
            .text_color(move || theme().muted_foreground),
        )
}

/// The sectioned directory: its letter headers stick while the page
/// scrolls, and the rows recycle in the page's own scroll view.
fn directory() -> impl IntoElement {
    // Shared by the row cells and the keys: the data never changes.
    let rows = Rc::new(directory_rows());
    let headers: Vec<usize> = (0..rows.len())
        .filter(|&i| matches!(rows[i], Row::Header(..)))
        .collect();
    let count = rows.len();
    let keys = rows.clone();
    virtual_list(count, move |at| {
        let rows = rows.clone();
        let row = computed(move || rows.get(at.index()).copied().unwrap_or(Row::Header(' ')));
        let is_header = move || matches!(row.get(), Row::Header(..));
        when(is_header, move || directory_header(row)).otherwise(move || directory_person(row))
    })
    .keys(move || {
        keys.iter()
            .map(|row| match *row {
                Row::Header(letter) => format!("header-{letter}"),
                Row::Person(person) => format!("person-{}", person.name),
            })
            .collect::<Vec<_>>()
    })
    .sticky_header_indices(headers)
    .viewport(ListViewport::Parent)
    .estimated_item_size(px(52.0))
}

/// The Orders demo: the parts table and the sectioned directory, one
/// page of scrolling.
#[component]
pub fn OrdersDemo() -> impl IntoElement {
    view().flex_1().child(
        scroll().flex_1().child(
            view()
                .gap_6()
                .pb_4()
                .child(heading("Orders").child(note("Table parts with a table's semantics")))
                .child(orders_table())
                .child(
                    heading("Directory").child(note("Sticky section headers over a recycled list")),
                )
                .child(directory()),
        ),
    )
}
