use axum::{extract::MatchedPath, extract::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn track_metrics(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    // §8.2: label by the matched route TEMPLATE, not the raw path with inlined
    // identifiers. The Quran surface alone is ~7,750 raw paths (6,236 ayahs +
    // 604 pages + 556 rukus + …) — labeling on the raw path is a metrics
    // cardinality incident. `MatchedPath` is populated only for matched routes
    // (this middleware is applied via `route_layer`), so unmatched/404 requests
    // fall through to the raw path and probe traffic does not explode labels.
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_string())
        // §8.2: a FIXED label for unmatched (404/probe) traffic — the public
        // branch is wildcard-CORS + unauthenticated, so scanner junk paths would
        // otherwise balloon the OTLP `http.route` label index (the same
        // cardinality failure the MatchedPath switch was meant to close).
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
