//! Word view (PARITY C10): web `quran/words/{gate,registry,tokens,types}.ts`
//! and `VerseWords.svelte`. Word tokens drive per-word popovers for the
//! plain Arabic scripts only, and only once a licensed word source is
//! registered. The web's registry is empty until the owner licenses one
//! (O23), so the reader renders verses exactly as before; this module
//! mirrors that gate.

use eq_core::quran::types::ArabicSource;
use nsh_core::element::AnyElement;

/// A licensed word source (`WordSource`): what a popover credits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordSource {
    pub id: &'static str,
    pub licence: &'static str,
    pub attribution: &'static str,
    pub meaning_language: &'static str,
}

/// `WORD_SOURCES`: empty until a source is licensed (O23).
pub const WORD_SOURCES: &[WordSource] = &[];

/// One slice of an ayah (`WordSlice`): an exact substring; word slices
/// count from 1, whitespace slices carry `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordSlice<'a> {
    pub text: &'a str,
    pub word: Option<u32>,
    pub start: usize,
    pub end: usize,
}

/// `sliceWords`: contiguous runs of whitespace / non-whitespace, so the
/// slices join back to `text` byte for byte.
pub fn slice_words(text: &str) -> Vec<WordSlice<'_>> {
    let mut out = Vec::new();
    let mut word = 0;
    let mut start = 0;
    let mut space: Option<bool> = None;
    for (i, c) in text.char_indices() {
        let is_space = c.is_whitespace();
        if space.is_some_and(|s| s != is_space) {
            out.push(slice(text, start, i, space == Some(true), &mut word));
            start = i;
        }
        space = Some(is_space);
    }
    if start < text.len() {
        out.push(slice(
            text,
            start,
            text.len(),
            space == Some(true),
            &mut word,
        ));
    }
    out
}

fn slice<'a>(
    text: &'a str,
    start: usize,
    end: usize,
    is_space: bool,
    word: &mut u32,
) -> WordSlice<'a> {
    if !is_space {
        *word += 1;
    }
    WordSlice {
        text: &text[start..end],
        word: (!is_space).then_some(*word),
        start,
        end,
    }
}

/// `wordSourceFor`: Uthmani and Simple Clean only (Tajweed carries inline
/// markup, IndoPak has its own renderer).
pub fn word_source_for(script: ArabicSource, sources: &[WordSource]) -> Option<&WordSource> {
    match script {
        ArabicSource::Uthmani | ArabicSource::AnnotatedUthmani | ArabicSource::SimpleClean => {
            sources.first()
        }
        ArabicSource::IndoPak | ArabicSource::Tajweed => None,
    }
}

/// The word-token view for a verse, when the script has a word source.
/// None while the registry is empty: the row keeps its normal text.
pub fn verse_words(script: ArabicSource) -> Option<AnyElement> {
    word_source_for(script, WORD_SOURCES)?;
    // A registered source would render `slice_words` tokens with popovers
    // here; none is licensed, so this is unreachable today.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_join_back_verbatim() {
        let text = "بِسْمِ  ٱللَّهِ\u{a0}ٱلرَّحْمَٰنِ ";
        let slices = slice_words(text);
        assert_eq!(slices.iter().map(|s| s.text).collect::<String>(), text);
        assert_eq!(slices.iter().filter_map(|s| s.word).max(), Some(3));
    }
}
