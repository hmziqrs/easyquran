//! The reader (PARITY C2, C3, C4, C5, C8, C11, C12; G1 reader side): one
//! screen for a surah and for page, juz and hizb ranges. Web spec:
//! `_reader/SurahReader.svelte`, `RangeReader.svelte`, `ReaderShell.svelte`
//! and the stores they use.
//!
//! Contract: `pub fn page(link: ReaderLink) -> impl IntoElement`
//! (`link.target`, plus the URL's `?mode=` and `?more=`).
//!
//! Shape: the scope is read once (`QuranStore::read_scope`: local first,
//! the read API when the script is not on the device), flattened into
//! rows (`model`), and shown by one `virtual_list`. A row is a surah band,
//! a verse (Ayah-by-Ayah) or a page paragraph (Reading). The topmost
//! viewable verse drives the position indicator, the session cursor
//! (throttled) and the read tracker. A mode, script or font change keeps
//! that verse in view: the list is remade scrolled to it.
//!
//! Seams for the next reader slice (translations UI, verse tools, word
//! view), each a new file here: `header::reader_bar(.., leading)` (the
//! translation button), `rows::verse_row(.., actions, extra_lanes)`, and
//! `model::route_translations` (the lanes this route stacks).

pub mod header;
pub mod lanes;
pub mod model;
pub mod rows;
pub mod session;
pub mod share;
pub mod status;
pub mod tracker;
pub mod translations;
pub mod verse_tools;
pub mod words;

use std::rc::Rc;

use eq_core::{
    quran::types::ArabicSource,
    routes::{ReaderMode, ReaderScope},
    store::sessions::ReaderTarget as SessionTarget,
};
use nsh_core::element::{AnyElement, ViewableItems};

use nsh_core::prelude::*;

use self::{
    model::{Failure, Item, Loaded, index_of, lanes_of, verse_at},
    session::{CursorThrottle, Tracking},
    status::notice_for,
};
use crate::{
    shell::{
        nav,
        routes::{ReaderLink, Route},
    },
    stores::{reader::ReaderStore, translations::StackStore},
    ui::layout::{page_insets, skeleton_of},
};

/// What the content area shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Loading,
    Failed(Failure),
    /// The read's generation and the mode: a change remakes the list.
    Ready(u64, ReaderMode),
}

/// Replaces the reader's own route (the next surah, a page link, a
/// resume elsewhere): the stack keeps its depth, the new page keeps the
/// tab's thread.
fn replace_reader(link: ReaderLink) {
    session::mark_from_reader();
    let route = Route::Reader(link);
    nav::stack().update(|routes| match routes.0.last_mut() {
        Some(top) => *top = route,
        None => routes.0.push(route),
    });
}

/// Debug builds: `EQ_SHOT_DIR` starts the window capture hook
/// (`ui::specimen::capture`) and `EQ_MODE=dark` shows the reader dark.
fn dev_hooks() {
    if !cfg!(debug_assertions) {
        return;
    }
    #[cfg(all(target_os = "macos", debug_assertions))]
    if let Ok(dir) = std::env::var("EQ_SHOT_DIR") {
        crate::ui::specimen::capture::start(dir.into());
    }
    if let Some(mode) = std::env::var("EQ_MODE")
        .ok()
        .and_then(|m| crate::stores::appearance::AppearanceMode::from_id(&m))
    {
        crate::stores::appearance::AppearanceStore::get().set_mode(mode);
    }
}

