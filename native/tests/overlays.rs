//! The overlays page, on a manual clock: dialogs close by Escape and the
//! scrim and return focus to the trigger; alerts answer a real flow; the
//! sheet adapts to the width; the popups open and close; toasts queue and
//! dismiss on the clock; a dock drag lands in the layout data.
//!
//! Every test was seen to fail once by breaking the code on purpose (the
//! "Break seen" column); the break was restored before this file was left.
//!
//! | test | break seen |
//! | --- | --- |
//! | the_dialog_closes_on_escape_and_returns_focus | the `on_cancel` escape branch made a no-op |
//! | the_dialog_keeps_escape_out_when_the_prop_says_so | the switch bound to a fresh `signal(true)` instead of `escape_closes` |
//! | the_dialog_closes_on_a_scrim_press | the scrim's `on_press` body emptied |
//! | the_alert_dialog_deletes_the_draft_from_the_flow | the `Some("delete")` answer rewired to the "kept" path |
//! | the_action_sheet_reports_the_cancel_answer | the dismissed answer rewired to "Kept the draft" |
//! | the_sheet_opens_as_a_side_panel_on_desktop_width | the width check replaced by a constant `Placement::Bottom` |
//! | the_sheet_opens_as_a_bottom_sheet_on_phone_width | "Tall" wrote detent 1 instead of 2; and the tall marker gated on `detent == 3` |
//! | the_popover_closes_on_a_press_outside | `on_open_change` stopped writing `open` (controlled popovers never open) |
//! | the_menu_selects_items_and_toggles_the_checkable_one | Rename's `on_select` no-op; then the checkable item stopped toggling `line_numbers` |
//! | the_menu_opens_as_a_sheet_on_a_phone | Rename's `on_select` no-op (same closure as the desktop menu) |
//! | the_context_menu_opens_on_a_right_click | Copy's `on_select` no-op |
//! | the_hover_card_opens_on_a_hover | the card stopped following open requests |
//! | the_tooltip_opens_from_the_app_on_touch | the toggle forced `shown = false` |
//! | toasts_queue_in_order_and_dismiss_on_the_clock | push ids offset so FIFO order broke; then the 4-second timeout removed |
//! | the_dock_drag_lands_in_the_layout_data | `on_layout_change` stopped writing `layout`; then Reset made a no-op |
//! | the_resizable_panel_steps_from_the_keyboard_path | "Wider" clamped to 200 (no movement) |

use std::time::Duration;

/// Scrolls the page until `selector` is on screen, a viewport at a time;
/// the page's lower sections sit below the fold of a window-sized view.
fn reveal(app: &mut Headless, selector: &str) {
    for _ in 0..12 {
        if app
            .wait_for_within(selector, Duration::from_millis(120))
            .is_ok()
        {
            return;
        }
        app.scroll("text:Overlays", Points(600.0)).unwrap();
    }
    panic!("{selector} never came on screen");
}

use nsh_core::{Points, geometry::point};
use nsh_engine::{EngineConfig, Root};
use nsh_starter::features::overlays::OverlaysTestRoot;
use nsh_testing::{DialogAnswer, Headless};

/// A desktop-sized surface at `width`, on the manual clock.
fn desktop(width: f32) -> Headless {
    let mut config = EngineConfig::new(Root::of::<OverlaysTestRoot>());
    config.viewport = nsh_core::Size::new(Points(width), Points(700.0));
    config.environment.os = nsh_core::platform::Os::MacOs;
    config.environment.idiom = nsh_core::platform::Idiom::Desktop;
    config.clock = nsh_core::time::Clock::manual(std::time::SystemTime::UNIX_EPOCH);
    Headless::start(config).unwrap()
}

/// A phone-sized surface, on the manual clock.
fn phone() -> Headless {
    let mut config = EngineConfig::new(Root::of::<OverlaysTestRoot>());
    config.clock = nsh_core::time::Clock::manual(std::time::SystemTime::UNIX_EPOCH);
    Headless::start(config).unwrap()
}

#[test]
fn the_dialog_closes_on_escape_and_returns_focus() {
    let mut app = desktop(900.0);
    app.wait_for("role:button[Rename file…]").unwrap();
    app.press("role:button[Rename file…]").unwrap();
    app.wait_for("text:Rename file").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for("text:Closed with Escape").unwrap();
    app.wait_for_gone("text:Rename file", Duration::from_secs(2))
        .unwrap();
    // The app restores the keyboard focus to the trigger.
    app.wait_for("text:Focus: rename trigger").unwrap();
}

#[test]
fn the_dialog_keeps_escape_out_when_the_prop_says_so() {
    let mut app = desktop(900.0);
    app.wait_for("role:button[Rename file…]").unwrap();
    // The switch's label is its content text.
    app.press("role:switch[Escape closes]").unwrap();
    app.press("role:button[Rename file…]").unwrap();
    app.wait_for("text:Rename file").unwrap();
    app.press_key("escape").unwrap();
    app.wait_for("text:Escape ignored (close_on_escape is off)")
        .unwrap();
    // Still open; closing it for the next assertion is unnecessary, the
    // app ends here.
}

