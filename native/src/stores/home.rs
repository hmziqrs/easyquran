//! Home state over `eq_core::store::home` (PARITY A3, G6; web
//! `lib/home/{layout,cards,reading-choice,card-undo}.svelte.ts`).
//!
//! The local database owns the widget layout and the cards: screens read
//! them through [`home_data`] (re-read whenever the sync store's
//! `Area::Home` version moves) and change them through the write fns here,
//! which bump that version. The first-run reading choice is a device
//! preference in the key-value store (`easyquran.home.reading`). The card
//! removal undo and the pending card entry are surface-wide globals: one
//! surface arms them, another reads them.

use eq_core::{
    home::AppCatalog,
    kv::KeyValue,
    quran::catalogue,
    store::{
        AppStore, StoreError,
        home::{self, HomeWidget},
        sessions::EntryIntent,
    },
};
use nsh_core::prelude::*;
use serde_json::{Value, json};

use crate::{
    services::storage::NshKv,
    stores::sync::{Area, SyncStore},
};

const READING_KEY: &str = "easyquran.home.reading";
const UNDO_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// The layout and the cards, as last read.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HomeData {
    pub loaded: bool,
    pub layout: Vec<HomeWidget>,
    pub cards: Vec<Value>,
}

impl HomeData {
    pub fn card(&self, id: &str) -> Option<&Value> {
        self.cards.iter().find(|c| card_id(c) == id)
    }

    pub fn full(&self) -> bool {
        self.cards.len() >= home::MAX_CARDS
    }

    pub fn widget(&self, id: &str) -> Option<&HomeWidget> {
        self.layout.iter().find(|w| w.id == id)
    }
}

/// A card's id (`""` when absent).
pub fn card_id(card: &Value) -> &str {
    card.get("id").and_then(Value::as_str).unwrap_or_default()
}

fn store() -> SyncStore {
    crate::features::bookmarks::store()
}

/// Layout and cards, re-read whenever the home area changes.
pub fn home_data() -> Signal<HomeData> {
    let out = signal(HomeData::default());
    let sync = store();
    effect(move || {
        let _ = sync.version(Area::Home);
        if !sync.ready() {
            return;
        }
        untracked(|| {
            sync.run(
                |s: &mut AppStore| {
                    Ok(HomeData {
                        loaded: true,
                        layout: home::layout(s)?,
                        cards: home::cards(s, &AppCatalog)?,
                    })
                },
                move |r| match r {
                    Ok(next) => {
                        let _ = out.try_set(next);
                    }
                    Err(e) => log::warn!("home read failed: {e}"),
                },
            );
        });
    });
    out
}

/// A home write; `then` gets the job's value (`None` when it failed).
pub fn write<T: Send + 'static>(
    job: impl FnOnce(&mut AppStore) -> Result<T, StoreError> + Send + 'static,
    then: impl FnOnce(Option<T>) + 'static,
) {
    store().write(Area::Home, job, move |r| match r {
        Ok(v) => then(Some(v)),
        Err(e) => {
            log::warn!("home write failed: {e}");
            then(None);
        }
    });
}

fn fire(job: impl FnOnce(&mut AppStore) -> Result<(), StoreError> + Send + 'static) {
    write(job, |_| ());
}

// --- layout ---

pub fn toggle_widget(id: &'static str) {
    fire(move |s| home::toggle_widget(s, id));
}

pub fn move_widget(id: &'static str, direction: i64) {
    fire(move |s| home::move_widget(s, id, direction));
}

pub fn set_widget_count(id: &'static str, count: i64) {
    fire(move |s| home::set_widget_count(s, id, count));
}

// --- cards ---

/// Adds a card (it is given a fresh id when it has none); `then(Some(id))`
/// once stored, `then(None)` when refused (store full or invalid).
pub fn add_card(mut card: Value, then: impl FnOnce(Option<String>) + 'static) {
    write(
        move |s| {
            if card_id(&card).is_empty() {
                let id = home::new_card_id(s);
                card["id"] = json!(id);
            }
            if card.get("createdAt").is_none() {
                card["createdAt"] = json!(eq_core::sync::clock::iso_from_ms(s.local_now()));
            }
            let id = card_id(&card).to_owned();
            Ok(home::add_card(s, &AppCatalog, &card)?.then_some(id))
        },
        move |r| then(r.flatten()),
    );
}

pub fn replace_card(card: Value) {
    fire(move |s| home::replace_card(s, &AppCatalog, &card));
}

pub fn increment_card(id: String) {
    fire(move |s| home::increment_card(s, &id));
}

pub fn reset_card(id: String) {
    fire(move |s| home::reset_card(s, &id));
}

/// `then(false)`: refused, the store is full.
pub fn duplicate_card(id: String, then: impl FnOnce(bool) + 'static) {
    write(
        move |s| home::duplicate_card(s, &AppCatalog, &id),
        move |r| then(r.unwrap_or(false)),
    );
}

pub fn set_card_source(id: String, source_id: Option<String>) {
    fire(move |s| home::set_card_source(s, &AppCatalog, &id, source_id.as_deref()));
}

// --- removal undo (card-undo.svelte.ts) ---

/// A removal that can still be undone; `failed`: the restore was refused.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CardUndo {
    pub card: Option<Value>,
    pub failed: bool,
    generation: u64,
}

/// The pending removal (tracked).
pub fn undo_state() -> Signal<CardUndo> {
    global::<CardUndo>()
}

