use std::marker::PhantomData;

use roxmltree::Node;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::Connection;
use sqlx::Row;

use crate::config::QuranSettings;

use super::store::{
    ArtifactFile, Artifacts, Bismillah, CatalogueEntry, Corpus, HizbQuarter, Juz, Manzil, Page,
    QuranMeta, QuranStore, Range, Ruku, Sajda, SajdaKind, Script, SURA_COUNT,
    VERSE_COUNT,
};

struct CorpusRow {
    index: u32,
    sura: u16,
    aya: u16,
    text: String,
}

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

pub async fn load_quran_store(settings: &QuranSettings) -> Result<QuranStore, QuranLoadError> {
    let uthmani_bytes = read_file(&settings.uthmani_path, "uthmani")?;
    let simple_clean_bytes = read_file(&settings.simple_clean_path, "simple-clean")?;
    let xml_bytes = read_file(&settings.metadata_xml_path, "metadata-xml")?;

    let uthmani_rows = read_corpus(&settings.uthmani_path, "uthmani").await?;
    let simple_clean_rows = read_corpus(&settings.simple_clean_path, "simple-clean").await?;

    validate_rows("uthmani", &uthmani_rows)?;
    validate_rows("simple-clean", &simple_clean_rows)?;
    let uthmani = Corpus::from_texts(
        &uthmani_rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
    );
    let simple_clean = Corpus::from_texts(
        &simple_clean_rows
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
    );

    let xml_str = std::str::from_utf8(&xml_bytes)
        .map_err(|e| inv(format!("metadata xml is not valid utf-8: {e}")))?;
    let doc = roxmltree::Document::parse(xml_str)?;
    let meta = build_meta(&doc, &uthmani_rows, &uthmani)?;

    let artifacts = Artifacts {
        uthmani: ArtifactFile {
            id: Script::Uthmani,
            size_bytes: uthmani_bytes.len() as u64,
        },
        simple_clean: ArtifactFile {
            id: Script::SimpleClean,
            size_bytes: simple_clean_bytes.len() as u64,
        },
    };

    let search = super::search::SearchIndex::build(&uthmani, &simple_clean);

    Ok(QuranStore {
        uthmani,
        simple_clean,
        meta,
        artifacts,
        search,
    })
}

/// Boot-load the translation catalogue (the §2-widened `index.min.json`). Fail-fast on a
/// missing or malformed file per Part 1: a bad catalogue aborts boot rather than serving a
/// half-state. No sqlite I/O here — files stay on-demand; this reads only the small JSON.
pub async fn load_catalogue(path: &str) -> Result<Box<[CatalogueEntry]>, QuranLoadError> {
    let bytes = read_file(path, "translations-catalogue")?;
    let raw: Vec<RawCatalogueEntry> = serde_json::from_slice(&bytes)
        .map_err(|e| inv(format!("translations catalogue parse ({path}): {e}")))?;
    invariant(!raw.is_empty(), || {
        format!("translations catalogue ({path}) is empty")
    })?;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(raw.len());
    for r in raw {
        invariant(seen.insert(r.id.clone()), || {
            format!("duplicate translation id {}", r.id)
        })?;
        let p = std::path::Path::new(&r.file.path);
        invariant(p.is_relative(), || {
            format!("translation {} file.path must be relative", r.id)
        })?;
        invariant(
            !p.components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
            || format!("translation {} file.path must not contain '..'", r.id),
        )?;
        out.push(CatalogueEntry {
            id: r.id.into_boxed_str(),
            language: r.language.into_boxed_str(),
            language_code: r.language_code.into_boxed_str(),
            direction: r.direction.into_boxed_str(),
            name: r.name.into_boxed_str(),
            translator: r.translator.map(|s| s.into_boxed_str()),
            path: r.file.path.into_boxed_str(),
            size_bytes: r.file.size_bytes,
        });
    }
    Ok(out.into_boxed_slice())
}

#[derive(serde::Deserialize)]
struct RawCatalogueEntry {
    id: String,
    language: String,
    #[serde(rename = "languageCode")]
    language_code: String,
    direction: String,
    name: String,
    #[serde(default)]
    translator: Option<String>,
    file: RawCatalogueFile,
}

#[derive(serde::Deserialize)]
struct RawCatalogueFile {
    path: String,
    #[serde(rename = "sizeBytes")]
    size_bytes: u64,
}

pub async fn load_translation_corpus(path: &str) -> Result<Corpus, QuranLoadError> {
    let rows = read_corpus(path, "translation").await?;
    validate_rows("translation", &rows)?;
    Ok(Corpus::from_texts(
        &rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
    ))
}

