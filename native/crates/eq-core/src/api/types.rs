//! Wire types of the endpoints the web app calls. Field names are the
//! backend's exactly (snake_case unless the backend says camelCase). Every
//! secret field is a [`Secret`], so `Debug` of any type here prints none.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ApiError, Secret};

/// The signed-in account (`decodeUserProfile`). Built from the backend's
/// user JSON, which may grow fields: unknown ones are ignored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserProfile {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub avatar_id: Option<i32>,
    pub is_verified: bool,
    /// Self-declared under 18 (A34).
    pub is_minor: bool,
    /// The account recorded its age (`age_declared_at` is set).
    pub age_declared: bool,
    pub role: String,
    pub two_fa_enabled: bool,
    pub oauth_provider: Option<String>,
}

impl UserProfile {
    /// Decodes the backend user JSON. `None` without a numeric `id` and a
    /// string `email`, as on web.
    pub fn decode(raw: &Value) -> Option<Self> {
        let o = raw.as_object()?;
        let id = i32::try_from(o.get("id")?.as_i64()?).ok()?;
        let email = o.get("email")?.as_str()?.to_owned();
        let string = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_owned);
        let flag = |k: &str| o.get(k).and_then(Value::as_bool).unwrap_or(false);
        let role = match o.get("role") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => "user".to_owned(),
        };
        Some(Self {
            id,
            name: string("name").unwrap_or_default(),
            email,
            avatar_id: o
                .get("avatar_id")
                .and_then(Value::as_i64)
                .and_then(|v| i32::try_from(v).ok()),
            is_verified: flag("is_verified"),
            is_minor: o.get("is_minor").and_then(Value::as_bool) == Some(true),
            age_declared: o.get("age_declared_at").is_some_and(Value::is_string),
            role,
            two_fa_enabled: flag("two_fa_enabled"),
            oauth_provider: string("oauth_provider"),
        })
    }

    pub(crate) fn decode_or_err(raw: &Value) -> Result<Self, ApiError> {
        Self::decode(raw).ok_or_else(|| ApiError::Decode("user profile".into()))
    }
}

/// How a sign-in answered. Every sign-in path (password, TOTP, passkey,
/// provider token) may answer `pending_login`: a TOTP step comes first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInOutcome {
    /// Signed in; the session was rotated.
    SignedIn(UserProfile),
    /// 2FA is on: send the code with this token (`/auth/v1/login/totp`).
    TotpRequired(Secret),
}

impl SignInOutcome {
    /// `{"status":"totp_required","totp_token"}`, else the user from
    /// `body.user` (passkey, providers) or the body itself (password).
    pub fn decode(body: &Value) -> Result<Self, ApiError> {
        if let Some(token) = pending_totp_token(body) {
            return Ok(Self::TotpRequired(token));
        }
        let user = body.get("user").unwrap_or(body);
        UserProfile::decode_or_err(user).map(Self::SignedIn)
    }
}

/// The TOTP token of a `totp_required` body.
pub fn pending_totp_token(body: &Value) -> Option<Secret> {
    if body.get("status").and_then(Value::as_str) != Some("totp_required") {
        return None;
    }
    body.get("totp_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(Secret::from)
}

/// `POST /auth/v1/register`.
#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: Secret,
    /// `YYYY-MM-DD`.
    pub date_of_birth: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turnstile_token: Option<Secret>,
}

/// `POST /auth/v1/2fa/setup`: show once, never store.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TwoFaSetup {
    pub secret: Secret,
    pub otpauth_url: Secret,
    pub backup_codes: Vec<Secret>,
}

/// `GET /auth/v1/reauth/status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct FreshAuthStatus {
    pub ready: bool,
    pub has_password: bool,
    pub two_fa_enabled: bool,
}

/// One row of `GET /auth/v1/sessions/list`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionInfo {
    /// The audit row id, the terminate id.
    pub id: i64,
    #[serde(rename = "isCurrent", default)]
    pub is_current: bool,
    /// The user agent the session signed in with.
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub ip_address: Option<String>,
    #[serde(default)]
    pub last_seen: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    /// The registered device this session belongs to, if any.
    #[serde(default)]
    pub device_id: Option<String>,
}

/// `POST /user/v1/update`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProfileUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<Secret>,
}

/// The reply of `POST /user/v1/update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileUpdate {
    pub user: UserProfile,
    /// An email change waits for its confirmation code.
    pub email_change_pending: bool,
}

/// The reply of `POST /user/v1/delete` (erasure receipt).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct AccountDeletion {
    pub receipt_id: Value,
    #[serde(default)]
    pub user_row_erased: bool,
}

/// One linked sign-in provider.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LinkedIdentity {
    pub provider: String,
    pub provider_user_id: String,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// `GET /user/v1/identities`, also the reply of `identities/unlink`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IdentitySummary {
    pub identities: Vec<LinkedIdentity>,
    pub has_password: bool,
    pub passkey_count: u64,
    pub can_unlink_any: bool,
    #[serde(default)]
    pub linkable_providers: Vec<String>,
}

/// `POST /passkey/v1/{login,register}/begin`: the WebAuthn options for the
/// platform authenticator and the handle to finish with.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PasskeyChallenge {
    pub challenge: Value,
    pub state_handle: Secret,
}

/// One passkey of `POST /passkey/v1/list`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PasskeyView {
    pub id: Value,
    pub credential_id: String,
    #[serde(default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub transports: Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
}

/// Sign-in providers with a native token endpoint (`/auth/{p}/v1/token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OAuthProvider {
    Google,
    Apple,
    Facebook,
    Github,
    Discord,
}

