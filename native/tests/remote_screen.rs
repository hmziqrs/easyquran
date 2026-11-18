//! The remote screen area, end to end: the real signing pipeline (throwaway
//! keys, a real `nsh publish` and `nsh verify`), the package running on this
//! crate's engine, and the status page whose revocation control flips.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use nsh_core::prelude::*;
use nsh_engine::{App, Clock, EngineConfig, Root};
use nsh_testing::Headless;

/// A signing step through the real CLI, whose stdout the caller parses.
fn nsh(args: &[&str]) -> String {
    let out = Command::new("nsh")
        .args(args)
        .output()
        .expect("the nsh CLI is on PATH (the gates need it too)");
    assert!(
        out.status.success(),
        "nsh {} failed: {} {}",
        args[0],
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Symlink creation is a per-OS API; the bundler needs the same link either way.
#[cfg(unix)]
fn make_node_modules_link(target: &str, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[cfg(windows)]
fn make_node_modules_link(target: &str, link: &std::path::Path) {
    // A junction, not a symlink: it needs no privilege and resolvers treat
    // it as a plain directory. NTFS stores the target verbatim, so a
    // forward-slash or relative target can fail to resolve; mklink reads
    // its target from the working directory anyway, hence the absolute path.
    let base = link.parent().expect("the link always sits in node_modules");
    let abs = base.join(target).to_string_lossy().replace('/', "\\");
    let link = link.as_os_str().to_string_lossy().replace('/', "\\");
    let out = Command::new("cmd")
        .args(["/C", "mklink", "/J", &link, &abs])
        .output()
        .expect("cmd is on PATH");
    assert!(
        out.status.success(),
        "mklink /J failed: {} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_signed_package_verifies_runs_and_then_refuses_after_revocation() {
    let dir = std::env::temp_dir().join(format!("nsh-starter-remote-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let keys = dir.join("keys");
    let key = |name: &str| keys.join(name).to_string_lossy().into_owned();

    nsh(&["keygen", &key("root")]);
    nsh(&["keygen", &key("release")]);
    let certified = nsh(&[
        "certify",
        "--root",
        &key("root.secret"),
        "--key",
        &key("release.pub"),
        "--apps",
        "com.example.nshstarter.*",
        "--days",
        "90",
        "-o",
        &key("release.cert"),
    ]);
    assert!(certified.contains("certified by root"), "{certified}");

    // The bundler resolves nsh-query and nsh-form through these links, as a
    // fresh clone's delivery step would make them.
    let package_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("packages/remote-screen");
    let modules = package_dir.join("node_modules");
    std::fs::create_dir_all(&modules).unwrap();
    for name in ["nsh-query", "nsh-form"] {
        let target = format!("../../../../{name}/packages/{name}");
        let link = modules.join(name);
        if link.symlink_metadata().is_err() {
            make_node_modules_link(&target, &link);
        }
    }

    let pkg = dir.join("remote.nshpkg");
    let entry = package_dir.join("App.tsx").to_string_lossy().into_owned();
    let published = nsh(&[
        "publish",
        &entry,
        "--key",
        &key("release.secret"),
        "--cert",
        &key("release.cert"),
        "-o",
        pkg.to_string_lossy().as_ref(),
    ]);
    assert!(
        published.contains("com.example.nshstarter.remote 1.0.0 (build 1)"),
        "{published}"
    );

    let verify = nsh(&[
        "verify",
        pkg.to_string_lossy().as_ref(),
        "--root",
        &key("root.pub"),
    ]);
    assert!(
        verify.contains("valid: com.example.nshstarter.remote"),
        "{verify}"
    );
    let sha = verify
        .split("sha256 ")
        .nth(1)
        .unwrap()
        .split([' ', ','])
        .next()
        .unwrap()
        .to_owned();

    // A host's trust store: the verified package runs on this crate's engine.
    let bytes = std::fs::read(&pkg).unwrap();
    let mut trust = nsh_engine::package::TrustStore::open(dir.join("trust")).unwrap();
    let root = nsh_engine::package::PublicKey::from_base64(
        std::fs::read_to_string(key("root.pub")).unwrap().trim(),
    )
    .unwrap();
    trust.add_root(root);
    let loaded = nsh_engine::load_package(&mut trust, &bytes, &dir.join("assets")).unwrap();
    assert_eq!(loaded.app.id, "com.example.nshstarter.remote");

    let config = EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_760_000_000)),
        // The package's bytecode imports `nsh/base`; a raw engine starts with
        // no UI sets, so the host's must be registered before it runs.
        ui_sets: vec![nsh_base::registry()],
        ..EngineConfig::new(App::Js(loaded.bundle))
    };
    let mut app = Headless::start(config).unwrap();
    // The signed bytecode renders the package's own UI; its dashboard fetch
    // has no network here, which is the sandbox's refusal, not the package's.
    app.wait_for("text:Register a screen").unwrap();
    app.wait_for("text:No host on this platform").unwrap();

    // The revocation: the same CLI and trust store must refuse the package.
    let list = dir.join("revocations.json");
    let revoked = nsh(&[
        "revoke",
        "--root",
        &key("root.secret"),
        "--sequence",
        "1",
        "--package",
        &sha,
        "-o",
        list.to_string_lossy().as_ref(),
    ]);
    assert!(revoked.contains("1 packages revoked"), "{revoked}");

    let refused = Command::new("nsh")
        .args([
            "verify",
            pkg.to_string_lossy().as_ref(),
            "--root",
            &key("root.pub"),
            "--revocations",
            list.to_string_lossy().as_ref(),
        ])
        .output()
        .unwrap();
    assert!(!refused.status.success(), "the revoked package verified");
    let message = format!(
        "{}{}",
        String::from_utf8_lossy(&refused.stdout),
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(message.contains("revoked: package"), "{message}");

    let revocations =
        nsh_engine::package::Revocations::from_json(&std::fs::read_to_string(&list).unwrap())
            .unwrap();
    trust.update_revocations(revocations).unwrap();
    let error = trust
        .verify(&bytes, nsh_engine::BYTECODE_VERSION)
        .expect_err("the trust store accepted a revoked package");
    assert!(error.to_string().contains("revoked"), "{error}");
}

/// The page's fixture: obviously synthetic values in the record's shape.
const FIXTURE: &str = r#"{
  "package": {
    "id": "com.example.nshstarter.remote",
    "name": "Remote screen",
    "version": "9.9.9",
    "build": 7,
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "bytes": 123456,
    "path": "packages/remote-screen/.nsh/out/com.example.nshstarter.remote-1.nshpkg"
  },
  "signature": {
    "release_key_id": "0000000000000000",
    "root_key_id": "1111111111111111",
    "certified_apps": "com.example.nshstarter.*",
    "cert_days": 90,
    "min_runtime": "0.4.0"
  },
  "revocation": {
    "sequence": 4,
    "package_revoked": true,
    "verify_error": "error revoked: package 0123456789abcdef"
  }
}"#;

/// Puts back the record a test replaced, on any exit path, panics included:
/// the page's real delivery facts must not keep a test's fixture.
struct RestoreRecord {
    path: PathBuf,
    previous: Option<String>,
}

impl Drop for RestoreRecord {
    fn drop(&mut self) {
        match &self.previous {
            Some(raw) => {
                let _ = std::fs::write(&self.path, raw);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

#[test]
fn the_status_page_reports_the_delivery_and_its_revocation_flips() {
    // The page reads one fixed path; swap in the fixture behind a guard, so
    // whatever a delivery run left there comes back even on a failed assert.
    let path = nsh_starter::features::remote_screen::state_path();
    let previous = std::fs::read_to_string(&path).ok();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, FIXTURE).unwrap();
    let _restore = RestoreRecord { path, previous };

    let config = EngineConfig {
        clock: Clock::manual(SystemTime::UNIX_EPOCH + Duration::from_secs(1_760_000_000)),
        ..EngineConfig::new(Root::of::<RemoteScreenHost>())
    };
    let mut app = Headless::start(config).unwrap();
    app.wait_for("text:Verified").unwrap();
    app.wait_for("text:/build 7/").unwrap();
    app.press("text:Apply revocation list").unwrap();
    app.wait_for("text:Revoked").unwrap();
    app.wait_for("text:/refused from now on/").unwrap();
}

/// A root around the page, the way the shell mounts it.
#[component]
fn RemoteScreenHost() -> impl IntoElement {
    view()
        .flex_1()
        .bg(move || nsh_base::theme().background)
        .child(nsh_starter::features::remote_screen::remote_screen_page())
}
