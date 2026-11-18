//! Online fallback (PARITY B4; mirror of `api-client.ts`, `range-fetch.ts`,
//! `fetch.ts`, `wire.ts`, `api-availability.ts`): reads a range or a surah
//! from the read API (`/quran/sources/{id}/…`, backend module `quran_v1`)
//! when the source is not on the device. Responses are decoded strictly:
//! a malformed record fails the read rather than showing partial text.

use serde::Deserialize;
use serde_json::Value;

use super::{
    catalogue::arabic_profile,
    meta::QuranMeta,
    types::{
        ArabicSource, Ayah, OpenerKind, OpenerPackaging, QuranScript, RangeText, SourceRef,
        SurahNormalization, SurahText,
    },
};
use crate::net::{NetError, Request, Transport};

/// Production read API base (`PUBLIC_QURAN_API_BASE` in compose:
/// `https://${DOMAIN}/api/quran`).
pub const PRODUCTION_API_BASE: &str = "https://easyquran.fyi/api/quran";
/// Most verses one range response carries (`RESPONSE_CAP`).
pub const RESPONSE_CAP: u32 = 300;
/// Cool-down after the API trips the breaker (`QURAN_API_COOLDOWN_MS`).
pub const API_COOLDOWN_MS: i64 = 30_000;
/// Server failures before the breaker opens.
pub const API_SERVER_FAILURE_THRESHOLD: u32 = 2;

/// Why an online read failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OnlineError {
    /// The breaker is open: no request was sent.
    #[error("quran api temporarily unavailable")]
    Unavailable,
    #[error("transport: {0}")]
    Transport(NetError),
    #[error("http {0}")]
    Http(u16),
    #[error("malformed: {0}")]
    Malformed(String),
}

fn malformed(m: &str) -> OnlineError {
    OnlineError::Malformed(m.to_owned())
}

/// `planRangeChunks`: one request up to the cap, two up to twice it.
pub fn plan_range_chunks(from: u32, to: u32) -> Result<Vec<(u32, u32)>, OnlineError> {
    if from < 1 || to < from {
        return Err(malformed("invalid range bounds"));
    }
    let count = to - from + 1;
    if count <= RESPONSE_CAP {
        return Ok(vec![(from, to)]);
    }
    if count > 2 * RESPONSE_CAP {
        return Err(malformed("range exceeds max chunk count"));
    }
    let mid = from + RESPONSE_CAP - 1;
    Ok(vec![(from, mid), (mid + 1, to)])
}

