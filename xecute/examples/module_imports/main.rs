//! Run: cargo run -p xecute --example module_imports
//! See controller.rs for request input examples and curl commands.

mod controller;
mod modules;

use controller::ExampleController;
use modules::{ExampleModule, GreetingModule};
use std::sync::Arc;
use xecute::ModuleGraph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // imports declares visibility; every module definition must also be registered.
    let container = ModuleGraph::new(vec![
        GreetingModule::definition(),
        ExampleModule::definition(),
    ])
    .build()?;
    container.initialize()?;
    let controller: Arc<ExampleController> = container.resolve()?;
    let app = ExampleController::router(controller);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Request examples: http://127.0.0.1:3000/examples");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn imported_service_and_request_inputs_work() {
        let container = ModuleGraph::new(vec![
            GreetingModule::definition(),
            ExampleModule::definition(),
        ])
        .build()
        .unwrap();
        container.initialize().unwrap();
        let app = ExampleController::router(container.resolve().unwrap());
        for (method, uri, content_type, body, status, expected) in [
            (
                "POST",
                "/examples",
                "application/json",
                r#"{"name":"Munna"}"#,
                200,
                r#""Hello, Munna!""#,
            ),
            (
                "POST",
                "/examples",
                "application/json",
                r#"{"name":""}"#,
                422,
                "",
            ),
            (
                "GET",
                "/examples/42?limit=5",
                "text/plain",
                "",
                200,
                r#"{"id":42,"limit":5,"request_id":"demo"}"#,
            ),
            ("GET", "/examples/42?limit=0", "text/plain", "", 422, ""),
            (
                "POST",
                "/examples/form",
                "application/x-www-form-urlencoded",
                "name=Munna",
                200,
                r#""Hello, Munna!""#,
            ),
            (
                "POST",
                "/examples/text",
                "text/plain",
                "hello",
                200,
                "hello",
            ),
            (
                "POST",
                "/examples/bytes",
                "application/octet-stream",
                "hello",
                200,
                "5",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("content-type", content_type)
                        .header("x-request-id", "demo")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), status, "{method} {uri}");
            if status == 200 {
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert_eq!(
                    std::str::from_utf8(&bytes).unwrap(),
                    expected,
                    "{method} {uri}"
                );
            }
        }
    }
}
