//! The forms feature's tests: every rule quiet until the first submit,
//! then following edits, the username check's debounce, server errors on their field,
//! draft autosave and restore, the dirty guard, the keyboard flow and the
//! settings form's load and save. Everything runs on a manual clock, with
//! the fake server's latency moved by `run_for`, and no network.
//!
//! Break seen — each test was failed once on purpose with the break named
//! here, then the code restored:
//!
//! | test | break seen |
//! |---|---|
//! | `errors_wait_for_submit_then_follow_edits` | `SubmitRule::check` registered its rule with `.rule` (change and blur too) |
//! | `a_failed_submit_scrolls_the_first_failing_row_into_view` | `fields.rs`: `RowReveal::provide` registered no `on_submit_invalid` hook |
//! | `the_username_check_debounces_and_drops_stale_answers` | `USERNAME_DEBOUNCE` raised to 10 s |
//! | `a_taken_username_reports_after_the_check` | `username_taken` answered false always — a mutation shared with the next test; each was seen failing under it |
//! | `a_taken_answer_shows_while_the_field_keeps_focus` | the shared mutation above, and on its own: the hint fell back to "Available" instead of the mirrored answer |
//! | `submitting_focuses_the_first_invalid_field` | the submit button's `on_press` did nothing |
//! | `a_server_error_lands_on_its_field_and_clears_on_edit` | the rejected code returned `SubmitError::form`, not `::field` |
//! | `submitting_creates_the_account` | `fake_server` always returned `Err` |
//! | `the_draft_autosaves_and_restores` | three, one per assertion: the draft restore skipped (fresh-app email empty); `from_draft` misread the stored date format, so the restored trigger stayed empty while the stored JSON still held `2026-10-14`; `password` kept in the draft (exclusion assertion) |
//! | `the_seeded_draft_restores_the_pickers` | `from_draft` left both pickers empty (the seeded restore was skipped) |
//! | `the_dirty_guard_blocks_back_until_discarded` | `pop_on_back(true)` constant |
//! | `settings_loads_saves_and_guards` | the settings `form.load` skipped |

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use nsh_base::{nav_page, nav_stack, theme};
use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_starter::features::forms::forms_page;
use nsh_testing::{DialogAnswer, Headless, MemoryStorage};

/// A wall time inside the calendar's range, for the date picker.
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_188_800)
}

const DRAFT_KEY: &str = "forms.sign_up.draft";
const SETTINGS_KEY: &str = "forms.settings";
const USERNAME_ANSWER: Duration = Duration::from_millis(750);
const SERVER_ANSWER: Duration = Duration::from_millis(900);

/// What the shell will mount: the feature's page inside a stack, over a
/// home page, so a back press past the feature pops the outer page.
#[derive(Debug, Clone, PartialEq)]
enum Route {
    Home,
    Forms,
}

#[component]
fn Harness() -> impl IntoElement {
    let routes = signal(vec![Route::Home, Route::Forms]);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(nav_stack().flex_1().child(each(
            move || routes.get(),
            |route: &Route| format!("{route:?}"),
            move |route, index| {
                let page = match route.get_untracked() {
                    Route::Home => nav_page().title("Home").child(text("Nothing here yet")),
                    Route::Forms => nav_page().title("Forms").child(forms_page()),
                };
                page.on_dismiss(move || {
                    routes.update(|r| r.truncate(index.get_untracked()));
                })
            },
        )))
}

fn start(storage: Arc<MemoryStorage>) -> Headless {
    let config = EngineConfig {
        clock: Clock::manual(now()),
        storage: Some(storage),
        ..EngineConfig::new(Root::of::<Harness>())
    };
    let mut app = Headless::start(config).unwrap();
    // `start` can return before the first frame; wait for something first.
    app.first_render().unwrap();
    app.wait_for("Create account").unwrap();
    app
}

