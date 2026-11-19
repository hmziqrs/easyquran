//! Goal reminders (PARITY G7, M1 local part; web
//! `settings/_components/GoalReminders.svelte`, `lib/reminders/*`).
//!
//! The schedule lives on the server (`notification/v1/reminder/{get,set}`),
//! one per account: on/off, a minute of the day in the device's zone, and
//! optional quiet hours. Signed out the row says to sign in.
//!
//! Delivery: the web's reminder is a server push. This app has no push yet
//! (contracts/devices.md Push matrix), and NSH 0.5.1 shows notifications
//! now but cannot schedule one for later (`NotificationCommand` is
//! `Permission | Show | Close | Listen | SetBadge`, nsh-core
//! `src/notifications.rs:102`). So [`start`] keeps an in-app timer that
//! shows the reminder at its minute while the app runs; a closed app gets
//! none until NSH grows scheduled notifications or push lands.

use eq_core::{
    api::types::ReminderSchedule,
    home::schedule::{
        DEFAULT_REMINDER_MINUTE, REMINDER_KIND, clock_from_minutes, in_quiet_hours,
        minutes_from_clock, minutes_until_reminder, reminder_problem,
    },
    i18n::keys,
};
use nsh_base::SelectItem;
use nsh_core::{
    element::AnyElement,
    notifications::{Notification, NotificationPermission},
    prelude::*,
};

use crate::{
    app::auth_store,
    features::downloads::rows::{caption, row_frame, row_label},
    stores::{
        home::now_minute,
        locale::{t, t_args, tr},
    },
    ui::{button::choice_button, field::field, select::select_field, tokens},
};

const NOTIFICATION_ID: &str = "goal-reminder";

/// The device's zone name (`deviceTimeZone`).
pub fn device_time_zone() -> String {
    let zone = locale_settings().time_zone.as_str().to_owned();
    if zone.is_empty() { "UTC".into() } else { zone }
}

/// The schedule the in-app timer follows (the last one loaded or saved).
#[derive(Debug, Clone, Default, PartialEq)]
struct Armed {
    schedule: Option<ReminderSchedule>,
    generation: u64,
}

fn fire_at_next(generation: u64) {
    let Some(s) = global::<Armed>().with_untracked(|a| a.schedule.clone()) else {
        return;
    };
    if !s.enabled {
        return;
    }
    let minutes = minutes_until_reminder(i64::from(s.reminder_minute), now_minute());
    let delay = std::time::Duration::from_secs(u64::try_from(minutes * 60).unwrap_or(60));
    set_timeout(delay, move || {
        if global::<Armed>().with_untracked(|a| a.generation) != generation {
            return;
        }
        let quiet = in_quiet_hours(
            now_minute(),
            s.quiet_start_minute.map(i64::from),
            s.quiet_end_minute.map(i64::from),
        );
        if !quiet {
            let note = Notification::new(
                NOTIFICATION_ID,
                tr(keys::SETTINGS_PRIVACY_REMINDERS),
                tr(keys::SETTINGS_PRIVACY_REMINDERS_INTRO),
            );
            spawn(async move {
                let _ = show_notification(note).await;
            });
        }
        fire_at_next(generation);
    });
}

fn arm(schedule: Option<ReminderSchedule>) {
    let slot = global::<Armed>();
    let generation = slot.with_untracked(|a| a.generation) + 1;
    slot.set(Armed {
        schedule,
        generation,
    });
    fire_at_next(generation);
}

/// Loads the account's schedule and keeps the in-app reminder timer for
/// it. Call once from the root (the timer lives with the caller's owner).
pub fn start() {
    let auth = auth_store();
    effect(move || {
        let signed_in = auth.user().is_some();
        let client = auth.client();
        untracked(move || {
            let Some(client) = client.filter(|_| signed_in) else {
                arm(None);
                return;
            };
            spawn(async move {
                if let Ok(found) = client.reminder_get(REMINDER_KIND).await {
                    arm(found);
                }
            });
        });
    });
}

fn hours() -> Vec<SelectItem> {
    (0..24)
        .map(|h| {
            let v = format!("{h:02}");
            SelectItem::new(v.clone(), v)
        })
        .collect()
}

fn minutes(current: &str) -> Vec<SelectItem> {
    let mut steps: Vec<String> = (0..12).map(|i| format!("{:02}", i * 5)).collect();
    if !current.is_empty() && !steps.iter().any(|m| m == current) {
        steps.push(current.to_owned());
        steps.sort();
    }
    steps
        .into_iter()
        .map(|m| SelectItem::new(m.clone(), m))
        .collect()
}

