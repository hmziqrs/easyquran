//! Normalized Arabic search (§7.1).
//!
//! Design: the **result set** is computed by substring-matching the normalized
//! simple-clean corpus (so online/offline results are identical regardless of
//! the rendered script, §7.3). **Highlighting** is computed per hit by
//! re-normalizing the *requested script's* verse and mapping offsets back into
//! it — this is correct for either script and sidesteps the fragile
//! cross-script char-map the two scripts defeat (U+0670 superscript alef is a
//! reading marker in "الرحمن" but the alef *letter* in "العالمين": same
//! codepoint, opposite roles, so no deterministic 1:1 map exists).

use memchr::memmem;

use super::normalize::normalize_arabic;
use super::store::{Corpus, VERSE_COUNT};

/// A highlight span as UTF-16 code-unit offsets into the response `ayah.text`
/// (the consumer is JavaScript, §6.3).
#[derive(Clone, Copy, Debug)]
pub struct SrcHighlight {
    pub start: u32,
    pub end: u32,
}

pub struct SearchIndex {
    /// Normalized simple-clean corpus, verses concatenated with no separator
    /// (per-verse byte offsets delimit them, so a scan never matches across a
    /// verse boundary).
    arena: Box<str>,
    /// Byte offset per verse; len `VERSE_COUNT + 1`. Verse `g` = `arena[byte_off[g-1]..byte_off[g]]`.
    byte_off: Box<[u32]>,
}

impl SearchIndex {
    /// Build from the simple-clean corpus (no SQLite). `uthmani` is accepted to
    /// document that it is intentionally unused for results (§7.3: results are
    /// a function of simple-clean only).
    pub fn build(_uthmani: &Corpus, simple_clean: &Corpus) -> Self {
        let mut arena = String::new();
        let mut byte_off: Vec<u32> = Vec::with_capacity(VERSE_COUNT as usize + 1);
        byte_off.push(0);
        for g in 1..=VERSE_COUNT {
            let (norm, _map) = normalize_arabic(simple_clean.verse(g).expect("simple-clean verse"));
            arena.push_str(&norm);
            byte_off.push(arena.len() as u32);
        }
        Self {
            arena: arena.into_boxed_str(),
            byte_off: byte_off.into_boxed_slice(),
        }
    }

    /// Substring-search the normalized simple-clean corpus (§7.1).
    ///
    /// Returns `(total, globals)` where `total` is the FULL match count (for
    /// pagination) and `globals` is the materialized `[offset, offset+limit)`
    /// window of matching global indices — allocations bounded by `limit`.
    /// Ascending by `globalIndex`.
    pub fn search(&self, norm_q: &str, limit: usize, offset: usize) -> (u32, Vec<u32>) {
        let needle = norm_q.as_bytes();
        if needle.is_empty() {
            return (0, Vec::new());
        }
        let arena = self.arena.as_bytes();
        let mut total: u32 = 0;
        let mut globals: Vec<u32> = Vec::new();
        for g in 1..=VERSE_COUNT {
            let start = self.byte_off[(g - 1) as usize] as usize;
            let end = self.byte_off[g as usize] as usize;
            let haystack = &arena[start..end];
            if haystack.len() < needle.len() {
                continue;
            }
            if memmem::find(haystack, needle).is_some() {
                total += 1;
                if (total as usize) > offset && globals.len() < limit {
                    globals.push(g);
                }
            }
        }
        (total, globals)
    }
}

/// Highlight spans of `norm_q` within `src_text` (the verbatim, rendered-script
/// verse). UTF-16 code-unit offsets into `src_text` (§6.3). Re-normalizes
/// `src_text` so the spans land on valid boundaries in whichever script the
/// response carries.
pub fn highlight(src_text: &str, norm_q: &str) -> Vec<SrcHighlight> {
    let (norm_v, vmap) = normalize_arabic(src_text);
    let needle = norm_q.as_bytes();
    let haystack = norm_v.as_bytes();
    if needle.is_empty() || haystack.len() < needle.len() {
        return Vec::new();
    }
    let nchars = norm_q.chars().count();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = memmem::find(&haystack[from..], needle) {
        let abs = from + rel;
        let nstart = norm_v[..abs].chars().count();
        let src_start = vmap[nstart] as usize;
        let src_end = vmap[nstart + nchars - 1] as usize + 1;
        let (s, e) = char_span_to_utf16(src_text, src_start, src_end);
        out.push(SrcHighlight { start: s, end: e });
        from = abs + needle.len();
    }
    out
}

/// UTF-16 code-unit span of `text[start_char..end_char)` (chars are Unicode
/// scalars; offsets are JS code units, §6.3).
fn char_span_to_utf16(text: &str, start_char: usize, end_char: usize) -> (u32, u32) {
    let mut u16: u32 = 0;
    let mut start_u16: Option<u32> = None;
    for (i, c) in text.chars().enumerate() {
        if i == start_char {
            start_u16 = Some(u16);
        }
        u16 += c.len_utf16() as u32;
        if i + 1 == end_char {
            return (start_u16.unwrap_or(0), u16);
        }
    }
    (start_u16.unwrap_or(0), u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `char_span_to_utf16` lands on valid UTF-16 boundaries.
    #[test]
    fn utf16_span_is_valid() {
        let s = "الم"; // 3 Arabic chars, each 1 UTF-16 unit
        assert_eq!(char_span_to_utf16(s, 0, 1), (0, 1));
        assert_eq!(char_span_to_utf16(s, 1, 3), (1, 3));
    }

    /// `highlight` returns spans inside the source text for a real query.
    #[test]
    fn highlight_finds_occurrence() {
        let (nq, _) = normalize_arabic("بسم");
        let spans = highlight("بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ", &nq);
        assert!(!spans.is_empty(), "expected at least one highlight");
        // span is within the (UTF-16) text length.
        let len16 = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ".chars().map(|c| c.len_utf16() as u32).sum::<u32>();
        for s in &spans {
            assert!(s.start < s.end && s.end <= len16, "bad span {s:?}");
        }
    }
}