/// A fresh app whose mount batches reach `observe` before the first render.
fn start_observing(
    storage: Arc<MemoryStorage>,
    observe: impl FnMut(&str, &nsh_core::MountBatch) + Send + 'static,
) -> Headless {
    let config = EngineConfig {
        clock: Clock::manual(now()),
        storage: Some(storage),
        ..EngineConfig::new(Root::of::<Harness>())
    };
    let mut app = Headless::start(config).unwrap();
    app.observe(observe);
    app.first_render().unwrap();
    app.wait_for("Create account").unwrap();
    app
}

fn seeded_storage(entries: &[(&str, &str)]) -> Arc<MemoryStorage> {
    MemoryStorage::new(
        entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string())),
    )
}

/// Fills every required field validly, with `code` as the verification
/// code, and lets the username check land.
fn fill_valid(app: &mut Headless, code: &str) {
    app.press("text:Pick a date").unwrap();
    app.press("label:2026-10-14").unwrap();
    app.type_text("id:signup-email", "ada@example.com").unwrap();
    app.type_text("id:signup-password", "correct horse")
        .unwrap();
    app.type_text("id:signup-confirm", "correct horse").unwrap();
    app.type_text("id:signup-username", "ada2").unwrap();
    app.type_text("id:signup-bio", "Rustacean.").unwrap();
    app.run_for(USERNAME_ANSWER).unwrap();
    app.press("text:Pro").unwrap();
    app.press("text:I accept the terms").unwrap();
    app.type_text("id:signup-code", code).unwrap();
    // The one-time code's field holds the focus and the headless driver
    // keeps delivering presses to it; a normal field takes it back.
    app.clear("id:signup-email").unwrap();
    app.type_text("id:signup-email", "ada@example.com").unwrap();
}

#[test]
fn errors_wait_for_submit_then_follow_edits() {
    let mut app = start(seeded_storage(&[]));
    app.type_text("id:signup-email", "not-an-email").unwrap();
    // Leaving the field is not a reason to scold: only a submit is.
    app.type_text("id:signup-password", "correct horse")
        .unwrap();
    assert!(
        !app.shows("That is not an email"),
        "the email rule spoke before the first submit"
    );
    app.press("text:Create account").unwrap();
    app.wait_for("That is not an email").unwrap();
    // After a submit the rule follows each edit.
    app.clear("id:signup-email").unwrap();
    app.type_text("id:signup-email", "ada@example.com").unwrap();
    app.wait_for_gone("That is not an email", Duration::from_secs(1))
        .unwrap();
}

#[test]
fn a_failed_submit_scrolls_the_first_failing_row_into_view() {
    let mut app = start(seeded_storage(&[]));
    app.wait_for("id:signup-email").unwrap();
    let before = scroll_tos(app.screen());
    // The empty form fails; the focus reveal alone would leave the
    // Email label under the header, so the page scrolls the row itself.
    app.press("text:Create account").unwrap();
    app.wait_until(
        "the page scrolled to the failing row",
        Duration::from_secs(1),
        |screen| scroll_tos(screen) > before,
    )
    .unwrap();
}

/// How many `ScrollTo` commands the app has sent so far.
fn scroll_tos(screen: &nsh_testing::Screen) -> usize {
    screen
        .commands()
        .iter()
        .filter(|(_, command)| matches!(command, nsh_core::Command::ScrollTo { .. }))
        .count()
}

#[test]
fn the_username_check_debounces_and_drops_stale_answers() {
    let mut app = start(seeded_storage(&[]));
    app.type_text("id:signup-username", "ada").unwrap();
    app.run_for(Duration::from_millis(200)).unwrap();
    assert!(
        !app.shows("That username is taken"),
        "the check answered before its debounce and latency"
    );
    // A newer edit drops the older answer, which never lands.
    app.clear("id:signup-username").unwrap();
    app.type_text("id:signup-username", "ada2").unwrap();
    app.run_for(USERNAME_ANSWER).unwrap();
    assert!(!app.shows("That username is taken"));
    app.wait_for("Available").unwrap();
}

