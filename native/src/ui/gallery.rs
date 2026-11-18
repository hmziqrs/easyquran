//! The control gallery: every wrapper in `crate::ui`, shown with its
//! states, in a scrolled, sectioned layout that renders in light and dark
//! and at phone and desktop width.

use nsh_base::dock::{DockAreaLayout, DockLayout};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::ui::{
    avatar, button, carousel, collapse, color, command, controls, dialog, dock, editor, field,
    layout, link, number, pagination, pickers, popups, resizable, select, sheet, table, tabs, text,
    toast, tree,
};

/// The UI gallery page, mounted by the shell; keep its name and signature.
pub fn ui_gallery_page() -> impl IntoElement {
    let open_dialog = signal(false);
    let open_sheet = signal(false);
    let open_form_sheet = signal(false);
    let open_full_dialog = signal(false);
    let deletable = signal(false);
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        .child(
            scroll()
                .flex_1()
                .content_inset(crate::ui::layout::page_insets(0.0))
                .test_id("gallery-scroll")
                .child(
                    layout::page_body()
                        .max_w(px(720.0))
                        .child(text::page_intro(
                            "UI gallery",
                            "Every control of the app's own library, with its states. \
                         Hover, press, focus and disable each one.",
                        ))
                        .child(buttons_section())
                        .child(native_section())
                        .child(choices_section())
                        .child(text_section())
                        .child(fields_section())
                        .child(controls_section())
                        .child(selection_section())
                        .child(nav_rows_section())
                        .child(pickers_section())
                        .child(overlays_section(
                            open_dialog,
                            open_sheet,
                            open_form_sheet,
                            open_full_dialog,
                            deletable,
                        ))
                        .child(data_section())
                        .child(surfaces_section())
                        .child(link_section()),
                ),
        )
        .child(toast::toast_host())
        .child(dialog::dialog_sheet(
            open_form_sheet,
            move || open_form_sheet.set(false),
            || {
                view()
                    .flex_col()
                    .gap_3()
                    .child(text::section_title("Invite a teammate"))
                    .child(text::muted(
                        "A page sheet: the presenting page stays visible behind it.",
                    ))
                    .into_element()
            },
        ))
        .child(dialog::dialog_of(
            open_full_dialog,
            move || open_full_dialog.set(false),
            move || {
                view()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(text::section_title("Review required"))
                    .child(text::body(
                        "A full-screen dialog covers the page until it is dismissed.",
                    ))
                    .child(button::button_with(
                        "Done",
                        button::ButtonVariant::Primary,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || open_full_dialog.set(false),
                    ))
                    .into_element()
            },
        ))
        .child(dialog::dialog_card(
            open_dialog,
            "Delete the draft?".into(),
            move || open_dialog.set(false),
            || {
                text::body("The draft and its autosave go with it. This cannot be undone.")
                    .into_element()
            },
            move || {
                view()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(button::button_with(
                        "Cancel",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || open_dialog.set(false),
                    ))
                    .child(button::button_with(
                        "Delete",
                        button::ButtonVariant::Danger,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || {
                            deletable.set(true);
                            open_dialog.set(false);
                        },
                    ))
                    .into_element()
            },
        ))
        .child(sheet::sheet_of(open_sheet, Placement::Bottom, || {
            view()
                .flex_col()
                .gap_3()
                .child(text::section_title("Filters"))
                .child(controls::switch_row("Only unread", signal(false)))
                .child(controls::switch_row("Sort by date", signal(true)))
                .into_element()
        }))
}

fn choices_section() -> impl IntoElement {
    let fruit = signal(None::<SharedString>);
    let tags = signal(Vec::<SharedString>::new());
    layout::section(
        "Choices from a list",
        view()
            .flex_col()
            .gap_4()
            .child(field::field(
                "Fruit",
                select::select_field(
                    "Fruit",
                    vec![
                        select::choice("apple", "Apple"),
                        select::choice("kiwi", "Kiwi"),
                        select::choice("pear", "Pear"),
                    ],
                    fruit,
                    "Pick a fruit",
                ),
            ))
            .child(field::field(
                "Tags",
                select::combobox_field(
                    "Tags",
                    vec![
                        select::choice("rust", "Rust"),
                        select::choice("swift", "Swift"),
                        select::choice("kotlin", "Kotlin"),
                    ],
                    tags,
                    "Type to tag",
                ),
            ))
            .child(text::muted(move || {
                format!(
                    "Fruit {} tags {}",
                    fruit.get().unwrap_or_default(),
                    tags.get().join(", ")
                )
            })),
    )
}

