use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::{get, post},
    Router,
};
use tower::ServiceExt;

use ruxlog::middlewares::security_headers::security_headers;
use ruxlog::middlewares::static_csrf::csrf_guard;

async fn ok_handler() -> StatusCode {
    StatusCode::OK
}

fn security_headers_router() -> Router {
    Router::new()
        .route("/test", get(ok_handler))
        .layer(middleware::from_fn(security_headers))
}

#[tokio::test]
async fn security_headers_present_on_response() {
    let app = security_headers_router();
    let response = app
        .oneshot(Request::builder().uri("/test").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let headers = response.headers();
    assert_eq!(
        headers
            .get("x-content-type-options")
            .map(|v| v.to_str().unwrap()),
        Some("nosniff")
    );
    assert_eq!(
        headers.get("x-frame-options").map(|v| v.to_str().unwrap()),
        Some("DENY")
    );
    assert_eq!(
        headers.get("referrer-policy").map(|v| v.to_str().unwrap()),
        Some("strict-origin-when-cross-origin")
    );
    assert_eq!(
        headers
            .get("permissions-policy")
            .map(|v| v.to_str().unwrap()),
        Some("camera=(), microphone=(), geolocation=()")
    );
    assert_eq!(
        headers.get("x-xss-protection").map(|v| v.to_str().unwrap()),
        Some("0")
    );
}

fn ensure_test_cookie_key() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::env::set_var("COOKIE_KEY", "test-cookie-key-not-for-production-use-32+");
    });
}

fn csrf_router() -> Router {
    use tower_sessions::{MemoryStore, SessionManagerLayer};
    ensure_test_cookie_key();
    let store = MemoryStore::default();
    Router::new()
        .route("/protected", post(ok_handler))
        .route(
            "/csrf/v1/generate",
            post(ruxlog::modules::csrf_v1::controller::generate),
        )
        .layer(middleware::from_fn(csrf_guard))
        .layer(SessionManagerLayer::new(store))
}

