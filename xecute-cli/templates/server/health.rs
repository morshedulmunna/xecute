//! Probe endpoints; extend readiness with real dependency checks when adding services.

use axum::Json;

/// Confirm the initialized server can respond; this does not check external dependencies.
pub(super) async fn check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
