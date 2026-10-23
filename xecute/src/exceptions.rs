//! Consistent error envelopes and explicit mappings from domain errors.

use crate::HttpError;
use axum::{
    Json, Router,
    extract::Request,
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

/// Public JSON error response; internal errors and submitted values are never serialized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    /// Stable public details for this failure.
    pub error: ErrorDetail,
}
/// Public error details. Custom messages must be static, intentional API text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorDetail {
    /// Machine-readable error code.
    pub code: String,
    /// Safe message for clients.
    pub message: String,
    /// Server-generated correlation identifier, empty without the exception boundary.
    pub request_id: String,
}

/// Return the default stable code and safe message for an HTTP failure.
pub fn default_error(status: StatusCode) -> (&'static str, &'static str) {
    match status {
        StatusCode::BAD_REQUEST => ("bad_request", "Invalid request"),
        StatusCode::UNAUTHORIZED => ("unauthorized", "Authentication required"),
        StatusCode::FORBIDDEN => ("forbidden", "Access denied"),
        StatusCode::NOT_FOUND => ("not_found", "Route not found"),
        StatusCode::METHOD_NOT_ALLOWED => ("method_not_allowed", "Method not allowed"),
        StatusCode::CONFLICT => ("conflict", "Resource conflict"),
        StatusCode::PAYLOAD_TOO_LARGE => (
            "payload_too_large",
            "Request body exceeds the configured limit",
        ),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => {
            ("unsupported_media_type", "Unsupported request content type")
        }
        StatusCode::UNPROCESSABLE_ENTITY => ("validation_failed", "Invalid request data"),
        StatusCode::TOO_MANY_REQUESTS => ("rate_limited", "Too many requests; retry later"),
        StatusCode::SERVICE_UNAVAILABLE => ("overloaded", "Server is busy; retry later"),
        StatusCode::GATEWAY_TIMEOUT => ("request_timeout", "Request exceeded its deadline"),
        _ if status.is_server_error() => ("internal_error", "Internal server error"),
        _ => ("request_failed", "Request failed"),
    }
}

/// Build an envelope from a safe error and the server's correlation identifier.
pub fn error_response(error: HttpError, request_id: &str) -> Response {
    let (code, message) = error.public_details();
    let mut response = (
        error.status(),
        Json(ErrorEnvelope {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
                request_id: request_id.into(),
            },
        }),
    )
        .into_response();
    response.extensions_mut().insert(error);
    response
}

/// Normalize a response, preserving explicit HttpError mappings and protocol/CORS headers.
/// Unknown bodies, extractor rejections, and server failures receive safe defaults.
pub fn normalize_error(response: &mut Response, request_id: &str) {
    if !(response.status().is_client_error() || response.status().is_server_error()) {
        return;
    }
    let error = response
        .extensions()
        .get::<HttpError>()
        .copied()
        .filter(|error| error.status() == response.status())
        .unwrap_or_else(|| HttpError::new(response.status()));
    *response.body_mut() = error_response(error, request_id).into_body();
    // Headers describing the replaced representation must not survive normalization.
    for name in [
        header::CONTENT_LENGTH,
        header::CONTENT_ENCODING,
        header::CONTENT_RANGE,
        header::ETAG,
        header::LAST_MODIFIED,
    ] {
        response.headers_mut().remove(name);
    }
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
}

/// Server-generated request identifier available to handlers via request extensions.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Add safe error envelopes and request IDs to a plain Axum router.
/// Install once after route assembly. Generated applications already include this behavior
/// in their logging/policy boundary; they must not install a second ID boundary.
pub fn exception_boundary(router: Router) -> Router {
    router.layer(middleware::from_fn(finalize))
}
async fn finalize(mut request: Request, next: Next) -> Response {
    let is_head = request.method() == axum::http::Method::HEAD;
    let id = uuid::Uuid::new_v4().to_string();
    request.extensions_mut().insert(RequestId(id.clone()));
    let mut response = next.run(request).await;
    normalize_error(&mut response, &id);
    if is_head {
        *response.body_mut() = axum::body::Body::empty();
    }
    response
        .headers_mut()
        .insert("x-request-id", id.parse().expect("UUID is a valid header"));
    response
}
