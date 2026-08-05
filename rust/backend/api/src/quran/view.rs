use crate::quran::store::{Corpus, QuranMeta, QuranStore, Script};

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum OpenerKindDto {
    Verse,
    Header,
    None,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum OpenerPackagingDto {
    NumberedAyah,
    EmbeddedPrefix,
    ChapterFlag,
    SeparateRow,
    Absent,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SurahNormalizationDto {
    pub surah: u16,
    pub source_id: String,
    pub script: String,
    pub source_profile: String,
    pub packaging: OpenerPackagingDto,
    pub opener_kind: OpenerKindDto,
    pub opener_text: Option<String>,
    pub opener_end_scalar: u32,
    pub body_start_scalar: u32,
}

#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct QuranSurahTextDto {
    pub source_id: String,
    pub script: String,
    pub verses: Vec<String>,
    pub normalization: SurahNormalizationDto,
}

#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("invalid surah {0}")]
    InvalidSurah(u16),
    #[error("could not locate surah {0} ayah 1")]
    Locate(u16),
    #[error("verse {0} missing from corpus")]
    VerseMissing(u32),
    #[error("embedded-prefix detection failed at surah {0}")]
    DetectFailed(u16),
    #[error("unsupported packaging at surah {0}")]
    UnsupportedPackaging(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PrefixCut {
    opener_end_scalar: u32,
    body_start_scalar: u32,
}

const ZERO_CUT: PrefixCut = PrefixCut {
    opener_end_scalar: 0,
    body_start_scalar: 0,
};

fn is_skeleton_mark(ch: char) -> bool {
    matches!(
        ch,
        '\u{064B}'..='\u{065F}' | '\u{0640}' | '\u{0670}' | '\u{06D6}'..='\u{06ED}'
    )
}

fn skeleton_scalars(s: &str) -> Vec<char> {
    s.chars().filter(|c| !is_skeleton_mark(*c)).collect()
}

fn detect_embedded_prefix(raw: &str, reference: &str) -> Option<PrefixCut> {
    let target = skeleton_scalars(reference);
    let scalars: Vec<char> = raw.chars().collect();
    let mut target_index = 0usize;
    let mut scalar_index = 0usize;
    while scalar_index < scalars.len() && target_index < target.len() {
        let scalar = scalars[scalar_index];
        scalar_index += 1;
        if is_skeleton_mark(scalar) {
            continue;
        }
        if scalar != target[target_index] {
            return None;
        }
        target_index += 1;
    }
    if target_index != target.len() {
        return None;
    }
    while scalar_index < scalars.len() && is_skeleton_mark(scalars[scalar_index]) {
        scalar_index += 1;
    }
    let opener_end_scalar = scalar_index as u32;
    while scalar_index < scalars.len() && scalars[scalar_index].is_whitespace() {
        scalar_index += 1;
    }
    let body_start_scalar = scalar_index as u32;
    if body_start_scalar == opener_end_scalar || body_start_scalar as usize >= scalars.len() {
        return None;
    }
    Some(PrefixCut {
        opener_end_scalar,
        body_start_scalar,
    })
}

fn scalar_slice(raw: &str, start: usize, end: usize) -> String {
    raw.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn canonical_opener_kind(surah: u16) -> OpenerKindDto {
    match surah {
        1 => OpenerKindDto::Verse,
        9 => OpenerKindDto::None,
        _ => OpenerKindDto::Header,
    }
}

fn packaging(surah: u16) -> OpenerPackagingDto {
    match surah {
        1 => OpenerPackagingDto::NumberedAyah,
        9 => OpenerPackagingDto::Absent,
        _ => OpenerPackagingDto::EmbeddedPrefix,
    }
}

fn source_profile(script: Script) -> String {
    match script {
        Script::Uthmani => "tanzil-uthmani-581cc540".to_string(),
        Script::SimpleClean => "tanzil-simple-clean-a0c52760".to_string(),
    }
}

pub fn normalization(
    store: &QuranStore,
    script: Script,
    surah: u16,
) -> Result<SurahNormalizationDto, ViewError> {
    if !(1..=114).contains(&surah) {
        return Err(ViewError::InvalidSurah(surah));
    }
    let start_global = store
        .meta()
        .global_of(surah, 1)
        .ok_or(ViewError::Locate(surah))?;
    let raw = store
        .verse(script, start_global)
        .ok_or(ViewError::VerseMissing(start_global))?;
    let reference = store.verse(script, 1).ok_or(ViewError::VerseMissing(1))?;

    let pk = packaging(surah);
    let opener_kind = canonical_opener_kind(surah);
    let (cut, opener_text) = match pk {
        OpenerPackagingDto::EmbeddedPrefix => {
            let d = detect_embedded_prefix(raw, reference).ok_or(ViewError::DetectFailed(surah))?;
            let opener = scalar_slice(raw, 0, d.opener_end_scalar as usize);
            (d, Some(opener))
        }
        OpenerPackagingDto::NumberedAyah => (ZERO_CUT, Some(raw.to_string())),
        OpenerPackagingDto::Absent => (ZERO_CUT, None),
        _ => return Err(ViewError::UnsupportedPackaging(surah)),
    };

    Ok(SurahNormalizationDto {
        surah,
        source_id: script.as_str().to_string(),
        script: script.as_str().to_string(),
        source_profile: source_profile(script),
        packaging: pk,
        opener_kind,
        opener_text,
        opener_end_scalar: cut.opener_end_scalar,
        body_start_scalar: cut.body_start_scalar,
    })
}

pub fn surah_text(
    store: &QuranStore,
    script: Script,
    surah: u16,
) -> Result<QuranSurahTextDto, ViewError> {
    if !(1..=114).contains(&surah) {
        return Err(ViewError::InvalidSurah(surah));
    }
    let sura = store.meta().sura(surah).ok_or(ViewError::Locate(surah))?;
    let mut verses = Vec::with_capacity(sura.ayas as usize);
    for g in sura.start_global..=sura.end_global {
        verses.push(
            store
                .verse(script, g)
                .ok_or(ViewError::VerseMissing(g))?
                .to_string(),
        );
    }
    let normalization = normalization(store, script, surah)?;
    Ok(QuranSurahTextDto {
        source_id: normalization.source_id.clone(),
        script: normalization.script.clone(),
        verses,
        normalization,
    })
}

/// Surah text for a translation source. Body-only: a translation has no basmala opener
/// (verse 1 is content, not a prefix), so packaging is Absent and there is nothing to split.
/// Reads from an arbitrary [`Corpus`] (translation pool) instead of the Arabic store; the
/// Arabic [`surah_text`] above is untouched and its parity fixtures remain the guard.
pub fn surah_text_translation(
    meta: &QuranMeta,
    corpus: &Corpus,
    source_id: &str,
    source_profile: &str,
    surah: u16,
) -> Result<QuranSurahTextDto, ViewError> {
    if !(1..=114).contains(&surah) {
        return Err(ViewError::InvalidSurah(surah));
    }
    let sura = meta.sura(surah).ok_or(ViewError::Locate(surah))?;
    let mut verses = Vec::with_capacity(sura.ayas as usize);
    for g in sura.start_global..=sura.end_global {
        verses.push(
            corpus
                .verse(g)
                .ok_or(ViewError::VerseMissing(g))?
                .to_string(),
        );
    }
    let normalization = normalization_translation(source_id, source_profile, surah)?;
    Ok(QuranSurahTextDto {
        source_id: normalization.source_id.clone(),
        script: normalization.script.clone(),
        verses,
        normalization,
    })
}

/// Body-only normalization for a translation: no opener detection (no basmala), scalars zero,
/// packaging Absent. Translations do not carry a canonical-view profile, so `source_profile`
/// is informational only (passed through for response-shape parity with Arabic).
pub fn normalization_translation(
    source_id: &str,
    source_profile: &str,
    surah: u16,
) -> Result<SurahNormalizationDto, ViewError> {
    if !(1..=114).contains(&surah) {
        return Err(ViewError::InvalidSurah(surah));
    }
    Ok(SurahNormalizationDto {
        surah,
        source_id: source_id.to_string(),
        script: "translation".to_string(),
        source_profile: source_profile.to_string(),
        packaging: OpenerPackagingDto::Absent,
        opener_kind: OpenerKindDto::None,
        opener_text: None,
        opener_end_scalar: 0,
        body_start_scalar: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::QuranSettings;
    use crate::quran::load_quran_store;

    fn settings() -> QuranSettings {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");
        QuranSettings {
            uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
            simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
            metadata_xml_path: format!("{base}/quran-data.xml"),
            translations_dir: format!("{base}/translations"),
            max_resident_translations: 8,
            max_resident_bytes: 48 * 1024 * 1024,
            translation_idle_ttl_secs: 1800,
        }
    }

    fn fixture(script: Script) -> Vec<serde_json::Value> {
        let raw = match script {
            Script::Uthmani => include_str!("testdata/view-uthmani.json"),
            Script::SimpleClean => include_str!("testdata/view-simple-clean.json"),
        };
        serde_json::from_str::<Vec<serde_json::Value>>(raw).expect("fixture parses")
    }

    async fn parity(script: Script) {
        let store = load_quran_store(&settings()).await.expect("store loads");
        let expected = fixture(script);
        assert_eq!(expected.len(), 114);
        let (mut n_numbered, mut n_embedded, mut n_absent) = (0u32, 0u32, 0u32);
        for surah in 1..=114u16 {
            let got = normalization(&store, script, surah).expect("normalization");
            let exp = &expected[(surah - 1) as usize];
            assert_eq!(got.surah, exp["surah"].as_u64().unwrap() as u16);
            assert_eq!(got.source_id, exp["sourceId"].as_str().unwrap());
            assert_eq!(got.source_profile, exp["sourceProfile"].as_str().unwrap());
            assert_eq!(
                serde_json::to_value(got.packaging).unwrap(),
                exp["packaging"]
            );
            assert_eq!(
                serde_json::to_value(got.opener_kind).unwrap(),
                exp["openerKind"]
            );
            assert_eq!(got.opener_text.as_deref(), exp["openerText"].as_str());
            assert_eq!(
                got.opener_end_scalar,
                exp["openerEndScalar"].as_u64().unwrap() as u32
            );
            assert_eq!(
                got.body_start_scalar,
                exp["bodyStartScalar"].as_u64().unwrap() as u32
            );
            assert!(got.opener_end_scalar <= got.body_start_scalar);
            assert_eq!(
                got.body_start_scalar > 0,
                got.packaging == OpenerPackagingDto::EmbeddedPrefix
            );
            match got.packaging {
                OpenerPackagingDto::NumberedAyah => n_numbered += 1,
                OpenerPackagingDto::EmbeddedPrefix => n_embedded += 1,
                OpenerPackagingDto::Absent => n_absent += 1,
                _ => {}
            }
        }
        assert_eq!((n_numbered, n_embedded, n_absent), (1, 112, 1));
    }

    #[tokio::test]
    async fn parity_uthmani() {
        parity(Script::Uthmani).await;
    }

    #[tokio::test]
    async fn parity_simple_clean() {
        parity(Script::SimpleClean).await;
    }

    #[tokio::test]
    async fn surah_text_shape_and_anchors() {
        let store = load_quran_store(&settings()).await.unwrap();
        let st = surah_text(&store, Script::Uthmani, 2).unwrap();
        assert_eq!(st.verses.len(), store.meta().sura(2).unwrap().ayas as usize);
        assert_eq!(st.normalization.opener_end_scalar, 39);
        assert_eq!(st.normalization.body_start_scalar, 40);

        let s1 = normalization(&store, Script::Uthmani, 1).unwrap();
        assert_eq!(s1.packaging, OpenerPackagingDto::NumberedAyah);
        assert_eq!(s1.opener_kind, OpenerKindDto::Verse);
        assert_eq!((s1.opener_end_scalar, s1.body_start_scalar), (0, 0));

        let s9 = normalization(&store, Script::Uthmani, 9).unwrap();
        assert_eq!(s9.packaging, OpenerPackagingDto::Absent);
        assert!(s9.opener_text.is_none());

        assert!(normalization(&store, Script::Uthmani, 0).is_err());
        assert!(normalization(&store, Script::Uthmani, 115).is_err());
    }
}
