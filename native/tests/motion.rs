//! The motion page on a manual clock: springs settle at their target,
//! swipes recover and dismiss, drops reorder the data, pan activation
//! gates a horizontal drag, and reduced motion swaps springs for instant
//! states. Steps settle the native animations themselves; the clock only
//! moves in `run_for`.
//!
//! Seen-to-fail record: every test below was seen to fail once against the
//! break named here, then the break was restored.
//!
//! | test | break |
//! | --- | --- |
//! | the_page_shows_every_section | springs.rs section title renamed |
//! | a_spring_settles_at_its_target_and_stays | spring target halved |
//! | a_tween_reports_the_same_settled_position | tween target halved |
//! | a_swipe_recovers_and_a_fling_dismisses_the_row | swipe `note` suppressed |
//! | restoring_brings_the_dismissed_rows_back | Restore set TASKS[1..] |
//! | a_drop_moves_the_item_in_the_data | drop's `order.update` suppressed |
//! | the_layout_section_records_its_new_order | layout `items.update` suppressed |
//! | a_route_flip_leaves_after_the_exit_and_enters | pane's `route.update` suppressed |
//! | the_transform_readout_follows_the_drag_and_resets | transform `drag.set` suppressed |
//! | the_header_demo_scrolls_to_rest_and_back | header `rest.set` suppressed |
//! | pan_activation_hands_the_scrubber_the_horizontal_drag | `claim_touches` removed |
//! | reduced_motion_swaps_the_springs_for_instant_states | ambient `reduce_motion()` ignored in springs |
//! | the_calm_preference_swaps_the_springs_without_the_system_setting | page's `calm` disconnected from springs |
//! | a_composite_stagger_runs_to_its_end | stagger stopped right after its start |

use std::time::Duration;

use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Environment, Root, Size};
use nsh_testing::Headless;

/// The page on its own, so the tests do not depend on the shell's root.
#[component]
fn MotionHarness() -> impl IntoElement {
    view()
        .flex_1()
        .child(nsh_starter::features::motion::motion_page())
}

fn start() -> Headless {
    start_with(Environment::default())
}

fn start_with(environment: Environment) -> Headless {
    let mut app = Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(844.0)),
        clock: Clock::manual(std::time::SystemTime::UNIX_EPOCH),
        environment,
        ..EngineConfig::new(Root::of::<MotionHarness>())
    })
    .unwrap();
    // `start` can return before the first frame; wait for the page title.
    app.first_render().unwrap();
    // The page heading is the phone header's; its summary marks the page.
    app.wait_for("text:Springs, tweens, layout animations and native gestures")
        .unwrap();
    app
}

fn on_screen(app: &Headless) -> String {
    app.screen().texts().join(" | ")
}

#[test]
fn the_page_shows_every_section() -> Result<(), String> {
    let mut app = start();
    for title in [
        "Reduced motion",
        "Spring and tween",
        "Composites",
        "Transitions",
        "Layout animations",
        "Swipe to dismiss",
        "Drag and drop",
        "Pan, pinch and rotation",
        "Scroll-linked header",
        "Pan activation",
    ] {
        app.wait_for(&format!("text:{title}"))?;
    }
    Ok(())
}

#[test]
fn a_spring_settles_at_its_target_and_stays() -> Result<(), String> {
    let mut app = start();
    app.press("text:Play to 240")?;
    app.wait_for("text:Rest 240 · finished")?;
    // No residual motion: whatever the clock runs after, the readout holds.
    app.run_for(Duration::from_secs(2))?;
    assert!(app.shows("Rest 240 · finished"), "{}", on_screen(&app));
    // The way back settles too, with the settings the chips picked; the
    // button's label names the target it now plays to.
    app.press("text:Wobbly")?;
    app.press("text:Play to 0")?;
    app.wait_for("text:Rest 0 · finished")?;
    Ok(())
}

#[test]
fn a_tween_reports_the_same_settled_position() -> Result<(), String> {
    let mut app = start();
    app.press("text:Tween")?;
    app.press("text:Play to 240")?;
    app.wait_for("text:Rest 240 · finished")?;
    Ok(())
}

#[test]
fn a_swipe_recovers_and_a_fling_dismisses_the_row() -> Result<(), String> {
    let mut app = start();
    // The headless finger is quick, so its projection carries the fling:
    // this swipe ends short of every snap line and springs home.
    app.drag("id:Beta", point(px(30.0), px(0.0)))?;
    app.wait_for("text:/^Beta recovered/")?;
    assert!(app.shows("Beta"), "{}", on_screen(&app));
    // A fling projects past the dismiss line: the row leaves the data.
    app.drag("id:Beta", point(px(-240.0), px(0.0)))?;
    app.wait_for("text:/^Beta dismissed/")?;
    app.wait_for_gone("id:Beta", Duration::from_secs(2))?;
    // The Delete underlay's action dismisses through the same data update.
    app.press("label:Delete Gamma")?;
    app.wait_for("text:/^Gamma dismissed/")?;
    app.wait_for_gone("id:Gamma", Duration::from_secs(2))?;
    Ok(())
}

