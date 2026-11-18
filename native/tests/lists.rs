//! Lists and data views, through the UI on a manual clock: sort orders,
//! off-screen jumps keeping per-card state, both chat loaders (older
//! appends, newer prepends without jumping), calendar selection, and
//! pagination. The "Break seen:" records next to each test name the
//! purposeful break whose failure was observed before the test passed.

// Break seen, chat_pages_stay_contiguous_with_the_window: `older_page`
// made to include `held` (`first..=held`), duplicating the seam id.
// Break seen, sorting_a_column_orders_the_rows: `sort_quotes` made to
// ignore the requested direction (`dir` identity) — failed at "the
// cheapest row should have left the screen".
// Break seen, a_scroll_past_the_screen_keeps_the_stars_with_their_cards:
// the star state moved into a cell-local signal in `card()` — failed at
// "the star lives in the cell, not in the card's data" (the header stayed
// at "0 starred").
// Break seen, the_start_loader_brings_new_messages_without_jumping:
// `load_newer` made to record the page's oldest id as the new window top
// (`newer_until = page[page.len() - 1]`) — the newer pages never
// completed, and the test failed waiting for "msg 300". (Removing
// `.keys(…)` from this inverted list changes nothing visible: an insert
// at the array's head is an append at the layout's tail, which the list
// handles without keys — keys stay per the coding guides.)
// Break seen, loading_older_messages_appends_at_the_top: `load_older`
// made to return early (no page ever appended) — failed waiting for
// "msg 226".
// Break seen, picking_a_day_selects_it_and_weekends_refuse: the
// calendar's `on_change` made a no-op — failed waiting for "Selected
// 2026-11-18. Weekends are off.".
// Break seen, choosing_a_page_shows_its_rows: `first()` pinned to page 1
// (`move || 1`) — the caption still reached "Page 3 of 50" (it reads the
// signal `pagination_of` writes), so the break failed at the
// "text:Archive 21" wait: the rows stayed on page 1.

use std::time::{Duration, SystemTime};

use nsh_base::{DataTableSort, DataTableSortDirection};
use nsh_core::{Size, prelude::*};
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_starter::features::lists::{
    calendar_view::CalendarDemo,
    chat::{ChatDemo, WINDOW_BOTTOM, WINDOW_TOP},
    grid::GridDemo,
    pager::PagerDemo,
    quotes::{QuotesDemo, quote_rows, sort_quotes},
};
use nsh_testing::Headless;

/// The engine's time during the runs: 2026-11-11, so "today" in the
/// calendar is fixed whatever machine runs this.
fn noonish() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_355_200)
}

/// Starts a demo as the whole app, phone-height and on the manual clock.
fn start<C: Component<Props = ()>>() -> Headless {
    let config = EngineConfig {
        viewport: Size::new(px(600.0), px(844.0)),
        clock: Clock::manual(noonish()),
        ..EngineConfig::new(Root::of::<C>())
    };
    let mut app = Headless::start(config).expect("the engine starts");
    // `start` can return before the first frame, so wait for something.
    app.first_render().unwrap();
    app
}

/// The quotes' expected order, the way the app sorts them.
fn expected_first(key: &str, direction: DataTableSortDirection) -> String {
    // `#[non_exhaustive]`: built from the default, never as a literal.
    let mut sort = DataTableSort::default();
    sort.key = key.to_owned().into();
    sort.direction = direction;
    let mut rows = quote_rows(3000);
    sort_quotes(&mut rows, &sort);
    rows[0].symbol.clone()
}

