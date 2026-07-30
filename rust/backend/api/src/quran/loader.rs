//! Boot loader for the immutable in-memory `QuranStore` (Phase 0, §4.1).
//!
//! This is the **only** place that touches SQLite for Quran content. It opens
//! its OWN read-only connections to the two Arabic artifacts (independent of
//! `sea_db`, §2), reads all rows in global-index order, parses the metadata
//! XML, tiles every navigation range, asserts the golden digests and the
//! §3.1/§3.2 invariants, computes the BLAKE3 `contentVersion`, and returns a
//! fully-validated store. `modules/quran_v1/` contains no sqlx/SQLite reference
//! (§10) — that absence is enforced structurally by owning the connections here.
//!
//! **Failure policy: fail fast (§4.1).** Any missing/corrupt source, XML
//! failure, tiling failure, or digest mismatch is a `QuranLoadError`; `main`
//! logs the specific invariant and exits non-zero. A partially loaded or
//! invariant-violating store would serve wrong scripture silently, which is
//! worse than an unavailable process.

use std::marker::PhantomData;
use std::sync::Arc;

use roxmltree::Node;
use sha2::{Digest, Sha256};
use sqlx::Connection;
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

use crate::config::QuranSettings;

use super::store::{
    ArtifactFile, Artifacts, Bismillah, Corpus, HizbQuarter, Juz, Manzil, Page, QuranMeta,
    QuranStore, Range, Ruku, SURA_COUNT, Sajda, SajdaKind, Script, SourceDigests, VERSE_COUNT,
};

/// Golden corpus-text digests from §3.3 — `sha256` of all 6,236 texts joined by
/// `\n` in global-index order (no trailing newline). Verified empirically
/// against the committed artifacts.
const GOLDEN_UTHMANI: &str = "32cc746d817cad9fd4366c7597bfceb177e7649233616c0a80309074b2eb99ee";
const GOLDEN_SIMPLE_CLEAN: &str = "375934722ccbfab0d97754df464deac0dcffe962dc0632cc1ce5c6ca25dcea67";

/// A loaded row from `quran_text`, in `index` (global) order.
struct CorpusRow {
    index: u32,
    sura: u16,
    aya: u16,
    text: String,
}

