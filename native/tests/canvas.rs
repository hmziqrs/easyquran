//! The canvas area's tests, on a manual clock: the responder drag leaves
//! a stroke and undo removes it, the drawing survives a re-layout, the
//! plots redraw on new data, and the frame loop advances only while the
//! manual clock runs and its pause holds. The shader and the web view
//! tests cover what headless can see: that the WGSL compiled and the
//! page's Rust side of the messaging is wired (no page runs headless).

use std::time::Duration;

use nsh_core::{Size, prelude::*};
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_starter::features::canvas::canvas_page;
use nsh_testing::Headless;

/// The page alone: no shell, so selectors name only this area's views.
#[component]
fn CanvasRoot() -> impl IntoElement {
    canvas_page()
}

/// Tall enough that every section is on screen at once; a manual clock,
/// so the frame loop moves only when a test runs it.
fn app() -> Headless {
    Headless::start(EngineConfig {
        viewport: Size::new(px(390.0), px(3200.0)),
        clock: Clock::manual(std::time::SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<CanvasRoot>())
    })
    .expect("the engine starts")
}

/// The tick the frame loop shows, if it is on screen.
fn tick_of(app: &mut Headless) -> Option<u32> {
    app.screen()
        .texts()
        .into_iter()
        .find_map(|t| t.strip_prefix("tick ")?.parse().ok())
}

#[test]
fn a_dragged_stroke_stays_and_undo_removes_it() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^0 strokes/")?;
    app.drag("id:sketch", point(px(90.0), px(50.0)))?;
    app.wait_for("text:/^1 strokes/")?;
    app.press("text:Undo")?;
    app.wait_for("text:/^0 strokes/")?;
    Ok(())
}

#[test]
fn the_drawing_survives_a_re_layout() -> Result<(), String> {
    let mut app = app();
    // `start` can return before the first frame, so wait for one first.
    app.wait_for("text:Sketch")?;
    app.drag("id:sketch", point(px(40.0), px(30.0)))?;
    app.wait_for("text:/^1 strokes/")?;
    // A wider window lays the canvas out again; the strokes are data, so
    // they redraw in the new width.
    app.set_viewport(Size::new(px(760.0), px(3200.0)))?;
    app.wait_for("text:/^1 strokes/")?;
    assert_eq!(app.screen().count("text:/^1 strokes/")?, 1);
    Ok(())
}

#[test]
fn the_charts_redraw_when_the_data_changes() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^batch #7 ·/")?;
    app.press("text:New data")?;
    app.wait_for("text:/^batch #8 ·/")?;
    Ok(())
}

#[test]
fn the_frame_loop_advances_on_the_manual_clock_and_a_pause_holds_it() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^tick /")?;
    // Pause, reset, play: the effect restarts the interval, so the ticks
    // that follow come from this moment on the manual clock.
    app.press("id:sim-toggle >> text:Pause")?;
    app.press("id:frame-loop-reset >> text:Reset")?;
    app.wait_for("text:tick 0")?;
    app.press("id:sim-toggle >> text:Play")?;
    app.run_for(Duration::from_millis(1000))?;
    let played = tick_of(&mut app).unwrap_or_else(|| panic!("the tick readout is on screen"));
    assert!(played >= 25, "a second of clock ticks the loop: {played}");
    app.press("id:sim-toggle >> text:Pause")?;
    app.run_for(Duration::from_millis(1000))?;
    let paused = tick_of(&mut app).unwrap_or_else(|| panic!("the tick readout is on screen"));
    assert_eq!(paused, played, "a paused loop holds its tick");
    Ok(())
}

#[test]
fn the_shader_compiles_and_its_children_draw_over_it() -> Result<(), String> {
    let mut app = app();
    // The caption is the shader's child: it mounts only over a compiled
    // program (a failed compile takes the fallback branch, whose text
    // differs). Headless draws no pixels; this proves the WGSL validated.
    app.wait_for("text:Aurora — WGSL over this box")?;
    assert!(!app.shows("Shader unavailable"));
    app.press("id:shader-pause >> text:Pause")?;
    app.wait_for("text:Resume")?;
    Ok(())
}

#[test]
fn posting_to_the_web_view_page_records_the_message() -> Result<(), String> {
    let mut app = app();
    // No page runs headless, so nothing comes back from it; the app's own
    // side of the exchange is what a headless run can see.
    app.wait_for("text:Post to the page")?;
    app.press("id:post-to-page >> text:Post to the page")?;
    app.wait_for("text:hi #1")?;
    Ok(())
}

#[test]
fn the_world_board_pans_zooms_and_selects_a_node_at_zoom() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^world 100% · pan 0, 0$/")?;
    // A drag on the empty board pans: the readout's pan follows the drag.
    app.drag("id:world-board", point(px(60.0), px(0.0)))?;
    app.wait_for("text:/^world 100% · pan 60, 0$/")?;
    // Zoom in: anchored at the view's center, so the pan scales by the
    // same ratio as the zoom.
    app.press("text:Zoom in")?;
    app.wait_for("text:/^world 125% · pan 75, 0$/")?;
    // The hit test runs in world space: the viewport center's world point
    // is 20, 0 — on Notes — and the readout names both exactly.
    app.drag("id:world-board", point(px(-100.0), px(0.0)))?;
    app.wait_for("text:/^center 20, 0 · under: Notes$/")?;
    // A further zoom keeps the node under the center: the same world
    // point, at a different zoom.
    app.press("text:Zoom in")?;
    app.wait_for("text:/^world 156% · pan -31, 0$/")?;
    app.wait_for("text:/^center 20, 0 · under: Notes$/")?;
    // A drag from the center selects Notes and moves it in world units:
    // 30 screen points at 156% is 19 world points.
    app.drag("id:world-board", point(px(30.0), px(0.0)))?;
    app.wait_for("text:/^selected: Notes · 89, 0$/")?;
    Ok(())
}

