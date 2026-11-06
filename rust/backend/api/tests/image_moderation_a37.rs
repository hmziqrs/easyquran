//! Image moderation and CSAM path (A37).
//!
//! - Production boot refuses media upload with no moderator configured.
//! - Upload screening (moderation, then known-hash match) runs before the dedup lookup in
//!   `media_v1::create`, so a refused image never reaches dedup or storage. Source-scan:
//!   the handler is the only caller, and the order is pinned here.
//! - `NoOpModerator` is gone, so no fail-open stub can be wired by accident.

use std::fs;
use std::path::PathBuf;

use ruxlog::config::settings::EnvClass;
use ruxlog::services::image_moderation::{moderation_boot_gate, MEDIA_UPLOAD_MOUNTED};

fn src_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn production_boot_refuses_media_upload_without_moderator() {
    assert!(MEDIA_UPLOAD_MOUNTED, "media_v1 is mounted in router.rs");
    let refused = moderation_boot_gate(EnvClass::Production, MEDIA_UPLOAD_MOUNTED, false);
    assert!(
        refused.is_err(),
        "production must refuse the unsafe combination"
    );
    assert!(moderation_boot_gate(EnvClass::Production, MEDIA_UPLOAD_MOUNTED, true).is_ok());
}

#[test]
fn screening_runs_before_dedup_in_media_create() {
    let path = src_root().join("modules/media_v1/controller.rs");
    let source = fs::read_to_string(&path).expect("read media controller");

    let screen = source
        .find("screen_upload(")
        .expect("media create must call screen_upload");
    let dedup = source
        .find("Media::find_by_hash(")
        .expect("media create must keep the dedup lookup");
    assert!(
        screen < dedup,
        "screen_upload must run before the dedup lookup (A37)"
    );

    let store = source
        .find("Media::create(&state.sea_db")
        .expect("media create must store the row");
    assert!(dedup < store, "dedup must run before storage");
}

#[test]
fn no_noop_moderator_remains_in_source() {
    let mut files = Vec::new();
    collect(&src_root(), &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("read source file");
        assert!(
            !source.contains("NoOpModerator"),
            "NoOpModerator is dead and must not return: {}",
            file.display()
        );
    }
}
