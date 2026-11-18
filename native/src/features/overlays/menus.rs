//! The anchored popups: `Popover` (a share panel), `Menu` (sections,
//! shortcuts, a checkable item and a submenu) and `ContextMenu` (a file
//! card's commands at the pointer or a held finger).

use super::{
    ButtonVariant, caption, overlay_button, popup_surface, scrim, shortcut, status_line, text_field,
};
use nsh_base::{
    MenuItemElement, MenuSeparatorElement, OpenChangeReason, PopoverEvent, context_menu, menu,
    menu_item, menu_separator, popover, submenu, theme,
};
use nsh_core::prelude::*;

/// A trigger the popup component owns the press of: the look of an
/// outline button, as a plain view, so the trigger's one toggle fires.
fn trigger_chip(label: &'static str, test_id: &'static str) -> impl IntoElement {
    view()
        .min_h_9()
        .px_4()
        .py_2()
        .flex_row()
        .items_center()
        .rounded(move || theme().radius)
        .border_1()
        .border_color(move || theme().border)
        .bg(move || theme().background)
        .transition_colors()
        .active(|s| s.bg(move || theme().accent))
        .test_id(test_id)
        .role(AccessibilityRole::Button)
        .child(
            text(label)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().foreground),
        )
}

/// A styled menu row's shared geometry and states: label leading, hint
/// trailing, on one line.
fn menu_row(row: MenuItemElement) -> MenuItemElement {
    row.flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .px_2()
        .py_1p5()
        .rounded_sm()
        .hover(|s| s.bg(move || theme().accent))
        .active(|s| s.bg(move || theme().accent))
        .focus(|s| s.bg(move || theme().accent))
        .disabled_style(|s| s.opacity(0.5))
}

/// A styled separator.
fn separator() -> MenuSeparatorElement {
    menu_separator()
        .h(px(1.0))
        .my_1()
        .bg(move || theme().border)
}

/// A small section label inside a menu (the app arranges its items
/// freely; the label is one of its children).
fn group_label(label: &'static str) -> impl IntoElement {
    text(label)
        .text_xs()
        .font_weight(FontWeight::SEMIBOLD)
        .px_3()
        .pt_2()
        .pb_1()
        .text_color(move || theme().muted_foreground)
}

/// A row's leading label.
fn row_label(label: &'static str) -> impl IntoElement {
    text(label)
        .text_sm()
        .text_color(move || theme().popover_foreground)
}

/// The reason a popup changed, named for the status line.
fn reason_note(open: bool, reason: OpenChangeReason) -> String {
    let why = match reason {
        OpenChangeReason::TriggerPress => "trigger",
        OpenChangeReason::PressOutside => "press outside",
        OpenChangeReason::BackdropPress => "scrim",
        OpenChangeReason::Cancel => "Escape",
        OpenChangeReason::Select => "choice",
        OpenChangeReason::Hover => "hover",
        OpenChangeReason::Focus => "focus",
        OpenChangeReason::LongPress => "long press",
        OpenChangeReason::ContextMenu => "context menu",
        _ => "app",
    };
    format!("{} ({})", if open { "opened" } else { "closed" }, why)
}

/// `Popover`: a share panel anchored to its button — a field, the people,
/// an invite — closing from the trigger, a press outside, Escape or back.
pub(crate) fn popover_section() -> impl IntoElement {
    #[derive(Clone, PartialEq)]
    struct Person {
        name: &'static str,
        role: &'static str,
    }
    let open = signal(false);
    let invite = signal(String::new());
    let people = signal(vec![
        Person {
            name: "Floyd Wang",
            role: "Owner",
        },
        Person {
            name: "Mira Chen",
            role: "Can edit",
        },
    ]);
    let status = signal("Closed".to_owned());
    let copied = signal(false);
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    overlay_button("Copy link", ButtonVariant::Outline, move || {
                        copied.set(true);
                    }),
                )
                .child(text(move || {
                    if copied.get() {
                        "Link copied".to_owned()
                    } else {
                        "Share the link with the panel, or copy it directly.".to_owned()
                    }
                })
                .text_xs()
                .text_color(move || theme().muted_foreground)),
        )
        .child(
            popover()
                .open(move || Some(open.get()))
                .on_open_change(move |e: &PopoverEvent| {
                    if let PopoverEvent::OpenChange(next, reason) = e {
                        // Controlled: the app follows the request.
                        open.set(*next);
                        status.set(reason_note(*next, *reason));
                    }
                })
                .placement(Placement::Bottom)
                .align(Align::End)
                .offset(px(6.0))
                .margin(px(8.0))
                .popup_style(popup_surface)
                .trigger(trigger_chip("Share", "share-trigger"))
                .child(
                    view()
                        .w(px(300.0))
                        .flex_col()
                        .gap_3()
                        .p_3()
                        .child(
                            text("Share “Q3 roadmap”")
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(move || theme().popover_foreground),
                        )
                        .child(
                            view()
                                .flex_row()
                                .gap_2()
                                .child(
                                    text_field(invite, "Add people by email")
                                        .test_id("invite-field")
                                        .flex_1(),
                                )
                                .child(
                                    overlay_button("Invite", ButtonVariant::Outline, move || {
                                        if invite.with(|v| v.contains('@')) {
                                            let email = invite.get_untracked();
                                            people.update(|p| {
                                                p.push(Person {
                                                    name: "New person",
                                                    role: "Can view",
                                                })
                                            });
                                            invite.set(String::new());
                                            status.set(format!("Invited {email}"));
                                        }
                                    })
                                    .disabled(move || !invite.with(|v| v.contains('@'))),
                                ),
                        )
                        .child(each(
                            people,
                            |person: &Person| person.name.to_owned(),
                            |person, _| {
                                view()
                                    .flex_row()
                                    .items_center()
                                    .justify_between()
                                    .px_2()
                                    .py_1()
                                    .child(
                                        text(move || person.get().name.to_owned())
                                            .text_sm()
                                            .text_color(move || theme().popover_foreground),
                                    )
                                    .child(
                                        text(move || person.get().role.to_owned())
                                            .text_xs()
                                            .text_color(move || theme().muted_foreground),
                                    )
                            },
                        )),
                ),
        )
        .child(caption(
            "Anchored, kept inside the safe area; a press outside closes it and still reaches what you hit — try Copy link while it is open.",
        ))
        .child(status_line({ let s = status; move || s.get() }))
}

