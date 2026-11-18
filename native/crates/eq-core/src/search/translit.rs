//! Transliteration folding for surah names (`quran/search/translit.ts`):
//! `Al-Baqarah`, `baqara`, `bakara` all fold to one key, and a bounded
//! optimal-string-alignment distance forgives a typo or two.

use std::sync::OnceLock;

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

use crate::quran::meta::QuranMeta;

/// Why a key is in a surah's table: a catalogue field or a curated alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    Field,
    Alias,
}

/// One folded key of a surah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslitKey {
    pub key: String,
    pub kind: KeyKind,
}

/// Leading article tokens dropped before joining (`ARTICLE_TOKENS`).
pub const ARTICLE_TOKENS: [&str; 12] = [
    "al", "aal", "el", "an", "ar", "as", "at", "az", "ad", "adh", "ash", "ath",
];

const VOWEL_FOLDS: [(&str, &str); 6] = [
    ("aa", "a"),
    ("ee", "i"),
    ("ii", "i"),
    ("oo", "u"),
    ("ou", "u"),
    ("uu", "u"),
];
const MAX_VOWEL_PASSES: usize = 3;
const MIN_BARE_ARTICLE_REMAINDER: usize = 3;

fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\'' | '’' | '`' | '.' | '_' | '-')
}

fn strip_bare_article(joined: String) -> String {
    for (prefix, width) in [("aal", 3usize), ("al", 2usize)] {
        if joined.starts_with(prefix)
            && joined.chars().count() - width >= MIN_BARE_ARTICLE_REMAINDER
        {
            return joined[width..].to_owned();
        }
    }
    joined
}

fn collapse_vowels(joined: String) -> String {
    let mut folded = joined;
    for _ in 0..MAX_VOWEL_PASSES {
        let mut next = folded.clone();
        for (from, to) in VOWEL_FOLDS {
            next = next.replace(from, to);
        }
        if next == folded {
            break;
        }
        folded = next;
    }
    folded
}

/// `translitKey`: lower case, accents off, separators split, a leading
/// article dropped, joined, a bare `al`/`aal` prefix dropped, long vowels
/// collapsed, `q` -> `k`, a final `ah` -> `a`, letters only.
pub fn translit_key(raw: &str) -> String {
    let lowered: String = raw
        .to_lowercase()
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .nfc()
        .collect();
    let tokens: Vec<&str> = lowered
        .split(is_separator)
        .filter(|t| !t.is_empty())
        .collect();
    let kept: &[&str] = match tokens.split_first() {
        Some((first, rest)) if !rest.is_empty() && ARTICLE_TOKENS.contains(first) => rest,
        _ => &tokens,
    };
    let mut key = strip_bare_article(kept.concat());
    key = collapse_vowels(key);
    key = key.replace('q', "k");
    if let Some(stem) = key.strip_suffix("ah") {
        key = format!("{stem}a");
    }
    key.chars().filter(char::is_ascii_lowercase).collect()
}

/// `osaDistance`: optimal string alignment distance of `a` and `b`, or
/// `max + 1` once it exceeds `max`.
pub fn osa_distance(a: &str, b: &str, max: usize) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return max + 1;
    }
    const INF: usize = usize::MAX / 4;
    let bl = b.len();
    let mut prev_prev = vec![INF; bl + 1];
    let mut prev = vec![INF; bl + 1];
    for (j, cell) in prev.iter_mut().enumerate().take(bl.min(max) + 1) {
        *cell = j;
    }
    for i in 1..=a.len() {
        let mut current = vec![INF; bl + 1];
        if i <= max {
            current[0] = i;
        }
        let low = 1.max(i.saturating_sub(max));
        let high = bl.min(i + max);
        let mut row_min = current[0];
        for j in low..=high {
            let substitution = prev[j - 1] + usize::from(a[i - 1] != b[j - 1]);
            let deletion = prev[j] + 1;
            let insertion = current[j - 1] + 1;
            let mut cell = substitution.min(deletion).min(insertion);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                cell = cell.min(prev_prev[j - 2] + 1);
            }
            current[j] = cell;
            row_min = row_min.min(cell);
        }
        if row_min > max {
            return max + 1;
        }
        prev_prev = prev;
        prev = current;
    }
    let distance = prev[bl];
    if distance > max { max + 1 } else { distance }
}

