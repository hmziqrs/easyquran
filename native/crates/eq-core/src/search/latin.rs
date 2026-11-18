//! Latin-script normalisation for translation search
//! (`quran/search/normalize-latin.ts`): lower case, NFD with marks
//! stripped, typographic quotes and dashes folded to ASCII, final sigma
//! folded, whitespace collapsed. Index copies and queries only.

use unicode_normalization::char::{decompose_canonical, is_combining_mark};

use super::normalize::{
    MAX_QUERY_LEN, MIN_QUERY_LEN, NormalizedMap, finish, push_unit, scalar_len,
};

fn ascii_fold(c: char) -> char {
    match c {
        '‘' | '’' | '‚' | '‛' => '\'',
        '“' | '”' | '„' | '‟' => '"',
        '‐' | '‑' | '‒' | '–' | '—' | '―' | '−' => '-',
        'ς' => 'σ',
        other => other,
    }
}

/// `normalizeLatinWithMap`; spans are byte offsets of the verbatim input.
pub fn normalize_latin_with_map(input: &str) -> NormalizedMap {
    let mut values: Vec<(char, u32, u32)> = Vec::with_capacity(input.len());
    for (at, c) in input.char_indices() {
        let start = at as u32;
        let end = (at + c.len_utf8()) as u32;
        for lowered in c.to_lowercase() {
            let mut decomposed = Vec::with_capacity(2);
            decompose_canonical(lowered, |d| decomposed.push(d));
            for d in decomposed {
                if is_combining_mark(d) {
                    if let Some(last) = values.last_mut() {
                        last.2 = end;
                    }
                    continue;
                }
                push_unit(&mut values, ascii_fold(d), start, end);
            }
        }
    }
    finish(values)
}

/// `normalizeLatin`.
pub fn normalize_latin(input: &str) -> String {
    normalize_latin_with_map(input).normalized
}

/// `isEligibleLatinQuery`: 3..=64 scalars.
pub fn is_eligible_latin_query(norm: &str) -> bool {
    let len = scalar_len(norm);
    (MIN_QUERY_LEN..=MAX_QUERY_LEN).contains(&len)
}

/// `containsArabicScript`: any char of the Arabic blocks.
pub fn contains_arabic_script(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '\u{0600}'..='\u{06FF}'
                | '\u{0750}'..='\u{077F}'
                | '\u{08A0}'..='\u{08FF}'
                | '\u{FB50}'..='\u{FDFF}'
                | '\u{FE70}'..='\u{FEFF}'
        )
    })
}