/// The peers caption's tick, if it is on screen. It looks at the peers
/// caption alone: the particle field's readout carries a tick too.
fn peer_tick_of(app: &mut Headless) -> Option<u32> {
    app.screen().texts().into_iter().find_map(|t| {
        if !t.contains("demo peers") {
            return None;
        }
        let ix = t.rfind("· tick ")?;
        t[ix + "· tick ".len()..].parse().ok()
    })
}

/// The particle field's tick: the readout leads with the dot count, so it
/// never collides with the frame loop's own tick readout.
fn field_tick_of(app: &mut Headless) -> Option<u32> {
    app.screen()
        .texts()
        .into_iter()
        .find_map(|t| t.strip_prefix("48 dots · tick ")?.parse().ok())
}

#[test]
fn the_shape_editor_selects_moves_and_resizes() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:selected bounds: none")?;
    // Every drag presses the canvas center; there it is on the rect's
    // interior, so the first drag selects and moves.
    app.drag("id:shape-edit", point(px(40.0), px(0.0)))?;
    app.wait_for("text:selected bounds: 54, 90 · 150 × 84")?;
    app.drag("id:shape-edit", point(px(0.0), px(18.0)))?;
    app.wait_for("text:selected bounds: 54, 108 · 150 × 84")?;
    // This drag parks the east handle on the center, so the next press
    // grabs the handle instead of the interior and grows the width.
    app.drag("id:shape-edit", point(px(-40.0), px(0.0)))?;
    app.wait_for("text:selected bounds: 14, 108 · 150 × 84")?;
    app.drag("id:shape-edit", point(px(40.0), px(0.0)))?;
    app.wait_for("text:selected bounds: 14, 108 · 190 × 84")?;
    app.press("id:shape-edit-reset >> text:Reset")?;
    app.wait_for("text:selected bounds: none")?;
    Ok(())
}

#[test]
fn the_bezier_editor_drags_a_control_point_and_reset_restores_it() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:c1 164, 120 · c2 250, 190")?;
    // The center press lands on c1 itself, and a grab sets the point to
    // the pointer, so the drag leaves c1 at press + delta.
    app.drag("id:bezier", point(px(30.0), px(-20.0)))?;
    app.wait_for("text:c1 194, 100 · c2 250, 190")?;
    app.press("id:bezier-reset >> text:Reset")?;
    app.wait_for("text:c1 164, 120 · c2 250, 190")?;
    Ok(())
}

#[test]
fn the_node_graph_moves_a_node_and_marquee_selects_two() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:0 selected")?;
    // The center press is on Input: the drag selects and moves it, and
    // the readout's first node follows.
    app.drag("id:node-graph", point(px(30.0), px(10.0)))?;
    app.wait_for("text:1 selected")?;
    app.wait_for("text:first node 146, 130")?;
    // This drag moves Input clear of the center, so the next press has
    // empty space under it and starts a marquee instead.
    app.drag("id:node-graph", point(px(-100.0), px(40.0)))?;
    app.wait_for("text:first node 46, 170")?;
    // A marquee from the center to the upper right crosses Filter and
    // Merge but neither of the others.
    app.drag("id:node-graph", point(px(180.0), px(-80.0)))?;
    app.wait_for("text:2 selected")?;
    app.press("id:node-graph-reset >> text:Reset")?;
    app.wait_for("text:0 selected")?;
    app.wait_for("text:first node 116, 120")?;
    Ok(())
}

#[test]
fn the_particle_field_advances_on_the_clock_and_a_pause_holds_it() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^48 dots · tick /")?;
    // Pause, reset, play: the effect restarts the interval, so the ticks
    // that follow come from this moment on the manual clock.
    app.press("id:particles-toggle >> text:Pause")?;
    app.press("id:particles-reset >> text:Reset")?;
    app.wait_for("text:48 dots · tick 0")?;
    app.press("id:particles-toggle >> text:Play")?;
    app.run_for(Duration::from_millis(1000))?;
    let played =
        field_tick_of(&mut app).unwrap_or_else(|| panic!("the field readout is on screen"));
    assert!(played >= 25, "a second of clock ticks the field: {played}");
    app.press("id:particles-toggle >> text:Pause")?;
    app.run_for(Duration::from_millis(1000))?;
    let paused =
        field_tick_of(&mut app).unwrap_or_else(|| panic!("the field readout is on screen"));
    assert_eq!(paused, played, "a paused field holds its tick");
    Ok(())
}

#[test]
fn the_text_and_image_specimens_draw_and_report_their_canvas() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:glyphs shape through the engine; the rules mark their baselines")?;
    // The image note ends in the canvas size, so the numbers also prove
    // the layout callback ran.
    app.wait_for("text:/^128 × 64 drawn at 1×, 2× and as a right-half crop · canvas \\d+ × 280$/")?;
    Ok(())
}

#[test]
fn the_shared_board_peers_ride_the_clock_and_a_drag_selects_a_card() -> Result<(), String> {
    let mut app = app();
    app.wait_for("text:/^2 demo peers orbit the board; peer 1 nudges its card · tick /")?;
    // The center press is on Plan: the drag selects it and moves it.
    app.drag("id:collab-board", point(px(25.0), px(10.0)))?;
    app.wait_for("text:selected: Plan")?;
    // The peers ride the interval on the manual clock, so the caption's
    // tick advances with no input at all.
    app.run_for(Duration::from_millis(2000))?;
    let moved = peer_tick_of(&mut app).unwrap_or_else(|| panic!("the peers readout is on screen"));
    assert!(moved >= 30, "two seconds of clock move the peers: {moved}");
    Ok(())
}
