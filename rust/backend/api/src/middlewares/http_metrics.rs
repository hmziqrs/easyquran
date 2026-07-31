use axum::{extract::MatchedPath, extract::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn track_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    // Template, not raw path: raw paths carry thousands of inlined IDs that
    // would explode http.route cardinality.
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        // Fixed label: scanner junk paths would otherwise balloon the label index.
        .unwrap_or_else(|| "unmatched".to_string());

    let metrics = crate::utils::telemetry::http_metrics();

    metrics.request_count.add(
        1,
        &[
            opentelemetry::KeyValue::new("http.method", method.clone()),
            opentelemetry::KeyValue::new("http.route", path.clone()),
        ],
    );

    let response = next.run(request).await;

    let duration = start.elapsed().as_millis() as f64;
    let status = response.status().as_u16();

    metrics.request_duration.record(
        duration,
        &[
            opentelemetry::KeyValue::new("http.method", method.clone()),
            opentelemetry::KeyValue::new("http.route", path.clone()),
            opentelemetry::KeyValue::new("http.status_code", status.to_string()),
        ],
    );

    metrics.response_status.add(
        1,
        &[
            opentelemetry::KeyValue::new("http.method", method),
            opentelemetry::KeyValue::new("http.route", path),
            opentelemetry::KeyValue::new("http.status_code", status.to_string()),
        ],
    );

    response
}
