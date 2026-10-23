//! Serve only regular public assets; reject hidden files and symlink traversal.

use axum::{Router, extract::{Request, State}, middleware::{self, Next}, response::{IntoResponse, Response}, http::StatusCode};
use crate::configs::StaticConfig;
use std::{path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;

/// Validate the public directory before binding the listener.
pub fn validate(config: &StaticConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !config.enabled { return Ok(()); }
    let metadata = std::fs::symlink_metadata(&config.directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("The public static directory must be a real public directory, not a symlink".into());
    }
    if config.directory.canonicalize()? == std::env::current_dir()?.canonicalize()? {
        return Err("The public static directory must not expose the application root".into());
    }
    Ok(())
}

/// Mount static files before global middleware so they receive the same policies.
pub fn mount(router: Router, config: &StaticConfig) -> Router {
    if !config.enabled { return router; }
    let assets = Router::new()
        .nest_service(&config.mount, ServeDir::new(&config.directory).append_index_html_on_directories(false))
        .route_layer(middleware::from_fn_with_state(Arc::new(config.clone()), restrict));
    router.merge(assets)
}

async fn restrict(State(config): State<Arc<StaticConfig>>, mut request: Request, next: Next) -> Response {
    let raw = request.uri().path().strip_prefix(&config.mount).unwrap_or("");
    let Ok(path) = percent_encoding::percent_decode_str(raw).decode_utf8() else { return StatusCode::NOT_FOUND.into_response(); };
    let mut current: PathBuf = config.directory.clone();
    // Check the configured root too, including changes made after startup.
    if !allowed(&current).await { return StatusCode::NOT_FOUND.into_response(); }
    for component in path.split('/').filter(|part| !part.is_empty()) {
        if component.starts_with('.') || component.contains(['\\', ':', '\0']) {
            return StatusCode::NOT_FOUND.into_response();
        }
        current.push(component);
        if !allowed(&current).await { return StatusCode::NOT_FOUND.into_response(); }
    }
    // Serve the explicit landing example for both mount URLs, without directory listings.
    if path.is_empty() || path == "/" {
        if !tokio::fs::symlink_metadata(config.directory.join("index.html")).await
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink()) {
            return StatusCode::NOT_FOUND.into_response();
        }
        let mut parts = request.uri().clone().into_parts();
        let target = format!("{}/index.html", config.mount);
        let target = match request.uri().query() {
            Some(query) => format!("{target}?{query}"),
            None => target,
        };
        parts.path_and_query = Some(target.parse().expect("valid static mount path"));
        *request.uri_mut() = axum::http::Uri::from_parts(parts).expect("valid static asset URI");
    }
    next.run(request).await
}

async fn allowed(path: &std::path::Path) -> bool {
    tokio::fs::symlink_metadata(path).await.is_ok_and(|m|
        !m.file_type().is_symlink() && (m.is_dir() || m.is_file()))
}
