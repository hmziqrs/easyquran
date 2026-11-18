//! Session transport (D6) and auth state, over a scripted fake transport.
//! Costly-bug class: a wrong header here signs people out or leaks the
//! session.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    future::Future,
    rc::Rc,
    task::{Context, Poll, Waker},
};

use serde_json::{Value, json};

use super::{
    ApiClient, ApiConfig, ApiError, ApiPost, AuthMachine, AuthPhase, Secret, SignInStep,
    TransitionKind,
    session::{CSRF_HEADER, SESSION_HEADER, SESSION_KEY},
};
use crate::{
    clock::ManualClock,
    device::{DeviceEnv, FormFactor, Platform, RegisterOutcome, ensure_device_registered},
    kv::{KeyStoreKind, KeyValue, KvError, MemoryKv, SecretStore},
    net::{NetFuture, Request, Response, Transport},
};

/// Runs a future whose awaits all complete at once (the fake transport's).
fn block_on<F: Future>(fut: F) -> F::Output {
    let mut fut = std::pin::pin!(fut);
    let mut cx = Context::from_waker(Waker::noop());
    match fut.as_mut().poll(&mut cx) {
        Poll::Ready(out) => out,
        Poll::Pending => panic!("fake transport never pends"),
    }
}

#[derive(Default)]
struct MemorySecrets {
    map: RefCell<BTreeMap<String, String>>,
    kind: Option<KeyStoreKind>,
}

impl SecretStore for MemorySecrets {
    fn get(&self, key: &str) -> Option<String> {
        self.map.borrow().get(key).cloned()
    }
    fn set(&self, key: &str, value: &str) -> Result<(), KvError> {
        self.map
            .borrow_mut()
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }
    fn remove(&self, key: &str) {
        self.map.borrow_mut().remove(key);
    }
    fn kind(&self) -> KeyStoreKind {
        self.kind.unwrap_or(KeyStoreKind::File)
    }
}

/// Answers each request with the next scripted reply for its path, and
/// records what was sent.
#[derive(Default)]
struct Script {
    replies: RefCell<VecDeque<(String, Response)>>,
    sent: RefCell<Vec<Request>>,
}

impl Script {
    fn reply(&self, path: &str, status: u16, headers: &[(&str, &str)], body: Value) {
        let body = if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).unwrap()
        };
        self.replies.borrow_mut().push_back((
            path.to_owned(),
            Response {
                status,
                headers: headers
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
                body,
            },
        ));
    }

    fn sent(&self) -> Vec<Request> {
        self.sent.borrow().clone()
    }
}

impl Transport for Script {
    fn send(&self, request: Request) -> NetFuture<'_, Response> {
        let (path, response) = self
            .replies
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| panic!("unscripted request {}", request.url));
        assert!(
            request.url.ends_with(&path),
            "expected {path}, got {}",
            request.url
        );
        self.sent.borrow_mut().push(request);
        Box::pin(async move { Ok(response) })
    }
}

fn header<'a>(req: &'a Request, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn setup(kind: KeyStoreKind) -> (Rc<Script>, Rc<MemorySecrets>, Rc<ApiClient>) {
    let script = Rc::new(Script::default());
    let secrets = Rc::new(MemorySecrets {
        kind: Some(kind),
        ..MemorySecrets::default()
    });
    let client = Rc::new(ApiClient::new(
        ApiConfig::local(),
        script.clone(),
        secrets.clone(),
    ));
    (script, secrets, client)
}

fn user(id: i32) -> Value {
    json!({ "id": id, "email": format!("u{id}@example.com"), "is_verified": true })
}

