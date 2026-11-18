//! The UI-locale preference and the locale-suggestion state, as pure
//! functions over [`KeyValue`].
//!
//! Web (`locale-suggest.svelte.ts`): the URL is the locale; once per page
//! load, on surfaces with an `/ar` twin, the browser's preferred language
//! (`extractLocaleFromNavigator`) is offered when it is a supported locale
//! other than the current one and the user has not already answered for it.
//! Answers live under `easyquran.localeSuggest` as
//! `{"v":1,"resolved":{"ar":"accepted"|"dismissed"}}`.
//!
//! Native has no URL, so the chosen locale is a stored preference
//! ([`LOCALE_PREF_KEY`]); without one the app follows the system language
//! ([`locale_from_languages`]). The suggestion keeps the web's key, schema
//! and decode rules.

use serde_json::{Map, Value};

use super::Locale;
use crate::kv::{KeyValue, KvError};

/// Where the user's explicit UI-locale choice is kept (`"en"` / `"ar"`).
/// Absent: follow the system language.
pub const LOCALE_PREF_KEY: &str = "easyquran.locale";

/// The web's suggestion key (`LOCALE_SUGGEST_STORAGE_KEY`).
pub const LOCALE_SUGGEST_STORAGE_KEY: &str = "easyquran.localeSuggest";

const SCHEMA_VERSION: u64 = 1;

/// The explicit locale choice, if any (an unknown stored value reads as
/// none).
pub fn load_locale_pref(kv: &dyn KeyValue) -> Option<Locale> {
    kv.get(LOCALE_PREF_KEY)
        .and_then(|code| Locale::from_code(&code))
}

/// Keeps `locale` as the explicit choice.
pub fn save_locale_pref(kv: &dyn KeyValue, locale: Locale) -> Result<(), KvError> {
    kv.set(LOCALE_PREF_KEY, locale.code())
}

/// Forgets the explicit choice: the app follows the system language again.
pub fn clear_locale_pref(kv: &dyn KeyValue) {
    kv.remove(LOCALE_PREF_KEY);
}

/// The first supported locale among the user's preferred languages, most
/// preferred first: each tag whole, then its base language (`ar-SA` ->
/// `ar`), as paraglide's `extractLocaleFromNavigator`. `_` separators
/// (platform identifiers) count as `-`.
pub fn locale_from_languages<'a>(languages: impl IntoIterator<Item = &'a str>) -> Option<Locale> {
    languages.into_iter().find_map(|tag| {
        Locale::from_code(tag).or_else(|| {
            let base = tag.split(['-', '_']).next().unwrap_or_default();
            Locale::from_code(base)
        })
    })
}

/// How the user answered a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestResolution {
    Accepted,
    Dismissed,
}

impl SuggestResolution {
    fn as_str(self) -> &'static str {
        match self {
            SuggestResolution::Accepted => "accepted",
            SuggestResolution::Dismissed => "dismissed",
        }
    }

    fn parse(value: &Value) -> Option<Self> {
        match value.as_str()? {
            "accepted" => Some(SuggestResolution::Accepted),
            "dismissed" => Some(SuggestResolution::Dismissed),
            _ => None,
        }
    }
}

/// The answers given per locale (the web's `ResolvedMap`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SuggestResolved {
    answers: [Option<SuggestResolution>; Locale::ALL.len()],
}

impl SuggestResolved {
    /// The answer for `locale`, if one was given.
    pub fn get(&self, locale: Locale) -> Option<SuggestResolution> {
        self.answers[locale.index()]
    }

    /// Records `answer` for `locale`.
    pub fn set(&mut self, locale: Locale, answer: SuggestResolution) {
        self.answers[locale.index()] = Some(answer);
    }

    /// The web's tolerant decode (`decodeResolved`): a payload whose `v` is
    /// present and not the current schema, junk, unknown locales and unknown
    /// values all drop out.
    pub fn decode(raw: &str) -> Self {
        let mut out = Self::default();
        let Ok(value) = serde_json::from_str::<Value>(raw) else {
            return out;
        };
        if let Some(v) = value.as_object().and_then(|o| o.get("v"))
            && v.as_u64() != Some(SCHEMA_VERSION)
        {
            return out;
        }
        let Some(resolved) = value.get("resolved").and_then(Value::as_object) else {
            return out;
        };
        for locale in Locale::ALL {
            if let Some(answer) = resolved
                .get(locale.code())
                .and_then(SuggestResolution::parse)
            {
                out.set(locale, answer);
            }
        }
        out
    }

    /// The payload the web writes: `{"v":1,"resolved":{...}}`.
    pub fn encode(&self) -> String {
        let mut resolved = Map::new();
        for locale in Locale::ALL {
            if let Some(answer) = self.get(locale) {
                resolved.insert(locale.code().into(), Value::from(answer.as_str()));
            }
        }
        let mut root = Map::new();
        root.insert("v".into(), Value::from(SCHEMA_VERSION));
        root.insert("resolved".into(), Value::Object(resolved));
        Value::Object(root).to_string()
    }

    /// The stored answers.
    pub fn load(kv: &dyn KeyValue) -> Self {
        kv.get(LOCALE_SUGGEST_STORAGE_KEY)
            .map(|raw| Self::decode(&raw))
            .unwrap_or_default()
    }

    /// Records `answer` for `locale` and stores the whole map.
    pub fn resolve(
        kv: &dyn KeyValue,
        locale: Locale,
        answer: SuggestResolution,
    ) -> Result<Self, KvError> {
        let mut map = Self::load(kv);
        map.set(locale, answer);
        kv.set(LOCALE_SUGGEST_STORAGE_KEY, &map.encode())?;
        Ok(map)
    }
}

/// The locale to offer, if any: the system's preferred supported locale
/// when it differs from `current` and has no recorded answer (the web's
/// `evaluate`, less its URL and modal conditions, which are the caller's).
pub fn suggest_locale<'a>(
    system_languages: impl IntoIterator<Item = &'a str>,
    current: Locale,
    resolved: &SuggestResolved,
) -> Option<Locale> {
    let suggested = locale_from_languages(system_languages)?;
    if suggested == current || resolved.get(suggested).is_some() {
        return None;
    }
    Some(suggested)
}
