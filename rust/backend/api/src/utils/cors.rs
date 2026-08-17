use std::sync::Arc;

use axum::http::HeaderValue;

/// The primary consumer origin; required to be present in any production list.
pub const EASYQURAN_ORIGIN: &str = "https://easyquran.fyi";

// One validated, immutable origin set built once at boot and shared between
// `CorsLayer` (HeaderValue list) and `origin_guard` (membership check). It holds
// its data behind an `Arc`, so `clone()` is cheap and the same parsed set is
// reused per request — no env read or parsing happens outside boot.
#[derive(Clone, Debug)]
pub struct AllowedOrigins {
    inner: Arc<AllowedOriginsInner>,
}

#[derive(Debug)]
struct AllowedOriginsInner {
    origins: Vec<String>,
    header_values: Vec<HeaderValue>,
}

impl AllowedOrigins {
    pub fn new(origins: Vec<String>) -> Result<Self, String> {
        let mut canonical = Vec::with_capacity(origins.len());
        let mut header_values = Vec::with_capacity(origins.len());
        for raw in &origins {
            let origin = parse_origin(raw)?;
            let hv = origin
                .parse::<HeaderValue>()
                .map_err(|e| format!("origin '{origin}' is not a valid header value: {e}"))?;
            canonical.push(origin);
            header_values.push(hv);
        }
        Ok(Self {
            inner: Arc::new(AllowedOriginsInner {
                origins: canonical,
                header_values,
            }),
        })
    }

    pub fn header_values(&self) -> &[HeaderValue] {
        &self.inner.header_values
    }

    pub fn origins(&self) -> &[String] {
        &self.inner.origins
    }

    pub fn contains_header(&self, value: &HeaderValue) -> bool {
        self.inner
            .header_values
            .iter()
            .any(|allowed| allowed == value)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.origins.is_empty()
    }
}

/// Validate ONE origin element. Pure: no env, no I/O. Accepts `http(s)://host[:port]`
/// with no path other than an optional root `/`, rejecting empty/non-ASCII entries,
/// `${...}` placeholders, credentials (`@`), query (`?`), fragment (`#`), any scheme
/// other than http/https, and a missing host. Returns the canonical origin (a lone
/// trailing `/` is stripped).
pub fn parse_origin(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("origin element is empty".to_string());
    }
    if !trimmed.is_ascii() {
        return Err(format!("origin '{trimmed}' contains non-ASCII characters"));
    }
    if trimmed.contains("${") {
        return Err(format!(
            "origin '{trimmed}' contains an unexpanded ${{...}} placeholder (write the domain \
             out literally; Compose does not expand env vars inside env_file values)"
        ));
    }
    if trimmed.contains('@') {
        return Err(format!(
            "origin '{trimmed}' contains credentials (user:pass@host is not a valid CORS origin)"
        ));
    }
    if trimmed.contains('?') || trimmed.contains('#') {
        return Err(format!(
            "origin '{trimmed}' must not include a query (?) or fragment (#)"
        ));
    }
    let (scheme, rest) = trimmed
        .strip_prefix("https://")
        .map(|r| ("https://", r))
        .or_else(|| trimmed.strip_prefix("http://").map(|r| ("http://", r)))
        .ok_or_else(|| format!("origin '{trimmed}' is not an absolute http(s):// origin"))?;
    let (authority, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, ""),
    };
    if !path.is_empty() && path != "/" {
        return Err(format!(
            "origin '{trimmed}' has a path '{path}' (only scheme://host[:port] is permitted)"
        ));
    }
    if authority.is_empty() {
        return Err(format!("origin '{trimmed}' is missing a host"));
    }
    validate_authority(authority)?;
    Ok(format!("{scheme}{authority}"))
}

fn validate_authority(authority: &str) -> Result<(), String> {
    // Bracketed IPv6 literal: [host] or [host]:port
    if let Some(after_open) = authority.strip_prefix('[') {
        let close = after_open
            .find(']')
            .ok_or_else(|| format!("origin authority '{authority}' has an unmatched '['"))?;
        let host = &after_open[..close];
        if host.is_empty() {
            return Err(format!("origin authority '{authority}' is missing a host"));
        }
        let tail = &after_open[close + 1..];
        if !tail.is_empty() {
            validate_port(tail)?;
        }
        return Ok(());
    }
    // host or host:port — a trailing port, if present, must be numeric.
    if let Some((host, port)) = authority.rsplit_once(':') {
        if host.is_empty() {
            return Err(format!("origin authority '{authority}' is missing a host"));
        }
        validate_port(port)?;
    }
    Ok(())
}

fn validate_port(port: &str) -> Result<(), String> {
    let p = port.trim_start_matches(':');
    if p.is_empty() {
        return Err(format!("port segment '{port}' is empty"));
    }
    if p.parse::<u16>().is_err() {
        return Err(format!(
            "port segment '{p}' is not a valid port number (0..=65535)"
        ));
    }
    Ok(())
}

