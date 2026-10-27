---
title: "Testing modules and HTTP"
description: "Override providers and exercise real routers in isolated tests."
---

Test HTTP behavior through a real router and replace providers when a test needs controlled data. Start with the generated feature tests, then use overrides for external dependencies.

Enable the runtime's optional test helpers only for development:

```toml
[dev-dependencies]
xecute = { version = "0.1.8", features = ["testing"] }
```

Generated manifests already include this entry. `TestingModule` compiles isolated
containers, applies overrides before initialization, and validates the final
dependency graph. Unknown or repeated overrides are errors, preventing a silently
unused mock. Values replace the same registered Rust type; for trait-based mocks,
use a concrete adapter containing `Arc<dyn YourTrait>` and replace that adapter.
Production containers remain unchanged.

```rust
use std::sync::Arc;
use xecute::testing::{TestApp, TestingModule};

let mut module = TestingModule::new();
for definition in crate::app_module::AppModule::definitions() {
    module = module.module(definition);
}
// Example: module = module.override_value(DatabaseGateway(Arc::new(FakeDatabase)));
let container = Arc::new(module.compile_async().await?);
let routes = crate::app_module::AppModule::router_with_container(container.clone()).await?;
let app = TestApp::new(crate::server::router(routes, &crate::configs::Config::default()));

let response = app.post("/example")
    .json(&serde_json::json!({"name": "Example"}))
    .send().await?
    .assert_status(axum::http::StatusCode::CREATED);
let body: serde_json::Value = response.json()?;
container.shutdown_async().await?;
```

Run these async helpers inside a Tokio runtime, such as `#[tokio::test]`.
The client runs the actual Router and middleware without a socket. It supports
arbitrary HTTP methods through `.request`, common method helpers, raw bodies,
headers, JSON payloads, and `.peer(SocketAddr)` for limiter tests. Loopback peer
information is supplied by default. Responses expose status, headers, bounded
bytes, typed JSON, and chainable `.assert_status`/`.assert_json` methods. Invalid
requests return test errors rather than panicking. Request and body collection
have a five-second timeout and a 1 MiB response limit; adjust with
`TestApp::timeout` and `response_limit`. This tests HTTP behavior in-process;
network/TLS/deployment behavior still belongs in deployment smoke tests.

The generated `src/tests/application.rs` demonstrates isolated container startup
and safe mapped errors. Resource tests demonstrate JSON validation and storage
through the same test client. The runtime test suite also covers trait adapters,
mock isolation, concurrent initialization, nested request scopes, and cleanup.

## Keep learning

[Runnable examples](/docs/testing/examples). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#testing-modules-and-http-assertions).
