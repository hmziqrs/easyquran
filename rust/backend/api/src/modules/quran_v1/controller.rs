//! Quran API request handlers (§6.1). Read-only, in-memory, no SQLite query.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Response, StatusCode};
use chrono::{NaiveDate, Timelike, Utc};
use serde::Serialize;
use sha2::Digest;

use crate::quran::{self, QuranStore, Range, RESPONSE_CAP, Script, SuraMeta, VERSE_COUNT};

use crate::AppState;

use super::cache;
use super::dto::*;
use super::error::{QPath, QQuery, QuranApiError};

// ── error + small helpers ───────────────────────────────────────────────────

fn invalid(msg: impl Into<String>) -> QuranApiError {
    QuranApiError::invalid(msg)
}

fn quran_not_found(msg: impl Into<String>) -> QuranApiError {
    QuranApiError::not_found(msg)
}

/// Range span exceeds the 300-ayah cap → 400 `range_too_large` (§6.1/§6.4). The
/// `{max, requested}` detail rides in the §6.4 `detail` field.
fn range_too_large(requested: u32) -> QuranApiError {
    QuranApiError::range_too_large(requested)
}

fn parse_script(opt: &Option<String>) -> Result<Script, QuranApiError> {
    match opt.as_deref() {
        None => Ok(Script::Uthmani),
        Some(s) => Script::parse(s).ok_or_else(|| {
            invalid(format!("unknown script '{s}'; expected one of: uthmani, simple-clean"))
        }),
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

/// Build a canonical resource key: `base` + sorted explicit query params (§8.1).
/// Same logical request ⇒ same key ⇒ same ETag.
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

/// Wrap `body` in the success envelope and respond with the Arabic cache policy
/// + conditional GET (§8.1).
fn json_cached<T: Serialize>(
    store: &QuranStore,
    headers: &HeaderMap,
    canonical_key: &str,
    body: T,
) -> Response<Body> {
    let env = Envelope::new(body, store.content_version());
    cache::respond_cached(
        &env,
        store.content_version(),
        canonical_key,
        cache::ARABIC_CACHE,
        cache::if_none_match(headers),
    )
}

/// Resolved ayah window after `from`/`to` narrowing + cap/pagination (§6.1).
struct Window {
    lo: u32,
    hi: u32,
    /// Unit total, before from/to/cursor (RangeMeta.total).
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
    // from/to: inclusive within-unit ordinals, required together (§6.1).
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

    // Cap (300) + cursor/limit pagination (§6.1).
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
                return Err(invalid(format!("cursor {c} is outside the window [{lo}, {hi}]")));
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

/// Core range-ayahs responder shared by every `…/ayahs` route (§6.1).
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
    // §8.1 canonical key: every accepted param, defaults explicit, alphabetical.
    // For RangeKind::Global the window is named by fromGlobal/toGlobal (not the
    // within-unit from/to), and they MUST be in the key or two overlapping
    // global ranges with an explicit cursor collide on one ETag (CDN poisoning).
    // The default cursor is the narrowed start `lo`, not `unit_start` (which
    // compute_window would reject if passed explicitly).
    let lo = unit_start + from.map(|n| (n as u32).saturating_sub(1)).unwrap_or(0);
    let mut parts: Vec<(&str, String)> = vec![
        ("cursor", cursor.unwrap_or(lo).to_string()),
        ("limit", limit.map(|l| l.min(RESPONSE_CAP)).unwrap_or(RESPONSE_CAP).to_string()),
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
    range_summary(store, kind, r.index, r.start_global, r.end_global, hizb, quarter_in_hizb)
}

fn quarter_hizb(index: u16) -> (u8, u8) {
    ((((index - 1) / 4) + 1) as u8, (((index - 1) % 4) + 1) as u8)
}

// ── surahs ──────────────────────────────────────────────────────────────────

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
    Ok(json_cached(store, &headers, &format!("surahs/{n}"), sura_dto(s)))
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
        &state, &headers, RangeKind::Surah, Some(n), None, None, s.start_global, s.end_global,
        q.from, q.to, q.cursor, q.limit, script, &format!("surahs/{n}/ayahs"),
    )
}

// ── single ayah + multi ─────────────────────────────────────────────────────

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

/// `/ayahs?keys=2:255,1:1` (up to 50, order preserved) OR
/// `/ayahs?fromGlobal=&toGlobal=` (paginated global range) — §6.1.
pub async fn ayahs_multi(
    State(state): State<AppState>,
    QQuery(q): QQuery<AyahsQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    match (q.keys.as_deref(), q.from_global, q.to_global) {
        (Some(keys), _, _) => {
            let parsed: Vec<&str> = keys.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
            if parsed.is_empty() {
                return Err(invalid("`keys` must contain at least one verse key (e.g. 2:255)"));
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
                let (s, a) = parse_verse_key(k)
                    .ok_or_else(|| invalid(format!("'{k}' is not a valid verse key (surah:ayah)")))?;
                let g = store.meta().global_of(s, a).ok_or_else(|| {
                    quran_not_found(format!("ayah {s}:{a} not found"))
                })?;
                let v = store.ayah_view(script, g).expect("ayah exists");
                ayahs.push(ayah_dto(&v));
            }
            let ck = canonical(
                "ayahs",
                vec![
                    ("keys", keys_csv),
                    ("script", script.as_str().to_string()),
                ],
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
                &state, &headers, RangeKind::Global, None, None, None, from, to, None, None,
                q.cursor, q.limit, script, "ayahs",
            )
        }
        (None, Some(_), None) | (None, None, Some(_)) => {
            Err(invalid("`fromGlobal` and `toGlobal` must be provided together"))
        }
        (None, None, None) => Err(invalid(
            "provide either `keys` (e.g. ?keys=2:255,1:1) or `fromGlobal`+`toGlobal`",
        )),
    }
}

/// `/ayahs/2:255` → 308 to `/quran/v1/ayahs/2/255`; an invalid single segment
/// (`/ayahs/abc`) → 400 (§6.1).
pub async fn ayah_key_redirect(
    QPath(key): QPath<String>,
    axum::extract::RawQuery(rq): axum::extract::RawQuery,
) -> Result<Response<Body>, QuranApiError> {
    let (surah, ayah) = parse_verse_key(&key)
        .ok_or_else(|| invalid(format!("'{key}' is not a valid verse key (expected surah:ayah, e.g. 2:255)")))?;
    // Preserve the query string (e.g. ?script=simple-clean) across the alias
    // redirect — §6.1 calls this one resource under two URLs (§6.1).
    let suffix = rq.filter(|s| !s.is_empty()).map(|s| format!("?{s}")).unwrap_or_default();
    Ok(Response::builder()
        .status(StatusCode::PERMANENT_REDIRECT)
        .header(header::LOCATION, format!("/quran/v1/ayahs/{surah}/{ayah}{suffix}"))
        .body(Body::empty())
        .expect("308 builds"))
}

/// Strict `^[1-9][0-9]{0,2}` ordinal on both sides of the colon.
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

// ── the five range families (juz / page / ruku / hizb-quarter / manzil) ─────

// ── the five range families (juz / page / ruku / hizb-quarter / manzil) ─────
//
// Each family has the same three-route shape (list / one / ayahs) — no family
// is a special case (§6.1). Handlers are written explicitly (not a macro) to
// avoid pulling in a proc-macro dep and to keep each field access self-evident.

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
                &state, &headers, $kind, Some(n), None, None, r.start_global, r.end_global,
                q.from, q.to, q.cursor, q.limit, script, &format!("{}/{n}/ayahs", $path),
            )
        }
    };
}

