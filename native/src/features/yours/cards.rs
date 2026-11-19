//! Home cards (PARITY G6; web `lib/home/{CardTile,CardForm,ScheduleEditor,
//! TimeSelect}.svelte`, `card-copy.ts`, `card-undo.svelte.ts`,
//! `yours/cards/[id]`): pinned surah / ayah / range / zikr / azkar set /
//! collection with a tap counter, an optional target, a recite schedule and
//! stats. The tile is shared by the Yours grid (with the actions menu) and
//! the home strip (with an arrow).

use eq_core::{
    home::{
        card_stats::card_stats,
        schedule::{SCHEDULE_PRESETS, SlotState, parse_time_hhmm, slot_state_for, slots_of},
        snippets::{snippet_for_slug, snippets},
    },
    i18n::{Key, keys},
    quran::{catalogue, meta::meta},
    routes::{ReaderLink, ReaderScope, ReaderTarget, TranslationRef, route_context},
    store::{
        home::{MAX_COLLECTION_ITEMS, MAX_SCHEDULE_SLOTS},
        sessions::EntryIntent,
    },
};
use nsh_base::{SelectItem, theme};
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{
    features::{
        bookmarks::{fmt, medium_date, surah_name},
        reminders::time_select,
    },
    shell::{nav, routes::Route},
    stores::{
        home::{self, HomeData, card_id, home_data, local_offset_minutes, now_minute, now_ms},
        locale::{locale, t, t_args, tr},
    },
    ui::{
        arabic::{ArabicStyle, QuranScript, plain_ayah},
        button::{ButtonSize, ButtonVariant, button, button_with, choice_button},
        controls::progress,
        field::{field, multiline_field, numeric_field, text_field},
        icons::lucide,
        layout::{card, eyebrow, page_body, page_insets},
        popups::{destructive_menu_entry, dropdown_menu, menu_divider, menu_entry},
        select::select_field,
        text::{muted, page_heading},
        tokens,
    },
};

pub(crate) const PLUS_ICON: &str = lucide!(r#"<path d="M5 12h14"/><path d="M12 5v14"/>"#);
const CHECK_ICON: &str = lucide!(r#"<path d="M20 6 9 17l-5-5"/>"#);
const ARROW_ICON: &str = lucide!(r#"<path d="M5 12h14"/><path d="m12 5 7 7-7 7"/>"#);
const RANGE_SPAN_MAX: i64 = 50;
const TARGET_MAX: i64 = 10_000;

// --- copy (card-copy.ts) -------------------------------------------------------

fn s<'a>(card: &'a Value, key: &str) -> Option<&'a str> {
    card.get(key).and_then(Value::as_str)
}

fn n(card: &Value, key: &str) -> Option<i64> {
    card.get(key).and_then(Value::as_i64)
}

pub(crate) fn kind_of(card: &Value) -> &str {
    s(card, "kind").unwrap_or_default()
}

fn kind_key(kind: &str) -> Key {
    match kind {
        "surah" => keys::READER_CARDS_KIND_SURAH,
        "ayah" => keys::READER_CARDS_KIND_AYAH,
        "range" => keys::READER_CARDS_KIND_RANGE,
        "zikr" => keys::READER_CARDS_KIND_ZIKR,
        "snippet" => keys::READER_CARDS_KIND_SNIPPET,
        _ => keys::READER_CARDS_KIND_COLLECTION,
    }
}

/// `ctxForSavedSource`: a catalogue translation, else the Arabic reader.
fn ctx_for(source_id: Option<&str>) -> Option<TranslationRef> {
    let id = source_id?;
    catalogue::translation(id)?;
    route_context(id)
}

fn source_label(source_id: Option<&str>) -> String {
    source_id
        .and_then(catalogue::translation)
        .map(|t| t.name.clone())
        .unwrap_or_default()
}

pub(crate) fn snippet_title(slug: &str) -> String {
    match snippet_for_slug(slug) {
        Some(sn) if locale().code() == "ar" => sn.title.ar.clone(),
        Some(sn) => sn.title.en.clone(),
        None => slug.to_owned(),
    }
}

fn join_meta(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ")
}

fn reader(surah: i64, ayah: Option<i64>, ctx: Option<TranslationRef>) -> Option<Route> {
    let surah = u16::try_from(surah).ok()?;
    meta().surah(surah)?;
    Some(Route::Reader(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(surah),
        translation: ctx,
        ayah: ayah.and_then(|a| u16::try_from(a).ok()),
    })))
}

/// Where a card (or one collection item) opens, plus the session entry a
/// card-origin visit carries (`cardEntryAttrs`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CardLink {
    pub route: Route,
    pub entry: EntryIntent,
}

fn item_link(card_ref: &str, item: &Value) -> Option<CardLink> {
    let source = s(item, "sourceId");
    let entry = |scope: String, bound_end: Option<String>| EntryIntent {
        origin: "card".into(),
        reference: Some(card_ref.to_owned()),
        scope: Some(scope),
        bound_end,
        ..EntryIntent::default()
    };
    if let Some(slug) = s(item, "slug") {
        return Some(CardLink {
            route: Route::Zikr {
                slug: slug.to_owned(),
                translation: ctx_for(source),
            },
            entry: entry(format!("snippet:{slug}"), None),
        });
    }
    let surah = n(item, "surah")?;
    match (n(item, "ayahStart"), n(item, "ayahEnd")) {
        (Some(a), Some(b)) => Some(CardLink {
            route: reader(surah, Some(a), ctx_for(source))?,
            entry: entry(
                format!("range:{surah}:{a}-{surah}:{b}"),
                Some(format!("{surah}:{b}")),
            ),
        }),
        _ => Some(CardLink {
            route: reader(surah, None, ctx_for(source))?,
            entry: entry(format!("surah:{surah}"), None),
        }),
    }
}

/// The link a card's title opens; `None` for a zikr phrase.
pub(crate) fn card_link(card: &Value) -> Option<CardLink> {
    let id = card_id(card);
    match kind_of(card) {
        "zikr" => None,
        "collection" => item_link(id, card.get("items")?.as_array()?.first()?),
        "ayah" => {
            let (surah, ayah) = (n(card, "surah")?, n(card, "ayah")?);
            let key = format!("{surah}:{ayah}");
            Some(CardLink {
                route: reader(surah, Some(ayah), ctx_for(s(card, "sourceId")))?,
                entry: EntryIntent {
                    origin: "card".into(),
                    reference: Some(id.to_owned()),
                    scope: Some(format!("range:{key}-{key}")),
                    bound_end: Some(key),
                    ..EntryIntent::default()
                },
            })
        }
        _ => item_link(id, card),
    }
}