fn schedule_expiry(generation: u64) {
    let state = undo_state();
    set_timeout(UNDO_WINDOW, move || {
        if state.with_untracked(|u| u.generation) == generation {
            state.set(CardUndo::default());
        }
    });
}

fn arm(card: Value, failed: bool) {
    let state = undo_state();
    let generation = state.with_untracked(|u| u.generation) + 1;
    state.set(CardUndo {
        card: Some(card),
        failed,
        generation,
    });
    schedule_expiry(generation);
}

/// Optimistic remove with a 5 s undo; rapid removals keep the newest.
pub fn remove_card(card: Value) {
    let id = card_id(&card).to_owned();
    arm(card, false);
    fire(move |s| home::remove_card(s, &id));
}

/// Restores the snapshot (appended last); a full store re-arms the banner
/// with its refusal and restarts the window.
pub fn undo_removal() {
    let Some(card) = undo_state().with_untracked(|u| u.card.clone()) else {
        return;
    };
    let snapshot = card.clone();
    add_card(card, move |added| match added {
        Some(_) => undo_state().set(CardUndo {
            generation: undo_state().with_untracked(|u| u.generation) + 1,
            ..CardUndo::default()
        }),
        None => arm(snapshot, true),
    });
}

// --- pending card entry (a navigation from a card into its zikr page) ---

#[derive(Debug, Clone, Default, PartialEq)]
struct PendingEntry(Option<EntryIntent>);

/// Tags the next zikr page visit as opened from a card.
pub fn set_card_entry(intent: EntryIntent) {
    global::<PendingEntry>().set(PendingEntry(Some(intent)));
}

/// The pending entry, cleared.
pub fn take_card_entry() -> Option<EntryIntent> {
    let slot = global::<PendingEntry>();
    let taken = slot.with_untracked(|p| p.0.clone());
    if taken.is_some() {
        slot.set(PendingEntry(None));
    }
    taken
}

// --- first-run reading choice (reading-choice.svelte.ts) ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadingChoice {
    Arabic,
    Translation(String),
}

fn decode_choice(raw: &str) -> Option<ReadingChoice> {
    let v: Value = serde_json::from_str(raw).ok()?;
    if v.get("v").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    match v.get("mode").and_then(Value::as_str)? {
        "arabic" => Some(ReadingChoice::Arabic),
        "translation" => {
            let id = v.get("sourceId").and_then(Value::as_str)?;
            catalogue::translation(id).map(|_| ReadingChoice::Translation(id.to_owned()))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct ChoiceSlot {
    hydrated: bool,
    chosen: Option<ReadingChoice>,
}

/// The stored choice (tracked); `None` until the reader picks one.
pub fn reading_choice() -> Option<ReadingChoice> {
    let slot = global::<ChoiceSlot>();
    if !slot.with_untracked(|s| s.hydrated) {
        let chosen = NshKv.get(READING_KEY).and_then(|raw| decode_choice(&raw));
        slot.set(ChoiceSlot {
            hydrated: true,
            chosen,
        });
    }
    slot.with(|s| s.chosen.clone())
}

pub fn choose_reading(choice: ReadingChoice) {
    let doc = match &choice {
        ReadingChoice::Arabic => json!({ "v": 1, "mode": "arabic" }),
        ReadingChoice::Translation(id) => json!({ "v": 1, "mode": "translation", "sourceId": id }),
    };
    if let Err(e) = NshKv.set(READING_KEY, &doc.to_string()) {
        log::warn!("reading choice not saved: {e}");
    }
    global::<ChoiceSlot>().set(ChoiceSlot {
        hydrated: true,
        chosen: Some(choice),
    });
}

/// First catalogue translation of a language code.
pub fn translation_for_language(code: &str) -> Option<String> {
    catalogue::translations()
        .iter()
        .find(|t| t.language_code == code)
        .map(|t| t.id.clone())
}

/// `suggestedChoice`: the first system language with a translation (Arabic
/// first means Arabic only); Arabic only when nothing matches.
pub fn suggested_choice() -> ReadingChoice {
    for lang in locale_settings().languages.iter() {
        let primary = lang
            .as_str()
            .split(['-', '_'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if primary == "ar" {
            return ReadingChoice::Arabic;
        }
        if let Some(id) = translation_for_language(&primary) {
            return ReadingChoice::Translation(id);
        }
    }
    ReadingChoice::Arabic
}

/// The reader context a choice opens in (`continueCtx`).
pub fn choice_context(choice: Option<&ReadingChoice>) -> Option<eq_core::routes::TranslationRef> {
    match choice {
        Some(ReadingChoice::Translation(id)) => eq_core::routes::route_context(id),
        _ => None,
    }
}

// --- local time ---

/// Minutes east of UTC of the device's zone now.
pub fn local_offset_minutes() -> i64 {
    i64::from(chrono::Local::now().offset().local_minus_utc()) / 60
}

/// Epoch ms now.
pub fn now_ms() -> i64 {
    crate::features::bookmarks::now_ms()
}

/// Minute of the local day now.
pub fn now_minute() -> i64 {
    (now_ms() / 60_000 + local_offset_minutes()).rem_euclid(24 * 60)
}

/// Today's local `YYYY-MM-DD`.
pub fn today() -> String {
    eq_core::store::goals::local_day(now_ms(), local_offset_minutes())
}
