//! The swipe-to-dismiss rows: a horizontal pan bound to `translate_x`,
//! its end projected to a snap point — open, recovered, or gone. Only `x`
//! is bound, so a vertical drag stays with the scroll view around the rows.

use nsh_base::theme;
use nsh_core::{element::LayoutAnimation, prelude::*};

use crate::ui;

use super::{api_line, section, status_line};

/// Where the row springs open to: the Delete underlay's width.
const OPEN: f32 = 96.0;
/// A projection past this far dismisses the row outright.
const FLING_AWAY: f32 = -180.0;
/// A projection past half the open width snaps open.
const OPEN_HALF: f32 = -48.0;

/// A row of the list.
#[derive(Debug, Clone, PartialEq)]
struct Task {
    id: u32,
    title: &'static str,
}

/// The list's rows.
const TASKS: [Task; 4] = [
    Task {
        id: 1,
        title: "Alpha",
    },
    Task {
        id: 2,
        title: "Beta",
    },
    Task {
        id: 3,
        title: "Gamma",
    },
    Task {
        id: 4,
        title: "Delta",
    },
];

/// One row: the front card pans; what it projects to decides open,
/// recovery or dismissal. Its title is its test id, so a step finds one
/// row.
fn row(task: Task, rows: Signal<Vec<Task>>, log: Signal<Vec<String>>) -> impl IntoElement {
    let x = animated_value(0.0);
    let title = task.title;
    let on_pan_end = {
        let (task, rows, log) = (task.clone(), rows, log);
        move |e: &PanEvent| {
            // Project where the fling lands, then settle on a snap point.
            let projected = e.value_x.unwrap_or(0.0) + e.velocity_x * 0.2;
            let settle = Spring {
                velocity: Some(e.velocity_x),
                ..Spring::default()
            };
            if projected < FLING_AWAY {
                dismiss(task.clone(), rows, log);
                return;
            }
            let to = if projected < OPEN_HALF { -OPEN } else { 0.0 };
            note(
                log,
                if to == 0.0 {
                    format!("{} recovered", task.title)
                } else {
                    format!("{} opened", task.title)
                },
            );
            x.spring(to, settle);
        }
    };
    view()
        .overflow_hidden()
        .rounded(move || theme().radius)
        .child(
            view()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(px(OPEN))
                .bg(move || theme().destructive)
                .items_center()
                .justify_center()
                .on_press(move |_: &()| dismiss(task.clone(), rows, log))
                .role(AccessibilityRole::Button)
                .label(format!("Delete {title}"))
                .active(|s| s.opacity(0.7))
                .child(
                    text("Delete")
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(crate::ui::tokens::destructive_foreground()),
                ),
        )
        .child(
            view()
                .test_id(title)
                .flex_row()
                .items_center()
                .gap(px(10.0))
                .h(px(56.0))
                .px(px(14.0))
                .bg(move || theme().card)
                .border_1()
                .border_color(move || theme().border)
                .rounded(move || theme().radius)
                .pan_values(Some(&x), None)
                .on_pan_end(on_pan_end)
                .bind_translate_x(&x)
                .child(
                    view()
                        .size(px(10.0))
                        .rounded(px(5.0))
                        .bg(move || theme().primary),
                )
                .child(
                    text(title)
                        .text_size(px(15.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().card_foreground),
                ),
        )
}

/// Adds `entry` to the log, latest first, at most three kept.
fn note(log: Signal<Vec<String>>, entry: String) {
    log.update(|log| {
        log.insert(0, entry);
        log.truncate(3);
    });
}

/// Takes the row out of the data; the removal animates as a layout change.
fn dismiss(task: Task, rows: Signal<Vec<Task>>, log: Signal<Vec<String>>) {
    animate_layout(LayoutAnimation::EASE_IN_EASE_OUT);
    rows.update(|rows| rows.retain(|t| t.id != task.id));
    log.update(|log| {
        log.insert(0, format!("{} dismissed", task.title));
        log.truncate(3);
    });
}

pub(crate) fn swipe_rows() -> impl IntoElement {
    let (rows, log) = (signal(TASKS.to_vec()), signal(Vec::<String>::new()));
    section(
        "Swipe to dismiss",
        "The pan writes the value natively; its end projects to a snap point",
        view()
            .flex_col()
            .gap(px(10.0))
            .child(each(
                rows,
                |task: &Task| task.id,
                move |task: ReadSignal<Task>, _: ReadSignal<usize>| {
                    let task = task.get();
                    row(task, rows, log)
                },
            ))
            .child(when(
                // The restore is there while any row is missing.
                move || rows.with(|r| r.len() < TASKS.len()),
                move || {
                    ui::button::button_with(
                        "Restore rows",
                        ui::button::ButtonVariant::Secondary,
                        ui::button::ButtonSize::Medium,
                        false,
                        Some("Restore rows"),
                        {
                            let rows = rows;
                            move || {
                                animate_layout(LayoutAnimation::EASE_IN_EASE_OUT);
                                rows.set(TASKS.to_vec());
                            }
                        },
                    )
                },
            ))
            .child(status_line("swipe-log", move || log_line(log)))
            .child(api_line(
                "Only x is bound, so vertical drags scroll",
                "pan_values(Some(&x), None) + on_pan_end",
            )),
    )
}

/// The log, latest first, on one line.
fn log_line(log: Signal<Vec<String>>) -> String {
    if log.with(Vec::is_empty) {
        "Nothing swiped yet".to_owned()
    } else {
        log.get().join(" · ")
    }
}
