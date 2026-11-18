//! Search page (PARITY E1, E2, E4, E5; `routes/(application)/search/**`).
//! Contract: `pub fn page() -> impl IntoElement`. The command palette's
//! sources (E3) live in `shell::palette` over `eq_core::search::palette`.
//!
//! The query box (with voice input where the platform recognizes speech),
//! the translation picker and chips, local recents, the nav suggestions
//! ("Quick jumps"), the surah suggestions, then one result section per
//! source: Arabic first, then the selected translations in selection order.
//! A result opens the reader at its verse, in the source it was found in.

mod engine;
mod picker;
pub(crate) mod results;
mod voice;

use eq_core::{
    i18n::keys,
    quran::{
        catalogue,
        meta::{Place, meta},
    },
    routes::{ReaderLink, ReaderScope, ReaderTarget, TranslationRef},
    search::{
        nav::{NavMatch, NavTarget},
        normalize::MIN_QUERY_LEN,
        palette::sajda_detail,
        recents::resolve_default_selection,
    },
};
use nsh_base::theme;
use nsh_core::prelude::*;

use engine::{ARABIC, Engine};

use crate::{
    app::quran_store,
    shell::{nav, routes::Route},
    stores::{
        locale::{locale, t, tr},
        search::{SearchStore, load_resume},
    },
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        chip::{ChipTone, chip},
        field::text_field,
        icons,
        layout::{empty_state, page_body, page_insets, pressable_card},
        text::{label, page_heading},
        tokens,
    },
};

/// Debug builds on macOS: `EQ_SHOT_DIR` starts the window capture hook,
/// `EQ_SEARCH_QUERY` seeds the query and `EQ_SEARCH_T` the selection
/// (for screenshots).
fn dev_hooks() {
    #[cfg(debug_assertions)]
    {
        if let Ok(q) = std::env::var("EQ_SEARCH_QUERY") {
            SearchStore::seed(q);
        }
        // The `?t=` deep link's ids (`parseSearchUrl`).
        if let Ok(t) = std::env::var("EQ_SEARCH_T") {
            SearchStore::get().set_selection(&eq_core::search::recents::searchable_ids(&t));
        }
    }
    crate::features::downloads::dev_capture();
}

fn reader_link(ctx: &Option<TranslationRef>, scope: ReaderScope, ayah: Option<u16>) -> Route {
    Route::Reader(ReaderLink::new(ReaderTarget {
        scope,
        translation: ctx.clone(),
        ayah,
    }))
}

/// Where a nav chip goes, in the suggestions' source (`hrefFor`).
fn nav_route(m: &NavMatch, ctx: &Option<TranslationRef>) -> Option<Route> {
    match m.target {
        NavTarget::Juz(n) => Some(reader_link(ctx, ReaderScope::Juz(n), None)),
        NavTarget::Page(n) => Some(reader_link(ctx, ReaderScope::Page(n), None)),
        NavTarget::Ayah { surah, ayah } | NavTarget::Sajda { surah, ayah, .. } => {
            meta().surah_local_page_for_ayah(surah, ayah)?;
            Some(reader_link(ctx, ReaderScope::Surah(surah), Some(ayah)))
        }
        NavTarget::Surah(s) => Some(reader_link(ctx, ReaderScope::Surah(s), None)),
        NavTarget::Place { .. } => None,
    }
}

fn surah_name(n: u16) -> String {
    meta()
        .surah(n)
        .map_or_else(|| n.to_string(), |s| s.name.clone())
}

fn tr_num(key: eq_core::i18n::Key, name: &'static str, n: i64) -> String {
    eq_core::i18n::tr_args(locale(), key, &[(name, n.into())])
}

/// `labelFor`.
fn nav_label(m: &NavMatch) -> String {
    match m.target {
        NavTarget::Juz(n) => tr_num(keys::SEARCH_NAV_JUZ, "num", i64::from(n)),
        NavTarget::Page(n) => tr_num(keys::SEARCH_NAV_PAGE, "num", i64::from(n)),
        NavTarget::Ayah { surah, ayah } => format!("{} {surah}:{ayah}", surah_name(surah)),
        NavTarget::Sajda {
            surah, ayah, kind, ..
        } => format!(
            "{} {surah}:{ayah} · {}",
            surah_name(surah),
            sajda_detail(kind)
        ),
        NavTarget::Surah(s) => format!("{s}. {}", surah_name(s)),
        NavTarget::Place { place, surah_count } => {
            let name = match place {
                Place::Meccan => tr(keys::SEARCH_NAV_PLACE_MECCAN),
                Place::Medinan => tr(keys::SEARCH_NAV_PLACE_MEDINAN),
            };
            let count = tr_num(keys::SEARCH_NAV_PLACE_COUNT, "count", surah_count as i64);
            format!("{name} · {count}")
        }
    }
}

