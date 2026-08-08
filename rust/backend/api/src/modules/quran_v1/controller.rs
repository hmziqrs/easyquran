use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Response, StatusCode};
use chrono::{NaiveDate, Timelike, Utc};
use serde::Serialize;
use sha2::Digest;

use crate::quran::{self, QuranStore, Range, Script, SuraMeta, RESPONSE_CAP, VERSE_COUNT};

use crate::AppState;

use super::cache;
use super::dto::*;
use super::error::{QPath, QQuery, QuranApiError};

fn invalid(msg: impl Into<String>) -> QuranApiError {
    QuranApiError::invalid(msg)
}

fn quran_not_found(msg: impl Into<String>) -> QuranApiError {
    QuranApiError::not_found(msg)
}

fn range_too_large(requested: u32) -> QuranApiError {
    QuranApiError::range_too_large(requested)
}

fn parse_script(opt: &Option<String>) -> Result<Script, QuranApiError> {
    match opt.as_deref() {
        None => Ok(Script::Uthmani),
        Some(s) => Script::parse(s).ok_or_else(|| {
            invalid(format!(
                "unknown script '{s}'; expected one of: uthmani, simple-clean"
            ))
        }),
    }
}

fn parse_source(
    s: &str,
    pool: &crate::quran::TranslationPool,
) -> Result<crate::quran::SourceId, QuranApiError> {
    if let Some(script) = Script::parse(s) {
        return Ok(crate::quran::SourceId::Arabic(script));
    }
    if let Some(id) = pool.parse_id(s) {
        return Ok(crate::quran::SourceId::Translation(id));
    }
    Err(invalid(format!(
        "unknown source '{s}'; expected an Arabic script (uthmani, simple-clean) or a catalogue translation id"
    )))
}

fn translation_profile(id: &str) -> String {
    format!("tanzil-{id}")
}

impl From<quran::ViewError> for QuranApiError {
    fn from(e: quran::ViewError) -> Self {
        match e {
            quran::ViewError::InvalidSurah(n) | quran::ViewError::Locate(n) => {
                quran_not_found(format!("surah {n} not found (valid 1..=114)"))
            }
            other => QuranApiError::internal(other.to_string()),
        }
    }
}

fn ayah_dto(v: &quran::AyahView) -> Ayah {
    Ayah {
        key: format!("{}:{}", v.surah, v.ayah),
        surah: v.surah,
        ayah: v.ayah,
        global_index: v.global_index,
        text: v.text.to_string(),
        sajda: v.sajda,
        juz: v.juz,
        page: v.page,
        ruku: v.ruku,
        hizb_quarter: v.hizb_quarter,
        manzil: v.manzil,
    }
}

fn sura_dto(s: &SuraMeta) -> SuraDto {
    SuraDto {
        index: s.index,
        ayah_count: s.ayas,
        start_global: s.start_global,
        end_global: s.end_global,
        revelation_order: s.revelation_order,
        ruku_count: s.ruku_count,
        place: s.place,
        name_arabic: s.name_arabic.to_string(),
        name_translit: s.name_translit.to_string(),
        name_english: s.name_english.to_string(),
        bismillah: s.bismillah,
    }
}

fn canonical(base: &str, mut parts: Vec<(&str, String)>) -> String {
    parts.sort_by(|a, b| a.0.cmp(b.0));
    if parts.is_empty() {
        return base.to_string();
    }
    let qs = parts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{base}?{qs}")
}

fn verse_key_of(store: &QuranStore, g: u32) -> VerseKey {
    let (surah, ayah) = store.meta().locate(g).expect("global in range");
    VerseKey::new(surah, ayah)
}

fn lookup_range<'a, K>(
    slice: &'a [Range<K>],
    n: u16,
    what: &str,
) -> Result<&'a Range<K>, QuranApiError> {
    if n == 0 {
        return Err(quran_not_found(format!("{what} {n} not found")));
    }
    slice
        .get((n as usize) - 1)
        .ok_or_else(|| quran_not_found(format!("{what} {n} not found (valid 1..={})", slice.len())))
}

fn json_cached<T: Serialize>(
    store: &QuranStore,
    headers: &HeaderMap,
    canonical_key: &str,
    body: T,
) -> Response<Body> {
    let env = Envelope::new(body);
    cache::respond_cached(
        &env,
        store.etag_tag(),
        canonical_key,
        cache::ARABIC_CACHE,
        cache::if_none_match(headers),
    )
}

fn json_cached_with_tag<T: Serialize>(
    tag: &str,
    headers: &HeaderMap,
    canonical_key: &str,
    body: T,
) -> Response<Body> {
    let env = Envelope::new(body);
    cache::respond_cached(
        &env,
        tag,
        canonical_key,
        cache::ARABIC_CACHE,
        cache::if_none_match(headers),
    )
}