#[test]
fn restoring_brings_the_dismissed_rows_back() -> Result<(), String> {
    let mut app = start();
    app.drag("id:Alpha", point(px(-240.0), px(0.0)))?;
    app.wait_for("text:/^Alpha dismissed/")?;
    app.press("role:button[Restore rows]")?;
    app.wait_for("id:Alpha")?;
    Ok(())
}

#[test]
fn a_drop_moves_the_item_in_the_data() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Order: One · Two · Three · Four")?;
    // One row down.
    app.drag("id:Two", point(px(0.0), px(64.0)))?;
    app.wait_for("text:Order: One · Three · Two · Four")?;
    // And back up, so the drop works both ways.
    app.drag("id:Two", point(px(0.0), px(-64.0)))?;
    app.wait_for("text:Order: One · Two · Three · Four")?;
    Ok(())
}

#[test]
fn the_layout_section_records_its_new_order() -> Result<(), String> {
    let mut app = start();
    app.press("role:button[Reverse]")?;
    app.wait_for("text:Reversed — order 5 · 4 · 3 · 2 · 1")?;
    app.press("role:button[Shuffle]")?;
    app.wait_for("text:Shuffled — order 1 · 2 · 3 · 5 · 4")?;
    Ok(())
}

#[test]
fn a_route_flip_leaves_after_the_exit_and_enters() -> Result<(), String> {
    let mut app = start();
    app.press("label:Route A — alpha")?;
    app.wait_for("text:Route B — beta")?;
    app.wait_for_gone("text:Route A — alpha", Duration::from_secs(2))?;
    // And back.
    app.press("label:Route B — beta")?;
    app.wait_for("text:Route A — alpha")?;
    Ok(())
}

#[test]
fn the_transform_readout_follows_the_drag_and_resets() -> Result<(), String> {
    let mut app = start();
    app.drag("id:transform-card", point(px(40.0), px(-20.0)))?;
    app.wait_for("text:Drag 45 pt · scale 1.00× · angle 0°")?;
    app.press("role:button[Reset]")?;
    app.wait_for("text:Drag 0 pt · scale 1.00× · angle 0°")?;
    Ok(())
}

#[test]
fn the_header_demo_scrolls_to_rest_and_back() -> Result<(), String> {
    let mut app = start();
    app.scroll("id:header-scroll", px(600.0))?;
    app.wait_for("text:At rest at 600")?;
    app.scroll("id:header-scroll", px(-600.0))?;
    app.wait_for("text:At rest at 0")?;
    Ok(())
}

#[test]
fn pan_activation_hands_the_scrubber_the_horizontal_drag() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:0 grants · idle")?;
    // A horizontal drag past the active offset claims the touches. Held
    // mid-drag the pad reports the scrub; the lift ends it, leaving the
    // playhead where the release put it (a scrubber keeps its position).
    app.drag_and_hold("id:scrub-pad", point(px(80.0), px(0.0)))?;
    app.wait_for("text:/^1 grant · scrubbing at/")?;
    app.drop_held()?;
    app.wait_for("text:/^1 grant · scrubbed to/")?;
    // A vertical drag passes the fail offset first: the scroll keeps it,
    // so no second grant and the playhead stays.
    app.drag("id:scrub-pad", point(px(0.0), px(60.0)))?;
    app.wait_for("text:/^1 grant · scrubbed to/")?;
    Ok(())
}

#[test]
fn a_composite_stagger_runs_to_its_end() -> Result<(), String> {
    let mut app = start();
    app.press("role:button[Stagger]")?;
    app.wait_for("text:Stagger finished")?;
    app.press("role:button[Sequence]")?;
    app.wait_for("text:Sequence finished")?;
    app.press("role:button[Pulse twice]")?;
    app.wait_for("text:Keyframes finished")?;
    Ok(())
}

#[test]
fn reduced_motion_swaps_the_springs_for_instant_states() -> Result<(), String> {
    let mut app = start_with(Environment {
        reduce_motion: true,
        ..Environment::default()
    });
    // The system setting: the drawer and the knob both go instantly.
    app.press("text:Reveal the drawer")?;
    app.wait_for("text:Instant 0")?;
    app.press("text:Play to 240")?;
    app.wait_for("text:Instant 240")?;
    assert!(!app.shows("Rest 240 · finished"));
    Ok(())
}

#[test]
fn the_calm_preference_swaps_the_springs_without_the_system_setting() -> Result<(), String> {
    let mut app = start();
    app.press("text:Calm motion on this page")?;
    app.press("text:Play to 240")?;
    app.wait_for("text:Instant 240")?;
    Ok(())
}