/// The platform's own controls (`ui::native`): UIKit's on iOS, AppKit's
/// on macOS, the app's styled ones elsewhere. The app's accent tints them.
fn native_section() -> impl IntoElement {
    use crate::ui::native::{
        NativeButtonStyle, is_native, native_button, native_segmented, native_switch,
    };
    let presses = signal(0_u32);
    let press = move || presses.update(|n| *n += 1);
    let airplane = signal(false);
    let range = signal(1_usize);
    layout::section(
        "Native controls",
        view()
            .flex_col()
            .gap_4()
            .child(text::muted(if is_native() {
                "Drawn by the system: its buttons, switch and segmented control, in the app's accent."
            } else {
                "The system draws these on iOS and macOS; here they are the app's own controls."
            }))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(native_button(
                        "Continue",
                        NativeButtonStyle::Filled,
                        None,
                        press,
                    ))
                    .child(native_button(
                        "Send",
                        NativeButtonStyle::Tinted,
                        Some("square.and.arrow.up"),
                        press,
                    ))
                    .child(native_button(
                        "Later",
                        NativeButtonStyle::Gray,
                        None,
                        press,
                    ))
                    .child(native_button(
                        "Glass",
                        NativeButtonStyle::Glass,
                        Some("sparkles"),
                        press,
                    ))
                    .child(native_button(
                        "Details",
                        NativeButtonStyle::Plain,
                        Some("info.circle"),
                        press,
                    )),
            )
            .child(text::muted(move || {
                format!("Native presses: {}", presses.get())
            }))
            .child(native_switch("Airplane mode", airplane))
            .child(native_segmented(&["Day", "Week", "Month"], range))
            .child(text::muted(move || {
                let ranges = ["Day", "Week", "Month"];
                format!(
                    "Airplane mode {} · range {}",
                    if airplane.get() { "on" } else { "off" },
                    ranges.get(range.get()).unwrap_or(&"none")
                )
            })),
    )
}

fn buttons_section() -> impl IntoElement {
    let pressed = signal(0_u32);
    let disabled = signal(true);
    layout::section(
        "Buttons",
        view()
            .flex_col()
            .gap_4()
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(button::button("Primary", || ()))
                    .child(button::button_with(
                        "Secondary",
                        button::ButtonVariant::Secondary,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        || (),
                    ))
                    .child(button::button_with(
                        "Outline",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        || (),
                    ))
                    .child(button::button_with(
                        "Ghost",
                        button::ButtonVariant::Ghost,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        || (),
                    ))
                    .child(button::button_with(
                        "Danger",
                        button::ButtonVariant::Danger,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        || (),
                    )),
            )
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(button::button_with(
                        "Small",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        || (),
                    ))
                    .child(button::button_with(
                        "Large",
                        button::ButtonVariant::Primary,
                        button::ButtonSize::Large,
                        false,
                        None,
                        || (),
                    ))
                    .child(button::icon_button("✚", "Add", || ()))
                    .child(button::icon_button("⋯", "More actions", || ()))
                    .child(button::icon_button("＋", "Disabled add", || ())),
            )
            .child(button::button_with(
                "Disabled",
                button::ButtonVariant::Primary,
                button::ButtonSize::Medium,
                true,
                None,
                || (),
            ))
            .child(
                view()
                    .flex_col()
                    .gap_2()
                    .child(button::button_with(
                        "Enable the delete below",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || disabled.set(false),
                    ))
                    .child(button::button_with(
                        "Delete account",
                        button::ButtonVariant::Danger,
                        button::ButtonSize::Medium,
                        move || disabled.get(),
                        None,
                        || (),
                    )),
            )
            .child(text::muted(move || {
                format!("Primary pressed {} times", pressed.get())
            })),
    )
}

fn no_picture() -> Option<String> {
    None
}

