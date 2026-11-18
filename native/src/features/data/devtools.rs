//! The query devtools: the keys this feature loaded with their status,
//! freshness and cached data, and the fake API's switches at run time.

use nsh_base::{switch, theme};
use nsh_core::prelude::*;
use nsh_query::{QueryKey, query_client};

use super::{DETAIL_TTL_MS, QueryInfo};
use crate::services::fake_api::{self, ApiError, Comment, Post};

/// One choice of a three-way switch row; `kind` names the switch it acts on.
fn choice(
    kind: &'static str,
    label: &'static str,
    value: u64,
    current: impl Fn() -> u64 + 'static,
) -> impl IntoElement {
    let selected = move || current() == value;
    crate::ui::button::choice_button(label, label, selected, false, move || {
        name_press(kind, value)
    })
}

/// Applies the choice's value to the switch the name selects.
fn name_press(name: &'static str, value: u64) {
    match name {
        "latency" => fake_api::set_latency(value),
        "failures" => fake_api::set_fail_every(value as u32),
        _ => {}
    }
}

/// The offline switch: a styled `nsh/base` switch, its parts the app's.
fn offline_switch() -> impl IntoElement {
    let api = fake_api::fake_api();
    switch()
        .accessibility_label("Offline")
        .checked(move || api.with(|api| api.offline))
        .on_change(move |next: &bool| fake_api::set_offline(*next))
        .track_style(|s| {
            s.w(px(44.0))
                .p(px(2.0))
                .rounded_full()
                .bg(move || theme().muted_foreground.opacity(0.3))
                .transition_colors()
                .checked(|s| s.bg(move || theme().destructive))
        })
        .thumb_style(|s| {
            s.size(px(20.0))
                .rounded_full()
                .bg(move || theme().background)
                .shadow_sm()
                .transition_transform()
                .checked(|s| s.translate_x(px(20.0)))
        })
}

/// One switch row: its label, why it is here, and the control.
fn switch_row(
    label: &'static str,
    note: &'static str,
    control: impl IntoElement,
) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            view()
                .flex_1()
                .gap_1()
                .child(
                    text(label)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().foreground),
                )
                .child(
                    text(note)
                        .text_xs()
                        .text_color(move || theme().muted_foreground),
                ),
        )
        .child(control)
}

/// The fake API's switches: latency, failures, offline, and the counters.
fn api_section() -> impl IntoElement {
    let api = fake_api::fake_api();
    view()
        .gap_3()
        .child(
            text("Fake API")
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(switch_row(
            "Latency",
            "How long every response waits",
            view().flex_row().gap_2().children(
                [
                    ("latency", "0 ms", 0_u64),
                    ("latency", "100 ms", 100),
                    ("latency", "400 ms", 400),
                ]
                .map(|(kind, label, value)| {
                    choice(kind, label, value, move || api.with(|api| api.latency_ms))
                }),
            ),
        ))
        .child(switch_row(
            "Failures",
            "Which requests the fake API rejects",
            view().flex_row().gap_2().children(
                [
                    ("failures", "Never", 0_u64),
                    ("failures", "Every 2nd", 2),
                    ("failures", "All", 1),
                ]
                .map(|(kind, label, value)| {
                    choice(kind, label, value, move || {
                        api.with(|api| api.fail_every as u64)
                    })
                }),
            ),
        ))
        .child(switch_row(
            "Offline",
            "Requests fail at once while it is on",
            offline_switch(),
        ))
        .child(
            text(move || {
                api.with(|api| {
                    format!(
                        "{} requests, {} failed, {} in flight",
                        api.requests, api.failures, api.in_flight
                    )
                })
            })
            .text_xs()
            .tabular_nums()
            .text_color(move || theme().muted_foreground),
        )
}

/// What the cache holds for `key`, when this devtools page can read it.
fn cached_note(key: &QueryKey) -> Option<String> {
    let client = query_client();
    match key.first_segment() {
        "post" => client
            .data::<Post, ApiError>(key.clone())
            .map(|post| format!("{} by {}, rev {}", post.title, post.author, post.revision)),
        "comments" => client
            .data::<Vec<Comment>, ApiError>(key.clone())
            .map(|comments| format!("{} comments", comments.len())),
        _ => None,
    }
}

/// One cache entry: the key, its last status, its freshness and its data.
fn entry_row(info: ReadSignal<QueryInfo>) -> impl IntoElement {
    // Both ends read the engine's clock (`nsh_core::time::unix_ms`), the
    // same source the queries' own timestamps and the manual test clock
    // use, so the age and the TTL verdict agree with the queries.
    let age_s = move || {
        let updated = info.with(|info| info.updated_ms);
        (nsh_core::time::unix_ms() as u64).saturating_sub(updated) / 1000
    };
    view()
        .gap_1()
        .p_3()
        .rounded(move || theme().radius)
        .bg(move || theme().card)
        .border_1()
        .border_color(move || theme().border)
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text(move || {
                        let key = info.with(|info| info.key.clone());
                        key.parts()
                            .iter()
                            .map(|part| part.as_ref())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(move || theme().card_foreground),
                )
                .child(
                    text(move || info.with(|info| info.status).to_owned())
                        .text_xs()
                        .text_color(move || match info.with_untracked(|info| info.status) {
                            "loaded" => theme().foreground,
                            "loading" => theme().muted_foreground,
                            _ => theme().destructive,
                        }),
                ),
        )
        .child(
            text(move || {
                let note = info.with(|info| info.note.clone());
                cached_note(&info.with_untracked(|info| info.key.clone()))
                    .map(|cached| format!("{note} — {cached}"))
                    .unwrap_or(note)
            })
            .text_xs()
            .text_color(move || theme().muted_foreground),
        )
        .child(
            text(move || {
                // Only the detail queries carry the TTL; the feed is not
                // cached, so it gets an age but no freshness verdict.
                let judged = info.with(|info| info.key.first_segment() == "post");
                match judged {
                    true => format!(
                        "{}s ago, {}",
                        age_s(),
                        if age_s() < DETAIL_TTL_MS / 1000 {
                            "fresh"
                        } else {
                            "stale"
                        }
                    ),
                    false => format!("{}s ago", age_s()),
                }
            })
            .text_xs()
            .tabular_nums()
            .text_color(move || theme().muted_foreground),
        )
}

/// The cache entries this feature loaded, from the query log.
fn cache_section() -> impl IntoElement {
    let log = super::query_log();
    view()
        .gap_3()
        .child(
            text("Query cache")
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(each(
            log,
            |info: &QueryInfo| {
                info.key
                    .parts()
                    .iter()
                    .map(|part| part.as_ref())
                    .collect::<Vec<_>>()
                    .join(",")
            },
            |info: ReadSignal<QueryInfo>, _index: ReadSignal<usize>| entry_row(info),
        ))
        .child(when(
            move || log.with(Vec::is_empty),
            || {
                text("Open the feed or a post, and its keys show up here")
                    .text_xs()
                    .text_color(move || theme().muted_foreground)
            },
        ))
}

/// The query devtools page: one scroll as the page's root, so the iOS
/// header's large title and soft edge follow it as on every page.
pub fn devtools_page() -> impl IntoElement {
    scroll()
        .flex_1()
        .content_inset(crate::ui::layout::page_insets(0.0))
        .child(
            crate::ui::layout::page_body()
                .child(crate::ui::text::page_intro(
                    "Query devtools",
                    "The fake API's knobs and the query cache: keys, states and timings.",
                ))
                .child(api_section())
                .child(cache_section()),
        )
}
