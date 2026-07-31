// Raw payload bytes (signatures need exact bytes — never re-encode), the full
// header set, and the URL query are all load-bearing: providers sign over
// different headers and Mercado Pago signs over the query string's data.id.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub provider: String,
    pub payload: Vec<u8>,
    pub headers: http::HeaderMap,
    pub query: Option<String>,
}