fn text_section() -> impl IntoElement {
    layout::section(
        "Text and marks",
        view()
            .flex_col()
            .gap_2()
            .child(text::title("Title"))
            .child(text::section_title("Section title"))
            .child(text::body("Body copy carries the work."))
            .child(text::muted("Secondary text: hints, captions, metadata."))
            .child(text::label("Field label"))
            .child(
                // Wraps: the code and five badges are wider than a phone.
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(text::code("nsh_base::button"))
                    .child(layout::badge("Default", layout::BadgeTone::Neutral))
                    .child(layout::badge("New", layout::BadgeTone::Success))
                    .child(layout::badge("Beta", layout::BadgeTone::Warning))
                    .child(layout::badge("Info", layout::BadgeTone::Info))
                    .child(layout::badge("Broken", layout::BadgeTone::Danger)),
            )
            .child(layout::divider())
            .child(layout::skeleton())
            .child(layout::skeleton_of(56.0)),
    )
}

fn fields_section() -> impl IntoElement {
    let name = signal(String::new());
    let password = signal(String::new());
    let bio = signal(String::new());
    let amount = signal(String::new());
    layout::section(
        "Fields",
        view()
            .flex_col()
            .gap_4()
            .child(field::field(
                "Display name",
                field::text_field(name, "Ada Lovelace"),
            ))
            .child(field::field(
                "Password",
                field::password_field(password, "••••••••"),
            ))
            .child(field::field(
                "About you",
                field::multiline_field(bio, "A few lines about you"),
            ))
            .child(field::field("Amount", field::numeric_field(amount, "0")))
            .child(text::muted(move || {
                format!(
                    "{} {} {} {}",
                    name.get(),
                    if password.get().is_empty() {
                        ""
                    } else {
                        "•••"
                    },
                    bio.get(),
                    amount.get()
                )
            })),
    )
}

fn controls_section() -> impl IntoElement {
    let wifi = signal(true);
    let subscribe = signal(false);
    let all = signal(2_usize);
    let plan = signal(SharedString::from("Standard"));
    let level = signal(60.0_f64);
    layout::section(
        "Controls",
        view()
            .flex_col()
            .gap_4()
            .child(controls::switch_row("Wi-Fi", wifi))
            .child(text::muted(move || {
                if wifi.get() {
                    "Wi-Fi is on".to_owned()
                } else {
                    "Wi-Fi is off".to_owned()
                }
            }))
            .child(controls::checkbox_row(
                "Subscribe to the changelog",
                subscribe,
            ))
            .child(controls::checkbox_tri_state("Select all", all, 2))
            .child(controls::radio_group_control(
                &["Basic", "Standard", "Pro"],
                plan,
            ))
            .child(controls::slider(level, 0.0, 100.0))
            .child(controls::progress(move || level.get() as f32))
            .child(controls::progress_indeterminate())
            .child(text::muted(move || {
                format!("Level {} of 100", level.get().round() as u32)
            }))
            .child(text::muted(move || {
                format!("{} of 2 selected, plan {}", all.get(), plan.get())
            })),
    )
}

fn selection_section() -> impl IntoElement {
    let tab = signal(0_usize);
    let bold = signal(true);
    let italic = signal(false);
    layout::section(
        "Selection and toggles",
        view()
            .flex_col()
            .gap_4()
            .child(tabs::tab_bar(
                move || tab.get(),
                &["Overview", "Activity", "Logs"],
                move |at| tab.set(at),
            ))
            .child(text::muted(move || {
                format!("Showing {}", ["Overview", "Activity", "Logs"][tab.get()])
            }))
            .child(tabs::toggle_bar(vec![
                tabs::toggle("Bold", bold, move |next| bold.set(next)).into_element(),
                tabs::toggle("Italic", italic, move |next| italic.set(next)).into_element(),
            ]))
            .child(tabs::toolbar_bar(vec![
                button::icon_button("↺", "Undo", || ()).into_element(),
                button::icon_button("↻", "Redo", || ()).into_element(),
                tabs::toolbar_divider().into_element(),
                button::button_with(
                    "Share",
                    button::ButtonVariant::Ghost,
                    button::ButtonSize::Small,
                    false,
                    None,
                    || (),
                )
                .into_element(),
            ]))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(avatar::avatar_image(no_picture, "Ada Lovelace", 40.0))
                    .child(avatar::avatar_group_of(
                        [
                            avatar::avatar_image(no_picture, "Grace Hopper", 32.0),
                            avatar::avatar_image(no_picture, "Alan Turing", 32.0),
                            avatar::avatar_image(no_picture, "Edsger Dijkstra", 32.0),
                        ],
                        2,
                    )),
            ),
    )
}