#[test]
fn a_taken_username_reports_after_the_check() {
    let mut app = start(seeded_storage(&[]));
    app.type_text("id:signup-username", "grace").unwrap();
    app.wait_for("Checking…").unwrap();
    app.run_for(USERNAME_ANSWER).unwrap();
    // The live hint answers as the user types; no submit needed.
    app.wait_for("That username is taken").unwrap();
}

#[test]
fn submitting_focuses_the_first_invalid_field() {
    // Inputs are not press targets, so the email input is known by the
    // mount item its placeholder names, watched from the first render.
    let email: Arc<Mutex<Option<nsh_core::NodeId>>> = Arc::new(Mutex::new(None));
    let seen = email.clone();
    let mut app = start_observing(seeded_storage(&[]), move |_, batch| {
        for item in &batch.items {
            if let nsh_core::MountItem::Input { id, config } = item
                && config.placeholder == "ada@example.com"
            {
                *seen.lock().unwrap() = Some(*id);
            }
        }
    });
    app.press("text:Create account").unwrap();
    app.wait_for("Enter your email").unwrap();
    let focused = app
        .screen()
        .commands()
        .iter()
        .rev()
        .find(|(_, command)| matches!(command, nsh_core::Command::Focus))
        .map(|(id, _)| *id);
    let email = *email.lock().unwrap();
    assert!(email.is_some(), "the email input was never seen mounting");
    assert_eq!(
        focused, email,
        "the failed submit did not focus the first invalid field"
    );
}

#[test]
fn a_taken_answer_shows_while_the_field_keeps_focus() {
    let mut app = start(seeded_storage(&[]));
    app.type_text("id:signup-username", "grace").unwrap();
    // The raw answer lands while the field still holds the focus; the
    // field's own error waits for blur, the hint must not say Available.
    app.wait_for("That username is taken").unwrap();
    assert!(
        !app.shows("Available"),
        "a taken username was offered as available: {:?}",
        app.screen().texts()
    );
}

#[test]
fn a_server_error_lands_on_its_field_and_clears_on_edit() {
    let mut app = start(seeded_storage(&[]));
    fill_valid(&mut app, "000000");
    app.press("text:Create account").unwrap();
    app.wait_for("Contacting the fake server…").unwrap();
    app.run_for(SERVER_ANSWER).unwrap();
    app.wait_for("That code was rejected. Any six digits work except 000000.")
        .unwrap();
    // Editing the field clears what the server gave it.
    app.clear("id:signup-code").unwrap();
    app.type_text("id:signup-code", "123456").unwrap();
    app.wait_for_gone(
        "That code was rejected. Any six digits work except 000000.",
        Duration::from_secs(2),
    )
    .unwrap();
}

#[test]
fn submitting_creates_the_account() {
    let mut app = start(seeded_storage(&[]));
    fill_valid(&mut app, "123456");
    app.press("text:Create account").unwrap();
    app.run_for(SERVER_ANSWER).unwrap();
    app.wait_for("Account created for ada@example.com").unwrap();
}

/// A draft with the date and time filled, for the picker round-trip.
const SEEDED_DRAFT: &str = r#"{"email":"","username":"","phone":"","referral":"","birth_date":"2011-11-11","meeting_time":"09:30","region":null,"interests":[],"plan":null,"terms":false,"newsletter":false,"age":21.0,"favorite_color":"","budget":null,"bio":""}"#;

