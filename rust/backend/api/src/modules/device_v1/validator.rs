use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use validator::{Validate, ValidationError};

use super::keys::{decode_exact, valid_hint, PublicJwk, RAW_SIGNATURE_BYTES};

/// Closed enums. Clients send these exact strings; anything else is refused.
pub const PLATFORMS: &[&str] = &["web", "ios", "android", "macos", "windows", "linux"];
pub const CLIENTS: &[&str] = &["web", "react_native", "gpui", "nsh"];
/// Same values as `reading_sessions.device`.
pub const FORM_FACTORS: &[&str] = &["phone", "tablet", "desktop"];
pub const BROWSERS: &[&str] = &["chrome", "firefox", "safari", "edge", "other"];
/// Where the key lives. Self-reported, a display label only (devices.md D3, rule 13).
pub const KEY_STORES: &[&str] = &["browser", "secure_hardware", "os_keystore", "file", "none"];

pub const NAME_MAX_CHARS: usize = 40;

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct V1RegisterDevicePayload {
    #[validate(range(min = 1))]
    pub expected_user_id: i32,
    #[validate(custom(function = "valid_device_id"))]
    pub device_id: String,
    #[validate(custom(function = "valid_platform"))]
    pub platform: String,
    #[validate(custom(function = "valid_client"))]
    pub client: String,
    #[validate(custom(function = "valid_form_factor"))]
    pub form_factor: String,
    #[validate(custom(function = "valid_browser"))]
    pub browser: Option<String>,
    #[validate(range(min = 1, max = 999))]
    pub os_major: Option<i32>,
    #[validate(length(min = 1, max = 32))]
    pub app_version: Option<String>,
    /// Push token, when the device has notification permission. Optional.
    #[validate(length(min = 1, max = 4096), custom(function = "valid_push_token"))]
    pub token: Option<String>,
    #[validate(custom(function = "valid_key_store"))]
    pub key_store: String,
    /// Public half of the device key. Present exactly when `key_store` is not `none`.
    pub public_key: Option<PublicJwk>,
    /// The session nonce the key signed (`POST /device/v1/challenge`).
    #[validate(length(equal = 43))]
    pub nonce: Option<String>,
    /// Raw `r||s` signature over the proof message, base64url.
    #[validate(length(equal = 86))]
    pub signature: Option<String>,
    /// Per-account continuity hint. Sent only with a key, and compared, never trusted.
    #[validate(length(equal = 43))]
    pub continuity_hint: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct V1RenameDevicePayload {
    #[validate(range(min = 1))]
    pub expected_user_id: i32,
    #[validate(custom(function = "valid_device_id"))]
    pub device_id: String,
    #[validate(length(min = 1, max = 400))]
    pub name: String,
    /// S2 stamp. Highest stamp wins.
    #[validate(length(min = 20, max = 100))]
    pub name_stamp: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct V1DeviceRefPayload {
    #[validate(range(min = 1))]
    pub expected_user_id: i32,
    #[validate(custom(function = "valid_device_id"))]
    pub device_id: String,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct V1DeleteTokenPayload {
    #[validate(range(min = 1))]
    pub expected_user_id: i32,
    #[validate(length(min = 1, max = 4096))]
    pub token: String,
}

/// Client-minted device id: 1 to 64 of `[A-Za-z0-9._-]`, the same rule as the
/// device part of an S2 stamp.
pub fn valid_device_id(id: &str) -> Result<(), ValidationError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_device_id")
            .with_message("device id must be 1 to 64 of [A-Za-z0-9._-]".into()))
    }
}

/// Push token shape (A36). FCM registration tokens are ASCII letters, digits, `:`,
/// `_` and `-`. Whitespace, control bytes, quotes and non-ASCII are refused before
/// the token reaches the device table or the FCM call.
pub fn valid_push_token(token: &str) -> Result<(), ValidationError> {
    let ok = token
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b':' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(ValidationError::new("invalid_push_token")
            .with_message("push token has characters outside [A-Za-z0-9:_-]".into()))
    }
}

