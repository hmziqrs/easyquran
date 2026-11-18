//! Rust access to this app's custom theme tokens (`themes/theme.json`):
//! every name there is read through exactly one function here, so a token
//! rename is one edit.

use nsh_base::theme;
use nsh_core::prelude::*;

/// The token's color, or a fallback that reads as a bug in review: a
/// misspelled token must not paint nothing.
fn token(name: &'static str, fallback: AdaptiveColor) -> AdaptiveColor {
    theme().color(name).unwrap_or(fallback)
}

/// Text and icons drawn on `theme().destructive`.
pub fn destructive_foreground() -> AdaptiveColor {
    token("destructive-foreground", theme().background)
}

/// Success states: saved, online, paid.
pub fn success() -> AdaptiveColor {
    token("success", theme().foreground)
}

/// Text on `success()`.
pub fn success_foreground() -> AdaptiveColor {
    token("success-foreground", theme().background)
}

/// Warning states: stale data, storage almost full.
pub fn warning() -> AdaptiveColor {
    token("warning", theme().foreground)
}

/// Text on `warning()`.
pub fn warning_foreground() -> AdaptiveColor {
    token("warning-foreground", theme().background)
}

/// Informational surfaces that are not errors.
pub fn info() -> AdaptiveColor {
    token("info", theme().foreground)
}

/// Text on `info()`.
pub fn info_foreground() -> AdaptiveColor {
    token("info-foreground", theme().background)
}

/// A rising figure; data meaning, not decoration.
pub fn up() -> AdaptiveColor {
    token("up", theme().foreground)
}

/// A falling figure; data meaning, not decoration.
pub fn down() -> AdaptiveColor {
    token("down", theme().foreground)
}

/// The scrim behind a modal, a sheet or a popup; darker in dark
/// appearance so the page reads through it.
pub fn backdrop() -> AdaptiveColor {
    token("backdrop", theme().foreground)
}

#[cfg(test)]
mod tests {
    /// Every accessor's token exists in the theme file: a rename on one
    /// side fails here instead of painting the fallback at run time.
    #[test]
    fn every_custom_token_is_declared() {
        let file = include_str!("../../themes/theme.json");
        let theme: serde_json::Value = serde_json::from_str(file).unwrap();
        let mut declared = serde_json::Map::new();
        for group in ["theme", "light", "dark"] {
            if let Some(vars) = theme["cssVars"][group].as_object() {
                declared.extend(vars.clone());
            }
        }
        for name in [
            "destructive-foreground",
            "success",
            "success-foreground",
            "warning",
            "warning-foreground",
            "info",
            "info-foreground",
            "up",
            "down",
            "backdrop",
        ] {
            assert!(
                declared.contains_key(name),
                "token `{name}` is missing from themes/theme.json"
            );
        }
    }
}
