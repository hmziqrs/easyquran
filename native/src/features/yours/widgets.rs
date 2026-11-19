//! The Yours widget column (web `yours/+page.svelte`, `lib/home/layout.svelte.ts`,
//! `reading-choice.svelte.ts`): the first-run reading choice, the hidden
//! widgets while editing, and the visible widgets in the stored layout's
//! order, each with move up / move down / hide while editing.

use eq_core::{
    i18n::{Key, keys},
    quran::catalogue,
    store::home::{HOME_WIDGETS, HomeWidget},
};
use nsh_core::{element::AnyElement, prelude::*};

use super::{
    bookmarks_preview, cards::cards_section, continue_threads::continue_threads,
    goals::reading_goals, history::thread_history, insights::reading_insights,
    notes::notes_preview,
};
use crate::{
    features::{bookmarks::shelf, daily_ayah::daily_widget},
    stores::{
        home::{
            self as home_store, HomeData, ReadingChoice, choose_reading, reading_choice,
            suggested_choice, translation_for_language,
        },
        locale::{t, tr},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        layout::eyebrow,
    },
};

/// The default `recents` count (`defaultCount`).
const RECENTS_DEFAULT: i64 = 5;

pub(super) fn widget_title(id: &str) -> Key {
    match id {
        "continue-reading" => keys::READER_INDEX_YOURS_CONTINUE,
        "recents" => keys::READER_THREADS_HISTORY,
        "notes" => keys::READER_WIDGET_NOTES_TITLE,
        "most-read" => keys::READER_WIDGET_MOST_READ_TITLE,
        "daily-ayah" => keys::READER_WIDGET_DAILY_AYAH_TITLE,
        "cards" => keys::READER_CARDS_TITLE,
        _ => keys::NAV_BOOKMARKS,
    }
}

pub(super) fn pill(
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

fn known(id: &str) -> Option<&'static str> {
    HOME_WIDGETS.iter().find(|s| s.0 == id).map(|s| s.0)
}

// --- first-run reading choice ---------------------------------------------------

fn choice_options() -> Vec<(ReadingChoice, String)> {
    let mut out = vec![(
        ReadingChoice::Arabic,
        tr(keys::READER_HOME_READ_IN_ARABIC).to_owned(),
    )];
    let mut push = |choice: ReadingChoice| {
        let ReadingChoice::Translation(id) = &choice else {
            return;
        };
        if out.iter().any(|(c, _)| *c == choice) {
            return;
        }
        let language = catalogue::translation(id)
            .map(|t| t.language.clone())
            .unwrap_or_default();
        let label = crate::features::bookmarks::fmt(
            keys::READER_HOME_READ_IN_WITH,
            &[("language", language.into())],
        );
        out.push((choice, label));
    };
    push(suggested_choice());
    if let Some(en) = translation_for_language("en") {
        push(ReadingChoice::Translation(en));
    }
    out
}

/// One line, until a pick persists; it never returns.
pub(super) fn reading_choice_line() -> impl IntoElement {
    let options = choice_options();
    let many = options.len() > 1;
    when(
        move || reading_choice().is_none() && many,
        move || {
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_2()
                .child(eyebrow(t(keys::READER_HOME_READ_IN)))
                .children(options.iter().cloned().map(|(choice, label)| {
                    pill(SharedString::from(label), move || {
                        choose_reading(choice.clone())
                    })
                    .into_element()
                }))
                .into_element()
        },
    )
}

// --- editing ---------------------------------------------------------------------

fn edit_controls(id: &'static str, index: usize, last: usize) -> AnyElement {
    view()
        .flex_row()
        .items_center()
        .gap_1()
        .child(button_with(
            "↑",
            ButtonVariant::Outline,
            ButtonSize::Small,
            index == 0,
            Some(tr(keys::READER_HOME_MOVE_UP)),
            move || {
                home_store::move_widget(id, -1);
            },
        ))
        .child(button_with(
            "↓",
            ButtonVariant::Outline,
            ButtonSize::Small,
            index == last,
            Some(tr(keys::READER_HOME_MOVE_DOWN)),
            move || {
                home_store::move_widget(id, 1);
            },
        ))
        .child(pill(t(keys::READER_HOME_HIDE), move || {
            home_store::toggle_widget(id)
        }))
        .into_element()
}

/// The hidden widgets, each a pill that shows it again (editing only).
pub(super) fn hidden_widgets(data: Signal<HomeData>, editing: Signal<bool>) -> impl IntoElement {
    dynamic(move || {
        if !editing.get() {
            return view().into_element();
        }
        let hidden: Vec<&'static str> = data.with(|d| {
            d.layout
                .iter()
                .filter(|w| !w.visible)
                .filter_map(|w| known(&w.id))
                .collect()
        });
        view()
            .w_full()
            .flex_col()
            .gap_2()
            .child(eyebrow(t(keys::READER_HOME_HIDDEN)))
            .child(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_1p5()
                    .children(hidden.into_iter().map(|id| {
                        pill(
                            SharedString::from(format!("{} +", tr(widget_title(id)))),
                            move || home_store::toggle_widget(id),
                        )
                        .into_element()
                    })),
            )
            .into_element()
    })
}

// --- widgets ---------------------------------------------------------------------

fn section(id: &'static str, controls: Option<AnyElement>, content: AnyElement) -> AnyElement {
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(
            view()
                .w_full()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(eyebrow(t(widget_title(id))))
                .child(controls),
        )
        .child(content)
        .into_element()
}

fn widget(
    w: &HomeWidget,
    id: &'static str,
    controls: Option<AnyElement>,
    notes_view: Signal<bool>,
) -> AnyElement {
    match id {
        "continue-reading" => section(id, controls, continue_threads().into_element()),
        "recents" => {
            let count = usize::try_from(w.count.unwrap_or(RECENTS_DEFAULT)).unwrap_or(5);
            section(id, controls, thread_history(count).into_element())
        }
        "bookmarks" => bookmarks_preview(shelf(), controls).into_element(),
        "notes" => view()
            .w_full()
            .flex_col()
            .gap_1()
            .child(controls.map(|c| view().flex_row().justify_end().child(c)))
            .child(notes_preview(move || notes_view.set(true)))
            .into_element(),
        "most-read" => section(
            id,
            controls,
            view()
                .w_full()
                .flex_col()
                .gap_4()
                .child(reading_insights())
                .child(reading_goals())
                .into_element(),
        ),
        "daily-ayah" => section(id, controls, daily_widget().into_element()),
        _ => cards_section(controls).into_element(),
    }
}

/// The visible widgets, in the layout's order.
pub(super) fn widget_column(
    data: Signal<HomeData>,
    editing: Signal<bool>,
    notes_view: Signal<bool>,
) -> impl IntoElement {
    // Only a layout change rebuilds the column (not a card edit).
    let layout = computed(move || data.with(|d| d.layout.clone()));
    dynamic(move || {
        let layout = layout.get();
        let editing_now = editing.get();
        let last = layout.len().saturating_sub(1);
        view()
            .w_full()
            .flex_col()
            .gap_6()
            .children(
                layout
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| w.visible)
                    .filter_map(|(index, w)| {
                        let id = known(&w.id)?;
                        let controls = editing_now.then(|| edit_controls(id, index, last));
                        Some(widget(w, id, controls, notes_view))
                    }),
            )
            .into_element()
    })
}