/// An `HH:MM` picker (web `TimeSelect.svelte`): hour 00-23 and minute in
/// steps of 5 (an off-grid stored minute stays selectable). Writes `value`
/// on every change and calls `on_change` after.
pub(crate) fn time_select(
    value: Signal<String>,
    hour_name: &'static str,
    minute_name: &'static str,
    on_change: impl Fn() + Copy + 'static,
) -> impl IntoElement {
    let initial = value.get_untracked();
    let (h0, m0) = initial.split_once(':').unwrap_or(("", ""));
    let hour = signal((!h0.is_empty()).then(|| SharedString::from(h0.to_owned())));
    let minute = signal((!m0.is_empty()).then(|| SharedString::from(m0.to_owned())));
    let minute_items = minutes(m0);
    effect(move || {
        let (Some(h), Some(m)) = (hour.get(), minute.get()) else {
            return;
        };
        let next = format!("{h}:{m}");
        untracked(move || {
            if value.get_untracked() != next {
                value.set(next);
                on_change();
            }
        });
    });
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            view()
                .w(px(84.0))
                .child(select_field(hour_name, hours(), hour, "--")),
        )
        .child(text(":").text_color(tokens::muted_text))
        .child(
            view()
                .w(px(84.0))
                .child(select_field(minute_name, minute_items, minute, "--")),
        )
}

