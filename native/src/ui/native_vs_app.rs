//! Native vs app: each control twice, the app's own beside the system's
//! (`ui::native`, `ui::native_kit`), bound to one signal, so a change on
//! either side shows on both and a screen can pick per control.

use nsh_base::{Date, theme};
use nsh_core::{
    element::{ImageAsset, asset},
    prelude::*,
};

use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use crate::ui::layout::{BadgeTone, badge};
use crate::ui::native::{NativeButtonStyle, native_button, native_segmented, native_switch};
use crate::ui::native_kit::{
    native_alert, native_checkbox, native_color_well, native_date, native_glass_bar,
    native_glass_button, native_popup, native_progress, native_radio_group, native_slider,
    native_spinner, native_stepper, native_tabs, native_text_field, on_ios, on_macos,
};
use crate::ui::{
    color, controls, dialog, field, layout, number, pickers, select, sheet, tabs, text, toast,
};

/// Glass needs something behind it to read as glass: the canvas page's
/// photo.
const BACKDROP: ImageAsset = asset!("canvas/photo.png");
const BAR_SYMBOLS: &[&str] = &["bold", "italic", "underline", "link"];
const BAR_LABELS: &[&str] = &["Bold", "Italic", "Underline", "Link"];
const SIZES: &[&str] = &["Small", "Medium", "Large"];
const PLANS: &[&str] = &["Free", "Pro", "Team"];
const VIEWS: &[&str] = &["List", "Grid", "Board"];
const PANES: &[&str] = &["General", "Privacy", "Advanced"];
const PANE_BODIES: &[&str] = &[
    "Name, appearance and the default page.",
    "What the app may read, and what it shares.",
    "Logging, caches and developer switches.",
];

/// The Native vs app page, mounted by the shell.
pub fn native_vs_app_page() -> impl IntoElement {
    let open_dialog = signal(false);
    let open_sheet = signal(false);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            scroll()
                .flex_1()
                .content_inset(layout::page_insets(0.0))
                .test_id("native-scroll")
                .child(
                    layout::page_body()
                        .max_w(px(860.0))
                        .child(text::page_intro(
                            "Native vs app",
                            "The app's control on the left, the system's on the right, \
                             sharing one value. Change either and both follow.",
                        ))
                        .child(text::muted(platform_note()))
                        .child(glass_row())
                        .child(buttons_row())
                        .child(switch_row())
                        .child(checkbox_row())
                        .child(radio_row())
                        .child(segmented_row())
                        .child(tabs_row())
                        .child(popup_row())
                        .child(text_row())
                        .child(slider_row())
                        .child(stepper_row())
                        .child(date_row())
                        .child(calendar_row())
                        .child(color_row())
                        .child(alerts_row(open_dialog, open_sheet)),
                ),
        )
        .child(dialog::dialog_card(
            open_dialog,
            "Discard changes?".into(),
            move || open_dialog.set(false),
            || text::body("Your edits since the last save are lost.").into_element(),
            move || {
                view()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(button_with(
                        "Cancel",
                        ButtonVariant::Outline,
                        ButtonSize::Small,
                        false,
                        None,
                        move || open_dialog.set(false),
                    ))
                    .child(button_with(
                        "Discard",
                        ButtonVariant::Danger,
                        ButtonSize::Small,
                        false,
                        None,
                        move || open_dialog.set(false),
                    ))
                    .into_element()
            },
        ))
        .child(sheet::sheet_of(open_sheet, Placement::Bottom, || {
            view()
                .flex_col()
                .gap_3()
                .child(text::section_title("Share"))
                .child(text::muted("The app's sheet: any content, the app's look."))
                .child(controls::switch_row("Include comments", signal(true)))
                .into_element()
        }))
        .child(toast::toast_host())
}

fn platform_note() -> &'static str {
    if on_macos() {
        "On macOS AppKit draws the right column."
    } else if on_ios() {
        "On iOS UIKit draws the right column. iOS has no checkbox, radio group or \
         in-page tabs, so those show the app's control there."
    } else {
        "This platform has no system controls wired here: the right column falls \
         back to the app's own. Run on macOS or iOS to compare."
    }
}