const MAX_FOLD_EDITS: usize = 2;
const MAX_TWO_EDIT_LENGTH_DELTA: usize = 1;

/// `maxEditsFor`: edits forgiven for a key of `len`.
pub fn max_edits_for(len: usize) -> usize {
    match len {
        0..=2 => 0,
        3..=5 => 1,
        _ => MAX_FOLD_EDITS,
    }
}

const ALIAS_EXACT_SCORE: f64 = 0.75;
const FIELD_EXACT_SCORE: f64 = 0.7;
const DISTANCE_ONE_SCORE: f64 = 0.5;
const DISTANCE_TWO_SCORE: f64 = 0.4;
const MIN_KEY_LENGTH: usize = 3;

fn needle_score(needle: &str, entry: &TranslitKey) -> f64 {
    if entry.key == needle {
        return match entry.kind {
            KeyKind::Alias => ALIAS_EXACT_SCORE,
            KeyKind::Field => FIELD_EXACT_SCORE,
        };
    }
    let (nl, kl) = (needle.chars().count(), entry.key.chars().count());
    let max = max_edits_for(nl.max(kl));
    if max == 0 {
        return 0.0;
    }
    if max == MAX_FOLD_EDITS && nl.abs_diff(kl) > MAX_TWO_EDIT_LENGTH_DELTA {
        return 0.0;
    }
    let distance = osa_distance(needle, &entry.key, max);
    if distance > max {
        return 0.0;
    }
    if distance == 1 {
        DISTANCE_ONE_SCORE
    } else {
        DISTANCE_TWO_SCORE
    }
}

/// A query's fold needles (`makeTranslitScorer`): the whole query's key
/// plus each word's.
#[derive(Debug, Clone)]
pub struct TranslitScorer {
    needles: Vec<String>,
}

impl TranslitScorer {
    /// `None` when the query folds to fewer than 3 letters.
    pub fn new(raw: &str) -> Option<Self> {
        let key = translit_key(raw);
        if key.chars().count() < MIN_KEY_LENGTH {
            return None;
        }
        let mut needles = vec![key];
        for word in raw.split_whitespace() {
            let k = translit_key(word);
            if k.chars().count() >= MIN_KEY_LENGTH && !needles.contains(&k) {
                needles.push(k);
            }
        }
        Some(Self { needles })
    }

    /// The best score of any needle against any key.
    pub fn score(&self, keys: &[TranslitKey]) -> f64 {
        let mut best = 0.0;
        for entry in keys {
            for needle in &self.needles {
                let s = needle_score(needle, entry);
                if s > best {
                    best = s;
                }
            }
        }
        best
    }
}

/// Curated surah nicknames folding cannot derive (`SURAH_KEYWORDS`).
pub const SURAH_KEYWORDS: [(&str, u16); 3] = [("tabarak", 67), ("tbarak", 67), ("yaroiatak", 75)];

/// Each surah's keys (name, transliteration, slug, deduplicated) plus its
/// alias keys, indexed by `num - 1` (`surahTranslitKeys` + `surahAliasKeys`).
pub fn surah_keys(meta: &QuranMeta) -> &'static [Vec<TranslitKey>] {
    static TABLE: OnceLock<Vec<Vec<TranslitKey>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        meta.surahs()
            .iter()
            .map(|s| {
                let mut keys: Vec<TranslitKey> = Vec::new();
                for field in [&s.name, &s.transliteration, &s.slug] {
                    let key = translit_key(field);
                    if !keys.iter().any(|k| k.key == key) {
                        keys.push(TranslitKey {
                            key,
                            kind: KeyKind::Field,
                        });
                    }
                }
                for (alias, num) in SURAH_KEYWORDS {
                    if num == s.num {
                        keys.push(TranslitKey {
                            key: translit_key(alias),
                            kind: KeyKind::Alias,
                        });
                    }
                }
                keys
            })
            .collect()
    })
}
