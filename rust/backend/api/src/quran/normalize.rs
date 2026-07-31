//! One normalization spec for BOTH corpus build and queries, so online and
//! offline match identically. Changing any rule here MUST bump
//! `crate::quran::SEARCH_VERSION` or cached/ETagged results go inconsistent.

pub fn normalize_arabic(input: &str) -> (String, Vec<u32>) {
    let mut out = String::with_capacity(input.len());
    let mut map = Vec::with_capacity(input.chars().count());
    let mut prev_was_space = true;
    for (i, ch) in input.chars().enumerate() {
        if is_combining_mark(ch) {
            continue;
        }
        let folded = fold(ch);
        if folded.is_whitespace() {
            if prev_was_space {
                continue;
            }
            prev_was_space = true;
            out.push(' ');
            map.push(i as u32);
        } else {
            prev_was_space = false;
            out.push(folded);
            map.push(i as u32);
        }
    }
    if out.ends_with(' ') {
        out.pop();
        map.pop();
    }
    (out, map)
}

fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{064B}'..='\u{0658}'
            | '\u{0640}' // TATWEEL: drop so a Uthmani re-normalization still substring-matches the needle.
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E8}'
            | '\u{06E9}' // ARABIC PLACE OF SAJDA — lone Quranic sign between ranges; dropped here.
            | '\u{06EA}'..='\u{06ED}'
    )
}

fn fold(ch: char) -> char {
    match ch {
        // U+0670 (superscript alef) is ambiguous: a real alef in "العٰلمين", a
        // reading marker in "الرحمن". Fold (not drop) — folding keeps the common
        // case correct; results are over simple-clean, so unaffected.
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' | '\u{0670}' => '\u{0627}',
        '\u{0649}' => '\u{064A}',
        '\u{0629}' => '\u{0647}',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_basics() {
        let (s, _) = normalize_arabic("  ٱلْحَمْدُ   لِلَّهِ  ");
        assert!(!s.starts_with(' ') && !s.ends_with(' '), "trimmed: {s:?}");
        assert!(!s.contains("  "), "whitespace collapsed: {s:?}");
    }

    #[test]
    fn folds_ta_marbuta() {
        let (s, _) = normalize_arabic("مَرْحَبَة");
        assert!(s.contains('\u{0647}'), "ta-marbuta→ha: {s:?}");
        assert!(!s.contains('\u{0629}'));
    }

    #[test]
    fn folds_alef_variants() {
        let (s, _) = normalize_arabic("إِنَّ آيَةً فِي سُورَة");
        assert!(!s.contains('\u{0622}'), "alef-madda folded: {s:?}");
        assert!(!s.contains('\u{0623}') && !s.contains('\u{0625}'), "hamza-alefs folded");
        assert!(!s.contains('\u{0671}'), "alef-wasla folded");
    }

    #[test]
    fn drops_harakat() {
        let (s, _) = normalize_arabic("بِسْمِ");
        assert_eq!(s.chars().filter(|c| *c != ' ').count(), 3);
    }

    #[test]
    fn map_length_matches_output() {
        let (s, map) = normalize_arabic("ٱلْحَمْدُ لِلَّهِ");
        assert_eq!(s.chars().count(), map.len());
    }
}