/// The settings row (`GoalReminders.svelte`).
pub fn goal_reminders() -> AnyElement {
    let auth = auth_store();
    let saved = signal::<Option<ReminderSchedule>>(None);
    let load_failed = signal(false);
    let busy = signal(false);
    let problem = signal::<Option<eq_core::i18n::Key>>(None);
    let clock = signal(clock_from_minutes(DEFAULT_REMINDER_MINUTE));
    let quiet_on = signal(false);
    let quiet_from = signal("22:00".to_owned());
    let quiet_until = signal("07:00".to_owned());
    let requested = signal(false);
    let enabled = move || saved.with(|s| s.as_ref().is_some_and(|s| s.enabled));
    let zone = move || {
        saved
            .with(|s| s.as_ref().map(|s| s.time_zone.clone()))
            .unwrap_or_else(device_time_zone)
    };

    let apply = move |s: ReminderSchedule| {
        clock.set(clock_from_minutes(i64::from(s.reminder_minute)));
        let quiet = s.quiet_start_minute.is_some() && s.quiet_end_minute.is_some();
        quiet_on.set(quiet);
        if let Some(m) = s.quiet_start_minute {
            quiet_from.set(clock_from_minutes(i64::from(m)));
        }
        if let Some(m) = s.quiet_end_minute {
            quiet_until.set(clock_from_minutes(i64::from(m)));
        }
        saved.set(Some(s.clone()));
        arm(Some(s));
    };

    effect(move || {
        let client = auth.client().filter(|_| auth.user().is_some());
        untracked(move || {
            let Some(client) = client else {
                return;
            };
            if requested.get_untracked() {
                return;
            }
            requested.set(true);
            spawn(async move {
                match client.reminder_get(REMINDER_KIND).await {
                    Ok(Some(s)) => apply(s),
                    Ok(None) => {}
                    Err(_) => load_failed.set(true),
                }
            });
        });
    });

    let persist = move |next_enabled: bool| {
        problem.set(None);
        let Some(reminder) = minutes_from_clock(&clock.get_untracked()) else {
            problem.set(Some(keys::SETTINGS_PRIVACY_REMINDERS_INVALID));
            return;
        };
        let quiet = quiet_on.get_untracked();
        let start = quiet
            .then(|| minutes_from_clock(&quiet_from.get_untracked()))
            .flatten();
        let end = quiet
            .then(|| minutes_from_clock(&quiet_until.get_untracked()))
            .flatten();
        if (quiet && (start.is_none() || end.is_none()))
            || reminder_problem(reminder, start, end).is_some()
        {
            problem.set(Some(keys::SETTINGS_PRIVACY_REMINDERS_INVALID));
            return;
        }
        let Some(client) = auth.client() else {
            return;
        };
        let to_i32 = |m: i64| i32::try_from(m).unwrap_or(0);
        let schedule = ReminderSchedule {
            kind: REMINDER_KIND.into(),
            enabled: next_enabled,
            time_zone: zone(),
            reminder_minute: to_i32(reminder),
            quiet_start_minute: start.map(to_i32),
            quiet_end_minute: end.map(to_i32),
            updated_at: None,
        };
        busy.set(true);
        spawn(async move {
            match client.reminder_set(&schedule).await {
                Ok(stored) => apply(stored),
                Err(_) => problem.set(Some(keys::SETTINGS_PRIVACY_REMINDERS_ERROR)),
            }
            busy.set(false);
        });
    };

    // Turning on asks for the notification permission first (the web's
    // push subscription step).
    let toggle_enabled = move || {
        if busy.get_untracked() {
            return;
        }
        if untracked(enabled) {
            persist(false);
            return;
        }
        busy.set(true);
        spawn(async move {
            let permission = request_notification_permission().await;
            busy.set(false);
            if permission != NotificationPermission::Granted {
                problem.set(Some(keys::SETTINGS_PRIVACY_REMINDERS_NEEDS_PUSH));
                return;
            }
            persist(true);
        });
    };

    let toggle_quiet = move || {
        if busy.get_untracked() {
            return;
        }
        quiet_on.set(!quiet_on.get_untracked());
        if !quiet_on.get_untracked() {
            persist(untracked(enabled));
        }
    };

    let on_off = move || {
        let key = if enabled() {
            keys::READER_ON
        } else {
            keys::READER_OFF
        };
        SharedString::from(tr(key))
    };
    let after_edit = move || persist(untracked(enabled));

    let signed_in = move || auth.user().is_some();
    row_frame()
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_4()
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_col()
                        .gap_0p5()
                        .child(row_label(t(keys::SETTINGS_PRIVACY_REMINDERS)))
                        .child(caption(t(keys::SETTINGS_PRIVACY_REMINDERS_INTRO))),
                )
                .child(when(signed_in, move || {
                    choice_button(
                        on_off,
                        tr(keys::SETTINGS_PRIVACY_REMINDERS),
                        enabled,
                        move || busy.get(),
                        toggle_enabled,
                    )
                    .into_element()
                })),
        )
        .child(
            when(signed_in, move || {
                view()
                    .w_full()
                    .flex_col()
                    .gap_3()
                    .child(when(
                        move || load_failed.get(),
                        || {
                            caption(t(keys::SETTINGS_PRIVACY_REMINDERS_LOAD_ERROR))
                                .text_color(tokens::danger)
                                .into_element()
                        },
                    ))
                    .child(
                        view()
                            .w_full()
                            .flex_row()
                            .flex_wrap()
                            .items_end()
                            .gap_4()
                            .child(field(
                                t(keys::SETTINGS_PRIVACY_REMINDERS_TIME),
                                time_select(
                                    clock,
                                    tr(keys::READER_CARDS_SCHEDULE_HOUR),
                                    tr(keys::READER_CARDS_SCHEDULE_MINUTE),
                                    after_edit,
                                ),
                            ))
                            .child(choice_button(
                                t(keys::SETTINGS_PRIVACY_REMINDERS_QUIET),
                                tr(keys::SETTINGS_PRIVACY_REMINDERS_QUIET),
                                move || quiet_on.get(),
                                move || busy.get(),
                                toggle_quiet,
                            ))
                            .child(when(
                                move || quiet_on.get(),
                                move || {
                                    view()
                                        .flex_row()
                                        .flex_wrap()
                                        .gap_4()
                                        .child(field(
                                            t(keys::SETTINGS_PRIVACY_REMINDERS_QUIET_FROM),
                                            time_select(
                                                quiet_from,
                                                tr(keys::READER_CARDS_SCHEDULE_HOUR),
                                                tr(keys::READER_CARDS_SCHEDULE_MINUTE),
                                                after_edit,
                                            ),
                                        ))
                                        .child(field(
                                            t(keys::SETTINGS_PRIVACY_REMINDERS_QUIET_UNTIL),
                                            time_select(
                                                quiet_until,
                                                tr(keys::READER_CARDS_SCHEDULE_HOUR),
                                                tr(keys::READER_CARDS_SCHEDULE_MINUTE),
                                                after_edit,
                                            ),
                                        ))
                                        .into_element()
                                },
                            )),
                    )
                    .child(caption(t_args(
                        keys::SETTINGS_PRIVACY_REMINDERS_ZONE,
                        move || vec![("zone", zone().into())],
                    )))
                    .child(dynamic(move || {
                        problem
                            .get()
                            .map(|key| caption(t(key)).text_color(tokens::danger).into_element())
                    }))
                    .into_element()
            })
            .otherwise(|| caption(t(keys::SETTINGS_PRIVACY_REMINDERS_SIGNED_OUT)).into_element()),
        )
        .into_element()
}
