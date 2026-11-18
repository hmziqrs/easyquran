//! Costly-bug tests over the real artifacts and the download path:
//! - byte conformance: eq-core reads equal the `sqlite3` CLI's bytes;
//! - canonical opener cuts equal the web's `prefix-cuts.json` fixture;
//! - tajweed corpus invariants over all 6236 rows;
//! - no hash function under `src/quran`;
//! - downloads: truncated / oversize bodies discarded, resume, promotion.
//!
//! Tests that need `/Users/hmziq/os/easyquran/db/quran` (or `EQ_QURAN_DB`)
//! print a notice and pass when it is absent.

use std::{
    cell::RefCell,
    future::Future,
    path::{Path, PathBuf},
    pin::pin,
    process::Command,
    task::{Context, Poll, Waker},
};

use super::{
    catalogue::{self, ARABIC_PROFILES},
    db::{ArabicDb, QuranDb, TranslationDb},
    meta::meta,
    store::{ArtifactBase, ArtifactState, ArtifactStore, StoreError, download},
    view::{source_view::scalar_slice, tajweed},
};
use crate::net::{NetError, NetFuture, Request, Response, Transport};

fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

fn db_root() -> Option<PathBuf> {
    let root = std::env::var_os("EQ_QURAN_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Users/hmziq/os/easyquran/db/quran"));
    if root.join("arabic").is_dir() {
        return Some(root);
    }
    eprintln!(
        "NOTICE: {} absent; Quran DB conformance skipped",
        root.display()
    );
    None
}

fn sqlite3_hex(path: &Path, sql: &str) -> Option<String> {
    let out = Command::new("sqlite3")
        .args(["-readonly", "-batch", "-noheader"])
        .arg(path)
        .arg(sql)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// Fixed verse sample: openers, a long verse, last verses, sajda.
const SAMPLE: [u32; 8] = [1, 2, 8, 262, 1160, 4612, 6230, 6236];

#[test]
fn arabic_reads_equal_sqlite3_bytes() {
    let Some(root) = db_root() else { return };
    let base = ArtifactBase::Directory(root);
    for profile in &ARABIC_PROFILES {
        let path = base.local_file(&profile.artifact()).unwrap();
        let db = ArabicDb::open(profile.source, &path, meta()).unwrap();
        for g in SAMPLE {
            let ours = &db.raw().range(g, g).unwrap()[0];
            let cli = sqlite3_hex(
                &path,
                &format!(r#"SELECT hex(text) FROM quran_text WHERE "index" = {g}"#),
            )
            .expect("sqlite3 CLI");
            assert_eq!(
                hex(ours.text.as_bytes()),
                cli,
                "{} #{g}",
                profile.profile_id
            );
        }
        let surah = db.read_surah(2).unwrap();
        let cli = sqlite3_hex(
            &path,
            "SELECT hex(text) FROM quran_text WHERE sura = 2 AND aya = 255",
        )
        .unwrap();
        assert_eq!(hex(surah.verses[254].as_bytes()), cli);
    }
}

#[test]
fn translation_reads_equal_sqlite3_bytes() {
    let Some(root) = db_root() else { return };
    let base = ArtifactBase::Directory(root);
    for id in [
        "en.sahih",
        "ur.jalandhry",
        "qul.r101.uz-alauddin",
        "quranenc.aa.hamza",
    ] {
        let entry = catalogue::translation(id).unwrap();
        let path = base.local_file(&entry.artifact()).unwrap();
        let db = TranslationDb::open(id, &path).unwrap();
        let range = db.read_range(1, 8).unwrap();
        assert_eq!(range.normalizations.len(), 2);
        for a in &range.ayahs {
            let cli = sqlite3_hex(
                &path,
                &format!(
                    r#"SELECT hex(text) FROM quran_text WHERE "index" = {}"#,
                    a.global_index
                ),
            )
            .unwrap();
            assert_eq!(hex(a.text.as_bytes()), cli, "{id} #{}", a.global_index);
        }
    }
}

#[test]
fn opener_cuts_match_web_fixture() {
    let Some(root) = db_root() else { return };
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../web/src/lib/quran/view/__fixtures__/prefix-cuts.json"
    ))
    .unwrap();
    let base = ArtifactBase::Directory(root);
    for src in fixture["sources"].as_array().unwrap() {
        let id = src["sourceId"].as_str().unwrap();
        let source = super::types::ArabicSource::from_id(id).unwrap();
        let profile = catalogue::arabic_profile(source);
        assert_eq!(src["sourceProfile"], profile.profile_id);
        let path = base.local_file(&profile.artifact()).unwrap();
        let db = ArabicDb::open(source, &path, meta()).unwrap();
        let first = db.raw().first_ayahs().unwrap();
        for expected in src["surahs"].as_array().unwrap() {
            let surah = expected["surah"].as_u64().unwrap() as u16;
            let raw = &first[usize::from(surah) - 1].1;
            let n = db.view().normalization(surah).unwrap();
            let got = serde_json::json!({
                "surah": n.surah,
                "openerKind": n.opener_kind,
                "packaging": n.packaging,
                "openerEndScalar": n.opener_end_scalar,
                "bodyStartScalar": n.body_start_scalar,
                "openerText": n.opener_text,
                "separator": scalar_slice(raw, n.opener_end_scalar as usize, Some(n.body_start_scalar as usize)),
                "bodyPrefix20": db.view().body(surah, 1, raw).chars().take(20).collect::<String>(),
            });
            assert_eq!(&got, expected, "{id} surah {surah}");
        }
    }
}