fn nav_rows_section() -> impl IntoElement {
    let which = signal(SharedString::from("Home"));
    layout::section(
        "Navigation rows",
        view()
            .flex_col()
            .gap_2()
            .children([
                crate::ui::nav::nav_row(
                    "Home",
                    None,
                    computed(move || which.get().as_ref() == "Home"),
                    "nav-home",
                    move || which.set(SharedString::from("Home")),
                )
                .into_element(),
                crate::ui::nav::nav_row(
                    "Inbox",
                    Some(view().child(layout::badge("4", layout::BadgeTone::Primary))),
                    computed(move || which.get().as_ref() == "Inbox"),
                    "nav-inbox",
                    move || which.set(SharedString::from("Inbox")),
                )
                .into_element(),
                crate::ui::nav::nav_row(
                    "Reports",
                    Some(crate::ui::nav::chevron()),
                    computed(move || which.get().as_ref() == "Reports"),
                    "nav-reports",
                    move || which.set(SharedString::from("Reports")),
                )
                .into_element(),
                crate::ui::nav::nav_row(
                    "Archive",
                    Some(crate::ui::nav::chevron()),
                    false,
                    "nav-archive",
                    || (),
                )
                .disabled(true)
                .into_element(),
            ])
            .child(text::muted(move || format!("{} row selected", which.get()))),
    )
}

fn pickers_section() -> impl IntoElement {
    let chosen = signal(Some(nsh_base::Date::Single(None)));
    let time = signal(SharedString::from("--:--"));
    let count = signal(Some(3.0_f64));
    let code = signal(SharedString::from(""));
    // The picker starts at the theme primary, resolved for the current
    // appearance: a picked color is data, not a style.
    let accent = signal(color(theme().primary));
    layout::section(
        "Pickers",
        view()
            .flex_col()
            .gap_4()
            .child(pickers::calendar_of(chosen))
            .child(pickers::date_picker_of("Start date", chosen, "Pick a date"))
            .child(field::field(
                "Start time",
                pickers::time_field_of("Start time", time),
            ))
            .child(field::field(
                "Seats",
                number::number_field("Seats", count, "1"),
            ))
            .child(field::field("Code", number::otp_field("Code", code, 6, 2)))
            .child(color::color_picker_of(accent, |_| ()))
            .child(
                view()
                    .flex_row()
                    .gap_2()
                    .child(color::color_swatch_of(color(theme().primary), true)),
            )
            .child(text::muted(move || {
                format!(
                    "Date {} time {} code {}",
                    chosen
                        .get()
                        .and_then(|date| date.start())
                        .map(|date| format!("{date}"))
                        .unwrap_or_else(|| "none".to_owned()),
                    time.get(),
                    code.get()
                )
            })),
    )
}

