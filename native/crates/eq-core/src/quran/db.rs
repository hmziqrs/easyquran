//! Read-only SQLite access to the immutable Quran artifacts (PARITY B1, B2;
//! mirror of `web/src/lib/quran/sql.ts`, `view/source-runtime.ts` and the
//! worker's staging check). Every file is opened `SQLITE_OPEN_READ_ONLY`
//! with `immutable=1` and `PRAGMA query_only`; the queries are the web
//! adapters' own SQL; text comes back exactly as stored (no trim, no
//! normalisation). Blocking: callers run these off the UI thread.

use std::{collections::HashMap, path::Path};

use rusqlite::{Connection, OpenFlags, types::ValueRef};

use super::{
    catalogue::{ArabicProfile, QURAN_ROW_COUNT, arabic_profile},
    meta::QuranMeta,
    types::{ArabicSource, Ayah, QuranScript, RangeText, SurahNormalization, SurahText},
    view::SourceView,
};

/// The web adapters' SQL (`TANZIL_QURAN_DATABASE`, `VARIANT_QURAN_DATABASE`).
pub mod sql {
    pub const COUNT: &str = "SELECT count(*) AS count FROM quran_text";
    pub const COORDINATES: &str = r#"SELECT "index" AS globalIndex, sura AS surah, aya AS ayah FROM quran_text ORDER BY "index""#;
    pub const FIRST_AYAHS: &str =
        "SELECT sura AS surah, text FROM quran_text WHERE aya = 1 ORDER BY sura";
    pub const SURAH: &str = "SELECT text FROM quran_text WHERE sura = ? ORDER BY aya";
    pub const RANGE: &str = r#"SELECT "index" AS globalIndex, sura AS surah, aya AS ayah, text FROM quran_text WHERE "index" BETWEEN ? AND ? ORDER BY "index""#;
    pub const ALL: &str = r#"SELECT "index" AS globalIndex, sura AS surah, aya AS ayah, text FROM quran_text ORDER BY "index""#;
    pub const VARIANT_OPENERS: &str =
        "SELECT sura AS surah, (SELECT text FROM quran_text WHERE sura = 1 AND aya = 1) AS text
       FROM quran_text WHERE aya = 1 AND sura > 1 AND sura <> 9 ORDER BY sura";
}

/// A database could not be opened or read, or failed a canonical check.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(String),
    #[error("[quran-sql] {0}")]
    Decode(String),
    #[error("[quran-source:{source_id}] {message}")]
    Invalid { source_id: String, message: String },
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sqlite(e.to_string())
    }
}

/// `file:` URI of a path, percent-encoding what a URI would misread.
fn file_uri(path: &Path) -> String {
    let mut out = String::from("file:");
    // Windows: `C:\x` becomes `/C:/x`, the form SQLite's URI parser takes.
    let mut p = path.to_string_lossy().replace('\\', "/");
    if !p.starts_with('/') {
        p.insert(0, '/');
    }
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out.push_str("?immutable=1");
    out
}

fn int(v: ValueRef<'_>, label: &str) -> Result<i64, DbError> {
    match v {
        ValueRef::Integer(i) => Ok(i),
        _ => Err(DbError::Decode(format!("{label} must be a safe integer"))),
    }
}

fn narrow<T: TryFrom<i64>>(v: i64, label: &str) -> Result<T, DbError> {
    T::try_from(v).map_err(|_| DbError::Decode(format!("{label} out of range")))
}

/// The cell as stored text, verbatim (`decodeTextField`).
fn text(v: ValueRef<'_>) -> Result<String, DbError> {
    match v {
        ValueRef::Text(bytes) => std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| DbError::Decode("text must be a string".into())),
        _ => Err(DbError::Decode("text must be a string".into())),
    }
}

/// One open, read-only `quran_text` database.
pub struct QuranDb {
    conn: Connection,
}

impl std::fmt::Debug for QuranDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuranDb").finish_non_exhaustive()
    }
}

