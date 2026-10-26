//! Exercise the actual application router through the in-process test client.

use axum::http::StatusCode;
use xecute::testing::TestApp;

#[tokio::test]
async fn creation_validates_before_persisting() {
    let app = TestApp::new(crate::app_module::AppModule::router().unwrap());
    for (name, expected) in [
        ("ab", StatusCode::UNPROCESSABLE_ENTITY),
        ("   ", StatusCode::UNPROCESSABLE_ENTITY),
        ("  Example  ", StatusCode::CREATED),
    ] {
        let _ = app.post("/__FEATURE__").json(&serde_json::json!({"name": name}))
            .send().await.unwrap().assert_status(expected);
    }
    for (payload, expected) in [
        ("not-json", StatusCode::BAD_REQUEST),
        (r#"{}"#, StatusCode::UNPROCESSABLE_ENTITY),
        (r#"{"name":42}"#, StatusCode::UNPROCESSABLE_ENTITY),
        (r#"{"name":"Example","unexpected":true}"#, StatusCode::UNPROCESSABLE_ENTITY),
    ] {
        let _ = app.post("/__FEATURE__").header("content-type", "application/json")
            .body(payload).send().await.unwrap().assert_status(expected);
    }
    let response = app.get("/__FEATURE__").send().await.unwrap().assert_status(StatusCode::OK);
    let records: serde_json::Value = response.json().unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);
    assert_eq!(records[0]["name"], "Example");
}