/// Opens a link: a zikr page reads its card entry on arrival.
pub(crate) fn open_link(link: CardLink) {
    home::set_card_entry(link.entry);
    nav::push(link.route);
}

fn item_label(item: &Value) -> String {
    if let Some(slug) = s(item, "slug") {
        return snippet_title(slug);
    }
    let surah = n(item, "surah").unwrap_or(0);
    match (n(item, "ayahStart"), n(item, "ayahEnd")) {
        (Some(a), Some(b)) => format!("{} · {surah}:{a}–{b}", surah_name(surah)),
        _ => surah_name(surah),
    }
}

/// Title, Arabic title, meta line (`tileCopyFor`).
pub(crate) struct TileCopy {
    pub title: String,
    pub arabic: bool,
    pub meta: String,
}

pub(crate) fn tile_copy(card: &Value) -> TileCopy {
    let kind = kind_of(card);
    let kind_label = tr(kind_key(kind)).to_owned();
    let source = source_label(s(card, "sourceId"));
    let surah = n(card, "surah").unwrap_or(0);
    match kind {
        "zikr" => TileCopy {
            title: s(card, "text").unwrap_or_default().to_owned(),
            arabic: true,
            meta: kind_label,
        },
        "snippet" => TileCopy {
            title: snippet_title(s(card, "slug").unwrap_or_default()),
            arabic: false,
            meta: join_meta(&[kind_label, source]),
        },
        "collection" => {
            let items = card
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let first = items.first();
            let title = match first {
                Some(f) if f.get("slug").is_some() => {
                    snippet_title(s(f, "slug").unwrap_or_default())
                }
                Some(f) => match (n(f, "ayahStart"), n(f, "ayahEnd")) {
                    (Some(a), Some(b)) => format!("{}:{a}–{b}", n(f, "surah").unwrap_or(0)),
                    _ => surah_name(n(f, "surah").unwrap_or(0)),
                },
                None => kind_label.clone(),
            };
            let count = items.len();
            let items_label = if count == 1 {
                tr(keys::READER_CARDS_ITEMS_ONE).to_owned()
            } else {
                fmt(
                    keys::READER_CARDS_ITEMS_MANY,
                    &[("count", count.to_string().into())],
                )
            };
            let source = source_label(first.and_then(|f| s(f, "sourceId")));
            TileCopy {
                title,
                arabic: false,
                meta: join_meta(&[kind_label, items_label, source]),
            }
        }
        "ayah" => TileCopy {
            title: surah_name(surah),
            arabic: false,
            meta: join_meta(&[
                kind_label,
                format!("{surah}:{}", n(card, "ayah").unwrap_or(0)),
                source,
            ]),
        },
        "range" => TileCopy {
            title: surah_name(surah),
            arabic: false,
            meta: join_meta(&[
                kind_label,
                format!(
                    "{surah}:{}–{}",
                    n(card, "ayahStart").unwrap_or(0),
                    n(card, "ayahEnd").unwrap_or(0)
                ),
                source,
            ]),
        },
        // A surah card's meta is "Surah" alone unless it pins a translation:
        // the tile hides a meta that only restates the title.
        _ => TileCopy {
            title: surah_name(surah),
            arabic: false,
            meta: source,
        },
    }
}

/// The card's rows for the detail page (`cardContents`).
fn card_contents(card: &Value) -> Vec<(String, Option<CardLink>)> {
    let id = card_id(card);
    match kind_of(card) {
        "collection" => card
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|i| (item_label(i), item_link(id, i)))
            .collect(),
        "zikr" => vec![(s(card, "text").unwrap_or_default().to_owned(), None)],
        "snippet" => vec![(
            snippet_title(s(card, "slug").unwrap_or_default()),
            card_link(card),
        )],
        _ => {
            let copy = tile_copy(card);
            let label = match kind_of(card) {
                "surah" => copy.title,
                _ => format!(
                    "{} · {}",
                    copy.title,
                    copy.meta.split(" · ").nth(1).unwrap_or_default()
                ),
            };
            vec![(label, card_link(card))]
        }
    }
}

/// `scheduleBadgeCopy`: (due, label).
pub(crate) fn badge_of(card: &Value) -> Option<(bool, String)> {
    if eq_core::home::next_zikr::is_target_done(card) {
        return None;
    }
    match slot_state_for(&slots_of(card), now_minute())? {
        SlotState::Due(_) => Some((true, tr(keys::READER_CARDS_DUE).to_owned())),
        SlotState::Later { minutes_until, .. } if minutes_until < 60 => Some((
            false,
            fmt(
                keys::READER_CARDS_NEXT_MINUTES,
                &[("minutes", minutes_until.to_string().into())],
            ),
        )),
        SlotState::Later { minutes_until, .. } => Some((
            false,
            fmt(
                keys::READER_CARDS_NEXT_HOURS,
                &[("hours", (minutes_until / 60).to_string().into())],
            ),
        )),
    }
}

// --- the tile (CardTile.svelte) ----------------------------------------------

fn icon(markup: &'static str, size: f32) -> impl IntoElement {
    svg(nsh_core::element::ImageSrc::svg_markup(markup)).size(px(size))
}

fn badge_pill(due: bool, label: String) -> impl IntoElement {
    view()
        .h(px(24.0))
        .flex_none()
        .items_center()
        .px_2()
        .rounded(tokens::radius_pill)
        .border_1()
        .border_color(move || {
            if due {
                tokens::border_strong()
            } else {
                theme().border
            }
        })
        .child(
            text(label)
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || {
                    if due {
                        theme().primary
                    } else {
                        tokens::muted_text()
                    }
                }),
        )
}

