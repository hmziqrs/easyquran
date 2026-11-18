//! The sign-up page: rules, an async username check with debounce, server
//! errors landing on fields by path, draft autosave to `Storage`, the
//! dirty guard on `pop_on_back`, the keyboard flow and autofill hints, and
//! one of every control. Everything a server would do here is fake and
//! in-process; nothing touches the network.

use std::time::Duration;

use nsh_base::rich_editor::view::{RichEditor, RichEditorApi, RichEditorChange, rich_editor};
use nsh_base::{
    ColorChannel, ColorSpace, DatePickerValue, DateTime, PickedDate, SelectEntry, SelectItem,
    SliderValue, TimeOfDay, checkbox, color_area, color_picker, color_slider, color_swatch,
    combobox, date_picker, nav_page, nav_stack, number_input, otp_input, radio, radio_group,
    select, slider, switch, theme, time_field,
};
use nsh_core::{
    element::{
        AutoCapitalize, AutoFill, KeyboardDismissMode, KeyboardTaps, KeyboardType, ReturnKeyType,
        Screen,
    },
    prelude::*,
    storage::Storage,
};
use nsh_form::{BindField, Field, FieldValidators, Form, SubmitError, field, rules};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::features::forms::fields::{
    RowReveal, SubmitRule, error_line, field_label, field_row, field_section, hint_line,
    input_shell, message_banner, page_header, revealed_row, submit_first_form,
};
use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use crate::ui::controls::progress_indeterminate;
use crate::ui::dialog::confirm_destructive;
use crate::ui::popups::{popup_backdrop, popup_surface, popup_trigger_frame};
use crate::ui::text;

/// The page scroll's top content inset, in points.
const SCROLL_TOP_INSET: f32 = 8.0;

/// The usernames the fake availability check reports as taken.
const TAKEN_USERNAMES: [&str; 4] = ["ada", "admin", "grace", "lovelace"];

/// The pause after the last keystroke before the check runs.
const USERNAME_DEBOUNCE: Duration = Duration::from_millis(400);

/// How long the fake availability check thinks.
const USERNAME_CHECK_LATENCY: Duration = Duration::from_millis(300);

/// How long the fake sign-up server thinks.
pub(crate) const SERVER_LATENCY: Duration = Duration::from_millis(800);

/// How long the fake code check thinks.
const CODE_CHECK_LATENCY: Duration = Duration::from_millis(500);

/// The draft's key in `Storage`; a fresh app restores it on mount.
const DRAFT_KEY: &str = "forms.sign_up.draft";

/// The sign-up's values.
#[derive(Clone, PartialEq)]
struct SignUp {
    email: String,
    password: String,
    confirm_password: String,
    username: String,
    phone: String,
    referral: String,
    birth_date: DateTime,
    meeting_time: Option<TimeOfDay>,
    region: Option<String>,
    interests: Vec<String>,
    plan: Option<String>,
    terms: bool,
    newsletter: bool,
    age: f64,
    favorite_color: String,
    verification_code: String,
    budget: Option<f64>,
    bio: String,
}

impl Default for SignUp {
    fn default() -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            username: String::new(),
            phone: String::new(),
            referral: String::new(),
            birth_date: DateTime::default(),
            meeting_time: None,
            region: None,
            interests: Vec::new(),
            plan: None,
            terms: false,
            newsletter: false,
            age: 21.0,
            favorite_color: String::new(),
            verification_code: String::new(),
            budget: None,
            bio: String::new(),
        }
    }
}

/// The draft as `Storage` keeps it: every non-secret value, the pickers as
/// ISO text. `password`, `confirm_password` and `verification_code` are
/// deliberately left out — a template does not save credentials.
#[derive(Default, Serialize, Deserialize)]
struct SignUpDraft {
    email: String,
    username: String,
    phone: String,
    referral: String,
    /// `"YYYY-MM-DD"`, as the date picker's value round-trips.
    birth_date: Option<String>,
    /// `"HH:MM"`, as the time field's value round-trips.
    meeting_time: Option<String>,
    region: Option<String>,
    interests: Vec<String>,
    plan: Option<String>,
    terms: bool,
    newsletter: bool,
    age: f64,
    favorite_color: String,
    budget: Option<f64>,
    bio: String,
}