/// `unwrapEnvelope`: `{ data }` peeled when present.
fn unwrap_envelope(v: Value) -> Value {
    match v {
        Value::Object(mut o) if o.contains_key("data") => o.remove("data").unwrap_or(Value::Null),
        other => other,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireAyah {
    key: String,
    surah: u16,
    ayah: u16,
    global_index: u32,
    text: String,
}

fn decode_ayah(v: &Value, meta: &QuranMeta) -> Result<Ayah, OnlineError> {
    let a: WireAyah =
        serde_json::from_value(v.clone()).map_err(|e| OnlineError::Malformed(e.to_string()))?;
    if a.surah < 1 || a.surah > 114 || a.ayah < 1 || a.global_index < 1 || a.global_index > 6236 {
        return Err(malformed("ayah coordinate out of range"));
    }
    if a.key != format!("{}:{}", a.surah, a.ayah) {
        return Err(malformed("ayah key mismatch"));
    }
    if meta.global_index_of(a.surah, a.ayah) != Some(a.global_index) {
        return Err(malformed("non-canonical ayah coordinate"));
    }
    Ok(Ayah {
        key: a.key,
        surah: a.surah,
        ayah: a.ayah,
        global_index: a.global_index,
        text: a.text,
    })
}

/// `decodeSurahNormalization` / `decodeTranslationNormalization`.
fn decode_normalization(v: &Value, source: &SourceRef) -> Result<SurahNormalization, OnlineError> {
    let n: SurahNormalization =
        serde_json::from_value(v.clone()).map_err(|e| OnlineError::Malformed(e.to_string()))?;
    if n.surah < 1 || n.surah > 114 || n.opener_end_scalar > n.body_start_scalar {
        return Err(malformed("normalization bounds"));
    }
    if n.source_id != source.id() {
        return Err(malformed("normalization source mismatch"));
    }
    match source {
        SourceRef::Arabic(s) => {
            let profile = arabic_profile(*s);
            if n.source_profile != profile.profile_id || n.script != profile.script() {
                return Err(malformed("normalization profile mismatch"));
            }
            if (n.opener_kind == OpenerKind::None) != n.opener_text.is_none() {
                return Err(malformed("opener text"));
            }
        }
        SourceRef::Translation(_) => {
            let plain = n.script == QuranScript::Translation
                && n.packaging == OpenerPackaging::Absent
                && n.opener_kind == OpenerKind::None
                && n.opener_text.is_none()
                && n.opener_end_scalar == 0
                && n.body_start_scalar == 0;
            if !plain {
                return Err(malformed("translation normalization"));
            }
        }
    }
    Ok(n)
}

/// `decodeQuranRangeText` / `decodeTranslationRangeText`.
pub fn decode_range_text(
    raw: Value,
    source: &SourceRef,
    meta: &QuranMeta,
) -> Result<RangeText, OnlineError> {
    let v = unwrap_envelope(raw);
    let ayahs_raw = v["ayahs"].as_array().ok_or(malformed("ayahs"))?;
    let norms_raw = v["normalizations"]
        .as_array()
        .ok_or(malformed("normalizations"))?;
    let mut ayahs: Vec<Ayah> = Vec::with_capacity(ayahs_raw.len());
    for item in ayahs_raw {
        let a = decode_ayah(item, meta)?;
        if ayahs
            .last()
            .is_some_and(|p| a.global_index != p.global_index + 1)
        {
            return Err(malformed("non-contiguous ayahs"));
        }
        ayahs.push(a);
    }
    let normalizations = norms_raw
        .iter()
        .map(|n| decode_normalization(n, source))
        .collect::<Result<Vec<_>, _>>()?;
    check_represented(&ayahs, &normalizations)?;
    Ok(RangeText {
        ayahs,
        normalizations,
    })
}

fn check_represented(ayahs: &[Ayah], norms: &[SurahNormalization]) -> Result<(), OnlineError> {
    let mut surahs: Vec<u16> = ayahs.iter().map(|a| a.surah).collect();
    surahs.dedup();
    let mut ns: Vec<u16> = norms.iter().map(|n| n.surah).collect();
    ns.sort_unstable();
    let mut sorted = surahs.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted != ns {
        return Err(malformed("normalizations do not match ayahs"));
    }
    Ok(())
}

/// `decodeQuranSurahText` / `decodeTranslationSurahText`.
pub fn decode_surah_text(raw: Value, source: &SourceRef) -> Result<SurahText, OnlineError> {
    let v = unwrap_envelope(raw);
    if v["sourceId"].as_str() != Some(source.id()) {
        return Err(malformed("surah source mismatch"));
    }
    let verses = v["verses"]
        .as_array()
        .ok_or(malformed("verses"))?
        .iter()
        .map(|x| x.as_str().map(str::to_owned).ok_or(malformed("verse")))
        .collect::<Result<Vec<_>, _>>()?;
    let normalization = decode_normalization(&v["normalization"], source)?;
    let script = normalization.script;
    if v["script"] != serde_json::to_value(script).unwrap_or(Value::Null) {
        return Err(malformed("surah script mismatch"));
    }
    Ok(SurahText {
        source_id: source.id().to_owned(),
        script,
        verses,
        normalization,
    })
}

/// `stitchRangeChunks`: exact coverage, consistent normalizations.
pub fn stitch_range_chunks(
    chunks: Vec<RangeText>,
    bounds: &[(u32, u32)],
) -> Result<RangeText, OnlineError> {
    let Some(&(mut expected, _)) = bounds.first() else {
        return Err(malformed("range stitch failed"));
    };
    let mut out = RangeText::default();
    for (chunk, &(from, to)) in chunks.into_iter().zip(bounds) {
        let want = (to - from + 1) as usize;
        let ok = chunk.ayahs.len() == want
            && chunk.ayahs.first().map(|a| a.global_index) == Some(from)
            && chunk.ayahs.last().map(|a| a.global_index) == Some(to);
        if !ok {
            return Err(malformed("range stitch failed"));
        }
        for a in chunk.ayahs {
            if a.global_index != expected {
                return Err(malformed("range stitch failed"));
            }
            expected += 1;
            out.ayahs.push(a);
        }
        for n in chunk.normalizations {
            match out.normalizations.iter().find(|e| e.surah == n.surah) {
                Some(existing) if *existing != n => return Err(malformed("range stitch failed")),
                Some(_) => {}
                None => out.normalizations.push(n),
            }
        }
    }
    check_represented(&out.ayahs, &out.normalizations)?;
    Ok(out)
}

/// Breaker state (`QuranApiAvailability`): passive, real reads are probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BreakerState {
    #[default]
    Closed,
    Open,
    HalfOpen,
}

/// Remembers API failures so a down API is not hammered.
#[derive(Debug, Clone, Default)]
pub struct ApiAvailability {
    state: BreakerState,
    open_until: i64,
    server_failures: u32,
}

impl ApiAvailability {
    /// May a request go now? `Ok(true)` when it is the half-open probe.
    pub fn acquire(&mut self, now_ms: i64) -> Result<bool, OnlineError> {
        match self.state {
            BreakerState::Closed => Ok(false),
            BreakerState::HalfOpen => Err(OnlineError::Unavailable),
            BreakerState::Open if now_ms < self.open_until => Err(OnlineError::Unavailable),
            BreakerState::Open => {
                self.state = BreakerState::HalfOpen;
                Ok(true)
            }
        }
    }

    /// Back to closed.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Records a failed read. Malformed data and 4xx are neutral.
    pub fn record_failure(&mut self, error: &OnlineError, probe: bool, now_ms: i64) {
        match error {
            OnlineError::Malformed(_) | OnlineError::Unavailable => self.reset(),
            OnlineError::Http(status) if *status < 500 => self.reset(),
            OnlineError::Http(_) => {
                self.server_failures += 1;
                if probe || self.server_failures >= API_SERVER_FAILURE_THRESHOLD {
                    self.open(now_ms);
                }
            }
            OnlineError::Transport(_) => self.open(now_ms),
        }
    }

    fn open(&mut self, now_ms: i64) {
        self.state = BreakerState::Open;
        self.open_until = now_ms + API_COOLDOWN_MS;
    }
}

/// The read API client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuranApi {
    /// Base without trailing slash (`https://easyquran.fyi/api/quran`).
    pub base: String,
}

