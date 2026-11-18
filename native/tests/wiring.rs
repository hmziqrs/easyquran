//! The proof the foundation slices are connected: the app's stores built
//! by `app::provide` over a scripted server (fake `Transport`), memory kv
//! and secrets and a temp data dir. Boots signed out; signing in through
//! `AuthStore` registers the device and runs sync rounds through the D6
//! session transport (`x-eq-session` + CSRF); a bookmark written through
//! the sync store is pushed; signing out clears the session value.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::Mutex,
    time::{Duration, SystemTime},
};

use easyquran::{
    app::{self, Services},
    stores::sync::Area,
    ui::button::button,
};
use eq_core::{
    api::{ApiConfig, AuthPhase, Secret},
    device::{DeviceEnv, FormFactor, Platform},
    kv::{KeyStoreKind, KeyValue, KvError, MemoryKv, SecretStore},
    net::{NetFuture, Request, Response, Transport},
    quran::store::ArtifactBase,
    store::bookmarks,
};
use nsh_core::prelude::*;
use nsh_engine::{Clock, EngineConfig, Root};
use nsh_testing::Headless;
use serde_json::{Value, json};

const BASE: &str = "https://api.test";
const SESSION: &str = "sess-7";
const CSRF: &str = "csrf-7";
const SESSION_KEY: &str = "eq.session";

/// Every request the fake server saw, in order.
static SENT: Mutex<Vec<Request>> = Mutex::new(Vec::new());
/// The secret store's contents, readable from the test thread.
static SECRETS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

fn sent() -> Vec<Request> {
    SENT.lock().unwrap().clone()
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn path(req: &Request) -> &str {
    req.url.strip_prefix(BASE).unwrap_or(&req.url)
}

fn body(req: &Request) -> Value {
    serde_json::from_slice(&req.body).unwrap_or(Value::Null)
}

struct TestSecrets;

impl SecretStore for TestSecrets {
    fn get(&self, key: &str) -> Option<String> {
        SECRETS.lock().unwrap().get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        SECRETS
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }
    fn remove(&self, key: &str) {
        SECRETS.lock().unwrap().remove(key);
    }
    fn kind(&self) -> KeyStoreKind {
        // Keyless registration: no challenge round in the script.
        KeyStoreKind::None
    }
}

fn user() -> Value {
    json!({ "id": 7, "email": "u7@example.com", "is_verified": true })
}

fn reply(status: u16, headers: &[(&str, &str)], body: Value) -> Response {
    Response {
        status,
        headers: headers
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        body: serde_json::to_vec(&body).unwrap(),
    }
}

/// An empty pull page plus `applied` for every pushed mutation.
fn sync_reply(path: &str, sent: &Value) -> Value {
    let results: Vec<Value> = sent["mutations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let id = m.get("id").or_else(|| m.get("key")).cloned();
            json!({ "id": id, "status": "applied" })
        })
        .collect();
    let mut page = match path {
        "/bookmark/v1/sync" => {
            json!({ "folders": [], "bookmarks": [], "defaultFolderId": null })
        }
        "/reading-session/v1/sync" => json!({ "sessions": [], "progress": [] }),
        _ => json!({ "preferences": [] }),
    };
    page["rev"] = json!(0);
    page["latestRev"] = json!(0);
    page["more"] = json!(false);
    page["serverTime"] = json!(0);
    page["syncEpoch"] = json!("epoch-1");
    page["tombstoneFloor"] = json!(0);
    page["results"] = json!(results);
    page
}

/// The scripted backend.
struct FakeServer;

impl FakeServer {
    fn answer(req: &Request) -> Response {
        match path(req) {
            "/csrf/v1/generate" => reply(200, &[], json!({ "token": CSRF })),
            "/auth/v1/log_in" => reply(
                200,
                &[("x-eq-session", SESSION), ("x-eq-session-rotated", "1")],
                user(),
            ),
            "/user/v1/get" => reply(200, &[], user()),
            "/device/v1/register" => reply(
                200,
                &[],
                json!({ "registered": true, "device_id": "0190f5c2-0000-7000-8000-000000000007" }),
            ),
            "/auth/v1/log_out" => reply(200, &[("x-eq-session", "")], json!({})),
            p if p.ends_with("/sync") => reply(200, &[], sync_reply(p, &body(req))),
            _ => reply(404, &[], json!({ "type": "NOT_FOUND", "status": 404 })),
        }
    }
}

impl Transport for FakeServer {
    fn send(&self, request: Request) -> NetFuture<'_, Response> {
        let response = Self::answer(&request);
        SENT.lock().unwrap().push(request);
        Box::pin(async move { Ok(response) })
    }
}