/// One control, compared: a title, a line on when to use which, the two
/// columns, and the shared value.
fn compare(
    title: &'static str,
    advice: &'static str,
    app: impl IntoElement,
    native: impl IntoElement,
    readout: impl Fn() -> String + 'static,
) -> impl IntoElement {
    let column = |tag: &'static str, tone: BadgeTone, body: AnyElement| {
        view()
            .flex_1()
            .min_w(px(260.0))
            .flex_col()
            .gap_3()
            .rounded_lg()
            .border_1()
            .border_color(move || theme().border)
            .bg(move || theme().card)
            .p_4()
            .child(view().flex_row().child(badge(tag, tone)))
            .child(body)
    };
    view()
        .flex_col()
        .gap_2()
        .child(text::section_title(title))
        .child(text::muted(advice))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(column("App", BadgeTone::Neutral, app.into_element()))
                .child(column("System", BadgeTone::Primary, native.into_element())),
        )
        .child(text::code(move || SharedString::from(readout())))
}

/// The photo with `controls` floating over it.
fn over_photo(controls: impl IntoElement) -> impl IntoElement {
    view()
        .w_full()
        .h(px(220.0))
        .rounded_lg()
        .overflow_hidden()
        .child(
            image(BACKDROP)
                .object_fit(ResizeMode::Cover)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0(),
        )
        .child(
            view()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .p_4()
                .flex_col()
                .justify_between()
                .child(controls),
        )
}

fn glass_row() -> impl IntoElement {
    let last = signal(SharedString::from("none"));
    let press = move |name: &'static str| move || last.set(name.into());
    let tool = move |at: usize| last.set((*BAR_LABELS.get(at).unwrap_or(&"?")).into());
    let app = over_photo(
        view()
            .flex_col()
            .gap_3()
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(button_with(
                        "Share",
                        ButtonVariant::Primary,
                        ButtonSize::Medium,
                        false,
                        None,
                        press("Share"),
                    ))
                    .child(button_with(
                        "Edit",
                        ButtonVariant::Secondary,
                        ButtonSize::Medium,
                        false,
                        None,
                        press("Edit"),
                    )),
            )
            .child(
                view()
                    .flex_row()
                    .self_start()
                    .gap_1()
                    .p_1()
                    .rounded_full()
                    .bg(move || theme().card)
                    .border_1()
                    .border_color(move || theme().border)
                    .children(BAR_LABELS.iter().enumerate().map(move |(at, label)| {
                        button_with(
                            *label,
                            ButtonVariant::Ghost,
                            ButtonSize::Small,
                            false,
                            None,
                            move || tool(at),
                        )
                    })),
            ),
    );
    let native = over_photo(
        view()
            .flex_col()
            .gap_3()
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(native_glass_button(
                        "Share",
                        Some("square.and.arrow.up"),
                        true,
                        press("Share"),
                    ))
                    .child(native_glass_button("Edit", None, false, press("Edit"))),
            )
            .child(native_glass_bar(BAR_SYMBOLS, BAR_LABELS, false, tool))
            .child(native_glass_bar(BAR_SYMBOLS, BAR_LABELS, true, tool)),
    );
    compare(
        "Liquid Glass",
        "Glass is for controls floating over content: toolbars, the tab bar, a play button. \
         Keep it off cards and lists. Regular glass first, clear (the second bar) over media.",
        app,
        native,
        move || format!("last pressed: {}", last.get()),
    )
}

