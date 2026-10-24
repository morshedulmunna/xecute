//! Probe endpoints; extend readiness with real dependency checks when adding services.

use axum::Json;

/// Confirm the initialized server can respond; this does not check external dependencies.
pub(super) async fn check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Discover enabled public endpoints without repeating them in startup logs.
pub(super) fn welcome(config: &crate::configs::Config) -> Json<serde_json::Value> {
    let versions = if config.docs.enabled && config.versioning_enabled {
        vec![serde_json::json!({
            "version": "v1",
            "documentation": "/docs/v1/",
            "openapi": "/docs/v1/openapi.json",
        })]
    } else {
        Vec::new()
    };
    Json(serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "status": "ok",
        "message": "Welcome to Xecute",
        "address": config.bind_address.to_string(),
        "url": super::bootstrap::local_url(config.bind_address),
        "documentation": if config.docs.enabled { Some("/docs/") } else { None },
        "openapi": if config.docs.enabled { Some("/docs/openapi.json") } else { None },
        "versions": versions,
        "static_files": if config.static_files.enabled { Some(config.static_files.mount.as_str()) } else { None },
    }))
}