struct Window {
    lo: u32,
    hi: u32,
    total: u32,
    next_cursor: Option<u32>,
}

fn compute_window(
    unit_start: u32,
    unit_end: u32,
    from: Option<u16>,
    to: Option<u16>,
    cursor: Option<u32>,
    limit: Option<u32>,
) -> Result<Window, QuranApiError> {
    let total = unit_end - unit_start + 1;
    let (lo, hi) = match (from, to) {
        (None, None) => (unit_start, unit_end),
        (Some(f), Some(t)) => {
            if f == 0 || t == 0 || f > t || (f as u32) > total || (t as u32) > total {
                return Err(invalid(format!(
                    "invalid range from={f} to={t} for a unit of {total} ayahs (inclusive, 1-based)"
                )));
            }
            (unit_start + (f as u32 - 1), unit_start + (t as u32 - 1))
        }
        _ => return Err(invalid("`from` and `to` must be provided together")),
    };
    let span = hi - lo + 1;

    let page_limit = match limit {
        Some(l) => {
            if l == 0 {
                return Err(invalid("`limit` must be >= 1"));
            }
            l.min(RESPONSE_CAP)
        }
        None => {
            if span > RESPONSE_CAP {
                return Err(range_too_large(span));
            }
            span
        }
    };
    let page_start = match cursor {
        Some(c) => {
            if !(lo..=hi).contains(&c) {
                return Err(invalid(format!(
                    "cursor {c} is outside the window [{lo}, {hi}]"
                )));
            }
            c
        }
        None => lo,
    };
    let page_end = (page_start + page_limit - 1).min(hi);
    let next_cursor = (page_end < hi).then_some(page_end + 1);
    Ok(Window {
        lo: page_start,
        hi: page_end,
        total,
        next_cursor,
    })
}

#[allow(clippy::too_many_arguments)]
fn serve_range_ayahs(
    state: &AppState,
    headers: &HeaderMap,
    kind: RangeKind,
    index: Option<u16>,
    hizb: Option<u8>,
    quarter_in_hizb: Option<u8>,
    unit_start: u32,
    unit_end: u32,
    from: Option<u16>,
    to: Option<u16>,
    cursor: Option<u32>,
    limit: Option<u32>,
    script: Script,
    canonical_base: &str,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let win = compute_window(unit_start, unit_end, from, to, cursor, limit)?;
    let ayahs: Vec<Ayah> = (win.lo..=win.hi)
        .map(|g| {
            store
                .ayah_view(script, g)
                .map(|v| ayah_dto(&v))
                .expect("ayah in validated range")
        })
        .collect();
    let total = win.total;
    let range = RangeMeta {
        kind,
        index,
        hizb,
        quarter_in_hizb,
        start_global: unit_start,
        end_global: unit_end,
        first: verse_key_of(store, unit_start),
        last: verse_key_of(store, unit_end),
        count: ayahs.len() as u32,
        total,
        script,
        next_cursor: win.next_cursor,
    };
    let body = AyahRange { range, ayahs };
    // Global ranges key on fromGlobal/toGlobal so overlapping ranges sharing a cursor don't collide on one ETag.
    let lo = unit_start + from.map(|n| (n as u32).saturating_sub(1)).unwrap_or(0);
    let mut parts: Vec<(&str, String)> = vec![
        ("cursor", cursor.unwrap_or(lo).to_string()),
        (
            "limit",
            limit
                .map(|l| l.min(RESPONSE_CAP))
                .unwrap_or(RESPONSE_CAP)
                .to_string(),
        ),
        ("script", script.as_str().to_string()),
    ];
    if matches!(kind, RangeKind::Global) {
        parts.push(("fromGlobal", unit_start.to_string()));
        parts.push(("toGlobal", unit_end.to_string()));
    } else {
        parts.push(("from", from.map(|n| n as u32).unwrap_or(1).to_string()));
        parts.push(("to", to.map(|n| n as u32).unwrap_or(total).to_string()));
    }
    let ck = canonical(canonical_base, parts);
    Ok(json_cached(store, headers, &ck, body))
}

fn range_summary(
    store: &QuranStore,
    kind: RangeKind,
    index: u16,
    start_global: u32,
    end_global: u32,
    hizb: Option<u8>,
    quarter_in_hizb: Option<u8>,
) -> RangeSummary {
    RangeSummary {
        kind,
        index,
        hizb,
        quarter_in_hizb,
        start_global,
        end_global,
        first: verse_key_of(store, start_global),
        last: verse_key_of(store, end_global),
        total: end_global - start_global + 1,
    }
}

fn range_summary_from<K>(
    store: &QuranStore,
    kind: RangeKind,
    r: &Range<K>,
    hizb: Option<u8>,
    quarter_in_hizb: Option<u8>,
) -> RangeSummary {
    range_summary(
        store,
        kind,
        r.index,
        r.start_global,
        r.end_global,
        hizb,
        quarter_in_hizb,
    )
}

