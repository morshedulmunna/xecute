//! Swagger, OpenAPI contracts, and documentation-specific browser policies.

use axum::{body::to_bytes, http::StatusCode};
use tower::ServiceExt;

use super::support::request;
use crate::{app_module::AppModule, configs::Config, server};

#[tokio::test]
async fn default_swagger_documents_the_example_and_serves_local_assets() {
    let config = Config::default();
    let app = server::router(
        AppModule::router()
            .await
            .unwrap()
            .merge(server::docs::router(&config, AppModule::api_operations()).unwrap()),
        &config,
    );

    let response = app
        .clone()
        .oneshot(request("/docs/openapi.json", "127.0.0.1"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(document["openapi"], "3.1.0");
    serde_json::from_value::<utoipa::openapi::OpenApi>(document.clone()).unwrap();
    let create = &document["paths"]["/example"]["post"];
    assert_eq!(create["operationId"], "createExample");
    let name =
        &create["requestBody"]["content"]["application/json"]["schema"]["properties"]["name"];
    assert_eq!(name["minLength"], 3);
    assert!(create["responses"]["201"]["content"]["application/json"].is_object());

    for path in [
        "/docs/",
        "/docs/swagger-ui-bundle.js",
        "/docs/swagger-initializer.js",
    ] {
        let response = app
            .clone()
            .oneshot(request(path, "127.0.0.1"))
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "missing Swagger asset: {path}"
        );
        assert!(
            response.headers()["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("script-src 'self'")
        );
    }

    let response = app.oneshot(request("/example", "127.0.0.1")).await.unwrap();
    assert_eq!(
        response.headers()["content-security-policy"],
        "default-src 'none'; frame-ancestors 'none'"
    );
}

#[tokio::test]
async fn disabling_swagger_removes_ui_and_spec_routes() {
    let mut config = Config::default();
    config.docs.enabled = false;
    let app = server::router(
        server::docs::router(&config, AppModule::api_operations()).unwrap(),
        &config,
    );
    for path in ["/docs/", "/docs/openapi.json"] {
        assert_eq!(
            app.clone()
                .oneshot(request(path, "127.0.0.1"))
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
    }
    assert!(
        Config::from_config(&crate::configs::ConfigFile::parse("[docs]\nenabled = \"invalid\"").unwrap())
            .is_err()
    );
}