fn controls(engine: Engine, picker_open: Signal<bool>) -> impl IntoElement {
    let query = signal(String::new());
    // The box follows the engine (recents, voice, the seed) and drives it.
    effect(move || {
        let input = engine.input.get();
        if query.get_untracked() != input {
            query.set(input);
        }
    });
    effect(move || {
        let typed = query.get();
        if engine.input.get_untracked() != typed {
            engine.run(typed);
        }
    });
    let store = SearchStore::get();
    view()
        .flex_col()
        .gap_3()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    text_field(query, tr(keys::SEARCH_PLACEHOLDER))
                        .flex_1()
                        .label(t(keys::SEARCH_INPUT_LABEL)),
                )
                .child(voice::voice_button(move |q| engine.run(q)))
                .child(button_with(
                    t(keys::SEARCH_FILTER_OPEN),
                    ButtonVariant::Outline,
                    ButtonSize::Medium,
                    false,
                    None,
                    move || picker_open.update(|o| *o = !*o),
                )),
        )
        .child(dynamic(move || {
            let ids = store.selection();
            if ids.is_empty() {
                return None;
            }
            let chips = ids.into_iter().filter_map(|id| {
                let entry = catalogue::translation(&id)?;
                let name = entry
                    .translator
                    .clone()
                    .unwrap_or_else(|| entry.name.clone());
                Some(
                    button_with(
                        format!("{name} ✕"),
                        ButtonVariant::Secondary,
                        ButtonSize::Small,
                        false,
                        None,
                        move || store.remove(&id),
                    )
                    .into_element(),
                )
            });
            Some(
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .children(chips)
                    .into_element(),
            )
        }))
        .child(dynamic(move || {
            engine
                .searching
                .get()
                .then(|| chip(t(keys::SEARCH_LOADING_MORE), ChipTone::Ghost, true).into_element())
        }))
}

fn recents(engine: Engine) -> impl IntoElement {
    let store = SearchStore::get();
    dynamic(move || {
        let queries = store.recents();
        if !engine.input.get().is_empty() || queries.is_empty() {
            return None;
        }
        let paused = store.paused();
        let pause = if paused {
            keys::SEARCH_RECENTS_RESUME
        } else {
            keys::SEARCH_RECENTS_PAUSE
        };
        Some(
            view()
                .flex_col()
                .gap_2()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(label(t(keys::SEARCH_RECENTS_HEADING)))
                        .child(
                            view()
                                .flex_row()
                                .gap_2()
                                .child(button_with(
                                    t(pause),
                                    ButtonVariant::Ghost,
                                    ButtonSize::Small,
                                    false,
                                    None,
                                    move || store.set_paused(!paused),
                                ))
                                .child(button_with(
                                    t(keys::SEARCH_RECENTS_CLEAR),
                                    ButtonVariant::Ghost,
                                    ButtonSize::Small,
                                    false,
                                    None,
                                    move || store.clear_recents(),
                                )),
                        ),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .children(queries.into_iter().map(|q| {
                            button_with(
                                q.clone(),
                                ButtonVariant::Outline,
                                ButtonSize::Small,
                                false,
                                None,
                                move || engine.run(q.clone()),
                            )
                            .into_element()
                        })),
                )
                .into_element(),
        )
    })
}

fn nav_suggestions(engine: Engine, ctx: Signal<Option<TranslationRef>>) -> impl IntoElement {
    dynamic(move || {
        let matches = engine.nav.get();
        if matches.is_empty() {
            return None;
        }
        let ctx = ctx.get();
        let chips = matches.into_iter().map(move |m| {
            let route = nav_route(&m, &ctx);
            button_with(
                nav_label(&m),
                ButtonVariant::Outline,
                ButtonSize::Small,
                route.is_none(),
                None,
                move || {
                    if let Some(r) = route.clone() {
                        nav::open(r);
                    }
                },
            )
            .into_element()
        });
        Some(
            view()
                .flex_col()
                .gap_2()
                .child(label(t(keys::SEARCH_SECTION_NAV)))
                .child(view().flex_row().flex_wrap().gap_2().children(chips))
                .into_element(),
        )
    })
}

