//! Behavioral regression coverage for configuration, errors, and provider lifecycles.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use tower::ServiceExt;
use xecute::{
    HttpError,
    exceptions::{ErrorEnvelope, exception_boundary},
};

#[derive(Debug)]
struct AlreadyExists;
impl From<AlreadyExists> for HttpError {
    fn from(_: AlreadyExists) -> Self {
        Self::public(
            StatusCode::CONFLICT,
            "email_taken",
            "Email is already registered",
        )
    }
}
async fn create() -> Result<(), HttpError> {
    Err(AlreadyExists.into())
}
#[tokio::test]
async fn domain_mapping_survives_normalization_and_protocol_headers_are_preserved() {
    let router = Router::new()
        .route("/domain", get(create))
        .route(
            "/unsafe",
            get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "password=PRIVATE") }),
        )
        .route(
            "/custom-500",
            get(|| async {
                HttpError::public(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "private",
                    "password=PRIVATE",
                )
            }),
        )
        .route(
            "/protocol",
            get(|| async {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [
                        (header::RETRY_AFTER, "3"),
                        (header::CONTENT_ENCODING, "gzip"),
                        (header::ETAG, "old"),
                    ],
                    "PRIVATE",
                )
            }),
        );
    let router = exception_boundary(router);
    let head = router
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::NOT_FOUND);
    assert!(to_bytes(head.into_body(), 1024).await.unwrap().is_empty());

    for (path, status, code, message) in [
        (
            "/domain",
            StatusCode::CONFLICT,
            "email_taken",
            "Email is already registered",
        ),
        (
            "/unsafe",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
        ),
        (
            "/custom-500",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Internal server error",
        ),
        (
            "/missing",
            StatusCode::NOT_FOUND,
            "not_found",
            "Route not found",
        ),
        (
            "/protocol",
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "Too many requests; retry later",
        ),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("x-request-id", "untrusted")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let id = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        assert_ne!(id, "untrusted");
        assert!(uuid::Uuid::parse_str(&id).is_ok());
        if path == "/protocol" {
            assert_eq!(response.headers()[header::RETRY_AFTER], "3");
            assert!(!response.headers().contains_key(header::CONTENT_ENCODING));
            assert!(!response.headers().contains_key(header::ETAG));
        }
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(!String::from_utf8_lossy(&body).contains("PRIVATE"));
        let envelope: ErrorEnvelope = serde_json::from_slice(&body).unwrap();
        assert_eq!(envelope.error.code, code);
        assert_eq!(envelope.error.message, message);
        assert_eq!(envelope.error.request_id, id);
    }
}
#[test]
fn invalid_success_status_and_invalid_custom_code_cannot_create_an_error_success() {
    assert_eq!(
        HttpError::new(StatusCode::OK).status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    let response = HttpError::public(StatusCode::CONFLICT, "BAD CODE", "PRIVATE").into_response();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        HttpError::public(StatusCode::CONFLICT, "BAD CODE", "PRIVATE")
            .public_details()
            .0,
        "conflict"
    );
}
