//! The drag-and-drop demo: rows whose vertical pan is bound natively; on
//! release the drop is applied to the data and the handle springs home,
//! while the reorder animates as a layout change.

use nsh_base::theme;
use nsh_core::{element::LayoutAnimation, prelude::*};

use super::{api_line, section, status_line};

/// A row's full height, gaps included: drops are counted in these.
const ROW: f32 = 64.0;

/// The items, in their data order.
const ITEMS: [&str; 4] = ["One", "Two", "Three", "Four"];

/// The order as the data holds it.
fn order_line(items: &[usize]) -> String {
    items
        .iter()
        .map(|i| ITEMS[*i])
        .collect::<Vec<_>>()
        .join(" · ")
}

/// One draggable row. The pan writes `y` on the UI thread; the release
/// moves the item in the data and springs the handle back to its row.
fn handle(
    slot: usize,
    label: &'static str,
    order: Signal<Vec<usize>>,
    dragging: Signal<Option<usize>>,
) -> impl IntoElement {
    let y = animated_value(0.0);
    let is_dragging = move || dragging.get() == Some(slot);
    view().h(px(ROW)).justify_center().child(
        view()
            .test_id(label)
            .flex_row()
            .items_center()
            .gap(px(10.0))
            .h(px(56.0))
            .px(px(14.0))
            .rounded(move || theme().radius)
            .bg(move || theme().secondary)
            .border_1()
            .style_when(is_dragging, |s| s.border_color(move || theme().primary))
            .style_when(
                move || !is_dragging(),
                |s| s.border_color(move || theme().border),
            )
            .pan_values(None, Some(&y))
            .on_pan_start(move |_: &PanEvent| dragging.set(Some(slot)))
            .on_pan_end(move |e: &PanEvent| {
                dragging.set(None);
                let steps = (e.translation.y.0 / ROW).round();
                if steps != 0.0
                    && let Some(to) = move_within(order.get_untracked(), slot, steps as i32)
                {
                    // The reorder animates with the commit that moves
                    // the rows; the handle's home is a spring.
                    animate_layout(LayoutAnimation::SPRING);
                    order.update(move |order| {
                        let from = order.iter().position(|&s| s == slot).expect("dragged row");
                        order.remove(from);
                        order.insert(to, slot);
                    });
                }
                y.spring(0.0, Spring::default());
            })
            .bind_translate_y(&y)
            .child(view().gap(px(3.0)).children((0..3).map(|_| {
                view()
                    .w(px(16.0))
                    .h(px(2.0))
                    .rounded(px(1.0))
                    .bg(move || theme().muted_foreground)
            })))
            .child(
                text(label)
                    .text_size(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || theme().secondary_foreground),
            ),
    )
}

/// Where `slot` lands after `steps` rows: an index of the order without it.
fn move_within(order: Vec<usize>, slot: usize, steps: i32) -> Option<usize> {
    let from = order.iter().position(|&s| s == slot)?;
    let to = (from as i32 + steps).clamp(0, order.len() as i32 - 1) as usize;
    (to != from).then_some(to)
}

pub(crate) fn drop_reorder() -> impl IntoElement {
    let order = signal((0..ITEMS.len()).collect::<Vec<usize>>());
    let dragging = signal(None::<usize>);
    section(
        "Drag and drop",
        "The pan is native; the drop lands in the data and springs home",
        view()
            .flex_col()
            .gap(px(8.0))
            // The rows render in the data's order, keyed by slot: a drop
            // moves the rows, and the layout animation animates the move.
            .child(each(
                order,
                |slot| *slot,
                move |slot: ReadSignal<usize>, _: ReadSignal<usize>| {
                    handle(slot.get(), ITEMS[slot.get()], order, dragging)
                },
            ))
            .child(status_line("drop-order", move || {
                format!("Order: {}", order_line(&order.get()))
            }))
            .child(api_line(
                "The drop settles on a spring",
                "pan_values(None, Some(&y)) + on_pan_end",
            )),
    )
}
