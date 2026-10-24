//! HTTP behavior tests for limits, health probes, CORS, errors, and concurrency.

use super::support::{app, request};
use crate::{
    configs::{Config, CorsConfig},
    server::router,
};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{HeaderValue, Method, StatusCode, header},
    routing::get,
    routing::post,
};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn successful_and_error_responses_have_ids_and_security_headers() {
    let app = app(&Config::default());
    for (path, status) in [
        ("/items", StatusCode::OK),
        ("/missing?secret=private", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(request(path, "127.0.0.1"))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let id = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        if status.is_client_error() {
            let body = to_bytes(response.into_body(), 4096).await.unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["error"]["request_id"], id);
            assert_eq!(body["error"]["code"], "not_found");
        }
    }
}

#[tokio::test]
async fn rate_limits_socket_peers_and_ignores_spoofed_forwarding_headers() {
    let app = app(&Config {
        rate_limit_requests: 1,
        ..Config::default()
    });
    assert_eq!(
        app.clone()
            .oneshot(request("/items", "127.0.0.1"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let mut spoofed = request("/items", "127.0.0.1");
    spoofed
        .headers_mut()
        .insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));
    let response = app.clone().oneshot(spoofed).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().contains_key(header::RETRY_AFTER));
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(
        app.clone()
            .oneshot(request("/items", "127.0.0.2"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    for path in ["/health/live", "/health/ready"] {
        assert_eq!(
            app.clone()
                .oneshot(request(path, "127.0.0.1"))
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn timeout_and_panic_return_safe_json_errors() {
    let config = Config {
        request_timeout: std::time::Duration::from_millis(5),
        ..Config::default()
    };
    let app = router(
        Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    StatusCode::OK
                }),
            )
            .route(
                "/panic",
                get(|| async {
                    panic!("private internal details");
                    #[allow(unreachable_code)]
                    StatusCode::OK
                }),
            ),
        &config,
    );
    for (path, status, code) in [
        ("/slow", StatusCode::GATEWAY_TIMEOUT, "request_timeout"),
        (
            "/panic",
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(path, "127.0.0.1"))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], code);
        assert!(!body.to_string().contains("private internal details"));
    }
}

#[tokio::test]
async fn rejects_declared_and_streamed_oversized_bodies() {
    let app = router(
        Router::new().route("/echo", post(|body: axum::body::Bytes| async move { body })),
        &Config {
            body_limit: 4,
            ..Config::default()
        },
    );
    for declared in [false, true] {
        let mut req = request("/echo", "127.0.0.1");
        *req.method_mut() = Method::POST;
        *req.body_mut() = Body::from("12345");
        if declared {
            req.headers_mut()
                .insert(header::CONTENT_LENGTH, HeaderValue::from_static("5"));
        }
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    }
}

#[tokio::test]
async fn cors_allows_only_configured_origins() {
    let app = app(&Config {
        cors: CorsConfig {
            origins: vec![HeaderValue::from_static("https://example.com")],
            ..CorsConfig::default()
        },
        ..Config::default()
    });
    for origin in ["https://example.com", "https://untrusted.example"] {
        let mut req = request("/items", "127.0.0.1");
        *req.method_mut() = Method::OPTIONS;
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        req.headers_mut().insert(
            header::ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("GET"),
        );
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            origin == "https://example.com"
        );
        assert!(response.headers().contains_key("x-request-id"));
    }
    let app = self::app(&Config::default());
    let mut req = request("/items", "127.0.0.1");
    req.headers_mut().insert(
        header::ORIGIN,
        HeaderValue::from_static("https://example.com"),
    );
    assert!(
        !app.oneshot(req)
            .await
            .unwrap()
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
}

#[tokio::test]
async fn concurrency_rejection_preserves_health_and_permits_are_released() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let handler_entered = entered.clone();
    let handler_release = release.clone();
    let app = router(
        Router::new().route(
            "/work",
            get(move || {
                let entered = handler_entered.clone();
                let release = handler_release.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    StatusCode::OK
                }
            }),
        ),
        &Config {
            max_in_flight: 1,
            ..Config::default()
        },
    );
    let first = tokio::spawn(app.clone().oneshot(request("/work", "127.0.0.1")));
    tokio::time::timeout(std::time::Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    let response = app
        .clone()
        .oneshot(request("/work", "127.0.0.2"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(
        app.clone()
            .oneshot(request("/health/live", "127.0.0.2"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    release.notify_one();
    assert_eq!(first.await.unwrap().unwrap().status(), StatusCode::OK);
    release.notify_one();
    assert_eq!(
        app.oneshot(request("/work", "127.0.0.2"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn json_rejections_and_method_errors_use_the_error_envelope() {
    let app = router(
        Router::new().route(
            "/json",
            post(|Json(value): Json<serde_json::Value>| async move { Json(value) }),
        ),
        &Config::default(),
    );
    let mut req = request("/json", "127.0.0.1");
    *req.method_mut() = Method::POST;
    *req.body_mut() = Body::from("{broken");
    req.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap()["error"]["code"],
        "bad_request"
    );
    let response = app.oneshot(request("/json", "127.0.0.1")).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(response.headers().contains_key(header::ALLOW));
}

#[tokio::test]
async fn root_welcomes_clients_and_only_advertises_enabled_docs() {
    for enabled in [true, false] {
        let mut config = Config::default();
        config.docs.enabled = enabled;
        config.static_files.enabled = enabled;
        config.static_files.mount = "/assets".into();
        let app = app(&config);
        let response = app.clone().oneshot(request("/", "127.0.0.1")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["status"], "ok");
        assert!(body.get("error").is_none());
        assert_eq!(body["address"], config.bind_address.to_string());
        assert_eq!(body["url"], format!("http://{}", config.bind_address));
        if enabled {
            assert_eq!(body["openapi"], "/docs/openapi.json");
            assert_eq!(body["versions"][0]["documentation"], "/docs/v1/");
            assert_eq!(body["versions"][0]["openapi"], "/docs/v1/openapi.json");
            assert_eq!(body["static_files"], "/assets");
        } else {
            assert!(body["openapi"].is_null());
            assert!(body["versions"].as_array().unwrap().is_empty());
            assert!(body["static_files"].is_null());
        }

        if enabled { assert_eq!(body["documentation"], "/docs/"); }
        else { assert!(body["documentation"].is_null()); }
        assert_eq!(app.oneshot(request("/still-missing", "127.0.0.1")).await.unwrap().status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn root_omits_version_links_when_versioning_is_disabled() {
    let mut config = Config::default();
    config.versioning_enabled = false;
    let response = app(&config).oneshot(request("/", "127.0.0.1")).await.unwrap();
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["documentation"], "/docs/");
    assert!(body["versions"].as_array().unwrap().is_empty());
}