#[test]
fn every_profile_loads_with_expected_packaging() {
    let Some(root) = db_root() else { return };
    let base = ArtifactBase::Directory(root);
    for profile in &ARABIC_PROFILES {
        let path = base.local_file(&profile.artifact()).unwrap();
        let db = ArabicDb::open(profile.source, &path, meta()).unwrap();
        let counts = super::view::source_view::packaging_counts(db.view().normalizations());
        let expected = if profile.variant {
            [1, 0, 0, 112, 1]
        } else {
            [1, 112, 0, 0, 1]
        };
        assert_eq!(counts, expected, "{}", profile.profile_id);
        let all = db.raw().all().unwrap();
        assert_eq!(all.len(), 6236);
    }
}

#[test]
fn tajweed_corpus_invariants() {
    let Some(root) = db_root() else { return };
    let db = QuranDb::open(&root.join("arabic/quran-tajweed.sqlite")).unwrap();
    let rows = db.all().unwrap();
    assert_eq!(rows.len(), 6236);
    let mut bad = Vec::new();
    for row in &rows {
        let stripped = tajweed::strip_tajweed_markup(&row.text);
        if stripped.contains(['[', ']']) {
            bad.push(format!("{} strip", row.key));
        }
        // Independent oracle: the corpus's ASCII is exactly markup.
        let oracle: String = row
            .text
            .chars()
            .filter(|c| !matches!(c, '[' | ']' | ':' | 'a'..='z' | '0'..='9'))
            .collect();
        if stripped != oracle {
            bad.push(format!("{} oracle", row.key));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

/// No hashing of Quran data, anywhere under `src/quran` (hard rule).
#[test]
fn no_hash_function_under_quran() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/quran");
    let needles = [
        ["sha", "2"].concat(),
        ["Sha", "256"].concat(),
        ["sha", "256"].concat(),
        ["SHA", "-256"].concat(),
        ["blake", "3"].concat(),
        ["md", "5::"].concat(),
        ["Hash", "er"].concat(),
        ["dige", "st("].concat(),
    ];
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for n in &needles {
                assert!(!text.contains(n.as_str()), "{} uses {n}", path.display());
            }
        }
    }
}

/// Serves one file, honouring `range`; optionally lies about the body.
struct FakeServer {
    bytes: Vec<u8>,
    ranged: bool,
    /// Cut every body to this many bytes.
    truncate_to: Option<usize>,
    /// Fail with offline after this many requests.
    fail_after: Option<usize>,
    calls: RefCell<usize>,
}

impl FakeServer {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            ranged: true,
            truncate_to: None,
            fail_after: None,
            calls: RefCell::new(0),
        }
    }
}

impl Transport for FakeServer {
    fn send(&self, request: Request) -> NetFuture<'_, Response> {
        let n = {
            let mut c = self.calls.borrow_mut();
            *c += 1;
            *c
        };
        let range = request
            .headers
            .iter()
            .find(|(k, _)| k == "range")
            .and_then(|(_, v)| v.strip_prefix("bytes="))
            .and_then(|r| r.split_once('-'))
            .and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)));
        let result = if self.fail_after.is_some_and(|f| n > f) {
            Err(NetError::Offline("pulled".into()))
        } else {
            let (status, mut body, headers) = match (self.ranged, range) {
                (true, Some((a, b))) => {
                    let b = b.min(self.bytes.len() - 1);
                    (
                        206,
                        self.bytes[a..=b].to_vec(),
                        vec![(
                            "content-range".to_owned(),
                            format!("bytes {a}-{b}/{}", self.bytes.len()),
                        )],
                    )
                }
                _ => (200, self.bytes.clone(), Vec::new()),
            };
            if let Some(t) = self.truncate_to {
                body.truncate(t);
            }
            Ok(Response {
                status,
                headers,
                body,
            })
        };
        Box::pin(async move { result })
    }
}

fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("eq-store-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn store(name: &str) -> ArtifactStore {
    ArtifactStore::new(
        temp_root(name),
        ArtifactBase::Remote("https://test.invalid/_quran".into()),
    )
}

const ID: &str = "en.sahih";

fn fake_bytes() -> Vec<u8> {
    vec![7; catalogue::translation(ID).unwrap().size_bytes as usize]
}

#[test]
fn truncated_body_is_discarded() {
    let s = store("truncated");
    let mut server = FakeServer::new(fake_bytes());
    server.truncate_to = Some(1000);
    let err = block_on(download(&s, &server, ID, |_| {})).unwrap_err();
    assert_eq!(err, StoreError::Truncated(ID.into()));
    assert_eq!(s.state(ID), ArtifactState::Absent);
    assert!(s.path_for(ID).is_none());
}

#[test]
fn size_mismatch_is_refused() {
    let s = store("mismatch");
    let mut bytes = fake_bytes();
    bytes.push(0);
    let mut server = FakeServer::new(bytes);
    server.ranged = false;
    let err = block_on(download(&s, &server, ID, |_| {})).unwrap_err();
    assert!(matches!(err, StoreError::SizeMismatch { .. }), "{err:?}");
    assert_eq!(s.state(ID), ArtifactState::Absent);
    assert!(matches!(
        s.seed(ID, &[1, 2, 3]),
        Err(StoreError::SizeMismatch { .. })
    ));
}

#[test]
fn invalid_database_is_never_promoted() {
    let s = store("invalid");
    let server = FakeServer::new(fake_bytes());
    let err = block_on(download(&s, &server, ID, |_| {})).unwrap_err();
    assert!(matches!(err, StoreError::Invalid(_)), "{err:?}");
    assert_eq!(s.state(ID), ArtifactState::Absent);
}

#[test]
fn offline_keeps_the_partial_and_resumes() {
    let Some(root) = db_root() else { return };
    let entry = catalogue::translation(ID).unwrap();
    let real = std::fs::read(root.join("translations").join(&entry.file_path)).unwrap();
    let s = store("resume");
    let mut server = FakeServer::new(real.clone());
    server.fail_after = Some(1);
    let err = block_on(download(&s, &server, ID, |_| {})).unwrap_err();
    assert!(err.is_resumable());
    assert_eq!(
        s.state(ID),
        ArtifactState::Partial {
            bytes: super::store::CHUNK_BYTES
        }
    );
    assert!(s.path_for(ID).is_none(), "a partial is never opened");
    let server = FakeServer::new(real);
    let mut seen = Vec::new();
    let path = block_on(download(&s, &server, ID, |p| seen.push(p.loaded))).unwrap();
    assert_eq!(seen[0], super::store::CHUNK_BYTES);
    assert_eq!(
        s.state(ID),
        ArtifactState::Present {
            bytes: entry.size_bytes
        }
    );
    assert!(
        TranslationDb::open(ID, &path)
            .unwrap()
            .read_range(1, 1)
            .is_ok()
    );
    let usage = s.usage();
    assert_eq!(usage.artifacts, vec![(ID.to_owned(), entry.size_bytes)]);
    assert_eq!(usage.total_bytes, entry.size_bytes);
    s.delete(ID).unwrap();
    assert_eq!(s.state(ID), ArtifactState::Absent);
}

#[test]
fn development_files_are_read_in_place_and_never_deleted() {
    let Some(root) = db_root() else { return };
    let s = ArtifactStore::new(temp_root("dev"), ArtifactBase::Directory(root.clone()));
    let path = s.path_for("uthmani").unwrap();
    assert_eq!(path, root.join("arabic/quran-uthmani.sqlite"));
    assert_eq!(
        s.delete("uthmani"),
        Err(StoreError::DevReadOnly("uthmani".into()))
    );
    assert!(path.is_file());
}

#[test]
fn evictions_follow_retention_rules() {
    use std::collections::BTreeMap;
    let day = 24 * 60 * 60 * 1000;
    let now = 100 * day;
    let cands = vec![
        ("a".to_owned(), 10),
        ("b".to_owned(), 10),
        ("c".to_owned(), 10),
    ];
    let used: BTreeMap<String, i64> = [
        ("a".to_owned(), now - 40 * day),
        ("b".to_owned(), now - day),
        ("c".to_owned(), now),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        super::store::compute_evictions(&cands, &used, now, &[]),
        vec!["a"]
    );
    assert!(super::store::compute_evictions(&cands, &used, now, &["a".into()]).is_empty());
}