fn quarter_hizb(index: u16) -> (u8, u8) {
    ((((index - 1) / 4) + 1) as u8, (((index - 1) % 4) + 1) as u8)
}

pub async fn list_surahs(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let data: Vec<SuraDto> = store.meta().suras().iter().map(sura_dto).collect();
    Ok(json_cached(store, &headers, "surahs", data))
}

pub async fn get_surah(
    State(state): State<AppState>,
    QPath(n): QPath<u16>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let s = store
        .meta()
        .sura(n)
        .ok_or_else(|| quran_not_found(format!("surah {n} not found (valid 1..=114)")))?;
    Ok(json_cached(
        store,
        &headers,
        &format!("surahs/{n}"),
        sura_dto(s),
    ))
}

pub async fn surah_ayahs(
    State(state): State<AppState>,
    QPath(n): QPath<u16>,
    QQuery(q): QQuery<RangeAyahsQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let s = store
        .meta()
        .sura(n)
        .ok_or_else(|| quran_not_found(format!("surah {n} not found (valid 1..=114)")))?;
    let script = parse_script(&q.script)?;
    serve_range_ayahs(
        &state,
        &headers,
        RangeKind::Surah,
        Some(n),
        None,
        None,
        s.start_global,
        s.end_global,
        q.from,
        q.to,
        q.cursor,
        q.limit,
        script,
        &format!("surahs/{n}/ayahs"),
    )
}

pub async fn get_ayah(
    State(state): State<AppState>,
    QPath((surah, ayah)): QPath<(u16, u16)>,
    QQuery(q): QQuery<ScriptQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    let g = store
        .meta()
        .global_of(surah, ayah)
        .ok_or_else(|| quran_not_found(format!("ayah {surah}:{ayah} not found")))?;
    let view = store
        .ayah_view(script, g)
        .ok_or_else(|| quran_not_found(format!("ayah {surah}:{ayah} not found")))?;
    let ck = canonical(
        &format!("ayahs/{surah}/{ayah}"),
        vec![("script", script.as_str().to_string())],
    );
    Ok(json_cached(store, &headers, &ck, ayah_dto(&view)))
}

pub async fn ayahs_multi(
    State(state): State<AppState>,
    QQuery(q): QQuery<AyahsQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    match (q.keys.as_deref(), q.from_global, q.to_global) {
        (Some(keys), _, _) => {
            if keys.len() > 1024 {
                return Err(invalid(
                    "`keys` is too long (max ~1024 bytes; up to 50 verse keys)",
                ));
            }
            let parsed: Vec<&str> = keys
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            if parsed.is_empty() {
                return Err(invalid(
                    "`keys` must contain at least one verse key (e.g. 2:255)",
                ));
            }
            if parsed.len() > 50 {
                return Err(invalid(format!(
                    "`keys` accepts at most 50 verse keys; got {}",
                    parsed.len()
                )));
            }
            let keys_csv = parsed.join(",");
            let mut ayahs = Vec::with_capacity(parsed.len());
            for &k in &parsed {
                let (s, a) = parse_verse_key(k).ok_or_else(|| {
                    invalid(format!("'{k}' is not a valid verse key (surah:ayah)"))
                })?;
                let g = store
                    .meta()
                    .global_of(s, a)
                    .ok_or_else(|| quran_not_found(format!("ayah {s}:{a} not found")))?;
                let v = store.ayah_view(script, g).expect("ayah exists");
                ayahs.push(ayah_dto(&v));
            }
            let ck = canonical(
                "ayahs",
                vec![("keys", keys_csv), ("script", script.as_str().to_string())],
            );
            Ok(json_cached(store, &headers, &ck, AyahsList { ayahs }))
        }
        (None, Some(from), Some(to)) => {
            if from == 0 || to == 0 || from > to || from > VERSE_COUNT || to > VERSE_COUNT {
                return Err(invalid(format!(
                    "fromGlobal={from} toGlobal={to} invalid (inclusive, 1..={VERSE_COUNT}, required together)"
                )));
            }
            serve_range_ayahs(
                &state,
                &headers,
                RangeKind::Global,
                None,
                None,
                None,
                from,
                to,
                None,
                None,
                q.cursor,
                q.limit,
                script,
                "ayahs",
            )
        }
        (None, Some(_), None) | (None, None, Some(_)) => Err(invalid(
            "`fromGlobal` and `toGlobal` must be provided together",
        )),
        (None, None, None) => Err(invalid(
            "provide either `keys` (e.g. ?keys=2:255,1:1) or `fromGlobal`+`toGlobal`",
        )),
    }
}

