//! The data feature's tests: the feed, its detail and the devtools driven
//! through the UI on a manual clock, against the local fake API.

use std::time::{Duration, SystemTime};

use nsh_base::{button, theme};
use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_testing::Headless;

use nsh_query::query_client;
use nsh_starter::features::data::{data_page, feed_key, query_devtools_page};

/// The test's surface: the data page on top and the devtools beneath, so a
/// switch flip reaches a live feed without unmounting it. `Remount feed`
/// drops the feed's cache entry and rebuilds the data page, which is how a
/// first load is made to fail: a remount alone is served the cached pages.
#[component]
pub fn DataTestApp() -> impl IntoElement {
    let live = signal(0_u64);
    view()
        .flex_1()
        .bg(move || theme().background)
        .child(
            view().flex_row().p_2().gap_2().child(
                button()
                    .on_press(move |_: &()| {
                        query_client().remove(feed_key());
                        live.update(|n| *n += 1);
                    })
                    .h(px(32.0))
                    .px_3()
                    .items_center()
                    .justify_center()
                    .rounded(move || theme().radius)
                    .bg(move || theme().secondary)
                    .child(
                        text("Remount feed")
                            .text_xs()
                            .text_color(move || theme().secondary_foreground),
                    ),
            ),
        )
        .child(view().flex_1().child(dynamic(move || {
            let live = live.get();
            untracked(|| {
                view()
                    .key(format!("data-{live}"))
                    .flex_1()
                    .child(data_page())
                    .into_element()
            })
        })))
        .child(
            view()
                .h(px(300.0))
                .border_t_1()
                .border_color(move || theme().border)
                .child(query_devtools_page()),
        )
}

fn start() -> Headless {
    let mut config = EngineConfig::new(Root::of::<DataTestApp>());
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_767_258_000));
    Headless::start(config).unwrap()
}

fn settle(app: &mut Headless) {
    app.run_for(Duration::from_secs(2)).unwrap();
}

#[test]
fn pagination_loads_more_pages() {
    let mut app = start();
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);
    assert_eq!(
        app.screen().count("label:Post 11, Shipping a Rust shell"),
        Ok(0)
    );

    app.scroll("id:feed-list", px(4000.0)).unwrap();
    settle(&mut app);
    app.wait_for("label:Post 11, Shipping a Rust shell")
        .unwrap();
    app.scroll("id:feed-list", px(8000.0)).unwrap();
    settle(&mut app);
    app.wait_for("label:Post 21, Shipping a Rust shell")
        .unwrap();
    // Past the last page the loader stops.
    app.scroll("id:feed-list", px(8000.0)).unwrap();
    settle(&mut app);
    assert!(app.shows("You're all caught up"));
}

#[test]
fn an_optimistic_like_rolls_back_when_the_write_fails() {
    let mut app = start();
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);
    // Post 1 starts liked with 3 likes (the dataset's formula).
    app.wait_for("text:♥ 3").unwrap();

    app.press("text:All").unwrap();
    app.press("role:button[Like post 1]").unwrap();
    // The like shows before the write has any answer.
    app.wait_for("text:♡ 2").unwrap();

    settle(&mut app);
    // The write failed, so the heart went back.
    app.wait_for("text:♥ 3").unwrap();
    assert_eq!(app.screen().count("text:♡ 2"), Ok(0));
}

#[test]
fn the_offline_banner_shows_and_writes_fail_while_offline() {
    let mut app = start();
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);
    // Post 2 starts unliked with 10 likes.
    app.wait_for("text:♡ 10").unwrap();

    app.press("label:Offline").unwrap();
    app.wait_for("text:Offline — requests fail until you turn the switch back")
        .unwrap();
    // Offline fails at once, so the optimistic heart never paints; the like
    // must not stick either way.
    app.press("role:button[Like post 2]").unwrap();
    settle(&mut app);
    app.wait_for("text:♡ 10").unwrap();

    app.press("label:Offline").unwrap();
    app.wait_for_gone(
        "text:Offline — requests fail until you turn the switch back",
        Duration::from_secs(1),
    )
    .unwrap();
}

#[test]
fn posting_a_comment_invalidates_the_detail_queries() {
    let mut app = start();
    app.wait_for("label:Post 3, Signals over snapshots")
        .unwrap();
    settle(&mut app);

    app.press("label:Post 3, Signals over snapshots").unwrap();
    app.wait_for("text:revision 1").unwrap();
    app.wait_for("text:2 comments").unwrap();

    app.type_text("text:Say something about this post", "Nice post")
        .unwrap();
    app.press("role:button[Post]").unwrap();
    settle(&mut app);
    // The mutation succeeded, and its invalidation refetched both queries.
    app.wait_for("text:Nice post").unwrap();
    app.wait_for("text:3 comments").unwrap();
    app.wait_for("text:revision 2").unwrap();
    assert!(
        app.screen()
            .input_text("text:Say something about this post")
            == Ok(Some(""))
    );

    // The feed row carries the new count too.
    app.press("role:button[Back to feed]").unwrap();
    app.wait_for("label:Post 3, Signals over snapshots")
        .unwrap();
    app.wait_for("text:3 comments").unwrap();
}

#[test]
fn a_stale_detail_serves_its_cache_and_then_revalidates() {
    let mut app = start();
    app.wait_for("label:Post 4, The five-minute query").unwrap();
    settle(&mut app);

    app.press("label:Post 4, The five-minute query").unwrap();
    app.wait_for("text:revision 1").unwrap();

    // While the answer is fresh, a second visit loads nothing at all.
    app.press("role:button[Back to feed]").unwrap();
    app.press("label:Post 4, The five-minute query").unwrap();
    app.wait_for("text:revision 1").unwrap();
    assert_eq!(app.screen().count("text:Refreshing…"), Ok(0));
    assert_eq!(app.screen().count("text:/^3 requests/"), Ok(1));

    // Past the detail TTL the stale answer is served at once, and the
    // refetch lands only when the test moves the clock.
    app.run_for(Duration::from_secs(3)).unwrap();
    app.press("role:button[Back to feed]").unwrap();
    app.press("label:Post 4, The five-minute query").unwrap();
    app.wait_for("text:revision 1").unwrap();
    assert!(app.shows("Refreshing…"));
    assert!(app.shows("revision 1"));
    app.run_for(Duration::from_millis(500)).unwrap();
    app.wait_for("text:revision 2").unwrap();
}

#[test]
fn retry_recovers_a_failed_feed() {
    let mut app = start();
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);

    app.press("text:All").unwrap();
    // A fresh page must load through the failure switch.
    app.press("text:Remount feed").unwrap();
    settle(&mut app);
    app.wait_for("text:Couldn't load the feed").unwrap();
    app.wait_for("text:Retry").unwrap();

    app.press("text:Never").unwrap();
    app.press("text:Retry").unwrap();
    settle(&mut app);
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
}

#[test]
fn the_devtools_list_the_keys_the_feature_loaded() {
    let mut app = start();
    app.wait_for("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);
    app.wait_for("text:feed").unwrap();
    assert!(app.shows("10 posts, more true"));
    assert!(app.shows("loaded"));

    app.press("label:Post 1, Shipping a Rust shell").unwrap();
    settle(&mut app);
    app.wait_for("text:post, 1").unwrap();
    app.wait_for("text:comments, 1").unwrap();
    // The cached post's data is readable through the query client.
    app.wait_for("text:/rev 1, 3 likes/").unwrap();
}