fn read_file(path: &str, what: &'static str) -> Result<Vec<u8>, QuranLoadError> {
    std::fs::read(path).map_err(|source| QuranLoadError::File { what, source })
}

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
        let index = r
            .try_get::<i64, _>("index")
            .map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let sura = r
            .try_get::<i64, _>("sura")
            .map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let aya = r
            .try_get::<i64, _>("aya")
            .map_err(|source| QuranLoadError::Sqlite { what, source })?;
        let text = r
            .try_get::<String, _>("text")
            .map_err(|source| QuranLoadError::Sqlite { what, source })?;
        out.push(CorpusRow {
            index: index as u32,
            sura: sura as u16,
            aya: aya as u16,
            text,
        });
    }
    Ok(out)
}

fn validate_rows(what: &'static str, rows: &[CorpusRow]) -> Result<(), QuranLoadError> {
    invariant(rows.len() == VERSE_COUNT as usize, || {
        format!("{what}: expected {VERSE_COUNT} rows, got {}", rows.len())
    })?;
    for (i, r) in rows.iter().enumerate() {
        let expected = i as u32 + 1;
        invariant(r.index == expected, || {
            format!(
                "{what}: row {i} has index {}, expected contiguous {expected}",
                r.index
            )
        })?;
    }
    Ok(())
}

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
    attr(node, name)?.parse::<u16>().map_err(|e| {
        inv(format!(
            "@{name}={} not a u16: {e}",
            attr(node, name).unwrap_or_default()
        ))
    })
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

fn sura_global_of(suras: &[crate::quran::store::SuraMeta], sura: u16, aya: u16) -> Option<u32> {
    let s = suras.get((sura as usize).checked_sub(1)?)?;
    s.global_of(aya)
}

fn sura_locate(suras: &[crate::quran::store::SuraMeta], g: u32) -> Option<(u16, u16)> {
    let s = suras
        .iter()
        .find(|s| g >= s.start_global && g <= s.end_global)?;
    let aya = (g - (s.start_global - 1)) as u16;
    Some((s.index, aya))
}

fn build_meta(
    doc: &roxmltree::Document<'_>,
    rows: &[CorpusRow],
    uthmani: &Corpus,
) -> Result<QuranMeta, QuranLoadError> {
    let root = doc.root_element();

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
        invariant((1..=SURA_COUNT as u16).contains(&index), || {
            format!("sura index {index} out of 1..=114")
        })?;
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
            bismillah: Bismillah::EmbeddedPrefix,
        });
    }
    invariant(suras.len() == SURA_COUNT, || {
        format!("expected {SURA_COUNT} <sura> elements, got {}", suras.len())
    })?;
    suras.sort_by_key(|s| s.index);
    for (i, s) in suras.iter().enumerate() {
        invariant(s.index == i as u16 + 1, || {
            format!("sura index not contiguous at {i}")
        })?;
    }

    let mut per_sura_count = vec![0u32; SURA_COUNT];
    for r in rows {
        let g = r.index;
        let (s, a) = sura_locate(&suras, g)
            .ok_or_else(|| inv(format!("row global {g} not in any surah")))?;
        invariant(r.sura == s, || {
            format!("row {g}: sura {} != expected {s} (§3.1)", r.sura)
        })?;
        invariant(r.aya == a, || {
            format!("row {g}: aya {} != expected {a} (§3.1)", r.aya)
        })?;
        per_sura_count[(s - 1) as usize] += 1;
    }
    for (i, s) in suras.iter().enumerate() {
        invariant(per_sura_count[i] == s.ayas as u32, || {
            format!(
                "sura {}: ayas={} but {rows} rows map to it (§3.1)",
                s.index,
                s.ayas,
                rows = per_sura_count[i]
            )
        })?;
    }

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
    invariant(first_ayah == 1 && none == 1 && embedded == 112, || {
        format!(
                "bismillah split expected 1/112/1 (FirstAyah/Embedded/None), got {first_ayah}/{embedded}/{none} (§3.3)"
            )
    })?;

    let juzs = collect_markers(root, "juzs", "juz");
    let pages = collect_markers(root, "pages", "page");
    let rukus_m = collect_markers(root, "rukus", "ruku");
    let quarters = collect_markers(root, "hizbs", "quarter");
    let manzils = collect_markers(root, "manzils", "manzil");

    let juzs_r = build_ranges::<Juz>(&suras, juzs, "juz")?;
    let pages_r = build_ranges::<Page>(&suras, pages, "page")?;
    let rukus_r = build_ranges::<Ruku>(&suras, rukus_m, "ruku")?;
    let quarters_r = build_ranges::<HizbQuarter>(&suras, quarters, "hizb-quarter")?;
    let manzils_r = build_ranges::<Manzil>(&suras, manzils, "manzil")?;

    invariant(juzs_r.len() == 30, || {
        format!("expected 30 juzs, got {}", juzs_r.len())
    })?;
    invariant(pages_r.len() == 604, || {
        format!("expected 604 pages, got {}", pages_r.len())
    })?;
    invariant(rukus_r.len() == 556, || {
        format!("expected 556 rukus, got {}", rukus_r.len())
    })?;
    invariant(quarters_r.len() == 240, || {
        format!("expected 240 hizb-quarters, got {}", quarters_r.len())
    })?;
    invariant(manzils_r.len() == 7, || {
        format!("expected 7 manzils, got {}", manzils_r.len())
    })?;

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
    invariant(sajdas.len() == 15, || {
        format!("expected 15 sajdas, got {}", sajdas.len())
    })?;
    sajdas.sort_by_key(|s| s.index);
    for (i, s) in sajdas.iter().enumerate() {
        invariant(s.index == i as u16 + 1, || {
            format!("sajda index not contiguous at {i}")
        })?;
    }

    let suras_arr: [crate::quran::store::SuraMeta; SURA_COUNT] =
        suras.try_into().map_err(|v: Vec<_>| {
            inv(format!(
                "sura vec was not length {SURA_COUNT} (got {})",
                v.len()
            ))
        })?;

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

