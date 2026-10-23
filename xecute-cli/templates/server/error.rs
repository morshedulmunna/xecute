//! Safe public error envelopes. Internal details belong in logs, not client responses.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
    request_id: String,
}

/// Map status codes to stable public messages with a request ID for log correlation.
pub fn response(status: StatusCode, request_id: &str) -> Response {
    let (code, message) = match status {
        StatusCode::BAD_REQUEST => ("bad_request", "Invalid request"),
        StatusCode::UNAUTHORIZED => ("unauthorized", "Authentication required"),
        StatusCode::FORBIDDEN => ("forbidden", "Access denied"),
        StatusCode::NOT_FOUND => ("not_found", "Route not found"),
        StatusCode::METHOD_NOT_ALLOWED => ("method_not_allowed", "Method not allowed"),
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
    };
    (
        status,
        Json(ErrorBody {
            error: ErrorDetail {
                code,
                message,
                request_id: request_id.into(),
            },
        }),
    )
        .into_response()
}