#[test]
fn the_draft_autosaves_and_restores() {
    let storage = seeded_storage(&[]);
    {
        let mut app = start(storage.clone());
        app.press("text:Pick a date").unwrap();
        app.press("label:2026-10-14").unwrap();
        app.type_text("id:signup-email", "ada@example.com").unwrap();
        app.type_text("id:signup-password", "correct horse")
            .unwrap();
        app.wait_until("the draft reached storage", Duration::from_secs(2), |_| {
            storage
                .entries()
                .get(DRAFT_KEY)
                .is_some_and(|json| json.contains("ada@example.com") && json.contains("2026-10-14"))
        })
        .unwrap();
        // Credentials are not a draft's business.
        let json = storage.entries().get(DRAFT_KEY).cloned().unwrap();
        assert!(
            !json.contains("correct horse"),
            "the draft stored the password: {json}"
        );
    }
    let app = start(storage);
    let restored = app.screen().input_text("id:signup-email").unwrap();
    assert_eq!(
        restored,
        Some("ada@example.com"),
        "the draft did not come back in a fresh app"
    );
    assert!(
        app.shows("2026/10/14"),
        "the picked birth date did not come back"
    );
}

#[test]
fn the_seeded_draft_restores_the_pickers() {
    let mut app = start(seeded_storage(&[(DRAFT_KEY, SEEDED_DRAFT)]));
    app.wait_for("2011/11/11").unwrap();
    assert!(
        app.shows("09") && app.shows("30"),
        "the seeded meeting time did not come back: {:?}",
        app.screen().texts()
    );
}

#[test]
fn the_dirty_guard_blocks_back_until_discarded() {
    let storage = seeded_storage(&[]);
    let mut app = start(storage.clone());
    app.type_text("id:signup-email", "ada@example.com").unwrap();
    app.on_dialog(|dialog| {
        assert_eq!(dialog.title.as_deref(), Some("Discard this draft?"));
        DialogAnswer::Press("Cancel".to_owned())
    });
    app.back().unwrap();
    assert!(app.shows("Create account"), "back left the dirty page");
    app.on_dialog(|_| DialogAnswer::Press("Discard draft".to_owned()));
    app.back().unwrap();
    assert_eq!(
        app.screen().input_text("id:signup-email").unwrap(),
        Some(""),
        "discard kept the typed email"
    );
    assert!(
        !storage.entries().contains_key(DRAFT_KEY),
        "discard left a draft behind: {:?}",
        storage.entries().get(DRAFT_KEY)
    );
    // Clean now, the same back pops, and the feature's stack empties.
    app.back().unwrap();
    app.wait_for_gone("Create account", Duration::from_secs(2))
        .unwrap();
    assert!(
        app.shows("Nothing here yet"),
        "back did not reach the page below the feature"
    );
}

#[test]
fn settings_loads_saves_and_guards() {
    let saved = r#"{"display_name":"Ada","start_page":"feed","editor_style":"markdown","notifications":true,"weekly_digest":false,"font_scale":1.0}"#;
    let storage = seeded_storage(&[(SETTINGS_KEY, saved)]);
    let mut app = start(storage.clone());
    app.press("text:App settings").unwrap();
    app.wait_for("Save settings").unwrap();
    assert_eq!(
        app.screen().input_text("id:settings-name").unwrap(),
        Some("Ada"),
        "form.load did not put the saved settings on the form"
    );
    app.type_text("id:settings-name", "Ada Lovelace").unwrap();
    app.press("text:Save settings").unwrap();
    app.run_for(SERVER_ANSWER).unwrap();
    app.wait_for("Saved").unwrap();
    let json = storage.entries().get(SETTINGS_KEY).cloned().unwrap();
    assert!(
        json.contains("Ada Lovelace"),
        "the save did not reach storage: {json}"
    );
    // Saved, so back pops without asking; then a dirty guard asks.
    app.type_text("id:settings-name", "Ada K").unwrap();
    app.on_dialog(|dialog| {
        assert_eq!(dialog.title.as_deref(), Some("Discard your changes?"));
        DialogAnswer::Press("Cancel".to_owned())
    });
    app.back().unwrap();
    assert!(app.shows("Save settings"), "back left the dirty settings");
}
