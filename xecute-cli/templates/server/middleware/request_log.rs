//! One completion log per request, with a generated UUID and precise duration.
//! Log route templates when matched and paths without query strings otherwise.

use axum::{
    body::HttpBody,
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::Instrument;

use xecute::exceptions::RequestId;

/// Finalize the response and emit only completion metadata, never request payloads.
pub(crate) async fn observe(mut request: Request, next: Next) -> Response {
    let id = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(RequestId(id.clone()));
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str())
        .unwrap_or_else(|| request.uri().path())
        .to_owned();
    // Prefer route templates; unmatched paths omit the query string.
    let span = tracing::info_span!("http_request", request_id = %id);
    async move {
        let started = Instant::now();
        let mut response = next.run(request).await;
        super::response::finalize(&mut response, &id);
        if method == axum::http::Method::HEAD { *response.body_mut() = axum::body::Body::empty(); }
        let status = response.status().as_u16();
        let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
        let response_bytes = response.body().size_hint().exact();
        if status >= 500 {
            tracing::error!(request_id = %id, method = %method, route, status, duration_ms, response_bytes, "request completed");
        } else if status == 429 {
            tracing::warn!(request_id = %id, method = %method, route, status, duration_ms, response_bytes, "request completed");
        } else {
            tracing::info!(request_id = %id, method = %method, route, status, duration_ms, response_bytes, "request completed");
        }
        response
    }.instrument(span).await
}
