//! The EasyQuran command palette (PARITY E3; `GlobalSearchPalette.svelte`,
//! `search/palette/**`, `stores/command-palette.svelte.ts`): every web
//! source (references, surahs, recents, Quran text, translation text,
//! ranges, sajdas, settings, search, site pages, actions) grouped and
//! scored as on the web by `eq_core::search::palette`, plus the shell's
//! own destinations. The in-memory sources answer every keystroke; the
//! two full-text sources run debounced, off the engine thread.
//!
//! Opened by `mod+k` / `/` (desktop) and the header's search button
//! (phones). The macOS command bar (`command_bar`) keeps its own static
//! list and still dispatches through [`dispatch`].

use std::time::Duration;

use eq_core::{
    quran::{
        catalogue::{self, Direction},
        meta::meta,
    },
    routes::{Link, TranslationRef},
    search::{
        nav::parse_query,
        palette::{
            Context, Entry, Group, Icon, Resume, TEXT_LIMIT, Target, dedupe, quran_text_entries,
            sections, sync_entries, translation_query, translation_text_enabled,
            translation_text_entries,
        },
    },
};
use nsh_base::{CommandEntry, CommandItem, command, dialog, theme};
use nsh_core::{element::AnyElement, prelude::*};

use crate::{
    app::auth_store,
    features::search::results::{highlighted_quran, highlighted_text},
    shell::{
        nav,
        routes::{Route, link_for_route, route_from_link},
        undo,
    },
    stores::{
        locale::tr,
        search::{SearchStore, load_resume, search_arabic, search_translation},
    },
    ui::tokens,
};

const DEBOUNCE: Duration = Duration::from_millis(140);

/// The palette is up.
#[derive(Default, Clone, PartialEq)]
pub struct Open(pub bool);

/// The palette's query (`commandPalette.query`).
#[derive(Default, Clone, PartialEq)]
struct Query(String);

/// Rows of the full-text sources for the current query.
#[derive(Default, Clone, PartialEq)]
struct AsyncRows(Vec<Entry>);

/// Where "Continue reading" resumes, read as the palette opens.
#[derive(Default, Clone, PartialEq)]
struct ResumeAt(Option<Resume>);

fn open_state() -> Signal<Open> {
    global::<Open>()
}

fn query() -> Signal<Query> {
    global::<Query>()
}

/// Opens the palette with `seed` as its query (`commandPalette.show`).
pub fn show(seed: &str) {
    query().set(Query(seed.to_owned()));
    load_resume(|r| global::<ResumeAt>().set(ResumeAt(r)));
    open_state().set(Open(true));
}

/// Shows or hides the palette (the shortcut, the header's button).
pub fn toggle() {
    if open_state().with_untracked(|o| o.0) {
        close();
        return;
    }
    show("");
}

/// The `` ` `` hotkey (`resumeToLastRead`): the reader at the thread
/// "Continue reading" would resume, if there is one.
pub fn resume_last_read() {
    load_resume(|resume| {
        let Some(r) = resume else { return };
        let ctx = Context {
            resume: Some(r),
            ..untracked(|| context(""))
        };
        let continue_row = sync_entries(&ctx)
            .into_iter()
            .find(|e| matches!(e.target, Target::Resume { .. }));
        if let Some(Entry {
            target: Target::Resume { link, .. },
            ..
        }) = continue_row
        {
            nav::open(Route::Reader(link));
        }
    });
}

fn close() {
    open_state().set(Open(false));
    query().set(Query(String::new()));
    global::<AsyncRows>().set(AsyncRows::default());
}

/// The reader's translation context, kept across jumps.
fn route_context() -> Option<TranslationRef> {
    match nav::current() {
        Route::Reader(link) => link.target.translation,
        _ => None,
    }
}

fn context(q: &str) -> Context<'static> {
    Context {
        meta: meta(),
        parsed: parse_query(q),
        route: route_context(),
        recents: SearchStore::get().recents(),
        resume: global::<ResumeAt>().get().0,
        authenticated: auth_store().user().is_some(),
    }
}

/// Which translation the palette searches: the reader's, else the last
/// read source's (`scopeFor`).
fn translation_scope(ctx: &Context<'_>) -> Option<String> {
    if let Some(t) = &ctx.route {
        return Some(t.id());
    }
    ctx.resume.as_ref().and_then(|r| r.source_id.clone())
}

fn scope_rtl(id: &str) -> Option<bool> {
    catalogue::translation(id).map(|t| t.direction == Direction::Rtl)
}

