//! Reading goals and streaks (PARITY G4; web
//! `yours/_components/ReadingGoals.svelte`, `lib/goals/*`). Goals live in
//! the local store (synced whole as `goals.reading` when signed in);
//! progress is computed from day totals, never stored: the device's
//! rollup signed out, the server's days signed in.

use eq_core::{
    i18n::keys,
    store::goals::{
        self, GoalDraft, GoalDraftError, GoalKind, GoalPreset, GoalState, ReadingDay, ReadingGoal,
    },
};
use nsh_base::{SelectItem, theme};
use nsh_core::prelude::*;

use crate::{
    app::auth_store,
    features::bookmarks::store,
    stores::{
        home::today,
        insights,
        locale::{t, t_args, tr},
        sync::Area,
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        controls::progress,
        field::{field, numeric_field, text_field},
        layout::eyebrow,
        select::select_field,
        text::muted,
        tokens,
    },
};

const HISTORY_DAYS: i64 = 400;
const CALENDAR_WEEKS: i64 = 5;
const PRESETS: [(GoalPreset, eq_core::i18n::Key); 3] = [
    (GoalPreset::Minutes30, keys::READER_GOALS_PRESET_MINUTES),
    (GoalPreset::Finish30, keys::READER_GOALS_PRESET_FINISH_30),
    (GoalPreset::Finish365, keys::READER_GOALS_PRESET_FINISH_365),
];

#[derive(Debug, Clone, PartialEq, Default)]
enum Days {
    #[default]
    Loading,
    Failed,
    Loaded(Vec<ReadingDay>),
}

fn error_key(e: GoalDraftError) -> eq_core::i18n::Key {
    match e {
        GoalDraftError::Dates => keys::READER_GOALS_ERROR_DATES,
        GoalDraftError::Span => keys::READER_GOALS_ERROR_SPAN,
        GoalDraftError::Minutes => keys::READER_GOALS_ERROR_MINUTES,
        GoalDraftError::Limit => keys::READER_GOALS_ERROR_LIMIT,
    }
}

fn state_key(s: GoalState) -> eq_core::i18n::Key {
    match s {
        GoalState::Paused => keys::READER_GOALS_STATE_PAUSED,
        GoalState::Completed => keys::READER_GOALS_STATE_COMPLETED,
        GoalState::Ended => keys::READER_GOALS_STATE_ENDED,
        GoalState::Active => keys::READER_GOALS_STATE_ACTIVE,
    }
}

fn goal_list() -> Signal<Vec<ReadingGoal>> {
    let out = signal(Vec::new());
    let sync = store();
    effect(move || {
        let _ = sync.version(Area::Goals);
        if !sync.ready() {
            return;
        }
        untracked(|| {
            sync.run(
                |s| goals::entries(s),
                move |r| match r {
                    Ok(list) => {
                        let _ = out.try_set(list);
                    }
                    Err(e) => log::warn!("goals read failed: {e}"),
                },
            );
        });
    });
    out
}

fn write_goals(
    job: impl FnOnce(
        &mut eq_core::store::AppStore,
    ) -> Result<Option<GoalDraftError>, eq_core::store::StoreError>
    + Send
    + 'static,
    error: Signal<Option<GoalDraftError>>,
) {
    store().write(Area::Goals, job, move |r| match r {
        Ok(problem) => error.set(problem),
        Err(e) => log::warn!("goals write failed: {e}"),
    });
}

fn small(
    label: impl IntoReactive<SharedString>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    button_with(
        label,
        ButtonVariant::Outline,
        ButtonSize::Small,
        false,
        None,
        on_press,
    )
}

fn calendar(days: Vec<ReadingDay>, today: String) -> impl IntoElement {
    let read: std::collections::HashSet<String> = days
        .iter()
        .filter(|d| d.ayahs_read > 0)
        .map(|d| d.local_day.clone())
        .collect();
    let cells = goals::calendar_days(&today, CALENDAR_WEEKS);
    let rows = cells.chunks(7).map(|week| {
        view()
            .w_full()
            .flex_row()
            .gap_1()
            .children(week.iter().map(|day| {
                let on = read.contains(day);
                let label = format!(
                    "{day}: {}",
                    tr(if on {
                        keys::READER_GOALS_DAY_READ
                    } else {
                        keys::READER_GOALS_DAY_NONE
                    })
                );
                view()
                    .flex_1()
                    .h(px(30.0))
                    .items_center()
                    .justify_center()
                    .rounded(tokens::radius_sm)
                    .bg(move || if on { theme().accent } else { theme().muted })
                    .label(label)
                    .child(
                        text(
                            day.get(8..10)
                                .unwrap_or_default()
                                .trim_start_matches('0')
                                .to_owned(),
                        )
                        .text_xs()
                        .text_color(move || {
                            if on {
                                theme().accent_foreground
                            } else {
                                tokens::muted_text()
                            }
                        }),
                    )
                    .into_element()
            }))
            .into_element()
    });
    view()
        .w_full()
        .flex_col()
        .gap_1()
        .children(rows.collect::<Vec<_>>())
}