impl Default for QuranApi {
    fn default() -> Self {
        Self {
            base: PRODUCTION_API_BASE.to_owned(),
        }
    }
}

async fn get_json(transport: &dyn Transport, url: String) -> Result<Value, OnlineError> {
    let res = transport
        .send(Request::get(url).header("accept", "application/json"))
        .await
        .map_err(OnlineError::Transport)?;
    if !res.ok() {
        return Err(OnlineError::Http(res.status));
    }
    res.json()
        .map_err(|e| OnlineError::Malformed(e.to_string()))
}

impl QuranApi {
    /// An API at `base`.
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_owned(),
        }
    }

    /// `readRange`: global `from..=to` of a source, chunked and stitched.
    pub async fn read_range(
        &self,
        transport: &dyn Transport,
        source_id: &str,
        from: u32,
        to: u32,
        meta: &QuranMeta,
    ) -> Result<RangeText, OnlineError> {
        let source = SourceRef::from_id(source_id);
        let bounds = plan_range_chunks(from, to)?;
        let mut chunks = Vec::with_capacity(bounds.len());
        for &(a, b) in &bounds {
            let url = format!("{}/sources/{source_id}/range?from={a}&to={b}", self.base);
            chunks.push(decode_range_text(
                get_json(transport, url).await?,
                &source,
                meta,
            )?);
        }
        stitch_range_chunks(chunks, &bounds)
    }

    /// `readSurah`.
    pub async fn read_surah(
        &self,
        transport: &dyn Transport,
        source_id: &str,
        surah: u16,
    ) -> Result<SurahText, OnlineError> {
        let source = SourceRef::from_id(source_id);
        let url = format!("{}/sources/{source_id}/surah/{surah}", self.base);
        decode_surah_text(get_json(transport, url).await?, &source)
    }

    /// [`Self::read_range`] through the breaker.
    #[allow(clippy::too_many_arguments)]
    pub async fn read_range_guarded(
        &self,
        transport: &dyn Transport,
        availability: &std::cell::RefCell<ApiAvailability>,
        now_ms: i64,
        source_id: &str,
        from: u32,
        to: u32,
        meta: &QuranMeta,
    ) -> Result<RangeText, OnlineError> {
        let probe = availability.borrow_mut().acquire(now_ms)?;
        let result = self.read_range(transport, source_id, from, to, meta).await;
        match &result {
            Ok(_) => availability.borrow_mut().reset(),
            Err(e) => availability.borrow_mut().record_failure(e, probe, now_ms),
        }
        result
    }
}

