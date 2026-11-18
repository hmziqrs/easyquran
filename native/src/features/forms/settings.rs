//! The settings form: loads what was saved with `form.load`, saves back
//! through `Storage`, and guards going back while it is dirty. The fake
//! server latency runs on the engine's clock.

use nsh_base::{
    SelectEntry, SelectItem, SliderValue, nav_page, number_input, radio, radio_group, select,
    slider, switch, theme,
};
use nsh_core::{
    element::{AutoCapitalize, KeyboardDismissMode, ReturnKeyType},
    prelude::*,
    storage::Storage,
};
use nsh_form::{BindField, Form, SubmitError, field, rules};
use serde::{Deserialize, Serialize};

use crate::features::forms::fields::{
    SubmitRule, error_line, field_row, field_section, hint_line, input_shell, page_header,
    submit_first_form,
};
use crate::features::forms::sign_up::{FormsRoute, SERVER_LATENCY};
use crate::ui::button::{ButtonSize, ButtonVariant, button_with};
use crate::ui::dialog::confirm_destructive;
use crate::ui::popups::{popup_backdrop, popup_surface, popup_trigger_frame};
use crate::ui::text;

/// The settings' key in `Storage`.
const SETTINGS_KEY: &str = "forms.settings";

/// The user-facing settings the form edits; `Storage` keeps it as JSON.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Settings {
    display_name: String,
    start_page: Option<String>,
    editor_style: Option<String>,
    notifications: bool,
    weekly_digest: bool,
    font_scale: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            start_page: Some("feed".to_owned()),
            editor_style: Some("markdown".to_owned()),
            notifications: true,
            weekly_digest: false,
            font_scale: 1.0,
        }
    }
}

pub(crate) fn settings_screen(routes: Signal<Vec<FormsRoute>>) -> Screen {
    let form = submit_first_form(Settings::default());
    // `load` replaces the values and their baseline, so what loaded reads
    // as clean, not dirty.
    if let Some(saved) = Storage::get_item(SETTINGS_KEY)
        .and_then(|json| serde_json::from_str::<Settings>(&json).ok())
    {
        form.load(saved);
    }

    let display_name = field!(form, display_name).check(rules::required("Enter a display name"));
    let start_page = field!(form, start_page);
    let editor_style = field!(form, editor_style);
    let notifications = field!(form, notifications);
    let weekly_digest = field!(form, weekly_digest);
    let font_scale = field!(form, font_scale);

    let save = move || {
        form.submit(move |values: Settings| async move {
            timer(SERVER_LATENCY).await;
            let json = serde_json::to_string(&values)
                .map_err(|_| SubmitError::form("The settings could not be written"))?;
            Storage::set_item(SETTINGS_KEY, json)
                .map_err(|_| SubmitError::form("Storage is full"))?;
            Ok(())
        });
    };

    let account = field_section(
        "Account",
        [text_field_row(
            "Display name",
            display_name,
            input_shell(move || display_name.error().is_some())
                .placeholder("How the app greets you")
                .auto_capitalize(AutoCapitalize::Words)
                .return_key_type(ReturnKeyType::Done)
                .on_submit(move |_: &str| save())
                .test_id("settings-name"),
        )],
    );

    let reading = field_section(
        "Reading",
        [
            select_row(start_page),
            radio_row(editor_style),
            font_scale_row(font_scale),
        ],
    );

    let delivery = field_section(
        "Delivery",
        [
            switch_row(notifications, "Push notifications"),
            switch_row(weekly_digest, "Weekly digest"),
            number_row(font_scale),
        ],
    );

    crate::ui::nav::themed_header(nav_page())
        .title("Settings")
        .header(page_header("Settings", Some(move || go_back(routes, form))))
        .pop_on_back(move || !form.is_dirty())
        .on_back_press(move || {
            if form.is_dirty() {
                confirm_discard(routes);
            }
        })
        .child(
            scroll()
                .flex_1()
                // nav_page paints the platform's color; the page takes the theme's.
                .bg(move || theme().background)
                .content_inset(crate::ui::layout::page_insets(8.0))
                .keyboard_dismiss_mode(KeyboardDismissMode::OnDrag)
                .child(
                    view()
                        .p_4()
                        .pb_safe_area_plus(px(24.0))
                        .gap_4()
                        .child(account)
                        .child(reading)
                        .child(delivery)
                        .child(
                            view()
                                .gap_2()
                                .child(view().test_id("settings-save").child(button_with(
                                    "Save settings",
                                    ButtonVariant::Primary,
                                    ButtonSize::Medium,
                                    move || form.is_submitting(),
                                    None,
                                    save,
                                )))
                                .child(when(
                                    move || form.submit_count() > 0 && !form.is_dirty(),
                                    move || text::muted("Saved"),
                                ))
                                .child(when(
                                    move || form.is_submitting(),
                                    move || text::muted("Saving…"),
                                ))
                                .child(error_line(move || form.error())),
                        ),
                ),
        )
        // The same soft header and tab-bar edges as the shell's pages.
        .child(crate::shell::soft_edges::page_edges())
}

