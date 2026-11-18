//! The post detail: the post, its author, its comments, an optimistic like
//! with rollback and a comment form on `nsh-form`, on a tracked query key.

use std::time::Duration;

use nsh_base::{button, theme};
use nsh_core::prelude::*;
use nsh_form::{BindField, SubmitError, field, rules, use_form};
use nsh_query::{
    MutationOptions, Query, QueryOptions, RetryPolicy, query_client, use_mutation_with,
    use_query_by,
};

use super::{
    LikeInput, apply_comment_count, apply_like, comments_key, post_key, record_query, revert_like,
};
use crate::services::fake_api::{self, ApiError, Comment, Post};

/// One comment draft: what the form holds.
#[derive(Clone, Default, PartialEq)]
struct CommentDraft {
    body: String,
}

/// The detail's like write: optimistic through the query's `set_data`, with
/// `rollback` and a feed edit if the fake API rejects it.
fn detail_like(
    post_query: Query<Post, ApiError>,
) -> nsh_query::Mutation<LikeInput, Post, ApiError> {
    use_mutation_with::<LikeInput, Post, ApiError, _, _>(
        MutationOptions::new()
            .on_mutate(move |input: &LikeInput| {
                if let Some(post) = post_query.data() {
                    let (liked, likes) = if post.liked == input.to {
                        (post.liked, post.likes)
                    } else if input.to {
                        (true, post.likes + 1)
                    } else {
                        (false, post.likes.saturating_sub(1))
                    };
                    let mut next = post;
                    next.liked = liked;
                    next.likes = likes;
                    post_query.set_data(next);
                    apply_like(input.post_id, liked, likes);
                }
            })
            .on_success(move |post: &Post, input: &LikeInput| {
                apply_like(input.post_id, post.liked, post.likes)
            })
            .on_error(move |_error: &ApiError, input: &LikeInput| {
                post_query.rollback();
                revert_like(input.post_id);
            }),
        |input: LikeInput, _signal| async move { fake_api::like(input.post_id, input.to).await },
    )
}