impl QuranDb {
    /// Opens `path` read-only and immutable; never creates or writes.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if !path.is_file() {
            return Err(DbError::Sqlite(format!("no file at {}", path.display())));
        }
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = Connection::open_with_flags(file_uri(path), flags)?;
        conn.pragma_update(None, "query_only", true)?;
        Ok(Self { conn })
    }

    /// `count`.
    pub fn count(&self) -> Result<u32, DbError> {
        let n: i64 = self.conn.query_row(sql::COUNT, [], |r| r.get(0))?;
        narrow(n, "count")
    }

    /// `coordinates`: `(globalIndex, surah, ayah)` in index order.
    pub fn coordinates(&self) -> Result<Vec<(u32, u16, u16)>, DbError> {
        let mut stmt = self.conn.prepare_cached(sql::COORDINATES)?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::with_capacity(QURAN_ROW_COUNT as usize);
        while let Some(r) = rows.next()? {
            out.push((
                narrow(int(r.get_ref(0)?, "globalIndex")?, "globalIndex")?,
                narrow(int(r.get_ref(1)?, "surah")?, "surah")?,
                narrow(int(r.get_ref(2)?, "ayah")?, "ayah")?,
            ));
        }
        Ok(out)
    }

    fn surah_text_rows(&self, query: &str) -> Result<Vec<(u16, String)>, DbError> {
        let mut stmt = self.conn.prepare_cached(query)?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push((
                narrow(int(r.get_ref(0)?, "surah")?, "surah")?,
                text(r.get_ref(1)?)?,
            ));
        }
        Ok(out)
    }

    /// `firstAyahs`: `(surah, 1:1 text)`.
    pub fn first_ayahs(&self) -> Result<Vec<(u16, String)>, DbError> {
        self.surah_text_rows(sql::FIRST_AYAHS)
    }

    /// The variant adapter's `openers`.
    pub fn variant_openers(&self) -> Result<Vec<(u16, String)>, DbError> {
        self.surah_text_rows(sql::VARIANT_OPENERS)
    }

    /// `surah`: every verse text of one surah, in order.
    pub fn surah(&self, surah: u16) -> Result<Vec<String>, DbError> {
        let mut stmt = self.conn.prepare_cached(sql::SURAH)?;
        let mut rows = stmt.query([surah])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(text(r.get_ref(0)?)?);
        }
        Ok(out)
    }

    fn ayahs(&self, query: &str, params: &[u32]) -> Result<Vec<Ayah>, DbError> {
        let mut stmt = self.conn.prepare_cached(query)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(params.iter()))?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            let surah: u16 = narrow(int(r.get_ref(1)?, "surah")?, "surah")?;
            let ayah: u16 = narrow(int(r.get_ref(2)?, "ayah")?, "ayah")?;
            out.push(Ayah {
                key: format!("{surah}:{ayah}"),
                surah,
                ayah,
                global_index: narrow(int(r.get_ref(0)?, "globalIndex")?, "globalIndex")?,
                text: text(r.get_ref(3)?)?,
            });
        }
        Ok(out)
    }

    /// `range`: global indexes `from..=to`.
    pub fn range(&self, from: u32, to: u32) -> Result<Vec<Ayah>, DbError> {
        self.ayahs(sql::RANGE, &[from, to])
    }

    /// `all`.
    pub fn all(&self) -> Result<Vec<Ayah>, DbError> {
        self.ayahs(sql::ALL, &[])
    }
}

fn invalid(source_id: &str, message: String) -> DbError {
    DbError::Invalid {
        source_id: source_id.to_owned(),
        message,
    }
}

/// The worker's staging check (`assertStagedQuranBytes`): 6236 rows,
/// contiguous global indexes from 1, coordinates in bounds, readable text.
/// A downloaded file that fails it is discarded, never opened for reading.
pub fn validate_artifact(path: &Path, source_id: &str) -> Result<(), DbError> {
    let db = QuranDb::open(path)?;
    let count = db.count()?;
    if count != QURAN_ROW_COUNT {
        return Err(invalid(
            source_id,
            format!("row count {count} != {QURAN_ROW_COUNT}"),
        ));
    }
    let rows = db.coordinates()?;
    if rows.len() != QURAN_ROW_COUNT as usize {
        return Err(invalid(
            source_id,
            format!("coordinate count {}", rows.len()),
        ));
    }
    for (i, (g, s, a)) in rows.iter().enumerate() {
        if *s < 1 || *s > 114 || *a < 1 {
            return Err(invalid(source_id, format!("bad coordinate {g}/{s}:{a}")));
        }
        if *g as usize != i + 1 {
            return Err(invalid(
                source_id,
                format!("non-contiguous globalIndex at {g}"),
            ));
        }
    }
    db.conn
        .query_row("SELECT text FROM quran_text LIMIT 1", [], |r| {
            r.get::<_, String>(0)
        })?;
    Ok(())
}

/// Uniq surahs of `ayahs`, in first-seen order.
fn surahs_of(ayahs: &[Ayah]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for a in ayahs {
        if !out.contains(&a.surah) {
            out.push(a.surah);
        }
    }
    out
}

