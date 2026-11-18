//! Canonical opener view per source (mirror of `view/source-view.ts`,
//! `view/canonical.ts`, `view/canonical-coordinates.ts`): which surahs carry
//! a bismillah header, where an embedded opener ends inside 1:1, and what
//! the displayed body of a first ayah is. The stored text is never changed:
//! the body is a suffix slice of the verbatim row.

use std::collections::HashMap;

use crate::quran::{
    catalogue::ArabicProfile,
    types::{OpenerKind, OpenerPackaging, SurahNormalization},
};

/// A source failed the canonical checks; it is not opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("[quran-view:{profile}] {message}")]
pub struct ViewError {
    pub profile: String,
    pub message: String,
}

/// What opens surah `surah` canonically (`canonicalOpenerKind`).
pub fn canonical_opener_kind(surah: u16) -> OpenerKind {
    match surah {
        1 => OpenerKind::Verse,
        9 => OpenerKind::None,
        _ => OpenerKind::Header,
    }
}

fn is_skeleton_mark(c: char) -> bool {
    matches!(c, '\u{064B}'..='\u{065F}' | '\u{0640}' | '\u{0670}' | '\u{06D6}'..='\u{06ED}')
}

/// JS `\s` (Unicode white space plus U+FEFF).
pub(crate) fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

/// Byte offset of scalar index `scalar` in `text` (`scalarToUtf16Index`,
/// in UTF-8). `None` past the end.
pub fn scalar_to_byte_index(text: &str, scalar: usize) -> Option<usize> {
    if scalar == 0 {
        return Some(0);
    }
    let mut count = 0;
    for (i, _) in text.char_indices() {
        if count == scalar {
            return Some(i);
        }
        count += 1;
    }
    (count == scalar).then_some(text.len())
}

/// Scalar slice `[start, end)` (`scalarSlice`); `end = None` to the end.
pub fn scalar_slice(text: &str, start: usize, end: Option<usize>) -> Option<&str> {
    let a = scalar_to_byte_index(text, start)?;
    let b = match end {
        Some(e) => scalar_to_byte_index(text, e)?,
        None => text.len(),
    };
    text.get(a..b)
}

/// Where an opener embedded at the start of `raw` ends (`PrefixCut`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrefixCut {
    pub opener_end_scalar: usize,
    pub body_start_scalar: usize,
}

/// Matches `reference`'s skeleton at the start of `raw`, skipping marks
/// (`detectEmbeddedPrefix`).
pub fn detect_embedded_prefix(raw: &str, reference: &str) -> Option<PrefixCut> {
    let target: Vec<char> = reference
        .chars()
        .filter(|&c| !is_skeleton_mark(c))
        .collect();
    let scalars: Vec<char> = raw.chars().collect();
    let mut t = 0;
    let mut i = 0;
    while i < scalars.len() && t < target.len() {
        let c = scalars[i];
        i += 1;
        if is_skeleton_mark(c) {
            continue;
        }
        if c != target[t] {
            return None;
        }
        t += 1;
    }
    if t != target.len() {
        return None;
    }
    while i < scalars.len() && is_skeleton_mark(scalars[i]) {
        i += 1;
    }
    let opener_end_scalar = i;
    while i < scalars.len() && is_js_space(scalars[i]) {
        i += 1;
    }
    let body_start_scalar = i;
    if body_start_scalar == opener_end_scalar || body_start_scalar >= scalars.len() {
        return None;
    }
    Some(PrefixCut {
        opener_end_scalar,
        body_start_scalar,
    })
}

/// The displayed body of a verse: for ayah 1 with an embedded opener, the
/// row after the cut; otherwise the row itself (`bodyText`).
pub fn body_text<'a>(raw: &'a str, ayah: u16, normalization: &SurahNormalization) -> &'a str {
    if ayah != 1 || normalization.body_start_scalar == 0 {
        return raw;
    }
    scalar_to_byte_index(raw, normalization.body_start_scalar as usize)
        .and_then(|i| raw.get(i..))
        .unwrap_or(raw)
}

/// One loaded Arabic source's opener partition (`QuranSourceView`).
#[derive(Debug, Clone)]
pub struct SourceView {
    pub profile: &'static ArabicProfile,
    normalizations: Vec<SurahNormalization>,
}

