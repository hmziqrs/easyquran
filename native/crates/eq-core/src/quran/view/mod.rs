//! View rules over verbatim text (PARITY B6): canonical openers, tajweed
//! segments, IndoPak word boxes, text marks, and the reader's verse view
//! model (mirror of `web/src/lib/quran/view/*`, `text-marks.ts`). Nothing
//! here changes stored text: every display string is the row itself or a
//! verbatim slice of it.

pub mod indopak;
pub mod source_view;
pub mod tajweed;
pub mod text_marks;

use crate::quran::{
    meta::{QuranMeta, SajdaKind},
    types::{Ayah, OpenerKind, QuranScript, SurahNormalization},
};

pub use source_view::{SourceView, ViewError, body_text, canonical_opener_kind};

/// The bismillah header text above a surah's first verse, plain-text form
/// (tajweed markup stripped; a no-op elsewhere). `None` when the surah has
/// no header (`headerText`).
pub fn header_text(normalization: &SurahNormalization) -> Option<String> {
    if normalization.opener_kind != OpenerKind::Header {
        return None;
    }
    normalization
        .opener_text
        .as_deref()
        .map(tajweed::strip_tajweed_markup)
}

/// The header's verbatim text (markup kept, for a tajweed-coloured header).
pub fn header_raw(normalization: &SurahNormalization) -> Option<&str> {
    if normalization.opener_kind != OpenerKind::Header {
        return None;
    }
    normalization.opener_text.as_deref()
}

/// One verse as the reader draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerseView {
    /// `"{surah}:{ayah}"`.
    pub key: String,
    pub surah: u16,
    pub ayah: u16,
    pub global_index: u32,
    /// The row, verbatim.
    pub raw: String,
    /// What is displayed: `raw`, or for 1:1 of an embedded-opener source
    /// the slice after the opener.
    pub body: String,
    pub script: QuranScript,
    pub sajda: Option<SajdaKind>,
}

impl VerseView {
    /// Tajweed runs of the body (one plain run for other scripts).
    pub fn tajweed_segments(&self) -> Vec<tajweed::TajweedSegment> {
        if tajweed::is_tajweed_script(self.script) {
            return tajweed::parse_tajweed_segments(&self.body);
        }
        vec![tajweed::TajweedSegment {
            text: self.body.clone(),
            rule: None,
        }]
    }

    /// IndoPak word boxes of the body; `None` for other scripts.
    pub fn indopak(&self) -> Option<indopak::IndoPakEnding> {
        (self.script == QuranScript::IndoPak).then(|| indopak::indopak_ending(&self.body))
    }

    /// Plain text for copy and share: markup stripped.
    pub fn plain_text(&self) -> String {
        if tajweed::is_tajweed_script(self.script) {
            return tajweed::strip_tajweed_markup(&self.body);
        }
        self.body.clone()
    }
}

/// One surah's run inside a range, with the header to draw above it when
/// the run starts at verse 1 (`QuranRangeGroup`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeGroup {
    pub surah: u16,
    pub verses: Vec<VerseView>,
    pub normalization: SurahNormalization,
    /// Plain header text (`headerText`) when the run opens the surah.
    pub opener: Option<String>,
}

/// Groups a range's verses by surah, cutting embedded openers off 1:1 and
/// marking sajdas (`groupRangeAyahs` + `displayVerses`). A verse whose
/// surah has no normalization is an error, never a silent default.
pub fn group_range(
    ayahs: &[Ayah],
    normalizations: &[SurahNormalization],
    meta: &QuranMeta,
) -> Result<Vec<RangeGroup>, String> {
    let mut groups: Vec<RangeGroup> = Vec::new();
    for ayah in ayahs {
        let starts_new = groups.last().is_none_or(|g| g.surah != ayah.surah);
        if starts_new {
            let normalization = normalizations
                .iter()
                .find(|n| n.surah == ayah.surah)
                .ok_or(format!(
                    "Missing Quran normalization for surah {}",
                    ayah.surah
                ))?
                .clone();
            let opener = if ayah.ayah == 1 {
                header_text(&normalization)
            } else {
                None
            };
            groups.push(RangeGroup {
                surah: ayah.surah,
                verses: Vec::new(),
                normalization,
                opener,
            });
        }
        let group = groups.last_mut().expect("pushed above");
        group.verses.push(VerseView {
            key: ayah.key.clone(),
            surah: ayah.surah,
            ayah: ayah.ayah,
            global_index: ayah.global_index,
            raw: ayah.text.clone(),
            body: body_text(&ayah.text, ayah.ayah, &group.normalization).to_owned(),
            script: group.normalization.script,
            sajda: meta.sajda_at(ayah.surah, ayah.ayah).map(|s| s.kind),
        });
    }
    Ok(groups)
}