/// The count hero: "40 of 33" never shows; past the target the count
/// stands alone with "Target reached".
pub(crate) fn count_line(count: i64, target: Option<i64>, of_key: Key) -> impl IntoElement {
    let done = target.is_some_and(|t| count >= t);
    view()
        .w_full()
        .flex_row()
        .items_end()
        .justify_between()
        .gap_2()
        .child(
            view()
                .flex_row()
                .items_end()
                .gap_1p5()
                .child(
                    text(count.to_string())
                        .text_size(px(26.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(move || theme().foreground),
                )
                .child(target.filter(|_| !done).map(|t| {
                    let label = if of_key == keys::READER_ZIKR_OF_TARGET {
                        fmt(of_key, &[("target", t.to_string().into())])
                    } else {
                        format!("of {t}") // i18n: none (web CardTile literal)
                    };
                    text(label).text_sm().text_color(tokens::muted_text)
                })),
        )
        .child(done.then(|| {
            view()
                .flex_row()
                .items_center()
                .gap_1()
                .text_color(move || theme().primary)
                .child(icon(CHECK_ICON, 14.0))
                .child(
                    text(t(keys::READER_CARDS_TARGET_DONE))
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM),
                )
        }))
}

pub(crate) fn target_bar(count: i64, target: Option<i64>) -> Option<AnyElement> {
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    target.map(|t| progress(((count as f64 / t.max(1) as f64).min(1.0)) as f32).into_element())
}

/// "Recite once": quiet until the target is met, then a soft primary wash.
pub(crate) fn recite_button(id: String, done: bool, name: String) -> impl IntoElement {
    let variant = if done {
        ButtonVariant::Secondary
    } else {
        ButtonVariant::Outline
    };
    let _ = name;
    view().w_full().child(
        button_with(
            t(keys::READER_CARDS_PLUS_ONE),
            variant,
            ButtonSize::Medium,
            false,
            None,
            move || {
                home::increment_card(id.clone());
            },
        )
        .w_full(),
    )
}

fn card_menu(card: Value, on_edit: Option<fn(Value)>, full: Signal<bool>) -> impl IntoElement {
    let id = card_id(&card).to_owned();
    let count = n(&card, "count").unwrap_or(0);
    let (open_id, dup_id, reset_id) = (id.clone(), id.clone(), id);
    let edit_card = card.clone();
    let mut items: Vec<AnyElement> = Vec::new();
    items.push(
        menu_entry(t(keys::READER_CARDS_OPEN), move || {
            nav::push(Route::YoursCard(open_id.clone()))
        })
        .into_element(),
    );
    if let Some(edit) = on_edit {
        items.push(
            menu_entry(t(keys::READER_CARDS_EDIT_TITLE), move || {
                edit(edit_card.clone())
            })
            .into_element(),
        );
    }
    items.push(
        menu_entry(t(keys::READER_CARDS_DUPLICATE), move || {
            home::duplicate_card(dup_id.clone(), move |ok| full.set(!ok));
        })
        .into_element(),
    );
    items.push(menu_divider().into_element());
    if count > 0 {
        items.push(
            menu_entry(t(keys::READER_CARDS_RESET), move || {
                home::reset_card(reset_id.clone())
            })
            .into_element(),
        );
    }
    items.push(
        destructive_menu_entry(t(keys::READER_CARDS_REMOVE), move || {
            home::remove_card(card.clone())
        })
        .into_element(),
    );
    dropdown_menu("⋯", items)
}

/// One card tile. `on_edit`: the Yours grid's actions menu (with Edit);
/// `None` with `menu = false`: the home strip's arrow.
pub(crate) fn card_tile(card: Value, menu: bool, on_edit: Option<fn(Value)>) -> AnyElement {
    let copy = tile_copy(&card);
    let count = n(&card, "count").unwrap_or(0);
    let target = n(&card, "target");
    let done = target.is_some_and(|t| count >= t);
    let link = card_link(&card);
    let badge = badge_of(&card);
    let meta_visible = !copy.meta.is_empty();
    let duplicate_full = signal(false);
    let title: AnyElement = match (&link, copy.arabic) {
        (Some(l), _) => {
            let l = l.clone();
            nsh_base::button()
                .on_press(move || open_link(l.clone()))
                .accessibility_label(copy.title.clone())
                .child(
                    text(copy.title.clone())
                        .text_base()
                        .font_weight(FontWeight::MEDIUM)
                        .truncate()
                        .text_color(move || theme().foreground),
                )
                .into_element()
        }
        (None, true) => plain_ayah(copy.title.clone(), None, || {
            ArabicStyle::of(QuranScript::Uthmani)
        })
        .text_color(move || theme().foreground)
        .into_element(),
        (None, false) => text(copy.title.clone())
            .text_base()
            .truncate()
            .into_element(),
    };
    let trailing: Option<AnyElement> = match (menu, link) {
        (true, _) => Some(card_menu(card.clone(), on_edit, duplicate_full).into_element()),
        (false, Some(l)) => Some(
            crate::ui::button::icon_svg_button(
                ARROW_ICON,
                tr(keys::READER_WIDGET_DAILY_AYAH_OPEN),
                move || open_link(l.clone()),
            )
            .into_element(),
        ),
        (false, None) => None,
    };
    view()
        .w_full()
        .flex_col()
        .gap_1()
        .p_4()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || {
            if done {
                tokens::border_strong()
            } else {
                theme().border
            }
        })
        .bg(tokens::surface)
        .child(
            view()
                .w_full()
                .flex_row()
                .items_start()
                .gap_2()
                .child(view().flex_1().min_w(px(0.0)).child(title))
                .child(trailing),
        )
        .child(when(
            move || duplicate_full.get(),
            || {
                text(t(keys::READER_CARDS_FULL))
                    .text_xs()
                    .text_color(tokens::danger)
                    .into_element()
            },
        ))
        .child((meta_visible || badge.is_some()).then(|| {
            view()
                .w_full()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    text(if meta_visible {
                        copy.meta.clone()
                    } else {
                        String::new()
                    })
                    .text_xs()
                    .truncate()
                    .text_color(tokens::muted_text),
                )
                .child(badge.map(|(due, label)| badge_pill(due, label)))
        }))
        .child(count_line(count, target, keys::READER_CARDS_TARGET_LABEL))
        .child(target_bar(count, target))
        .child(
            view()
                .pt_2()
                .child(recite_button(card_id(&card).to_owned(), done, copy.title)),
        )
        .into_element()
}

// --- the form (CardForm.svelte + ScheduleEditor.svelte) ---------------------

const ZIKR_PRESETS: [(&str, &str); 5] = [
    ("سُبْحَانَ اللَّهِ", "Subhanallah"),
    ("الْحَمْدُ لِلَّهِ", "Alhamdulillah"),
    ("اللَّهُ أَكْبَرُ", "Allahu Akbar"),
    ("لَا إِلَٰهَ إِلَّا اللَّهُ", "La ilaha illa Allah"),
    ("أَسْتَغْفِرُ اللَّهَ", "Astaghfirullah"),
];