plain_family!(list_juzs, get_juz, juz_ayahs, juzs, RangeKind::Juz, "juzs", "juzs");
plain_family!(list_pages, get_page, page_ayahs, pages, RangeKind::Page, "pages", "pages");
plain_family!(list_rukus, get_ruku, ruku_ayahs, rukus, RangeKind::Ruku, "rukus", "rukus");
plain_family!(list_manzils, get_manzil, manzil_ayahs, manzils, RangeKind::Manzil, "manzils", "manzils");

pub async fn list_hizb_quarters(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let data = list_family_slice(store, RangeKind::HizbQuarter, &store.meta().hizb_quarters, true);
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
        &state, &headers, RangeKind::HizbQuarter, Some(n), Some(hz), Some(qi),
        r.start_global, r.end_global, q.from, q.to, q.cursor, q.limit, script,
        &format!("hizb-quarters/{n}/ayahs"),
    )
}

// ── sajdas ──────────────────────────────────────────────────────────────────

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
    Ok(json_cached(store, &headers, &format!("sajdas/{n}"), sajda_dto(s)))
}

// ── scripts (§5.1) ──────────────────────────────────────────────────────────

pub async fn scripts(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let cv = store.content_version().to_string();
    // Lazy-once HEAD verification (§5.1): resolve + verify both download URLs the
    // first time /scripts is hit, then serve the cached result for the process.
    // Avoids per-request CDN latency/availability coupling; held under a Mutex
    // so concurrent first-requests single-flight.
    let scripts = {
        let mut guard = state.quran_scripts.lock().await;
        match guard.as_ref() {
            // Cached only on prior full success.
            Some(cached) => cached.to_vec(),
            None => {
                let public_url = state
                    .settings
                    .object_storage
                    .public_url
                    .trim_end_matches('/')
                    .to_string();
                let resolved = resolve_scripts(&state, &cv, &public_url).await;
                // Only pin the cache when BOTH artifacts verified: a transient
                // HEAD failure (5xx/network) or a not-yet-uploaded artifact must
                // self-heal on the next request rather than being cached as
                // permanently omitted for the process life (§5.1).
                if resolved.len() == 2 {
                    *guard = Some(resolved.clone());
                }
                resolved
            }
        }
    };
    // Fold the verified-set state into the validator: an empty/partial response
    // (artifacts transiently unverifiable, pre-self-heal) must NOT share the full
    // list's ETag — else a CDN/conditional-GET pins the empty body as if it were
    // complete (§8.1, §5.1). A partial/unverified response is also not long-cached
    // (no-store) so the next request re-probes and self-heals.
    let verified = scripts.len();
    let canonical = format!("scripts?verified={verified}");
    let cache_control = if verified == 2 {
        cache::ARABIC_CACHE
    } else {
        cache::NO_STORE
    };
    let env = Envelope::new(ScriptsData { scripts }, &cv);
    Ok(cache::respond_cached(
        &env,
        &cv,
        &canonical,
        cache_control,
        cache::if_none_match(&headers),
    ))
}

