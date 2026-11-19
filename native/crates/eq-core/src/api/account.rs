//! Account page logic with no UI (PARITY I4, I7; web
//! `routes/(account)/account/account-view.ts`, `lib/auth/account-client.ts`):
//! identity providers and the last-method rule, device and session labels
//! from fixed tables (nothing from the wire is echoed as a label), the
//! device-name draft rule and the rename stamp.

use super::types::{DeviceView, IdentitySummary, SessionInfo};
use crate::store::{AppStore, StoreError};

/// The providers `user/v1/identities` lists and links (`IdentityProvider`).
pub const IDENTITY_PROVIDERS: [&str; 3] = ["github", "discord", "google"];

/// Whether `value` is one of [`IDENTITY_PROVIDERS`].
pub fn is_identity_provider(value: &str) -> bool {
    IDENTITY_PROVIDERS.contains(&value)
}

/// `identityProviderLabel`: the brand name, `None` for an unknown provider.
pub fn identity_provider_label(provider: &str) -> Option<&'static str> {
    match provider {
        "github" => Some("GitHub"),
        "discord" => Some("Discord"),
        "google" => Some("Google"),
        _ => None,
    }
}

/// `countSignInMethods`: every linked identity, the password (if set), and
/// the passkey factor (if at least one exists).
pub fn count_sign_in_methods(summary: &IdentitySummary) -> usize {
    let known = summary
        .identities
        .iter()
        .filter(|i| is_identity_provider(&i.provider))
        .count();
    known + usize::from(summary.has_password) + usize::from(summary.passkey_count > 0)
}

/// `canUnlinkIdentity`: more than one sign-in method remains.
pub fn can_unlink_identity(summary: &IdentitySummary) -> bool {
    count_sign_in_methods(summary) > 1
}

/// The providers offered to link: listed by the server, not yet linked.
pub fn linkable_providers(summary: &IdentitySummary) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for p in &summary.linkable_providers {
        let Some(known) = IDENTITY_PROVIDERS.iter().find(|k| **k == p.as_str()) else {
            continue;
        };
        let linked = summary.identities.iter().any(|i| i.provider == *known);
        if !linked && !out.contains(known) {
            out.push(known);
        }
    }
    out
}

// Order matters: Chromium forks before the Chrome and Safari tokens their
// user agents also carry; iOS before macOS ("like Mac OS X").
const BROWSERS: [(&[&str], &str); 6] = [
    (&["edg/", "edge/", "edgios/", "edga/"], "Edge"),
    (&["opr/", "opios/"], "Opera"),
    (&["samsungbrowser/"], "Samsung Internet"),
    (&["firefox/", "fxios/"], "Firefox"),
    (&["chrome/", "crios/"], "Chrome"),
    (&["safari/"], "Safari"),
];

const OPERATING_SYSTEMS: [(&[&str], &str); 7] = [
    (&["windows"], "Windows"),
    (&["cros"], "ChromeOS"),
    (&["iphone", "ipod"], "iOS"),
    (&["ipad"], "iPadOS"),
    (&["android"], "Android"),
    (&["mac os x", "macintosh"], "macOS"),
    (&["linux"], "Linux"),
];

