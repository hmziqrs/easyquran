//! The HTTP seam over NSH's own `fetch`: one small trait so the real HTTP
//! library replaces [`NshHttp`] in one file, with no app code changing.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use nsh_core::element::{HttpRequest, HttpResponse};

use crate::foundation::error::AppError;

/// A request in flight, boxed because `fetch`'s future is the engine's own
/// (not `Send`, not nameable).
pub type HttpFut<'a> = Pin<Box<dyn Future<Output = Result<HttpResponse, AppError>> + 'a>>;

/// The app's HTTP client. Errors mean the request never got an answer
/// (offline, refused by the sandbox); a non-2xx status is an
/// [`HttpResponse`] whose `ok()` is false, as HTTP works.
pub trait HttpService: Send + Sync {
    /// Sends `request` through the engine's network stack, under the app's
    /// `capabilities.network`.
    fn send(&self, request: HttpRequest) -> HttpFut<'_>;

    /// `GET url`. Convenience with the same error contract as
    /// [`HttpService::send`].
    fn get(&self, url: &str) -> HttpFut<'_> {
        self.send(HttpRequest::get(url))
    }

    /// `POST body` to `url`.
    fn post(&self, url: &str, body: Vec<u8>) -> HttpFut<'_> {
        self.send(HttpRequest::post(url, body))
    }
}

/// The local implementation over NSH's `fetch`: what the app runs until a
/// real HTTP library takes the seam.
#[derive(Debug, Default)]
pub struct NshHttp;

impl HttpService for NshHttp {
    fn send(&self, request: HttpRequest) -> HttpFut<'_> {
        Box::pin(async move {
            nsh_core::element::fetch(request).await.map_err(|reason| {
                crate::foundation::error::AppError::new("network", "The request could not be made")
                    .detail(reason)
            })
        })
    }
}

/// The app's shared client. Swapping the implementation means editing this
/// file's `CLIENT` line; call sites never change.
pub fn client() -> Arc<dyn HttpService> {
    static CLIENT: std::sync::LazyLock<Arc<dyn HttpService>> =
        std::sync::LazyLock::new(|| Arc::new(NshHttp));
    CLIENT.clone()
}