/// The shell's destinations as rows (native): ids are route keys, so the
/// command bar's choices dispatch the same way.
fn destinations(ctx: &Context<'_>) -> Vec<Entry> {
    Route::DESTINATIONS
        .iter()
        .filter(|r| **r != Route::Home)
        .filter_map(|route| {
            let label = route.label_str();
            let score = if ctx.parsed.is_empty {
                0.0
            } else {
                eq_core::search::suggest::score_text(label, &ctx.parsed.text)
            };
            if !ctx.parsed.is_empty && score == 0.0 {
                return None;
            }
            let link = link_for_route(route)?;
            let href = eq_core::routes::link_path(&link).unwrap_or_default();
            Some(Entry {
                id: route.key_str().to_owned(),
                source: "shell.destinations",
                group: Group::Destinations,
                label: label.to_owned(),
                detail: None,
                arabic: None,
                preview: None,
                icon: Icon::Note,
                score,
                target: Target::Link(link),
                dedupe_key: Some(format!("href:{href}")),
            })
        })
        .collect()
}

/// Every row for the query, deduplicated, in section order.
fn rows(q: &str) -> Vec<Entry> {
    let ctx = context(q);
    let mut all = sync_entries(&ctx);
    all.extend(global::<AsyncRows>().get().0);
    all.extend(destinations(&ctx));
    sections(dedupe(all))
        .into_iter()
        .flat_map(|s| s.entries)
        .collect()
}

fn group_label(group: Group) -> String {
    match group.label_key() {
        Some(key) => tr(key).to_owned(),
        // i18n: native-only group
        None => "Go to".to_owned(),
    }
}

/// Runs the full-text sources for `q` after the debounce, unless the
/// query moved on.
fn search_text(q: String) {
    global::<AsyncRows>().set(AsyncRows::default());
    let ctx = untracked(|| context(&q));
    let arabic = eq_core::search::palette::quran_text_enabled(&ctx.parsed);
    let scope = translation_scope(&ctx);
    let rtl = scope.as_deref().and_then(scope_rtl);
    let translated = translation_text_enabled(&ctx.parsed, rtl);
    if !arabic && !translated {
        return;
    }
    let text = translation_query(&ctx.parsed);
    spawn(async move {
        timer(DEBOUNCE).await;
        let stale = || query().with_untracked(|cur| cur.0 != q);
        if stale() {
            return;
        }
        let ctx = untracked(|| context(&q));
        let mut rows = Vec::new();
        if arabic && let Ok(page) = search_arabic(ctx.parsed.text.clone(), TEXT_LIMIT, 0).await {
            rows.extend(quran_text_entries(&ctx, &page.results));
        }
        if translated
            && let Some(id) = scope
            && let Ok(page) = search_translation(id.clone(), text, TEXT_LIMIT, 0).await
        {
            rows.extend(translation_text_entries(
                &ctx,
                &id,
                catalogue::peek_translation_name(&id),
                rtl == Some(true),
                &page.results,
            ));
        }
        if !stale() {
            global::<AsyncRows>().set(AsyncRows(rows));
        }
    });
}

fn run_entry(entry: &Entry) {
    match &entry.target {
        Target::Link(Link::Reader(link)) => nav::open(Route::Reader(link.clone())),
        Target::Link(link) => nav::open(route_from_link(link.clone())),
        Target::Verse { link, .. } | Target::Resume { link, .. } => {
            nav::open(Route::Reader(link.clone()));
        }
        Target::FillQuery(q) => {
            query().set(Query(q.clone()));
            return;
        }
        Target::ToggleTheme => crate::shell::toggle_appearance(),
    }
    close();
}

/// Runs the command `id` names and closes the palette: a row of the
/// current list, a destination's route key, or an `action:<name>` of the
/// macOS command bar.
pub(crate) fn dispatch(id: &str) {
    let q = query().with_untracked(|q| q.0.clone());
    if let Some(entry) = untracked(|| rows(&q)).into_iter().find(|e| e.id == id) {
        run_entry(&entry);
        return;
    }
    if let Some(route) = Route::DESTINATIONS.iter().find(|r| r.key_str() == id) {
        nav::open(route.clone());
    } else {
        match id {
            "action:undo" => undo::undo(),
            "action:redo" => undo::redo(),
            "action:appearance" => crate::shell::toggle_appearance(),
            _ => {}
        }
    }
    close();
}

