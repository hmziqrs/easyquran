//! The release room: an inverted chat that opens mid-history. Scrolling
//! up loads older pages at the visual top; scrolling to the bottom loads
//! newer messages at the start, and the keys keep the visible messages in
//! place while they arrive.
//!
//! The window the chat opens with is ids `WINDOW_BOTTOM..=WINDOW_TOP`
//! (240..=280) of a room that runs `OLDEST..=ROOM_END` (1..=300): older
//! pages 239 and below and newer pages 281 and above both exist to load,
//! so neither loader starts exhausted. `older_page` and `newer_page` are
//! the only place the page math lives, so the loads stay contiguous with
//! the window (no gap, no duplicate) by construction.

use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use crate::ui::field::text_field;
use nsh_base::theme;
use nsh_core::{element::SubmitBehavior, prelude::*};

use super::{heading, note};

/// What the messages say, in turn.
const LINES: [&str; 6] = [
    "Did the build go out?",
    "Yes, 2.4 is live on both stores",
    "Crash rate?",
    "Flat at 0.02 %",
    "The list scrolls so much smoother now",
    "Recycling plus native layout, no JS per frame",
];

/// How many messages a load brings, at either end.
pub const PAGE: u32 = 15;

/// The oldest id the room's history reaches.
pub const OLDEST: u32 = 1;

/// The newest id the room's history reaches at first.
pub const ROOM_END: u32 = 300;

/// The newest id the chat opens with; 281..=ROOM_END load at the start.
pub const WINDOW_TOP: u32 = 280;

/// The oldest id the chat opens with; OLDEST..=WINDOW_BOTTOM-1 load at
/// the top.
pub const WINDOW_BOTTOM: u32 = 240;

/// One message. `id` orders the room: higher is newer.
#[derive(Clone, PartialEq)]
struct Message {
    id: u32,
    body: String,
    mine: bool,
}

/// A message of the room's history: its body from the script.
fn history(id: u32) -> Message {
    Message {
        id,
        body: LINES[(id as usize) % LINES.len()].to_owned(),
        mine: id.is_multiple_of(3),
    }
}

/// The ids one older load appends: `PAGE` ids ending just below `held`
/// (the oldest id the window holds, already in it), never past
/// [`OLDEST`](OLDEST). Newest-first, the order an append needs.
pub fn older_page(held: u32) -> Vec<u32> {
    let first = held.saturating_sub(PAGE).max(OLDEST);
    (first..held).rev().collect()
}

/// The ids one newer load prepends: `PAGE` ids starting just above
/// `held` (the newest id the window holds, already in it), never past
/// `ceiling` (the room's newest). Newest-first, the order a prepend
/// needs.
pub fn newer_page(held: u32, ceiling: u32) -> Vec<u32> {
    let last = held.saturating_add(PAGE).min(ceiling);
    ((held + 1)..=last).rev().collect()
}

/// A bubble: mine at the end in the accent, theirs at the start in a card,
/// the script line with the room's id under it.
fn bubble(message: Memo<Message>) -> impl IntoElement {
    let mine = move || message.get().mine;
    let theirs = move || !mine();
    view()
        .style_when(mine, |v| v.self_end().items_end())
        .style_when(theirs, |v| v.self_start().items_start())
        .max_w(relative(0.78))
        .child(
            view()
                .px(px(14.0))
                .py(px(8.0))
                .rounded_lg()
                .style_when(mine, |v| v.bg(move || theme().primary).rounded_br(px(4.0)))
                .style_when(theirs, |v| {
                    v.bg(move || theme().card)
                        .border_1()
                        .border_color(move || theme().border)
                        .rounded_bl(px(4.0))
                })
                .child(
                    text(move || message.get().body.clone())
                        .text_sm()
                        .text_color(move || {
                            if mine() {
                                theme().primary_foreground
                            } else {
                                theme().card_foreground
                            }
                        }),
                ),
        )
        .child(
            text(move || format!("msg {}", message.get().id))
                .px_1()
                .text_xs()
                .tabular_nums()
                .text_color(move || theme().muted_foreground),
        )
}

