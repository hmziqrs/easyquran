use std::net::IpAddr;

pub trait IpSource: Send + Sync {
    fn resolve(&self, request: &axum::extract::Request) -> String;
}

#[derive(Clone, Copy, Default, Debug)]
pub struct ClientIpSource;

impl IpSource for ClientIpSource {
    fn resolve(&self, request: &axum::extract::Request) -> String {
        request
            .extensions()
            .get::<axum_client_ip::ClientIp>()
            .map(|ip| ip.0.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Trusted internal caller. The bucket label is a fixed non-IP string so an
/// internal identity can never parse as a BanUnit and never enters W3a IP
/// escalation; it also never shares an external IP's content bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InternalServiceId {
    WebSsr,
}

impl InternalServiceId {
    pub fn bucket_label(self) -> &'static str {
        match self {
            InternalServiceId::WebSsr => "internal-webssr",
        }
    }
}

/// Typed request identity resolved before rate limiting. Replaces encoding a
/// service name as a fake IP: only `External` is a verified client `IpAddr`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestIdentity {
    External(IpAddr),
    InternalService(InternalServiceId),
}

impl RequestIdentity {
    pub fn bucket_key(&self) -> String {
        match self {
            RequestIdentity::External(ip) => ip.to_string(),
            RequestIdentity::InternalService(id) => id.bucket_label().to_string(),
        }
    }

    /// True only for a verified external IP. W3a escalation keys on this so a
    /// service identity (or an absent identity) can never escalate.
    pub fn is_external_ip(&self) -> bool {
        matches!(self, RequestIdentity::External(_))
    }
}

/// Resolves a typed identity for a request. `None` → no verified identity; the
/// limiter falls back to a shared non-IP "unknown" bucket (never escalation).
pub trait IdentitySource: Send + Sync {
    fn resolve(&self, request: &axum::extract::Request) -> Option<RequestIdentity>;
}

/// Default source: prefer the `RequestIdentity` extension (set by the ingress
/// middleware); fall back to the legacy `ClientIp` extension → `External`.
#[derive(Clone, Copy, Default, Debug)]
pub struct ClientIpIdentitySource;

impl IdentitySource for ClientIpIdentitySource {
    fn resolve(&self, request: &axum::extract::Request) -> Option<RequestIdentity> {
        if let Some(id) = request.extensions().get::<RequestIdentity>() {
            return Some(id.clone());
        }
        request
            .extensions()
            .get::<axum_client_ip::ClientIp>()
            .map(|ip| RequestIdentity::External(ip.0))
    }
}

#[derive(Clone)]
pub struct FnIpSource<F>(pub F)
where
    F: Fn(&axum::extract::Request) -> String + Send + Sync + 'static;

impl<F> IpSource for FnIpSource<F>
where
    F: Fn(&axum::extract::Request) -> String + Send + Sync + 'static,
{
    fn resolve(&self, request: &axum::extract::Request) -> String {
        (self.0)(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn client_ip_reads_resolved_extension() {
        let mut req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(axum_client_ip::ClientIp(IpAddr::V4(Ipv4Addr::new(
                203, 0, 113, 50,
            ))));
        assert_eq!(ClientIpSource.resolve(&req), "203.0.113.50");
    }

    #[test]
    fn client_ip_ignores_spoofed_headers_without_extension() {
        let req = axum::http::Request::builder()
            .header("x-forwarded-for", "1.2.3.4")
            .header("x-real-ip", "5.6.7.8")
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(ClientIpSource.resolve(&req), "unknown");
    }

    #[test]
    fn client_ip_fallback_to_unknown() {
        let req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(ClientIpSource.resolve(&req), "unknown");
    }

    #[test]
    fn external_identity_bucket_key_is_ip() {
        let id = RequestIdentity::External(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 50)));
        assert_eq!(id.bucket_key(), "203.0.113.50");
        assert!(id.is_external_ip());
    }

    #[test]
    fn internal_identity_bucket_key_is_not_an_ip() {
        let id = RequestIdentity::InternalService(InternalServiceId::WebSsr);
        let key = id.bucket_key();
        assert_eq!(key, "internal-webssr");
        assert!(!id.is_external_ip());
        assert!(
            key.parse::<IpAddr>().is_err(),
            "internal bucket key must never parse as an IP (escalation guard): {key}"
        );
    }

    #[test]
    fn identity_source_prefers_request_identity_extension() {
        let mut req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(axum_client_ip::ClientIp(IpAddr::V4(Ipv4Addr::new(
                203, 0, 113, 50,
            ))));
        req.extensions_mut()
            .insert(RequestIdentity::InternalService(InternalServiceId::WebSsr));
        let resolved = ClientIpIdentitySource.resolve(&req);
        assert_eq!(
            resolved,
            Some(RequestIdentity::InternalService(InternalServiceId::WebSsr))
        );
    }

    #[test]
    fn identity_source_falls_back_to_client_ip_extension() {
        let mut req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(axum_client_ip::ClientIp(IpAddr::V4(Ipv4Addr::new(
                198, 51, 100, 7,
            ))));
        assert_eq!(
            ClientIpIdentitySource.resolve(&req),
            Some(RequestIdentity::External(IpAddr::V4(Ipv4Addr::new(
                198, 51, 100, 7
            ))))
        );
    }

    #[test]
    fn identity_source_none_when_unresolved() {
        let req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(ClientIpIdentitySource.resolve(&req), None);
    }
}