/// Parse a comma-separated origin list. Pure. Whitespace-only elements are rejected.
pub fn parse_origins(raw: &str) -> Result<Vec<String>, String> {
    raw.split(',').map(parse_origin).collect()
}

/// Localhost/LAN origins permitted in explicit non-production only. Never returned
/// for production — production takes its complete consumer list from ALLOWED_ORIGINS.
pub fn dev_default_origins(admin_port: Option<&str>, consumer_port: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = [
        "http://localhost:8081",
        "http://localhost:8080",
        "http://127.0.0.1:8080",
        "http://127.0.0.1:8000",
        "http://127.0.0.1:8888",
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://localhost:3000",
        "http://127.0.0.1:3000",
        "http://127.0.0.1:3001",
        "http://127.0.0.1:3002",
        "http://127.0.0.1:3333",
        "https://127.0.0.1:3333",
        "http://192.168.0.101:3333",
        "http://192.168.0.101:3000",
        "http://192.168.0.101:8000",
        "http://192.168.0.101:8080",
        "http://192.168.0.101:8888",
        "http://192.168.0.23:3333",
        "http://192.168.0.23:3000",
        "http://192.168.0.23:8080",
        "http://192.168.0.23:8888",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for port in admin_port.into_iter().chain(consumer_port) {
        // Skip non-numeric ports rather than poisoning the dev list; boot validation
        // still rejects a numeric-but-bogus value via parse_origin.
        if port.parse::<u16>().is_ok() {
            v.push(format!("http://localhost:{port}"));
            v.push(format!("http://127.0.0.1:{port}"));
        }
    }
    v
}

/// Resolve the boot-time `AllowedOrigins` from its inputs (pure given its arguments,
/// so a serialized env matrix stays testable). `is_prod` enforces HTTPS-only +
/// EasyQuran-origin + no localhost/LAN. `env_raw` is the raw ALLOWED_ORIGINS value
/// (None or trim-empty ⇒ unset).
pub fn build_allowed_origins(
    is_prod: bool,
    env_raw: Option<&str>,
    admin_port: Option<&str>,
    consumer_port: Option<&str>,
) -> Result<AllowedOrigins, String> {
    let env_origins: Vec<String> = match env_raw.map(str::trim) {
        Some(s) if !s.is_empty() => parse_origins(s)?,
        _ => Vec::new(),
    };
    if is_prod {
        if env_origins.is_empty() {
            return Err(
                "ALLOWED_ORIGINS is unset/empty in production. Set it to a comma-separated list \
                 of HTTPS origins permitted to call /api (must include https://easyquran.fyi)."
                    .to_string(),
            );
        }
        for origin in &env_origins {
            let rest = origin.strip_prefix("https://").ok_or_else(|| {
                format!("ALLOWED_ORIGINS entry '{origin}' must use HTTPS in production")
            })?;
            let host = host_of(rest);
            if host_is_local(host) {
                return Err(format!(
                    "ALLOWED_ORIGINS entry '{origin}' is a localhost/LAN origin; production \
                     permits public HTTPS origins only"
                ));
            }
        }
        if !env_origins.iter().any(|o| o.as_str() == EASYQURAN_ORIGIN) {
            return Err(format!(
                "ALLOWED_ORIGINS must include {EASYQURAN_ORIGIN} in production (the primary \
                 consumer origin)"
            ));
        }
        AllowedOrigins::new(env_origins)
    } else {
        let mut all = dev_default_origins(admin_port, consumer_port);
        all.extend(env_origins);
        AllowedOrigins::new(all)
    }
}

/// Boot-time entry point: reads ALLOWED_ORIGINS / ADMIN_PORT / CONSUMER_PORT and the
/// environment class, then delegates to `build_allowed_origins`. The whole parse +
/// policy decision completes once at boot; the result is shared via AppState, never
/// re-read per request.
pub fn allowed_origins_from_env() -> Result<AllowedOrigins, String> {
    let is_prod = crate::config::settings::is_production()?;
    let env_raw = std::env::var("ALLOWED_ORIGINS").ok();
    let admin_port = std::env::var("ADMIN_PORT").ok();
    let consumer_port = std::env::var("CONSUMER_PORT").ok();
    build_allowed_origins(
        is_prod,
        env_raw.as_deref(),
        admin_port.as_deref(),
        consumer_port.as_deref(),
    )
}

/// Extract the host portion (no port, no brackets) from the text that follows
/// `scheme://` in a canonical origin.
fn host_of(authority_after_scheme: &str) -> &str {
    if let Some(after_open) = authority_after_scheme.strip_prefix('[') {
        return after_open.split(']').next().unwrap_or("");
    }
    match authority_after_scheme.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host,
        _ => authority_after_scheme,
    }
}