fn progress_label(goal: &ReadingGoal, done: f64, target: i64) -> String {
    #[allow(clippy::cast_possible_truncation)]
    let done = done.round() as i64;
    let key = match goal.kind {
        GoalKind::FinishQuran => keys::READER_GOALS_PROGRESS_VERSES,
        GoalKind::DailyMinutes => keys::READER_GOALS_PROGRESS_MINUTES,
    };
    crate::features::bookmarks::fmt(
        key,
        &[
            ("done", done.to_string().into()),
            ("target", target.to_string().into()),
        ],
    )
}

fn goal_row(
    goal: ReadingGoal,
    days: Vec<ReadingDay>,
    today: String,
    error: Signal<Option<GoalDraftError>>,
) -> AnyElement {
    let p = goals::goal_progress(&goal, &days, &today);
    let title = match goal.kind {
        GoalKind::FinishQuran => tr(keys::READER_GOALS_KIND_FINISH).to_owned(),
        GoalKind::DailyMinutes => crate::features::bookmarks::fmt(
            keys::READER_GOALS_PER_DAY,
            &[("minutes", goal.daily_minutes.unwrap_or(0).into())],
        ),
    };
    let window = crate::features::bookmarks::fmt(
        keys::READER_GOALS_WINDOW,
        &[
            ("start", goal.start_day.clone().into()),
            ("end", goals::window_end(&goal, &today).into()),
        ],
    );
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    let fraction = if p.target > 0 {
        (p.done / p.target as f64).min(1.0) as f32
    } else {
        0.0
    };
    let paused = goal.paused_from.is_some();
    let (id_pause, id_remove) = (goal.id.clone(), goal.id.clone());
    let day = today.clone();
    view()
        .w_full()
        .flex_col()
        .gap_1p5()
        .px_3()
        .py(px(10.0))
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .child(text(title).text_sm().text_color(move || theme().foreground))
                .child(
                    text(t(state_key(p.state)))
                        .text_xs()
                        .text_color(tokens::muted_text),
                ),
        )
        .child(text(window).text_xs().text_color(tokens::muted_text))
        .child(progress(fraction))
        .child(
            text(progress_label(&goal, p.done, p.target))
                .text_xs()
                .text_color(tokens::muted_text),
        )
        .child((goal.kind == GoalKind::DailyMinutes).then(|| {
            text(t(keys::READER_GOALS_MINUTES_NOTE))
                .text_xs()
                .text_color(tokens::muted_text)
        }))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(small(
                    t(if paused {
                        keys::READER_GOALS_RESUME
                    } else {
                        keys::READER_GOALS_PAUSE
                    }),
                    move || {
                        let (id, day) = (id_pause.clone(), day.clone());
                        write_goals(
                            move |s| {
                                if paused {
                                    goals::resume(s, &id, &day)?;
                                } else {
                                    goals::pause(s, &id, &day)?;
                                }
                                Ok(None)
                            },
                            error,
                        );
                    },
                ))
                .child(small(t(keys::READER_GOALS_REMOVE), move || {
                    let id = id_remove.clone();
                    write_goals(
                        move |s| {
                            goals::remove(s, &id)?;
                            Ok(None)
                        },
                        error,
                    );
                })),
        )
        .into_element()
}