/// Build both artifact entries and HEAD-verify each download URL with
/// `Accept-Encoding: identity` (§5.1). An artifact that 404s or whose identity
/// `Content-Length` ≠ `sizeBytes` is omitted. Axum never proxies the bytes.
async fn resolve_scripts(state: &AppState, cv: &str, public_url: &str) -> Vec<Artifact> {
    let store = &state.quran;
    let entries = [
        (store.artifacts.uthmani.clone(), "quran-uthmani.sqlite"),
        (store.artifacts.simple_clean.clone(), "quran-simple-clean.sqlite"),
    ];
    let mut out = Vec::with_capacity(2);
    for (file, filename) in entries {
        let id = file.id.as_str().to_string();
        let url = format!("{public_url}/quran/arabic/{id}/{cv}/{filename}");
        match verify_head(&state.http_client, &url, file.size_bytes).await {
            Ok(true) => out.push(Artifact {
                id,
                size_bytes: file.size_bytes,
                sha256: file.sha256.to_string(),
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

/// `GET /openapi.json` — the Quran API OpenAPI document, served on the public
/// branch (§6.1). Feature-gated (`openapi`).
#[cfg(feature = "openapi")]
pub async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    use utoipa::OpenApi;
    axum::Json(crate::docs::ApiDoc::openapi())
}

// ── version (§8.1) ──────────────────────────────────────────────────────────

pub async fn version(
    State(state): State<AppState>,
    QQuery(_): QQuery<NoQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let data = VersionData {
        api_version: "v1".to_string(),
        search_version: quran::SEARCH_VERSION.to_string(),
        source_digests: SourceDigestsDto {
            uthmani: store.source_digests().uthmani.to_string(),
            simple_clean: store.source_digests().simple_clean.to_string(),
        },
        translations: Vec::new(),
    };
    // Fold searchVersion into the validator: a normalization change bumps
    // searchVersion WITHOUT changing contentVersion (§7.1/§8.1), so the body
    // changes while a contentVersion-only ETag would stay fixed → stale 304.
    let cv = store.content_version();
    let etag = cache::weak_etag(&format!("{cv}+{}", quran::SEARCH_VERSION), "version");
    let env = Envelope::new(data, cv);
    Ok(cache::respond_cached_with_etag(
        &env,
        &etag,
        cache::ARABIC_CACHE,
        cache::if_none_match(&headers),
    ))
}

// ── health/ready (§8.4) — operational, not enveloped, no-store ──────────────

/// The first `#[utoipa::path]` annotation (Phase 1c — OpenAPI is greenfield;
/// the `openapi` feature is off by default). Non-enveloped, so the cleanest
/// example; the rest of the surface is added incrementally.
#[cfg_attr(
    feature = "openapi",
    utoipa::path(
        get,
        path = "/quran/v1/health/ready",
        tag = "quran",
        responses((status = 200, description = "Readiness + version/observability surface", body = crate::modules::quran_v1::dto::HealthReady))
    )
)]
pub async fn health_ready(
    State(state): State<AppState>,
    QQuery(_unused): QQuery<NoQuery>,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let body = HealthReady {
        ready: true,
        content_version: store.content_version().to_string(),
        search_version: quran::SEARCH_VERSION.to_string(),
        source_digests: SourceDigestsDto {
            uthmani: store.source_digests().uthmani.to_string(),
            simple_clean: store.source_digests().simple_clean.to_string(),
        },
        verse_count: VERSE_COUNT,
        surah_count: quran::SURA_COUNT as u16,
    };
    let bytes = serde_json::to_vec(&body).expect("health serializes");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(header::CACHE_CONTROL, cache::NO_STORE)
        .body(Body::from(bytes))
        .expect("health response builds"))
}

// ── random (§8.5) ───────────────────────────────────────────────────────────

/// Frozen ayah-of-the-day constants (§8.5). K is coprime to 6236 (1559 prime),
/// giving a full 6236-day cycle; the multiplier destroys the day-to-day +1 march
/// of a plain modulus. Changing these changes the answer for every past date.
const RANDOM_K: i64 = 1103;
const RANDOM_C: i64 = 4177;

pub async fn random(
    State(state): State<AppState>,
    QQuery(q): QQuery<RandomQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    // Read the clock ONCE and derive both the ayah date and the seconds-to-midnight
    // cache TTL from the same instant — two separate reads could straddle 00:00
    // UTC and serve yesterday's ayah with max-age=86400 (stale for a day, §8.5).
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
    let body = RandomAyah { date: date_str.clone(), ayah: ayah_dto(&view) };
    let ck = canonical(
        "random",
        vec![("date", date_str), ("script", script.as_str().to_string())],
    );
    let cache_control = if q.date.is_some() {
        cache::IMMUTABLE_CACHE.to_string()
    } else {
        // Seconds remaining until 00:00 UTC (§8.5): avoid holding yesterday's
        // ayah for most of today. Same `now` as the date above (no straddle).
        let left = 86400 - now.num_seconds_from_midnight() as u64;
        format!("public, max-age={}", left.max(1))
    };
    let env = Envelope::new(body, store.content_version());
    Ok(cache::respond_cached(
        &env,
        store.content_version(),
        &ck,
        &cache_control,
        cache::if_none_match(&headers),
    ))
}

/// The deterministic UTC-date → global-ayah mapping (§8.5). Pure, so the
/// permutation property is testable without HTTP.
///
/// `ayah = ((daysSinceEpoch * K + C) % 6236) + 1` with K=1103, C=4177. K is
/// coprime to 6236 (1559 prime), so every ayah appears exactly once per 6236-day
/// cycle; the multiplier destroys the plain-modulus day-to-day +1 march.
fn random_global(date: NaiveDate) -> u32 {
    let epoch = NaiveDate::from_ymd_opt(1970, 1, 1).expect("epoch");
    let days = date.signed_duration_since(epoch).num_days();
    (((days * RANDOM_K + RANDOM_C).rem_euclid(VERSE_COUNT as i64)) as u32) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §10: `/random` over 6,236 consecutive dates yields a permutation of
    /// `1..=6236`, and no two consecutive dates differ by 1.
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
        // Every ayah appeared exactly once in the 6236-day cycle.
        assert!(seen[1..=VERSE_COUNT as usize].iter().all(|x| *x));
    }
}