fn row_view(entry: Option<Entry>, label: SharedString) -> AnyElement {
    let Some(entry) = entry else {
        return view()
            .px_2()
            .py_1p5()
            .child(text(label).text_sm())
            .into_element();
    };
    let head = view()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            view()
                .flex_1()
                .flex_col()
                .gap_0p5()
                .child(
                    text(entry.label.clone())
                        .text_sm()
                        .number_of_lines(1)
                        .text_color(move || theme().popover_foreground),
                )
                .child(entry.detail.clone().map(|d| {
                    text(d)
                        .text_xs()
                        .number_of_lines(1)
                        .text_color(tokens::muted_text)
                })),
        )
        .child(
            entry
                .arabic
                .clone()
                .map(|a| text(a).text_base().text_color(tokens::quran_foreground)),
        );
    let preview = entry.preview.map(|p| {
        let body = if p.quran {
            highlighted_quran(p.text, &p.highlights, 18.0)
        } else {
            highlighted_text(p.text, &p.highlights, p.rtl)
        };
        body.number_of_lines(2)
    });
    view()
        .px_2()
        .py_1p5()
        .flex_col()
        .gap_1()
        .child(head)
        .child(preview)
        .into_element()
}

fn command_list() -> AnyElement {
    // Rows follow the query, the recents, the full-text results and the
    // route; the command does no filtering of its own.
    let entries = computed(move || {
        let q = query().with(|q| q.0.clone());
        rows(&q)
    });
    effect(move || {
        let q = query().with(|q| q.0.clone());
        untracked(|| search_text(q));
    });
    let items = move || {
        entries.with(|list| {
            list.iter()
                .map(|e| {
                    CommandItem::new(e.id.clone(), e.label.clone()).group(group_label(e.group))
                })
                .collect::<Vec<_>>()
        })
    };
    // Wrapper gap: ui::command::command_palette filters by itself and draws
    // label-only rows; the palette needs app-answered rows with previews.
    command()
        .items(items)
        .query(move || Some(SharedString::from(query().with(|q| q.0.clone()))))
        .on_query_change(move |q: &SharedString| query().set(Query(q.to_string())))
        .searchable(true)
        .filterable(false)
        .auto_focus(true)
        .separator(true)
        .accessibility_label("Command palette")
        // i18n: palette placeholder (web: search_placeholder family)
        .placeholder(SharedString::from("Search verses, surahs, pages…"))
        .search_bar_style(|s| s.p_2().border_b_1().border_color(move || theme().border))
        .search_style(|s| {
            s.h(px(36.0))
                .px_3()
                .text_sm()
                .text_color(move || theme().popover_foreground)
        })
        .list_style(|s| s.p_1().gap(px(2.0)))
        .item_class("rounded-md focus:bg-accent selected:font-medium")
        .group_style(|s| {
            s.pt_2()
                .px_2()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().muted_foreground)
        })
        .separator_style(|s| s.h(px(1.0)).my_1().bg(move || theme().border))
        .render_item(move |item: &CommandEntry| {
            let entry =
                entries.with_untracked(|l| l.iter().find(|e| e.id == item.id.as_ref()).cloned());
            row_view(entry, item.label.clone())
        })
        .on_select(|id: &SharedString| dispatch(id))
        .on_cancel(close)
        .into_element()
}

/// The palette over everything, while it is open. Escape and back close it.
pub fn palette_dialog() -> AnyElement {
    let open = open_state();
    when(
        move || open.with(|open| open.0),
        || {
            // Wrapper gap: ui::dialog_of lays out a full page, so a
            // floating card over a scrim keeps nsh_base::dialog here.
            dialog()
                .close_on_escape(false)
                .on_cancel(close)
                .flex_1()
                .flex_col()
                .items_center()
                .bg(move || theme().foreground.opacity(0.25))
                .child(
                    view()
                        .w_full()
                        .max_w(px(620.0))
                        .max_h(px(520.0))
                        .flex_col()
                        .overflow_hidden()
                        .rounded_lg()
                        .border_1()
                        .border_color(move || theme().border.opacity(0.5))
                        // Liquid Glass where the system has it (iOS 26);
                        // elsewhere a heavy blur of the page behind a
                        // translucent fill (Windows, Linux, Android, web).
                        .glass_effect(nsh_core::GlassEffect::Clear)
                        .backdrop_blur(px(40.0))
                        .bg(move || theme().popover.opacity(0.6))
                        .shadow_lg()
                        .child(command_list()),
                )
                .into_element()
        },
    )
    .into_element()
}