pub async fn ayah_key_redirect(
    QPath(key): QPath<String>,
    axum::extract::RawQuery(rq): axum::extract::RawQuery,
) -> Result<Response<Body>, QuranApiError> {
    let (surah, ayah) = parse_verse_key(&key).ok_or_else(|| {
        invalid(format!(
            "'{key}' is not a valid verse key (expected surah:ayah, e.g. 2:255)"
        ))
    })?;
    let suffix = rq
        .filter(|s| !s.is_empty())
        .map(|s| format!("?{s}"))
        .unwrap_or_default();
    Ok(Response::builder()
        .status(StatusCode::PERMANENT_REDIRECT)
        .header(
            header::LOCATION,
            format!("/quran/ayahs/{surah}/{ayah}{suffix}"),
        )
        .body(Body::empty())
        .expect("308 builds"))
}

fn parse_verse_key(s: &str) -> Option<(u16, u16)> {
    let (a, b) = s.split_once(':')?;
    if !valid_ordinal(a) || !valid_ordinal(b) {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn valid_ordinal(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_digit() && c != '0')
        && chars.all(|c| c.is_ascii_digit())
        && s.len() <= 3
}

fn list_family_slice<K>(
    store: &QuranStore,
    kind: RangeKind,
    slice: &[Range<K>],
    include_hizb: bool,
) -> Vec<RangeSummary> {
    slice
        .iter()
        .map(|r| {
            if include_hizb {
                let (h, q) = quarter_hizb(r.index);
                range_summary_from(store, kind, r, Some(h), Some(q))
            } else {
                range_summary_from(store, kind, r, None, None)
            }
        })
        .collect()
}

macro_rules! plain_family {
    ($list:ident, $get:ident, $ayahs:ident, $field:ident, $kind:expr, $plural:literal, $path:literal) => {
        pub async fn $list(
            State(state): State<AppState>,
            QQuery(_): QQuery<NoQuery>,
            headers: HeaderMap,
        ) -> Result<Response<Body>, QuranApiError> {
            let store = &state.quran;
            let data = list_family_slice(store, $kind, &store.meta().$field, false);
            Ok(json_cached(store, &headers, $plural, data))
        }

        pub async fn $get(
            State(state): State<AppState>,
            QPath(n): QPath<u16>,
            QQuery(_): QQuery<NoQuery>,
            headers: HeaderMap,
        ) -> Result<Response<Body>, QuranApiError> {
            let store = &state.quran;
            let r = lookup_range(&store.meta().$field, n, $plural)?;
            Ok(json_cached(
                store,
                &headers,
                &format!("{plural}/{n}", plural = $plural),
                range_summary_from(store, $kind, r, None, None),
            ))
        }

        pub async fn $ayahs(
            State(state): State<AppState>,
            QPath(n): QPath<u16>,
            QQuery(q): QQuery<RangeAyahsQuery>,
            headers: HeaderMap,
        ) -> Result<Response<Body>, QuranApiError> {
            let store = &state.quran;
            let r = lookup_range(&store.meta().$field, n, $plural)?;
            let script = parse_script(&q.script)?;
            serve_range_ayahs(
                &state,
                &headers,
                $kind,
                Some(n),
                None,
                None,
                r.start_global,
                r.end_global,
                q.from,
                q.to,
                q.cursor,
                q.limit,
                script,
                &format!("{}/{n}/ayahs", $path),
            )
        }
    };
}

plain_family!(
    list_juzs,
    get_juz,
    juz_ayahs,
    juzs,
    RangeKind::Juz,
    "juzs",
    "juzs"
);
plain_family!(
    list_pages,
    get_page,
    page_ayahs,
    pages,
    RangeKind::Page,
    "pages",
    "pages"
);
plain_family!(
    list_rukus,
    get_ruku,
    ruku_ayahs,
    rukus,
    RangeKind::Ruku,
    "rukus",
    "rukus"
);
plain_family!(
    list_manzils,
    get_manzil,
    manzil_ayahs,
    manzils,
    RangeKind::Manzil,
    "manzils",
    "manzils"
);

pub async fn list_hizb_quarters(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let data = list_family_slice(
        store,
        RangeKind::HizbQuarter,
        &store.meta().hizb_quarters,
        true,
    );
    Ok(json_cached(store, &headers, "hizb-quarters", data))
}

pub async fn get_hizb_quarter(
    State(state): State<AppState>,
    QPath(n): QPath<u16>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let r = lookup_range(&store.meta().hizb_quarters, n, "hizb-quarter")?;
    let (hz, qi) = quarter_hizb(r.index);
    Ok(json_cached(
        store,
        &headers,
        &format!("hizb-quarters/{n}"),
        range_summary_from(store, RangeKind::HizbQuarter, r, Some(hz), Some(qi)),
    ))
}

pub async fn hizb_quarter_ayahs(
    State(state): State<AppState>,
    QPath(n): QPath<u16>,
    QQuery(q): QQuery<RangeAyahsQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let r = lookup_range(&store.meta().hizb_quarters, n, "hizb-quarter")?;
    let script = parse_script(&q.script)?;
    let (hz, qi) = quarter_hizb(r.index);
    serve_range_ayahs(
        &state,
        &headers,
        RangeKind::HizbQuarter,
        Some(n),
        Some(hz),
        Some(qi),
        r.start_global,
        r.end_global,
        q.from,
        q.to,
        q.cursor,
        q.limit,
        script,
        &format!("hizb-quarters/{n}/ayahs"),
    )
}

fn sajda_dto(s: &quran::Sajda) -> SajdaDto {
    SajdaDto {
        index: s.index,
        surah: s.sura,
        ayah: s.aya,
        global_index: s.global_index,
        kind: s.kind,
    }
}

pub async fn list_sajdas(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let data: Vec<SajdaDto> = store.meta().sajdas.iter().map(sajda_dto).collect();
    Ok(json_cached(store, &headers, "sajdas", data))
}

pub async fn get_sajda(
    State(state): State<AppState>,
    QPath(n): QPath<u16>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let s = store
        .meta()
        .sajdas
        .iter()
        .find(|s| s.index == n)
        .ok_or_else(|| quran_not_found(format!("sajda {n} not found (valid 1..=15)")))?;
    Ok(json_cached(
        store,
        &headers,
        &format!("sajdas/{n}"),
        sajda_dto(s),
    ))
}

pub async fn scripts(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let scripts = {
        let mut guard = state.quran_scripts.lock().await;
        match guard.as_ref() {
            Some(cached) => cached.to_vec(),
            None => {
                let public_url = state
                    .settings
                    .object_storage
                    .public_url
                    .trim_end_matches('/')
                    .to_string();
                let resolved = resolve_scripts(&state, &public_url).await;
                // Only cache once fully verified: a transient HEAD failure must self-heal, not be pinned incomplete for life.
                if resolved.len() == 2 {
                    *guard = Some(resolved.clone());
                }
                resolved
            }
        }
    };
    // Partial responses use no-store and a distinct ETag, or a CDN pins the empty body as complete.
    let verified = scripts.len();
    let canonical = format!("scripts?verified={verified}");
    let cache_control = if verified == 2 {
        cache::ARABIC_CACHE
    } else {
        cache::NO_STORE
    };
    let env = Envelope::new(ScriptsData { scripts });
    Ok(cache::respond_cached(
        &env,
        store.etag_tag(),
        &canonical,
        cache_control,
        cache::if_none_match(&headers),
    ))
}

async fn resolve_scripts(state: &AppState, public_url: &str) -> Vec<Artifact> {
    let store = &state.quran;
    let entries = [
        (store.artifacts.uthmani.clone(), "quran-uthmani.sqlite"),
        (
            store.artifacts.simple_clean.clone(),
            "quran-simple-clean.sqlite",
        ),
    ];
    let mut out = Vec::with_capacity(2);
    for (file, filename) in entries {
        let id = file.id.as_str().to_string();
        let url = format!("{public_url}/tanzil/arabic/{filename}");
        match verify_head(&state.http_client, &url, file.size_bytes).await {
            Ok(true) => out.push(Artifact {
                id,
                size_bytes: file.size_bytes,
                download_url: url,
            }),
            _ => tracing::warn!(
                script = %id,
                url = %url,
                "/scripts artifact HEAD check failed; omitting (§5.1)"
            ),
        }
    }
    out
}

async fn verify_head(client: &reqwest::Client, url: &str, expected_size: u64) -> Result<bool, ()> {
    let resp = client
        .head(url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .await
        .map_err(|_| ())?;
    if !resp.status().is_success() {
        return Ok(false);
    }
    let len = resp
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    Ok(len == Some(expected_size))
}

pub async fn sources(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let total = 2 + state.translation_pool.catalogue().len();
    let sources = {
        let mut guard = state.quran_sources.lock().await;
        match guard.as_ref() {
            Some(cached) => cached.to_vec(),
            None => {
                let public_url = state
                    .settings
                    .object_storage
                    .public_url
                    .trim_end_matches('/')
                    .to_string();
                let resolved = resolve_sources(&state, &public_url).await;
                // Memoize only when fully verified — a transient HEAD failure self-heals rather
                // than pinning a truncated list.
                if resolved.len() == total {
                    *guard = Some(resolved.clone());
                }
                resolved
            }
        }
    };
    let verified = sources.len();
    let canonical = format!("sources?verified={verified}");
    let cache_control = if verified == total {
        cache::ARABIC_CACHE
    } else {
        cache::NO_STORE
    };
    let tag = store.etag_tag().to_string();
    let env = Envelope::new(SourcesData { sources });
    Ok(cache::respond_cached(
        &env,
        &tag,
        &canonical,
        cache_control,
        cache::if_none_match(&headers),
    ))
}

/// Every readable source — 2 Arabic scripts + every catalogue translation — each HEAD-verified.
/// A `JoinSet` fans the cold HEADs out concurrently so 117 checks do not serialize past the
/// request budget. An unverified entry is omitted; the handler then stamps `no-store` so a CDN
/// cannot pin a truncated half-list (same discipline as `/scripts`).
async fn resolve_sources(state: &AppState, public_url: &str) -> Vec<SourceDto> {
    let store = &state.quran;
    let mut candidates: Vec<(SourceDto, u64)> =
        Vec::with_capacity(2 + state.translation_pool.catalogue().len());

    for (file, filename, display) in [
        (&store.artifacts.uthmani, "quran-uthmani.sqlite", "Uthmani"),
        (
            &store.artifacts.simple_clean,
            "quran-simple-clean.sqlite",
            "Simple Clean",
        ),
    ] {
        let url = format!("{public_url}/tanzil/arabic/{filename}");
        candidates.push((
            SourceDto {
                id: file.id.as_str().to_string(),
                kind: SourceKind::Arabic,
                language: "Arabic".to_string(),
                language_code: "ar".to_string(),
                direction: "rtl".to_string(),
                name: display.to_string(),
                translator: None,
                size_bytes: file.size_bytes,
                download_url: url,
            },
            file.size_bytes,
        ));
    }

    for e in state.translation_pool.catalogue().values() {
        let url = format!("{public_url}/tanzil/translations/{}", e.path);
        candidates.push((
            SourceDto {
                id: e.id.to_string(),
                kind: SourceKind::Translation,
                language: e.language.to_string(),
                language_code: e.language_code.to_string(),
                direction: e.direction.to_string(),
                name: e.name.to_string(),
                translator: e.translator.as_deref().map(String::from),
                size_bytes: e.size_bytes,
                download_url: url,
            },
            e.size_bytes,
        ));
    }

    let client = state.http_client.clone();
    // Bound concurrent HEADs so a cold /sources resolve cannot consume the shared http_client's
    // whole connection pool (also used by mail/billing/FCM). 32 keeps 117 HEADs snappy on R2.
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    let mut set = tokio::task::JoinSet::new();
    for (idx, (dto, size)) in candidates.iter().enumerate() {
        let client = client.clone();
        let url = dto.download_url.clone();
        let size = *size;
        let sem = sem.clone();
        set.spawn(async move {
            let _permit = sem.acquire().await.expect("HEAD semaphore not closed");
            (idx, verify_head(&client, &url, size).await.unwrap_or(false))
        });
    }
    let mut ok = vec![false; candidates.len()];
    while let Some(res) = set.join_next().await {
        let (idx, verified) = res.expect("HEAD task must not panic");
        ok[idx] = verified;
    }
    candidates
        .into_iter()
        .zip(ok)
        .filter_map(|((dto, _), keep)| keep.then_some(dto))
        .collect()
}

#[cfg(feature = "openapi")]
pub async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    use utoipa::OpenApi;
    axum::Json(crate::docs::ApiDoc::openapi())
}

#[cfg_attr(
    feature = "openapi",
    utoipa::path(
        get,
        path = "/quran/health/ready",
        tag = "quran",
        responses((status = 200, description = "Readiness + observability surface", body = crate::modules::quran_v1::dto::HealthReady))
    )
)]
pub async fn health_ready(
    State(state): State<AppState>,
    QQuery(_unused): QQuery<NoQuery>,
) -> Result<Response<Body>, QuranApiError> {
    let pool = state.translation_pool.stats().await;
    let body = HealthReady {
        ready: true,
        verse_count: VERSE_COUNT,
        surah_count: quran::SURA_COUNT as u16,
        arabic_resident_bytes: (state.quran.uthmani.bytes() + state.quran.simple_clean.bytes())
            as u64,
        loading: QuranLoadingHealth {
            arabic_load_duration_ms: state.quran_runtime_metrics.arabic_load_duration_ms,
            translation_catalogue_load_duration_ms: state
                .quran_runtime_metrics
                .translation_catalogue_load_duration_ms,
            translation_catalogue_entries: state
                .quran_runtime_metrics
                .translation_catalogue_entries,
        },
        translation_pool: TranslationPoolHealth {
            resident_count: pool.resident_count,
            resident_bytes: pool.resident_bytes,
            max_resident_count: state.settings.quran.max_resident_translations,
            max_resident_bytes: state.settings.quran.max_resident_bytes,
            idle_ttl_seconds: state.settings.quran.translation_idle_ttl_secs,
            builds: pool.builds,
            lookups: pool.lookups,
            hit_rate: pool.hit_rate,
            evictions: pool.evictions,
            evictions_per_minute: pool.evictions_per_minute,
        },
    };
    let bytes = serde_json::to_vec(&body).expect("health serializes");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, cache::NO_STORE)
        .body(Body::from(bytes))
        .expect("health response builds"))
}

