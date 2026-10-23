//! Exercise receiver-based routes, automatic validation, and OpenAPI contracts.

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::ServiceExt;
use validator::Validate;
use xecute::prelude::*;

#[derive(Deserialize, Validate, JsonSchema)]
struct Payload {
    #[validate(length(min = 3))]
    name: String,
}

#[derive(Serialize, JsonSchema)]
struct Reply {
    name: String,
}

#[controller("/v1/test")]
#[derive(Injectable)]
struct TestController {
    #[inject(default)]
    calls: std::sync::atomic::AtomicUsize,
}

#[routes]
impl TestController {
    #[post("/create")]
    #[api(summary = "Create", tag = "Tests", operation_id = "createTest")]
    #[body(json = Payload)]
    #[resp(200, "Created", json = Reply)]
    #[resp(422, "Invalid payload")]
    async fn create(&self, #[body] request: Payload) -> Result<Json<Reply>, HttpError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(Json(Reply { name: request.name }))
    }

    #[get("/search")]
    #[resp(200, "Query result", json = Reply)]
    async fn search(&self, #[query] request: Payload) -> Json<Reply> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Json(Reply { name: request.name })
    }
}

#[tokio::test]
async fn receiver_handlers_validate_before_running() {
    let controller = xecute::Container::builder()
        .injectable::<TestController>()
        .build()
        .unwrap()
        .resolve()
        .unwrap();
    let app = TestController::router(Arc::clone(&controller));
    for (payload, expected) in [
        (r#"{"name":"ab"}"#, 422),
        (r#"{"name":"Valid"}"#, 200),
        (r#"{}"#, 422),
        (r#"{"name":42}"#, 422),
        ("invalid", 400),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/v1/test/create")
            .header("content-type", "application/json")
            .body(Body::from(payload))
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), expected);
        if expected == 200 {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(
                    &to_bytes(response.into_body(), 4096).await.unwrap()
                )
                .unwrap()["name"],
                "Valid"
            );
        }
    }
    for (path, expected) in [
        ("/v1/test/search?name=ab", 422),
        ("/v1/test/search?name=Valid", 200),
        ("/v1/test/search", 400),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status()
                .as_u16(),
            expected
        );
    }
    assert_eq!(
        controller.calls.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
}

#[test]
fn openapi_includes_routes_payloads_and_response_contracts() {
    let spec = xecute::openapi_document("Test", "1", TestController::api_operations()).unwrap();
    assert_eq!(spec["openapi"], "3.1.0");
    let create = &spec["paths"]["/v1/test/create"]["post"];
    assert_eq!(create["operationId"], "createTest");
    assert_eq!(
        create["requestBody"]["content"]["application/json"]["schema"]["properties"]["name"]["type"],
        "string"
    );
    assert!(
        create["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["name"]
            .is_object()
    );
    assert!(
        xecute::openapi_document(
            "Test",
            "1",
            TestController::api_operations()
                .into_iter()
                .chain(TestController::api_operations())
                .collect()
        )
        .is_err()
    );
}

#[derive(Injectable)]
struct LegacyController {}

#[xecute::controller("/legacy")]
impl LegacyController {
    #[get("")]
    async fn list() -> Json<Vec<String>> {
        Json(Vec::new())
    }
}

#[tokio::test]
async fn original_impl_controller_syntax_remains_supported() {
    let app = LegacyController::router(Arc::new(LegacyController {}));
    assert_eq!(
        app.oneshot(
            Request::builder()
                .uri("/legacy")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap()
        .status(),
        HttpStatus::OK
    );
}