/// The detail page's content: the post card, its comments and the comment
/// form. `with_back` adds the bar the feed's internal presentation closes
/// the detail with.
pub fn post_detail(id: u64, with_back: bool) -> impl IntoElement {
    // The key follows this signal. The page is built with the presented id,
    // and everything that names a post reads it, so re-keying the page
    // fetches and invalidates the right post.
    let current = signal(id);
    let post_query = use_query_by::<Post, ApiError, _, _, _>(
        move || post_key(current.get()),
        QueryOptions::new()
            .stale_while_revalidate(
                Duration::from_millis(super::DETAIL_TTL_MS),
                Duration::from_secs(300),
            )
            .retry(RetryPolicy::new(2).with_delay(200)),
        move |_context| {
            let id = current.get_untracked();
            let key = post_key(id);
            record_query(&key, "loading", format!("post {id}"));
            async move {
                let result = fake_api::post(id).await;
                match &result {
                    Ok(post) => record_query(
                        &key,
                        "loaded",
                        format!("rev {}, {} likes", post.revision, post.likes),
                    ),
                    Err(error) => record_query(&key, "error", error.to_string()),
                }
                result
            }
        },
    );
    let comments_query = use_query_by::<Vec<Comment>, ApiError, _, _, _>(
        move || comments_key(current.get()),
        QueryOptions::new()
            .ttl(Duration::from_secs(60))
            .retry(RetryPolicy::new(2).with_delay(200)),
        move |_context| {
            let id = current.get_untracked();
            let key = comments_key(id);
            record_query(&key, "loading", format!("comments of post {id}"));
            async move {
                let result = fake_api::comments(id).await;
                match &result {
                    Ok(comments) => {
                        record_query(&key, "loaded", format!("{} comments", comments.len()))
                    }
                    Err(error) => record_query(&key, "error", error.to_string()),
                }
                result
            }
        },
    );
    let like = detail_like(post_query);
    let add_comment = use_mutation_with::<String, Comment, ApiError, _, _>(
        MutationOptions::new()
            // The prefix form cannot follow the tracked key, so the success
            // hook invalidates the presented post's two queries itself.
            .on_success(move |_comment: &Comment, _body: &String| {
                let id = current.get_untracked();
                let client = query_client();
                client.invalidate(post_key(id));
                client.invalidate(comments_key(id));
            }),
        move |body: String, _signal| {
            let id = current.get_untracked();
            async move { fake_api::add_comment(id, body).await }
        },
    );
    // What the detail loads is server truth for the feed's row too.
    effect(move || {
        if let Some(post) = post_query.data() {
            apply_comment_count(post.id, post.comments);
        }
    });
    let form = use_form(CommentDraft::default());
    let body = field!(form, body).rule(rules::required("Write a comment first"));
    let comments = computed(move || comments_query.data().unwrap_or_default());
    let like_post = move || {
        let liked = post_query.data().map(|post| post.liked).unwrap_or(false);
        like.mutate(LikeInput {
            post_id: current.get_untracked(),
            to: !liked,
        });
    };
    let content = view().flex_1().child(
        scroll()
            .flex_1()
            .keyboard_taps(KeyboardTaps::Handled)
            .child(
                view()
                    .p_4()
                    .gap_4()
                    .child(post_card(id, post_query, like_post))
                    .child(comments_section(comments))
                    .child(comment_form(form, body, add_comment)),
            ),
    );
    if with_back {
        view()
            .flex_1()
            .p_safe_area()
            .bg(move || theme().background)
            .text_color(move || theme().foreground)
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .py_2()
                    .gap_1()
                    .child(
                        // A chevron, not the word "Back": the shell's page
                        // header has its own Back, and test selectors must
                        // tell the two apart.
                        button()
                            .accessibility_label("Back to feed")
                            .on_press(move |_: &()| super::close_post())
                            .h(px(36.0))
                            .w(px(36.0))
                            .items_center()
                            .justify_center()
                            .rounded(move || theme().radius)
                            .active(|s| s.bg(move || theme().muted))
                            .child(text("‹").text_lg().text_color(move || theme().foreground)),
                    ),
            )
            .child(content)
    } else {
        view()
            .flex_1()
            .p_safe_area()
            .bg(move || theme().background)
            .text_color(move || theme().foreground)
            .child(content)
    }
}

/// The post card: title, author, body, the like control and how fresh the
/// answer is.
fn post_card(
    id: u64,
    post_query: Query<Post, ApiError>,
    like_post: impl Fn() + 'static,
) -> impl IntoElement {
    view()
        .gap_2()
        .p_4()
        .rounded(move || theme().radius)
        .bg(move || theme().card)
        .border_1()
        .border_color(move || theme().border)
        .child(
            text(move || {
                post_query
                    .data()
                    .map(|post| post.title.clone())
                    .unwrap_or_else(|| "Loading…".to_owned())
            })
            .text_lg()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(move || theme().card_foreground),
        )
        .child(
            text(move || {
                post_query
                    .data()
                    .map(|post| format!("By {}", post.author))
                    .unwrap_or_default()
            })
            .text_xs()
            .text_color(move || theme().muted_foreground),
        )
        .child(
            text(move || {
                post_query
                    .data()
                    .map(|post| post.body.clone())
                    .unwrap_or_default()
            })
            .text_sm()
            .text_color(move || theme().card_foreground),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    button()
                        .accessibility_label(SharedString::from(format!("Like post {id}")))
                        .disabled(move || post_query.data().is_none())
                        .on_press(move |_: &()| like_post())
                        .h(px(32.0))
                        .px_3()
                        .items_center()
                        .justify_center()
                        .rounded(move || theme().radius)
                        .bg(move || theme().muted)
                        .active(|s| s.opacity(0.7))
                        .child(
                            text(move || {
                                post_query
                                    .data()
                                    .map(|post| {
                                        format!(
                                            "{} {}",
                                            if post.liked { "♥" } else { "♡" },
                                            post.likes
                                        )
                                    })
                                    .unwrap_or_else(|| "♡".to_owned())
                            })
                            .text_xs()
                            .text_color(move || theme().card_foreground),
                        ),
                )
                .child(
                    text(move || match post_query.data() {
                        Some(post) => format!("revision {}", post.revision),
                        None => String::new(),
                    })
                    .text_xs()
                    .text_color(move || theme().muted_foreground),
                )
                .child(when(
                    move || post_query.is_fetching() && post_query.data().is_some(),
                    || {
                        text("Refreshing…")
                            .text_xs()
                            .text_color(move || theme().muted_foreground)
                    },
                )),
        )
        .child(when(
            move || post_query.is_failure(),
            move || {
                text(move || {
                    post_query
                        .error()
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "The request failed".to_owned())
                })
                .text_xs()
                .text_color(move || theme().destructive)
            },
        ))
        .child(when(
            move || post_query.is_failure(),
            move || {
                crate::ui::button::button_with(
                    "Retry",
                    crate::ui::button::ButtonVariant::Secondary,
                    crate::ui::button::ButtonSize::Small,
                    false,
                    None,
                    move || post_query.refetch(),
                )
            },
        ))
}

