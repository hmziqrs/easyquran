//! Toasts: a `ToastManager` (ordered storage, timeouts, pause while the
//! stack is expanded), a `ToastStack` layout and styled `Toast` roots,
//! triggered by the page's real flows.

use std::{rc::Rc, time::Duration};

use super::{ButtonVariant, caption, overlay_button, status_line};
use nsh_base::{
    Anchor, ToastManager, ToastMotion, ToastOptions, ToastStackState, ToastTransitionStatus, theme,
    toast, toast_stack,
};
use nsh_core::{prelude::*, time::now};

/// How many toasts show at once.
const LIMIT: usize = 3;
/// How often the lifecycle advances while a toast is mounted.
const TICK: Duration = Duration::from_millis(50);

/// What a toast says and offers.
#[derive(Clone, PartialEq)]
struct ToastData {
    title: String,
    description: Option<String>,
    /// The label of the button that dismisses it, when it has one.
    action: Option<&'static str>,
}

/// One mounted toast as the stack shows it.
type ToastRow = (u64, ToastData, ToastTransitionStatus);

/// The trigger section and the toast host, in one component so both read
/// the same manager.
#[component]
pub(crate) fn ToastsArea() -> impl IntoElement {
    let manager = stored(ToastManager::<u64, ToastData>::new(ToastMotion::sonner()));
    let shown = signal(Vec::<ToastRow>::new());
    let queue_note = signal("Queue: (empty)".to_owned());
    let stack_state = ToastStackState::new();
    let sync = move || {
        let (rows, note) = manager.with(|m| {
            let rows = m
                .visible(LIMIT)
                .map(|(id, data, status)| (*id, data.clone(), status))
                .collect::<Vec<_>>();
            let titles: Vec<&str> = m.iter().map(|(_, data, _)| data.title.as_str()).collect();
            let note = if titles.is_empty() {
                "Queue: (empty)".to_owned()
            } else {
                format!("Queue: {}", titles.join(" → "))
            };
            (rows, note)
        });
        shown.set(rows);
        queue_note.set(note);
    };
    // Advances timeouts and exits while the page lives; paused while the
    // stack is expanded, as the manager expects.
    set_interval(TICK, move || {
        if manager.with(|m| m.advance(now(), stack_state.is_expanded()).changed) {
            sync();
        }
    });
    let next_id = stored(1_u64);
    let push = move |data: ToastData, timeout: Option<Duration>| {
        let id = next_id.with(|n| {
            *n += 1;
            *n
        });
        manager.with(|m| m.push(id, data, ToastOptions::timeout(timeout), now()));
        sync();
    };
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(
                    overlay_button("Save the draft", ButtonVariant::Primary, move || {
                        push(
                            ToastData {
                                title: "Draft saved".to_owned(),
                                description: Some("Release notes".to_owned()),
                                action: None,
                            },
                            Some(Duration::from_secs(4)),
                        );
                    })                    ,
                )
                .child(
                    overlay_button("Sync now", ButtonVariant::Outline, move || {
                        push(
                            ToastData {
                                title: "Sync failed".to_owned(),
                                description: Some(
                                    "Check your connection and try again.".to_owned(),
                                ),
                                action: Some("Retry"),
                            },
                            None,
                        );
                    })                    ,
                )
                .child(
                    overlay_button("Announce", ButtonVariant::Ghost, move || {
                        push(
                            ToastData {
                                title: "Invitation sent".to_owned(),
                                description: None,
                                action: None,
                            },
                            Some(Duration::from_secs(3)),
                        );
                    })                    ,
                ),
        )
        .child(caption(
            "Newest nearest the corner; the queue line reads oldest to newest, each toast with its own timeout — or none, until its action.",
        ))
        .child(status_line({ let s = queue_note; move || s.get() }))
        .child(toast_stack_host(
            shown,
            stack_state,
            Rc::new(move |id: &u64| {
                manager.with(|m| m.dismiss(id, now()));
                sync();
            }),
        ))
}

/// Where the toasts show: at the bottom edge, inside the safe area, in a
/// corner on the desktops and centered on a phone.
fn toast_stack_host(
    shown: Signal<Vec<ToastRow>>,
    stack_state: ToastStackState,
    dismiss: Rc<dyn Fn(&u64)>,
) -> impl IntoElement {
    let desktop = is_desktop();
    overlay()
        .left_0()
        .right_0()
        .bottom_0()
        .pointer_events(PointerEvents::BoxNone)
        .p_4()
        .pb(move || px(safe_area().bottom.0 + 16.0))
        .flex_col()
        .items_center()
        .when(desktop, |v| v.items_end())
        .child(
            toast_stack()
                .placement(if desktop {
                    Anchor::BottomRight
                } else {
                    Anchor::BottomCenter
                })
                .state(stack_state)
                .w(px(360.0))
                .on_cancel({
                    let dismiss = dismiss.clone();
                    move || {
                        // Escape and back dismiss the newest toast.
                        if let Some(row) = shown
                            .get()
                            .iter()
                            .rev()
                            .find(|(_, _, status)| *status != ToastTransitionStatus::Ending)
                        {
                            dismiss(&row.0);
                        }
                    }
                })
                .child(each(
                    shown,
                    |row: &ToastRow| row.0,
                    move |row, _| {
                        let for_request = dismiss.clone();
                        let for_action = dismiss.clone();
                        let for_close = dismiss.clone();
                        toast()
                            .transition_status(move || row.get().2)
                            .accessibility_label(move || {
                                let data = row.get().1;
                                Some(SharedString::from(match data.description {
                                    Some(description) => {
                                        format!("{}. {}", data.title, description)
                                    }
                                    None => data.title,
                                }))
                            })
                            .on_dismiss_request(move || for_request(&row.get().0))
                            .rounded(move || theme().radius.max(px(12.0)))
                            .px_4()
                            .py_3()
                            .bg(move || theme().card)
                            .border_1()
                            .border_color(move || theme().border)
                            .shadow_lg()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .transition_all()
                            .duration(200.0)
                            .starting_class("opacity-0 translate-y-4")
                            .ending_class("opacity-0 scale-95")
                            .swiping_class("transition-none")
                            .child(
                                view()
                                    .flex_1()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        text(move || row.get().1.title.clone())
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(move || theme().card_foreground),
                                    )
                                    .child(
                                        text(move || {
                                            row.get().1.description.clone().unwrap_or_default()
                                        })
                                        .text_xs()
                                        .text_color(move || theme().muted_foreground),
                                    ),
                            )
                            .when_some(row.get().1.action, |v, action| {
                                v.child(overlay_button(action, ButtonVariant::Outline, {
                                    let for_action = for_action;
                                    move || for_action(&row.get().0)
                                }))
                            })
                            .child(
                                view()
                                    .size(px(28.0))
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .active(|s| s.bg(move || theme().muted))
                                    .on_press(move || for_close(&row.get().0))
                                    .test_id("toast-close")
                                    .child(
                                        text("×")
                                            .text_sm()
                                            .text_color(move || theme().muted_foreground),
                                    ),
                            )
                    },
                )),
        )
}
