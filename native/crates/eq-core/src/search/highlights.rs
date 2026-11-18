//! Highlight presentation (`quran/search/highlights.ts`): untrusted
//! ranges sorted, clamped, merged, and the text cut into plain and
//! highlighted segments that join back to the verbatim text.

use super::corpus::Highlight;

/// One run of a hit's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
    pub highlighted: bool,
}

impl Segment {
    /// The run's text.
    pub fn text<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start..self.end]
    }
}

/// `highlightSegments`, over byte offsets: ranges off a char boundary or
/// outside the text are dropped, ends past the text clamped, overlaps
/// merged.
pub fn highlight_segments(text: &str, highlights: &[Highlight]) -> Vec<Segment> {
    let len = text.len();
    let mut ranges: Vec<(usize, usize)> = highlights
        .iter()
        .filter(|h| h.end > h.start && h.start < len)
        .map(|h| (h.start, h.end.min(len)))
        .filter(|(s, e)| text.is_char_boundary(*s) && text.is_char_boundary(*e))
        .collect();
    ranges.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    let mut out = Vec::new();
    let mut cursor = 0;
    for (start, end) in merged {
        if cursor < start {
            out.push(Segment {
                start: cursor,
                end: start,
                highlighted: false,
            });
        }
        out.push(Segment {
            start,
            end,
            highlighted: true,
        });
        cursor = end;
    }
    if cursor < len {
        out.push(Segment {
            start: cursor,
            end: len,
            highlighted: false,
        });
    }
    out
}