/// An Arabic source, opened and checked against the canonical coordinates
/// (`loadQuranSource`).
#[derive(Debug)]
pub struct ArabicDb {
    pub profile: &'static ArabicProfile,
    db: QuranDb,
    view: SourceView,
}

impl ArabicDb {
    /// Opens and validates the artifact at `path` for `source`.
    pub fn open(source: ArabicSource, path: &Path, meta: &QuranMeta) -> Result<Self, DbError> {
        let profile = arabic_profile(source);
        let id = source.id();
        let db = QuranDb::open(path)?;
        let count = db.count()?;
        if count != profile.canonical_row_count {
            return Err(invalid(
                id,
                format!("row count {count} != {}", profile.canonical_row_count),
            ));
        }
        let rows = db.coordinates()?;
        let mut i = 0;
        for s in meta.surahs() {
            for ayah in 1..=s.ayah_count {
                let global = s.start_global + u32::from(ayah) - 1;
                if rows.get(i) != Some(&(global, s.num, ayah)) {
                    return Err(invalid(
                        id,
                        format!("coordinate {i} != {global}/{}:{ayah}", s.num),
                    ));
                }
                i += 1;
            }
        }
        if i != rows.len() {
            return Err(invalid(
                id,
                format!("coordinate row count {} != {i}", rows.len()),
            ));
        }
        let first = db.first_ayahs()?;
        let mut openers = HashMap::new();
        if profile.variant {
            for (surah, text) in db.variant_openers()? {
                if openers.insert(surah, text).is_some() {
                    return Err(invalid(id, format!("duplicate opener for surah {surah}")));
                }
            }
        }
        let view =
            SourceView::build(profile, &first, &openers).map_err(|e| invalid(id, e.to_string()))?;
        Ok(Self { profile, db, view })
    }

    /// The opener partition.
    pub fn view(&self) -> &SourceView {
        &self.view
    }

    /// The raw database (for search corpora and conformance tests).
    pub fn raw(&self) -> &QuranDb {
        &self.db
    }

    /// One surah (`readSurah`).
    pub fn read_surah(&self, surah: u16) -> Result<SurahText, DbError> {
        let normalization = self
            .view
            .normalization(surah)
            .ok_or_else(|| invalid(self.profile.source.id(), format!("invalid surah {surah}")))?
            .clone();
        Ok(SurahText {
            source_id: self.profile.source.id().to_owned(),
            script: self.profile.script(),
            verses: self.db.surah(surah)?,
            normalization,
        })
    }

    /// Global range `from..=to` (`readRange`).
    pub fn read_range(&self, from: u32, to: u32) -> Result<RangeText, DbError> {
        let ayahs = self.db.range(from, to)?;
        let normalizations = surahs_of(&ayahs)
            .into_iter()
            .filter_map(|s| self.view.normalization(s).cloned())
            .collect();
        Ok(RangeText {
            ayahs,
            normalizations,
        })
    }
}

/// A translation source, opened read-only.
#[derive(Debug)]
pub struct TranslationDb {
    pub id: String,
    db: QuranDb,
}

impl TranslationDb {
    /// Opens the translation artifact at `path`.
    pub fn open(id: &str, path: &Path) -> Result<Self, DbError> {
        Ok(Self {
            id: id.to_owned(),
            db: QuranDb::open(path)?,
        })
    }

    /// The raw database.
    pub fn raw(&self) -> &QuranDb {
        &self.db
    }

    /// One surah (`readTranslationSurah`).
    pub fn read_surah(&self, surah: u16) -> Result<SurahText, DbError> {
        Ok(SurahText {
            source_id: self.id.clone(),
            script: QuranScript::Translation,
            verses: self.db.surah(surah)?,
            normalization: SurahNormalization::translation(&self.id, surah),
        })
    }

    /// Global range `from..=to` (`readTranslationRange`).
    pub fn read_range(&self, from: u32, to: u32) -> Result<RangeText, DbError> {
        let ayahs = self.db.range(from, to)?;
        let normalizations = surahs_of(&ayahs)
            .into_iter()
            .map(|s| SurahNormalization::translation(&self.id, s))
            .collect();
        Ok(RangeText {
            ayahs,
            normalizations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_escapes_reserved_characters() {
        assert_eq!(
            file_uri(Path::new("/a b/q?#%.sqlite")),
            "file:/a%20b/q%3F%23%25.sqlite?immutable=1"
        );
    }
}