const RANDOM_K: i64 = 1103;
const RANDOM_C: i64 = 4177;

pub async fn random(
    State(state): State<AppState>,
    QQuery(q): QQuery<RandomQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    // Read the clock once: deriving date and TTL from two reads can straddle 00:00 UTC and serve yesterday's ayah for a full day.
    let now = Utc::now();
    let date = match q.date.as_deref() {
        None => now.date_naive(),
        Some(s) => parse_strict_date(s)?,
    };
    let g = random_global(date);
    let view = store
        .ayah_view(script, g)
        .ok_or_else(|| quran_not_found("random ayah resolved out of range"))?;
    let date_str = date.format("%Y-%m-%d").to_string();
    let body = RandomAyah {
        date: date_str.clone(),
        ayah: ayah_dto(&view),
    };
    let ck = canonical(
        "random",
        vec![("date", date_str), ("script", script.as_str().to_string())],
    );
    let cache_control = if q.date.is_some() {
        cache::IMMUTABLE_CACHE.to_string()
    } else {
        let left = 86400 - now.num_seconds_from_midnight() as u64;
        format!("public, max-age={}", left.max(1))
    };
    let env = Envelope::new(body);
    Ok(cache::respond_cached(
        &env,
        store.etag_tag(),
        &ck,
        &cache_control,
        cache::if_none_match(&headers),
    ))
}