#[cfg(test)]
mod push_token_tests {
    use super::valid_push_token;

    #[test]
    fn fcm_shaped_tokens_pass_and_malformed_ones_fail() {
        assert!(valid_push_token("fcm-token-a").is_ok());
        assert!(valid_push_token("dGhpcyBpcyBh_-Zm:APA91bHx").is_ok());
        assert!(valid_push_token("has space").is_err());
        assert!(valid_push_token("line\nbreak").is_err());
        assert!(valid_push_token("quote\"injection").is_err());
        assert!(valid_push_token("tökén").is_err());
        assert!(valid_push_token("nul\0byte").is_err());
    }
}

fn valid_platform(v: &str) -> Result<(), ValidationError> {
    one_of(v, PLATFORMS, "invalid_platform")
}

fn valid_client(v: &str) -> Result<(), ValidationError> {
    one_of(v, CLIENTS, "invalid_client")
}

fn valid_form_factor(v: &str) -> Result<(), ValidationError> {
    one_of(v, FORM_FACTORS, "invalid_form_factor")
}

fn valid_browser(v: &str) -> Result<(), ValidationError> {
    one_of(v, BROWSERS, "invalid_browser")
}

fn valid_key_store(v: &str) -> Result<(), ValidationError> {
    one_of(v, KEY_STORES, "invalid_key_store")
}

fn one_of(value: &str, allowed: &[&str], code: &'static str) -> Result<(), ValidationError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(ValidationError::new(code)
            .with_message(format!("must be one of: {}", allowed.join(", ")).into()))
    }
}

/// Cross-field rules a per-field validator cannot see. The client decides the
/// platform family, and only a web client carries a browser.
pub fn check_registration_shape(p: &V1RegisterDevicePayload) -> Result<(), &'static str> {
    let platform_ok = match p.client.as_str() {
        "web" => p.platform == "web",
        "react_native" => matches!(p.platform.as_str(), "ios" | "android"),
        "gpui" => matches!(p.platform.as_str(), "macos" | "windows" | "linux"),
        "nsh" => matches!(
            p.platform.as_str(),
            "ios" | "android" | "macos" | "windows" | "linux"
        ),
        _ => false,
    };
    if !platform_ok {
        return Err("platform does not match client");
    }
    let browser_ok = (p.client == "web") == p.browser.is_some();
    if !browser_ok {
        return Err("browser is only sent by the web client");
    }
    check_key_shape(p)
}

/// Store label per client, and the key fields that must travel together.
fn check_key_shape(p: &V1RegisterDevicePayload) -> Result<(), &'static str> {
    let store_ok = match p.client.as_str() {
        "web" => matches!(p.key_store.as_str(), "browser" | "none"),
        "react_native" => matches!(p.key_store.as_str(), "secure_hardware" | "none"),
        "gpui" => matches!(p.key_store.as_str(), "os_keystore" | "file" | "none"),
        "nsh" => matches!(
            p.key_store.as_str(),
            "secure_hardware" | "os_keystore" | "file" | "none"
        ),
        _ => false,
    };
    if !store_ok {
        return Err("key_store does not match client");
    }
    let has_key = p.public_key.is_some();
    if (p.key_store == "none") == has_key {
        return Err("key_store and public_key disagree");
    }
    if !has_key {
        // Without a key nothing is proven, so no proof fields may ride along.
        return if p.nonce.is_none() && p.signature.is_none() {
            Ok(())
        } else {
            Err("nonce and signature need a public_key")
        };
    }
    let (Some(nonce), Some(signature)) = (p.nonce.as_deref(), p.signature.as_deref()) else {
        return Err("a public_key needs a nonce and a signature");
    };
    if decode_exact(nonce, 32).is_none() {
        return Err("nonce is not 32 bytes of base64url");
    }
    if decode_exact(signature, RAW_SIGNATURE_BYTES).is_none() {
        return Err("signature must be 64 raw bytes of base64url");
    }
    // A hint without a key is ignored, so a client with a broken keystore still
    // registers. A hint with a key must be well formed.
    match p.continuity_hint.as_deref() {
        Some(hint) if !valid_hint(hint) => Err("continuity_hint must be 32 bytes of base64url"),
        _ => Ok(()),
    }
}

