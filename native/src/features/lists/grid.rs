//! The gallery grid: two thousand cards in three columns that recycle, a
//! header of the viewable rows, jumps to far rows, and stars that stay
//! with their card because the card is where the state lives.

use std::collections::HashSet;

use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use nsh_base::{button, theme};
use nsh_core::prelude::*;

use super::{heading, note};

/// How many cards the grid holds.
const CARDS: usize = 2000;

/// The far row the jump button goes to.
const FAR: u32 = 1500;

/// A card's height in points; four heights so the rows differ.
fn card_height(id: usize) -> f32 {
    84.0 + (id % 4) as f32 * 22.0
}

/// A card: its number, a meter of it through the set, and its star. Every
/// value is a binding, so the cell follows the card it is handed.
fn card(at: Position, starred: Signal<HashSet<usize>>) -> impl IntoElement {
    let id = move || at.index();
    let n = move || at.index() + 1;
    let is_starred = move || starred.get().contains(&id());
    view()
        .h(move || px(card_height(at.index())))
        .rounded_md()
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().card)
        .p_3()
        .justify_between()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    text(move || format!("Card {}", n()))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .tabular_nums()
                        .text_color(move || theme().card_foreground),
                )
                .child(
                    button()
                        .accessibility_label(move || Some(format!("Star {}", n()).into()))
                        .hit_slop(8.0)
                        .on_press(move |_: &()| {
                            starred.update(|set| {
                                if !set.remove(&id()) {
                                    set.insert(id());
                                }
                            })
                        })
                        .child(
                            text(move || if is_starred() { "★" } else { "☆" })
                                .text_base()
                                .text_color(move || {
                                    if is_starred() {
                                        theme().primary
                                    } else {
                                        theme().muted_foreground
                                    }
                                }),
                        ),
                ),
        )
        .child(
            view()
                .h(px(4.0))
                .rounded_full()
                .bg(move || theme().muted)
                .child(
                    view()
                        .h_full()
                        .rounded_full()
                        .bg(move || theme().chart_2)
                        .w(move || relative(((id() * 37 % 100) as f32 + 1.0) / 101.0)),
                ),
        )
}

/// The grid's header: the viewable range and the jumps, outside the
/// recycling.
fn grid_header(
    visible: Signal<Option<(usize, usize)>>,
    starred: Signal<HashSet<usize>>,
    list: ElementRef,
) -> impl IntoElement {
    view()
        .gap_2()
        .pb_2()
        .child(
            view()
                .flex_row()
                .items_end()
                .justify_between()
                .child(heading("Grid").child(note("Recycled cells; stars live in the data")))
                .child(
                    text(move || match visible.get() {
                        Some((first, last)) => format!("Viewable {first}–{last}"),
                        None => "Viewable —".to_owned(),
                    })
                    .test_id("grid-visible")
                    .text_sm()
                    .tabular_nums()
                    .text_color(move || theme().muted_foreground),
                ),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(move || format!("{} starred", starred.get().len()))
                        .test_id("grid-starred")
                        .text_sm()
                        .tabular_nums()
                        .text_color(move || theme().muted_foreground),
                )
                .child(button_with(
                    "Jump to card 1500",
                    ButtonVariant::Secondary,
                    ButtonSize::Small,
                    false,
                    None,
                    move || list.scroll_to_index(FAR, percentage(0.5), px(0.0), true),
                ))
                .child(button_with(
                    "Back to the top",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    None,
                    move || list.scroll_to_index(0_u32, percentage(0.0), px(0.0), true),
                )),
        )
}

/// The Grid demo: columns, viewability, jumps, and per-card state.
#[component]
pub fn GridDemo() -> impl IntoElement {
    // Per-card state, keyed by id: a signal made in a row would travel
    // with the cell to the next card.
    let starred = signal(HashSet::<usize>::new());
    // The first and last viewable rows, of the last report.
    let visible = signal(None::<(usize, usize)>);
    let list = element_ref();
    let header = grid_header(visible, starred, list);
    virtual_list(CARDS, move |at| card(at, starred))
        .test_id("grid")
        .columns(3)
        .estimated_item_size(px(110.0))
        .row_gap(px(8.0))
        .column_gap(px(8.0))
        .content_padding(px(2.0))
        .viewability(Viewability {
            item_visible_threshold: Some(Percentage(0.5)),
            ..Viewability::default()
        })
        .on_viewable_items_change(move |rows: &ViewableItems| {
            if let Some((first, last)) = rows.viewable {
                log::info!("grid: viewable rows {first}..={last}");
            }
            visible.set(rows.viewable);
        })
        .header(header)
        .flex_1()
        .element_ref(&list)
}