/// Whether the read API can serve an Arabic source at all: the backend's
/// `Script::parse` (`rust/backend/api/src/quran/store.rs`) has no
/// `uthmani-annotated`, so that source is local-only (the web worker falls
/// back to its reader seat for the same reason). Production availability
/// of the others is further gated by `/quran/scripts`.
pub fn api_serves(source: ArabicSource) -> bool {
    source != ArabicSource::AnnotatedUthmani
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::meta::meta;

    #[test]
    fn chunk_plan() {
        assert_eq!(plan_range_chunks(1, 300).unwrap(), vec![(1, 300)]);
        assert_eq!(
            plan_range_chunks(1, 301).unwrap(),
            vec![(1, 300), (301, 301)]
        );
        assert!(plan_range_chunks(1, 601).is_err());
        assert!(plan_range_chunks(5, 4).is_err());
    }

    fn translation_range(from: u32, to: u32) -> Value {
        let m = meta();
        let ayahs: Vec<Value> = (from..=to)
            .map(|g| {
                let (s, a) = m.verse_at_global(g).unwrap();
                serde_json::json!({"key": format!("{s}:{a}"), "surah": s, "ayah": a, "globalIndex": g, "text": format!("t{g}")})
            })
            .collect();
        let mut surahs: Vec<u16> = (from..=to)
            .map(|g| m.verse_at_global(g).unwrap().0)
            .collect();
        surahs.dedup();
        let norms: Vec<Value> = surahs
            .iter()
            .map(|s| serde_json::to_value(SurahNormalization::translation("en.sahih", *s)).unwrap())
            .collect();
        serde_json::json!({"data": {"ayahs": ayahs, "normalizations": norms}})
    }

    #[test]
    fn decodes_and_stitches_translation_ranges() {
        let src = SourceRef::from_id("en.sahih");
        let a = decode_range_text(translation_range(1, 300), &src, meta()).unwrap();
        let b = decode_range_text(translation_range(301, 310), &src, meta()).unwrap();
        let s = stitch_range_chunks(vec![a.clone(), b], &[(1, 300), (301, 310)]).unwrap();
        assert_eq!(s.ayahs.len(), 310);
        assert!(stitch_range_chunks(vec![a], &[(2, 301)]).is_err());
        let wrong = decode_range_text(
            translation_range(1, 3),
            &SourceRef::from_id("en.other"),
            meta(),
        );
        assert!(wrong.is_err());
    }

    #[test]
    fn breaker_opens_and_half_opens() {
        let mut b = ApiAvailability::default();
        assert_eq!(b.acquire(0), Ok(false));
        b.record_failure(&OnlineError::Http(503), false, 0);
        assert_eq!(b.acquire(0), Ok(false));
        b.record_failure(&OnlineError::Http(503), false, 0);
        assert_eq!(b.acquire(1), Err(OnlineError::Unavailable));
        assert_eq!(b.acquire(API_COOLDOWN_MS), Ok(true));
        assert_eq!(b.acquire(API_COOLDOWN_MS), Err(OnlineError::Unavailable));
        b.reset();
        b.record_failure(&OnlineError::Http(404), false, 0);
        assert_eq!(b.acquire(0), Ok(false));
    }
}