impl SignUpDraft {
    fn of(values: &SignUp) -> Self {
        Self {
            email: values.email.clone(),
            username: values.username.clone(),
            phone: values.phone.clone(),
            referral: values.referral.clone(),
            birth_date: values
                .birth_date
                .start()
                .map(|day| day.format("%Y-%m-%d").to_string()),
            meeting_time: values
                .meeting_time
                .map(|time| time.0.format("%H:%M").to_string()),
            region: values.region.clone(),
            interests: values.interests.clone(),
            plan: values.plan.clone(),
            terms: values.terms,
            newsletter: values.newsletter,
            age: values.age,
            favorite_color: values.favorite_color.clone(),
            budget: values.budget,
            bio: values.bio.clone(),
        }
    }
}

impl SignUp {
    fn from_draft(draft: SignUpDraft) -> Self {
        Self {
            email: draft.email,
            username: draft.username,
            phone: draft.phone,
            referral: draft.referral,
            birth_date: draft
                .birth_date
                .and_then(|text| chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d").ok())
                .and_then(|day| day.and_hms_opt(0, 0, 0))
                .map(|day| DateTime::Single(Some(day)))
                .unwrap_or_default(),
            meeting_time: draft
                .meeting_time
                .and_then(|text| chrono::NaiveTime::parse_from_str(&text, "%H:%M").ok())
                .map(TimeOfDay),
            region: draft.region,
            interests: draft.interests,
            plan: draft.plan,
            terms: draft.terms,
            newsletter: draft.newsletter,
            age: draft.age,
            favorite_color: draft.favorite_color,
            budget: draft.budget,
            bio: draft.bio,
            ..Self::default()
        }
    }
}

/// The feature's inner routes; the stack is `sign_up_page`'s.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FormsRoute {
    SignUp,
    Settings,
}

/// The feature's stack: the sign-up at the bottom, settings pushed.
pub(crate) fn sign_up_page() -> impl IntoElement {
    let routes = signal(vec![FormsRoute::SignUp]);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(nav_stack().flex_1().child(each(
            move || routes.get(),
            |route: &FormsRoute| format!("{route:?}"),
            move |route, index| {
                let page = match route.get_untracked() {
                    FormsRoute::SignUp => sign_up_screen(routes),
                    FormsRoute::Settings => {
                        crate::features::forms::settings::settings_screen(routes)
                    }
                };
                page.on_dismiss(move || {
                    routes.update(|r| r.truncate(index.get_untracked()));
                })
            },
        )))
}

/// A bound text field row: the app's look, the field's binding and error.
fn text_field_row(
    label: &'static str,
    field: Field<SignUp, String>,
    input: nsh_base::Input,
) -> View {
    field_row(label, field.bind(input), move || field.error())
}

/// The username row. The hint mirrors the live check's answer as the user
/// types; the field's error waits for a submit like every other rule.
fn username_row(
    username: Field<SignUp, String>,
    answer: Signal<String>,
    input: nsh_base::Input,
) -> View {
    let hint = move || {
        if username.error().is_some() {
            // The field's error line says it, in red, below the input.
            SharedString::from("")
        } else if !answer.get().is_empty() {
            answer.get().into()
        } else if username.value().chars().count() < 3 {
            SharedString::from("")
        } else {
            // The live check is between the debounce and its answer.
            SharedString::from("Checking…")
        }
    };
    revealed_row(
        view()
            .gap(px(6.0))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(field_label("Username"))
                    .child(hint_line(hint)),
            )
            .child(username.bind(input))
            .child(error_line(move || username.error())),
        move || username.error().is_some(),
    )
}