fn parse_strict_date(s: &str) -> Result<NaiveDate, QuranApiError> {
    let parts: Vec<&str> = s.split('-').collect();
    let bad = || invalid(format!("date '{s}' is not a strict ISO 8601 calendar date (YYYY-MM-DD)"));
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
        invalid(format!("date '{s}' is not a valid calendar date (e.g. 2026-02-30)"))
    })
}

// ── search (Phase 2, §7.1) ──────────────────────────────────────────────────

/// `GET /search?q=…` — substring search of the normalized simple-clean corpus
/// (§7.1). Performs no SQLite query. Results are ascending by `globalIndex`;
/// `total` is the full match count for pagination.
pub async fn search(
    State(state): State<AppState>,
    QQuery(q): QQuery<SearchQuery>,
    headers: HeaderMap,
) -> Result<Response<Body>, QuranApiError> {
    let store = &state.quran;
    let script = parse_script(&q.script)?;
    // L9: cheap raw-size guard before the (allocating) normalization, so a huge
    // `q` is rejected without a transient oversized allocation.
    if q.q.len() > 512 {
        return Err(invalid("query is too long (max 512 bytes before normalization)"));
    }
    // Normalize q with the SAME function used to build the corpus (§7.1).
    let (norm_q, _q_map) = quran::normalize_arabic(&q.q);
    let scalar_len = norm_q.chars().count();
    if !(3..=64).contains(&scalar_len) {
        return Err(invalid(format!(
            "query must be 3..=64 Unicode scalar values after normalization; got {scalar_len}"
        )));
    }
    // L8: reject limit==0 (parity with the range handler). offset is "capped at
    // 500" (§7.1) — clamp, mirroring limit.min(50), not reject.
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
                .map(|h| Highlight { start: h.start, end: h.end })
                .collect();
            SearchHit {
                ayah: ayah_dto(&view),
                highlights,
            }
        })
        .collect();
    let body = SearchResponse {
        // Echo the NORMALIZED query (not the raw input): the ETag is a function
        // of the normalized query (§8.1), so the body must agree — else two raw
        // queries that normalize identically (e.g. الله vs ٱللَّه) share one ETag
        // while their echoed `query` differs (RFC 9110 representation mismatch).
        query: norm_q.clone(),
        total,
        limit: limit as u16,
        offset: offset as u32,
        results,
    };
    // The ETag folds in searchVersion — search is a pure function of
    // (contentVersion, searchVersion, normalized q, script, limit, offset) (§8.1).
    // L6: fold a hex digest of `norm_q` (not the raw Arabic UTF-8) so the header
    // value stays ASCII and survives strict proxies/CDNs.
    let cv = store.content_version();
    let q_digest = hex::encode(&sha2::Sha256::digest(norm_q.as_bytes())[..8]);
    let etag = cache::weak_etag(
        &format!("{cv}+{}", quran::SEARCH_VERSION),
        &format!("search?limit={limit}&offset={offset}&q={q_digest}&script={}", script.as_str()),
    );
    let env = Envelope::new(body, cv);
    Ok(cache::respond_cached_with_etag(
        &env,
        &etag,
        cache::SEARCH_CACHE,
        cache::if_none_match(&headers),
    ))
}
