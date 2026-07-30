//! Shared Arabic search normalization (§7.1).
//!
//! One specification, used to build the corpus AND to normalize queries, so
//! online and offline match identically. The frozen rule set is identified by
//! [`crate::quran::SEARCH_VERSION`] = `arabic-search-v1`; changing any rule
//! here bumps that label and invalidates search ETags (§8.1).

/// Normalize `input` for substring search.
///
/// Returns the normalized string plus a map from each OUTPUT char index to its
/// SOURCE char index, so a hit found in the normalized text can be mapped back
/// to the (un-normalized, verbatim) response text for highlighting (§7.1).
///
/// Rules (§7.1; §11.1 ta-marbuta resolved ON):
/// - drop Arabic combining marks (harakat, Quranic signs, tatweel);
/// - fold hamza-bearing alefs (`0622`/`0623`/`0625`) AND superscript alef
///   (`0670`) → bare alef (`0627`) — `0670` carries a real alef in many verses,
///   so it folds (not drops);
/// - fold alef-wasla (`0671`) → alef;
/// - fold alef-maqsura (`0649`) → ya (`064A`);
/// - fold ta-marbuta (`0629`) → ha (`0647`);
/// - collapse whitespace runs to one space and trim both ends.
pub fn normalize_arabic(input: &str) -> (String, Vec<u32>) {
    let mut out = String::with_capacity(input.len());
    let mut map = Vec::with_capacity(input.chars().count());
    let mut prev_was_space = true; // trim leading whitespace
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
        '\u{064B}'..='\u{0658}' // tanwin/fathatan… + harakat + madda/hamza/subscript/inverted/noon-ghunna
            | '\u{0640}' // TATWEEL/kashida — orthographic elongation only (6,848 in Uthmani); drop so a Uthmani-highlight re-normalization still substring-matches the needle (§7.3).
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E8}'
            | '\u{06E9}' // ARABIC PLACE OF SAJDA ۩ — in 15 Uthmani verses; drop (§7.1 "remove Quranic signs").
            | '\u{06EA}'..='\u{06ED}' // Quranic annotation signs
    )
}

fn fold(ch: char) -> char {
    match ch {
        // Alef variants → bare alef. U+0670 (superscript alef) usually carries a
        // real alef (e.g. "العٰلمين"); folding it → alef keeps Uthmani highlights
        // correct for that common case. (It is genuinely ambiguous — in "الرحمن"
        // it is a reading marker — so a few Uthmani highlights may still be empty;
        // results are unaffected, being computed over simple-clean, §7.3.)
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' | '\u{0670}' => '\u{0627}',
        '\u{0649}' => '\u{064A}', // alef-maqsura → ya
        '\u{0629}' => '\u{0647}', // ta-marbuta → ha (§11.1)
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Normalization is idempotent-ish and trims/collapses whitespace.
    #[test]
    fn normalizes_basics() {
        let (s, _) = normalize_arabic("  ٱلْحَمْدُ   لِلَّهِ  ");
        assert!(!s.starts_with(' ') && !s.ends_with(' '), "trimmed: {s:?}");
        assert!(!s.contains("  "), "whitespace collapsed: {s:?}");
    }

    /// Ta-marbuta folds to ha (§11.1, resolved ON).
    #[test]
    fn folds_ta_marbuta() {
        let (s, _) = normalize_arabic("مَرْحَبَة");
        assert!(s.contains('\u{0647}'), "ta-marbuta→ha: {s:?}");
        assert!(!s.contains('\u{0629}'));
    }

    /// Hamza-bearing alefs and alef-maqsura fold.
    #[test]
    fn folds_alef_variants() {
        let (s, _) = normalize_arabic("إِنَّ آيَةً فِي سُورَة");
        assert!(!s.contains('\u{0622}'), "alef-madda folded: {s:?}");
        assert!(!s.contains('\u{0623}') && !s.contains('\u{0625}'), "hamza-alefs folded");
        assert!(!s.contains('\u{0671}'), "alef-wasla folded");
    }

    /// Harakat are removed.
    #[test]
    fn drops_harakat() {
        let (s, _) = normalize_arabic("بِسْمِ");
        // ب س م only — no diacritics.
        assert_eq!(s.chars().filter(|c| *c != ' ').count(), 3);
    }

    /// The output→source map is exactly as long as the output.
    #[test]
    fn map_length_matches_output() {
        let (s, map) = normalize_arabic("ٱلْحَمْدُ لِلَّهِ");
        assert_eq!(s.chars().count(), map.len());
    }
}