fn sign_up_screen(routes: Signal<Vec<FormsRoute>>) -> Screen {
    let form = submit_first_form(SignUp::default());
    // The draft restores before anything reads the form; `load` makes the
    // restored values the baseline too, so a restored draft is not dirty.
    if let Some(draft) = Storage::get_item(DRAFT_KEY)
        .and_then(|json| serde_json::from_str::<SignUpDraft>(&json).ok())
    {
        form.load(SignUp::from_draft(draft));
    }

    let email = field!(form, email)
        .check(rules::required("Enter your email"))
        .check(rules::email("That is not an email"));
    let password = field!(form, password)
        .check(rules::required("Choose a password"))
        .check(rules::min_length(8, "Use at least 8 characters"));
    let confirm_password = field!(form, confirm_password).check(rules::matches(
        |values: &SignUp| &values.password,
        "Passwords do not match",
    ));
    let username = field!(form, username)
        .check(rules::required("Pick a username"))
        .check(rules::min_length(3, "Use at least 3 characters"))
        .validators(
            FieldValidators::new()
                .on_dynamic_async(check_username)
                .on_dynamic_async_debounce(USERNAME_DEBOUNCE),
        );
    let phone = field!(form, phone).check(phone_rule());
    let referral =
        field!(form, referral).check(rules::unsigned_number::<u32, SignUp>("Use digits only"));
    let birth_date = field!(form, birth_date).check(|date: &DateTime, _: &SignUp| {
        (!date.start().is_some()).then(|| "Pick your birth date".to_owned())
    });
    let meeting_time = field!(form, meeting_time);
    let region = field!(form, region);
    let interests = field!(form, interests);
    let plan = field!(form, plan).check(rules::required("Choose a plan"));
    let newsletter = field!(form, newsletter);
    let age = field!(form, age);
    let favorite_color = field!(form, favorite_color);
    let budget = field!(form, budget).check(budget_rule());
    let verification_code = field!(form, verification_code)
        .check(rules::required("Enter the 6 digits"))
        .check(otp_rule());
    let bio = field!(form, bio).check(|bio: &String, _: &SignUp| {
        (!doc_has_text(bio)).then(|| "Write a line or two about you".to_owned())
    });
    let terms = field!(form, terms).check(|terms: &bool, _: &SignUp| {
        (!terms).then(|| "Accept the terms to continue".to_owned())
    });

    // What the fake code check said about the code as it was completed.
    let code_check = signal(String::new());
    // What the availability check answered for the name as it stands —
    // raw, unlike the field's error, which waits for the first submit.
    let username_answer = signal(String::new());
    let username_seen = stored(0_u64);
    effect(move || {
        let name = username.value();
        let seen = username_seen.with(|seen| {
            *seen += 1;
            *seen
        });
        // A new edit clears the last answer and drops an older one's write.
        username_answer.set(String::new());
        if name.chars().count() < 3 {
            return;
        }
        let answer = username_answer;
        spawn(async move {
            timer(USERNAME_DEBOUNCE + USERNAME_CHECK_LATENCY).await;
            if username_seen.with(|seen| *seen) == seen {
                answer.set(if username_taken(&name) {
                    "That username is taken".to_owned()
                } else {
                    "Available".to_owned()
                });
            }
        });
    });
    // The email the fake server accepted, for the success banner.
    let created = signal(None::<String>);

    // Draft autosave: an actual edit lands in `Storage` as it happens, and
    // a clean form (fresh page, Discard) writes nothing.
    effect(move || {
        if !form.is_dirty() {
            return;
        }
        let draft = SignUpDraft::of(&form.values());
        if let Ok(json) = serde_json::to_string(&draft) {
            let _ = Storage::set_item(DRAFT_KEY, json);
        }
    });

    // A failed submit scrolls the first failing row's label into view.
    let reveal = RowReveal::provide(form, SCROLL_TOP_INSET);

    let submit = move || form.submit(move |values: SignUp| fake_server(values, created));

    // The bio editor reads its first document once, before the body ends.
    let bio_api = component_ref::<RichEditor>();
    let initial_bio = serde_json::from_str::<Value>(&bio.value_untracked()).ok();

    let account = field_section(
        "Account",
        [
            text_field_row(
                "Email",
                email,
                input_shell(move || email.error().is_some())
                    .placeholder("ada@example.com")
                    .keyboard_type(KeyboardType::Email)
                    .auto_capitalize(AutoCapitalize::None)
                    .auto_correct(false)
                    .auto_fill(AutoFill::Email)
                    .return_key_type(ReturnKeyType::Done)
                    .on_submit(move |_: &str| submit())
                    .test_id("signup-email"),
            ),
            text_field_row(
                "Password",
                password,
                input_shell(move || password.error().is_some())
                    .placeholder("At least 8 characters")
                    .masked(true)
                    .auto_fill(AutoFill::NewPassword)
                    .return_key_type(ReturnKeyType::Done)
                    .on_submit(move |_: &str| submit())
                    .test_id("signup-password"),
            ),
            text_field_row(
                "Confirm password",
                confirm_password,
                input_shell(move || confirm_password.error().is_some())
                    .placeholder("Type the password again")
                    .masked(true)
                    .auto_fill(AutoFill::NewPassword)
                    .return_key_type(ReturnKeyType::Done)
                    .on_submit(move |_: &str| submit())
                    .test_id("signup-confirm"),
            ),
            username_row(
                username,
                username_answer,
                input_shell(move || username.error().is_some())
                    .placeholder("Lowercase letters and digits")
                    .auto_capitalize(AutoCapitalize::None)
                    .auto_correct(false)
                    .auto_fill(AutoFill::Username)
                    .return_key_type(ReturnKeyType::Done)
                    .on_submit(move |_: &str| submit())
                    .test_id("signup-username"),
            ),
            phone_row(phone),
            text_field_row(
                "Referral code",
                referral,
                input_shell(move || referral.error().is_some())
                    .placeholder("Who sent you here")
                    .keyboard_type(KeyboardType::Number)
                    .auto_fill(AutoFill::Off)
                    .return_key_type(ReturnKeyType::Done)
                    .test_id("signup-referral"),
            ),
        ],
    );

    let profile = field_section(
        "Profile",
        [
            date_row(birth_date),
            time_row(meeting_time),
            region_row(region),
            interests_row(interests),
            plan_row(plan),
            age_row(age),
        ],
    );

    let preferences = field_section(
        "Preferences",
        [
            switch_row(newsletter),
            color_row(favorite_color),
            budget_row(budget),
        ],
    );

    let verify = field_section(
        "Verify",
        [
            code_row(verification_code, code_check),
            bio_row(bio, bio_api, initial_bio),
        ],
    );

    let consent = view()
        .gap_3()
        .child(terms_row(terms))
        .child(submit_button(form, created))
        .child(when(
            move || form.is_submitting(),
            move || {
                view()
                    .gap_2()
                    .child(progress_indeterminate())
                    .child(text::muted("Contacting the fake server…"))
            },
        ))
        .child(when(
            move || form.error().is_some(),
            move || {
                message_banner(
                    move || form.error().unwrap_or_default().into(),
                    move || theme().destructive,
                )
            },
        ))
        .child(when(
            move || created.get().is_some(),
            move || {
                let email = created.get().unwrap_or_default();
                message_banner(
                    move || format!("Account created for {email}").into(),
                    move || theme().primary,
                )
            },
        ))
        .child(button_with(
            "App settings",
            ButtonVariant::Ghost,
            ButtonSize::Medium,
            false,
            None,
            move || open_settings(routes),
        ));

    sign_up_nav(form)
        .child(
            reveal
                .scroll(scroll())
                .flex_1()
                // nav_page paints the platform's color; the page takes the theme's.
                .bg(move || theme().background)
                // The focused field's label keeps clear of the page header when
                // the reveal scrolls it to the top.
                .content_inset(crate::ui::layout::page_insets(SCROLL_TOP_INSET))
                .keyboard_dismiss_mode(KeyboardDismissMode::OnDrag)
                .keyboard_taps(KeyboardTaps::Handled)
                .test_id("forms-scroll")
                .child(
                    reveal.content(
                        view()
                            .p_4()
                            .pb_safe_area_plus(px(24.0))
                            .gap_4()
                            .child(account)
                            .child(profile)
                            .child(preferences)
                            .child(verify)
                            .child(consent),
                    ),
                ),
        )
        // The same soft header and tab-bar edges as the shell's pages.
        .child(crate::shell::soft_edges::page_edges())
}