const KINDS: [&str; 6] = ["surah", "ayah", "range", "zikr", "snippet", "collection"];

fn slot_key(label: &str) -> Option<Key> {
    match label {
        "Fajr" => Some(keys::READER_CARDS_SLOT_FAJR),
        "Dhuhr" => Some(keys::READER_CARDS_SLOT_DHUHR),
        "Asr" => Some(keys::READER_CARDS_SLOT_ASR),
        "Maghrib" => Some(keys::READER_CARDS_SLOT_MAGHRIB),
        "Isha" => Some(keys::READER_CARDS_SLOT_ISHA),
        "Morning" => Some(keys::READER_CARDS_SLOT_MORNING),
        "Evening" => Some(keys::READER_CARDS_SLOT_EVENING),
        _ => None,
    }
}

fn slot_label(label: &str) -> String {
    slot_key(label).map_or_else(|| label.to_owned(), |k| tr(k).to_owned())
}

#[derive(Clone, Copy, PartialEq)]
struct SlotRow {
    key: u64,
    id: Signal<String>,
    label: Signal<String>,
    time: Signal<String>,
}

#[derive(Clone, Copy, PartialEq)]
struct ItemRow {
    key: u64,
    snippet_mode: Signal<bool>,
    surah: Signal<Option<SharedString>>,
    start: Signal<String>,
    end: Signal<String>,
    slug: Signal<Option<SharedString>>,
}

fn next_key() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn slot_row(id: String, label: String, time: String) -> SlotRow {
    SlotRow {
        key: next_key(),
        id: signal(id),
        label: signal(label),
        time: signal(time),
    }
}

fn item_row(item: Option<&Value>) -> ItemRow {
    let opt = |v: Option<String>| v.map(SharedString::from);
    let num = |k: &str| {
        item.and_then(|i| n(i, k))
            .map(|x| x.to_string())
            .unwrap_or_default()
    };
    ItemRow {
        key: next_key(),
        snippet_mode: signal(item.is_some_and(|i| i.get("slug").is_some())),
        surah: signal(opt(item.and_then(|i| n(i, "surah")).map(|x| x.to_string()))),
        start: signal(num("ayahStart")),
        end: signal(num("ayahEnd")),
        slug: signal(opt(item.and_then(|i| s(i, "slug")).map(str::to_owned))),
    }
}

fn surah_items() -> Vec<SelectItem> {
    meta()
        .surahs()
        .iter()
        .map(|s| SelectItem::new(s.num.to_string(), format!("{}. {}", s.num, s.name)))
        .collect()
}

fn snippet_items() -> Vec<SelectItem> {
    snippets()
        .iter()
        .map(|sn| SelectItem::new(sn.slug.clone(), snippet_title(&sn.slug)))
        .collect()
}

fn translation_items() -> Vec<SelectItem> {
    let mut out = vec![SelectItem::new(
        "",
        tr(keys::READER_CARDS_TRANSLATION_ARABIC),
    )];
    out.extend(
        catalogue::translations()
            .iter()
            .filter(|t| !t.is_tafsir())
            .map(|t| SelectItem::new(t.id.clone(), format!("{} · {}", t.language, t.name))),
    );
    out
}

fn bounded(raw: &str, min: i64, max: i64) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok().filter(|v| (min..=max).contains(v))
}

fn ayah_max(surah: i64) -> i64 {
    crate::features::bookmarks::ayah_count(surah).max(1)
}

fn opt_str(sig: Signal<Option<SharedString>>) -> Option<String> {
    sig.get_untracked()
        .map(|v| v.to_string())
        .filter(|v| !v.is_empty())
}

