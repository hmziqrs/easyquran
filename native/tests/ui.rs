//! The UI library's headless tests: the gallery renders its sections and
//! the key wrappers react, on a manual clock, through the UI only.

use std::time::{Duration, SystemTime};

use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Points, Root, Size};
use nsh_starter::ui::gallery::ui_gallery_page;
use nsh_testing::Headless;

/// The gallery as a root: the page the shell mounts, alone in a view.
#[component]
fn GalleryRoot() -> impl IntoElement {
    view().flex_1().child(ui_gallery_page())
}

fn start() -> Headless {
    Headless::start(EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<GalleryRoot>())
    })
    .unwrap()
}

/// Scrolls the gallery down by `points`, as a user flicks, so the next
/// section's controls are in the viewport (a press hit-tests in the root).
fn flick(app: &mut Headless, points: f32) -> Result<(), String> {
    use nsh_core::geometry::px;
    app.scroll("id:gallery-scroll", px(points))
}

/// The gallery as a desktop: pointer idioms get right-click context menus.
fn start_as_desktop() -> Headless {
    use nsh_core::platform::{Idiom, Os};
    Headless::start(EngineConfig {
        environment: nsh_engine::Environment {
            os: Os::MacOs,
            idiom: Idiom::Desktop,
            ..nsh_engine::Environment::default()
        },
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<GalleryRoot>())
    })
    .unwrap()
}

fn start_at_desktop_width() -> Headless {
    Headless::start(EngineConfig {
        viewport: Size::new(Points(1200.0), Points(800.0)),
        clock: Clock::manual(SystemTime::UNIX_EPOCH),
        ..EngineConfig::new(Root::of::<GalleryRoot>())
    })
    .unwrap()
}

#[test]
fn the_gallery_sections_render() -> Result<(), String> {
    let mut app = start();
    for section in [
        "Buttons",
        "Fields",
        "Controls",
        "Choices from a list",
        "Pickers",
        "Overlays",
        "Data views",
        "Surfaces",
    ] {
        app.wait_for(&format!("text:{section}"))?;
    }
    Ok(())
}

#[test]
fn the_native_controls_fall_back_to_the_apps_own() -> Result<(), String> {
    // Headless is an iOS phone on a macOS host: no UIKit, so the section
    // draws the app's controls and they drive the same state.
    let mut app = start();
    app.wait_for("text:Native presses: 0")?;
    app.press("role:button[Continue]")?;
    app.wait_for("text:Native presses: 1")?;
    app.press("role:button[Send]")?;
    app.wait_for("text:Native presses: 2")?;
    app.wait_for("text:Airplane mode off · range Week")?;
    app.press("role:switch[Airplane mode]")?;
    app.press("role:button[Month]")?;
    app.wait_for("text:Airplane mode on · range Month")?;
    Ok(())
}

#[test]
fn a_press_flips_the_switch() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Wi-Fi is on")?;
    flick(&mut app, 1100.0)?;
    app.press("role:switch[Wi-Fi]")?;
    app.wait_for("text:Wi-Fi is off")?;
    app.press("role:switch[Wi-Fi]")?;
    app.wait_for("text:Wi-Fi is on")?;
    Ok(())
}

#[test]
fn tabs_move_the_selection() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Showing Overview")?;
    app.press("role:tab[Activity]")?;
    app.wait_for("text:Showing Activity")?;
    app.press("role:tab[Logs]")?;
    app.wait_for("text:Showing Logs")?;
    Ok(())
}

#[test]
fn the_radio_group_picks_one() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:2 of 2 selected, plan Standard")?;
    flick(&mut app, 1200.0)?;
    app.press("role:radio[Pro]")?;
    app.wait_for("text:2 of 2 selected, plan Pro")?;
    Ok(())
}

#[test]
fn the_tri_state_checkbox_selects_all_or_none() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:2 of 2 selected, plan Standard")?;
    flick(&mut app, 1200.0)?;
    app.press("role:checkbox[Select all]")?;
    app.wait_for("text:0 of 2 selected, plan Standard")?;
    Ok(())
}