/// The screen's page furniture: title, drawn header, the dirty guard.
fn sign_up_nav(form: Form<SignUp>) -> Screen {
    crate::ui::nav::themed_header(nav_page())
        .title("Sign up")
        .header(page_header("Sign up", None::<fn()>))
        .pop_on_back(move || !form.is_dirty())
        .on_back_press(move || {
            // Discard clears the draft; the next back goes through, the
            // form now clean.
            if form.is_dirty() {
                confirm_discard(form);
            }
        })
}

fn phone_row(phone: Field<SignUp, String>) -> View {
    text_field_row(
        "Phone",
        phone,
        input_shell(move || phone.error().is_some())
            .placeholder("+1 555 0100")
            .keyboard_type(KeyboardType::Phone)
            .auto_capitalize(AutoCapitalize::None)
            .auto_fill(AutoFill::Tel)
            .return_key_type(ReturnKeyType::Done)
            .test_id("signup-phone"),
    )
}

fn date_row(birth_date: Field<SignUp, DateTime>) -> View {
    field_row(
        "Birth date",
        view().test_id("signup-birth-date").child(
            date_picker()
                .value(move || DatePickerValue::Controlled(birth_date.value()))
                .on_change(move |picked: &PickedDate| birth_date.set(picked.value))
                .accessibility_label("Birth date")
                .offset(px(4.0))
                .popup_style(popup_surface)
                .backdrop_style(popup_backdrop)
                .trigger(popup_trigger_frame(
                    move || birth_text(birth_date.value()).to_string(),
                    true,
                )),
        ),
        move || birth_date.error(),
    )
}

