use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use validator::{Validate, ValidationError};

/// Closed enums. Clients send these exact strings; anything else is refused.
pub const PLATFORMS: &[&str] = &["web", "ios", "android", "macos", "windows", "linux"];
pub const CLIENTS: &[&str] = &["web", "react_native", "gpui"];
/// Same values as `reading_sessions.device`.
pub const FORM_FACTORS: &[&str] = &["phone", "tablet", "desktop"];
pub const BROWSERS: &[&str] = &["chrome", "firefox", "safari", "edge", "other"];

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
    #[validate(length(min = 1, max = 4096))]
    pub token: Option<String>,
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
        _ => false,
    };
    if !platform_ok {
        return Err("platform does not match client");
    }
    let browser_ok = (p.client == "web") == p.browser.is_some();
    if browser_ok {
        Ok(())
    } else {
        Err("browser is only sent by the web client")
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
        }
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
