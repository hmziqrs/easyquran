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
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn client_ip_reads_resolved_extension() {
        let mut req = axum::http::Request::builder()
            .body(axum::body::Body::empty())
            .unwrap();
        req.extensions_mut().insert(axum_client_ip::ClientIp(IpAddr::V4(
            Ipv4Addr::new(203, 0, 113, 50),
        )));
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
}