#[test]
fn full_native_flow_keeps_rotates_and_clears_the_session() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    let kv: Rc<dyn KeyValue> = Rc::new(MemoryKv::new());
    let machine = AuthMachine::new(client.clone(), kv.clone());
    let kinds: Rc<RefCell<Vec<TransitionKind>>> = Rc::default();
    let seen = kinds.clone();
    machine.on_transition(move |t| seen.borrow_mut().push(t.kind));

    // Fresh install: CSRF first (empty header), the reply carries a session.
    script.reply(
        "/csrf/v1/generate",
        200,
        &[(SESSION_HEADER, "anon0")],
        json!({"token": "csrf0"}),
    );
    // Sign-in rotates the session id.
    script.reply(
        "/auth/v1/log_in",
        200,
        &[(SESSION_HEADER, "sess1"), ("x-eq-session-rotated", "1")],
        user(7),
    );
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "csrf1"}));
    let step = block_on(machine.log_in("u7@example.com", &Secret::new("pw-123456789")));
    assert!(matches!(step, Ok(SignInStep::Done(ref u)) if u.id == 7));
    assert_eq!(secrets.get(SESSION_KEY).as_deref(), Some("sess1"));

    // A response without the header keeps the value; the new CSRF is sent.
    script.reply("/device/v1/list", 200, &[], json!({"devices": []}));
    block_on(client.devices()).unwrap();
    assert_eq!(secrets.get(SESSION_KEY).as_deref(), Some("sess1"));

    let sent = script.sent();
    assert_eq!(
        header(&sent[0], SESSION_HEADER),
        Some(""),
        "empty when none"
    );
    assert_eq!(header(&sent[1], SESSION_HEADER), Some("anon0"));
    assert_eq!(header(&sent[1], CSRF_HEADER), Some("csrf0"));
    assert_eq!(
        header(&sent[2], SESSION_HEADER),
        Some("sess1"),
        "refetch on rotated"
    );
    assert_eq!(header(&sent[3], CSRF_HEADER), Some("csrf1"));
    for req in &sent {
        for banned in [
            "cookie",
            "origin",
            "sec-fetch-site",
            "sec-fetch-mode",
            "sec-fetch-dest",
        ] {
            assert!(
                header(req, banned).is_none(),
                "{banned} sent to {}",
                req.url
            );
        }
        assert!(
            header(req, SESSION_HEADER).is_some(),
            "header on every call"
        );
    }

    // Revoked elsewhere: 401 clears the value, signs out with the notice.
    script.reply(
        "/device/v1/list",
        401,
        &[],
        json!({"error": {"code": "AUTH_UNAUTHENTICATED"}}),
    );
    let err = block_on(client.devices()).unwrap_err();
    assert!(err.is_session_lost());
    assert_eq!(secrets.get(SESSION_KEY), None);
    assert!(!client.has_csrf());
    assert_eq!(
        machine.phase(),
        AuthPhase::SignedOut {
            session_expired: true
        }
    );
    assert_eq!(
        *kinds.borrow(),
        [
            TransitionKind::Login,
            TransitionKind::CurrentSessionTerminated
        ]
    );
}

#[test]
fn log_out_clears_the_stored_value() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    let machine = AuthMachine::new(client.clone(), Rc::new(MemoryKv::new()));
    secrets.set(SESSION_KEY, "sess1").unwrap();
    // Hydrate reads with GETs: no CSRF before them.
    script.reply(
        "/auth/v1/login/totp/pending",
        200,
        &[],
        json!({"status": "none"}),
    );
    script.reply("/user/v1/get", 200, &[], user(3));
    block_on(machine.hydrate());
    assert!(matches!(machine.phase(), AuthPhase::SignedIn(_)));

    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "c"}));
    script.reply(
        "/auth/v1/log_out",
        200,
        &[(SESSION_HEADER, "")],
        json!({"message": "Logged out"}),
    );
    block_on(machine.log_out(None)).unwrap();
    assert_eq!(secrets.get(SESSION_KEY), None);
    assert!(!client.has_csrf());
    assert_eq!(
        machine.phase(),
        AuthPhase::SignedOut {
            session_expired: false
        }
    );
}

