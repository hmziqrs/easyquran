// Raw payload bytes, full header set, and URL query are load-bearing: providers sign over different headers and Mercado Pago signs over the query string's data.id — never re-encode.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub provider: String,
    pub payload: Vec<u8>,
    pub headers: http::HeaderMap,
    pub query: Option<String>,
}
