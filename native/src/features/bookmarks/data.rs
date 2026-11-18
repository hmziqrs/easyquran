//! What the bookmark and Yours screens share: the bookmark shelf read from
//! the sync store, verse routes, copy helpers, and the feedback bar that
//! stands in for the web's action toasts (Undo, Change).

use std::{rc::Rc, time::Duration};

use eq_core::{
    i18n::{self, Arg, Key, keys},
    quran::meta::meta,
    routes::{ReaderLink, ReaderScope, ReaderTarget, TranslationRef},
    store::bookmarks::{self, Bookmark, BookmarkFolder},
    sync::clock::ms_from_iso,
};
use nsh_base::theme;
use nsh_core::prelude::*;

use crate::{
    app::sync_store,
    shell::{nav, routes::Route},
    stores::{locale::locale, sync::Area},
    ui::{
        button::{ButtonSize, ButtonVariant, button_with},
        tokens,
    },
};

/// The sync store, also from listeners and tasks that run without the
/// tree's context: the handle is kept surface-wide the first time it is found.
pub(crate) fn store() -> crate::stores::sync::SyncStore {
    let cache = global::<Option<crate::stores::sync::SyncStore>>();
    if let Some(found) = crate::stores::sync::sync_store() {
        if cache.with_untracked(Option::is_none) {
            cache.set(Some(found));
        }
        return found;
    }
    cache.get_untracked().unwrap_or_else(sync_store)
}

/// `tr_args` in the current locale, now.
pub(crate) fn fmt(key: Key, args: &[(&str, Arg<'_>)]) -> String {
    i18n::tr_args(locale(), key, args)
}

/// Epoch ms now.
pub(crate) fn now_ms() -> i64 {
    nsh_core::time::unix_ms() as i64
}

/// The surah's display name (`quranData.surahByNum(n).name`, else `Surah n`).
pub(crate) fn surah_name(num: i64) -> String {
    u16::try_from(num)
        .ok()
        .and_then(|n| meta().surah(n))
        .map_or_else(|| format!("Surah {num}"), |s| s.name.clone())
}

/// The surah's verse count, 0 when unknown.
pub(crate) fn ayah_count(num: i64) -> i64 {
    u16::try_from(num)
        .ok()
        .and_then(|n| meta().surah(n))
        .map_or(0, |s| i64::from(s.ayah_count))
}

/// `ctxForSavedSource`: a saved translation still in the catalogue reopens
/// in that translation, anything else in the Arabic reader.
pub(crate) fn saved_ctx(source_id: Option<&str>) -> Option<TranslationRef> {
    let id = source_id?;
    eq_core::quran::catalogue::translation(id)?;
    eq_core::routes::route_context(id)
}

/// The reader at a verse of a surah, in `ctx`.
pub(crate) fn verse_route(surah: i64, ayah: i64, ctx: Option<TranslationRef>) -> Option<Route> {
    let surah = u16::try_from(surah).ok()?;
    let ayah = u16::try_from(ayah).ok()?;
    Some(Route::Reader(ReaderLink::new(ReaderTarget {
        scope: ReaderScope::Surah(surah),
        translation: ctx,
        ayah: Some(ayah),
    })))
}

/// Opens the reader at a verse in its saved translation (else Arabic).
pub(crate) fn open_verse(surah: i64, ayah: i64, source_id: Option<&str>) {
    if let Some(route) = verse_route(surah, ayah, saved_ctx(source_id)) {
        nav::push(route);
    }
}

/// "Saved 3 days ago" from an ISO stamp; `None` for an empty or bad stamp.
pub(crate) fn saved_label(iso: &str) -> Option<String> {
    let at = ms_from_iso(iso)?;
    let days = ((now_ms() - at).max(0)) / 86_400_000;
    let when = i18n::format_past(locale(), days, i18n::TimeUnit::Day);
    Some(fmt(keys::READER_BOOKMARKS_SAVED, &[("when", when.into())]))
}

/// `Intl.DateTimeFormat(locale, { dateStyle: "medium" })` for epoch ms.
pub(crate) fn medium_date(ms: i64) -> String {
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const AR: [&str; 12] = [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ];
    let (y, m, d) = civil_from_days(ms.div_euclid(86_400_000));
    let at = usize::try_from(m - 1).unwrap_or(0).min(11);
    match locale() {
        i18n::Locale::Ar => format!("{d} {} {y}", AR[at]),
        i18n::Locale::En => format!("{} {d}, {y}", EN[at]),
    }
}

/// Days since the epoch -> (year, month 1..=12, day 1..=31), proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// The translation's catalogue name, else "Arabic" (`sourceLabel`).
pub(crate) fn source_label(source_id: Option<&str>) -> String {
    source_id
        .and_then(eq_core::quran::catalogue::translation)
        .map_or_else(
            || i18n::tr(locale(), keys::READER_NOTES_SOURCE_ARABIC).to_owned(),
            |t| t.name.clone(),
        )
}

// --- the shelf -------------------------------------------------------------

/// The bookmark domain as the screens show it.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Shelf {
    pub folders: Vec<BookmarkFolder>,
    pub bookmarks: Vec<Bookmark>,
    pub pending: usize,
    pub rejected: usize,
    pub capped: usize,
    pub loaded: bool,
}

impl Shelf {
    pub fn count_in(&self, folder_id: &str) -> usize {
        self.bookmarks
            .iter()
            .filter(|b| b.folder_id == folder_id)
            .count()
    }

    pub fn folder(&self, id: &str) -> Option<&BookmarkFolder> {
        self.folders.iter().find(|f| f.id == id)
    }
}

/// A shelf that re-reads whenever the bookmark area changes. Call while
/// building a page (its effect lives with the page).
pub(crate) fn shelf() -> Signal<Shelf> {
    let shelf = signal(Shelf::default());
    let sync = store();
    let seq = stored(0_u64);
    effect(move || {
        let _ = sync.version(Area::Bookmarks);
        if !sync.ready() {
            return;
        }
        let ticket = seq.with(|n| {
            *n += 1;
            *n
        });
        untracked(|| {
            sync.run(
                |store| {
                    Ok(Shelf {
                        folders: bookmarks::folders(store)?,
                        bookmarks: bookmarks::bookmarks(store)?,
                        pending: bookmarks::pending(store)?,
                        rejected: bookmarks::rejected_count(store)?,
                        capped: bookmarks::capped_count(store),
                        loaded: true,
                    })
                },
                move |out| {
                    // A newer read is on its way: this one is stale.
                    if seq.try_with(|n| *n != ticket).unwrap_or(true) {
                        return;
                    }
                    match out {
                        Ok(next) => {
                            let _ = shelf.try_set(next);
                        }
                        Err(e) => log::warn!("bookmarks read failed: {e}"),
                    }
                },
            );
        });
    });
    shelf
}

/// A bookmark write; the area's version moves after it, so every shelf
/// re-reads. `then` gets the job's value when it succeeded.
pub(crate) fn write<T: Send + 'static>(
    job: impl FnOnce(&mut eq_core::store::AppStore) -> Result<T, eq_core::store::StoreError>
    + Send
    + 'static,
    then: impl FnOnce(T) + 'static,
) {
    store().write(Area::Bookmarks, job, move |out| match out {
        Ok(value) => then(value),
        Err(e) => {
            log::warn!("bookmark write failed: {e}");
            crate::ui::toast::notify(i18n::tr(locale(), keys::READER_BOOKMARKS_SAVE_ERROR));
        }
    });
}