fn buttons_row() -> impl IntoElement {
    let last = signal(SharedString::from("none"));
    let press = move |name: &'static str| move || last.set(name.into());
    let app = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(button_with(
            "Continue",
            ButtonVariant::Primary,
            ButtonSize::Medium,
            false,
            None,
            press("Continue"),
        ))
        .child(button_with(
            "Send",
            ButtonVariant::Secondary,
            ButtonSize::Medium,
            false,
            None,
            press("Send"),
        ))
        .child(button_with(
            "Later",
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            press("Later"),
        ))
        .child(button_with(
            "Details",
            ButtonVariant::Ghost,
            ButtonSize::Medium,
            false,
            None,
            press("Details"),
        ));
    let native = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(native_button(
            "Continue",
            NativeButtonStyle::Filled,
            None,
            press("Continue"),
        ))
        .child(native_button(
            "Send",
            NativeButtonStyle::Tinted,
            Some("square.and.arrow.up"),
            press("Send"),
        ))
        .child(native_button(
            "Later",
            NativeButtonStyle::Gray,
            None,
            press("Later"),
        ))
        .child(native_button(
            "Details",
            NativeButtonStyle::Plain,
            Some("info.circle"),
            press("Details"),
        ));
    compare(
        "Buttons",
        "System buttons for dialogs, toolbars and settings; the app's for branded calls to action.",
        app,
        native,
        move || format!("last pressed: {}", last.get()),
    )
}

fn switch_row() -> impl IntoElement {
    let on = signal(true);
    compare(
        "Switch",
        "An on/off setting that applies at once. On macOS the system prefers a checkbox for most of these.",
        controls::switch_row("Wi-Fi", on),
        native_switch("Wi-Fi", on),
        move || format!("on: {}", on.get()),
    )
}

fn checkbox_row() -> impl IntoElement {
    let on = signal(false);
    compare(
        "Checkbox",
        "Options in a form or a settings pane, applied on save.",
        controls::checkbox_row("Show hidden files", on),
        native_checkbox("Show hidden files", on),
        move || format!("checked: {}", on.get()),
    )
}

fn radio_row() -> impl IntoElement {
    let size = signal(SharedString::from("Medium"));
    compare(
        "Radio group",
        "One of a few visible options. Past five, use a pop-up.",
        controls::radio_group_control(SIZES, size),
        native_radio_group(SIZES, size),
        move || format!("size: {}", size.get()),
    )
}

fn segmented_row() -> impl IntoElement {
    let at = signal(0_usize);
    compare(
        "Segmented control",
        "Switching the view of the same content.",
        tabs::tab_bar(move || at.get(), VIEWS, move |next| at.set(next)),
        native_segmented(VIEWS, at),
        move || format!("view: {}", VIEWS.get(at.get()).unwrap_or(&"none")),
    )
}

fn tabs_row() -> impl IntoElement {
    let at = signal(0_usize);
    let app = view()
        .flex_col()
        .gap_2()
        .child(tabs::tab_bar(
            move || at.get(),
            PANES,
            move |next| at.set(next),
        ))
        .child(view().p_3().child(text::body(move || {
            SharedString::from(*PANE_BODIES.get(at.get()).unwrap_or(&""))
        })));
    compare(
        "Tabs",
        "Separate panes of one window: tabs over a box on macOS. iOS keeps tabs to the tab bar.",
        app,
        native_tabs(PANES, PANE_BODIES, at),
        move || format!("pane: {}", PANES.get(at.get()).unwrap_or(&"none")),
    )
}

fn popup_row() -> impl IntoElement {
    let plan = signal(Some(SharedString::from("Pro")));
    let items = PLANS.iter().map(|p| select::choice(p, p)).collect();
    compare(
        "Pop-up / select",
        "One of many, closed until asked. The system menu matches every other app's.",
        select::select_field("Plan", items, plan, "Choose a plan"),
        native_popup("Plan", PLANS, plan),
        move || format!("plan: {}", plan.get().unwrap_or_default()),
    )
}

fn text_row() -> impl IntoElement {
    let name = signal(String::new());
    let query = signal(String::new());
    let app = view()
        .flex_col()
        .gap_3()
        .child(field::text_field(name, "Display name"))
        .child(field::text_field(query, "Search"));
    let native = view()
        .flex_col()
        .gap_3()
        .child(native_text_field(name, "Display name", false))
        .child(native_text_field(query, "Search", true));
    compare(
        "Text and search fields",
        "The system field brings the platform's editing menu, autofill and spell check.",
        app,
        native,
        move || format!("name: {:?} · search: {:?}", name.get(), query.get()),
    )
}

