//! Arabic search normalisation (`quran/search/normalize.ts`). Applied to
//! index copies and queries only: the displayed text stays verbatim, and
//! the offset maps point every normalised char back at its span (byte
//! offsets) of the verbatim input.

/// Shortest plain query the text search runs (`MIN_QUERY_LEN`).
pub const MIN_QUERY_LEN: usize = 3;
/// Longest query, in scalars (`MAX_QUERY_LEN`).
pub const MAX_QUERY_LEN: usize = 64;
/// Hits per page (`DEFAULT_LIMIT`).
pub const DEFAULT_LIMIT: usize = 20;
/// Largest page (`MAX_LIMIT`).
pub const MAX_LIMIT: usize = 50;
/// Deepest page start (`MAX_OFFSET`).
pub const MAX_OFFSET: usize = 500;

const BARE_ALEF: char = 'ا';
const YA: char = 'ي';
const HA: char = 'ه';

/// A normalised string with, per normalised char, the byte span of the
/// input it came from (`starts[i]..ends[i]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NormalizedMap {
    pub normalized: String,
    pub starts: Vec<u32>,
    pub ends: Vec<u32>,
}

impl NormalizedMap {
    /// The normalised text as chars (the index space of `starts`/`ends`).
    pub fn chars(&self) -> Vec<char> {
        self.normalized.chars().collect()
    }
}

/// Harakat, tatweel, superscript alef and the end-of-ayah sign
/// (`[ً-٘ـٰ۝]`).
fn is_removed(c: char) -> bool {
    matches!(
        c,
        '\u{064B}'..='\u{0658}' | '\u{0640}' | '\u{0670}' | '\u{06DD}'
    )
}

/// Builds a map from `(value, start, end)` units, dropping a trailing
/// space.
pub(crate) fn finish(mut values: Vec<(char, u32, u32)>) -> NormalizedMap {
    if values.last().is_some_and(|v| v.0 == ' ') {
        values.pop();
    }
    let mut out = NormalizedMap {
        normalized: String::with_capacity(values.len() * 2),
        starts: Vec::with_capacity(values.len()),
        ends: Vec::with_capacity(values.len()),
    };
    for (value, start, end) in values {
        out.normalized.push(value);
        out.starts.push(start);
        out.ends.push(end);
    }
    out
}

/// Pushes `value` (already folded) with whitespace runs collapsed to one
/// space and leading whitespace dropped. Returns nothing; mutates `values`.
pub(crate) fn push_unit(values: &mut Vec<(char, u32, u32)>, value: char, start: u32, end: u32) {
    if value.is_whitespace() {
        let Some(last) = values.last_mut() else {
            return;
        };
        if last.0 == ' ' {
            last.2 = end;
            return;
        }
        values.push((' ', start, end));
        return;
    }
    values.push((value, start, end));
}

fn fold_arabic(c: char) -> char {
    match c {
        'آ' | 'أ' | 'إ' | 'ٱ' => BARE_ALEF,
        'ى' => YA,
        'ة' => HA,
        other => other,
    }
}

/// `normalizeArabicWithMap`: drops harakat/tatweel/dagger alef/U+06DD
/// (their bytes join the previous unit's span), folds alef forms, alef
/// maqsura and ta marbuta, collapses and trims whitespace.
pub fn normalize_arabic_with_map(input: &str) -> NormalizedMap {
    let mut values: Vec<(char, u32, u32)> = Vec::with_capacity(input.len() / 2);
    for (at, c) in input.char_indices() {
        let start = at as u32;
        let end = (at + c.len_utf8()) as u32;
        if is_removed(c) {
            if let Some(last) = values.last_mut() {
                last.2 = end;
            }
            continue;
        }
        push_unit(&mut values, fold_arabic(c), start, end);
    }
    finish(values)
}

/// `normalizeArabic`.
pub fn normalize_arabic(input: &str) -> String {
    normalize_arabic_with_map(input).normalized
}

/// Scalar count (`scalarLength`).
pub fn scalar_len(s: &str) -> usize {
    s.chars().count()
}

/// Quranic ornaments and stop marks, searchable on their own
/// (`[ۖ-ۜ۞-ۭ]`).
fn is_ornament(c: char) -> bool {
    matches!(c, '\u{06D6}'..='\u{06DC}' | '\u{06DE}'..='\u{06ED}')
}

/// `isEligibleQuery`: 3..=64 scalars, or shorter when it holds an
/// ornament.
pub fn is_eligible_query(norm: &str) -> bool {
    let len = scalar_len(norm);
    if len < 1 || len > MAX_QUERY_LEN {
        return false;
    }
    len >= MIN_QUERY_LEN || norm.chars().any(is_ornament)
}

/// Every non-overlapping occurrence of `needle` in `hay`, as char index
/// ranges `(first, last)` of the haystack (the JS `indexOf` loop).
pub(crate) fn char_occurrences(hay: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let needle_chars = needle.chars().count();
    let mut chars_before = 0usize;
    let mut byte_cursor = 0usize;
    for (byte, _) in hay.match_indices(needle) {
        chars_before += hay[byte_cursor..byte].chars().count();
        byte_cursor = byte;
        out.push((chars_before, chars_before + needle_chars - 1));
    }
    out
}

/// Clamps a page request (`clamp(limit, 0, MAX_LIMIT)`, offset to
/// `MAX_OFFSET`).
pub fn clamp_page(limit: usize, offset: usize) -> (usize, usize) {
    (limit.min(MAX_LIMIT), offset.min(MAX_OFFSET))
}