fn surah_suggestions(engine: Engine, ctx: Signal<Option<TranslationRef>>) -> impl IntoElement {
    dynamic(move || {
        let list = engine.surahs.get();
        if list.is_empty() {
            return None;
        }
        let ctx = ctx.get();
        let rows = list.into_iter().filter_map(move |s| {
            let surah = meta().surah(s.num)?;
            let route = reader_link(&ctx, ReaderScope::Surah(s.num), None);
            Some(
                pressable_card(surah.name.clone(), move || nav::open(route.clone()))
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_3()
                    .child(
                        view()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                text(format!("{}. {}", surah.num, surah.name))
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(move || theme().foreground),
                            )
                            .child(
                                text(format!("{} · {}", surah.transliteration, surah.meaning))
                                    .text_xs()
                                    .text_color(tokens::muted_text),
                            ),
                    )
                    .child(
                        text(surah.arabic.clone())
                            .text_base()
                            .text_color(tokens::quran_foreground),
                    )
                    .into_element(),
            )
        });
        Some(
            view()
                .flex_col()
                .gap_2()
                .child(label(t(keys::SEARCH_SECTION_SURAHS)))
                .child(view().flex_col().gap_2().children(rows))
                .into_element(),
        )
    })
}

fn results(engine: Engine) -> impl IntoElement {
    let store = SearchStore::get();
    dynamic(move || {
        let committed = engine.committed.get();
        let input_empty = engine.input.get().is_empty();
        let too_short = committed.chars().count() < MIN_QUERY_LEN && engine.nav.get().is_empty();
        if too_short {
            let message = if input_empty {
                keys::SEARCH_EMPTY_IDLE
            } else {
                keys::SEARCH_TOO_SHORT
            };
            return vec![
                empty_state(Some(icons::SEARCH), t(keys::SEARCH_TITLE), t(message), None)
                    .into_element(),
            ];
        }
        // Arabic first, then the selection's order.
        let ids: Vec<String> = engine
            .sections
            .with(|l| l.iter().map(|s| s.id.clone()).collect());
        let mut ordered: Vec<String> = ids.iter().filter(|id| *id == ARABIC).cloned().collect();
        for id in store.selection() {
            if ids.contains(&id) {
                ordered.push(id);
            }
        }
        if ordered.is_empty() {
            return vec![
                empty_state(
                    Some(icons::SEARCH),
                    t(keys::SEARCH_TITLE),
                    t(keys::SEARCH_PICK_PROMPT),
                    None,
                )
                .into_element(),
            ];
        }
        ordered
            .into_iter()
            .map(|id| results::section_view(engine, id))
            .collect()
    })
}

/// The search page.
pub fn page() -> impl IntoElement {
    dev_hooks();
    let engine = Engine::new();
    let picker_open = signal(false);
    // The suggestions open in the last-read source, else Arabic.
    let suggest_ctx = signal(None::<TranslationRef>);
    let store = SearchStore::get();
    quran_store().refresh();
    load_resume(move |resume| {
        let last = resume.and_then(|r| r.source_id);
        if let Some(id) = &last {
            suggest_ctx.set(Some(TranslationRef::from_id(id)));
        }
        store.set_selection(&resolve_default_selection(
            &[],
            &untracked(|| store.selection()),
            last.as_deref(),
            &[],
        ));
    });
    if let Some(q) = SearchStore::take_seed() {
        engine.run(q);
    }
    // A selection change re-runs the committed query.
    let last_selection = signal(None::<Vec<String>>);
    effect(move || {
        let ids = store.selection();
        let previous = last_selection.get_untracked();
        last_selection.set(Some(ids.clone()));
        if previous.is_some_and(|p| p != ids) {
            let q = engine.committed.get_untracked();
            if q.chars().count() >= MIN_QUERY_LEN {
                engine.run(q);
            }
        }
    });
    // A translation landing on the device joins the running search.
    effect(move || {
        let _ = quran_store().installed();
        untracked(|| engine.artifacts_changed());
    });
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .gap_5()
            .child(page_heading(t(keys::SEARCH_TITLE)))
            .child(controls(engine, picker_open))
            .child(when(move || picker_open.get(), picker::picker))
            .child(recents(engine))
            .child(nav_suggestions(engine, suggest_ctx))
            .child(surah_suggestions(engine, suggest_ctx))
            .child(
                view()
                    .flex_col()
                    .gap_6()
                    .label(t(keys::SEARCH_RESULTS_LABEL))
                    .child(results(engine)),
            ),
    )
}