/// The picked date as the trigger shows it, or the prompt.
fn birth_text(date: DateTime) -> SharedString {
    date.start()
        .map(|day| day.format("%Y/%m/%d").to_string())
        .unwrap_or_else(|| "Pick a date".to_owned())
        .into()
}

/// The time field wears the input frame itself (border, background and
/// segment states), as the ui pickers' own wrapper does.
fn time_row(meeting_time: Field<SignUp, Option<TimeOfDay>>) -> View {
    field_row(
        "Meeting time",
        view().test_id("signup-time").child(
            time_field()
                .value(move || meeting_time.value())
                .on_change(move |time: &TimeOfDay| {
                    meeting_time.set(Some(*time));
                    meeting_time.blur();
                })
                .accessibility_label("Meeting time")
                .flex_row()
                .items_center()
                .min_h(px(44.0))
                .px_2()
                .gap_1()
                .rounded(move || theme().radius)
                .border_1()
                .border_color(move || theme().input)
                .bg(move || theme().background)
                .segment_class(
                    "min-w-7 px-1 py-1.5 rounded-sm text-sm text-popover-foreground text-center \
                     hover:bg-accent focus:bg-accent",
                )
                .separator_style(|s| s.text_sm().text_color(move || theme().muted_foreground)),
        ),
        move || meeting_time.error(),
    )
}

fn region_row(region: Field<SignUp, Option<String>>) -> View {
    const REGIONS: [(&str, &str); 5] = [
        ("eu", "Europe"),
        ("na", "North America"),
        ("apac", "Asia-Pacific"),
        ("sa", "South America"),
        ("amea", "Africa & Middle East"),
    ];
    let title = |value: Option<String>| {
        value
            .and_then(|value| {
                REGIONS
                    .iter()
                    .find(|(key, _)| *key == value)
                    .map(|(_, title)| *title)
            })
            .unwrap_or("Pick a region")
    };
    field_row(
        "Region",
        view().test_id("signup-region").child(
            select()
                .items(
                    REGIONS
                        .iter()
                        .map(|(value, title)| SelectItem::new(*value, *title))
                        .collect::<Vec<SelectItem>>(),
                )
                .value(move || region.value().map(SharedString::from))
                .on_change(move |value: &Option<SharedString>| {
                    region.set(value.as_deref().map(str::to_owned));
                    region.blur();
                })
                .accessibility_label("Region")
                .offset(px(4.0))
                .popup_style(popup_surface)
                .backdrop_style(popup_backdrop)
                .item_class("rounded-md focus:bg-accent selected:font-medium disabled:opacity-50")
                .render_item(|item: &SelectEntry| {
                    view().px_2().py_1p5().child(
                        text(item.title.clone())
                            .text_sm()
                            .text_color(move || theme().popover_foreground),
                    )
                })
                .trigger(popup_trigger_frame(
                    move || title(region.value()).to_string(),
                    true,
                )),
        ),
        move || region.error(),
    )
}

fn interests_row(interests: Field<SignUp, Vec<String>>) -> View {
    field_row(
        "Interests",
        view().test_id("signup-interests").child(
            combobox()
                .items(vec![
                    SelectItem::new("cycling", "Cycling"),
                    SelectItem::new("photography", "Photography"),
                    SelectItem::new("cooking", "Cooking"),
                    SelectItem::new("climbing", "Climbing"),
                    SelectItem::new("board-games", "Board games"),
                    SelectItem::new("astronomy", "Astronomy"),
                ])
                .multiple(true)
                .value(move || {
                    Some(
                        interests
                            .value()
                            .into_iter()
                            .map(SharedString::from)
                            .collect::<Vec<_>>(),
                    )
                })
                .on_change(move |values: &Vec<SharedString>| {
                    interests.set(values.iter().map(|value| value.to_string()).collect());
                    interests.blur();
                })
                .placeholder("Type to filter")
                .accessibility_label("Interests")
                .popup_style(|s| {
                    s.p_1()
                        .rounded_lg()
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().popover)
                        .shadow_md()
                })
                .item_class("rounded-md focus:bg-accent selected:font-medium disabled:opacity-50")
                .input_style(|s| {
                    s.h(px(44.0))
                        .px(px(12.0))
                        .text_size(px(15.0))
                        .rounded(move || theme().radius)
                        .border_1()
                        .border_color(move || theme().input)
                        .bg(move || theme().background)
                }),
        ),
        move || interests.error(),
    )
}