fn data_dir() -> PathBuf {
    std::env::temp_dir().join(format!("eq-wiring-{}", std::process::id()))
}

fn services() -> Services {
    Services {
        transport: Rc::new(FakeServer),
        kv: Rc::new(MemoryKv::new()) as Rc<dyn KeyValue>,
        secrets: Some(Rc::new(TestSecrets)),
        data_dir: Some(data_dir()),
        api: ApiConfig::new(BASE),
        artifact_base: ArtifactBase::Directory(data_dir().join("artifacts")),
        device: DeviceEnv {
            platform: Platform::Macos,
            form_factor: FormFactor::Desktop,
            os_major: None,
            app_version: Some("0.1.0".into()),
            os_id: None,
        },
    }
}

fn phase_label(phase: &AuthPhase) -> &'static str {
    match phase {
        AuthPhase::Unknown => "phase:unknown",
        AuthPhase::SignedOut { .. } => "phase:signed-out",
        AuthPhase::PendingTotp => "phase:totp",
        AuthPhase::Unverified(_) | AuthPhase::SignedIn(_) => "phase:signed-in",
        AuthPhase::Banned(_) => "phase:banned",
    }
}

#[component]
fn WiringRoot() -> impl IntoElement {
    let stores = app::provide(services());
    let (auth, sync) = (stores.auth, stores.sync);
    view()
        .flex_1()
        .child(text(move || SharedString::from(phase_label(&auth.phase()))))
        .child(text(move || {
            SharedString::from(format!("sync-ready:{}", sync.ready()))
        }))
        .child(button("Sign in", move || {
            spawn(async move {
                let _ = auth
                    .sign_in("u7@example.com".into(), Secret::new("pw-123456789"))
                    .await;
            });
        }))
        .child(button("Bookmark", move || {
            sync.write(
                Area::Bookmarks,
                |store| bookmarks::toggle(store, 1, 1, None).map(|_| ()),
                |_| {},
            );
        }))
        .child(button("Sign out", move || {
            spawn(async move {
                let _ = auth.sign_out(None).await;
            });
        }))
}

fn sync_posts(after: usize) -> Vec<Request> {
    sent()
        .into_iter()
        .skip(after)
        .filter(|r| path(r).ends_with("/sync"))
        .collect()
}

#[test]
fn stores_are_wired_through_the_session_transport() -> Result<(), String> {
    let _ = std::fs::remove_dir_all(data_dir());
    let mut config = EngineConfig::new(Root::of::<WiringRoot>());
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    let mut app = Headless::start(config).unwrap();

    // Boot: no stored session → signed out without a probe.
    app.wait_for("text:phase:signed-out")?;
    app.wait_for("text:sync-ready:true")?;
    assert!(sent().iter().all(|r| path(r) != "/user/v1/get"));

    // Sign in: phase, device registration, sync rounds with the session.
    app.press("role:button[Sign in]")?;
    app.wait_for("text:phase:signed-in")?;
    app.run_for(Duration::from_secs(1))?;
    assert_eq!(
        SECRETS.lock().unwrap().get(SESSION_KEY).map(String::as_str),
        Some(SESSION)
    );
    assert!(
        sent().iter().any(|r| path(r) == "/device/v1/register"),
        "device registered after sign-in"
    );
    let rounds = sync_posts(0);
    assert!(!rounds.is_empty(), "sign-in kicked a sync round");
    for post in &rounds {
        assert_eq!(header(post, "x-eq-session"), Some(SESSION));
        assert_eq!(header(post, "csrf-token"), Some(CSRF));
        assert_eq!(body(post)["expectedUserId"], 7);
        assert_eq!(body(post)["client"]["platform"], "macos");
    }

    // A bookmark written through the store is pushed (2 s edit flush).
    let before = sent().len();
    app.press("role:button[Bookmark]")?;
    app.run_for(Duration::from_secs(3))?;
    let pushed = sync_posts(before).into_iter().any(|r| {
        path(&r) == "/bookmark/v1/sync"
            && body(&r)["mutations"]
                .as_array()
                .is_some_and(|m| !m.is_empty())
    });
    assert!(pushed, "the bookmark reached the server");

    // Sign out: the session value is gone.
    app.press("role:button[Sign out]")?;
    app.wait_for("text:phase:signed-out")?;
    assert_eq!(SECRETS.lock().unwrap().get(SESSION_KEY), None);

    let _ = std::fs::remove_dir_all(data_dir());
    Ok(())
}