impl OAuthProvider {
    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::Apple => "apple",
            OAuthProvider::Facebook => "facebook",
            OAuthProvider::Github => "github",
            OAuthProvider::Discord => "discord",
        }
    }

    /// Google and Apple id tokens carry a server nonce
    /// (`/auth/{p}/v1/token/nonce`) first.
    pub fn has_token_nonce(self) -> bool {
        matches!(self, OAuthProvider::Google | OAuthProvider::Apple)
    }
}

/// What a provider SDK (or the desktop PKCE flow) hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderToken {
    /// Google: the id token, and an access token when the SDK gives one.
    Google {
        id_token: Secret,
        access_token: Option<Secret>,
    },
    /// Apple: the identity token.
    Apple { identity_token: Secret },
    /// Facebook, GitHub, Discord: the access token.
    Access(Secret),
}

impl ProviderToken {
    pub(crate) fn body(&self) -> Value {
        match self {
            ProviderToken::Google {
                id_token,
                access_token,
            } => {
                let mut body = serde_json::json!({ "id_token": id_token.expose() });
                if let Some(access) = access_token {
                    body["access_token"] = Value::from(access.expose());
                }
                body
            }
            ProviderToken::Apple { identity_token } => {
                serde_json::json!({ "identity_token": identity_token.expose() })
            }
            ProviderToken::Access(token) => serde_json::json!({ "access_token": token.expose() }),
        }
    }
}

/// `POST /auth/{google,apple}/v1/token/nonce`: `provider_nonce` goes into
/// the provider request; the id token carries it back.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NativeTokenNonce {
    pub nonce: String,
    #[serde(rename = "providerNonce")]
    pub provider_nonce: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: u64,
}

/// `POST /device/v1/challenge`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceChallenge {
    /// Single use, bound to this session, five minutes.
    pub nonce: String,
    /// Per account, never rotated: the continuity hint key.
    pub salt: Secret,
}

/// Public half of a device key: exactly `kty`, `crv`, `x`, `y`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicJwk {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: String,
}

/// `POST /device/v1/register` (`V1RegisterDevicePayload`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceRegistration {
    pub expected_user_id: i32,
    pub device_id: String,
    pub platform: String,
    pub client: String,
    pub form_factor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_major: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    pub key_store: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<PublicJwk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuity_hint: Option<String>,
}

/// The register reply: keep `device_id`, which may differ from the one sent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceRegistered {
    pub registered: bool,
    pub device_id: String,
}

/// One device of `POST /device/v1/list`. Names are user text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub display_name: Option<String>,
    pub platform: String,
    pub client: String,
    pub form_factor: String,
    pub browser: Option<String>,
    pub os_major: Option<i32>,
    pub app_version: Option<String>,
    pub last_seen_at: String,
    pub is_current: bool,
}

/// A stored goal reminder (`notification/v1/reminder/*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReminderSchedule {
    pub kind: String,
    pub enabled: bool,
    pub time_zone: String,
    pub reminder_minute: i32,
    pub quiet_start_minute: Option<i32>,
    pub quiet_end_minute: Option<i32>,
    #[serde(default, skip_serializing)]
    pub updated_at: Option<String>,
}

/// The only reminder kind (`ReminderKind::Goal`).
pub const REMINDER_KIND_GOAL: &str = "goal";

/// `POST /report/v1/submit` (`buildReportPayload` output).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportBody {
    /// `post`, `comment` or `translation`.
    pub target_kind: String,
    pub target_id: String,
    /// `spam`, `harassment`, `hate`, `sexual`, `violence`, `copyright`,
    /// `csam`, `threat` or `other`.
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
}

/// One reading event of `POST /user/analytics/v1/events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingEvent {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub surah: i32,
    pub ayah: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    pub ms: i64,
    /// `YYYY-MM-DD` in the reader's zone.
    pub local_day: String,
    /// RFC 3339 with offset.
    pub at: String,
}

/// One day of reading totals.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayTotal {
    pub local_day: String,
    pub ayahs_read: i64,
    pub ms: i64,
}

/// `GET /user/analytics/v1/summary` totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct AnalyticsTotals {
    pub ayahs: i64,
    pub ms: i64,
    pub days: i64,
}

/// A most-read surah.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct SurahTotal {
    pub surah: i64,
    pub reads: i64,
    pub ms: i64,
}

/// A most-read ayah.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct AyahTotal {
    pub surah: i64,
    pub ayah: i64,
    pub reads: i64,
    pub ms: i64,
}

/// `GET /user/analytics/v1/summary`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub totals: AnalyticsTotals,
    pub top_surahs: Vec<SurahTotal>,
    pub top_ayahs: Vec<AyahTotal>,
    pub days: Vec<DayTotal>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn user_profile_mirrors_the_web_decoder() {
        let user = UserProfile::decode(&json!({
            "id": 7, "email": "a@b.c", "is_verified": true, "role": "user",
            "age_declared_at": "2026-01-01T00:00:00Z", "two_fa_enabled": false
        }))
        .unwrap();
        assert!(user.age_declared && user.is_verified && !user.is_minor);
        assert_eq!(user.name, "");
        assert!(UserProfile::decode(&json!({"email": "x"})).is_none());
    }

    #[test]
    fn sign_in_outcomes_cover_every_path() {
        let pending = SignInOutcome::decode(&json!({"status":"totp_required","totp_token":"t"}));
        assert!(matches!(pending, Ok(SignInOutcome::TotpRequired(_))));
        let passkey = SignInOutcome::decode(&json!({"status":"ok","user":{"id":1,"email":"e"}}));
        assert!(matches!(passkey, Ok(SignInOutcome::SignedIn(u)) if u.id == 1));
        let password = SignInOutcome::decode(&json!({"id":2,"email":"e"}));
        assert!(matches!(password, Ok(SignInOutcome::SignedIn(u)) if u.id == 2));
    }
}