#[test]
fn pending_login_needs_the_totp_step() {
    let (script, _secrets, client) = setup(KeyStoreKind::File);
    let machine = AuthMachine::new(client.clone(), Rc::new(MemoryKv::new()));
    script.reply(
        "/csrf/v1/generate",
        200,
        &[(SESSION_HEADER, "a")],
        json!({"token": "c"}),
    );
    script.reply(
        "/auth/v1/log_in",
        200,
        &[(SESSION_HEADER, "a2")],
        json!({"status": "totp_required", "totp_token": "tt"}),
    );
    let step = block_on(machine.log_in("u@example.com", &Secret::new("pw")));
    assert_eq!(step, Ok(SignInStep::TotpRequired));
    assert_eq!(machine.phase(), AuthPhase::PendingTotp);
    assert!(machine.user().is_none(), "not signed in before the code");

    // A wrong code shares AUTH_009 with CSRF but is not retried.
    script.reply(
        "/auth/v1/login/totp",
        401,
        &[],
        json!({"type": "AUTH_009", "status": 401}),
    );
    let wrong = block_on(machine.submit_totp(&Secret::new("000000")));
    assert!(wrong.is_err());
    assert_eq!(machine.phase(), AuthPhase::PendingTotp);

    script.reply(
        "/auth/v1/login/totp",
        200,
        &[(SESSION_HEADER, "s"), ("x-eq-session-rotated", "1")],
        user(5),
    );
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "c2"}));
    let done = block_on(machine.submit_totp(&Secret::new("123456")));
    assert!(matches!(done, Ok(SignInStep::Done(ref u)) if u.id == 5));
    let sent = script.sent();
    let body: Value = serde_json::from_slice(&sent[3].body).unwrap();
    assert_eq!(body["totp_token"], "tt");
}

#[test]
fn a_csrf_refusal_refetches_the_token_and_retries_once() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    secrets.set(SESSION_KEY, "s").unwrap();
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "stale"}));
    script.reply(
        "/bookmark/v1/sync",
        401,
        &[],
        json!({"type": "AUTH_009", "status": 401, "context": {"reason": "mismatch"}}),
    );
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "fresh"}));
    script.reply("/bookmark/v1/sync", 200, &[], json!({"applied": 1}));
    let out = block_on(client.post_json("/bookmark/v1/sync", json!({}))).unwrap();
    assert_eq!(out["applied"], 1);
    assert_eq!(header(&script.sent()[3], CSRF_HEADER), Some("fresh"));
    assert_eq!(
        secrets.get(SESSION_KEY).as_deref(),
        Some("s"),
        "not a lost session"
    );
}

#[test]
fn invalid_session_values_are_never_stored() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    secrets.set(SESSION_KEY, "good").unwrap();
    script.reply("/user/v1/get", 200, &[(SESSION_HEADER, "a;b=c")], user(1));
    block_on(client.get_user()).unwrap();
    assert_eq!(secrets.get(SESSION_KEY).as_deref(), Some("good"));
}

#[test]
fn keyed_registration_signs_a_fresh_nonce_and_adopts_the_server_id() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    secrets.set(SESSION_KEY, "s").unwrap();
    let kv = MemoryKv::new();
    let clock = ManualClock::new(1_000);
    let env = DeviceEnv {
        platform: Platform::Linux,
        form_factor: FormFactor::Desktop,
        os_major: Some(6),
        app_version: Some("1.0.0".into()),
        os_id: Some("  ABC  ".into()),
    };
    let nonce = crate::device::keys::b64u_encode(&[9_u8; 32]);
    let salt = crate::device::keys::b64u_encode(&[3_u8; 32]);
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "c"}));
    script.reply(
        "/device/v1/challenge",
        200,
        &[],
        json!({"nonce": nonce, "salt": salt}),
    );
    script.reply(
        "/device/v1/register",
        409,
        &[],
        json!({"type": "DB_003", "status": 409}),
    );
    script.reply(
        "/device/v1/challenge",
        200,
        &[],
        json!({"nonce": nonce, "salt": salt}),
    );
    script.reply(
        "/device/v1/register",
        200,
        &[],
        json!({"registered": true, "device_id": "server-id"}),
    );
    let profile = crate::api::types::UserProfile::decode(&user(7)).unwrap();
    let out = block_on(ensure_device_registered(
        &client, &kv, &clock, &env, &profile, None,
    ));
    assert_eq!(
        out,
        Ok(RegisterOutcome::Registered {
            device_id: "server-id".into(),
            keyed: true
        })
    );
    let sent = script.sent();
    let first: Value = serde_json::from_slice(&sent[2].body).unwrap();
    let retry: Value = serde_json::from_slice(&sent[4].body).unwrap();
    assert_ne!(first["device_id"], retry["device_id"], "409 mints a new id");
    assert_ne!(first["public_key"], retry["public_key"], "and a new key");
    assert_eq!(retry["client"], "gpui");
    assert_eq!(retry["key_store"], "file");
    let mut members: Vec<&String> = retry["public_key"].as_object().unwrap().keys().collect();
    members.sort();
    assert_eq!(members, ["crv", "kty", "x", "y"]);
    let expected_hint = crate::device::keys::continuity_hint(&[3_u8; 32], "abc");
    assert_eq!(retry["continuity_hint"], expected_hint.as_str());
    assert_eq!(
        kv.get("easyquran.device.id.7").as_deref(),
        Some("server-id")
    );
    let stored = crate::device::register::load_device_key(secrets.as_ref(), 7).unwrap();
    assert_eq!(stored.device_id, "server-id");

    // Not due again within the day.
    let again = block_on(ensure_device_registered(
        &client, &kv, &clock, &env, &profile, None,
    ));
    assert_eq!(again, Ok(RegisterOutcome::NotDue));
}

