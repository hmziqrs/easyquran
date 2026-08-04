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
            | '\u{0670}' // SUPERSCRIPT ALEF (category Mn): drop to match the web's /\p{Mn}/u so online/offline agree (v2; was fold→alef).
            | '\u{0640}' // TATWEEL: drop so a Uthmani re-normalization still substring-matches the needle.
            | '\u{06D6}'..='\u{06DC}'
            | '\u{06DF}'..='\u{06E8}'
            | '\u{06E9}' // ARABIC PLACE OF SAJDA — lone Quranic sign between ranges; dropped here.
            | '\u{06EA}'..='\u{06ED}'
    )
}

fn fold(ch: char) -> char {
    match ch {
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => '\u{0627}',
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
        assert!(
            !s.contains('\u{0623}') && !s.contains('\u{0625}'),
            "hamza-alefs folded"
        );
        assert!(!s.contains('\u{0671}'), "alef-wasla folded");
    }

    #[test]
    fn drops_harakat() {
        let (s, _) = normalize_arabic("بِسْمِ");
        assert_eq!(s.chars().filter(|c| *c != ' ').count(), 3);
    }

    #[test]
    fn drops_superscript_alef() {
        // U+0670 (superscript alef) is category Mn — dropped, not folded to bare alef, so Rust matches the web's /\p{Mn}/u.
        // Input: alef-wasla, lam, ra, shadda, fatha, ha, meem, sukun, superscript-alef, nun (ٱلرَّحْمَٰن).
        let (s, _) = normalize_arabic(
            "\u{0671}\u{0644}\u{0631}\u{0651}\u{0654}\u{062D}\u{0645}\u{0652}\u{0670}\u{0646}",
        );
        assert!(!s.contains('\u{0670}'), "U+0670 dropped: {s:?}");
        // Expected: bare-alef lam ra ha meem nun (الرحمن), identical to the web's normalizeArabic output.
        assert_eq!(s, "\u{0627}\u{0644}\u{0631}\u{062D}\u{0645}\u{0646}");
    }

    #[test]
    fn map_length_matches_output() {
        let (s, map) = normalize_arabic("ٱلْحَمْدُ لِلَّهِ");
        assert_eq!(s.chars().count(), map.len());
    }
}
