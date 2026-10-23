//! Response normalization, request-ID propagation, and API security headers.

use crate::server::error;
use axum::{
    http::{HeaderValue, header},
    response::Response,
};

/// Keep protocol headers while replacing errors with safe JSON and adding security headers.
pub(super) fn finalize(response: &mut Response, id: &str) {
    // Normalize framework errors while retaining protocol/CORS/retry headers.
    if response.status().is_client_error() || response.status().is_server_error() {
        let normalized = error::response(response.status(), id);
        *response.body_mut() = normalized.into_body();
        response.headers_mut().remove(header::CONTENT_LENGTH);
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    response
        .headers_mut()
        .insert("x-request-id", id.parse().unwrap());
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("DENY"));
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    // Only trusted Swagger routes may serve browser scripts/styles; API responses stay strict.
    let csp = if response
        .extensions()
        .get::<crate::server::docs::SwaggerAsset>()
        .is_some()
        && response.status().is_success()
    {
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'"
    } else {
        "default-src 'none'; frame-ancestors 'none'"
    };
    response
        .headers_mut()
        .insert("content-security-policy", HeaderValue::from_static(csp));
}
