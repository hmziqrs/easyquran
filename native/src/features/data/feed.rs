//! The feed: a virtualized infinite list of posts with pull to refresh,
//! optimistic likes, skeletons and an error state with a Retry button, on
//! the local fake API.

use nsh_base::{button, theme};
use nsh_core::prelude::*;
use nsh_query::{
    InfiniteQueryOptions, Mutation, MutationOptions, QueryStatus, RetryPolicy, use_infinite_query,
    use_mutation_with,
};

use super::{
    FeedEdits, LikeInput, apply_like, feed_edits, feed_key, open_post, record_query, revert_like,
};
use crate::services::fake_api::{self, ApiError, Post, PostPage};

/// The like write of the feed: optimistic through the feed edits, rolled
/// back if the fake API rejects it.
fn feed_like(rows: Memo<Vec<Post>>) -> Mutation<LikeInput, Post, ApiError> {
    use_mutation_with::<LikeInput, Post, ApiError, _, _>(
        MutationOptions::new()
            .on_mutate(move |input: &LikeInput| {
                // Read untracked: the like is computed from the row as shown.
                if let Some(post) = rows
                    .get_untracked()
                    .iter()
                    .find(|post| post.id == input.post_id)
                {
                    let (liked, likes) = if post.liked == input.to {
                        (post.liked, post.likes)
                    } else if input.to {
                        (true, post.likes + 1)
                    } else {
                        (false, post.likes.saturating_sub(1))
                    };
                    apply_like(post.id, liked, likes);
                }
            })
            .on_success(move |post: &Post, _input: &LikeInput| {
                apply_like(post.id, post.liked, post.likes)
            })
            .on_error(move |_error: &ApiError, input: &LikeInput| revert_like(input.post_id)),
        |input: LikeInput, _signal| async move { fake_api::like(input.post_id, input.to).await },
    )
}

/// One feed row: the post card with its like button. `post` is a memo, so a
/// recycled cell follows the row it now shows.
fn post_row(post: Memo<Post>, like: Mutation<LikeInput, Post, ApiError>) -> impl IntoElement {
    let heart = move || {
        let (liked, likes) = post.with(|post| (post.liked, post.likes));
        format!("{} {likes}", if liked { "♥" } else { "♡" })
    };
    view()
        .flex_row()
        .gap_3()
        .p_4()
        .label(move || {
            // Tracked: a recycled cell must re-read the row it now shows.
            SharedString::from(post.with(|post| format!("Post {}, {}", post.id, post.title)))
        })
        .rounded(move || theme().radius)
        .bg(move || theme().card)
        .border_1()
        .border_color(move || theme().border)
        .active(|s| s.bg(move || theme().accent))
        .on_press(move || open_post(post.with_untracked(|post| post.id)))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap_1()
                .child(
                    text(move || post.with(|post| post.title.clone()))
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(move || theme().card_foreground),
                )
                .child(
                    text(move || post.with(|post| post.body.clone()))
                        .text_xs()
                        .number_of_lines(2)
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    text(move || post.with(|post| post.author.clone()))
                        .text_xs()
                        .text_color(move || theme().muted_foreground),
                )
                .child(
                    text(move || {
                        let comments = post.with(|post| post.comments);
                        match comments {
                            0 => "No comments yet".to_owned(),
                            1 => "1 comment".to_owned(),
                            n => format!("{n} comments"),
                        }
                    })
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
                ),
        )
        .child(
            button()
                .accessibility_label(move || {
                    SharedString::from(post.with(|post| format!("Like post {}", post.id)))
                })
                .on_press(move || {
                    let (liked, id) = post.with_untracked(|post| (post.liked, post.id));
                    like.mutate(LikeInput {
                        post_id: id,
                        to: !liked,
                    });
                })
                .h(px(32.0))
                .px_3()
                .items_center()
                .justify_center()
                .rounded(move || theme().radius)
                .bg(move || theme().muted)
                .active(|s| s.opacity(0.7))
                .child(
                    text(heart)
                        .text_xs()
                        .text_color(move || theme().card_foreground),
                ),
        )
}

/// Placeholder rows shown while the first page loads.
fn skeletons() -> impl IntoElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .p_4()
        .gap_3()
        .children((0..4).map(|_| {
            view()
                .w_full()
                .h(px(88.0))
                .rounded(move || theme().radius)
                .bg(move || theme().muted)
        }))
}

/// The error state: what failed and a way back.
fn error_state(error: String, on_retry: impl Fn() + 'static) -> impl IntoElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_3()
        .p_4()
        .child(
            text("Couldn't load the feed")
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(
            text(error)
                .text_xs()
                .text_color(move || theme().muted_foreground),
        )
        .child(crate::ui::button::button_with(
            "Retry",
            crate::ui::button::ButtonVariant::Primary,
            crate::ui::button::ButtonSize::Small,
            false,
            None,
            on_retry,
        ))
}

/// The banner shown while the offline switch is on.
fn offline_banner() -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap_2()
        .px_4()
        .py_2()
        .bg(move || theme().muted)
        .child(
            text("Offline — requests fail until you turn the switch back")
                .text_xs()
                .text_color(move || theme().foreground),
        )
}