fn random_global(date: NaiveDate) -> u32 {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch");
    let days = date.signed_duration_since(epoch).num_days();
    (((days * RANDOM_K + RANDOM_C).rem_euclid(VERSE_COUNT as i64)) as u32) + 1
}

fn parse_strict_date(s: &str) -> Result<NaiveDate, QuranApiError> {
    let parts: Vec<&str> = s.split('-').collect();
    let bad = || {
        invalid(format!(
            "date '{s}' is not a strict ISO 8601 calendar date (YYYY-MM-DD)"
        ))
    };
    if parts.len() != 3
        || parts[0].len() != 4
        || parts[1].len() != 2
        || parts[2].len() != 2
        || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(bad());
    }
    let (y, m, d) = match (
        parts[0].parse::<i32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) {
        (Ok(y), Ok(m), Ok(d)) => (y, m, d),
        _ => return Err(bad()),
    };
    NaiveDate::from_ymd_opt(y, m, d).ok_or_else(|| {
        invalid(format!(
            "date '{s}' is not a valid calendar date (e.g. 2026-02-30)"
        ))
    })
}

pub async fn search(
    State(state): State<AppState>,
    QQuery(q): QQuery<SearchQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    if q.q.len() > 512 {
        return Err(invalid(
            "query is too long (max 512 bytes before normalization)",
        ));
    }
    let (norm_q, _q_map) = quran::normalize_arabic(&q.q);
    let scalar_len = norm_q.chars().count();
    let has_ornament = quran::contains_searchable_ornament(&norm_q);
    if scalar_len == 0 || scalar_len > 64 || (scalar_len < 3 && !has_ornament) {
        return Err(invalid(format!(
            "query must be 3..=64 Unicode scalar values after normalization (or contain a Quranic ornament mark below 3); got {scalar_len}"
        )));
    }
    let limit = q.limit.unwrap_or(20);
    if limit == 0 {
        return Err(invalid("`limit` must be >= 1"));
    }
    let limit = limit.min(50) as usize;
    let offset = q.offset.unwrap_or(0).min(500) as usize;

    let (total, globals) = store.search.search(&norm_q, limit, offset);
    let results: Vec<SearchHit> = globals
        .into_iter()
        .map(|g| {
            let view = store
                .ayah_view(script, g)
                .expect("a search match is a valid ayah");
            let highlights = quran::highlight(view.text, &norm_q)
                .into_iter()
                .map(|h| Highlight {
                    start: h.start,
                    end: h.end,
                })
                .collect();
            SearchHit {
                kind: SearchHitKind::Ayah,
                ayah: ayah_dto(&view),
                highlights,
            }
        })
        .collect();
    let body = SearchResponse {
        // Echo the normalized query: the ETag keys on it, so echoing raw makes two same-normalizing queries share an ETag with differing bodies.
        query: norm_q.clone(),
        total,
        limit: limit as u16,
        offset: offset as u32,
        results,
    };
    let q_digest = hex::encode(&sha2::Sha256::digest(norm_q.as_bytes())[..8]);
    let etag = cache::weak_etag(
        store.etag_tag(),
        &format!(
            "search?limit={limit}&offset={offset}&q={q_digest}&script={}",
            script.as_str()
        ),
    );
    let env = Envelope::new(body);
    Ok(cache::respond_cached_with_etag(
        &env,
        &etag,
        cache::SEARCH_CACHE,
        cache::if_none_match(&headers),
    ))
}