#[test]
fn ios_without_secure_hardware_registers_keyless() {
    let (script, secrets, client) = setup(KeyStoreKind::None);
    secrets.set(SESSION_KEY, "s").unwrap();
    let env = DeviceEnv {
        platform: Platform::Ios,
        form_factor: FormFactor::Phone,
        os_major: Some(19),
        app_version: None,
        os_id: None,
    };
    script.reply("/csrf/v1/generate", 200, &[], json!({"token": "c"}));
    script.reply(
        "/device/v1/register",
        200,
        &[],
        json!({"registered": true, "device_id": "d"}),
    );
    let profile = crate::api::types::UserProfile::decode(&user(2)).unwrap();
    let out = block_on(ensure_device_registered(
        &client,
        &MemoryKv::new(),
        &ManualClock::new(0),
        &env,
        &profile,
        None,
    ));
    assert!(matches!(
        out,
        Ok(RegisterOutcome::Registered { keyed: false, .. })
    ));
    let body: Value = serde_json::from_slice(&script.sent()[1].body).unwrap();
    assert_eq!(body["client"], "react_native");
    assert_eq!(body["key_store"], "none");
    assert!(body.get("public_key").is_none() && body.get("nonce").is_none());
}

#[test]
fn secrets_never_show_in_debug_or_display() {
    let (script, secrets, client) = setup(KeyStoreKind::File);
    secrets.set(SESSION_KEY, "SESSION-VALUE-xyz").unwrap();
    script.reply(
        "/csrf/v1/generate",
        200,
        &[],
        json!({"token": "CSRF-TOKEN-xyz"}),
    );
    script.reply(
        "/auth/v1/2fa/setup",
        200,
        &[],
        json!({
            "secret": "TOTPSECRETxyz", "otpauth_url": "otpauth://xyz", "backup_codes": ["BACKUPxyz"]
        }),
    );
    let setup = block_on(client.twofa_setup(None)).unwrap();
    let machine = AuthMachine::new(client.clone(), Rc::new(MemoryKv::new()));
    script.reply(
        "/auth/v1/log_in",
        200,
        &[],
        json!({"status": "totp_required", "totp_token": "TOTPTOKENxyz"}),
    );
    let _ = block_on(machine.log_in("a@b.c", &Secret::new("PASSWORDxyz")));
    let outcome = crate::api::types::SignInOutcome::decode(
        &json!({"status": "totp_required", "totp_token": "TOTPTOKENxyz"}),
    )
    .unwrap();
    let key = crate::device::DeviceKey::generate();
    let error = ApiError::Http(crate::api::HttpError::decode(
        401,
        &json!({"type":"AUTH_001"}),
        None,
    ));
    let shown = [
        format!("{client:?}"),
        format!("{machine:?}"),
        format!("{setup:?}"),
        format!("{outcome:?}"),
        format!("{key:?}"),
        format!("{error:?}"),
        format!("{error}"),
        format!("{:?}", Secret::new("PASSWORDxyz")),
    ]
    .join("\n");
    for secret in [
        "SESSION-VALUE",
        "CSRF-TOKEN",
        "TOTPSECRET",
        "otpauth://",
        "BACKUP",
        "TOTPTOKEN",
        "PASSWORD",
        key.to_secret().expose(),
    ] {
        assert!(!shown.contains(secret), "{secret} leaked:\n{shown}");
    }
}