fn plan_row(plan: Field<SignUp, Option<String>>) -> View {
    let option = |value: &'static str, label: &'static str| {
        let selected = move || plan.value().as_deref() == Some(value);
        radio()
            .checked(selected)
            .on_change(move |_: &bool| {
                plan.set(Some(value.to_owned()));
                plan.blur();
            })
            .accessibility_label(label)
            .flex_row()
            .items_center()
            .gap_3()
            .min_h(px(44.0))
            .child(
                view()
                    .size(px(20.0))
                    .rounded_full()
                    .border_2()
                    .border_color(move || {
                        if selected() {
                            theme().primary
                        } else {
                            theme().border
                        }
                    })
                    .items_center()
                    .justify_center()
                    .child(
                        view()
                            .size(px(8.0))
                            .rounded_full()
                            .style_when(selected, |s| s.bg(move || theme().primary)),
                    ),
            )
            .child(text(label).text_size(px(15.0)))
    };
    field_row(
        "Plan",
        radio_group()
            .flex_row()
            .gap_1()
            .child(option("free", "Free"))
            .child(option("pro", "Pro"))
            .child(option("team", "Team")),
        move || plan.error(),
    )
}

fn age_row(age: Field<SignUp, f64>) -> View {
    field_row(
        "Age",
        view()
            .gap_1()
            .child(hint_line(move || {
                format!("{} years", age.value().round() as u32).into()
            }))
            .child(
                slider()
                    .value(move || Some(SliderValue::Single(age.value())))
                    .min(13.0)
                    .max(99.0)
                    .on_change(move |value: &SliderValue| {
                        age.set(value.start());
                        age.blur();
                    })
                    .accessibility_label("Age")
                    .track_class("h-6 justify-center")
                    .indicator_class("h-1.5 rounded-full bg-muted")
                    .range_class("rounded-full bg-primary")
                    .thumb_class("size-5 rounded-full border-2 border-primary bg-background")
                    .flex_1(),
            ),
        move || age.error(),
    )
}

fn switch_row(newsletter: Field<SignUp, bool>) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .min_h(px(44.0))
        .child(text("Send me the newsletter").text_size(px(15.0)))
        .child(
            switch()
                .checked(move || newsletter.value())
                .on_change(move |on: &bool| {
                    newsletter.set(*on);
                    newsletter.blur();
                })
                .accessibility_label("Newsletter")
                .track_style(|s| {
                    s.w(px(44.0))
                        .p(px(2.0))
                        .rounded_full()
                        .bg(move || theme().muted_foreground.opacity(0.3))
                        .transition_colors()
                        .checked(|s| s.bg(move || theme().primary))
                })
                .thumb_style(|s| {
                    s.size(px(20.0))
                        .rounded_full()
                        .bg(move || theme().background)
                        .shadow_sm()
                        .transition_transform()
                        .checked(|s| s.translate_x(px(20.0)))
                }),
        )
}