fn overlays_section(
    open_dialog: Signal<bool>,
    open_sheet: Signal<bool>,
    open_form_sheet: Signal<bool>,
    open_full_dialog: Signal<bool>,
    deletable: Signal<bool>,
) -> impl IntoElement {
    let menu_open = signal(false);
    let popover_open = signal(false);
    let show_hidden = signal(true);
    layout::section(
        "Overlays",
        view()
            .flex_col()
            .gap_4()
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(popups::dropdown_menu(
                        "Actions",
                        [
                            popups::menu_entry("Rename…", || ()).into_element(),
                            popups::menu_entry("Duplicate", || ()).into_element(),
                            popups::checkable_menu_entry("Show hidden", show_hidden, move |next| {
                                show_hidden.set(next)
                            })
                            .into_element(),
                            popups::submenu_of(
                                "Move to…",
                                [
                                    popups::SubmenuEntry {
                                        label: "Projects",
                                        on_select: || {},
                                        destructive: false,
                                    },
                                    popups::SubmenuEntry {
                                        label: "Archive",
                                        on_select: || {},
                                        destructive: false,
                                    },
                                ],
                            )
                            .into_element(),
                            popups::menu_divider().into_element(),
                            popups::destructive_menu_entry("Delete…", || ()).into_element(),
                        ],
                    ))
                    .child(button::button_with(
                        "Filters sheet",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        move || open_sheet.set(true),
                    ))
                    .child(button::button_with(
                        "Delete draft…",
                        button::ButtonVariant::Danger,
                        button::ButtonSize::Medium,
                        move || !deletable.get(),
                        None,
                        move || {
                            dialog::confirm_destructive(
                                "Delete the draft?",
                                "This cannot be undone.",
                                "Delete",
                                move |confirmed| {
                                    if confirmed {
                                        toast::notify("Draft deleted");
                                    }
                                },
                            )
                        },
                    ))
                    .child(
                        button::button("Notify", || toast::notify("Copied to the clipboard"))
                            .into_element(),
                    ),
            )
            // Triggers hug their labels in one wrapping row; alone in the
            // column each stretched to the full width.
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(popups::popover_of(
                        button::button_with(
                            "Details",
                            button::ButtonVariant::Outline,
                            button::ButtonSize::Medium,
                            false,
                            None,
                            move || popover_open.set(!popover_open.get()),
                        ),
                        view()
                            .w(px(220.0))
                            .flex_col()
                            .gap_1()
                            .child(text::body("Shipped yesterday"))
                            .child(text::muted("Popovers keep the task going.")),
                        move |open| popover_open.set(open),
                    ))
                    .child(
                        view()
                            .tooltip(popups::tooltip_of("Keyboard: mod+/"))
                            .child(button::icon_button("?", "Help", || ())),
                    )
                    .child(popups::hover_card_of(
                        button::button_with(
                            "Hover me",
                            button::ButtonVariant::Ghost,
                            button::ButtonSize::Medium,
                            false,
                            None,
                            || (),
                        ),
                        view()
                            .w(px(200.0))
                            .child(text::body("Ada Lovelace"))
                            .child(text::muted("Writes the first programs.")),
                    ))
                    .child(button::button_with(
                        "Command palette",
                        button::ButtonVariant::Secondary,
                        button::ButtonSize::Medium,
                        false,
                        None,
                        move || menu_open.set(true),
                    )),
            )
            // Commands on the object under the pointer or finger: right
            // click on the desktops, a held finger on touch screens.
            .child(popups::context_menu_of(
                layout::card().child(text::body(
                    "Right-click (or hold) this card for its commands.",
                )),
                [
                    popups::menu_entry("Duplicate card", || ()).into_element(),
                    popups::menu_divider().into_element(),
                    popups::destructive_menu_entry("Delete card…", || ()).into_element(),
                ],
            ))
            .child(command::command_palette(
                vec![
                    command::command_entry("open.gallery", "Open the UI gallery"),
                    command::command_entry("toggle.dark", "Toggle dark appearance"),
                    command::command_entry("copy.id", "Copy the app id"),
                ],
                "Type a command…",
                move |_| toast::notify("Command run"),
            ))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .child(button::button_with(
                        "Open dialog",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || open_dialog.set(true),
                    ))
                    .child(button::button_with(
                        "Open page sheet",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || open_form_sheet.set(true),
                    ))
                    .child(button::button_with(
                        "Open full dialog",
                        button::ButtonVariant::Outline,
                        button::ButtonSize::Small,
                        false,
                        None,
                        move || open_full_dialog.set(true),
                    )),
            ),
    )
}

fn data_section() -> impl IntoElement {
    let page = signal(2_u32);
    let expanded = signal(vec![SharedString::from("src")]);
    let selected = signal(SharedString::from("src/ui"));
    layout::section(
        "Data views",
        view()
            .flex_col()
            .gap_4()
            .child(table::table_of(
                &["Invoice", "Status", "Total"],
                &[
                    vec!["INV001".into(), "Paid".into(), "$120.00".into()],
                    vec!["INV002".into(), "Due".into(), "$80.50".into()],
                ],
            ))
            .child(
                table::data_table_of(
                    vec![
                        table::column("symbol", "Symbol", 120.0),
                        table::column("last", "Last", 90.0),
                        table::column("change", "Change", 90.0),
                    ],
                    nsh_base::DataTableRows::new(2)
                        .texts("symbol", ["AAPL", "MSFT"])
                        .numbers("last", [231.5, 412.0])
                        .numbers("change", [1.2, -0.4]),
                )
                .h(px(160.0)),
            )
            .child(tree::tree_of(
                vec![tree::tree_row(
                    "src",
                    "src",
                    vec![
                        tree::tree_row("src/ui", "ui", vec![]),
                        tree::tree_row("src/shell.rs", "shell.rs", vec![]),
                    ],
                )],
                expanded,
                selected,
            ))
            .child(pagination::pagination_of(page, 12))
            .child(text::muted(move || format!("Page {} of 12", page.get()))),
    )
}

