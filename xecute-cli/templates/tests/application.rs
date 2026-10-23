//! Build the real application with an isolated DI container and test public errors.

use axum::{Router, http::StatusCode, routing::get};
use std::sync::Arc;
use xecute::{HttpError, exceptions::ErrorEnvelope, testing::{TestApp, TestingModule}};

#[tokio::test]
async fn application_can_use_an_isolated_testing_container() {
    let mut module = TestingModule::new();
    for definition in crate::app_module::AppModule::definitions() {
        module = module.module(definition);
    }
    let container = Arc::new(module.compile().unwrap());
    let routes = crate::app_module::AppModule::router_with_container(container).unwrap();
    let app = TestApp::new(crate::server::router(routes, &crate::configs::Config::default()));
    let _ = app.get("/").send().await.unwrap().assert_status(StatusCode::OK);
}

#[tokio::test]
async fn public_error_mapping_preserves_correlation_and_redacts_internal_failures() {
    let routes = Router::new()
        .route("/expected", get(|| async { HttpError::public(StatusCode::CONFLICT, "record_exists", "Record already exists") }))
        .route("/unexpected", get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "private database error") }));
    let app = TestApp::new(crate::server::router(routes, &crate::configs::Config::default()));
    for (path, status, code) in [
        ("/expected", StatusCode::CONFLICT, "record_exists"),
        ("/unexpected", StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    ] {
        let response = app.get(path).send().await.unwrap().assert_status(status);
        let body: ErrorEnvelope = response.json().unwrap();
        assert_eq!(body.error.code, code);
        assert_eq!(body.error.request_id, response.headers["x-request-id"].to_str().unwrap());
        assert!(!body.error.message.contains("database"));
    }
}