// --- feedback --------------------------------------------------------------

/// One message with one action (Undo, Change): the web's action toast.
#[derive(Clone)]
pub(crate) struct Offer {
    id: u64,
    message: SharedString,
    action_label: SharedString,
    action: Rc<dyn Fn()>,
}

#[derive(Default, Clone)]
struct OfferSlot {
    next: u64,
    current: Option<Offer>,
}

fn slot() -> Signal<OfferSlot> {
    global::<OfferSlot>()
}

/// How long an offer stays.
const OFFER_FOR: Duration = Duration::from_secs(6);

/// Shows `message` with one action in the feedback bar; a new offer
/// replaces the last one (one toast slot, as on the web).
pub(crate) fn offer(
    message: impl Into<SharedString>,
    action_label: impl Into<SharedString>,
    action: impl Fn() + 'static,
) {
    let mut id = 0;
    slot().update(|s| {
        s.next += 1;
        id = s.next;
        s.current = Some(Offer {
            id,
            message: message.into(),
            action_label: action_label.into(),
            action: Rc::new(action),
        });
    });
    let _ = set_timeout(OFFER_FOR, move || {
        if slot().with_untracked(|s| s.current.as_ref().is_some_and(|o| o.id == id)) {
            slot().update(|s| s.current = None);
        }
    });
}

fn take_offer() -> Option<Offer> {
    let current = slot().with_untracked(|s| s.current.clone());
    if current.is_some() {
        slot().update(|s| s.current = None);
    }
    current
}

/// The bar that shows the current offer (message + action) at its place in
/// the page. Mount once per surface that makes offers: the bookmarks page
/// and Yours do; the reader mounts it next to its verse actions.
pub fn feedback_bar() -> impl IntoElement {
    let shown = move || slot().with(|s| s.current.is_some());
    when(shown, move || {
        let message = move || {
            slot().with(|s| {
                s.current
                    .as_ref()
                    .map(|o| o.message.clone())
                    .unwrap_or_default()
            })
        };
        let label = move || {
            slot().with(|s| {
                s.current
                    .as_ref()
                    .map(|o| o.action_label.clone())
                    .unwrap_or_default()
            })
        };
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
                text(message)
                    .text_sm()
                    .flex_1()
                    .text_color(tokens::foreground_secondary),
            )
            .child(button_with(
                label,
                ButtonVariant::Secondary,
                ButtonSize::Small,
                false,
                None,
                move || {
                    if let Some(offer) = take_offer() {
                        (offer.action)();
                    }
                },
            ))
            .text_color(move || theme().foreground)
    })
}
