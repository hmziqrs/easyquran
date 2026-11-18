//! The app's typed config in `nsh_core::storage::Storage`, versioned with
//! migrations: one JSON document under one key, loaded once per read,
//! migrated forward on load, saved whole.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::event_bus;

/// The storage key the config document lives under.
pub const CONFIG_KEY: &str = "app.config";

/// The schema version this app writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The app's user-set config. `#[non_exhaustive]`: build with
/// [`AppConfig::new`] (the app's real defaults; the derived
/// [`AppConfig::default`] is the blank record) and assign, so a field can
/// join without breaking callers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
#[non_exhaustive]
pub struct AppConfig {
    /// The schema the document was written with (`SCHEMA_VERSION` here).
    pub schema_version: u32,
    /// What the user is called (the first-run flow asks once).
    pub display_name: String,
    /// Which appearance to follow: `"system"`, `"light"` or `"dark"`.
    pub theme: String,
    /// Whether analytics and monitoring sinks may send (their local sinks
    /// always keep; this gates anything real later).
    pub telemetry_enabled: bool,
}

impl AppConfig {
    /// The config a fresh app runs with.
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            display_name: String::new(),
            theme: "system".to_owned(),
            telemetry_enabled: true,
        }
    }
}

/// The config as the app runs with now: stored, or [`AppConfig::new`]
/// when nothing (or something unreadable) is stored. A document older
/// than [`SCHEMA_VERSION`] is migrated and saved back, so the stored form
/// converges on the current one.
pub fn load() -> AppConfig {
    let Some(raw) = nsh_core::storage::Storage::get_item(CONFIG_KEY) else {
        return AppConfig::new();
    };
    // The stored document's version, read before migrating: `migrate`
    // always answers the current schema, so it cannot say who was old.
    let stored_version = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|value| value.get("schema_version").and_then(Value::as_u64));
    match migrate(&raw) {
        Ok(config) => {
            if stored_version != Some(u64::from(SCHEMA_VERSION)) {
                // Converge: the next read sees the current schema.
                let _ = save(&config);
            }
            config
        }
        Err(error) => {
            crate::foundation::logging::log_error(&format!(
                "config unreadable, starting fresh: {error}"
            ));
            AppConfig::new()
        }
    }
}

/// Saves `config` whole. Announces `app.config` on the bus when it lands.
pub fn save(config: &AppConfig) -> Result<(), crate::foundation::error::AppError> {
    let mut current = config.clone();
    current.schema_version = SCHEMA_VERSION;
    let json = serde_json::to_string(&current).map_err(|e| {
        crate::foundation::error::AppError::new("internal", "The config could not be encoded")
            .detail(e.to_string())
    })?;
    nsh_core::storage::Storage::set_item(CONFIG_KEY, json)?;
    event_bus::publish(event_bus::TOPIC_CONFIG_SAVED, CONFIG_KEY);
    Ok(())
}

/// Parses the stored document at any schema version this app ever wrote,
/// migrated forward to `SCHEMA_VERSION`.
pub fn migrate(raw: &str) -> Result<AppConfig, crate::foundation::error::AppError> {
    let value: Value = serde_json::from_str(raw).map_err(|e| {
        crate::foundation::error::AppError::new("invalid", "The config is not JSON")
            .detail(e.to_string())
    })?;
    match value.get("schema_version").and_then(Value::as_u64) {
        Some(1..) => serde_json::from_value(value).map_err(|e| {
            crate::foundation::error::AppError::new("invalid", "The config's fields do not fit")
                .detail(e.to_string())
        }),
        // Schema 0 (the bootstrap build) stored `{ "name", "dark" }`; no
        // version field at all is a schema-0 document too.
        Some(0) | None => {
            let v0 = serde_json::from_value::<ConfigV0>(value).map_err(|e| {
                crate::foundation::error::AppError::new("invalid", "The config has no known shape")
                    .detail(e.to_string())
            })?;
            Ok(AppConfig {
                schema_version: SCHEMA_VERSION,
                display_name: v0.name,
                theme: if v0.dark { "dark" } else { "system" }.to_owned(),
                // Schema 0 had no telemetry choice; leaving it on keeps
                // what the app was already doing.
                telemetry_enabled: true,
                ..AppConfig::default()
            })
        }
    }
}

/// The schema-0 document.
#[derive(Deserialize)]
struct ConfigV0 {
    name: String,
    dark: bool,
}
