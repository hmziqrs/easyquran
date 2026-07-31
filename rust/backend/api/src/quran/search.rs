use memchr::memmem;

use super::normalize::normalize_arabic;
use super::store::{Corpus, VERSE_COUNT};

#[derive(Clone, Copy, Debug)]
pub struct SrcHighlight {
    pub start: u32,
    pub end: u32,
}

pub struct SearchIndex {
    arena: Box<str>,
    byte_off: Box<[u32]>,
}

impl SearchIndex {
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

    #[test]
    fn utf16_span_is_valid() {
        let s = "الم";
        assert_eq!(char_span_to_utf16(s, 0, 1), (0, 1));
        assert_eq!(char_span_to_utf16(s, 1, 3), (1, 3));
    }

    #[test]
    fn highlight_finds_occurrence() {
        let (nq, _) = normalize_arabic("بسم");
        let spans = highlight("بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ", &nq);
        assert!(!spans.is_empty(), "expected at least one highlight");
        let len16 = "بِسْمِ ٱللَّهِ ٱلرَّحْمَـٰنِ ٱلرَّحِيمِ".chars().map(|c| c.len_utf16() as u32).sum::<u32>();
        for s in &spans {
            assert!(s.start < s.end && s.end <= len16, "bad span {s:?}");
        }
    }
}