/// The reader page.
pub fn page(link: ReaderLink) -> impl IntoElement {
    log::debug!("reader: open {:?}", link.target);
    dev_hooks();
    let quran = crate::app::quran_store();
    let sync = crate::app::sync_store();
    let prefs = ReaderStore::get();
    let came_from_reader = session::take_from_reader();

    let scope = link.target.scope;
    // `?mode=` wins over the stored mode (the layout's param sync).
    if let Some(mode) = link.mode {
        prefs.set_mode(mode);
    }
    if let ReaderScope::Surah(n) = scope {
        prefs.set_current(n);
    }
    let route_translation = link.target.translation.as_ref().map(|t| t.id());
    let route_ref = stored(link.target.translation.clone());
    let stack = StackStore::get();
    // `?more=` seeds the stack (the layout's param sync): known ids only.
    let known_more: Vec<String> = link
        .more
        .iter()
        .filter(|id| eq_core::quran::catalogue::translation(id).is_some())
        .cloned()
        .collect();
    if !known_more.is_empty() {
        stack.set_ids(&known_more);
    }
    let anchor_verse = match (scope, link.target.ayah) {
        (ReaderScope::Surah(n), Some(a)) => Some((n, a)),
        _ => None,
    };

    let state = signal(Shape::Loading);
    let generation = stored(0_u64);
    let loaded = signal_with(None::<Rc<Loaded>>, SignalEquals::Never);
    let retry = signal(0_u32);
    let lane_order = signal(Vec::<String>::new());
    let last_read = stored(None::<(ArabicSource, u32)>);
    let top = signal(None::<(u16, u16)>);
    let jump = stored(anchor_verse);
    let built = stored(false);
    let list = element_ref();
    let settings_open = signal(false);
    let offer = signal(false);
    let entered = signal(false);

    let items = computed(move || {
        loaded.with(|l| {
            l.as_ref()
                .map(|l| model::build_items(l, prefs.mode()))
                .unwrap_or_default()
        })
    });
    let source_id = move || -> String {
        route_translation
            .clone()
            .unwrap_or_else(|| prefs.script().id().to_owned())
    };
    let source_id = Rc::new(source_id);

    // Read the scope; again on a script change, Retry, or a stack change
    // (lanes only: the list stays up and its rows update in place).
    let held_for_read = link.clone();
    effect(move || {
        let source: ArabicSource = prefs.script();
        let attempt = retry.get();
        let translations = model::route_translations(&held_for_read, &stack.active_ids());
        let lanes_only = built.get()
            && last_read.replace(Some((source, attempt))) == Some((source, attempt))
            && matches!(state.get_untracked(), Shape::Ready(..));
        let order = translations.clone();
        if lanes_only {
            spawn(async move {
                if let Ok(read) = quran.read_scope(scope, source, translations).await {
                    let (lanes, failed_lanes) = lanes_of(
                        read.translations
                            .into_iter()
                            .map(|l| (l.source_id, l.result.map(|(text, _)| text)))
                            .collect(),
                    );
                    batch(|| {
                        loaded.update(|l| {
                            if let Some(l) = l {
                                let mut next = (**l).clone();
                                next.lanes = lanes;
                                next.failed_lanes = failed_lanes;
                                *l = Rc::new(next);
                            }
                        });
                        lane_order.set(order);
                    });
                }
            });
            return;
        }
        if built.get() {
            if let Some(v) = top.get_untracked() {
                jump.replace(Some(v));
            }
        }
        state.set(Shape::Loading);
        spawn(async move {
            let read = quran.read_scope(scope, source, translations).await;
            let arabic = read.map(|r| (r.arabic, r.translations));
            match arabic {
                Err(e) => state.set(Shape::Failed(Failure::of(&e))),
                Ok((Err(e), _)) => state.set(Shape::Failed(Failure::of(&e))),
                Ok((Ok((groups, served)), lanes)) => {
                    let (lanes, failed_lanes) = lanes_of(
                        lanes
                            .into_iter()
                            .map(|l| (l.source_id, l.result.map(|(text, _)| text)))
                            .collect(),
                    );
                    let generation = generation.with(|g| {
                        *g += 1;
                        *g
                    });
                    batch(|| {
                        lane_order.set(order);
                        loaded.set(Some(Rc::new(Loaded {
                            groups,
                            served,
                            source,
                            lanes,
                            failed_lanes,
                        })));
                        state.set(Shape::Ready(generation, prefs.mode()));
                    });
                }
            }
        });
    });

    // Moves the list to a verse: now when it is up, else when it is made.
    let scroll_to = move |surah: u16, ayah: u16| {
        let index = items.with(|v| row_for(v, surah, ayah));
        match (built.get(), index) {
            (true, Some(i)) => {
                list.scroll_to_index(
                    u32::try_from(i).unwrap_or(0),
                    Percentage(0.0),
                    px(0.0),
                    false,
                );
            }
            _ => {
                jump.replace(Some((surah, ayah)));
            }
        }
    };

    // The tab's reading thread for this route (`trackReaderThread`).
    {
        let first = model::first_verse(&scope).unwrap_or((1, 1));
        let (surah, ayah) = anchor_verse.unwrap_or(first);
        let target = SessionTarget {
            scope: model::scope_key(&scope),
            surah: i64::from(surah),
            ayah: i64::from(ayah),
            exact: anchor_verse.is_some(),
            bound_end: None,
            source_id: Some(source_id()),
        };
        let asked = stored(false);
        let link = link.clone();
        // The app database opens in the background: enter once it is up.
        effect(move || {
            if !sync.ready() || asked.get() {
                return;
            }
            asked.replace(true);
            let link = link.clone();
            session::enter(sync, target.clone(), came_from_reader, move |out| {
                if let Some((result, offered)) = out {
                    if result.resumed {
                        let place = (
                            u16::try_from(result.thread.surah).ok(),
                            u16::try_from(result.thread.ayah).ok(),
                        );
                        if let (Some(s), Some(a)) = place {
                            if model::in_scope(&scope, s, a) {
                                scroll_to(s, a);
                            } else {
                                replace_reader(model::link_to(
                                    &link,
                                    ReaderScope::Surah(s),
                                    Some(a),
                                ));
                            }
                        }
                    }
                    offer.set(offered.is_some());
                }
                entered.set(true);
            });
        });
    }

    // Cursor: the topmost verse, throttled; a surah reader adds the anchor.
    let throttle = CursorThrottle::new();
    {
        let source_id = source_id.clone();
        effect(move || {
            let (Some((surah, ayah)), true) = (top.get(), entered.get()) else {
                return;
            };
            let source = Some(source_id());
            let anchor = match scope {
                ReaderScope::Surah(_) => model::local_anchor(surah, ayah),
                _ => None,
            };
            throttle.push(move || session::cursor(sync, surah, ayah, source, anchor));
        });
    }

    // The resume pill hides itself (`AUTO_HIDE_MS`).
    effect(move || {
        if !offer.get() {
            return;
        }
        spawn(async move {
            timer(session::OFFER_HIDE).await;
            offer.set(false);
            session::dismiss_offer(sync);
        });
    });

    let tracking = Tracking::install(sync);

    let change_mode = move |mode: ReaderMode| {
        if let Some(v) = top.get_untracked() {
            jump.replace(Some(v));
        }
        prefs.set_mode(mode);
        state.update(|s| {
            if let Shape::Ready(g, _) = *s {
                *s = Shape::Ready(g, mode);
            }
        });
    };

    let on_view = move |v: &ViewableItems| {
        tracking.interaction();
        let now = v.viewable;
        if let Some((first, _)) = now {
            top.set(items.with(|rows| verse_at(rows, first)));
        }
        let (entered_rows, left_rows) = view_delta(v.previous, now);
        let verses_of = |rows: &[usize]| -> Vec<(u16, u16, usize)> {
            items.with(|all| {
                rows.iter()
                    .filter_map(|i| all.get(*i))
                    .flat_map(item_letters)
                    .collect()
            })
        };
        let first_now = now.map_or(usize::MAX, |(f, _)| f);
        let (above, below): (Vec<usize>, Vec<usize>) =
            left_rows.into_iter().partition(|i| *i < first_now);
        tracking.intersect(&verses_of(&entered_rows), true, false);
        tracking.intersect(&verses_of(&above), false, true);
        tracking.intersect(&verses_of(&below), false, false);
    };

    let held_link = stored(link.clone());
    let go = move |next: ReaderScope| {
        replace_reader(held_link.with(|l| model::link_to(l, next, None)));
    };
    let previous = model::neighbour(scope, -1);
    let next = model::neighbour(scope, 1);

    let shape = computed(move || state.get());
    let content = dynamic(move || {
        let shape = shape.get();
        untracked(move || match shape {
            Shape::Loading => loading().into_element(),
            Shape::Failed(f) => {
                status::failure_view(f, move || retry.update(|n| *n += 1)).into_element()
            }
            Shape::Ready(..) => {
                built.replace(true);
                let initial = jump
                    .replace(None)
                    .and_then(|(s, a)| items.with(|v| row_for(v, s, a)));
                let mut reader = virtual_list(
                    move || items.with(Vec::len),
                    move |at| {
                        view().w_full().child(dynamic(move || {
                            let item = items.with(|v| v.get(at.index()).cloned());
                            // A face change can flip the weight: remake.
                            prefs.arabic_font();
                            let failed = loaded.with(|l| {
                                l.as_ref()
                                    .map(|l| l.failed_lanes.clone())
                                    .unwrap_or_default()
                            });
                            let order = lane_order.get();
                            let route = route_ref.get();
                            untracked(|| {
                                render_item(
                                    item,
                                    prefs,
                                    Row {
                                        route,
                                        failed,
                                        order,
                                    },
                                )
                            })
                        }))
                    },
                )
                .keys(move || items.with(|v| v.iter().map(Item::key).collect::<Vec<_>>()))
                .estimated_item_size(px(160.0))
                .footer(status::neighbour_nav(previous, next, go))
                .on_viewable_items_change(on_view)
                .on_scroll_drag_start(move |_: &Point<Points>| tracking.interaction())
                .content_inset(page_insets(0.0))
                .element_ref(&list)
                .flex_1();
                if let Some(i) = initial {
                    reader = reader.initial_scroll_index(i);
                }
                reader.into_element()
            }
        })
    });

    let notice = computed(move || {
        loaded.with(|l| {
            l.as_ref()
                .and_then(|l| notice_for(l.served, !l.failed_lanes.is_empty()))
        })
    });

    view()
        .flex_1()
        .child(header::reader_bar(
            prefs,
            top,
            settings_open,
            change_mode,
            Some(translations::translation_button(
                link,
                top,
                prefs,
                replace_reader,
            )),
        ))
        .child(dynamic(move || match notice.get() {
            Some(n) => status::banner(n).into_element(),
            None => view().into_element(),
        }))
        .child(when(offer, move || {
            status::resume_pill(
                move || {
                    offer.set(false);
                    session::start_over(sync, move |place| {
                        let first = model::first_verse(&scope);
                        if let Some((s, a)) = place.or(first) {
                            scroll_to(s, a);
                        }
                    });
                },
                move || {
                    offer.set(false);
                    session::dismiss_offer(sync);
                },
            )
        }))
        .child(view().flex_1().child(content))
        .child(header::settings_sheet(prefs, settings_open, change_mode))
}