fn small_button(
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

fn schedule_editor(slots: Signal<Vec<SlotRow>>) -> impl IntoElement {
    let add = move |label: String, time: String| {
        if slots.with_untracked(Vec::len) >= MAX_SCHEDULE_SLOTS {
            return;
        }
        let id = format!("slot-{}-{}", now_ms(), next_key());
        slots.update(|v| v.push(slot_row(id, label, time)));
    };
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(eyebrow(t(keys::READER_CARDS_SCHEDULE)))
        .child(when(
            move || slots.with(Vec::is_empty),
            || muted(t(keys::READER_CARDS_SCHEDULE_EMPTY)).into_element(),
        ))
        .child(dynamic(move || {
            if slots.with(Vec::len) >= MAX_SCHEDULE_SLOTS {
                return muted(t(keys::READER_CARDS_SCHEDULE_FULL)).into_element();
            }
            view()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(SCHEDULE_PRESETS.map(|(label, time)| {
                    small_button(
                        SharedString::from(format!("{} {time}", slot_label(label))),
                        move || {
                            add(slot_label(label), time.to_owned());
                        },
                    )
                    .into_element()
                }))
                .child(small_button(
                    t(keys::READER_CARDS_SCHEDULE_ADD),
                    move || add(String::new(), "08:00".into()),
                ))
                .into_element()
        }))
        .child(each(
            move || slots.get(),
            |row| row.key,
            move |row, _| {
                let row = row.get_untracked();
                view()
                    .w_full()
                    .flex_row()
                    .flex_wrap()
                    .items_end()
                    .gap_2()
                    .child(view().min_w(px(140.0)).flex_1().child(field(
                        t(keys::READER_CARDS_SCHEDULE_NAME),
                        text_field(row.label, ""),
                    )))
                    .child(field(
                        t(keys::READER_CARDS_SCHEDULE_TIME),
                        time_select(
                            row.time,
                            tr(keys::READER_CARDS_SCHEDULE_HOUR),
                            tr(keys::READER_CARDS_SCHEDULE_MINUTE),
                            || (),
                        ),
                    ))
                    .child(small_button(
                        t(keys::READER_CARDS_SCHEDULE_REMOVE),
                        move || {
                            slots.update(|v| v.retain(|r| r.key != row.key));
                        },
                    ))
            },
        ))
}

fn collection_builder(items: Signal<Vec<ItemRow>>) -> impl IntoElement {
    let mode_items = || {
        vec![
            SelectItem::new("ref", tr(keys::READER_CARDS_COLLECTION_REF)),
            SelectItem::new("snippet", tr(keys::READER_CARDS_COLLECTION_SNIPPET)),
        ]
    };
    view()
        .w_full()
        .flex_col()
        .gap_2()
        .child(eyebrow(t(keys::READER_CARDS_COLLECTION_ITEMS)))
        .child(each(
            move || items.get(),
            |row| row.key,
            move |row, _| {
                let row = row.get_untracked();
                let mode = signal(Some(SharedString::from(
                    if row.snippet_mode.get_untracked() {
                        "snippet"
                    } else {
                        "ref"
                    },
                )));
                effect(move || {
                    let snippet = mode.get().as_deref() == Some("snippet");
                    untracked(move || row.snippet_mode.set(snippet));
                });
                let shift = move |by: i64| {
                    items.update(|v| {
                        let Some(i) = v.iter().position(|r| r.key == row.key) else {
                            return;
                        };
                        let j = i as i64 + by;
                        if j >= 0 && (j as usize) < v.len() {
                            v.swap(i, j as usize);
                        }
                    });
                };
                view()
                    .w_full()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded(tokens::radius_md)
                    .border_1()
                    .border_color(move || theme().border)
                    .child(
                        view()
                            .w_full()
                            .flex_row()
                            .flex_wrap()
                            .items_end()
                            .gap_2()
                            .child(view().min_w(px(160.0)).child(field(
                                t(keys::READER_CARDS_ITEM_KIND),
                                select_field(
                                    tr(keys::READER_CARDS_ITEM_KIND),
                                    mode_items(),
                                    mode,
                                    "",
                                ),
                            )))
                            .child(view().flex_1())
                            .child(small_button("↑", move || shift(-1)))
                            .child(small_button("↓", move || shift(1)))
                            .child(small_button(t(keys::READER_CARDS_ITEM_REMOVE), move || {
                                items.update(|v| v.retain(|r| r.key != row.key));
                            })),
                    )
                    .child(
                        when(
                            move || row.snippet_mode.get(),
                            move || {
                                select_field(
                                    tr(keys::READER_CARDS_PICK_SNIPPET),
                                    snippet_items(),
                                    row.slug,
                                    tr(keys::READER_CARDS_PICK_SNIPPET),
                                )
                                .into_element()
                            },
                        )
                        .otherwise(move || {
                            view()
                                .w_full()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(view().min_w(px(200.0)).flex_1().child(select_field(
                                    tr(keys::READER_CARDS_PICK_SURAH),
                                    surah_items(),
                                    row.surah,
                                    tr(keys::READER_CARDS_PICK_SURAH),
                                )))
                                .child(view().w(px(110.0)).child(numeric_field(
                                    row.start,
                                    tr(keys::READER_CARDS_RANGE_START),
                                )))
                                .child(view().w(px(110.0)).child(numeric_field(
                                    row.end,
                                    tr(keys::READER_CARDS_RANGE_END),
                                )))
                                .into_element()
                        }),
                    )
            },
        ))
        .child(dynamic(move || {
            if items.with(Vec::len) >= MAX_COLLECTION_ITEMS {
                return muted(t(keys::READER_CARDS_COLLECTION_FULL)).into_element();
            }
            small_button(t(keys::READER_CARDS_COLLECTION_ADD), move || {
                items.update(|v| v.push(item_row(None)));
            })
            .into_element()
        }))
}

/// The add / edit form. `initial`: the card being edited (its kind is
/// locked). `on_close` runs after Cancel and after a successful Save.
pub(crate) fn card_form(
    initial: Option<Value>,
    on_close: impl Fn() + Copy + 'static,
) -> impl IntoElement {
    let editing = initial.is_some();
    let init = initial.clone().unwrap_or(Value::Null);
    let kind = signal(kind_of(&init).to_owned());
    if kind.get_untracked().is_empty() {
        kind.set("surah".into());
    }
    let num = |k: &str| n(&init, k).map(|x| x.to_string()).unwrap_or_default();
    let surah = signal(n(&init, "surah").map(|x| SharedString::from(x.to_string())));
    let ayah = signal(num("ayah"));
    let start = signal(num("ayahStart"));
    let end = signal(num("ayahEnd"));
    let zikr = signal(s(&init, "text").unwrap_or_default().to_owned());
    let first_slug = snippets()
        .first()
        .map(|sn| sn.slug.clone())
        .unwrap_or_default();
    let slug = signal(Some(SharedString::from(
        s(&init, "slug").map_or(first_slug, str::to_owned),
    )));
    let items_init: Vec<ItemRow> = init
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|i| item_row(Some(i)))
        .collect();
    let items = signal(items_init);
    let slots = signal(
        slots_of(&init)
            .into_iter()
            .map(|sl| slot_row(sl.id, sl.label, sl.time))
            .collect::<Vec<_>>(),
    );
    let target = signal(num("target"));
    let source_init = match kind_of(&init) {
        "collection" => init
            .get("items")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|i| s(i, "sourceId"))
            .map(str::to_owned),
        _ => s(&init, "sourceId").map(str::to_owned),
    };
    let source = signal(Some(SharedString::from(source_init.unwrap_or_default())));
    let error = signal::<Option<Key>>(None);

    let build = move || -> Result<Value, Key> {
        let target_raw = target.get_untracked();
        let target_v = match target_raw.trim() {
            "" => None,
            raw => Some(bounded(raw, 1, TARGET_MAX).ok_or(keys::READER_CARDS_INVALID_RANGE)?),
        };
        let source_id = opt_str(source);
        let mut schedule = Vec::new();
        for row in slots.get_untracked() {
            let time = parse_time_hhmm(row.time.get_untracked().trim())
                .ok_or(keys::READER_CARDS_SCHEDULE_INVALID)?;
            let label = row.label.get_untracked().trim().to_owned();
            let label = if label.is_empty() {
                time.clone()
            } else {
                label
            };
            schedule.push(json!({ "id": row.id.get_untracked(), "label": label, "time": time }));
        }
        let base = untracked(|| init.clone());
        let mut card = json!({
            "id": card_id(&base),
            "count": n(&base, "count").unwrap_or(0),
        });
        if let Some(c) = s(&base, "createdAt") {
            card["createdAt"] = json!(c);
        }
        if let Some(ev) = base.get("events") {
            card["events"] = ev.clone();
        }
        if let Some(tv) = target_v {
            card["target"] = json!(tv);
        }
        if !schedule.is_empty() {
            card["schedule"] = json!(schedule);
        }
        let k = kind.get_untracked();
        card["kind"] = json!(k);
        match k.as_str() {
            "zikr" => {
                let text = zikr.get_untracked().trim().to_owned();
                if text.is_empty() {
                    return Err(keys::READER_CARDS_ZIKR_TEXT);
                }
                card["text"] = json!(text);
            }
            "snippet" => {
                card["slug"] = json!(opt_str(slug).unwrap_or_default());
                card["sourceId"] = json!(source_id);
            }
            "collection" => {
                let mut built = Vec::new();
                for row in items.get_untracked() {
                    if row.snippet_mode.get_untracked() {
                        if let Some(sl) = opt_str(row.slug) {
                            built.push(json!({ "slug": sl, "sourceId": source_id }));
                        }
                        continue;
                    }
                    let Some(sn) = opt_str(row.surah).and_then(|v| v.parse::<i64>().ok()) else {
                        continue;
                    };
                    let (a, b) = (row.start.get_untracked(), row.end.get_untracked());
                    if a.trim().is_empty() && b.trim().is_empty() {
                        built.push(json!({ "surah": sn, "sourceId": source_id }));
                        continue;
                    }
                    let max = ayah_max(sn);
                    match (bounded(&a, 1, max), bounded(&b, 1, max)) {
                        (Some(a), Some(b)) if a <= b && b - a < RANGE_SPAN_MAX => {
                            built.push(json!({ "surah": sn, "ayahStart": a, "ayahEnd": b, "sourceId": source_id }));
                        }
                        _ => return Err(keys::READER_CARDS_INVALID_RANGE),
                    }
                }
                if built.is_empty() {
                    return Err(keys::READER_CARDS_COLLECTION_EMPTY);
                }
                card["items"] = json!(built);
            }
            other => {
                let sn = opt_str(surah)
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or(keys::READER_CARDS_NEEDS_SURAH)?;
                card["surah"] = json!(sn);
                card["sourceId"] = json!(source_id);
                let max = ayah_max(sn);
                if other == "ayah" {
                    card["ayah"] = json!(
                        bounded(&ayah.get_untracked(), 1, max)
                            .ok_or(keys::READER_CARDS_INVALID_AYAH)?
                    );
                }
                if other == "range" {
                    match (
                        bounded(&start.get_untracked(), 1, max),
                        bounded(&end.get_untracked(), 1, max),
                    ) {
                        (Some(a), Some(b)) if a <= b && b - a < RANGE_SPAN_MAX => {
                            card["ayahStart"] = json!(a);
                            card["ayahEnd"] = json!(b);
                        }
                        _ => return Err(keys::READER_CARDS_INVALID_RANGE),
                    }
                }
            }
        }
        Ok(card)
    };

    let submit = move || {
        error.set(None);
        let card = match build() {
            Ok(c) => c,
            Err(k) => {
                error.set(Some(k));
                return;
            }
        };
        if editing {
            home::replace_card(card);
            on_close();
            return;
        }
        home::add_card(card, move |added| match added {
            Some(_) => on_close(),
            None => error.set(Some(keys::READER_CARDS_FULL)),
        });
    };

    let kind_is = move |k: &'static str| move || kind.get() == k;
    view()
        .w_full()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .bg(tokens::surface)
        .child(
            text(t(if editing {
                keys::READER_CARDS_EDIT_TITLE
            } else {
                keys::READER_CARDS_ADD_TITLE
            }))
            .text_sm()
            .font_weight(FontWeight::MEDIUM),
        )
        .child(if editing {
            muted(t(keys::READER_CARDS_KIND_LOCKED)).into_element()
        } else {
            view()
                .flex_row()
                .flex_wrap()
                .gap_1p5()
                .children(KINDS.map(|k| {
                    choice_button(
                        t(kind_key(k)),
                        tr(kind_key(k)),
                        move || kind.get() == k,
                        false,
                        move || {
                            if kind.get_untracked() != k {
                                kind.set(k.into());
                                error.set(None);
                            }
                        },
                    )
                    .into_element()
                }))
                .into_element()
        })
        .child(when(
            move || matches!(kind.get().as_str(), "surah" | "ayah" | "range"),
            move || {
                field(
                    t(keys::READER_CARDS_PICK_SURAH),
                    select_field(
                        tr(keys::READER_CARDS_PICK_SURAH),
                        surah_items(),
                        surah,
                        tr(keys::READER_CARDS_PICK_SURAH),
                    ),
                )
                .into_element()
            },
        ))
        .child(when(kind_is("ayah"), move || {
            field(t(keys::READER_CARDS_AYAH_NUMBER), numeric_field(ayah, "")).into_element()
        }))
        .child(when(kind_is("range"), move || {
            view()
                .flex_row()
                .gap_2()
                .child(view().flex_1().child(field(
                    t(keys::READER_CARDS_RANGE_START),
                    numeric_field(start, ""),
                )))
                .child(view().flex_1().child(field(
                    t(keys::READER_CARDS_RANGE_END),
                    numeric_field(end, ""),
                )))
                .into_element()
        }))
        .child(when(kind_is("zikr"), move || {
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_1p5()
                        .children(ZIKR_PRESETS.map(|(ar, label)| {
                            choice_button(
                                label,
                                label,
                                move || zikr.get() == ar,
                                false,
                                move || zikr.set(ar.to_owned()),
                            )
                            .into_element()
                        })),
                )
                .child(field(
                    t(keys::READER_CARDS_ZIKR_TEXT),
                    multiline_field(zikr, ""),
                ))
                .into_element()
        }))
        .child(when(kind_is("snippet"), move || {
            field(
                t(keys::READER_CARDS_PICK_SNIPPET),
                select_field(
                    tr(keys::READER_CARDS_PICK_SNIPPET),
                    snippet_items(),
                    slug,
                    tr(keys::READER_CARDS_PICK_SNIPPET),
                ),
            )
            .into_element()
        }))
        .child(when(kind_is("collection"), move || {
            collection_builder(items).into_element()
        }))
        .child(when(
            move || kind.get() != "zikr",
            move || {
                field(
                    t(keys::READER_CARDS_TRANSLATION),
                    select_field(
                        tr(keys::READER_CARDS_TRANSLATION),
                        translation_items(),
                        source,
                        tr(keys::READER_CARDS_TRANSLATION_ARABIC),
                    ),
                )
                .into_element()
            },
        ))
        .child(field(
            t(keys::READER_CARDS_TARGET_LABEL),
            numeric_field(target, ""),
        ))
        .child(schedule_editor(slots))
        .child(dynamic(move || {
            error.get().map(|k| {
                text(t(k))
                    .text_sm()
                    .text_color(tokens::danger)
                    .into_element()
            })
        }))
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(button(t(keys::READER_CARDS_SAVE), submit))
                .child(button_with(
                    t(keys::READER_CARDS_CANCEL),
                    ButtonVariant::Ghost,
                    ButtonSize::Medium,
                    false,
                    None,
                    on_close,
                )),
        )
}