fn surfaces_section() -> impl IntoElement {
    let open_details = signal(true);
    let accordion = signal(vec![SharedString::from("billing")]);
    let page = signal(0_u32);
    let sizes = signal(Vec::new());
    let row_sizes = signal(Vec::new());
    let layout_signal = signal(DockAreaLayout::new(
        DockLayout::h_split()
            .child(DockLayout::tabs().panel("files"), Some(px(200.0)))
            .child(DockLayout::tabs().panel("editor"), None),
    ));
    layout::section(
        "Surfaces",
        view()
            .flex_col()
            .gap_4()
            .child(layout::card().child(text::body("A card: the raised surface.")))
            .child(collapse::collapsible_section(
                "Details",
                open_details,
                view()
                    .flex_col()
                    .gap_1()
                    .child(text::body("Opened with a springing reveal."))
                    .child(text::muted("State inside survives reopenings.")),
            ))
            .child(collapse::accordion_of(
                &[
                    collapse::AccordionEntry {
                        id: "shipping",
                        title: "Shipping",
                        content: "Two to four days.".into(),
                    },
                    collapse::AccordionEntry {
                        id: "billing",
                        title: "Billing",
                        content: "Invoiced monthly.".into(),
                    },
                ],
                accordion,
                false,
            ))
            .child(
                carousel::carousel_of(
                    "Gallery",
                    vec![
                        text::body("First slide").into_element(),
                        text::body("Second slide").into_element(),
                        text::body("Third slide").into_element(),
                    ],
                    page,
                )
                .h(px(120.0)),
            )
            .child(
                resizable::v_resizable_panels(
                    vec![
                        text::body("Top pane").into_element(),
                        text::muted("Drag the handle").into_element(),
                    ],
                    sizes,
                )
                .h(px(140.0)),
            )
            .child(
                resizable::h_resizable_panels(
                    vec![
                        text::body("Leading pane").into_element(),
                        text::muted("Drag the handle").into_element(),
                    ],
                    row_sizes,
                )
                .h(px(120.0)),
            )
            .child(
                dock::dock_workspace(
                    vec![
                        dock::DockPanelSpec {
                            key: "files",
                            title: "Files",
                            content: "main.rs".into(),
                        },
                        dock::DockPanelSpec {
                            key: "editor",
                            title: "Editor",
                            content: "The workspace.".into(),
                        },
                    ],
                    layout_signal,
                )
                .h(px(220.0))
                .rounded_lg()
                .border_1()
                .border_color(move || theme().border)
                .overflow_hidden(),
            )
            .child(field::field(
                "Notes",
                editor::editor_of(
                    "Notes",
                    signal(SharedString::from("fn main() {}\n")),
                    "rust",
                ),
            ))
            .child(editor::rich_editor_of(
                serde_json::json!({
                    "type": "doc",
                    "content": [
                        { "type": "paragraph", "content": [
                            { "type": "text", "text": "Write something. " },
                            { "type": "text", "marks": [{ "type": "bold" }], "text": "Bold works." }
                        ]}
                    ]
                }),
                "Write something…",
            ))
            .child(editor::diff_view_of(
                "fn main() {}\n".into(),
                "fn main() {\n    println!(\"hi\");\n}\n".into(),
            ))
            .child(editor::markdown_of(
                "# Notes\n\nA **document** with `code`, rendered natively.".to_owned(),
            )),
    )
}

fn link_section() -> impl IntoElement {
    let rail = element_ref();
    layout::section(
        "Link and scroll bar",
        view()
            .flex_col()
            .gap_4()
            .child(link::link_out("https://nsh.dev", "Read the NSH docs"))
            .child(
                view()
                    .relative()
                    .h(px(120.0))
                    .child(
                        scroll().flex_1().scrollbar(false).element_ref(&rail).child(
                            view()
                                .flex_col()
                                .gap_1()
                                .p_3()
                                .children((1..=30).map(|line| {
                                    text::body(format!("Line {line} of the scrolled content"))
                                })),
                        ),
                    )
                    .child(
                        view()
                            .absolute()
                            .top_0()
                            .right_0()
                            .bottom_0()
                            .child(crate::ui::scrollbar::scrollbar_for(rail, "Content")),
                    ),
            ),
    )
}
