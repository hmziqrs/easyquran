use std::env;

pub fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Some(true),
                "0" | "false" | "no" | "off" => Some(false),
                _ => None,
            }
        })
        .unwrap_or(default)
}

pub fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

/// Percentage env var: result is clamped to 0..=100 (NOT a generic u8 parser).
pub fn env_u8(key: &str, default: u8) -> u8 {
    let candidate = env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u8>().ok())
        .unwrap_or(default);
    candidate.clamp(0, 100)
}

pub fn env_with_fallback(keys: &[&str], default: Option<&str>) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }

    default.map(|value| value.to_string())
}

pub fn parse_env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}