// --- the Yours section ----------------------------------------------------------

/// Which form is open: none, a new card, or the card being edited.
#[derive(Debug, Clone, PartialEq, Default)]
enum FormState {
    #[default]
    Closed,
    New,
    Edit(Value),
}

fn form_signal() -> Signal<FormState> {
    global::<FormState>()
}

fn edit_card(card: Value) {
    form_signal().set(FormState::Edit(card));
}

/// The removal undo banner (shared with the detail page's removal).
pub(crate) fn undo_banner() -> impl IntoElement {
    let undo = home::undo_state();
    dynamic(move || {
        let state = undo.get();
        state.card.as_ref()?;
        Some(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_3()
                .px_4()
                .py(px(10.0))
                .rounded(tokens::radius_xl)
                .border_1()
                .border_color(tokens::border_strong)
                .bg(tokens::background_subtle)
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            text(t(keys::READER_CARDS_REMOVED))
                                .text_sm()
                                .text_color(tokens::foreground_secondary),
                        )
                        .child(state.failed.then(|| {
                            text(t(keys::READER_CARDS_FULL))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(tokens::danger)
                        })),
                )
                .child(button_with(
                    t(keys::READER_BOOKMARKS_UNDO),
                    ButtonVariant::Secondary,
                    ButtonSize::Small,
                    false,
                    None,
                    home::undo_removal,
                ))
                .into_element(),
        )
    })
}