/// The feed page: the detail the feed presents over itself is `data_page`'s,
/// in `mod.rs`.
pub fn feed_page() -> impl IntoElement {
    let posts = use_infinite_query::<u64, PostPage, ApiError, _, _>(
        feed_key(),
        InfiniteQueryOptions::new(0_u64, |page: &PostPage| page.next_cursor)
            .retry(RetryPolicy::new(2).with_delay(200)),
        |cursor: u64, _context| {
            let key = feed_key();
            record_query(&key, "loading", format!("page {cursor}"));
            async move {
                let result = fake_api::posts_page(cursor).await;
                match &result {
                    Ok(page) => record_query(
                        &key,
                        "loaded",
                        format!(
                            "{} posts, more {}",
                            page.posts.len(),
                            page.next_cursor.is_some()
                        ),
                    ),
                    Err(error) => record_query(&key, "error", error.to_string()),
                }
                result
            }
        },
    );
    let edits = feed_edits();
    // The rows the list shows: the loaded pages with the edits merged over
    // them, so an optimistic like shows before and after its write.
    let rows = computed(move || {
        let edits = edits.get();
        posts
            .pages()
            .iter()
            .flat_map(|page| page.posts.iter().cloned())
            .map(|mut post| {
                if let Some(&(liked, likes)) = edits.likes.get(&post.id) {
                    post.liked = liked;
                    post.likes = likes;
                }
                if let Some(&count) = edits.comment_counts.get(&post.id) {
                    post.comments = count;
                }
                post
            })
            .collect::<Vec<Post>>()
    });
    let like = feed_like(rows);
    let refreshing = signal(false);
    // Reloads go through this tick: the infinite query's fetch task belongs
    // to whoever pressed, and the Retry control's branch unmounts the moment
    // the reset flips the status, so it must not own the load.
    let reloads = signal(0_u32);
    let api = fake_api::fake_api();
    // Zero pages with a failure is the error state; zero pages otherwise is
    // the skeletons; pages mean the rows.
    let loaded = move || posts.page_count() > 0;
    let failed = move || posts.page_count() == 0 && posts.status() == QueryStatus::Failure;
    let error_text = move || {
        posts
            .error()
            .map(|error| error.to_string())
            .unwrap_or_else(|| "The request failed".to_owned())
    };
    // The banner, the skeletons and the error ride in the list's header, so
    // the list is mounted from the first frame: the iOS header only slides
    // its glass and edge blur over a scroll view that is there when the
    // screen is set up, and a list mounted after the first page came too late.
    let status = view()
        .gap_3()
        .child(when(move || api.with(|api| api.offline), offline_banner))
        .child(when(
            move || !loaded() && !failed(),
            move || skeletons().into_element(),
        ))
        .child(when(failed, move || {
            error_state(error_text(), move || reloads.update(|n| *n += 1))
        }));
    let list = virtual_list(
        move || rows.with(Vec::len),
        move |at| {
            let post = computed(move || rows.with(|rows| rows[at.index()].clone()));
            post_row(post, like)
        },
    )
    .keys(move || rows.with(|rows| rows.iter().map(|post| post.id).collect::<Vec<_>>()))
    .estimated_item_size(px(108.0))
    .content_padding(px(16.0))
    .content_inset(crate::ui::layout::page_insets(0.0))
    .row_gap(px(12.0))
    .on_load_more(move |_: &()| posts.fetch_next_page())
    .load_more_threshold(1.0)
    .test_id("feed-list")
    .refreshing(refreshing)
    .on_refresh(move |_: &()| {
        // The pages about to arrive carry the server truth; drop the edits.
        edits.set(FeedEdits::default());
        refreshing.set(true);
        reloads.update(|n| *n += 1);
    })
    .header(status)
    .footer(
        view()
            .gap_2()
            .items_center()
            .py_2()
            .child(when(
                move || posts.is_fetching_next_page(),
                || {
                    text("Loading more…")
                        .text_xs()
                        .text_color(move || theme().muted_foreground)
                },
            ))
            .child(when(
                move || !posts.has_next_page() && posts.page_count() > 0,
                || {
                    text("You're all caught up")
                        .text_xs()
                        .text_color(move || theme().muted_foreground)
                },
            )),
    )
    .flex_1();
    // The reload runs here, under the page. Two effects: the reload's own
    // run must not track the status it resets, or it would refetch itself.
    // A reload asked for from outside the feed (the header's button).
    let outside = super::refresh_request();
    let seen = outside.get_untracked();
    effect(move || {
        if outside.get() != seen {
            untracked(|| reloads.update(|n| *n += 1));
        }
    });
    effect(move || {
        let tick = reloads.get();
        if tick > 0 {
            posts.refetch();
        }
    });
    // The spinner goes down when the first page of the run lands or the run
    // fails.
    effect(move || {
        let settled = posts.page_count() >= 1 || posts.status() == QueryStatus::Failure;
        if refreshing.get() && settled {
            refreshing.set(false);
        }
    });
    view()
        .flex_1()
        .text_color(move || theme().foreground)
        .child(list)
}
