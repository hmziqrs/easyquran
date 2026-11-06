// Source-scan guard (devices.md D2 rule 9; D3 adds the key and hint fields): device identity never reaches
// analytics, rate-limit keys, ban tables, the reader's quran routes, or logs.
// Sync tables are the one allowed carrier of a device id, so they are not scanned.
use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "device_id",
    "user_devices",
    "user_device::",
    "UserDevice",
    "public_key",
    "key_thumbprint",
    "continuity_hint",
    "merged_key_thumbprint",
    "hint_conflict",
    "user_device_salt",
    "device_v1::keys",
    "device_v1::challenge",
];

/// Files whose job is analytics, rate limiting, abuse or bans, plus the reader
/// routes. Matched by path fragment against `src/`-relative paths.
const GUARDED_PATH_FRAGMENTS: &[&str] = &[
    "analytic",
    "rate_limit",
    "rate-limit",
    "abuse",
    "ban",
    "telemetry",
    "middlewares/client_ip",
    "quran_v1",
    "quran/",
];

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn device_identity_stays_out_of_analytics_limits_bans_and_reader_routes() {
    let root = src_root();
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        !files.is_empty(),
        "source scan found no files under {root:?}"
    );

    let mut scanned = 0usize;
    let mut offenders = Vec::new();
    for file in files {
        let rel = file
            .strip_prefix(&root)
            .unwrap_or(&file)
            .to_string_lossy()
            .replace('\\', "/");
        if !GUARDED_PATH_FRAGMENTS.iter().any(|frag| rel.contains(frag)) {
            continue;
        }
        scanned += 1;
        let text = fs::read_to_string(&file).unwrap_or_default();
        for needle in FORBIDDEN {
            if text.contains(needle) {
                offenders.push(format!("{rel}: contains `{needle}`"));
            }
        }
    }

    assert!(
        scanned > 10,
        "guard matched too few files ({scanned}); fragments are stale"
    );
    assert!(
        offenders.is_empty(),
        "device identity leaked into a guarded surface:\n{}",
        offenders.join("\n")
    );
}