async fn csrf_bootstrap(app: &Router) -> (String, String, String) {
    use axum::body::to_bytes;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/csrf/v1/generate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let set_cookie = res
        .headers()
        .get("set-cookie")
        .expect("generate sets a session cookie")
        .to_str()
        .unwrap()
        .to_string();
    let pair = set_cookie.split(';').next().unwrap_or(&set_cookie).trim();
    let name = pair.split('=').next().unwrap_or("").to_string();
    let value = pair.split('=').nth(1).unwrap_or("").to_string();

    let bytes = to_bytes(res.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let token = json["token"].as_str().unwrap().to_string();
    (token, name, value)
}

#[tokio::test]
async fn csrf_rejects_missing_token() {
    let app = csrf_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protected")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_rejects_invalid_token() {
    let app = csrf_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protected")
                .header("csrf-token", "invalid-token-value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_rejects_token_bound_to_a_different_session() {
    let app = csrf_router();
    let (token_a, _name_a, _value_a) = csrf_bootstrap(&app).await;
    let (_token_b, name_b, value_b) = csrf_bootstrap(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protected")
                .header("csrf-token", &token_a)
                .header("cookie", format!("{name_b}={value_b}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_accepts_valid_token() {
    let app = csrf_router();
    let (token, name, value) = csrf_bootstrap(&app).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protected")
                .header("csrf-token", &token)
                .header("cookie", format!("{name}={value}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn csrf_exempt_bootstrap_path_needs_no_token() {
    let app = csrf_router();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/csrf/v1/generate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn empty_string_not_accepted_as_totp_code() {
    use ruxlog::utils::twofa;

    let secret = twofa::generate_secret_base32(20).expect("CSPRNG available");
    assert!(twofa::verify_totp_code_at(
        &secret,
        "",
        chrono::Utc::now().fixed_offset(),
        twofa::DEFAULT_TOTP_STEP,
        twofa::DEFAULT_TOTP_DIGITS,
        1,
    )
    .is_none());
}

#[test]
fn non_numeric_totp_code_rejected() {
    use ruxlog::utils::twofa;

    let secret = twofa::generate_secret_base32(20).expect("CSPRNG available");
    assert!(twofa::verify_totp_code_at(
        &secret,
        "abcdef",
        chrono::Utc::now().fixed_offset(),
        twofa::DEFAULT_TOTP_STEP,
        twofa::DEFAULT_TOTP_DIGITS,
        1,
    )
    .is_none());
}

#[test]
fn wrong_length_totp_code_rejected() {
    use ruxlog::utils::twofa;

    let secret = twofa::generate_secret_base32(20).expect("CSPRNG available");
    assert!(twofa::verify_totp_code_at(
        &secret,
        "12345",
        chrono::Utc::now().fixed_offset(),
        twofa::DEFAULT_TOTP_STEP,
        twofa::DEFAULT_TOTP_DIGITS,
        1,
    )
    .is_none());
}

#[test]
fn backup_code_constant_time_compare() {
    use ruxlog::utils::twofa;

    let codes = vec!["ABCD-EFGH-JKLM".to_string()];
    let hashes = twofa::hash_backup_codes(&codes);

    assert!(twofa::consume_backup_code(&hashes, "ABCD-EFGH-JKLN").is_none());
    assert!(twofa::consume_backup_code(&hashes, "ABCD-EFGH-JKLM").is_some());
}

#[test]
fn user_json_never_leaks_secret_fields() {
    use ruxlog::db::sea_models::user::{self, UserRole};

    let now = chrono::Utc::now().fixed_offset();
    let model = user::Model {
        id: 1,
        name: "Test".into(),
        email: "test@example.com".into(),
        password: Some("dummy-argon2-hash".into()),
        avatar_id: None,
        is_verified: true,
        role: UserRole::User,
        two_fa_enabled: true,
        two_fa_secret: Some("JBSWY3DPEHPK3PXP".into()),
        two_fa_backup_codes: Some(serde_json::json!(["$argon2id$dummy"])),
        two_fa_last_totp_counter: None,
        google_id: None,
        oauth_provider: None,
        session_auth_secret: "test-secret".into(),
        created_at: now,
        updated_at: now,
    };

    let json = serde_json::to_value(&model).expect("user model must serialize");
    let obj = json.as_object().expect("serialized user is an object");

    for secret_field in ["password", "two_fa_secret", "two_fa_backup_codes"] {
        assert!(
            !obj.contains_key(secret_field),
            "{secret_field} leaked into serialized user JSON: {json}"
        );
    }

    assert_eq!(
        obj.get("email").and_then(|v| v.as_str()),
        Some("test@example.com")
    );
}

#[test]
fn error_codes_distinct_status_for_auth_vs_db() {
    use ruxlog::error::codes::ErrorCode;

    let auth_401 = vec![
        ErrorCode::InvalidCredentials,
        ErrorCode::SessionExpired,
        ErrorCode::InvalidToken,
    ];
    for code in &auth_401 {
        assert_eq!(code.status_code(), axum::http::StatusCode::UNAUTHORIZED);
    }

    let auth_403 = vec![
        ErrorCode::Unauthorized,
        ErrorCode::AccountLocked,
        ErrorCode::EmailVerificationRequired,
    ];
    for code in &auth_403 {
        assert_eq!(code.status_code(), axum::http::StatusCode::FORBIDDEN);
    }

    assert_eq!(
        ErrorCode::RecordNotFound.status_code(),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        ErrorCode::DuplicateEntry.status_code(),
        axum::http::StatusCode::CONFLICT
    );
    assert_eq!(
        ErrorCode::DatabaseConnectionError.status_code(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn auth_path_logs_contain_no_sensitive_values() {
    // W8f invariant: auth-path logs emit only opaque user id, provider NAME,
    // result, trace id, client_ip — NEVER email, OTP/verification code,
    // recipient address, or subject. Source-level guard against reintroduction.
    let email_verify = include_str!("../src/modules/email_verification_v1/controller.rs");
    let forgot_pw = include_str!("../src/modules/forgot_password_v1/controller.rs");
    let mail_none = include_str!("../src/services/mail/none.rs");
    let user_actions = include_str!("../src/db/sea_models/user/actions.rs");

    assert!(
        !email_verify.contains("code = %payload.code"),
        "email_verification_v1 controller must not log the OTP (code = %payload.code)"
    );

    assert!(
        !forgot_pw.contains("email = %"),
        "forgot_password_v1 controller must not log the recovery email address"
    );

    assert!(
        !mail_none.contains("recipient = %msg.to"),
        "noop mail provider must not log the recipient address"
    );
    assert!(
        !mail_none.contains("subject = %msg.subject"),
        "noop mail provider must not log the email subject"
    );

    assert!(
        !user_actions.contains("email = %"),
        "user actions must not log email (create/find_by_email/create_from_google/admin_create)"
    );

    // --- W8f step 6: broaden the source-level guard across every auth/mail path ---

    // auth_v1/controller.rs — login, register, 2FA. email/password/code are
    // payload fields that must never reach tracing output (only user_id/ip/result).
    let auth_v1 = include_str!("../src/modules/auth_v1/controller.rs");
    assert!(
        !auth_v1.contains("email = %payload"),
        "auth_v1 controller must not log the login/register email (email = %payload.*)"
    );
    assert!(
        !auth_v1.contains("password = %"),
        "auth_v1 controller must not log the password"
    );
    assert!(
        !auth_v1.contains("code = %payload"),
        "auth_v1 controller must not log the 2FA/backup code (code = %payload.*)"
    );

    // OAuth provider controllers. `code = %err.code` is the ErrorCode enum
    // (legitimate, opaque) — never confuse it with the authorization/OTP code,
    // so scope to `code = %payload` and `email = %<source>`.
    let google = include_str!("../src/modules/google_auth_v1/controller.rs");
    assert!(
        !google.contains("email = %claims"),
        "google_auth_v1 must not log the id_token claims email"
    );
    assert!(
        !google.contains("email = %user_info"),
        "google_auth_v1 must not log the userinfo email"
    );
    assert!(
        !google.contains("code = %payload"),
        "google_auth_v1 must not log the OAuth authorization code"
    );
    assert!(
        !google.contains("password = %"),
        "google_auth_v1 must not log credentials"
    );

    let apple = include_str!("../src/modules/apple_auth_v1/controller.rs");
    assert!(
        !apple.contains("email = %claims"),
        "apple_auth_v1 must not log the id_token claims email"
    );
    assert!(
        !apple.contains("code = %payload"),
        "apple_auth_v1 must not log the OAuth authorization code"
    );
    assert!(
        !apple.contains("password = %"),
        "apple_auth_v1 must not log credentials"
    );

    let facebook = include_str!("../src/modules/facebook_auth_v1/controller.rs");
    assert!(
        !facebook.contains("email = %user_info"),
        "facebook_auth_v1 must not log the Graph API email"
    );
    assert!(
        !facebook.contains("code = %payload"),
        "facebook_auth_v1 must not log the OAuth authorization code"
    );
    assert!(
        !facebook.contains("password = %"),
        "facebook_auth_v1 must not log credentials"
    );

    let github = include_str!("../src/modules/github_auth_v1/controller.rs");
    assert!(
        !github.contains("email = %"),
        "github_auth_v1 must not log the GitHub email"
    );
    assert!(
        !github.contains("code = %payload"),
        "github_auth_v1 must not log the OAuth authorization code"
    );
    assert!(
        !github.contains("password = %"),
        "github_auth_v1 must not log credentials"
    );

    // services/oauth/login.rs — find_or_create_user_for_oauth receives the IdP
    // email/name; only user_id + provider NAME may appear.
    let oauth_login = include_str!("../src/services/oauth/login.rs");
    assert!(
        !oauth_login.contains("email = %"),
        "oauth::login must not log the IdP email"
    );
    assert!(
        !oauth_login.contains("code = %"),
        "oauth::login must not log any code value"
    );
    assert!(
        !oauth_login.contains("password = %"),
        "oauth::login must not log credentials"
    );

    // Mail providers — recipient/subject/to are PII and must never be logged;
    // only opaque domain + provider name + redacted rate key are permitted.
    let smtp = include_str!("../src/services/mail/smtp.rs");
    assert!(
        !smtp.contains("recipient = %msg"),
        "smtp provider must not log the recipient address"
    );
    assert!(
        !smtp.contains("subject = %msg"),
        "smtp provider must not log the email subject"
    );
    assert!(
        !smtp.contains("to = %msg"),
        "smtp provider must not log the recipient (msg.to)"
    );
    assert!(
        !smtp.contains("password = %"),
        "smtp provider must not log the SMTP password"
    );

    let cloudflare = include_str!("../src/services/mail/cloudflare.rs");
    assert!(
        !cloudflare.contains("recipient = %"),
        "cloudflare provider must not log the recipient address"
    );
    assert!(
        !cloudflare.contains("subject = %msg"),
        "cloudflare provider must not log the email subject"
    );
    assert!(
        !cloudflare.contains("to = %msg"),
        "cloudflare provider must not log the recipient (msg.to)"
    );
    assert!(
        !cloudflare.contains("email = %"),
        "cloudflare provider must not log an email address"
    );

    let mail_router = include_str!("../src/services/mail/router.rs");
    assert!(
        !mail_router.contains("recipient = %msg"),
        "mail router must not log the full recipient address (domain-only is allowed)"
    );
    assert!(
        !mail_router.contains("subject = %msg"),
        "mail router must not log the email subject"
    );
    assert!(
        !mail_router.contains("email = %msg"),
        "mail router must not log the recipient email"
    );
    assert!(
        !mail_router.contains("to = %msg"),
        "mail router must not log msg.to"
    );
}

#[tokio::test]
async fn tracing_capture_asserts_no_pii_in_mail_drop_path() {
    // W8f step 6: tracing-capture guard. Drives a representative mail path
    // (NoOpMailProvider::send — the MAIL_PROVIDER=none drop/failure path) under
    // a capturing fmt subscriber and asserts no recipient/code/subject VALUE
    // appears in the captured event fields. The provider is handed a message
    // whose `to`/`subject` carry real-looking PII; the invariant is that those
    // values never reach tracing output.
    //
    // login / forgot-password-generate need a full AppState + DB harness
    // (integration scope) and are covered by the source-level guard above; this
    // capture test is the runtime check for the callable mail-failure path.
    use ruxlog::services::mail::none::NoOpMailProvider;
    use ruxlog::services::mail::{MailProvider, OutboundEmail};
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    #[derive(Clone)]
    struct CaptureMaker(Arc<Mutex<Vec<u8>>>);
    impl<'a> MakeWriter<'a> for CaptureMaker {
        type Writer = CaptureBuf;
        fn make_writer(&'a self) -> Self::Writer {
            CaptureBuf(self.0.clone())
        }
    }
    struct CaptureBuf(Arc<Mutex<Vec<u8>>>);
    impl Write for CaptureBuf {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let subscriber = tracing_subscriber::fmt()
        .with_writer(CaptureMaker(buf.clone()))
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);

    let pii_email = "victim@example.com";
    let pii_code = "123456";
    let msg = OutboundEmail {
        to: pii_email.to_string(),
        subject: format!("Reset code {pii_code}"),
        html: None,
        text: None,
        template: None,
    };
    let provider = NoOpMailProvider;
    let _ = MailProvider::send(&provider, msg).await;

    drop(guard);
    let captured = String::from_utf8(buf.lock().unwrap().clone()).expect("utf8");

    // Capture must have actually happened — else the negative asserts are vacuous.
    assert!(
        captured.contains("mail dropped"),
        "captured tracing output should include the noop-drop event; got: {captured}"
    );
    assert!(
        !captured.contains(pii_email),
        "recipient address leaked into noop mailer tracing output: {captured}"
    );
    assert!(
        !captured.contains(pii_code),
        "verification/reset code leaked into noop mailer tracing output: {captured}"
    );
}