/// `Menu`: sections, shortcut hints, a checkable item and an Export
/// submenu (a cascading popup on the desktops, in place on phones).
pub(crate) fn menu_section() -> impl IntoElement {
    let line_numbers = signal(true);
    let last = signal("No choice yet".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            menu()
                .placement(Placement::Bottom)
                .align(Align::Start)
                .offset(px(4.0))
                .margin(px(8.0))
                .popup_style(popup_surface)
                .backdrop_style(scrim)
                .trigger(trigger_chip("Document actions", "actions-trigger"))
                .child(group_label("Document"))
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Rename")
                            .on_select({
                                                                move || last.set("Renamed".to_owned())
                            }),
                    )
                    .child(row_label("Rename…"))
                    .child(shortcut("⌘R")),
                )
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Duplicate")
                            .on_select({
                                                                move || last.set("Duplicated".to_owned())
                            }),
                    )
                    .child(row_label("Duplicate")),
                )
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Show line numbers")
                            .checked(move || Some(line_numbers.get()))
                            .on_select(move || {
                                line_numbers.set(!line_numbers.get_untracked());
                                last.set("Toggled line numbers".to_owned());
                            })
                            .indicator(
                                text("✓")
                                    .text_sm()
                                    .text_color(move || theme().primary)
                                    .opacity(move || {
                                        if line_numbers.get() {
                                            1.0_f32
                                        } else {
                                            0.0_f32
                                        }
                                    }),
                            ),
                    )
                    .child(row_label("Show line numbers"))
                    .child(text(" ").text_sm()),
                )
                .child(separator())
                .child(group_label("Export"))
                .child(
                    submenu()
                        .offset(px(4.0))
                        .trigger(
                            menu_row(menu_item())
                                .child(row_label("Export as…"))
                                .child(shortcut("›")),
                        )
                        .back(menu_row(menu_item()).child(row_label("‹ Export")))
                        .child(
                            menu_row(
                                menu_item()
                                    .accessibility_label("Export as Markdown")
                                    .on_select({
                                                                                move || last.set("Exported Markdown".to_owned())
                                    }),
                            )
                            .child(row_label("Markdown")),
                        )
                        .child(
                            menu_row(
                                menu_item()
                                    .accessibility_label("Export as HTML")
                                    .on_select({
                                                                                move || last.set("Exported HTML".to_owned())
                                    }),
                            )
                            .child(row_label("HTML")),
                        ),
                )
                .child(separator())
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Delete")
                            .on_select({
                                                                move || last.set("Deleted".to_owned())
                            }),
                    )
                    .child(
                        text("Delete…")
                            .text_sm()
                            .text_color(move || theme().destructive),
                    ),
                ),
        )
        .child(caption(
            "A popup at the trigger with a cascading submenu; a sheet at the bottom on phones. The checkmark and the choice history outlive the menu.",
        ))
        .child(status_line({ let s = last; move || s.get() }))
        .child(status_line(move || {
            if line_numbers.get() {
                "Line numbers: on".to_owned()
            } else {
                "Line numbers: off".to_owned()
            }
        }))
}

/// `ContextMenu`: commands for the object under the pointer or finger —
/// the secondary button on the desktops, a held finger on touch screens.
pub(crate) fn context_menu_section() -> impl IntoElement {
    let last = signal("No command yet".to_owned());
    view()
        .flex_col()
        .gap_3()
        .child(
            context_menu()
                .popup_style(popup_surface)
                .backdrop_style(scrim)
                .trigger(
                    view()
                        .w_full()
                        .flex_col()
                        .gap_1()
                        .p_4()
                        .rounded(move || theme().radius)
                        .border_1()
                        .border_color(move || theme().border)
                        .bg(move || theme().muted.opacity(0.4))
                        .test_id("file-card")
                        .child(
                            text("quarterly-report.md")
                                .text_sm()
                                .font_family(crate::ui::text::mono_family)
                                .text_color(move || theme().foreground),
                        )
                        .child(caption("Right-click here, or hold a finger.")),
                )
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Copy")
                            .on_select(move || last.set("Copied".to_owned())),
                    )
                    .child(row_label("Copy"))
                    .child(shortcut("⌘C")),
                )
                .child(
                    menu_row(
                        menu_item()
                            .accessibility_label("Move to archive")
                            .on_select(move || last.set("Moved to archive".to_owned())),
                    )
                    .child(row_label("Move to archive…")),
                )
                .child(separator())
                .child(
                    menu_row(menu_item().on_select(move || last.set("Deleted".to_owned()))).child(
                        text("Delete")
                            .text_sm()
                            .text_color(move || theme().destructive),
                    ),
                ),
        )
        .child(caption(
            "The menu opens where the pointer was; on touch screens it is a sheet.",
        ))
        .child(status_line({
            let s = last;
            move || s.get()
        }))
}
