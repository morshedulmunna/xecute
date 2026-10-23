//! Probe endpoints; extend readiness with real dependency checks when adding services.

use axum::Json;

/// Confirm the initialized server can respond; this does not check external dependencies.
pub(super) async fn check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Welcome response for the application root.
pub(super) async fn welcome(docs_enabled: bool) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "status": "ok",
        "message": "Welcome to Xecute",
        "documentation": if docs_enabled { Some("/docs/") } else { None },
    }))
}