fn build_ranges<K>(
    suras: &[crate::quran::store::SuraMeta],
    mut markers: Vec<Marker>,
    what: &'static str,
) -> Result<Box<[Range<K>]>, QuranLoadError> {
    let n = markers.len();
    invariant(n > 0, || format!("{what}: no markers"))?;
    markers.sort_by_key(|m| m.index);
    for (i, m) in markers.iter().enumerate() {
        invariant(m.index == i as u16 + 1, || {
            format!(
                "{what}: marker index not contiguous at {i} (got {})",
                m.index
            )
        })?;
    }
    let starts: Vec<u32> = markers
        .iter()
        .map(|m| {
            sura_global_of(suras, m.sura, m.aya).ok_or_else(|| {
                inv(format!(
                    "{what}[{}]: marker ({},{}) out of range",
                    m.index, m.sura, m.aya
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for w in starts.windows(2) {
        invariant(w[0] < w[1], || {
            format!("{what}: marker index order != global-index order")
        })?;
    }

    let mut out: Vec<Range<K>> = Vec::with_capacity(n);
    for (i, &start_global) in starts.iter().enumerate() {
        let end_global = if i + 1 < n {
            starts[i + 1] - 1
        } else {
            VERSE_COUNT
        };
        let (start_sura, start_aya) = (markers[i].sura, markers[i].aya);
        let (end_sura, end_aya) = sura_locate(suras, end_global).ok_or_else(|| {
            inv(format!(
                "{what}[{}]: end_global {end_global} not locatable",
                markers[i].index
            ))
        })?;
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
        out.last()
            .map(|r| r.end_global == VERSE_COUNT)
            .unwrap_or(false),
        || format!("{what}: last range must end at global {VERSE_COUNT}"),
    )?;
    for w in out.windows(2) {
        invariant(w[0].end_global + 1 == w[1].start_global, || {
            format!(
                "{what}: tiling gap/overlap between ranges {} and {}",
                w[0].index, w[1].index
            )
        })?;
    }
    Ok(out.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quran::{Bismillah, SajdaKind, Script};

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

    fn tiles<K>(r: &[Range<K>]) -> bool {
        r.first().map(|x| x.start_global == 1).unwrap_or(false)
            && r.last()
                .map(|x| x.end_global == VERSE_COUNT)
                .unwrap_or(false)
            && r.windows(2)
                .all(|w| w[0].end_global + 1 == w[1].start_global)
    }

    #[tokio::test]
    async fn loads_and_validates_all_invariants() {
        let store = load_quran_store(&settings())
            .await
            .expect("store must load");

        assert_eq!(store.meta.suras().len(), 114);
        assert_eq!(store.meta.juzs.len(), 30);
        assert_eq!(store.meta.pages.len(), 604);
        assert_eq!(store.meta.rukus.len(), 556);
        assert_eq!(store.meta.hizb_quarters.len(), 240);
        assert_eq!(store.meta.manzils.len(), 7);
        assert_eq!(store.meta.sajdas.len(), 15);

        assert!(tiles(&store.meta.juzs), "juzs must tile");
        assert!(tiles(&store.meta.pages), "pages must tile");
        assert!(tiles(&store.meta.rukus), "rukus must tile");
        assert!(tiles(&store.meta.hizb_quarters), "hizb-quarters must tile");
        assert!(tiles(&store.meta.manzils), "manzils must tile");

        assert_eq!(store.meta.sura(1).unwrap().start_global, 1);
        assert_eq!(store.meta.sura(1).unwrap().end_global, 7);
        assert_eq!(store.meta.sura(2).unwrap().start_global, 8);
        assert_eq!(store.meta.sura(2).unwrap().end_global, 293);
        assert_eq!(store.meta.sura(114).unwrap().end_global, VERSE_COUNT);

        for g in 1..=VERSE_COUNT {
            let (s, a) = store.meta().locate(g).expect("locate");
            assert_eq!(store.meta().global_of(s, a), Some(g), "round-trip at g={g}");
        }
        assert_eq!(store.meta().global_of(1, 1), Some(1));
        assert_eq!(store.meta().global_of(2, 1), Some(8));
        assert_eq!(store.meta().global_of(7, 206), Some(1160));
        assert_eq!(store.meta().global_of(114, 6), Some(VERSE_COUNT));

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

        assert_eq!(store.meta().global_of(32, 15), Some(3518));
        assert_eq!(store.sajda_at(3518), Some(SajdaKind::Obligatory));

        let nav = store.navigation(1).unwrap();
        assert_eq!(
            (nav.juz, nav.page, nav.ruku, nav.hizb_quarter, nav.manzil),
            (1, 1, 1, 1, 1)
        );

        let u = store.verse(Script::Uthmani, 1).unwrap();
        let sc = store.verse(Script::SimpleClean, 1).unwrap();
        assert!(u.contains('ٱ') || u.contains('ا'));
        assert!(!sc.is_empty());

        for g in 1..=VERSE_COUNT {
            assert!(
                store.ayah_view(Script::Uthmani, g).is_some(),
                "ayah_view g={g}"
            );
        }
    }

    #[tokio::test]
    async fn verbatim_named_anchors() {
        let store = load_quran_store(&settings()).await.expect("store loads");
        let basmala_u = store.verse(Script::Uthmani, 1).unwrap();
        let basmala_sc = store.verse(Script::SimpleClean, 1).unwrap();

        let g2730 = store.meta().global_of(27, 30).unwrap();
        let v2730_sc = store.verse(Script::SimpleClean, g2730).unwrap();
        assert!(
            v2730_sc.contains(basmala_sc),
            "27:30 simple-clean must contain the 1:1 basmala verbatim"
        );

        let g91 = store.meta().global_of(9, 1).unwrap();
        let v91_u = store.verse(Script::Uthmani, g91).unwrap();
        assert!(
            !v91_u.starts_with(basmala_u),
            "9:1 must not carry the basmala"
        );

        for sura in [95u16, 97u16] {
            let g = store.meta().global_of(sura, 1).unwrap();
            let t = store.verse(Script::Uthmani, g).unwrap();
            assert!(
                t.contains('\u{0651}'),
                "uthmani {sura}:1 should carry a shadda"
            );
            let t_sc = store.verse(Script::SimpleClean, g).unwrap();
            assert!(
                !t_sc.contains('\u{0651}'),
                "simple-clean {sura}:1 has no harakat"
            );
        }
    }

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
            translations_dir: tmp.to_string_lossy().into_owned(),
            max_resident_translations: 8,
            max_resident_bytes: 48 * 1024 * 1024,
            translation_idle_ttl_secs: 1800,
        };
        let store = load_quran_store(&temp_settings)
            .await
            .expect("loads from temp copies");

        let u_before = store.verse(Script::Uthmani, 1).unwrap().to_string();
        let sc_before = store.verse(Script::SimpleClean, 1160).unwrap().to_string();

        for name in ["u.sqlite", "s.sqlite", "m.xml"] {
            std::fs::set_permissions(tmp.join(name), std::fs::Permissions::from_mode(0o000))
                .unwrap();
        }

        assert_eq!(store.verse(Script::Uthmani, 1).unwrap(), u_before);
        assert_eq!(store.verse(Script::SimpleClean, 1160).unwrap(), sc_before);

        for name in ["u.sqlite", "s.sqlite", "m.xml"] {
            let _ =
                std::fs::set_permissions(tmp.join(name), std::fs::Permissions::from_mode(0o644));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