/// The row to scroll to for a verse: its surah band when it opens the
/// surah, so the title shows above it.
fn row_for(items: &[Item], surah: u16, ayah: u16) -> Option<usize> {
    let i = index_of(items, surah, ayah)?;
    let opens_after_band = i > 0 && matches!(items.get(i - 1), Some(Item::Header { .. }));
    let first_of_row =
        items.get(i).and_then(|item| item.verses().first().copied()) == Some((surah, ayah));
    if opens_after_band && first_of_row {
        return Some(i - 1);
    }
    Some(i)
}

/// Rows that came into view and rows that left, between two reports.
fn view_delta(
    before: Option<(usize, usize)>,
    now: Option<(usize, usize)>,
) -> (Vec<usize>, Vec<usize>) {
    let inside = |r: Option<(usize, usize)>, i: usize| r.is_some_and(|(a, b)| i >= a && i <= b);
    let came = now
        .map(|(a, b)| (a..=b).filter(|i| !inside(before, *i)).collect())
        .unwrap_or_default();
    let went = before
        .map(|(a, b)| (a..=b).filter(|i| !inside(now, *i)).collect())
        .unwrap_or_default();
    (came, went)
}

/// The verses of a row with their Arabic letter counts.
fn item_letters(item: &Item) -> Vec<(u16, u16, usize)> {
    match item {
        Item::Header { .. } => Vec::new(),
        Item::Verse { verse, .. } => vec![(
            verse.surah,
            verse.ayah,
            tracker::letters_or_fallback(&verse.body),
        )],
        Item::Flow { verses, .. } => verses
            .iter()
            .map(|v| (v.surah, v.ayah, tracker::letters_or_fallback(&v.body)))
            .collect(),
    }
}