fn slider_row() -> impl IntoElement {
    let volume = signal(40.0_f64);
    let level = move || volume.get() / 100.0;
    let app = view()
        .flex_col()
        .gap_3()
        .child(controls::slider(volume, 0.0, 100.0))
        .child(controls::progress(move || volume.get() as f32))
        .child(controls::progress_indeterminate());
    let native = view()
        .flex_col()
        .gap_3()
        .child(native_slider(volume, 0.0, 100.0))
        .child(native_progress(level))
        .child(native_spinner());
    compare(
        "Slider, progress and activity",
        "The slider drives both progress bars below it.",
        app,
        native,
        move || format!("volume: {:.0}", volume.get()),
    )
}

fn stepper_row() -> impl IntoElement {
    let count = signal(Some(2.0_f64));
    let native_count = signal(2.0_f64);
    // The two controls hold different shapes (the field may be empty); the
    // guards stop the pair echoing.
    effect(move || {
        if let Some(n) = count.get()
            && n != native_count.get_untracked()
        {
            native_count.set(n);
        }
    });
    effect(move || {
        let n = native_count.get();
        if count.get_untracked() != Some(n) {
            count.set(Some(n));
        }
    });
    let native = view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(text::body(move || {
            SharedString::from(format!("{} seats", native_count.get()))
        }))
        .child(native_stepper(native_count, 0.0, 20.0, 1.0));
    compare(
        "Stepper",
        "Small counts nudged by one. macOS pairs the stepper with a field or label.",
        number::number_field("Seats", count, "Seats"),
        native,
        move || format!("seats: {:?}", count.get()),
    )
}

/// Today in UTC: a picker with no date shows AppKit's reference date
/// (1 January 2001), so both sides start on a real one.
fn today() -> Option<Date> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    chrono::DateTime::from_timestamp(now.as_secs() as i64, 0).map(|d| Date::from(d.date_naive()))
}

fn date_row() -> impl IntoElement {
    let day = signal(today());
    compare(
        "Date picker",
        "The system picker follows the user's calendar, locale and first weekday.",
        pickers::date_picker_of("Due date", day, "Pick a date"),
        native_date(day, false),
        move || format!("date: {:?}", day.get().and_then(|d| d.start())),
    )
}

fn calendar_row() -> impl IntoElement {
    let day = signal(today());
    compare(
        "Calendar",
        "A month always open, for picking among nearby days.",
        pickers::calendar_of(day),
        native_date(day, true),
        move || format!("date: {:?}", day.get().and_then(|d| d.start())),
    )
}

fn color_row() -> impl IntoElement {
    let tint = signal(theme().primary.light);
    compare(
        "Color",
        "The system well opens the platform's color panel, with the user's saved colors.",
        color::color_picker_of(tint, |_| {}),
        native_color_well(tint),
        move || {
            let c = tint.get();
            format!(
                "hsla: {:.0}° {:.0}% {:.0}%",
                c.h * 360.0,
                c.s * 100.0,
                c.l * 100.0
            )
        },
    )
}

fn alerts_row(open_dialog: Signal<bool>, open_sheet: Signal<bool>) -> impl IntoElement {
    let answer = signal(SharedString::from("none"));
    let answered = move |chosen: Option<&'static str>| {
        answer.set(chosen.unwrap_or("Cancel").into());
    };
    let app = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(button_with(
            "Dialog",
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            move || open_dialog.set(true),
        ))
        .child(button_with(
            "Sheet",
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            move || open_sheet.set(true),
        ));
    let native = view()
        .flex_row()
        .flex_wrap()
        .gap_2()
        .child(button_with(
            "Alert",
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            move || {
                native_alert(
                    "Discard changes?",
                    "Your edits since the last save are lost.",
                    &["Save"],
                    Some("Discard"),
                    false,
                    answered,
                )
            },
        ))
        .child(button_with(
            "Action sheet",
            ButtonVariant::Outline,
            ButtonSize::Medium,
            false,
            None,
            move || {
                native_alert(
                    "Share",
                    "Send the draft to:",
                    &["Mail", "Messages", "Copy link"],
                    None,
                    true,
                    answered,
                )
            },
        ));
    compare(
        "Dialogs and sheets",
        "System alerts for short decisions (a sheet on the macOS window); the app's for any content.",
        app,
        native,
        move || format!("system answer: {}", answer.get()),
    )
}
