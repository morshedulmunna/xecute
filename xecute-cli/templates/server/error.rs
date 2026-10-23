//! Public error normalization is shared with the runtime.

pub use xecute::exceptions::normalize_error;

/// Build a safe failure before the outer response boundary adds final headers.
pub fn response(status: axum::http::StatusCode, request_id: &str) -> axum::response::Response {
    xecute::exceptions::error_response(xecute::HttpError::new(status), request_id)
}
