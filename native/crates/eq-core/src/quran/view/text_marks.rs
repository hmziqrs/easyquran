//! Derive-only text-kind rules (mirror of `web/src/lib/quran/text-marks.ts`):
//! which catalogue ids are tafsir, and the transliteration marks
//! (`<u>` long vowel, `<b>` silent letter) parsed in the view layer.

/// Tafsir works filed among the Arabic translations (TRX-01).
pub const TAFSIR_TRANSLATION_IDS: [&str; 2] = ["ar.jalalayn", "ar.muyassar"];

/// Commentary, not a reading translation.
pub fn is_tafsir_translation(id: &str) -> bool {
    TAFSIR_TRANSLATION_IDS.contains(&id)
}

/// One run of a transliteration verse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransliterationSegment {
    pub text: String,
    /// `<u>`: a long vowel to stretch.
    pub underline: bool,
    /// `<b>`: a silent or assimilated letter.
    pub strong: bool,
}

/// A `<u>`, `</U>`, `<b>`, `</B>` tag at `at`: (closing, is_u, length).
fn tag_at(text: &[u8], at: usize) -> Option<(bool, bool, usize)> {
    if text.get(at) != Some(&b'<') {
        return None;
    }
    let (closing, letter_at) = match text.get(at + 1) {
        Some(b'/') => (true, at + 2),
        _ => (false, at + 1),
    };
    let is_u = match text.get(letter_at)? {
        b'u' | b'U' => true,
        b'b' | b'B' => false,
        _ => return None,
    };
    (text.get(letter_at + 1) == Some(&b'>')).then_some((closing, is_u, letter_at + 2 - at))
}

/// Parses the marks (`parseTransliterationSegments`); any other character,
/// a stray `<` included, passes through verbatim.
pub fn parse_transliteration_segments(text: &str) -> Vec<TransliterationSegment> {
    if !text.contains('<') {
        return vec![TransliterationSegment {
            text: text.to_owned(),
            underline: false,
            strong: false,
        }];
    }
    let bytes = text.as_bytes();
    let mut segments = Vec::new();
    let (mut underline, mut strong) = (false, false);
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let Some((closing, is_u, len)) = tag_at(bytes, i) else {
            i += 1;
            continue;
        };
        if i > start {
            segments.push(TransliterationSegment {
                text: text[start..i].to_owned(),
                underline,
                strong,
            });
        }
        if is_u {
            underline = !closing;
        } else {
            strong = !closing;
        }
        i += len;
        start = i;
    }
    if start < bytes.len() {
        segments.push(TransliterationSegment {
            text: text[start..].to_owned(),
            underline,
            strong,
        });
    }
    segments
}

/// Copy/share text with the marks removed (`stripTransliterationMarkup`).
pub fn strip_transliteration_markup(text: &str) -> String {
    parse_transliteration_segments(text)
        .into_iter()
        .map(|s| s.text)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tafsir_ids() {
        assert!(is_tafsir_translation("ar.jalalayn"));
        assert!(!is_tafsir_translation("en.sahih"));
    }

    #[test]
    fn transliteration_marks() {
        let s = parse_transliteration_segments("Bismi All<u>a</U>hi a<b>l</B>rra");
        assert_eq!(s.len(), 5);
        assert!(s[1].underline && s[1].text == "a");
        assert!(s[3].strong && s[3].text == "l");
        assert_eq!(
            strip_transliteration_markup("x <u>a</U> <z> y"),
            "x a <z> y"
        );
    }
}
