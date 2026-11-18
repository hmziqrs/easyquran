//! Match-to-display alignment (`quran/search/alignment.ts`): the search
//! matches one source's normalised text and highlights another's verbatim
//! text, so every normalised match char is mapped onto a display span
//! (word by word when the word counts agree, else across the whole text).

use super::normalize::{NormalizedMap, normalize_arabic_with_map};

/// A match surface: the normalised text plus, per char, the display
/// byte span it lights.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Alignment {
    pub norm: String,
    pub starts: Vec<u32>,
    pub ends: Vec<u32>,
}

#[derive(Clone, Copy)]
struct Range {
    start: usize,
    end: usize,
}

fn word_ranges(text: &[char]) -> Vec<Range> {
    let mut out = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = text[start..]
            .iter()
            .position(|c| *c == ' ')
            .map_or(text.len(), |p| start + p);
        out.push(Range { start, end });
        start = end + 1;
    }
    out
}

fn align_range(
    matching: &[char],
    mr: Range,
    display: &[char],
    dr: Range,
    aligned: &mut [Option<usize>],
) {
    let ml = mr.end - mr.start;
    let dl = dr.end - dr.start;
    let width = dl + 1;
    let mut costs = vec![0u32; (ml + 1) * width];
    for i in 1..=ml {
        costs[i * width] = i as u32;
    }
    for (j, cost) in costs.iter_mut().enumerate().take(dl + 1).skip(1) {
        *cost = j as u32;
    }
    for i in 1..=ml {
        for j in 1..=dl {
            let equal = matching[mr.start + i - 1] == display[dr.start + j - 1];
            let diagonal = costs[(i - 1) * width + j - 1] + u32::from(!equal);
            let deleted = costs[(i - 1) * width + j] + 1;
            let inserted = costs[i * width + j - 1] + 1;
            costs[i * width + j] = diagonal.min(deleted).min(inserted);
        }
    }
    let (mut i, mut j) = (ml, dl);
    while i > 0 || j > 0 {
        let current = costs[i * width + j];
        let equal = i > 0 && j > 0 && matching[mr.start + i - 1] == display[dr.start + j - 1];
        if equal && current == costs[(i - 1) * width + j - 1] {
            aligned[mr.start + i - 1] = Some(dr.start + j - 1);
            i -= 1;
            j -= 1;
            continue;
        }
        if i > 0 && current == costs[(i - 1) * width + j] + 1 {
            i -= 1;
            continue;
        }
        if j > 0 && current == costs[i * width + j - 1] + 1 {
            j -= 1;
            continue;
        }
        aligned[mr.start + i - 1] = Some(dr.start + j - 1);
        i -= 1;
        j -= 1;
    }
}

fn align_to_display(matching: &NormalizedMap, display: &NormalizedMap) -> (Vec<u32>, Vec<u32>) {
    let mc = matching.chars();
    let dc = display.chars();
    let mut aligned: Vec<Option<usize>> = vec![None; mc.len()];
    let mw = word_ranges(&mc);
    let dw = word_ranges(&dc);
    if mw.len() == dw.len() {
        for (word, (m, d)) in mw.iter().zip(&dw).enumerate() {
            align_range(&mc, *m, &dc, *d, &mut aligned);
            if word + 1 < mw.len() {
                aligned[m.end] = Some(d.end);
            }
        }
    } else {
        align_range(
            &mc,
            Range {
                start: 0,
                end: mc.len(),
            },
            &dc,
            Range {
                start: 0,
                end: dc.len(),
            },
            &mut aligned,
        );
    }

    let n = aligned.len();
    let mut previous = vec![None; n];
    let mut next = vec![None; n];
    let mut nearest = None;
    for at in 0..n {
        previous[at] = nearest;
        nearest = aligned[at].or(nearest);
    }
    nearest = None;
    for at in (0..n).rev() {
        next[at] = nearest;
        nearest = aligned[at].or(nearest);
    }

    let mut starts = Vec::with_capacity(n);
    let mut ends = Vec::with_capacity(n);
    for at in 0..n {
        if let Some(d) = aligned[at] {
            starts.push(display.starts[d]);
            ends.push(display.ends[d]);
            continue;
        }
        // Collapse onto the nearest surviving neighbour: the next word's
        // start, else the previous one's end, else the very beginning.
        let boundary = match (next[at], previous[at]) {
            (Some(after), _) => display.starts[after],
            (None, Some(before)) => display.ends[before],
            (None, None) => 0,
        };
        starts.push(boundary);
        ends.push(boundary);
    }
    (starts, ends)
}

/// `alignSearchText`: the match text's normalised surface, mapped onto
/// byte spans of `display_text`.
pub fn align_search_text(match_text: &str, display_text: &str) -> Alignment {
    let matching = normalize_arabic_with_map(match_text);
    if match_text == display_text {
        return Alignment {
            norm: matching.normalized,
            starts: matching.starts,
            ends: matching.ends,
        };
    }
    let display = normalize_arabic_with_map(display_text);
    let (starts, ends) = align_to_display(&matching, &display);
    Alignment {
        norm: matching.normalized,
        starts,
        ends,
    }
}