/// What a verse row needs beyond its item: the route's translation (the
/// tools' links and report), the lanes that failed, and the lane order.
struct Row {
    route: Option<eq_core::routes::TranslationRef>,
    failed: Vec<String>,
    order: Vec<String>,
}

fn render_item(item: Option<Item>, prefs: ReaderStore, row: Row) -> AnyElement {
    match item {
        None => view().into_element(),
        Some(Item::Header { surah, opener }) => {
            rows::surah_header(surah, opener.as_ref(), prefs).into_element()
        }
        Some(Item::Verse { verse, lanes }) => {
            let tools = verse_tools::verse_tools(&verse, row.route, prefs);
            let mut extra = vec![lanes::stacked_lanes(&lanes, &row.failed, &row.order, prefs)];
            extra.extend(words::verse_words(prefs.script()));
            rows::verse_row(&verse, &[], prefs, Some(tools), extra).into_element()
        }
        Some(Item::Flow { verses, .. }) => rows::flow_row(&verses, prefs).into_element(),
    }
}

/// The loading state: quiet placeholder lines where the text will be.
fn loading() -> impl IntoElement {
    view()
        .flex_1()
        .p_4()
        .gap_3()
        .child(skeleton_of(36.0))
        .child(skeleton_of(72.0))
        .child(skeleton_of(72.0))
        .child(skeleton_of(72.0))
}
