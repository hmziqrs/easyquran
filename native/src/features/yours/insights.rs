//! Reading insights (PARITY G5; web `yours/_components/ReadingInsights.svelte`):
//! totals, most-read surahs and verses. Signed out from this device's
//! rollup, signed in from `user/analytics/v1/summary` (after a flush).

use eq_core::{api::types::AnalyticsSummary, i18n::keys};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    app::auth_store,
    features::bookmarks::surah_name,
    stores::{
        insights,
        locale::{t, t_args},
    },
    ui::{layout::eyebrow, text::muted, tokens},
};

#[derive(Debug, Clone, PartialEq, Default)]
enum Load {
    #[default]
    Loading,
    Done(Option<AnalyticsSummary>),
}

fn count_row(label: String, count: i64) -> impl IntoElement {
    view()
        .w_full()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            text(label)
                .text_sm()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .text_color(move || theme().foreground),
        )
        .child(
            text(count.to_string())
                .text_sm()
                .text_color(tokens::muted_text),
        )
}

fn column(title: eq_core::i18n::Key, rows: Vec<(String, i64)>) -> impl IntoElement {
    view()
        .flex_1()
        .min_w(px(220.0))
        .flex_col()
        .gap_2()
        .px_4()
        .py_3()
        .child(eyebrow(t(title)))
        .children(
            rows.into_iter()
                .map(|(l, n)| count_row(l, n).into_element()),
        )
}

fn summary_card(s: AnalyticsSummary) -> AnyElement {
    let totals = s.totals;
    let surahs = s
        .top_surahs
        .iter()
        .map(|e| (surah_name(e.surah), e.reads))
        .collect();
    let ayahs = s
        .top_ayahs
        .iter()
        .map(|e| {
            (
                format!("{} {}:{}", surah_name(e.surah), e.surah, e.ayah),
                e.reads,
            )
        })
        .collect();
    view()
        .w_full()
        .flex_col()
        .overflow_hidden()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
        .child(
            view()
                .w_full()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(move || theme().border)
                .child(
                    text(t_args(keys::READER_INSIGHTS_SUMMARY, move || {
                        vec![
                            ("ayahs", totals.ayahs.into()),
                            ("minutes", ((totals.ms + 30_000) / 60_000).into()),
                            ("days", totals.days.into()),
                        ]
                    }))
                    .text_sm()
                    .text_color(move || theme().foreground),
                ),
        )
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .child(column(keys::READER_INSIGHTS_TOP_SURAHS, surahs))
                .child(column(keys::READER_INSIGHTS_TOP_AYAHS, ayahs)),
        )
        .into_element()
}

/// The insights block; re-fetches when the account changes.
pub(crate) fn reading_insights() -> impl IntoElement {
    let state = signal(Load::Loading);
    let auth = auth_store();
    effect(move || {
        let _ = auth.user().map(|u| u.id);
        untracked(move || {
            spawn(async move {
                let summary = insights::fetch_summary().await;
                let _ = state.try_set(Load::Done(summary));
            });
        });
    });
    dynamic(move || match state.get() {
        Load::Loading => muted("…").into_element(),
        Load::Done(Some(s)) if s.totals.ayahs > 0 => summary_card(s),
        Load::Done(_) => muted(t(keys::READER_WIDGET_MOST_READ_EMPTY)).into_element(),
    })
}