#[test]
fn the_dialog_closes_on_a_scrim_press() {
    let mut app = desktop(900.0);
    app.wait_for("role:button[Rename file…]").unwrap();
    app.press("role:button[Rename file…]").unwrap();
    app.wait_for("text:Rename file").unwrap();
    // Since 0.5.0 a press lands where the view is drawn: the scrim's
    // center is under the dialog, so press its uncovered corner.
    app.press_at("label:Dismiss", point(Points(8.0), Points(8.0)))
        .unwrap();
    app.wait_for("text:Closed with the scrim").unwrap();
    app.wait_for_gone("text:Rename file", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_alert_dialog_deletes_the_draft_from_the_flow() {
    let mut app = desktop(900.0);
    // The test answers the system alert itself.
    app.answer_dialogs_myself();
    app.wait_for("role:button[Delete draft…]").unwrap();
    app.press("role:button[Delete draft…]").unwrap();
    app.wait_until("the alert to answer", Duration::from_secs(2), |screen| {
        !screen.dialogs().is_empty()
    })
    .unwrap();
    app.answer_dialog(DialogAnswer::Press("Delete draft".to_owned()))
        .unwrap();
    app.wait_for("text:Deleted the draft").unwrap();
    app.wait_for("text:Draft: deleted").unwrap();
    // With no draft left, the destructive entry disables.
    assert!(!app.screen().enabled("role:button[Delete draft…]").unwrap());
}

#[test]
fn the_action_sheet_reports_the_cancel_answer() {
    let mut app = desktop(900.0);
    app.answer_dialogs_myself();
    app.wait_for("role:button[Share options…]").unwrap();
    app.press("role:button[Share options…]").unwrap();
    app.wait_until("the sheet to answer", Duration::from_secs(2), |screen| {
        !screen.dialogs().is_empty()
    })
    .unwrap();
    app.answer_dialog(DialogAnswer::Dismiss).unwrap();
    app.wait_for("text:Dismissed without a choice").unwrap();
}

#[test]
fn the_sheet_opens_as_a_side_panel_on_desktop_width() {
    let mut app = desktop(900.0);
    reveal(&mut app, "role:button[Notification settings…]");
    app.press("role:button[Notification settings…]").unwrap();
    // The panel itself draws on the overlay layer, which the device phase
    // captures; headless sees the flow's state.
    app.wait_for("text:Opened as a right panel").unwrap();
    // The panel draws on the overlay layer, which the device phase
    // captures; headless sees the flow's state only.
}

#[test]
fn the_sheet_opens_as_a_bottom_sheet_on_phone_width() {
    let mut app = phone();
    reveal(&mut app, "role:button[Notification settings…]");
    // The detents are the section's data; the panel's popup style follows,
    // and the tall detent alone shows the extra rows. The page's detent
    // buttons sit under the open sheet's scrim (0.5.0 presses where a view
    // is drawn), so each detent is picked while the sheet is closed.
    app.press("role:button[Tall]").unwrap();
    app.wait_for("text:Detent step 2 of 0–2").unwrap();
    app.press("role:button[Notification settings…]").unwrap();
    app.wait_for("text:Opened as a bottom panel").unwrap();
    app.wait_for("id:sheet-tall-marker").unwrap();
    app.press("role:button[Done]").unwrap();
    app.wait_for("text:Closed (Done)").unwrap();
    app.press("role:button[Short]").unwrap();
    app.wait_for("text:Detent step 0 of 0–2").unwrap();
    app.press("role:button[Notification settings…]").unwrap();
    app.wait_for("text:Opened as a bottom panel").unwrap();
    app.wait_for("text:Done").unwrap();
    app.wait_for_gone("id:sheet-tall-marker", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_popover_closes_on_a_press_outside() {
    let mut app = desktop(900.0);
    reveal(&mut app, "id:share-trigger");
    app.press("id:share-trigger").unwrap();
    app.wait_for("text:opened (trigger)").unwrap();
    app.wait_for("text:Share “Q3 roadmap”").unwrap();
    // A press outside closes it and goes on: the copy chip's press fires.
    app.press("role:button[Copy link]").unwrap();
    app.wait_for("text:closed (press outside)").unwrap();
    app.wait_for("text:Link copied").unwrap();
    app.wait_for_gone("text:Share “Q3 roadmap”", Duration::from_secs(2))
        .unwrap();
}

#[test]
fn the_menu_selects_items_and_toggles_the_checkable_one() {
    let mut app = desktop(900.0);
    reveal(&mut app, "id:actions-trigger");
    app.press("id:actions-trigger").unwrap();
    app.wait_for("role:menuitem[Rename]").unwrap();
    app.press("role:menuitem[Rename]").unwrap();
    app.wait_for("text:Renamed").unwrap();
    app.wait_for_gone("role:menuitem[Rename]", Duration::from_secs(2))
        .unwrap();
    app.press("id:actions-trigger").unwrap();
    app.wait_for("role:menuitemcheckbox[Show line numbers]")
        .unwrap();
    app.wait_for("text:Line numbers: on").unwrap();
    app.press("role:menuitemcheckbox[Show line numbers]")
        .unwrap();
    app.wait_for("text:Line numbers: off").unwrap();
}

#[test]
fn the_context_menu_opens_on_a_right_click() {
    let mut app = desktop(900.0);
    reveal(&mut app, "id:file-card");
    app.right_click("id:file-card").unwrap();
    app.wait_for("role:menuitem[Copy]").unwrap();
    app.press("role:menuitem[Copy]").unwrap();
    app.wait_for("text:Copied").unwrap();
}

#[test]
fn the_menu_opens_as_a_sheet_on_a_phone() {
    let mut app = phone();
    reveal(&mut app, "id:actions-trigger");
    app.press("id:actions-trigger").unwrap();
    app.wait_for("role:menuitem[Rename]").unwrap();
    // On a phone the menu is a sheet at the bottom, with its items.
    app.press("role:menuitem[Rename]").unwrap();
    app.wait_for("text:Renamed").unwrap();
}

#[test]
fn the_hover_card_opens_on_a_hover() {
    let mut app = desktop(900.0);
    reveal(&mut app, "id:hover-card-trigger");
    app.hover("id:hover-card-trigger").unwrap();
    // The card opens after its 150 ms delay, on the manual clock.
    app.run_for(Duration::from_millis(400)).unwrap();
    app.wait_for("text:opened (hover)").unwrap();
    app.wait_for("role:button[Follow]").unwrap();
    app.press("role:button[Follow]").unwrap();
    app.wait_for("text:Following Mira Chen").unwrap();
}

#[test]
fn the_tooltip_opens_from_the_app_on_touch() {
    let mut app = desktop(900.0);
    reveal(&mut app, "role:button[Toggle hint]");
    app.press("role:button[Toggle hint]").unwrap();
    app.wait_for("text:Hint shown").unwrap();
    app.wait_for("text:Saves the draft (⌘S)").unwrap();
    // The toggle drives the same value the pointer requests, so it closes.
    app.press("role:button[Toggle hint]").unwrap();
    app.wait_for("text:Hint hidden").unwrap();
}

#[test]
fn toasts_queue_in_order_and_dismiss_on_the_clock() {
    let mut app = desktop(900.0);
    reveal(&mut app, "role:button[Save the draft]");
    app.press("role:button[Save the draft]").unwrap();
    app.press("role:button[Sync now]").unwrap();
    app.press("role:button[Announce]").unwrap();
    // Oldest first, newest last.
    app.wait_for("text:Queue: Draft saved → Sync failed → Invitation sent")
        .unwrap();
    app.wait_for("text:Draft saved").unwrap();
    // The saved toast's 4 s timeout runs on the manual clock; its exit
    // transition keeps it mounted 200 ms more.
    app.run_for(Duration::from_millis(4_400)).unwrap();
    // The announce toast's 3 s timeout is gone with it; only the sync
    // toast, which has no timeout, remains.
    app.wait_for("text:Queue: Sync failed").unwrap();
    app.wait_for_gone("text:Draft saved", Duration::from_secs(2))
        .unwrap();
    // The sync toast's action dismisses it.
    app.press("role:button[Retry]").unwrap();
    app.wait_for("text:Queue: (empty)").unwrap();
}

#[test]
fn the_dock_drag_lands_in_the_layout_data() {
    let mut app = desktop(900.0);
    // Reveal the card's bottom dock, so the whole area is on screen.
    reveal(&mut app, "id:tab-terminal");
    app.wait_for("text:Layout: row([Readme, main.rs] [Preview])")
        .unwrap();
    // Drag the Preview tab onto the first group, below its tab bar.
    app.drag("id:tab-preview", point(Points(-130.0), Points(140.0)))
        .unwrap();
    app.wait_for("text:Layout: row([Readme, main.rs, Preview])")
        .unwrap();
    // The layout is the app's data: Reset writes the initial one back.
    app.press("role:button[Reset layout]").unwrap();
    app.wait_for("text:Layout: row([Readme, main.rs] [Preview])")
        .unwrap();
}

#[test]
fn the_resizable_panel_steps_from_the_keyboard_path() {
    let mut app = desktop(900.0);
    reveal(&mut app, "role:button[Wider]");
    app.press("role:button[Wider]").unwrap();
    app.wait_for("text:First panel at 240 pt").unwrap();
    app.press("role:button[Narrower]").unwrap();
    app.wait_for("text:First panel at 200 pt").unwrap();
}