/// True for localhost, loopback, private, link-local, and unspecified IP hosts.
/// Production rejects these; non-production permits them via the dev defaults.
fn host_is_local(host: &str) -> bool {
    let h = host.trim();
    if h.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if let Ok(ip) = h.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
        };
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_origin_valid_https() {
        assert_eq!(
            parse_origin("https://easyquran.fyi").unwrap(),
            "https://easyquran.fyi"
        );
        assert_eq!(
            parse_origin(" https://hmziq.rs ").unwrap(),
            "https://hmziq.rs"
        );
    }

    #[test]
    fn parse_origin_valid_http_dev() {
        assert_eq!(
            parse_origin("http://localhost:8080").unwrap(),
            "http://localhost:8080"
        );
        assert_eq!(
            parse_origin("http://192.168.0.101:3333").unwrap(),
            "http://192.168.0.101:3333"
        );
    }

    #[test]
    fn parse_origin_strips_lone_trailing_slash() {
        assert_eq!(
            parse_origin("https://easyquran.fyi/").unwrap(),
            "https://easyquran.fyi"
        );
    }

    #[test]
    fn parse_origin_rejects_empty() {
        assert!(parse_origin("").is_err());
        assert!(parse_origin("   ").is_err());
    }

    #[test]
    fn parse_origin_rejects_non_ascii() {
        let err = parse_origin("https://例え.jp").expect_err("unicode must fail");
        assert!(err.contains("non-ASCII"), "{err}");
    }

    #[test]
    fn parse_origin_rejects_placeholder() {
        let err = parse_origin("https://${DOMAIN}").expect_err("placeholder must fail");
        assert!(err.contains("placeholder"), "{err}");
    }

    #[test]
    fn parse_origin_rejects_credentials() {
        let err = parse_origin("https://user:pass@example.com").expect_err("credentials must fail");
        assert!(err.contains("credentials"), "{err}");
    }

    #[test]
    fn parse_origin_rejects_query_and_fragment() {
        assert!(parse_origin("https://example.com?x=1").is_err());
        assert!(parse_origin("https://example.com#frag").is_err());
    }

    #[test]
    fn parse_origin_rejects_non_root_path() {
        let err = parse_origin("https://example.com/app").expect_err("path must fail");
        assert!(err.contains("path"), "{err}");
    }

    #[test]
    fn parse_origin_rejects_bad_scheme() {
        let err = parse_origin("ftp://example.com").expect_err("non-http(s) scheme must fail");
        assert!(err.contains("absolute http(s)"), "{err}");
    }

    #[test]
    fn parse_origin_rejects_missing_host() {
        assert!(parse_origin("https://").is_err());
        assert!(parse_origin("https://:8080").is_err());
    }

    #[test]
    fn parse_origin_rejects_bad_port() {
        assert!(parse_origin("https://example.com:notaport").is_err());
        // 65536 is one past u16::MAX.
        assert!(parse_origin("https://example.com:65536").is_err());
    }

    #[test]
    fn parse_origins_rejects_empty_element() {
        assert!(parse_origins("https://a.com,").is_err());
        assert!(parse_origins(",https://a.com").is_err());
        assert!(parse_origins("https://a.com,,https://b.com").is_err());
    }

    // --- build_allowed_origins: production happy path -------------------------

    #[test]
    fn build_production_accepts_valid_list() {
        let v = build_allowed_origins(
            true,
            Some("https://easyquran.fyi,https://hmziq.rs"),
            None,
            None,
        )
        .expect("valid production list builds");
        assert!(v.origins().contains(&"https://easyquran.fyi".to_string()));
        assert!(v.origins().contains(&"https://hmziq.rs".to_string()));
        // HeaderValue membership matches an incoming Origin byte-for-byte.
        let incoming: HeaderValue = "https://easyquran.fyi".parse().unwrap();
        assert!(v.contains_header(&incoming));
        let evil: HeaderValue = "https://evil.example".parse().unwrap();
        assert!(!v.contains_header(&evil));
    }

    // --- build_allowed_origins: production boot failures -----------------------

    #[test]
    fn build_production_rejects_unset() {
        let err = build_allowed_origins(true, None, None, None).expect_err("unset must fail");
        assert!(
            err.contains("unset/empty") && err.contains("production"),
            "{err}"
        );
    }

    #[test]
    fn build_production_rejects_trim_empty() {
        let err =
            build_allowed_origins(true, Some("   "), None, None).expect_err("empty must fail");
        assert!(err.contains("unset/empty"), "{err}");
    }

    #[test]
    fn build_production_requires_easyquran_origin() {
        let err = build_allowed_origins(
            true,
            Some("https://hmziq.rs,https://hzmiqrs.com"),
            None,
            None,
        )
        .expect_err("missing EasyQuran origin must fail");
        assert!(err.contains(EASYQURAN_ORIGIN), "{err}");
    }

    #[test]
    fn build_production_rejects_http_origin() {
        let err = build_allowed_origins(
            true,
            Some("https://easyquran.fyi,http://hmziq.rs"),
            None,
            None,
        )
        .expect_err("HTTP origin must fail in production");
        assert!(err.contains("HTTPS"), "{err}");
    }

    #[test]
    fn build_production_rejects_placeholder() {
        let err = build_allowed_origins(true, Some("https://${DOMAIN}"), None, None)
            .expect_err("placeholder must fail boot");
        assert!(err.contains("placeholder"), "{err}");
    }

    #[test]
    fn build_production_rejects_localhost() {
        let err = build_allowed_origins(
            true,
            Some("https://easyquran.fyi,https://localhost:3000"),
            None,
            None,
        )
        .expect_err("localhost must fail in production");
        assert!(err.contains("localhost/LAN"), "{err}");
    }

    #[test]
    fn build_production_rejects_lan_ip() {
        let err = build_allowed_origins(
            true,
            Some("https://easyquran.fyi,https://192.168.0.5"),
            None,
            None,
        )
        .expect_err("LAN origin must fail in production");
        assert!(err.contains("localhost/LAN"), "{err}");
    }

    #[test]
    fn build_production_rejects_credentialed() {
        let err = build_allowed_origins(
            true,
            Some("https://easyquran.fyi,https://u:p@hmziq.rs"),
            None,
            None,
        )
        .expect_err("credentials must fail boot");
        assert!(err.contains("credentials"), "{err}");
    }

    #[test]
    fn build_production_rejects_unicode() {
        let err = build_allowed_origins(true, Some("https://例え.jp"), None, None)
            .expect_err("unicode must fail boot");
        assert!(err.contains("non-ASCII"), "{err}");
    }

    #[test]
    fn build_production_rejects_path() {
        let err = build_allowed_origins(true, Some("https://easyquran.fyi/app"), None, None)
            .expect_err("path must fail boot");
        assert!(err.contains("path"), "{err}");
    }

    #[test]
    fn build_production_rejects_query() {
        let err = build_allowed_origins(true, Some("https://easyquran.fyi?x=1"), None, None)
            .expect_err("query must fail boot");
        assert!(err.contains("query") || err.contains("fragment"), "{err}");
    }

    // --- build_allowed_origins: non-production keeps LAN gated to dev ---------

    #[test]
    fn build_non_production_includes_dev_defaults_and_lan() {
        let v = build_allowed_origins(false, None, None, None).expect("dev defaults build");
        let names: Vec<&str> = v.origins().iter().map(String::as_str).collect();
        assert!(
            names.contains(&"http://localhost:8080"),
            "localhost kept in dev"
        );
        assert!(
            names.contains(&"http://localhost:5173"),
            "Vite dev server origin kept in dev"
        );
        assert!(
            names.contains(&"http://192.168.0.101:3333"),
            "LAN origins stay permitted in non-production"
        );
        // An off-origin HeaderValue is still rejected.
        let evil: HeaderValue = "https://evil.example".parse().unwrap();
        assert!(!v.contains_header(&evil));
    }

    #[test]
    fn build_non_production_merges_env_origins() {
        let v = build_allowed_origins(false, Some("https://hmziq.rs"), None, None).unwrap();
        assert!(v.origins().contains(&"https://hmziq.rs".to_string()));
        // Dev defaults are still present alongside env.
        assert!(v
            .origins()
            .iter()
            .any(|o| o.starts_with("http://localhost")));
    }

    #[test]
    fn build_non_production_includes_dynamic_ports() {
        let v = build_allowed_origins(false, None, Some("9001"), Some("4000")).unwrap();
        assert!(v.origins().contains(&"http://localhost:9001".to_string()));
        assert!(v.origins().contains(&"http://127.0.0.1:4000".to_string()));
    }

    #[test]
    fn build_non_production_skips_non_numeric_port() {
        // A bogus port does not poison the list; boot does not fail in dev.
        let v = build_allowed_origins(false, None, Some("not-a-port"), None).unwrap();
        assert!(!v.origins().iter().any(|o| o.contains("not-a-port")));
    }

    #[test]
    fn host_is_local_detects_private_and_loopback() {
        assert!(host_is_local("localhost"));
        assert!(host_is_local("127.0.0.1"));
        assert!(host_is_local("::1"));
        assert!(host_is_local("192.168.0.5"));
        assert!(host_is_local("10.0.0.1"));
        assert!(!host_is_local("easyquran.fyi"));
        assert!(!host_is_local("hmziq.rs"));
    }
}
