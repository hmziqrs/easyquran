//! End-to-end headless parity tests exercising every testable PARITY.md row
//! against the real NSH engine (`Headless`) and a scripted API backend
//! (`FakeServer`). Keeps all tests fast, deterministic, and isolated in
//! per-test temp directories.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use easyquran::{
    app::{self, Services},
    shell::{
        self, nav,
        routes::{ReaderLink, ReaderScope, ReaderTarget, Route, SettingsSection},
    },
    stores::{consent::ConsentStore, locale},
    ui::{button::button, toast::notify},
};
use eq_core::{
    api::{ApiConfig, AuthPhase, Secret, types::ReportBody},
    device::{DeviceEnv, FormFactor, Platform},
    i18n::Locale,
    kv::{KeyStoreKind, KeyValue, KvError, MemoryKv, SecretStore},
    net::{NetFuture, Request, Response, Transport},
    quran::store::ArtifactBase,
    store::{bookmarks, preferences},
};
use nsh_core::{
    geometry::{Points, Size},
    prelude::*,
    time::Clock,
};
use nsh_engine::{EngineConfig, Root};
use nsh_testing::Headless;
use serde_json::{Value, json};

const BASE: &str = "https://api.parity.test";
const SESSION: &str = "sess-parity";
const CSRF: &str = "csrf-parity";

static SENT: Mutex<Vec<Request>> = Mutex::new(Vec::new());
static SECRETS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
static REQUIRE_TOTP: AtomicBool = AtomicBool::new(false);
static FRESH_AUTH_READY: AtomicBool = AtomicBool::new(false);

fn clear_harness() {
    SENT.lock().unwrap().clear();
    SECRETS.lock().unwrap().clear();
    REQUIRE_TOTP.store(false, Ordering::SeqCst);
    FRESH_AUTH_READY.store(false, Ordering::SeqCst);
}

fn sent() -> Vec<Request> {
    SENT.lock().unwrap().clone()
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
        KeyStoreKind::None
    }
}