/// The grid of tiles, `limit` of them.
pub(crate) fn card_grid(data: Signal<HomeData>, limit: usize, menu: bool) -> impl IntoElement {
    view().w_full().flex_row().flex_wrap().gap_2().child(each(
        move || data.with(|d| d.cards.iter().take(limit).cloned().collect::<Vec<_>>()),
        |c| format!("{}:{}", card_id(c), c),
        move |c, _| {
            view().min_w(px(240.0)).flex_1().child(card_tile(
                c.get_untracked(),
                menu,
                menu.then_some(edit_card as fn(Value)),
            ))
        },
    ))
}

/// The cards section of Yours (and home's cards widget): count, undo
/// banner, the open form, the grid, Add card.
pub(crate) fn cards_section(trailing: Option<AnyElement>) -> impl IntoElement {
    let data = home_data();
    let form = form_signal();
    form.set(FormState::Closed);
    let close = move || form.set(FormState::Closed);
    let count = move || data.with(|d| d.cards.len());
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
                .child(eyebrow(t(keys::READER_CARDS_TITLE)))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(
                            text(move || SharedString::from(count().to_string()))
                                .text_xs()
                                .text_color(tokens::muted_text),
                        )
                        .child(trailing),
                ),
        )
        .child(undo_banner())
        .child(dynamic(move || match form.get() {
            FormState::Closed if data.with(HomeData::full) => {
                Some(muted(t(keys::READER_CARDS_FULL)).into_element())
            }
            FormState::Closed => None,
            FormState::New => Some(card_form(None, close).into_element()),
            FormState::Edit(c) => Some(card_form(Some(c), close).into_element()),
        }))
        .child(dynamic(move || {
            let closed = form.with(|f| *f == FormState::Closed);
            if count() == 0 {
                return closed.then(|| {
                    card()
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(px(14.0))
                                .child(
                                    view()
                                        .size(px(44.0))
                                        .flex_none()
                                        .items_center()
                                        .justify_center()
                                        .rounded(tokens::radius_sm)
                                        .bg(move || tokens::hue_soft(2))
                                        .text_color(move || tokens::hue_legible(2))
                                        .child(icon(PLUS_ICON, 20.0)),
                                )
                                .child(
                                    view().flex_1().child(
                                        text(t(keys::READER_CARDS_EMPTY))
                                            .text_sm()
                                            .text_color(tokens::foreground_secondary),
                                    ),
                                ),
                        )
                        .child(
                            view()
                                .flex_row()
                                .child(button(t(keys::READER_CARDS_ADD), move || {
                                    form.set(FormState::New)
                                })),
                        )
                        .into_element()
                });
            }
            let can_add = closed && !data.with(HomeData::full);
            Some(
                view()
                    .w_full()
                    .flex_col()
                    .gap_3()
                    .child(card_grid(data, usize::MAX, true))
                    .child(can_add.then(|| {
                        view()
                            .flex_row()
                            .child(button(t(keys::READER_CARDS_ADD), move || {
                                form.set(FormState::New)
                            }))
                    }))
                    .into_element(),
            )
        }))
}

// --- the detail page (yours/cards/[id]) ----------------------------------------

fn stat(label: Key, value: String) -> impl IntoElement {
    view()
        .min_w(px(120.0))
        .flex_1()
        .flex_col()
        .gap_1()
        .p_3()
        .rounded(tokens::radius_md)
        .border_1()
        .border_color(move || theme().border)
        .child(eyebrow(t(label)))
        .child(
            text(value)
                .text_xl()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || theme().foreground),
        )
}

fn streak_days(count: i64) -> String {
    let key = if count == 1 {
        keys::READER_CARDS_STREAK_DAY
    } else {
        keys::READER_CARDS_STREAK_DAYS
    };
    fmt(key, &[("count", count.to_string().into())])
}

fn activity_strip(days: Vec<(String, i64)>) -> impl IntoElement {
    let max = days.iter().map(|d| d.1).max().unwrap_or(0).max(1);
    view()
        .w_full()
        .h(px(48.0))
        .flex_row()
        .items_end()
        .gap(px(3.0))
        .children(days.into_iter().map(move |(day, c)| {
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            let h = if c == 0 {
                3.0
            } else {
                6.0 + 42.0 * (c as f32 / max as f32)
            };
            view()
                .flex_1()
                .h(px(h))
                .rounded(tokens::radius_sm)
                .bg(move || {
                    if c == 0 {
                        theme().muted
                    } else {
                        theme().primary
                    }
                })
                .label(format!("{day}: {c}"))
                .into_element()
        }))
}

