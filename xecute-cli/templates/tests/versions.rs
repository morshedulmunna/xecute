//! Real versioned routes and independent Swagger documents use the same registration.
use super::support::request;
use crate::{app_module::AppModule, configs::Config, server};
use axum::{Router, body::to_bytes, http::StatusCode};
use tower::ServiceExt;

fn app(config: &Config) -> Router {
    let base = AppModule::router().unwrap();
    let operations = AppModule::api_operations();
    let versions = server::versions::groups(config, base.clone(), operations.clone()).unwrap();
    server::router(versions.mount(base)
        .merge(server::docs::router(config, operations).unwrap())
        .merge(server::docs::versioned_router(config, &versions).unwrap()), config)
}
#[tokio::test]
async fn versions_have_independent_routes_schemas_and_local_swagger_assets() {
    let app = app(&Config::default());
    for (path, status) in [
        ("/v1/example", StatusCode::OK), ("/v2/status", StatusCode::OK),
        ("/v2/example", StatusCode::NOT_FOUND), ("/v1/status", StatusCode::NOT_FOUND),
        ("/docs/v1/", StatusCode::OK), ("/docs/v2/", StatusCode::OK),
        ("/docs/v1/swagger-ui-bundle.js", StatusCode::OK), ("/docs/v2/swagger-ui-bundle.js", StatusCode::OK),
    ] {
        assert_eq!(app.clone().oneshot(request(path, "127.0.0.1")).await.unwrap().status(), status, "{path}");
    }
    for (version, expected, absent) in [("v1", "/v1/example", "/v2/status"), ("v2", "/v2/status", "/v1/example")] {
        let response = app.clone().oneshot(request(&format!("/docs/{version}/openapi.json"), "127.0.0.1")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let document: serde_json::Value = serde_json::from_slice(&to_bytes(response.into_body(), 1048576).await.unwrap()).unwrap();
        assert!(document["paths"][expected].is_object());
        assert!(document["paths"][absent].is_null());
        assert_eq!(document["info"]["version"], version);
        serde_json::from_value::<utoipa::openapi::OpenApi>(document).unwrap();
    }
}
#[tokio::test]
async fn docs_and_versioning_can_be_disabled_independently() {
    let config = Config { versioning_enabled: false, ..Config::default() };
    let disabled = app(&config);
    assert_eq!(disabled.oneshot(request("/v1/example", "127.0.0.1")).await.unwrap().status(), StatusCode::NOT_FOUND);
    let mut config = Config::default(); config.docs.enabled = false;
    let disabled_docs = app(&config);
    assert_eq!(disabled_docs.clone().oneshot(request("/v1/example", "127.0.0.1")).await.unwrap().status(), StatusCode::OK);
    assert_eq!(disabled_docs.oneshot(request("/docs/v1/openapi.json", "127.0.0.1")).await.unwrap().status(), StatusCode::NOT_FOUND);
}
