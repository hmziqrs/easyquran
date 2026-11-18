//! The capability registry: what `nsh.json` declares, read at compile
//! time, so the diagnostics page and guards reflect the real declaration
//! instead of a hand-copied list.

use std::sync::LazyLock;

use nsh_core::element::SharedString;

/// The capability keys NSH 0.5.0 knows, in the order of the runtime's
/// own list (`KNOWN` in `nsh-core/src/access.rs`). Since 0.5.0 an unknown
/// key is only a build warning, so this list is checked against the source.
pub const KNOWN_CAPABILITIES: &[&str] = &[
    "windows",
    "network",
    "webview",
    "statusBar",
    "clipboard",
    "picker",
    "speech",
    "notifications",
    "audio",
    "openUrl",
    "process",
    "fs",
    "host",
];

/// One capability of the app's declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capability {
    /// The key in `nsh.json` (`"network"`).
    pub name: SharedString,
    /// Whether `nsh.json` declares it. A host app may still grant less.
    pub declared: bool,
}

/// The app's `nsh.json`, read at compile time.
static MANIFEST: LazyLock<Option<serde_json::Value>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../../nsh.json")).ok());

/// The declared capabilities: every object under `nsh.json`
/// `"capabilities"`, known keys first, extras after.
pub fn registry() -> Vec<Capability> {
    let mut registry: Vec<Capability> = KNOWN_CAPABILITIES
        .iter()
        .map(|name| Capability {
            name: (*name).into(),
            declared: declared(name),
        })
        .collect();
    if let Some(extras) = MANIFEST
        .as_ref()
        .and_then(|json| json.get("capabilities"))
        .and_then(serde_json::Value::as_object)
    {
        for name in extras.keys() {
            if !KNOWN_CAPABILITIES.contains(&name.as_str()) {
                registry.push(Capability {
                    name: name.clone().into(),
                    declared: true,
                });
            }
        }
    }
    registry
}

/// Whether the manifest declares `name` (the key being present counts;
/// what it grants is the platform's reading).
pub fn declared(name: &str) -> bool {
    MANIFEST
        .as_ref()
        .and_then(|json| json.get("capabilities"))
        .and_then(serde_json::Value::as_object)
        .is_some_and(|caps| caps.contains_key(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break seen: `registry` returned an empty vec — failed on "{known} appears once".
    #[test]
    fn every_known_key_is_listed_once() {
        let names = registry();
        for known in KNOWN_CAPABILITIES {
            assert_eq!(
                names.iter().filter(|c| c.name.as_ref() == *known).count(),
                1,
                "{known} appears once"
            );
        }
    }

    // Break seen: `declared` returned false always — failed on `assert!(declared("clipboard"))`.
    #[test]
    fn the_manifest_is_read_as_declared() {
        // Pin the current nsh.json: what the platform area declared is
        // declared, and what it has not is not.
        assert!(declared("clipboard"));
        assert!(declared("webview"));
        assert!(!declared("network"));
    }
}