impl SourceView {
    /// Builds and checks the 114 normalizations
    /// (`createQuranSourceView`). `first_ayahs` are `(surah, 1:1 text)`;
    /// `openers` the trusted opener text per surah for separate-row sources.
    pub fn build(
        profile: &'static ArabicProfile,
        first_ayahs: &[(u16, String)],
        openers: &HashMap<u16, String>,
    ) -> Result<Self, ViewError> {
        let fail = |message: String| ViewError {
            profile: profile.profile_id.to_owned(),
            message,
        };
        let mut first: HashMap<u16, &str> = HashMap::new();
        for (surah, text) in first_ayahs {
            if first.insert(*surah, text.as_str()).is_some() {
                return Err(fail(format!("duplicate first ayah for surah {surah}")));
            }
        }
        if first.len() != 114 {
            return Err(fail(format!("first-ayah count {} != 114", first.len())));
        }
        let reference = *first
            .get(&profile.reference_opener_surah)
            .ok_or_else(|| fail("missing reference opener".into()))?;
        let mut normalizations = Vec::with_capacity(114);
        for surah in 1..=114_u16 {
            let raw = *first
                .get(&surah)
                .ok_or_else(|| fail(format!("missing {surah}:1")))?;
            let packaging = profile.packaging(surah);
            let opener_kind = canonical_opener_kind(surah);
            let mut cut = PrefixCut {
                opener_end_scalar: 0,
                body_start_scalar: 0,
            };
            let opener_text: Option<String> = match packaging {
                OpenerPackaging::EmbeddedPrefix => {
                    if opener_kind != OpenerKind::Header {
                        return Err(fail(format!("embedded prefix at non-header surah {surah}")));
                    }
                    cut = detect_embedded_prefix(raw, reference).ok_or_else(|| {
                        fail(format!("expected embedded opener prefix at {surah}:1"))
                    })?;
                    let opener = scalar_slice(raw, 0, Some(cut.opener_end_scalar));
                    let sep = scalar_slice(raw, cut.opener_end_scalar, Some(cut.body_start_scalar));
                    let body = scalar_slice(raw, cut.body_start_scalar, None);
                    match (opener, sep, body) {
                        (Some(o), Some(s), Some(b)) if format!("{o}{s}{b}") == raw => {
                            Some(o.to_owned())
                        }
                        _ => return Err(fail(format!("lossy partition at {surah}:1"))),
                    }
                }
                OpenerPackaging::NumberedAyah => {
                    if opener_kind != OpenerKind::Verse {
                        return Err(fail(format!("numbered opener at non-verse surah {surah}")));
                    }
                    Some(raw.to_owned())
                }
                OpenerPackaging::ChapterFlag | OpenerPackaging::SeparateRow => {
                    if opener_kind != OpenerKind::Header {
                        return Err(fail(format!("{packaging:?} at non-header surah {surah}")));
                    }
                    let text = openers
                        .get(&surah)
                        .filter(|t| !t.is_empty())
                        .ok_or_else(|| {
                            fail(format!("missing trusted opener text for surah {surah}"))
                        })?;
                    Some(text.clone())
                }
                OpenerPackaging::Absent => {
                    if opener_kind != OpenerKind::None {
                        return Err(fail(format!("absent opener at canonical surah {surah}")));
                    }
                    None
                }
            };
            let body = match cut.body_start_scalar {
                0 => raw,
                n => scalar_slice(raw, n, None).unwrap_or(""),
            };
            if body.is_empty() {
                return Err(fail(format!("empty body at {surah}:1")));
            }
            if body.trim_matches(is_js_space) != body {
                return Err(fail(format!("untrimmed body at {surah}:1")));
            }
            if (cut.body_start_scalar > 0) != (packaging == OpenerPackaging::EmbeddedPrefix) {
                return Err(fail(format!("cut/package mismatch at {surah}:1")));
            }
            if (opener_kind == OpenerKind::None) != opener_text.is_none() {
                return Err(fail(format!("missing opener at {surah}")));
            }
            normalizations.push(SurahNormalization {
                surah,
                source_id: profile.source.id().to_owned(),
                script: profile.script(),
                source_profile: profile.profile_id.to_owned(),
                packaging,
                opener_kind,
                opener_text,
                opener_end_scalar: cut.opener_end_scalar as u32,
                body_start_scalar: cut.body_start_scalar as u32,
            });
        }
        Ok(Self {
            profile,
            normalizations,
        })
    }

    /// Normalization of surah `surah` (1..=114).
    pub fn normalization(&self, surah: u16) -> Option<&SurahNormalization> {
        if surah == 0 {
            return None;
        }
        self.normalizations.get(usize::from(surah) - 1)
    }

    /// All 114.
    pub fn normalizations(&self) -> &[SurahNormalization] {
        &self.normalizations
    }

    /// The displayed body of `surah:ayah` (`view.body`).
    pub fn body<'a>(&self, surah: u16, ayah: u16, raw: &'a str) -> &'a str {
        match self.normalization(surah) {
            Some(n) => body_text(raw, ayah, n),
            None => raw,
        }
    }
}

/// Counts per packaging (`packagingCounts`), in enum order:
/// numbered, embedded, chapter-flag, separate-row, absent.
pub fn packaging_counts(normalizations: &[SurahNormalization]) -> [usize; 5] {
    let mut counts = [0; 5];
    for n in normalizations {
        let i = match n.packaging {
            OpenerPackaging::NumberedAyah => 0,
            OpenerPackaging::EmbeddedPrefix => 1,
            OpenerPackaging::ChapterFlag => 2,
            OpenerPackaging::SeparateRow => 3,
            OpenerPackaging::Absent => 4,
        };
        counts[i] += 1;
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_indexes_are_code_points() {
        let s = "a\u{1F310}b";
        assert_eq!(scalar_to_byte_index(s, 1), Some(1));
        assert_eq!(scalar_to_byte_index(s, 2), Some(5));
        assert_eq!(scalar_to_byte_index(s, 3), Some(6));
        assert_eq!(scalar_to_byte_index(s, 4), None);
        assert_eq!(scalar_slice(s, 1, Some(2)), Some("\u{1F310}"));
    }

    #[test]
    fn embedded_prefix_skips_marks_and_requires_a_body() {
        let reference = "بِسْمِ ٱللَّهِ";
        let raw = "بِسْمِ ٱللَّهِ الٓمٓ";
        let cut = detect_embedded_prefix(raw, reference).unwrap();
        assert_eq!(scalar_slice(raw, cut.body_start_scalar, None), Some("الٓمٓ"));
        assert_eq!(detect_embedded_prefix(reference, reference), None);
        assert_eq!(detect_embedded_prefix("قُلْ", reference), None);
    }
}