/// The Chat demo: inverted, opened on ids 240..=280 of 1..=300.
#[component]
pub fn ChatDemo() -> impl IntoElement {
    // The room's ids the window reaches at either end; the loaders stop at
    // the room's ends. These drive the loaders, nothing displays them.
    let older_until = stored(WINDOW_BOTTOM);
    let newer_until = stored(WINDOW_TOP);
    // The room's newest id, grown by Receive; the start loader brings
    // everything above `newer_until` up to it.
    let received = stored(ROOM_END);
    let messages = signal(
        (WINDOW_BOTTOM..=WINDOW_TOP)
            .rev()
            .map(history)
            .collect::<Vec<_>>(),
    );
    let draft = signal(String::new());
    let send = move || {
        let body = draft.get_untracked().trim().to_owned();
        if body.is_empty() {
            return;
        }
        draft.set(String::new());
        // A sent message is in the window at once: above whatever leads,
        // and the start loader's ceiling moves with it.
        let id = received.with(|last| {
            *last += 1;
            *last
        });
        newer_until.with(|until| *until = id);
        messages.update(|m| {
            m.insert(
                0,
                Message {
                    id,
                    body,
                    mine: true,
                },
            )
        });
    };
    // Receive grows the room only: the message arrives at the start when
    // the user next approaches it, through the loader, like any chat.
    let receive = move |_: &()| {
        received.with(|last| {
            *last += 1;
        });
    };
    // Scrolled to the visual top: older pages join at the end.
    let load_older = move |_: &()| {
        let held = older_until.with(|until| *until);
        let page = older_page(held);
        if page.is_empty() {
            return;
        }
        older_until.with(|until| *until = page[page.len() - 1]);
        messages.update(|m| m.extend(page.into_iter().map(history)));
    };
    // Scrolled to the visual bottom: newer messages join at the start, and
    // the keys keep what is on screen where it is.
    let load_newer = move |_: &()| {
        let ceiling = received.with(|last| *last);
        let held = newer_until.with(|until| *until);
        let page = newer_page(held, ceiling);
        if page.is_empty() {
            return;
        }
        newer_until.with(|until| *until = page[0]);
        messages.update(|m| {
            m.splice(0..0, page.into_iter().map(history));
        });
    };
    let list = element_ref();
    let room = virtual_list(
        move || messages.with(Vec::len),
        move |at| {
            // The cell's message, read in a binding so the cell follows it.
            let message = computed(move || {
                messages.with(|m| {
                    m.get(at.index()).cloned().unwrap_or(Message {
                        id: 0,
                        body: String::new(),
                        mine: false,
                    })
                })
            });
            bubble(message)
        },
    )
    .test_id("chat-list")
    .keys(move || messages.with(|m| m.iter().map(|m| m.id).collect::<Vec<_>>()))
    .inverted()
    .estimated_item_size(px(56.0))
    .row_gap(px(6.0))
    .content_padding_x(px(4.0))
    .on_load_more(load_older)
    .on_load_more_at_start(load_newer)
    .flex_1()
    .element_ref(&list);
    view()
        .flex_1()
        .gap_2()
        .child(heading("Chat").child(note(
            "Inverted, opened mid-history; the ends load and stay put",
        )))
        .child(room)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .pb_2()
                .child(button_with(
                    "Receive",
                    ButtonVariant::Secondary,
                    ButtonSize::Small,
                    false,
                    Some("Receive a message"),
                    move || receive(&()),
                ))
                .child(button_with(
                    "Latest",
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    false,
                    Some("Latest message"),
                    move || list.scroll_to_index(0, percentage(0.0), px(0.0), false),
                ))
                .child(
                    text_field(draft, "Message")
                        .test_id("chat-draft")
                        .on_submit(move |_: &str| send())
                        .submit_behavior(SubmitBehavior::Submit)
                        .flex_1()
                        .h(px(40.0))
                        .rounded_full(),
                )
                .child(button_with(
                    "↑",
                    ButtonVariant::Primary,
                    ButtonSize::Small,
                    false,
                    Some("Send"),
                    send,
                )),
        )
}
