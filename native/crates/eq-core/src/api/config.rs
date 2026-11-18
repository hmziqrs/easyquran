//! API base URL (contracts/devices.md D6). Production serves the API under
//! `/api` behind the proxy (prefix stripped), so the base ends in `/api`.
//! Local development talks to the API container directly on port 8888
//! (`docker-compose.local.yml`); an Android emulator reaches the host at
//! `10.0.2.2`.

/// Production API root.
pub const PRODUCTION_BASE: &str = "https://easyquran.fyi/api";
/// Local API root (`docker-compose.local.yml`, `127.0.0.1:8888`).
pub const LOCAL_BASE: &str = "http://localhost:8888";
/// Local API root as an Android emulator sees the host.
pub const ANDROID_EMULATOR_BASE: &str = "http://10.0.2.2:8888";

/// Where API calls go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    base: String,
}

impl ApiConfig {
    /// `base` with any trailing `/` removed.
    pub fn new(base: impl Into<String>) -> Self {
        let base = base.into();
        Self {
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    /// The production API.
    pub fn production() -> Self {
        Self::new(PRODUCTION_BASE)
    }

    /// The local API (`http://localhost:8888`).
    pub fn local() -> Self {
        Self::new(LOCAL_BASE)
    }

    /// The API root, without a trailing `/`.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// `base + path`; `path` starts with `/`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self::production()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_base_ends_in_api() {
        assert!(ApiConfig::production().base().ends_with("/api"));
        assert_eq!(
            ApiConfig::new("http://localhost:8888/").url("/csrf/v1/generate"),
            "http://localhost:8888/csrf/v1/generate"
        );
    }
}