fn custom_form(
    count: impl Fn() -> usize + Copy + 'static,
    error: Signal<Option<GoalDraftError>>,
) -> impl IntoElement {
    let now = today();
    let kind = signal(Some(SharedString::from(GoalKind::DailyMinutes.as_str())));
    let minutes = signal("15".to_owned());
    let start = signal(now.clone());
    let end = signal(goals::add_days(&now, 29));
    let draft = move || {
        let kind = if kind.get().as_deref() == Some(GoalKind::FinishQuran.as_str()) {
            GoalKind::FinishQuran
        } else {
            GoalKind::DailyMinutes
        };
        GoalDraft {
            kind,
            daily_minutes: (kind == GoalKind::DailyMinutes)
                .then(|| minutes.get().trim().parse().unwrap_or(0)),
            start_day: start.get().trim().to_owned(),
            end_day: end.get().trim().to_owned(),
        }
    };
    let problem = move || goals::draft_error(&draft(), count());
    let is_minutes = move || kind.get().as_deref() != Some(GoalKind::FinishQuran.as_str());
    view()
        .w_full()
        .flex_col()
        .gap_2p5()
        .child(muted(t(keys::READER_GOALS_CUSTOM_HEADING)))
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .gap_3()
                .child(view().min_w(px(200.0)).flex_1().child(field(
                    t(keys::READER_GOALS_KIND),
                    select_field(
                        tr(keys::READER_GOALS_KIND),
                        vec![
                            SelectItem::new(
                                GoalKind::DailyMinutes.as_str(),
                                tr(keys::READER_GOALS_KIND_MINUTES),
                            ),
                            SelectItem::new(
                                GoalKind::FinishQuran.as_str(),
                                tr(keys::READER_GOALS_KIND_FINISH),
                            ),
                        ],
                        kind,
                        "",
                    ),
                )))
                .child(when(is_minutes, move || {
                    view()
                        .min_w(px(200.0))
                        .flex_1()
                        .child(field(
                            t(keys::READER_GOALS_MINUTES_LABEL),
                            numeric_field(minutes, "15"),
                        ))
                        .into_element()
                }))
                .child(view().min_w(px(200.0)).flex_1().child(field(
                    t(keys::READER_GOALS_START_LABEL),
                    text_field(start, "YYYY-MM-DD"),
                )))
                .child(view().min_w(px(200.0)).flex_1().child(field(
                    t(keys::READER_GOALS_END_LABEL),
                    text_field(end, "YYYY-MM-DD"),
                ))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(button_with(
                    t(keys::READER_GOALS_CREATE),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    move || problem().is_some(),
                    None,
                    move || {
                        let d = untracked(draft);
                        write_goals(move |s| goals::add(s, &d), error);
                    },
                ))
                .child(dynamic(move || {
                    problem().map(|p| {
                        text(t(error_key(p)))
                            .text_xs()
                            .text_color(tokens::muted_text)
                            .into_element()
                    })
                })),
        )
}

/// The goals card.
pub(crate) fn reading_goals() -> impl IntoElement {
    let auth = auth_store();
    let list = goal_list();
    let days = signal(Days::Loading);
    let error = signal::<Option<GoalDraftError>>(None);
    let day = signal(today());
    effect(move || {
        let _ = auth.user().map(|u| u.id);
        untracked(move || {
            let now = today();
            day.set(now.clone());
            let from = goals::add_days(&now, -(HISTORY_DAYS - 1));
            let _ = spawn(async move {
                let next = match insights::fetch_days(from, now).await {
                    Some(d) => Days::Loaded(d),
                    None => Days::Failed,
                };
                let _ = days.try_set(next);
            });
        });
    });
    let count = move || list.with(Vec::len);
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .px_4()
        .py_3()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
        .child(eyebrow(t(keys::READER_GOALS_TITLE)))
        .child(when(
            move || auth.user().is_none(),
            || muted(t(keys::READER_GOALS_LOCAL_NOTE)).into_element(),
        ))
        .child(dynamic(move || {
            let loaded = match days.get() {
                Days::Loading => return muted("…").into_element(),
                Days::Failed => {
                    return text(t(keys::READER_GOALS_UNAVAILABLE))
                        .text_sm()
                        .text_color(tokens::danger)
                        .into_element();
                }
                Days::Loaded(d) => d,
            };
            let now = day.get();
            let streak = goals::streak_of(&loaded, &now);
            let goals_now = list.get();
            let rows: Vec<AnyElement> = goals_now
                .into_iter()
                .map(|g| goal_row(g, loaded.clone(), now.clone(), error))
                .collect();
            let empty = rows.is_empty();
            view()
                .w_full()
                .flex_col()
                .gap_3()
                .child(
                    text(t_args(keys::READER_GOALS_STREAK, move || {
                        vec![("count", streak.into())]
                    }))
                    .text_sm()
                    .text_color(move || theme().foreground),
                )
                .child(muted(t(keys::READER_GOALS_CALENDAR)))
                .child(calendar(loaded, now))
                .child(when(
                    move || empty,
                    || muted(t(keys::READER_GOALS_NONE)).into_element(),
                ))
                .child(view().w_full().flex_col().gap_3().children(rows))
                .into_element()
        }))
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_3()
                .pt_3()
                .border_t_1()
                .border_color(move || theme().border)
                .child(muted(t(keys::READER_GOALS_START_HEADING)))
                .child(view().flex_row().flex_wrap().gap_2().children(PRESETS.map(
                    |(preset, key)| {
                        small(t(key), move || {
                            let d = goals::preset_draft(preset, &today());
                            write_goals(move |s| goals::add(s, &d), error);
                        })
                        .into_element()
                    },
                )))
                .child(custom_form(count, error))
                .child(dynamic(move || {
                    error.get().map(|e| {
                        text(t(error_key(e)))
                            .text_xs()
                            .text_color(tokens::danger)
                            .into_element()
                    })
                })),
        )
}