/// Fail-fast boot error carrying the specific invariant that failed (§4.1).
#[derive(Debug, thiserror::Error)]
pub enum QuranLoadError {
    #[error("quran source file unreadable ({what}): {source}")]
    File {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("quran sqlite open/read ({what}): {source}")]
    Sqlite {
        what: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("quran metadata xml parse: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("quran invariant violated: {0}")]
    Invariant(String),
}

fn inv<S: Into<String>>(msg: S) -> QuranLoadError {
    QuranLoadError::Invariant(msg.into())
}

fn invariant(cond: bool, msg: impl FnOnce() -> String) -> Result<(), QuranLoadError> {
    if cond {
        Ok(())
    } else {
        Err(inv(msg()))
    }
}

/// Build the immutable `QuranStore` from the configured sources (§4.1).
pub async fn load_quran_store(settings: &QuranSettings) -> Result<QuranStore, QuranLoadError> {
    // 1. Raw file bytes for the BLAKE3 contentVersion (§8.1).
    let uthmani_bytes = read_file(&settings.uthmani_path, "uthmani")?;
    let simple_clean_bytes = read_file(&settings.simple_clean_path, "simple-clean")?;
    let xml_bytes = read_file(&settings.metadata_xml_path, "metadata-xml")?;

    // 2. Read both corpora from SQLite, read-only + immutable (§4.1).
    let uthmani_rows = read_corpus(&settings.uthmani_path, "uthmani").await?;
    let simple_clean_rows = read_corpus(&settings.simple_clean_path, "simple-clean").await?;

    // 3. Build corpora + validate row count and index contiguity.
    validate_rows("uthmani", &uthmani_rows)?;
    validate_rows("simple-clean", &simple_clean_rows)?;
    let uthmani = Corpus::from_texts(&uthmani_rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>());
    let simple_clean =
        Corpus::from_texts(&simple_clean_rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>());

    // 4. Golden digests (§3.3). This is the load-bearing test: a per-row
    //    equality check against the same source cannot detect a normalizing
    //    *loader*, because both sides would be wrong identically.
    let dig_uthmani = corpus_digest(&uthmani);
    let dig_simple_clean = corpus_digest(&simple_clean);
    invariant(
        dig_uthmani == GOLDEN_UTHMANI,
        || {
            format!(
                "uthmani golden digest mismatch: expected {GOLDEN_UTHMANI}, computed {dig_uthmani} \
                 — a normalizing loader or wrong source is corrupting ayah text (§3.3)"
            )
        },
    )?;
    invariant(
        dig_simple_clean == GOLDEN_SIMPLE_CLEAN,
        || format!("simple-clean golden digest mismatch: expected {GOLDEN_SIMPLE_CLEAN}, computed {dig_simple_clean} (§3.3)"),
    )?;

    // 5. Parse the metadata XML + build/validate metadata. Cross-checks the
    //    SQLite rows against the XML surah table (§3.1) and derives bismillah
    //    from the verbatim Uthmani text (§3.3).
    let xml_str = std::str::from_utf8(&xml_bytes)
        .map_err(|e| inv(format!("metadata xml is not valid utf-8: {e}")))?;
    let doc = roxmltree::Document::parse(xml_str)?;
    let meta = build_meta(&doc, &uthmani_rows, &uthmani)?;

    // 6. contentVersion = blake3(uthmani_bytes ‖ simple_clean_bytes ‖ xml_bytes),
    //    leading 16 hex chars (§8.1). The concatenation order is part of the
    //    contract; the web client does not recompute it.
    let mut hasher = blake3::Hasher::new();
    hasher.update(&uthmani_bytes);
    hasher.update(&simple_clean_bytes);
    hasher.update(&xml_bytes);
    let content_version: Arc<str> = {
        let hex = hasher.finalize().to_hex();
        Arc::from(hex.as_str()[..16].to_string())
    };

    // 7. Optional pinned assertion (§8.1): QURAN_CONTENT_VERSION survives only
    //    as an assertion, never as the source of the value.
    if let Some(expected) = settings.expected_content_version.as_deref() {
        invariant(
            expected == &*content_version,
            || {
                format!(
                    "QURAN_CONTENT_VERSION assertion failed: env pinned {expected:?} but the \
                     computed content version is {content_version} (§8.1)"
                )
            },
        )?;
    }

    // File-level artifacts for `/scripts` (§5.1): sha256 + size of the published
    // SQLite file (reusing the raw bytes already read for the BLAKE3 digest).
    let artifacts = Artifacts {
        uthmani: ArtifactFile {
            id: Script::Uthmani,
            size_bytes: uthmani_bytes.len() as u64,
            sha256: file_sha256(&uthmani_bytes).into_boxed_str(),
        },
        simple_clean: ArtifactFile {
            id: Script::SimpleClean,
            size_bytes: simple_clean_bytes.len() as u64,
            sha256: file_sha256(&simple_clean_bytes).into_boxed_str(),
        },
    };

    // §7.1 search index: normalized simple-clean corpus. Built from the
    // in-memory corpus — no SQLite. Results are a function of simple-clean
    // only (§7.3); highlighting re-normalizes the requested script per hit.
    let search = super::search::SearchIndex::build(&uthmani, &simple_clean);

    Ok(QuranStore {
        uthmani,
        simple_clean,
        meta,
        content_version,
        source_digests: SourceDigests {
            uthmani: dig_uthmani.into_boxed_str(),
            simple_clean: dig_simple_clean.into_boxed_str(),
        },
        artifacts,
        search,
    })
}

/// `sha256` hex of a file's raw bytes (the §5.1 `Artifact.sha256`, not the §3.3
/// corpus-text digest).
fn file_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn read_file(path: &str, what: &'static str) -> Result<Vec<u8>, QuranLoadError> {
    std::fs::read(path).map_err(|source| QuranLoadError::File { what, source })
}

/// Open the SQLite artifact read-only + immutable (§4.1) and read every row in
/// global-index order. The connection is local to the loader and closed here.
async fn read_corpus(path: &str, what: &'static str) -> Result<Vec<CorpusRow>, QuranLoadError> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .immutable(true);
    let mut conn = SqliteConnection::connect_with(&opts)
        .await
        .map_err(|source| QuranLoadError::Sqlite { what, source })?;
    let rows = sqlx::query(r#"SELECT "index", sura, aya, text FROM quran_text ORDER BY "index""#)
        .fetch_all(&mut conn)
        .await
        .map_err(|source| QuranLoadError::Sqlite { what, source })?;
    conn.close()
        .await
        .map_err(|source| QuranLoadError::Sqlite { what, source })?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        // SQLite stores INTEGER as i64; cast down after range is validated below.
        let index = r.try_get::<i64, _>("index").map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let sura = r.try_get::<i64, _>("sura").map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let aya = r.try_get::<i64, _>("aya").map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let text = r.try_get::<String, _>("text").map_err(|source| QuranLoadError::Sqlite { what, source })?;
        out.push(CorpusRow {
            index: index as u32,
            sura: sura as u16,
            aya: aya as u16,
            text,
        });
    }
    Ok(out)
}

/// §3.1 + row-count + contiguity checks on the raw rows (before building the
/// arena, so a wrong source fails loud).
fn validate_rows(what: &'static str, rows: &[CorpusRow]) -> Result<(), QuranLoadError> {
    invariant(
        rows.len() == VERSE_COUNT as usize,
        || format!("{what}: expected {VERSE_COUNT} rows, got {}", rows.len()),
    )?;
    for (i, r) in rows.iter().enumerate() {
        let expected = i as u32 + 1;
        invariant(
            r.index == expected,
            || format!("{what}: row {i} has index {}, expected contiguous {expected}", r.index),
        )?;
    }
    Ok(())
}

/// `sha256` of all 6,236 texts joined by `\n` in global-index order (no trailing
/// newline) — the §3.3 golden digest representation.
fn corpus_digest(corpus: &Corpus) -> String {
    let joined = corpus.joined_for_digest();
    let mut h = Sha256::new();
    h.update(joined.as_bytes());
    hex::encode(h.finalize())
}

// ── metadata construction + validation ──────────────────────────────────────

struct Marker {
    index: u16,
    sura: u16,
    aya: u16,
}

fn find_child<'a, 'input>(parent: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    parent
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
}

fn attr(node: Node<'_, '_>, name: &str) -> Result<String, QuranLoadError> {
    node.attribute(name)
        .map(str::to_owned)
        .ok_or_else(|| inv(format!("missing attribute @{name}")))
}

fn attr_u16(node: Node<'_, '_>, name: &str) -> Result<u16, QuranLoadError> {
    attr(node, name)?
        .parse::<u16>()
        .map_err(|e| inv(format!("@{name}={} not a u16: {e}", attr(node, name).unwrap_or_default())))
}

fn attr_u32(node: Node<'_, '_>, name: &str) -> Result<u32, QuranLoadError> {
    attr(node, name)?
        .parse::<u32>()
        .map_err(|e| inv(format!("@{name} not a u32: {e}")))
}

fn elem_children<'a, 'input>(parent: Node<'a, 'input>, tag: &str) -> Vec<Node<'a, 'input>> {
    parent
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == tag)
        .collect()
}

/// `global_of(sura, aya)` against a surah slice (§3.1).
fn sura_global_of(suras: &[crate::quran::store::SuraMeta], sura: u16, aya: u16) -> Option<u32> {
    let s = suras.get((sura as usize).checked_sub(1)?)?;
    s.global_of(aya)
}

/// `locate(g)` against a surah slice (§3.1).
fn sura_locate(suras: &[crate::quran::store::SuraMeta], g: u32) -> Option<(u16, u16)> {
    let s = suras.iter().find(|s| g >= s.start_global && g <= s.end_global)?;
    let aya = (g - (s.start_global - 1)) as u16;
    Some((s.index, aya))
}

fn build_meta(
    doc: &roxmltree::Document<'_>,
    rows: &[CorpusRow],
    uthmani: &Corpus,
) -> Result<QuranMeta, QuranLoadError> {
    // `doc.root_element()` is the `<quran>` element; `doc.root()` is the
    // document node whose direct children are PIs/text, not the wrappers.
    let root = doc.root_element();

    // ── suras ──
    let suras_node =
        find_child(root, "suras").ok_or_else(|| inv("metadata xml missing <suras> wrapper"))?;
    let mut suras: Vec<crate::quran::store::SuraMeta> = Vec::with_capacity(SURA_COUNT);
    for n in elem_children(suras_node, "sura") {
        let index = attr_u16(n, "index")?;
        let ayas = attr_u16(n, "ayas")?;
        let start_zero = attr_u32(n, "start")?;
        let name = attr(n, "name")?;
        let tname = attr(n, "tname")?;
        let ename = attr(n, "ename")?;
        let place = crate::quran::store::Place::parse(&attr(n, "type")?)
            .ok_or_else(|| inv(format!("sura {index}: unknown place type")))?;
        let order = attr_u16(n, "order")?;
        let rukus = attr_u16(n, "rukus")?;
        invariant(
            (1..=SURA_COUNT as u16).contains(&index),
            || format!("sura index {index} out of 1..=114"),
        )?;
        let start_global = start_zero + 1;
        let end_global = start_zero + ayas as u32;
        suras.push(crate::quran::store::SuraMeta {
            index,
            ayas,
            start_global,
            end_global,
            revelation_order: order,
            ruku_count: rukus,
            place,
            name_arabic: name.into_boxed_str(),
            name_translit: tname.into_boxed_str(),
            name_english: ename.into_boxed_str(),
            bismillah: Bismillah::EmbeddedPrefix, // resolved below from verbatim text
        });
    }
    invariant(
        suras.len() == SURA_COUNT,
        || format!("expected {SURA_COUNT} <sura> elements, got {}", suras.len()),
    )?;
    suras.sort_by_key(|s| s.index);
    for (i, s) in suras.iter().enumerate() {
        invariant(s.index == i as u16 + 1, || format!("sura index not contiguous at {i}"))?;
    }

    // ── §3.1 global-index invariant: cross-check SQLite rows vs XML surahs ──
    let mut per_sura_count = vec![0u32; SURA_COUNT];
    for r in rows {
        let g = r.index;
        let (s, a) = sura_locate(&suras, g).ok_or_else(|| inv(format!("row global {g} not in any surah")))?;
        invariant(r.sura == s, || format!("row {g}: sura {} != expected {s} (§3.1)", r.sura))?;
        invariant(r.aya == a, || format!("row {g}: aya {} != expected {a} (§3.1)", r.aya))?;
        per_sura_count[(s - 1) as usize] += 1;
    }
    for (i, s) in suras.iter().enumerate() {
        invariant(
            per_sura_count[i] == s.ayas as u32,
            || format!("sura {}: ayas={} but {rows} rows map to it (§3.1)", s.index, s.ayas, rows = per_sura_count[i]),
        )?;
    }

    // ── bismillah (§3.3): surah 1's ayah 1 *is* the basmala (FirstAyah); surah 9
    //    (At-Tawba) is the sole surah with no basmala (None); all 112 others
    //    embed the basmala as a prefix of ayah 1 (EmbeddedPrefix). 95:1 / 97:1
    //    carry an extra U+0651 shadda on the first letter, so a naive
    //    `starts_with(basmala)` misfires on them — the distribution is a fixed
    //    property of the source and is pinned by the 1/112/1 assert below (and
    //    any underlying text change trips the golden digest).
    let basmala = uthmani
        .verse(1)
        .ok_or_else(|| inv("uthmani verse 1 (basmala) missing"))?;
    let mut first_ayah = 0usize;
    let mut none = 0usize;
    let mut embedded = 0usize;
    for s in suras.iter_mut() {
        let a1 = uthmani
            .verse(s.start_global)
            .ok_or_else(|| inv(format!("uthmani first ayah of surah {} missing", s.index)))?;
        s.bismillah = if a1 == basmala {
            first_ayah += 1;
            Bismillah::FirstAyah
        } else if s.index == 9 {
            none += 1;
            Bismillah::None
        } else {
            embedded += 1;
            Bismillah::EmbeddedPrefix
        };
    }
    invariant(
        first_ayah == 1 && none == 1 && embedded == 112,
        || {
            format!(
                "bismillah split expected 1/112/1 (FirstAyah/Embedded/None), got {first_ayah}/{embedded}/{none} (§3.3)"
            )
        },
    )?;

    // ── navigation ranges (§3.2): one builder for all five families ──
    let juzs = collect_markers(root, "juzs", "juz");
    let pages = collect_markers(root, "pages", "page");
    let rukus_m = collect_markers(root, "rukus", "ruku");
    // `<hizbs>` wraps 240 `<quarter>` markers (§3.2).
    let quarters = collect_markers(root, "hizbs", "quarter");
    let manzils = collect_markers(root, "manzils", "manzil");

    let juzs_r = build_ranges::<Juz>(&suras, juzs, "juz")?;
    let pages_r = build_ranges::<Page>(&suras, pages, "page")?;
    let rukus_r = build_ranges::<Ruku>(&suras, rukus_m, "ruku")?;
    let quarters_r = build_ranges::<HizbQuarter>(&suras, quarters, "hizb-quarter")?;
    let manzils_r = build_ranges::<Manzil>(&suras, manzils, "manzil")?;

    // §3.2/§4.1: pin the expected family counts at boot (fail fast). Downstream
    // relies on these bounds (e.g. store.navigation() casts manzil index to u8,
    // quarter_hizb divides by 4); a wrong-count source must not boot silently.
    invariant(juzs_r.len() == 30, || format!("expected 30 juzs, got {}", juzs_r.len()))?;
    invariant(pages_r.len() == 604, || format!("expected 604 pages, got {}", pages_r.len()))?;
    invariant(rukus_r.len() == 556, || format!("expected 556 rukus, got {}", rukus_r.len()))?;
    invariant(
        quarters_r.len() == 240,
        || format!("expected 240 hizb-quarters, got {}", quarters_r.len()),
    )?;
    invariant(manzils_r.len() == 7, || format!("expected 7 manzils, got {}", manzils_r.len()))?;

    // ── sajdas (§3.2, 15 markers) ──
    let sajdas_node =
        find_child(root, "sajdas").ok_or_else(|| inv("metadata xml missing <sajdas> wrapper"))?;
    let mut sajdas: Vec<Sajda> = Vec::with_capacity(15);
    for n in elem_children(sajdas_node, "sajda") {
        let index = attr_u16(n, "index")?;
        let sura = attr_u16(n, "sura")?;
        let aya = attr_u16(n, "aya")?;
        let kind = SajdaKind::parse(&attr(n, "type")?)
            .ok_or_else(|| inv(format!("sajda {index}: unknown type")))?;
        let global_index = sura_global_of(&suras, sura, aya)
            .ok_or_else(|| inv(format!("sajda {index}: ({sura},{aya}) out of range")))?;
        sajdas.push(Sajda {
            index,
            sura,
            aya,
            global_index,
            kind,
        });
    }
    invariant(
        sajdas.len() == 15,
        || format!("expected 15 sajdas, got {}", sajdas.len()),
    )?;
    sajdas.sort_by_key(|s| s.index);
    for (i, s) in sajdas.iter().enumerate() {
        invariant(s.index == i as u16 + 1, || format!("sajda index not contiguous at {i}"))?;
    }

    let suras_arr: [crate::quran::store::SuraMeta; SURA_COUNT] = suras
        .try_into()
        .map_err(|v: Vec<_>| inv(format!("sura vec was not length {SURA_COUNT} (got {})", v.len())))?;

    Ok(QuranMeta {
        suras: suras_arr,
        juzs: juzs_r,
        pages: pages_r,
        rukus: rukus_r,
        hizb_quarters: quarters_r,
        manzils: manzils_r,
        sajdas: sajdas.into_boxed_slice(),
    })
}

fn collect_markers(root: Node<'_, '_>, wrapper: &str, elem: &str) -> Vec<Marker> {
    let Some(w) = find_child(root, wrapper) else {
        return Vec::new();
    };
    elem_children(w, elem)
        .into_iter()
        .filter_map(|n| {
            let index = attr_u16(n, "index").ok()?;
            let sura = attr_u16(n, "sura").ok()?;
            let aya = attr_u16(n, "aya").ok()?;
            Some(Marker { index, sura, aya })
        })
        .collect()
}

/// Tile one start-marker family over `[1, VERSE_COUNT]` (§3.2). Validates:
/// marker `index` is contiguous 1..N; marker index order matches global-index
/// order; the family tiles `[1, 6236]` with no gaps or overlaps; and the last
/// range ends at `VERSE_COUNT`.
fn build_ranges<K>(
    suras: &[crate::quran::store::SuraMeta],
    mut markers: Vec<Marker>,
    what: &'static str,
) -> Result<Box<[Range<K>]>, QuranLoadError> {
    let n = markers.len();
    invariant(n > 0, || format!("{what}: no markers"))?;
    markers.sort_by_key(|m| m.index);
    for (i, m) in markers.iter().enumerate() {
        invariant(
            m.index == i as u16 + 1,
            || format!("{what}: marker index not contiguous at {i} (got {})", m.index),
        )?;
    }
    // Resolve each marker to its global start.
    let starts: Vec<u32> = markers
        .iter()
        .map(|m| {
            sura_global_of(suras, m.sura, m.aya)
                .ok_or_else(|| inv(format!("{what}[{}]: marker ({},{}) out of range", m.index, m.sura, m.aya)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    // marker index order must match global-index order (§3.2).
    for w in starts.windows(2) {
        invariant(w[0] < w[1], || format!("{what}: marker index order != global-index order"))?;
    }

    let mut out: Vec<Range<K>> = Vec::with_capacity(n);
    for (i, &start_global) in starts.iter().enumerate() {
        let end_global = if i + 1 < n {
            starts[i + 1] - 1
        } else {
            VERSE_COUNT
        };
        let (start_sura, start_aya) = (markers[i].sura, markers[i].aya);
        let (end_sura, end_aya) = sura_locate(suras, end_global)
            .ok_or_else(|| inv(format!("{what}[{}]: end_global {end_global} not locatable", markers[i].index)))?;
        out.push(Range {
            index: markers[i].index,
            start_global,
            end_global,
            start_sura,
            start_aya,
            end_sura,
            end_aya,
            _family: PhantomData,
        });
    }

    invariant(
        out.first().map(|r| r.start_global == 1).unwrap_or(false),
        || format!("{what}: first range must start at global 1"),
    )?;
    invariant(
        out.last().map(|r| r.end_global == VERSE_COUNT).unwrap_or(false),
        || format!("{what}: last range must end at global {VERSE_COUNT}"),
    )?;
    for w in out.windows(2) {
        invariant(
            w[0].end_global + 1 == w[1].start_global,
            || format!("{what}: tiling gap/overlap between ranges {} and {}", w[0].index, w[1].index),
        )?;
    }
    Ok(out.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::{Bismillah, SajdaKind, Script};

    /// Paths resolve against the committed artifacts at the repo root
    /// (`CARGO_MANIFEST_DIR` = `…/rust/backend/api`; repo root is 3 levels up).
    fn settings() -> QuranSettings {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");
        QuranSettings {
            uthmani_path: format!("{base}/arabic/quran-uthmani.sqlite"),
            simple_clean_path: format!("{base}/arabic/quran-simple-clean.sqlite"),
            metadata_xml_path: format!("{base}/quran-data.xml"),
            expected_content_version: None,
        }
    }

    fn tiles<K>(r: &[Range<K>]) -> bool {
        r.first().map(|x| x.start_global == 1).unwrap_or(false)
            && r.last().map(|x| x.end_global == VERSE_COUNT).unwrap_or(false)
            && r.windows(2).all(|w| w[0].end_global + 1 == w[1].start_global)
    }

    #[tokio::test]
    async fn loads_and_validates_all_invariants() {
        let store = load_quran_store(&settings()).await.expect("store must load");

        // §3.3 golden digests (also asserted inside the loader — this double-checks
        // the stored values are the constants).
        assert_eq!(&*store.source_digests.uthmani, GOLDEN_UTHMANI);
        assert_eq!(&*store.source_digests.simple_clean, GOLDEN_SIMPLE_CLEAN);

        // §3.2 metadata counts.
        assert_eq!(store.meta.suras().len(), 114);
        assert_eq!(store.meta.juzs.len(), 30);
        assert_eq!(store.meta.pages.len(), 604);
        assert_eq!(store.meta.rukus.len(), 556);
        assert_eq!(store.meta.hizb_quarters.len(), 240);
        assert_eq!(store.meta.manzils.len(), 7);
        assert_eq!(store.meta.sajdas.len(), 15);

        // Every range family tiles [1, 6236] with no gaps or overlaps.
        assert!(tiles(&store.meta.juzs), "juzs must tile");
        assert!(tiles(&store.meta.pages), "pages must tile");
        assert!(tiles(&store.meta.rukus), "rukus must tile");
        assert!(tiles(&store.meta.hizb_quarters), "hizb-quarters must tile");
        assert!(tiles(&store.meta.manzils), "manzils must tile");

        // §8.1 contentVersion = 16 hex chars, BLAKE3-derived.
        assert_eq!(store.content_version().len(), 16);
        assert!(store
            .content_version()
            .chars()
            .all(|c| c.is_ascii_hexdigit()));

        // §3.1 surah spans: surah 1 → 1..7, surah 2 → 8..293, surah 114 ends at 6236.
        assert_eq!(store.meta.sura(1).unwrap().start_global, 1);
        assert_eq!(store.meta.sura(1).unwrap().end_global, 7);
        assert_eq!(store.meta.sura(2).unwrap().start_global, 8);
        assert_eq!(store.meta.sura(2).unwrap().end_global, 293);
        assert_eq!(store.meta.sura(114).unwrap().end_global, VERSE_COUNT);

        // global_of / locate round-trip across the whole corpus.
        for g in 1..=VERSE_COUNT {
            let (s, a) = store.meta().locate(g).expect("locate");
            assert_eq!(store.meta().global_of(s, a), Some(g), "round-trip at g={g}");
        }
        // Spot-check known anchors (§3.1).
        assert_eq!(store.meta().global_of(1, 1), Some(1));
        assert_eq!(store.meta().global_of(2, 1), Some(8));
        assert_eq!(store.meta().global_of(7, 206), Some(1160));
        assert_eq!(store.meta().global_of(114, 6), Some(VERSE_COUNT));

        // §3.2 hizb / quarterInHizb derivation covers 1..60 and 1..4.
        let hizbs: std::collections::HashSet<u16> = store
            .meta
            .hizb_quarters
            .iter()
            .map(|q| ((q.index - 1) / 4) + 1)
            .collect();
        assert_eq!(hizbs.len(), 60, "hizb 1..=60");
        let qih: std::collections::HashSet<u16> = store
            .meta
            .hizb_quarters
            .iter()
            .map(|q| ((q.index - 1) % 4) + 1)
            .collect();
        assert_eq!(qih.len(), 4, "quarterInHizb 1..=4");

        // §3.3 bismillah split: exactly 1 FirstAyah, 1 None, 112 EmbeddedPrefix.
        let (mut fa, mut none, mut emb) = (0, 0, 0);
        for s in store.meta.suras() {
            match s.bismillah {
                Bismillah::FirstAyah => fa += 1,
                Bismillah::None => none += 1,
                Bismillah::EmbeddedPrefix => emb += 1,
            }
        }
        assert_eq!((fa, emb, none), (1, 112, 1));
        assert_eq!(store.meta.sura(1).unwrap().bismillah, Bismillah::FirstAyah);
        assert_eq!(store.meta.sura(9).unwrap().bismillah, Bismillah::None);

        // Sajda at a known obligatory ayah (sura 32 aya 15 → global 3518).
        assert_eq!(store.meta().global_of(32, 15), Some(3518));
        assert_eq!(store.sajda_at(3518), Some(SajdaKind::Obligatory));

        // navigation() for g=1 is all-first.
        let nav = store.navigation(1).unwrap();
        assert_eq!((nav.juz, nav.page, nav.ruku, nav.hizb_quarter, nav.manzil), (1, 1, 1, 1, 1));

        // Verbatim text: 1:1 is the basmala in both scripts (§3.3).
        let u = store.verse(Script::Uthmani, 1).unwrap();
        let sc = store.verse(Script::SimpleClean, 1).unwrap();
        assert!(u.contains('ٱ') || u.contains('ا')); // sanity: non-empty Arabic
        assert!(!sc.is_empty());

        // ayah_view builds for every ayah without panic.
        for g in 1..=VERSE_COUNT {
            assert!(store.ayah_view(Script::Uthmani, g).is_some(), "ayah_view g={g}");
        }
    }

    /// §10 Verbatim responses: 1:1, 9:1, 27:30, 95:1, 97:1 remain unchanged.
    #[tokio::test]
    async fn verbatim_named_anchors() {
        let store = load_quran_store(&settings()).await.expect("store loads");
        let basmala_u = store.verse(Script::Uthmani, 1).unwrap();
        let basmala_sc = store.verse(Script::SimpleClean, 1).unwrap();

        // 27:30 contains the basmala mid-ayah, byte-identical to 1:1, and in
        // simple-clean is the only non-first ayah that does (§3.3).
        let g2730 = store.meta().global_of(27, 30).unwrap();
        let v2730_sc = store.verse(Script::SimpleClean, g2730).unwrap();
        assert!(
            v2730_sc.contains(basmala_sc),
            "27:30 simple-clean must contain the 1:1 basmala verbatim"
        );

        // 9:1 has no basmala (surah 9 omits it).
        let g91 = store.meta().global_of(9, 1).unwrap();
        let v91_u = store.verse(Script::Uthmani, g91).unwrap();
        assert!(!v91_u.starts_with(basmala_u), "9:1 must not carry the basmala");

        // 95:1 / 97:1 carry an extra U+0651 shadda in Uthmani (§3.3) — assert a
        // shadda is present (diacritics survive verbatim; simple-clean has none).
        for sura in [95u16, 97u16] {
            let g = store.meta().global_of(sura, 1).unwrap();
            let t = store.verse(Script::Uthmani, g).unwrap();
            assert!(t.contains('\u{0651}'), "uthmani {sura}:1 should carry a shadda");
            let t_sc = store.verse(Script::SimpleClean, g).unwrap();
            assert!(!t_sc.contains('\u{0651}'), "simple-clean {sura}:1 has no harakat");
        }
    }

    /// §10 No-SQLite-after-startup: after the store is built, revoke all file
    /// access (chmod 000 the sources) and assert the store still serves
    /// byte-identical text — proving reads are in-memory, not SQLite. Runs on
    /// temp COPIES so it cannot disrupt parallel tests that read the originals.
    #[cfg(unix)]
    #[tokio::test]
    async fn no_sqlite_access_after_startup() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = std::env::temp_dir().join(format!("quran-chmod-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../db/quran/tanzil");
        for (src, name) in [
            ("arabic/quran-uthmani.sqlite", "u.sqlite"),
            ("arabic/quran-simple-clean.sqlite", "s.sqlite"),
            ("quran-data.xml", "m.xml"),
        ] {
            std::fs::copy(format!("{base}/{src}"), tmp.join(name)).unwrap();
        }
        let temp_settings = QuranSettings {
            uthmani_path: tmp.join("u.sqlite").to_string_lossy().into_owned(),
            simple_clean_path: tmp.join("s.sqlite").to_string_lossy().into_owned(),
            metadata_xml_path: tmp.join("m.xml").to_string_lossy().into_owned(),
            expected_content_version: None,
        };
        let store = load_quran_store(&temp_settings)
            .await
            .expect("loads from temp copies");

        // Baseline before revoking access.
        let u_before = store.verse(Script::Uthmani, 1).unwrap().to_string();
        let sc_before = store.verse(Script::SimpleClean, 1160).unwrap().to_string();

        // chmod 000 every source — any file/SQLite access would now fail.
        for name in ["u.sqlite", "s.sqlite", "m.xml"] {
            std::fs::set_permissions(tmp.join(name), std::fs::Permissions::from_mode(0o000))
                .unwrap();
        }

        // The store still serves byte-identical text purely from memory (§10).
        assert_eq!(store.verse(Script::Uthmani, 1).unwrap(), u_before);
        assert_eq!(store.verse(Script::SimpleClean, 1160).unwrap(), sc_before);

        // Restore + cleanup so a rerun works.
        for name in ["u.sqlite", "s.sqlite", "m.xml"] {
            let _ = std::fs::set_permissions(tmp.join(name), std::fs::Permissions::from_mode(0o644));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