fn color_row(favorite_color: Field<SignUp, String>) -> View {
    // Swatch presets are the picker's data, not the app's look.
    const SWATCHES: [&str; 5] = ["#0ea5e9", "#22c55e", "#eab308", "#ef4444", "#a855f7"];
    field_row(
        "Favorite color",
        view()
            .gap_2()
            .child(
                color_picker()
                    .value(move || Hsla::parse_hex(&favorite_color.value()).ok())
                    .on_change(move |event: &nsh_base::ColorPickerEvent| {
                        if let nsh_base::ColorPickerEvent::Change(_, formatted) = event {
                            favorite_color.set(formatted.to_string());
                            favorite_color.blur();
                        }
                    })
                    .gap_2()
                    .child(
                        color_area()
                            .flex_1()
                            .h(px(120.0))
                            .accessibility_label("Color area")
                            .panel_class("rounded-md")
                            .thumb_class("size-4 rounded-full border-2 border-ring"),
                    )
                    .child(
                        color_slider()
                            .h(px(24.0))
                            .color_space(ColorSpace::Hsb)
                            .channel(ColorChannel::Hue)
                            .accessibility_label("Hue")
                            .track_class("h-3 rounded-full")
                            .thumb_class("size-5 rounded-full border-2 border-ring"),
                    ),
            )
            .child(view().flex_row().gap_2().children(SWATCHES.map(|hex| {
                color_swatch()
                    .color(Hsla::parse_hex(hex).ok())
                    .accessibility_label(hex)
                    .selected_style(|s| s.border_2().border_color(move || theme().foreground))
                    .size(px(28.0))
                    .rounded_full()
            })))
            .child(hint_line(move || favorite_color.value().into())),
        move || favorite_color.error(),
    )
}

fn budget_row(budget: Field<SignUp, Option<f64>>) -> View {
    field_row(
        "Daily budget",
        view().test_id("signup-budget").child(
            number_input()
                .value(move || budget.value())
                .min(0.0)
                .max(10_000.0)
                .step(50.0)
                .placeholder("0")
                .controls_right(true)
                .on_change(move |value: &Option<f64>| {
                    budget.set(*value);
                    budget.blur();
                })
                .on_blur(move || budget.blur())
                .accessibility_label("Daily budget")
                .input_style(|s| {
                    s.h(px(44.0))
                        .px(px(12.0))
                        .text_size(px(15.0))
                        .border_1()
                        .border_color(move || theme().input)
                        .rounded(move || theme().radius)
                        .bg(move || theme().background)
                }),
        ),
        move || budget.error(),
    )
}

fn code_row(code: Field<SignUp, String>, code_check: Signal<String>) -> View {
    let check = code_check;
    let field = code;
    view()
        .gap_1()
        .child(field_row(
            "Verification code",
            view().test_id("signup-code").child(
                otp_input()
                    .length(6)
                    .groups(2)
                    .value(move || Some(SharedString::from(field.value())))
                    .on_change(move |value: &SharedString| field.set(value.to_string()))
                    .on_complete(move |_: &nsh_base::OtpInputCompleteEvent| {
                        // A complete code is done being typed: put the
                        // keyboard away, or it covers the next control.
                        dismiss_keyboard();
                        check.set("Checking the code…".into());
                        let entered = field.value_untracked();
                        spawn(async move {
                            timer(CODE_CHECK_LATENCY).await;
                            check.set(if entered == "000000" {
                                "That code was rejected".to_owned()
                            } else {
                                "Code looks good".to_owned()
                            });
                        });
                    })
                    .on_blur(move || field.blur())
                    .accessibility_label("Verification code")
                    .group_class("flex-row gap-2")
                    .cell_class(
                        "size-11 items-center justify-center rounded-md border border-input \
                         text-base focus:border-ring data-filled:border-primary",
                    ),
            ),
            move || code.error(),
        ))
        .child(hint_line(move || check.get().into()))
}

fn bio_row(
    bio: Field<SignUp, String>,
    editor_api: ComponentRef<RichEditor>,
    initial: Option<Value>,
) -> View {
    let api = editor_api;
    let field = bio;
    field_row(
        "Short bio",
        view()
            .h(px(140.0))
            .p_2()
            .rounded(move || theme().radius)
            .border_1()
            .border_color(move || {
                if bio.error().is_some() {
                    theme().destructive
                } else {
                    theme().input
                }
            })
            .bg(move || theme().background)
            .test_id("signup-bio")
            .child(
                rich_editor()
                    .component_ref(&api)
                    .content(initial)
                    .placeholder("A few words about you")
                    .on_change(move |_: &RichEditorChange| {
                        if let Some(api) = api.api::<RichEditorApi>() {
                            field.set(api.doc_json().to_string());
                            field.blur();
                        }
                    })
                    .on_blur(move || field.blur())
                    .accessibility_label("Short bio")
                    .flex_1(),
            ),
        move || bio.error(),
    )
}