/// The msg ids the screen holds, sorted, deduplicated.
fn visible_msgs(app: &Headless) -> Vec<u32> {
    let mut ids: Vec<u32> = app
        .screen()
        .texts()
        .iter()
        .filter_map(|t| t.strip_prefix("msg ").and_then(|n| n.parse().ok()))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[test]
fn chat_pages_stay_contiguous_with_the_window() {
    use nsh_starter::features::lists::chat::{OLDEST, PAGE, newer_page, older_page};

    // The window and the counters, exactly as the demo holds them. The
    // ceiling of a newer page is the room's newest, which Receive grows.
    let mut window: Vec<u32> = (240..=280).rev().collect();
    let (mut newer_until, mut older_until) = (280_u32, 240_u32);
    let mut received = 300_u32;

    // The demo's two loads, as its handlers apply them.
    macro_rules! load_newer {
        () => {{
            let page = newer_page(newer_until, received);
            newer_until = page[0];
            window.splice(0..0, page);
        }};
    }
    macro_rules! load_older {
        () => {{
            let page = older_page(older_until);
            older_until = page[page.len() - 1];
            window.extend(page);
        }};
    }

    // Newer pages until the room's end, then older ones back down, then a
    // Receive and its page: every order the user can produce.
    load_newer!();
    load_newer!();
    load_older!();
    load_older!();
    received += 1;
    load_newer!();
    load_older!();
    received += 1;
    load_newer!();

    // The window is a gapless, duplicate-free run of consecutive ids, and
    // its ends are exactly the loaders' bookkeeping.
    for pair in window.windows(2) {
        assert_eq!(pair[0], pair[1] + 1, "a gap or duplicate at {}", pair[1]);
    }
    assert_eq!(window.first(), Some(&newer_until), "the top drifted");
    assert_eq!(window.last(), Some(&older_until), "the bottom drifted");
    assert!(window.first().unwrap() <= &received, "above the room");
    assert!(window.last().unwrap() >= &OLDEST, "below the room's floor");
    // This session exhausted the room's newest and walked three older
    // pages down from the window's floor: the ends say exactly that.
    assert_eq!(newer_until, received, "not caught up to the room");
    assert_eq!(older_until, 240 - 3 * PAGE, "older pages drifted");
}

#[test]
fn sorting_a_column_orders_the_rows() {
    let mut app = start::<QuotesDemo>();
    // In table order the first quote leads.
    app.wait_for("text:AAPL").unwrap();

    // One press of the price title: ascending, cheapest first.
    app.press("id:sort:price").unwrap();
    let cheapest = expected_first("price", DataTableSortDirection::Ascending);
    app.wait_for(&format!("text:{cheapest}")).unwrap();
    // The old first row sat mid-ranked; it is off screen now.
    assert!(
        app.screen().count("text:AAPL") == Ok(0),
        "the unsorted first row should have left the screen"
    );

    // A second press: descending, priciest first.
    app.press("id:sort:price").unwrap();
    let priciest = expected_first("price", DataTableSortDirection::Descending);
    app.wait_for(&format!("text:{priciest}")).unwrap();
    assert!(
        app.screen().count(&format!("text:{cheapest}")) == Ok(0),
        "the cheapest row should have left the screen"
    );
}

#[test]
fn a_scroll_past_the_screen_keeps_the_stars_with_their_cards() {
    let mut app = start::<GridDemo>();
    app.wait_for("text:Card 1").unwrap();

    // Star the second card: the header counts the card's own state.
    app.press("label:Star 2").unwrap();
    assert_eq!(app.screen().count("text:★"), Ok(1));
    assert_eq!(
        app.screen().count("text:1 starred"),
        Ok(1),
        "the star lives in the cell, not in the card's data"
    );

    // Scroll far down, then further, then half a screen back up: no
    // starred card is in view anywhere there.
    app.scroll("id:grid", px(4000.0)).unwrap();
    app.scroll("id:grid", px(2000.0)).unwrap();
    app.wait_for("text:Card 145").unwrap();
    app.scroll("id:grid", px(-600.0)).unwrap();
    app.wait_for("text:Card 135").unwrap();
    assert_eq!(
        app.screen().count("text:★"),
        Ok(0),
        "a star travelled with a cell"
    );

    // Back almost at the top: the star is still on the second card.
    app.scroll("id:grid", px(-5200.0)).unwrap();
    app.wait_for("text:Card 2").unwrap();
    assert_eq!(app.screen().count("text:★"), Ok(1));
    assert!(
        app.screen().count("label:Star 2") != Ok(0),
        "the star is not on card 2"
    );
}

#[test]
fn the_start_loader_brings_new_messages_without_jumping() {
    let mut app = start::<ChatDemo>();
    // The chat opens on ids 240..=280; 281..=300 are newer pages it holds
    // none of, and it opens parked at the start.
    app.wait_for("text:msg 280").unwrap();

    // A screen up, then back to the start: the approach loads the newer
    // pages, and the room's newest ends up at the bottom edge.
    app.scroll("id:chat-list", px(-800.0)).unwrap();
    app.scroll("id:chat-list", px(2000.0)).unwrap();
    app.wait_for("text:msg 300").unwrap();

    // Far enough up that the start's approach latch clears, and the
    // messages this offset shows are the no-jump baseline.
    app.scroll("id:chat-list", px(-1600.0)).unwrap();
    let before = visible_msgs(&app);

    // A message arrives, the list approaches the start (the loader
    // prepends it), and the test returns to the very same offset.
    app.press("label:Receive a message").unwrap();
    app.scroll("id:chat-list", px(800.0)).unwrap();
    app.scroll("id:chat-list", px(-800.0)).unwrap();
    let after = visible_msgs(&app);

    // The same offset shows the same messages: every window id well
    // inside the pool is still on screen, and the arrival is not (it
    // joined out of sight, at the start's edge).
    let kept = WINDOW_BOTTOM..=(WINDOW_TOP - 5);
    let missing: Vec<u32> = before
        .iter()
        .copied()
        .filter(|id| kept.contains(id) && !after.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "the visible messages shifted when the newer one joined: \\
         {before:?} -> {after:?}, missing {missing:?}"
    );
    assert!(
        app.screen().count("text:msg 301") == Ok(0),
        "the arrival landed in the visible rows"
    );

    // The arrival is the bottom row: the Latest jump binds and mounts it.
    app.press("label:Latest message").unwrap();
    app.wait_for("text:msg 301").unwrap();
}

#[test]
fn loading_older_messages_appends_at_the_top() {
    let mut app = start::<ChatDemo>();
    app.wait_for("text:msg 280").unwrap();

    // Toward the visual top: the older loader appends its first page
    // (ids 225..=239, seamless with the window's 240).
    app.scroll("id:chat-list", px(-1200.0)).unwrap();
    app.wait_for("text:msg 226").unwrap();
    assert!(
        app.screen().count("text:msg 239") != Ok(0),
        "the page left a gap at the window's bottom"
    );

    // Deeper: pages keep appending, still one gapless run.
    app.scroll("id:chat-list", px(-2000.0)).unwrap();
    app.wait_for("text:msg 170").unwrap();

    // Whatever is mounted is one gapless run of ids: no holes, no dupes.
    let ids = visible_msgs(&app);
    let span = ids.last().unwrap_or(&0) - ids.first().unwrap_or(&0) + 1;
    assert_eq!(
        span,
        ids.len() as u32,
        "the mounted messages have a gap or duplicate: {ids:?}"
    );
}

#[test]
fn picking_a_day_selects_it_and_weekends_refuse() {
    let mut app = start::<CalendarDemo>();
    // The schedule opens holding November 12th, 2026.
    app.wait_for("text:November 2026").unwrap();
    app.wait_for("text:Selected 2026-11-12. Weekends are off.")
        .unwrap();

    // A Wednesday is picked and reported.
    app.press("label:2026-11-18").unwrap();
    app.wait_for("text:Selected 2026-11-18. Weekends are off.")
        .unwrap();

    // The Saturday of that week is off: the press is refused.
    app.press("label:2026-11-14").unwrap_err();
    assert!(
        app.screen()
            .count("text:Selected 2026-11-18. Weekends are off.")
            != Ok(0),
        "a refused press changed the selection"
    );
}

#[test]
fn choosing_a_page_shows_its_rows() {
    let mut app = start::<PagerDemo>();
    app.wait_for("text:Archive 1").unwrap();
    app.wait_for("text:Page 1 of 50").unwrap();

    app.press("text:3").unwrap();
    app.wait_for("text:Page 3 of 50").unwrap();
    app.wait_for("text:Archive 21").unwrap();
    assert!(
        app.screen().count("text:Archive 1") == Ok(0),
        "the first page's rows lingered"
    );
}
