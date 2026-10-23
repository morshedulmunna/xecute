//! Typed HTTP boundaries: decode and validate input before invoking controllers.

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Query, Request},
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use validator::Validate;

/// HTTP status codes usable by controllers without importing Axum internals.
pub type HttpStatus = StatusCode;

/// Safe HTTP failure with a JSON envelope; request boundaries add correlation IDs.
#[derive(Debug, Clone, Copy)]
pub struct HttpError {
    status: StatusCode,
    public: Option<(&'static str, &'static str)>,
}

impl HttpError {
    /// Construct a failure from its public status code; non-error statuses become 500.
    pub const fn new(status: StatusCode) -> Self {
        let status = if status.as_u16() >= 400 && status.as_u16() <= 599 {
            status
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self {
            status,
            public: None,
        }
    }

    /// Map an expected domain failure to explicit public 4xx details.
    /// Codes must contain only lowercase ASCII letters, digits, or underscores.
    /// 5xx failures always keep the default safe details, even when custom text is supplied.
    pub fn public(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        let mut error = Self::new(status);
        if status.is_client_error()
            && !code.is_empty()
            && code.len() <= 64
            && code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            error.public = Some((code, message));
        }
        error
    }
    /// Return safe response details; never serialize an internal error's Display text.
    pub fn public_details(self) -> (&'static str, &'static str) {
        self.public
            .unwrap_or_else(|| crate::exceptions::default_error(self.status))
    }

    /// Return the response status without revealing submitted payload values.
    pub const fn status(self) -> StatusCode {
        self.status
    }
}

impl From<StatusCode> for HttpError {
    fn from(status: StatusCode) -> Self {
        Self::new(status)
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "HTTP {}", self.status)
    }
}

impl std::error::Error for HttpError {}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        crate::exceptions::error_response(self, "")
    }
}

/// JSON extractor used by generated `#[body]` adapters to validate DTOs automatically.
pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = HttpError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(|error| HttpError::new(error.status()))?;
        value
            .validate()
            .map_err(|_| HttpError::new(StatusCode::UNPROCESSABLE_ENTITY))?;
        Ok(Self(value))
    }
}

/// Query extractor used by generated `#[query]` adapters to validate DTOs automatically.
pub struct ValidatedQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ValidatedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(value) = Query::<T>::from_request_parts(parts, state)
            .await
            .map_err(|error| HttpError::new(error.status()))?;
        value
            .validate()
            .map_err(|_| HttpError::new(StatusCode::UNPROCESSABLE_ENTITY))?;
        Ok(Self(value))
    }
}

/// Documentation attached to one declared controller response.
#[derive(Debug, Serialize)]
pub struct ApiResponse {
    /// HTTP response status.
    pub status: u16,
    /// Human-readable response description.
    pub description: &'static str,
    /// Rust DTO name for a JSON response, if declared.
    pub json_type: Option<&'static str>,
    /// JSON Schema describing the response payload.
    pub schema: Option<serde_json::Value>,
}

/// Route documentation collected by `#[routes]`; this is metadata, not an OpenAPI schema.
#[derive(Debug, Serialize)]
pub struct ApiOperation {
    /// HTTP method.
    pub method: &'static str,
    /// Full route path, including the controller prefix.
    pub path: String,
    /// Short endpoint description.
    pub summary: &'static str,
    /// Documentation grouping tag.
    pub tag: &'static str,
    /// Stable operation identifier.
    pub operation_id: &'static str,
    /// Rust DTO name for the JSON body, if declared.
    pub body_type: Option<&'static str>,
    /// JSON Schema describing the request payload.
    pub body_schema: Option<serde_json::Value>,
    /// JSON Schema whose properties are documented as query parameters.
    pub query_schema: Option<serde_json::Value>,
    /// Declared response statuses and types.
    pub responses: Vec<ApiResponse>,
}