#[test]
fn the_dialog_card_opens_and_cancels() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Overlays")?;
    flick(&mut app, 2400.0)?;
    app.press("role:button[Open dialog]")?;
    app.wait_for("text:Delete the draft?")?;
    app.press("role:button[Cancel]")?;
    app.wait_for_gone("text:Delete the draft?", Duration::from_secs(1))?;
    Ok(())
}

#[test]
fn the_dialog_card_confirms_the_deletion() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Overlays")?;
    flick(&mut app, 2400.0)?;
    app.press("role:button[Open dialog]")?;
    app.wait_for("text:Delete the draft?")?;
    app.press("role:button[Delete]")?;
    app.wait_for_gone("text:Delete the draft?", Duration::from_secs(1))?;
    // The confirm enabled the destructive action of the overlays section.
    app.wait_for("text:Delete draft…")?;
    Ok(())
}

#[test]
fn a_notify_shows_a_toast_that_times_out() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Overlays")?;
    flick(&mut app, 2400.0)?;
    app.press("role:button[Notify]")?;
    app.run_for(Duration::from_millis(300))?;
    app.wait_for("text:Copied to the clipboard")?;
    // The toast lives 5 s on the manual clock; nothing moves but the test.
    app.run_for(Duration::from_secs(6))?;
    app.wait_for_gone("text:Copied to the clipboard", Duration::from_secs(1))?;
    Ok(())
}

#[test]
fn pagination_moves_the_page() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Page 2 of 12")?;
    flick(&mut app, 4200.0)?;
    app.press("role:button[3]")?;
    app.wait_for("text:Page 3 of 12")?;
    Ok(())
}

#[test]
fn typing_reaches_the_text_field() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Display name")?;
    app.type_text("text:Ada Lovelace", "Grace")?;
    app.wait_until(
        "the typed name reaches the caption",
        Duration::from_secs(2),
        |screen| screen.texts().iter().any(|text| text.starts_with("Grace ")),
    )?;
    Ok(())
}

#[test]
fn the_page_sheet_opens_and_closes_on_back() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Overlays")?;
    flick(&mut app, 2400.0)?;
    app.press("role:button[Open page sheet]")?;
    app.wait_for("text:Invite a teammate")?;
    app.back()?;
    app.wait_for_gone("text:Invite a teammate", Duration::from_secs(1))?;
    Ok(())
}

#[test]
fn the_context_menu_opens_on_a_right_click() -> Result<(), String> {
    let mut app = start_as_desktop();
    app.wait_for("text:Right-click (or hold) this card for its commands.")?;
    flick(&mut app, 2400.0)?;
    app.right_click("text:Right-click (or hold) this card for its commands.")?;
    app.wait_for("text:Duplicate card")?;
    Ok(())
}

#[test]
fn the_full_dialog_closes_from_its_own_button() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Overlays")?;
    flick(&mut app, 2400.0)?;
    app.press("role:button[Open full dialog]")?;
    app.wait_for("text:Review required")?;
    app.press("role:button[Done]")?;
    app.wait_for_gone("text:Review required", Duration::from_secs(1))?;
    Ok(())
}

#[test]
fn a_nav_row_press_selects_it() -> Result<(), String> {
    let mut app = start();
    app.wait_for("text:Navigation rows")?;
    app.wait_for("text:Home row selected")?;
    app.press("id:nav-inbox")?;
    app.wait_for("text:Inbox row selected")?;
    app.press("id:nav-reports")?;
    app.wait_for("text:Reports row selected")?;
    Ok(())
}

#[test]
fn the_gallery_renders_at_desktop_width() -> Result<(), String> {
    let mut app = start_at_desktop_width();
    for section in ["Buttons", "Controls", "Overlays", "Surfaces", "Data views"] {
        app.wait_for(&format!("text:{section}"))?;
    }
    // The heading is the phone header's large title; the summary is the page's.
    app.wait_for("text:/^Every control of the app's own library/")?;
    Ok(())
}