/// Display name: NFC, trimmed, 1 to 40 characters. No control characters, no
/// line or paragraph separators, no bidi controls, so the name renders as the
/// plain text it is.
pub fn normalise_device_name(raw: &str) -> Result<String, &'static str> {
    let name: String = raw.nfc().collect::<String>().trim().to_string();
    let len = name.chars().count();
    if len == 0 || len > NAME_MAX_CHARS {
        return Err("name must be 1 to 40 characters");
    }
    if name.chars().any(is_unsafe_name_char) {
        return Err("name contains control or bidi characters");
    }
    Ok(name)
}

fn is_unsafe_name_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(client: &str, platform: &str, browser: Option<&str>) -> V1RegisterDevicePayload {
        V1RegisterDevicePayload {
            expected_user_id: 1,
            device_id: "dev-1".into(),
            platform: platform.into(),
            client: client.into(),
            form_factor: "desktop".into(),
            browser: browser.map(str::to_string),
            os_major: None,
            app_version: None,
            token: None,
            key_store: "none".into(),
            public_key: None,
            nonce: None,
            signature: None,
            continuity_hint: None,
        }
    }

    fn keyed(client: &str, platform: &str, store: &str) -> V1RegisterDevicePayload {
        let mut p = reg(client, platform, None);
        p.key_store = store.into();
        p.public_key = Some(PublicJwk {
            kty: "EC".into(),
            crv: "P-256".into(),
            x: "A".repeat(43),
            y: "A".repeat(43),
        });
        p.nonce = Some("A".repeat(43));
        p.signature = Some("A".repeat(86));
        p
    }

    #[test]
    fn key_store_is_closed_and_fits_the_client() {
        assert!(check_registration_shape(&keyed("react_native", "ios", "secure_hardware")).is_ok());
        assert!(check_registration_shape(&keyed("gpui", "linux", "file")).is_ok());
        assert!(check_registration_shape(&keyed("react_native", "ios", "browser")).is_err());
        assert!(check_registration_shape(&keyed("gpui", "linux", "secure_hardware")).is_err());
        assert!(valid_key_store("keychain").is_err());
    }

    #[test]
    fn a_public_key_and_its_store_travel_together() {
        let mut p = keyed("gpui", "linux", "os_keystore");
        p.key_store = "none".into();
        assert!(check_registration_shape(&p).is_err());

        let mut none_with_proof = reg("gpui", "linux", None);
        none_with_proof.nonce = Some("A".repeat(43));
        assert!(check_registration_shape(&none_with_proof).is_err());

        let mut no_signature = keyed("gpui", "linux", "os_keystore");
        no_signature.signature = None;
        assert!(check_registration_shape(&no_signature).is_err());
    }

    #[test]
    fn proof_fields_have_exact_wire_lengths() {
        let mut short_nonce = keyed("gpui", "linux", "os_keystore");
        short_nonce.nonce = Some("A".repeat(42));
        assert!(check_registration_shape(&short_nonce).is_err());

        let mut der_sized = keyed("gpui", "linux", "os_keystore");
        der_sized.signature = Some("A".repeat(84));
        assert!(check_registration_shape(&der_sized).is_err());
    }

    #[test]
    fn a_hint_needs_a_well_formed_value_only_with_a_key() {
        let mut bad = keyed("gpui", "linux", "os_keystore");
        bad.continuity_hint = Some("A".repeat(43));
        assert!(check_registration_shape(&bad).is_ok());
        bad.continuity_hint = Some("+".repeat(43));
        assert!(check_registration_shape(&bad).is_err());

        let mut ignored = reg("gpui", "linux", None);
        ignored.continuity_hint = Some("+".repeat(43));
        assert!(check_registration_shape(&ignored).is_ok());
    }

    #[test]
    fn device_id_rule_matches_stamp_device_part() {
        assert!(valid_device_id("0190f2c4-7b1a-7c3d-9e4f-0123456789ab").is_ok());
        assert!(valid_device_id("").is_err());
        assert!(valid_device_id(&"a".repeat(65)).is_err());
        assert!(valid_device_id("has space").is_err());
        assert!(valid_device_id("slash/x").is_err());
    }

    #[test]
    fn shape_pairs_client_with_platform_and_browser() {
        assert!(check_registration_shape(&reg("web", "web", Some("chrome"))).is_ok());
        assert!(check_registration_shape(&reg("web", "web", None)).is_err());
        assert!(check_registration_shape(&reg("web", "ios", Some("chrome"))).is_err());
        assert!(check_registration_shape(&reg("react_native", "ios", None)).is_ok());
        assert!(check_registration_shape(&reg("react_native", "web", None)).is_err());
        assert!(check_registration_shape(&reg("react_native", "ios", Some("safari"))).is_err());
        assert!(check_registration_shape(&reg("gpui", "linux", None)).is_ok());
        assert!(check_registration_shape(&reg("gpui", "ios", None)).is_err());
    }

    #[test]
    fn nsh_spans_mobile_and_desktop() {
        for platform in ["ios", "android", "macos", "windows", "linux"] {
            assert!(check_registration_shape(&reg("nsh", platform, None)).is_ok());
        }
        assert!(check_registration_shape(&reg("nsh", "web", None)).is_err());
        assert!(check_registration_shape(&reg("nsh", "ios", Some("safari"))).is_err());
        assert!(check_registration_shape(&keyed("nsh", "ios", "secure_hardware")).is_ok());
        assert!(check_registration_shape(&keyed("nsh", "android", "secure_hardware")).is_ok());
        assert!(check_registration_shape(&keyed("nsh", "macos", "os_keystore")).is_ok());
        assert!(check_registration_shape(&keyed("nsh", "linux", "file")).is_ok());
        assert!(check_registration_shape(&keyed("nsh", "ios", "browser")).is_err());
        assert!(check_registration_shape(&keyed("nsh", "ios", "none")).is_err());
        assert!(valid_client("nsh").is_ok());
    }

    #[test]
    fn names_are_nfc_trimmed_and_bounded() {
        assert_eq!(
            normalise_device_name("  Ali's phone  ").unwrap(),
            "Ali's phone"
        );
        // e + combining acute composes to a single char under NFC.
        assert_eq!(normalise_device_name("e\u{0301}").unwrap(), "\u{00E9}");
        assert!(normalise_device_name("   ").is_err());
        assert!(normalise_device_name(&"x".repeat(41)).is_err());
        assert!(normalise_device_name(&"x".repeat(40)).is_ok());
    }

    #[test]
    fn names_refuse_controls_and_bidi_overrides() {
        assert!(normalise_device_name("a\nb").is_err());
        assert!(normalise_device_name("a\u{202E}b").is_err());
        assert!(normalise_device_name("a\u{2067}b").is_err());
        assert!(normalise_device_name("a\u{200F}b").is_err());
        assert!(normalise_device_name("a\u{2028}b").is_err());
    }

    #[test]
    fn unknown_fields_are_refused() {
        let raw = r#"{"expected_user_id":1,"device_id":"d","platform":"web","client":"web",
            "form_factor":"desktop","browser":"chrome","user_agent":"Mozilla/5.0"}"#;
        assert!(serde_json::from_str::<V1RegisterDevicePayload>(raw).is_err());
    }
}