/// A bound text field row (settings' fields are its own value type).
fn text_field_row(
    label: &'static str,
    field: nsh_form::Field<Settings, String>,
    input: nsh_base::Input,
) -> View {
    field_row(label, field.bind(input), move || field.error())
}

fn select_row(start_page: nsh_form::Field<Settings, Option<String>>) -> View {
    let field = start_page;
    const PAGES: [(&str, &str); 3] = [
        ("feed", "Feed"),
        ("profile", "Profile"),
        ("bookmarks", "Bookmarks"),
    ];
    let title = |value: Option<String>| {
        value
            .and_then(|value| {
                PAGES
                    .iter()
                    .find(|(key, _)| *key == value)
                    .map(|(_, title)| *title)
            })
            .unwrap_or("Pick a page")
    };
    field_row(
        "Start page",
        view().test_id("settings-start-page").child(
            select()
                .items(
                    PAGES
                        .iter()
                        .map(|(value, title)| SelectItem::new(*value, *title))
                        .collect::<Vec<SelectItem>>(),
                )
                .value(move || field.value().map(SharedString::from))
                .on_change(move |value: &Option<SharedString>| {
                    field.set(value.as_deref().map(str::to_owned));
                    field.blur();
                })
                .accessibility_label("Start page")
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
                    move || title(field.value()).to_string(),
                    true,
                )),
        ),
        move || field.error(),
    )
}

fn radio_row(editor_style: nsh_form::Field<Settings, Option<String>>) -> View {
    let field = editor_style;
    let option = |value: &'static str, label: &'static str| {
        let selected = move || field.value().as_deref() == Some(value);
        radio()
            .checked(selected)
            .on_change(move |_: &bool| {
                field.set(Some(value.to_owned()));
                field.blur();
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
        "Editor style",
        radio_group()
            .flex_row()
            .gap_1()
            .child(option("markdown", "Markdown"))
            .child(option("plain", "Plain text")),
        move || field.error(),
    )
}

fn font_scale_row(font_scale: nsh_form::Field<Settings, f64>) -> View {
    let field = font_scale;
    field_row(
        "Text size",
        view()
            .gap_1()
            .child(hint_line(move || format!("{:.1}×", field.value()).into()))
            .child(
                slider()
                    .value(move || Some(SliderValue::Single(field.value())))
                    .min(0.8)
                    .max(1.6)
                    .step(0.1)
                    .on_change(move |value: &SliderValue| {
                        field.set(value.start());
                        field.blur();
                    })
                    .accessibility_label("Text size")
                    .track_class("h-6 justify-center")
                    .indicator_class("h-1.5 rounded-full bg-muted")
                    .range_class("rounded-full bg-primary")
                    .thumb_class("size-5 rounded-full border-2 border-primary bg-background")
                    .flex_1(),
            ),
        move || field.error(),
    )
}

fn switch_row(notifications: nsh_form::Field<Settings, bool>, label: &'static str) -> View {
    let field = notifications;
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .min_h(px(44.0))
        .child(text(label).text_size(px(15.0)))
        .child(
            switch()
                .checked(move || field.value())
                .on_change(move |on: &bool| {
                    field.set(*on);
                    field.blur();
                })
                .accessibility_label(label)
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

/// A second number field over the same value the slider holds, so a form
/// can show one value through two controls.
fn number_row(font_scale: nsh_form::Field<Settings, f64>) -> View {
    let field = font_scale;
    field_row(
        "Exact text size",
        view().test_id("settings-font-scale").child(
            number_input()
                .value(move || Some(field.value()))
                .min(0.8)
                .max(1.6)
                .step(0.1)
                .on_change(move |value: &Option<f64>| {
                    field.set(value.unwrap_or(1.0));
                    field.blur();
                })
                .on_blur(move || field.blur())
                .accessibility_label("Exact text size")
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
        move || field.error(),
    )
}

/// Back from the drawn header: ask first while the form is dirty.
fn go_back(routes: Signal<Vec<FormsRoute>>, form: Form<Settings>) {
    if form.is_dirty() {
        confirm_discard(routes);
    } else {
        routes.update(|r| {
            r.pop();
        });
    }
}

/// The confirm the guard raises; discard pops the settings page.
fn confirm_discard(routes: Signal<Vec<FormsRoute>>) {
    confirm_destructive(
        "Discard your changes?",
        "The settings you edited here go back to what was saved.",
        "Discard changes",
        move |discard| {
            if discard {
                routes.update(|r| {
                    r.pop();
                });
            }
        },
    );
}