pub async fn source_surah(
    State(state): State<AppState>,
    QPath((source, surah)): QPath<(String, u16)>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let source_id = parse_source(&source, &state.translation_pool)?;
    if store.meta().sura(surah).is_none() {
        return Err(quran_not_found(format!(
            "surah {surah} not found (valid 1..=114)"
        )));
    }
    match source_id {
        quran::SourceId::Arabic(script) => {
            let body = quran::surah_text_view(store, script, surah)?;
            Ok(json_cached(
                store,
                &headers,
                &format!("sources/{source}/surah/{surah}"),
                body,
            ))
        }
        quran::SourceId::Translation(id) => {
            let profile = translation_profile(id.as_str());
            let corpus = state
                .translation_pool
                .get_or_build(&id)
                .await
                .map_err(|e| QuranApiError::internal(e.to_string()))?;
            let body =
                quran::surah_text_translation(store.meta(), &corpus, id.as_str(), &profile, surah)?;
            Ok(json_cached_with_tag(
                &profile,
                &headers,
                &format!("sources/{source}/surah/{surah}"),
                body,
            ))
        }
    }
}

pub async fn source_range(
    State(state): State<AppState>,
    QPath(source): QPath<String>,
    QQuery(q): QQuery<RangeTextQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let source_id = parse_source(&source, &state.translation_pool)?;
    let from = q.from.unwrap_or(1).max(1);
    let to = q.to.unwrap_or(VERSE_COUNT).min(VERSE_COUNT);
    if from > to {
        return Err(invalid(format!("from ({from}) must be <= to ({to})")));
    }
    let count = to - from + 1;
    if count > RESPONSE_CAP {
        return Err(range_too_large(count));
    }

    // Resolve the verse-text source once: Arabic reads the resident store; a translation
    // cold-loads its corpus through the pool (single-flight). Both stay immutable reads.
    let script = match &source_id {
        quran::SourceId::Arabic(s) => Some(*s),
        quran::SourceId::Translation(_) => None,
    };
    let corpus = match &source_id {
        quran::SourceId::Translation(id) => Some(
            state
                .translation_pool
                .get_or_build(id)
                .await
                .map_err(|e| QuranApiError::internal(e.to_string()))?,
        ),
        quran::SourceId::Arabic(_) => None,
    };
    let profile = match &source_id {
        quran::SourceId::Translation(id) => translation_profile(id.as_str()),
        quran::SourceId::Arabic(_) => store.etag_tag().to_string(),
    };

    let mut ayahs = Vec::with_capacity(count as usize);
    let mut represented: Vec<u16> = Vec::new();
    let mut prev_surah: Option<u16> = None;
    for g in from..=to {
        let (surah, ayah) = store
            .meta()
            .locate(g)
            .ok_or_else(|| QuranApiError::internal(format!("global {g} not locatable")))?;
        let text = match script {
            Some(sc) => store
                .verse(sc, g)
                .ok_or_else(|| QuranApiError::internal(format!("verse {g} missing")))?
                .to_string(),
            None => corpus
                .as_ref()
                .expect("translation corpus resolved above")
                .verse(g)
                .ok_or_else(|| QuranApiError::internal(format!("verse {g} missing")))?
                .to_string(),
        };
        ayahs.push(LeanAyah {
            key: format!("{surah}:{ayah}"),
            surah,
            ayah,
            global_index: g,
            text,
        });
        if prev_surah != Some(surah) {
            represented.push(surah);
            prev_surah = Some(surah);
        }
    }
    let mut normalizations = Vec::with_capacity(represented.len());
    for surah in &represented {
        let norm = match script {
            Some(sc) => quran::surah_normalization(store, sc, *surah)?,
            None => quran::normalization_translation(source.as_str(), &profile, *surah)?,
        };
        normalizations.push(norm);
    }
    let body = RangeText {
        ayahs,
        normalizations,
    };
    Ok(json_cached_with_tag(
        &profile,
        &headers,
        &canonical(
            &format!("sources/{source}/range"),
            vec![("from", from.to_string()), ("to", to.to_string())],
        ),
        body,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_is_full_permutation_no_consecutive_diff_one() {
        let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        let mut seen = vec![false; VERSE_COUNT as usize + 1];
        let mut prev: Option<u32> = None;
        for d in 0..VERSE_COUNT {
            let date = epoch + chrono::Duration::days(d as i64);
            let g = random_global(date);
            assert!((1..=VERSE_COUNT).contains(&g), "day {d} → out of range {g}");
            assert!(!seen[g as usize], "duplicate {g} → not a permutation");
            seen[g as usize] = true;
            if let Some(p) = prev {
                assert!(
                    (g as i32 - p as i32).abs() != 1,
                    "consecutive dates {p} → {g} differ by 1 (predictable march)"
                );
            }
            prev = Some(g);
        }
        assert!(seen[1..=VERSE_COUNT as usize].iter().all(|x| *x));
    }
}