/// The comments of the post, oldest first.
fn comments_section(comments: Memo<Vec<Comment>>) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            text(move || format!("{} comments", comments.with(Vec::len)))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(each(
            comments,
            |comment: &Comment| comment.id,
            |comment: ReadSignal<Comment>, _index: ReadSignal<usize>| {
                view()
                    .gap_1()
                    .p_3()
                    .rounded(move || theme().radius)
                    .bg(move || theme().card)
                    .border_1()
                    .border_color(move || theme().border)
                    .child(
                        text(move || comment.with(|comment| comment.author.clone()))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(move || theme().card_foreground),
                    )
                    .child(
                        text(move || comment.with(|comment| comment.body.clone()))
                            .text_xs()
                            .text_color(move || theme().card_foreground),
                    )
            },
        ))
        .child(when(
            move || comments.with(Vec::is_empty),
            || {
                text("No comments yet")
                    .text_xs()
                    .text_color(move || theme().muted_foreground)
            },
        ))
}

/// The comment form: one body field on `nsh-form`, posted through the
/// mutation whose success invalidates the detail queries.
fn comment_form(
    form: nsh_form::Form<CommentDraft>,
    body: nsh_form::Field<CommentDraft, String>,
    add_comment: nsh_query::Mutation<String, Comment, ApiError>,
) -> impl IntoElement {
    view()
        .gap_2()
        .child(
            text("Add a comment")
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(move || theme().foreground),
        )
        .child(body.bind(crate::ui::field::text_field_frame(
            "Say something about this post",
        )))
        .child(when(
            move || body.error().is_some(),
            move || {
                text(move || body.error().unwrap_or_default())
                    .text_xs()
                    .text_color(move || theme().destructive)
            },
        ))
        .child(when(
            move || form.error().is_some(),
            move || {
                text(move || form.error().unwrap_or_default())
                    .text_xs()
                    .text_color(move || theme().destructive)
            },
        ))
        .child(crate::ui::button::button_with(
            move || {
                if form.is_submitting() {
                    "Posting…"
                } else {
                    "Post"
                }
            },
            crate::ui::button::ButtonVariant::Primary,
            crate::ui::button::ButtonSize::Medium,
            move || form.is_submitting(),
            // A stable name while the label alternates with "Posting…".
            Some("Post"),
            move || {
                form.submit(move |values: CommentDraft| async move {
                    match add_comment.mutate_async(values.body).await {
                        Some(Ok(_)) => {
                            form.load(CommentDraft::default());
                            Ok(())
                        }
                        Some(Err(error)) => Err(SubmitError::form(error.to_string())),
                        None => Err(SubmitError::form("That comment was superseded")),
                    }
                });
            },
        ))
}
