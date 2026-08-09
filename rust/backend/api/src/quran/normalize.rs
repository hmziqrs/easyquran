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

pub fn contains_searchable_ornament(s: &str) -> bool {
    // The standalone Quranic ornaments kept by the normalizer (06D6–06DC, 06DE–06ED; 06DD is
    // stripped). A query built around one of these is eligible below MIN_QUERY_LEN so a lone
    // mark search works like quran.com (۞→199, ۩→15) without relaxing the 3-char floor for text.
    s.chars()
        .any(|c| matches!(c, '\u{06D6}'..='\u{06DC}' | '\u{06DE}'..='\u{06ED}'))
}

fn is_combining_mark(ch: char) -> bool {
    // quran.com parity: the index strips intra-cluster combining marks (harakat, maddah
    // U+0653/U+0654, tatweel, superscript alef) so a bare query still matches Uthmani's
    // decomposed alef-madda. Standalone Quranic ornaments (U+06D6–U+06DC rub-el-hizb U+06DE,
    // small waw/yeh U+06E5/U+06E6, sajda U+06E9, signs U+06EA–U+06ED) are KEPT — they are
    // searchable tokens, matching quran.com (۞→199, ۩→15). See docs/quran-system.md (Normalization).
    matches!(
        ch,
        '\u{064B}'..='\u{0658}' // harakat + maddah U+0653 / hamza-above U+0654 (intra-cluster).
            | '\u{0670}' // SUPERSCRIPT ALEF (consonant-chair role).
            | '\u{0640}' // TATWEEL.
            | '\u{06DD}' // END OF AYAH (0 occurrences in our corpora; stripped for quran.com parity).
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
        // U+0670 (superscript alef, consonant-chair role) is stripped, not folded to bare alef.
        // Input: alef-wasla, lam, ra, shadda, hamza-above, ha, meem, sukun, superscript-alef, nun (ٱلرَّحْمَٰن).
        let (s, _) = normalize_arabic(
            "\u{0671}\u{0644}\u{0631}\u{0651}\u{0654}\u{062D}\u{0645}\u{0652}\u{0670}\u{0646}",
        );
        assert!(!s.contains('\u{0670}'), "U+0670 dropped: {s:?}");
        // Expected: bare-alef lam ra ha meem nun (الرحمن) — shadda/hamza-above/sukun/superscript-alef all stripped; wasla folded.
        assert_eq!(s, "\u{0627}\u{0644}\u{0631}\u{062D}\u{0645}\u{0646}");
    }

    #[test]
    fn map_length_matches_output() {
        let (s, map) = normalize_arabic("ٱلْحَمْدُ لِلَّهِ");
        assert_eq!(s.chars().count(), map.len());
    }

    #[derive(serde::Deserialize)]
    struct ParityNormalizeCase {
        input: String,
        expected: String,
    }

    #[derive(serde::Deserialize)]
    struct ParityCorpus {
        normalize: Vec<ParityNormalizeCase>,
    }

    #[test]
    fn normalize_parity_corpus() {
        let json = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../web/src/lib/quran/__fixtures__/parity.json"
        ));
        let corpus: ParityCorpus = serde_json::from_str(json).expect("parity.json must parse");
        assert!(!corpus.normalize.is_empty(), "corpus must carry normalize cases");
        for case in &corpus.normalize {
            let (got, _) = normalize_arabic(&case.input);
            assert_eq!(got, case.expected, "parity case input={:?}", case.input);
        }
    }
}