fn user_json() -> Value {
    json!({
        "id": 42,
        "name": "Parity Reader",
        "email": "parity@example.com",
        "is_verified": true,
        "is_minor": false,
        "age_declared_at": null,
        "role": "user",
        "two_fa_enabled": false
    })
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

fn sync_reply(p: &str, sent: &Value) -> Value {
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
    let mut page = match p {
        "/bookmark/v1/sync" => {
            json!({ "folders": [], "bookmarks": [], "defaultFolderId": null })
        }
        "/reading-session/v1/sync" => json!({ "sessions": [], "progress": [] }),
        _ => json!({ "preferences": [] }),
    };
    page["rev"] = json!(1);
    page["latestRev"] = json!(1);
    page["more"] = json!(false);
    page["serverTime"] = json!(1_700_000_000_000_i64);
    page["syncEpoch"] = json!("epoch-parity");
    page["tombstoneFloor"] = json!(0);
    page["results"] = json!(results);
    page
}

struct FakeServer;

impl FakeServer {
    fn answer(req: &Request) -> Response {
        let p = path(req);
        match p {
            "/csrf/v1/generate" => reply(200, &[], json!({ "token": CSRF })),
            "/auth/v1/log_in" => {
                if REQUIRE_TOTP.load(Ordering::SeqCst) {
                    reply(
                        200,
                        &[],
                        json!({ "status": "totp_required", "totp_token": "totp-tok-42" }),
                    )
                } else {
                    reply(
                        200,
                        &[("x-eq-session", SESSION), ("x-eq-session-rotated", "1")],
                        user_json(),
                    )
                }
            }
            "/auth/v1/login/totp" => reply(
                200,
                &[("x-eq-session", SESSION), ("x-eq-session-rotated", "1")],
                user_json(),
            ),
            "/auth/v1/log_out" => reply(200, &[("x-eq-session", "")], json!({})),
            "/user/v1/get" => reply(200, &[], user_json()),
            "/user/v1/update" => {
                let mut u = user_json();
                if let Some(name) = body(req).get("name").and_then(Value::as_str) {
                    u["name"] = json!(name);
                }
                u["email_change_pending"] = json!(false);
                reply(200, &[], u)
            }
            "/user/v1/age/declare" => {
                let mut u = user_json();
                u["age_declared_at"] = json!("2026-11-24T00:00:00Z");
                reply(200, &[], u)
            }
            "/user/v1/identities" => reply(
                200,
                &[],
                json!({
                    "identities": [
                        {
                            "provider": "google",
                            "provider_user_id": "g-42",
                            "created_at": "2026-11-01T00:00:00Z"
                        }
                    ],
                    "has_password": true,
                    "passkey_count": 0,
                    "can_unlink_any": true,
                    "linkable_providers": ["github", "discord"]
                }),
            ),
            "/user/v1/identities/unlink" => reply(
                200,
                &[],
                json!({
                    "identities": [],
                    "has_password": true,
                    "passkey_count": 0,
                    "can_unlink_any": false,
                    "linkable_providers": ["google", "github", "discord"]
                }),
            ),
            "/auth/v1/reauth/status" => reply(
                200,
                &[],
                json!({
                    "ready": FRESH_AUTH_READY.load(Ordering::SeqCst),
                    "has_password": true,
                    "two_fa_enabled": false
                }),
            ),
            "/auth/v1/reauth" => {
                FRESH_AUTH_READY.store(true, Ordering::SeqCst);
                reply(200, &[], json!({ "ready": true }))
            }
            "/auth/v1/2fa/setup" => reply(
                200,
                &[],
                json!({
                    "secret": "JBSWY3DPEHPK3PXP",
                    "otpauth_url": "otpauth://totp/EasyQuran:parity@example.com?secret=JBSWY3DPEHPK3PXP",
                    "backup_codes": ["CODE-1", "CODE-2"]
                }),
            ),
            "/auth/v1/sessions/list" => reply(
                200,
                &[],
                json!({
                    "data": [
                        {
                            "id": 101,
                            "isCurrent": true,
                            "device": "EasyQuran macOS",
                            "ip_address": "127.0.0.1",
                            "last_seen": "2026-11-24T00:00:00Z",
                            "created_at": "2026-11-24T00:00:00Z"
                        },
                        {
                            "id": 102,
                            "isCurrent": false,
                            "device": "Firefox Desktop",
                            "ip_address": "127.0.0.2",
                            "last_seen": "2026-11-23T00:00:00Z",
                            "created_at": "2026-11-23T00:00:00Z"
                        }
                    ]
                }),
            ),
            p if p.starts_with("/auth/v1/sessions/terminate/") => reply(200, &[], json!({})),
            "/device/v1/register" => reply(
                200,
                &[],
                json!({ "registered": true, "device_id": "0190f5c2-0000-7000-8000-000000000042" }),
            ),
            "/device/v1/list" => reply(
                200,
                &[],
                json!({
                    "devices": [
                        {
                            "device_id": "0190f5c2-0000-7000-8000-000000000042",
                            "name": "MacBook",
                            "platform": "macos",
                            "client": "nsh",
                            "form_factor": "desktop",
                            "current": true,
                            "session_count": 1
                        }
                    ]
                }),
            ),
            "/device/v1/rename" => reply(200, &[], json!({ "renamed": true })),
            "/device/v1/delete" => reply(200, &[], json!({ "revoked": true })),
            "/user/v1/delete" => reply(
                200,
                &[("x-eq-session", "")],
                json!({ "receipt_id": "rcpt-42", "user_row_erased": true }),
            ),
            "/notification/v1/reminder/get" => reply(
                200,
                &[],
                json!({
                    "reminder": {
                        "kind": "goal_daily",
                        "enabled": true,
                        "time_zone": "UTC",
                        "reminder_minute": 480,
                        "quiet_start_minute": 1320,
                        "quiet_end_minute": 420
                    }
                }),
            ),
            "/notification/v1/reminder/set" => reply(200, &[], json!({ "reminder": body(req) })),
            "/user/analytics/v1/events" => reply(200, &[], json!({ "accepted": 1 })),
            "/user/analytics/v1/summary" => reply(
                200,
                &[],
                json!({
                    "totals": { "ayahs": 128, "ms": 600000, "days": 4 },
                    "topSurahs": [{ "surah": 1, "reads": 14, "ms": 60000 }],
                    "topAyahs": [{ "surah": 1, "ayah": 1, "reads": 7, "ms": 15000 }],
                    "days": [{ "day": "2026-11-24", "ayahs": 32, "ms": 120000 }]
                }),
            ),
            p if p.starts_with("/user/analytics/v1/days") => reply(
                200,
                &[],
                json!({
                    "days": [
                        { "day": "2026-11-24", "ayahs": 32, "ms": 120000 }
                    ]
                }),
            ),
            "/appeal/v1/submit" => reply(200, &[], json!({ "received": true })),
            "/report/v1/submit" => reply(200, &[], json!({ "received": true })),
            "/newsletter/v1/confirm" => reply(200, &[], json!({ "confirmed": true })),
            "/newsletter/v1/subscribe" => reply(200, &[], json!({ "subscribed": true })),
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

fn test_data_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("eq-parity-{tag}-{}", std::process::id()))
}

fn artifact_base(tag: &str) -> ArtifactBase {
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../db/quran");
    if local
        .join("arabic/quran-uthmani-annotated.sqlite")
        .is_file()
    {
        return ArtifactBase::Directory(local);
    }
    ArtifactBase::Directory(test_data_dir(tag).join("artifacts"))
}

fn make_services(tag: &str) -> Services {
    let dir = test_data_dir(tag);
    Services {
        transport: Rc::new(FakeServer),
        kv: Rc::new(MemoryKv::new()) as Rc<dyn KeyValue>,
        secrets: Some(Rc::new(TestSecrets)),
        data_dir: Some(dir),
        api: ApiConfig::new(BASE),
        artifact_base: artifact_base(tag),
        device: DeviceEnv {
            platform: Platform::Macos,
            form_factor: FormFactor::Desktop,
            os_major: Some(15),
            app_version: Some("0.1.0".into()),
            os_id: None,
        },
    }
}

/// Queued command executed on the engine thread when the test presses the
/// hidden driver button (`drive-step`).
#[derive(Clone, Default)]
struct Drive {
    route: Option<Route>,
    toast: Option<String>,
    sign_in_totp: bool,
    submit_totp: bool,
    set_locale: Option<Locale>,
    write_note: bool,
    minor_gate: Option<bool>,
    send_report: bool,
    fetch_insights: bool,
}

static PENDING_DRIVE: Mutex<Option<Drive>> = Mutex::new(None);

#[component]
fn ParityApp() -> impl IntoElement {
    locale::install();
    let stores = app::provide(make_services("suite"));
    shell::install();
    let auth = stores.auth;
    let sync = stores.sync;
    let run_pending = move || {
        let Some(cmd) = PENDING_DRIVE.lock().unwrap().take() else {
            return;
        };
        if let Some(loc) = cmd.set_locale {
            locale::set_locale(loc);
        }
        if let Some(minor) = cmd.minor_gate {
            ConsentStore::get().set_minor_account(minor);
        }
        if let Some(msg) = cmd.toast {
            notify(msg);
        }
        if let Some(route) = cmd.route {
            nav::open(route);
        }
        if cmd.sign_in_totp {
            spawn(async move {
                let _ = auth
                    .sign_in("parity@example.com".into(), Secret::new("password-123456"))
                    .await;
            });
        }
        if cmd.submit_totp {
            spawn(async move {
                let _ = auth.submit_totp(Secret::new("123456")).await;
            });
        }
        if cmd.write_note {
            sync.write(
                easyquran::stores::sync::Area::Bookmarks,
                |s| bookmarks::toggle(s, 1, 1, None).map(|_| ()),
                |_| {},
            );
            sync.write(
                easyquran::stores::sync::Area::Preferences,
                |s| preferences::set_reading_bookmark(s, 1, 1, None).map(|_| ()),
                |_| {},
            );
        }
        if cmd.send_report
            && let Some(client) = auth.client()
        {
            let body = ReportBody {
                target_kind: "translation".into(),
                target_id: "en.sahih:1:1".into(),
                reason: "other".into(),
                details: Some("Typo check".into()),
                contact_email: None,
            };
            spawn(async move {
                let _ = client.report_submit(&body).await;
            });
        }
        if cmd.fetch_insights {
            spawn(async move {
                let _ = easyquran::stores::insights::fetch_summary().await;
            });
        }
    };
    view()
        .flex_1()
        .direction(locale::layout_direction)
        .child(shell::shell())
        .child(
            view()
                .flex_row()
                .gap_2()
                .child(button("drive-step", run_pending))
                .child(text(move || {
                    let p = match auth.phase() {
                        AuthPhase::SignedIn(_) => "signed-in",
                        AuthPhase::PendingTotp => "pending-totp",
                        AuthPhase::SignedOut { .. } => "signed-out",
                        AuthPhase::Unverified(_) => "unverified",
                        AuthPhase::Banned(_) => "banned",
                        AuthPhase::Unknown => "unknown",
                    };
                    SharedString::from(format!("auth-phase:{p}"))
                }))
                .child(text(move || {
                    SharedString::from(format!(
                        "consent-allowed:{}",
                        ConsentStore::get().analytics_allowed()
                    ))
                }))
                .child(text(move || {
                    SharedString::from(format!("locale-now:{:?}", locale::locale()))
                })),
        )
}

fn start_desktop() -> Headless {
    let mut config = EngineConfig::new(Root::of::<ParityApp>());
    config.viewport = Size::new(Points(1280.0), Points(800.0));
    config.clock = Clock::manual(SystemTime::UNIX_EPOCH);
    Headless::start(config).unwrap()
}

fn send_drive(app: &mut Headless, f: impl FnOnce(&mut Drive)) {
    let mut cmd = Drive::default();
    f(&mut cmd);
    *PENDING_DRIVE.lock().unwrap() = Some(cmd);
    app.press("role:button[drive-step]").unwrap();
}

#[test]
fn parity_full_app_exercise() -> Result<(), String> {
    clear_harness();
    let _ = std::fs::remove_dir_all(test_data_dir("suite"));
    let mut app = start_desktop();
    app.wait_for("text:auth-phase:signed-out")?;

    // --- A4: Marketing / static pages (About, FAQ, Contact, Privacy, Terms) ---
    for (route, needle) in [
        (
            Route::About,
            "text:easyquran is a free Qur'an reader with no ads.",
        ),
        (Route::Faq, "text:Is easyquran free?"),
        (Route::Contact, "text:support@easyquran.fyi"),
        (Route::Privacy, "text:1. What we collect"),
        (Route::Terms, "text:Terms"),
    ] {
        send_drive(&mut app, move |d| d.route = Some(route));
        app.wait_for(needle)?;
    }

    // --- A7: Keyboard shortcuts (`/`, `mod+,`) ---
    app.press_key("/")?;
    app.wait_for("text:Go to")?;
    app.press_key("Escape")?;
    app.wait_for_gone("text:Go to", Duration::from_secs(5))?;
    app.press_key("mod+,")?;
    app.wait_for("label:settings-nav-appearance")?;

    // --- A8: Toast host ---
    send_drive(&mut app, |d| d.toast = Some("Parity toast live".into()));
    app.wait_for("text:Parity toast live")?;

    // --- J2-J8 & N1-N3 & O2: Every Settings section + consent + locale switch ---
    for section in SettingsSection::ALL {
        send_drive(&mut app, move |d| d.route = Some(Route::Settings(section)));
        app.run_for(Duration::from_millis(50))?;
    }
    // Privacy section: analytics toggle + consent gating (N1, N2, N3, J6)
    send_drive(&mut app, |d| {
        d.route = Some(Route::Settings(SettingsSection::Privacy));
    });
    app.wait_for("text:Analytics")?;
    app.wait_for("text:consent-allowed:false")?;
    app.press("label:Analytics")?;
    app.wait_for("text:consent-allowed:true")?;
    // Minor gate (I13) forces analytics off even when stored preference is on.
    send_drive(&mut app, |d| d.minor_gate = Some(true));
    app.wait_for("text:consent-allowed:false")?;
    send_drive(&mut app, |d| d.minor_gate = Some(false));
    app.wait_for("text:consent-allowed:true")?;
    app.press("label:Analytics")?;
    app.wait_for("text:consent-allowed:false")?;

    // Locale switch English -> Arabic -> English (O2)
    send_drive(&mut app, |d| d.set_locale = Some(Locale::Ar));
    app.wait_for("text:locale-now:Ar")?;
    send_drive(&mut app, |d| d.set_locale = Some(Locale::En));
    app.wait_for("text:locale-now:En")?;

    // --- K2 / B5: Downloads screen ---
    send_drive(&mut app, |d| d.route = Some(Route::Downloads));
    app.wait_for("text:Downloads")?;

    // --- C3 / C8 / C11: Page, Juz, Hizb readers + mode toggle + typography ---
    for scope in [
        ReaderScope::Page(1),
        ReaderScope::Juz(1),
        ReaderScope::Hizb(1),
    ] {
        let link = ReaderLink::new(ReaderTarget::arabic(scope));
        send_drive(&mut app, move |d| d.route = Some(Route::Reader(link)));
        app.wait_for("text:Page 1 · Juz 1 · Hizb 1")?;
    }
    // Switch reader mode Ayahs -> Reading -> Ayahs and adjust font size (C8)
    app.press("text:Reading")?;
    app.run_for(Duration::from_millis(50))?;
    app.press("text:Ayahs")?;
    app.run_for(Duration::from_millis(50))?;
    app.press("text:A+")?;
    app.press("text:A\u{2212}")?;
    app.run_for(Duration::from_millis(50))?;

    // Open ShareDialog -> switch Format to Image card -> download PNG card (C9)
    let fatiha = ReaderLink::new(ReaderTarget::arabic(ReaderScope::Surah(1)));
    send_drive(&mut app, move |d| d.route = Some(Route::Reader(fatiha)));
    app.wait_for("text:Al-Fatihah")?;
    app.press("label:Share options >> nth=0")?;
    app.wait_for("text:Image card")?;
    app.press("text:Image card")?;
    app.wait_for("text:Download image")?;
    app.press("text:Download image")?;
    app.wait_for("text:Image saved")?;
    let card_path = std::env::temp_dir().join("easyquran/share/easyquran-1-1-1.png");
    assert!(
        card_path.is_file(),
        "verse card PNG must exist at {}",
        card_path.display()
    );
    let _ = std::fs::remove_file(&card_path);
    app.back()?;
    app.wait_for_gone("text:Share verses", Duration::from_secs(5))?;

    // --- E2 / E4: Search page ---
    send_drive(&mut app, |d| d.route = Some(Route::Search));
    app.wait_for("text:Search")?;

    // --- I2 / I3 / I8 / F4 / J9: Sign in with TOTP pending_login -> device register + sync ---
    REQUIRE_TOTP.store(true, Ordering::SeqCst);
    send_drive(&mut app, |d| d.sign_in_totp = true);
    app.wait_for("text:auth-phase:pending-totp")?;
    send_drive(&mut app, |d| d.route = Some(Route::Totp));
    app.run_for(Duration::from_millis(50))?;
    send_drive(&mut app, |d| d.submit_totp = true);
    app.wait_for("text:auth-phase:signed-in")?;
    app.wait_until(
        "device registered and sync ran",
        Duration::from_secs(5),
        |_| {
            let reqs = sent();
            reqs.iter().any(|r| path(r) == "/device/v1/register")
                && reqs.iter().any(|r| path(r).ends_with("/sync"))
        },
    )?;

    // Verify os_major was included in device registration (I8).
    let reg = sent()
        .into_iter()
        .find(|r| path(r) == "/device/v1/register")
        .expect("register request");
    assert_eq!(body(&reg)["os_major"], json!(15));

    // --- F1 / F3 / F4: Write bookmark + reading place -> sync pushes ---
    send_drive(&mut app, |d| d.write_note = true);
    app.run_for(Duration::from_secs(3))?;

    // --- G3 / G4 / G5 / G6 / G7 / F2: Yours hub (insights, goals, notes, cards, reminders) ---
    send_drive(&mut app, |d| {
        d.route = Some(Route::Yours);
        d.fetch_insights = true;
    });
    app.wait_for("text:Your cards")?;
    app.wait_for("text:Add card")?;
    app.wait_until("personal insights fetched", Duration::from_secs(5), |_| {
        sent()
            .iter()
            .any(|r| path(r) == "/user/analytics/v1/summary")
    })?;
    app.wait_until("goal reminder loaded", Duration::from_secs(5), |_| {
        sent()
            .iter()
            .any(|r| path(r) == "/notification/v1/reminder/get")
    })?;

    // --- I3 / I4 / I6 / I7 / I9 / I13: Account page ---
    send_drive(&mut app, |d| d.route = Some(Route::Account));
    app.wait_until(
        "account endpoints fetched (devices, sessions, identities, reauth)",
        Duration::from_secs(5),
        |_| {
            let reqs = sent();
            reqs.iter().any(|r| path(r) == "/device/v1/list")
                && reqs.iter().any(|r| path(r) == "/auth/v1/sessions/list")
                && reqs.iter().any(|r| path(r) == "/user/v1/identities")
                && reqs.iter().any(|r| path(r) == "/auth/v1/reauth/status")
        },
    )?;
    app.wait_for("text:Parity Reader")?;
    app.wait_for("text:Google")?;
    // Connected accounts unlink confirm flow (I4)
    app.press("role:button[Unlink]")?;
    app.wait_for("role:button[Confirm unlink]")?;
    app.press("role:button[Confirm unlink]")?;
    app.wait_until("identity unlinked", Duration::from_secs(5), |_| {
        sent()
            .iter()
            .any(|r| path(r) == "/user/v1/identities/unlink")
    })?;

    // --- I10: Appeal route ---
    let valid_token = "a".repeat(64);
    send_drive(&mut app, move |d| {
        d.route = Some(Route::Appeal(Some(valid_token)));
    });
    app.wait_for("text:Your statement")?;

    // --- I11: Report content endpoint ---
    send_drive(&mut app, |d| d.send_report = true);
    app.wait_until("report submitted", Duration::from_secs(5), |_| {
        sent().iter().any(|r| path(r) == "/report/v1/submit")
    })?;

    // --- I12: Newsletter confirm route ---
    send_drive(&mut app, |d| {
        d.route = Some(Route::NewsletterConfirm {
            email: "parity@example.com".into(),
            token: "confirm-tok-1".into(),
        });
    });
    app.wait_until("newsletter confirmed", Duration::from_secs(5), |_| {
        sent().iter().any(|r| path(r) == "/newsletter/v1/confirm")
    })?;

    let _ = std::fs::remove_dir_all(test_data_dir("suite"));
    Ok(())
}
