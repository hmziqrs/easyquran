//! Surah scoring shared by the palette and the search page
//! (`search/palette/scoring.ts`, `search/page/surah-suggest.ts`).

use crate::quran::meta::{QuranMeta, Surah};

use super::{
    normalize::normalize_arabic,
    translit::{TranslitScorer, surah_keys},
};

fn is_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(c, '-' | '\'' | '’' | '.')
}

fn is_subsequence(hay: &str, needle: &str) -> bool {
    let mut wanted = needle.chars().peekable();
    for c in hay.chars() {
        match wanted.peek() {
            None => return true,
            Some(w) if *w == c => {
                wanted.next();
            }
            Some(_) => {}
        }
    }
    wanted.peek().is_none()
}

fn score_prefix(hay: &str, q: &str) -> Option<f64> {
    if hay.is_empty() || q.is_empty() {
        return Some(0.0);
    }
    if hay == q {
        return Some(1.0);
    }
    if hay.starts_with(q) {
        return Some(0.9);
    }
    if let Some(at) = hay.find(q)
        && at > 0
    {
        let boundary = hay[..at].chars().next_back().is_some_and(is_boundary);
        return Some(if boundary { 0.8 } else { 0.6 });
    }
    None
}

/// `scoreText` (palette): exact 1, prefix 0.9, word start 0.8, substring
/// 0.6, subsequence 0.3, else 0.
pub fn score_text(haystack: &str, needle: &str) -> f64 {
    let hay = haystack.trim().to_lowercase();
    let q = needle.trim().to_lowercase();
    if let Some(score) = score_prefix(&hay, &q) {
        return score;
    }
    if is_subsequence(&hay, &q) { 0.3 } else { 0.0 }
}

/// The search page's `scoreText`: as the palette's, without the
/// subsequence tier.
fn score_text_strict(haystack: &str, needle: &str) -> f64 {
    let hay = haystack.trim().to_lowercase();
    let q = needle.trim().to_lowercase();
    score_prefix(&hay, &q).unwrap_or(0.0)
}

/// `scoreFields`: the best score across fields.
pub fn score_fields<S: AsRef<str>>(fields: &[S], needle: &str) -> f64 {
    fields
        .iter()
        .map(|f| score_text(f.as_ref(), needle))
        .fold(0.0, f64::max)
}

/// `scoreArabic`: over the search normalisation.
pub fn score_arabic(haystack: &str, normalized_needle: &str) -> f64 {
    if normalized_needle.is_empty() {
        return 0.0;
    }
    let hay = normalize_arabic(haystack);
    if hay.is_empty() {
        return 0.0;
    }
    if hay == normalized_needle {
        return 1.0;
    }
    if hay.starts_with(normalized_needle) {
        return 0.9;
    }
    if hay.contains(normalized_needle) {
        return 0.6;
    }
    0.0
}

/// One scored surah.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoredSurah<'a> {
    pub surah: &'a Surah,
    pub score: f64,
}

fn translit_score(scorer: Option<&TranslitScorer>, meta: &QuranMeta, num: u16) -> f64 {
    let keys = surah_keys(meta);
    match (scorer, keys.get(usize::from(num) - 1)) {
        (Some(scorer), Some(keys)) => scorer.score(keys),
        _ => 0.0,
    }
}

fn sort_scored(scored: &mut [ScoredSurah<'_>]) {
    scored.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.surah.num.cmp(&b.surah.num))
    });
}

/// The palette's surah ranking (`quran-surahs.ts` `rank`): Latin fields
/// with subsequence, normalised Arabic name, transliteration fold.
pub fn rank_surahs<'a>(
    meta: &'a QuranMeta,
    needle: &str,
    arabic_needle: &str,
) -> Vec<ScoredSurah<'a>> {
    let scorer = TranslitScorer::new(needle);
    let mut scored: Vec<ScoredSurah<'a>> = meta
        .surahs()
        .iter()
        .filter_map(|s| {
            let score = score_fields(&[&s.name, &s.transliteration, &s.meaning, &s.slug], needle)
                .max(score_arabic(&s.arabic_plain, arabic_needle))
                .max(translit_score(scorer.as_ref(), meta, s.num));
            (score > 0.0).then_some(ScoredSurah { surah: s, score })
        })
        .collect();
    sort_scored(&mut scored);
    scored
}

/// Suggestions the search page shows (`MAX_SUGGESTIONS`).
pub const MAX_SUGGESTIONS: usize = 5;

/// `suggestSurahs`: the search page's jump list, best first, capped.
pub fn suggest_surahs<'a>(meta: &'a QuranMeta, raw: &str, limit: usize) -> Vec<ScoredSurah<'a>> {
    let query = raw.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let scorer = TranslitScorer::new(query);
    let arabic = normalize_arabic(query);
    let mut scored: Vec<ScoredSurah<'a>> = meta
        .surahs()
        .iter()
        .filter_map(|s| {
            let score = [&s.name, &s.transliteration, &s.meaning, &s.slug]
                .iter()
                .map(|f| score_text_strict(f, query))
                .fold(0.0, f64::max)
                .max(score_arabic(&s.arabic_plain, &arabic))
                .max(translit_score(scorer.as_ref(), meta, s.num));
            (score > 0.0).then_some(ScoredSurah { surah: s, score })
        })
        .collect();
    sort_scored(&mut scored);
    scored.truncate(limit);
    scored
}
