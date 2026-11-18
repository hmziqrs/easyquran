//! The data feature: the feed, its detail page and the query devtools on
//! `nsh-query`, backed by the local fake API in `src/services/fake_api.rs`
//! (owned by this area). Owned by the data area. The shell mounts
//! `data_page`, `data_detail_page` and `query_devtools_page`; keep their
//! names and signatures.
//!
//! Navigation seam: a tap on a feed post calls [`open_post`], and
//! [`data_page`] presents the detail over the feed until [`close_post`].
//! Deep links mount [`data_detail_page`] directly. Once the shell's route
//! enum lands, it can instead watch [`detail_request`] and push the detail
//! route on its `nav_stack`.

mod detail;
mod devtools;
mod feed;

use std::collections::HashMap;

use nsh_core::prelude::*;
use nsh_query::QueryKey;

use crate::services::fake_api;

/// The feed page: an infinite, virtualized, pull-to-refresh feed of posts.
/// A tap on a post presents the detail over the feed until it is closed.
pub fn data_page() -> impl IntoElement {
    let request = detail_request();
    // The detail covers the feed instead of replacing it: the feed's pages
    // and scroll position survive the round trip.
    // A shell that pushes the detail as its own route (phones: it watches
    // `detail_request` and says so through `pushes_detail`) takes it from
    // here; anywhere else, the feed presents it itself.
    let pushed = pushes_detail();
    view().flex_1().child(feed::feed_page()).child(when(
        move || !pushed.get() && request.get().is_some(),
        move || {
            // Tracked: a different id presented while the overlay is up
            // rebuilds the detail for that post instead of showing the old
            // one.
            dynamic(move || {
                let id = request.get().unwrap_or(0);
                untracked(|| {
                    view()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(detail::post_detail(id, true))
                        .into_element()
                })
            })
            .into_element()
        },
    ))
}

/// The detail page for one post; `id` comes from the shell's route.
pub fn data_detail_page(id: u64) -> impl IntoElement {
    detail::post_detail(id, false)
}

/// The query devtools: cache entries and the fake API's switches.
pub fn query_devtools_page() -> impl IntoElement {
    devtools::devtools_page()
}

/// The feed's infinite-query key.
pub fn feed_key() -> QueryKey {
    QueryKey::from("feed")
}

/// The detail query's key for one post.
pub fn post_key(id: u64) -> QueryKey {
    let id = id.to_string();
    QueryKey::new(["post", id.as_str()])
}

/// The comments query's key for one post.
pub fn comments_key(id: u64) -> QueryKey {
    let id = id.to_string();
    QueryKey::new(["comments", id.as_str()])
}

/// How long a detail query's answer stays fresh before it serves stale and
/// revalidates.
pub const DETAIL_TTL_MS: u64 = fake_api::DETAIL_TTL_MS;

/// The post the feed wants open, if any.
pub fn detail_request() -> Signal<Option<u64>> {
    global::<Option<u64>>()
}

/// Set by a shell that pushes the detail route itself when a post is
/// opened; the feed then presents no overlay of its own.
pub fn pushes_detail() -> Signal<bool> {
    global::<bool>()
}

/// Bumped to reload the feed from outside it (the shell's native header
/// button on iOS).
pub fn refresh_request() -> Signal<u32> {
    global::<u32>()
}

/// Reloads the feed from its first page.
pub fn refresh_feed() {
    refresh_request().update(|n| *n += 1);
}

/// Opens a post's detail over the feed.
pub fn open_post(id: u64) {
    detail_request().set(Some(id));
}

/// Closes a detail the feed presented.
pub fn close_post() {
    detail_request().set(None);
}

/// One like write: the post and what its liked state becomes.
#[derive(Clone, PartialEq, Eq)]
pub struct LikeInput {
    pub post_id: u64,
    pub to: bool,
}

/// What the app knows about a post that the loaded feed pages predate: a
/// like or a comment count set through a mutation, keyed by post id. The
/// feed merges them over its pages, so an optimistic edit survives until a
/// fresh page carries the server truth itself.
#[derive(Clone, Default, PartialEq)]
pub struct FeedEdits {
    pub likes: HashMap<u64, (bool, u64)>,
    pub comment_counts: HashMap<u64, u64>,
}

/// The surface's feed edits.
pub fn feed_edits() -> Signal<FeedEdits> {
    global::<FeedEdits>()
}

/// Applies a like (an optimistic one first, the server's after it).
pub fn apply_like(post_id: u64, liked: bool, likes: u64) {
    feed_edits().update(|edits| {
        edits.likes.insert(post_id, (liked, likes));
    });
}

/// Drops a like's edit after its write failed.
pub fn revert_like(post_id: u64) {
    feed_edits().update(|edits| {
        edits.likes.remove(&post_id);
    });
}

/// Applies a comment count the server confirmed.
pub fn apply_comment_count(post_id: u64, count: u64) {
    feed_edits().update(|edits| {
        edits.comment_counts.insert(post_id, count);
    });
}

/// What the devtools know about one key this feature loaded. nsh-query's
/// client cannot list its entries, so the fetchers record their own keys;
/// the cached data itself is read through `query_client`.
#[derive(Clone, PartialEq)]
pub struct QueryInfo {
    pub key: QueryKey,
    /// "loading", "loaded" or "error" at the last transition.
    pub status: &'static str,
    pub note: String,
    pub updated_ms: u64,
}

/// The surface's query log, one entry per key.
pub fn query_log() -> Signal<Vec<QueryInfo>> {
    global::<Vec<QueryInfo>>()
}

/// Records one transition of `key` for the devtools page.
pub fn record_query(key: &QueryKey, status: &'static str, note: String) {
    let info = QueryInfo {
        key: key.clone(),
        status,
        note,
        // The engine's clock: what the queries' own timestamps and the
        // manual test clock use, so the devtools' ages agree with them.
        updated_ms: nsh_core::time::unix_ms() as u64,
    };
    query_log().update(
        |log| match log.iter_mut().find(|entry| entry.key == info.key) {
            Some(entry) => *entry = info,
            None => log.push(info),
        },
    );
}