fn detail(card: Value, editing: Signal<bool>, duplicate_failed: Signal<bool>) -> AnyElement {
    let copy = tile_copy(&card);
    let id = card_id(&card).to_owned();
    let count = n(&card, "count").unwrap_or(0);
    let target = n(&card, "target");
    let done = target.is_some_and(|t| count >= t);
    let stats = card_stats(&card, now_ms(), local_offset_minutes());
    let badge = badge_of(&card);
    let slots = slots_of(&card);
    let (dup_id, reset_id) = (id.clone(), id.clone());
    let remove_snapshot = card.clone();
    let edit_snapshot = card.clone();
    let title: AnyElement = if copy.arabic {
        plain_ayah(copy.title.clone(), None, || {
            ArabicStyle::of(QuranScript::Uthmani)
        })
        .into_element()
    } else {
        page_heading(SharedString::from(copy.title.clone())).into_element()
    };
    let last = stats
        .last_recited_at
        .map_or_else(|| tr(keys::READER_CARDS_NEVER).to_owned(), medium_date);
    view()
        .w_full()
        .flex_col()
        .gap_5()
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_1()
                .child(title)
                .child((!copy.meta.is_empty()).then(|| {
                    text(copy.meta.clone())
                        .text_sm()
                        .text_color(tokens::muted_text)
                }))
                .child(badge.map(|(due, label)| view().flex_row().child(badge_pill(due, label)))),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(small_button(t(keys::READER_CARDS_EDIT_TITLE), move || {
                    editing.set(!editing.get_untracked())
                }))
                .child(small_button(t(keys::READER_CARDS_DUPLICATE), move || {
                    home::duplicate_card(dup_id.clone(), move |ok| duplicate_failed.set(!ok));
                }))
                .child(button_with(
                    t(keys::READER_CARDS_RESET),
                    ButtonVariant::Outline,
                    ButtonSize::Small,
                    count == 0,
                    None,
                    move || {
                        home::reset_card(reset_id.clone());
                    },
                ))
                .child(button_with(
                    t(keys::READER_CARDS_REMOVE),
                    ButtonVariant::Danger,
                    ButtonSize::Small,
                    false,
                    None,
                    move || {
                        home::remove_card(remove_snapshot.clone());
                        nav::pop();
                    },
                )),
        )
        .child(when(
            move || duplicate_failed.get(),
            || {
                text(t(keys::READER_CARDS_FULL))
                    .text_sm()
                    .text_color(tokens::danger)
                    .into_element()
            },
        ))
        .child(when(
            move || editing.get(),
            move || {
                card_form(Some(edit_snapshot.clone()), move || editing.set(false)).into_element()
            },
        ))
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(stat(
                    keys::READER_CARDS_STATS_TOTAL,
                    stats.total.to_string(),
                ))
                .child(stat(
                    keys::READER_CARDS_STATS_TODAY,
                    stats.today.to_string(),
                ))
                .child(stat(keys::READER_CARDS_STATS_WEEK, stats.last7.to_string()))
                .child(stat(
                    keys::READER_CARDS_STATS_MONTH,
                    stats.last30.to_string(),
                )),
        )
        .child(
            view()
                .w_full()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(stat(
                    keys::READER_CARDS_STREAK_CURRENT,
                    streak_days(stats.current_streak),
                ))
                .child(stat(
                    keys::READER_CARDS_STREAK_BEST,
                    streak_days(stats.best_streak),
                ))
                .child(stat(keys::READER_CARDS_LAST_RECITED, last)),
        )
        .child(count_line(count, target, keys::READER_CARDS_TARGET_LABEL))
        .child(target_bar(count, target))
        .child(recite_button(id, done, copy.title.clone()))
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(eyebrow(t(keys::READER_CARDS_ACTIVITY)))
                .child(activity_strip(stats.activity30)),
        )
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(eyebrow(t(keys::READER_CARDS_CONTENTS)))
                .children(card_contents(&card).into_iter().map(|(label, link)| {
                    match link {
                        Some(l) => button_with(
                            SharedString::from(label),
                            ButtonVariant::Outline,
                            ButtonSize::Small,
                            false,
                            None,
                            move || open_link(l.clone()),
                        )
                        .into_element(),
                        None => text(label).text_sm().into_element(),
                    }
                })),
        )
        .child(
            view()
                .w_full()
                .flex_col()
                .gap_2()
                .child(eyebrow(t(keys::READER_CARDS_SCHEDULE)))
                .child(
                    slots
                        .is_empty()
                        .then(|| muted(t(keys::READER_CARDS_SCHEDULE_EMPTY))),
                )
                .children(slots.into_iter().map(|sl| {
                    view()
                        .flex_row()
                        .justify_between()
                        .gap_3()
                        .child(text(sl.label).text_sm())
                        .child(text(sl.time).text_sm().text_color(tokens::muted_text))
                        .into_element()
                })),
        )
        .into_element()
}

/// `yours/cards/[id]`.
pub(crate) fn card_page(id: String) -> impl IntoElement {
    let data = home_data();
    let editing = signal(false);
    let duplicate_failed = signal(false);
    scroll().flex_1().content_inset(page_insets(0.0)).child(
        page_body()
            .child(view().flex_row().child(button_with(
                t(keys::READER_CARDS_BACK),
                ButtonVariant::Ghost,
                ButtonSize::Small,
                false,
                None,
                nav::pop,
            )))
            .child(undo_banner())
            .child(dynamic(move || {
                if !data.with(|d| d.loaded) {
                    return muted("…").into_element();
                }
                match data.with(|d| d.card(&id).cloned()) {
                    Some(c) => detail(c, editing, duplicate_failed),
                    None => muted(t(keys::READER_CARDS_NOT_FOUND)).into_element(),
                }
            }))
            .text_color(move || theme().foreground),
    )
}

/// `t_args` re-export for the home strip's "{extra} more" line.
pub(crate) fn strip_more(extra: usize) -> impl Fn() -> SharedString + 'static {
    t_args(keys::READER_CARDS_STRIP_MORE, move || {
        vec![("extra", extra.to_string().into())]
    })
}
