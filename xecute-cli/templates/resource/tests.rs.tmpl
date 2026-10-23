//! Verify payload validation and repository persistence through HTTP endpoints.

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

/// Invalid payloads never enter storage; valid payloads return 201 and are retrievable.
#[tokio::test]
async fn creation_validates_before_persisting() {
    let app = crate::app_module::AppModule::router().unwrap();
    for (name, expected) in [
        ("ab", StatusCode::UNPROCESSABLE_ENTITY),
        ("   ", StatusCode::UNPROCESSABLE_ENTITY),
        ("Example", StatusCode::CREATED),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/__FEATURE__")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::json!({"name": name}).to_string()))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(request).await.unwrap().status(),
            expected
        );
    }
    // Decoding and validation failures must not reach the command/service or storage.
    for (payload, expected) in [
        ("not-json", StatusCode::BAD_REQUEST),
        (r#"{}"#, StatusCode::UNPROCESSABLE_ENTITY),
        (r#"{"name":42}"#, StatusCode::UNPROCESSABLE_ENTITY),
        (r#"{"name":"Example","unexpected":true}"#, StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/__FEATURE__")
            .header("content-type", "application/json")
            .body(Body::from(payload))
            .unwrap();
        assert_eq!(app.clone().oneshot(request).await.unwrap().status(), expected);
    }
    let response = app
        .oneshot(
            Request::builder()
                .uri("/__FEATURE__")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let records: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);
    assert_eq!(records[0]["name"], "Example");
}