fn terms_row(terms: Field<SignUp, bool>) -> View {
    view()
        .gap_1()
        .child(
            checkbox()
                .checked(move || terms.value())
                .on_change(move |checked: &bool| {
                    terms.set(*checked);
                    terms.blur();
                })
                .accessibility_label("Accept the terms")
                .flex_row()
                .items_center()
                .gap_3()
                .indicator_style(|s| {
                    s.size(px(20.0))
                        .items_center()
                        .justify_center()
                        .rounded(move || theme().radius)
                        .border_2()
                        .border_color(move || theme().border)
                        .checked(|s| {
                            s.bg(move || theme().primary)
                                .border_color(move || theme().primary)
                        })
                })
                .indicator(
                    text("✓")
                        .text_size(px(13.0))
                        .text_color(move || theme().primary_foreground),
                )
                .child(text("I accept the terms").text_size(px(15.0))),
        )
        .child(error_line(move || terms.error()))
}

fn submit_button(form: Form<SignUp>, created: Signal<Option<String>>) -> impl IntoElement {
    view().test_id("signup-submit").child(button_with(
        move || {
            if form.is_submitting() {
                SharedString::from("Creating account…")
            } else {
                SharedString::from("Create account")
            }
        },
        ButtonVariant::Primary,
        ButtonSize::Large,
        move || form.is_submitting(),
        None,
        move || form.submit(move |values: SignUp| fake_server(values, created)),
    ))
}

/// The fake sign-up server: latency, then a field error, else success.
async fn fake_server(values: SignUp, created: Signal<Option<String>>) -> Result<(), SubmitError> {
    timer(SERVER_LATENCY).await;
    if values.verification_code == "000000" {
        return Err(SubmitError::field(
            "verification_code",
            "That code was rejected. Any six digits work except 000000.",
        ));
    }
    if username_taken(&values.username) {
        return Err(SubmitError::field(
            "username",
            "That username was taken while you typed. Try another.",
        ));
    }
    created.set(Some(values.email));
    Ok(())
}

/// The fake availability check the async rule awaits.
async fn check_username(name: String) -> Option<String> {
    timer(USERNAME_CHECK_LATENCY).await;
    username_taken(&name).then(|| "That username is taken".to_owned())
}

fn username_taken(name: &str) -> bool {
    TAKEN_USERNAMES.contains(&name.trim().to_lowercase().as_str())
}

/// Optional; digits, `+`, `(`, `)`, `-`, `.`, spaces, at least 7 digits.
fn phone_rule() -> impl Fn(&String, &SignUp) -> Option<String> {
    |phone: &String, _: &SignUp| {
        if phone.is_empty() {
            return None;
        }
        let allowed = phone
            .chars()
            .all(|c| c.is_ascii_digit() || "+()-. ".contains(c));
        let digits = phone.chars().filter(|c| c.is_ascii_digit()).count();
        (!allowed || digits < 7).then(|| "Use digits, with optional + ( ) - and spaces".to_owned())
    }
}

fn otp_rule() -> impl Fn(&String, &SignUp) -> Option<String> {
    |code: &String, _: &SignUp| {
        let shaped = code.chars().all(|c| c.is_ascii_digit()) && code.chars().count() == 6;
        (!code.is_empty() && !shaped).then(|| "Enter the 6 digits".to_owned())
    }
}

fn budget_rule() -> impl Fn(&Option<f64>, &SignUp) -> Option<String> {
    |budget: &Option<f64>, _: &SignUp| {
        budget
            .is_some_and(|value| !(0.0..=10_000.0).contains(&value))
            .then(|| "Between 0 and 10,000".to_owned())
    }
}

/// Whether a ProseMirror document holds any non-blank text.
fn doc_has_text(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .is_some_and(|doc| node_has_text(&doc))
}

fn node_has_text(node: &Value) -> bool {
    if node.get("type").and_then(Value::as_str) == Some("text") {
        return node
            .get("text")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty());
    }
    node.get("content")
        .and_then(Value::as_array)
        .is_some_and(|nodes| nodes.iter().any(node_has_text))
}

fn open_settings(routes: Signal<Vec<FormsRoute>>) {
    routes.update(|r| {
        if r.len() < 2 {
            r.push(FormsRoute::Settings);
        }
    });
}

/// The confirm the dirty guard raises; discard clears the draft.
fn confirm_discard(form: Form<SignUp>) {
    confirm_destructive(
        "Discard this draft?",
        "The sign-up draft you started goes away.",
        "Discard draft",
        move |discard| {
            if discard {
                form.reset();
                Storage::remove_item(DRAFT_KEY);
            }
        },
    );
}