fn first_match(source: &str, table: &[(&[&str], &'static str)]) -> Option<&'static str> {
    table
        .iter()
        .find(|(needles, _)| needles.iter().any(|n| source.contains(n)))
        .map(|(_, label)| *label)
}

/// `sessionDeviceLabel`: "Chrome · macOS" from a session's user agent;
/// `None` when nothing in the tables matches (the caller shows its
/// localized unknown label).
pub fn session_device_label(user_agent: Option<&str>) -> Option<String> {
    let ua = user_agent.filter(|u| !u.is_empty())?.to_ascii_lowercase();
    let browser = first_match(&ua, &BROWSERS);
    let os = first_match(&ua, &OPERATING_SYSTEMS);
    match (browser, os) {
        (Some(b), Some(o)) => Some(format!("{b} · {o}")),
        (Some(b), None) => Some(b.to_owned()),
        (None, Some(o)) => Some(o.to_owned()),
        (None, None) => None,
    }
}

fn web_browser_label(device: &DeviceView) -> Option<&'static str> {
    match device.browser.as_deref()? {
        "chrome" => Some("Chrome"),
        "firefox" => Some("Firefox"),
        "safari" => Some("Safari"),
        "edge" => Some("Edge"),
        _ => None,
    }
}

fn mobile_label(device: &DeviceView) -> Option<&'static str> {
    let tablet = device.form_factor == "tablet";
    match (device.platform.as_str(), tablet) {
        ("ios", true) => Some("iPad"),
        ("ios", false) => Some("iPhone"),
        ("android", true) => Some("Android tablet"),
        ("android", false) => Some("Android phone"),
        _ => None,
    }
}

fn platform_label(platform: &str) -> Option<&'static str> {
    match platform {
        "macos" => Some("Mac"),
        "windows" => Some("Windows PC"),
        "linux" => Some("Linux PC"),
        _ => None,
    }
}

/// `deviceTypeLabel`: the browser for the web client, the platform
/// otherwise; `None` for a value outside the tables.
pub fn device_type_label(device: &DeviceView) -> Option<&'static str> {
    if device.platform == "web" {
        return web_browser_label(device);
    }
    mobile_label(device).or_else(|| platform_label(&device.platform))
}

/// `deviceTitle`: the user's own name when set, else the device type.
pub fn device_title(device: &DeviceView) -> Option<String> {
    if let Some(name) = device.display_name.as_deref().filter(|n| !n.is_empty()) {
        return Some(name.to_owned());
    }
    device_type_label(device).map(str::to_owned)
}

/// `deviceIconName`: phone-shaped for iOS and Android.
pub fn is_handheld(device: &DeviceView) -> bool {
    matches!(device.platform.as_str(), "ios" | "android")
}

/// Names the server accepts: 1 to 40 characters after trimming.
pub const DEVICE_NAME_MAX_CHARS: usize = 40;

/// `normaliseDeviceNameDraft`: the trimmed name, `None` out of range.
pub fn normalise_device_name_draft(draft: &str) -> Option<String> {
    let name = draft.trim();
    let len = name.chars().count();
    if len == 0 || len > DEVICE_NAME_MAX_CHARS {
        return None;
    }
    Some(name.to_owned())
}

/// Sessions with no listed device: they show as their own rows.
pub fn unbound_sessions(sessions: &[SessionInfo], devices: &[DeviceView]) -> Vec<SessionInfo> {
    sessions
        .iter()
        .filter(|s| match s.device_id.as_deref() {
            Some(id) => !devices.iter().any(|d| d.id == id),
            None => true,
        })
        .cloned()
        .collect()
}

/// The `name_stamp` for `device/v1/rename`, from this account's sync clock
/// on this install, so a newer name always wins (contracts/sync.md).
/// `None` while signed out of the store.
pub fn device_name_stamp(store: &mut AppStore) -> Result<Option<String>, StoreError> {
    store.stamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_labels_come_only_from_the_tables() {
        let mac_chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/120.0 Safari/537.36";
        assert_eq!(
            session_device_label(Some(mac_chrome)).as_deref(),
            Some("Chrome · macOS")
        );
        let iphone = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Version/17.0 \
                      Mobile/15E148 Safari/604.1";
        assert_eq!(
            session_device_label(Some(iphone)).as_deref(),
            Some("Safari · iOS")
        );
        assert_eq!(session_device_label(Some("<script>evil</script>")), None);
        assert_eq!(session_device_label(None), None);
    }

    #[test]
    fn device_names_are_bounded() {
        assert_eq!(normalise_device_name_draft("   "), None);
        assert_eq!(
            normalise_device_name_draft("  Desk  ").as_deref(),
            Some("Desk")
        );
        assert_eq!(normalise_device_name_draft(&"x".repeat(41)), None);
        assert!(normalise_device_name_draft(&"ي".repeat(40)).is_some());
    }
}
