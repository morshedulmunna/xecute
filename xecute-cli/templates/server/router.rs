//! HTTP route assembly and middleware order. Later layers wrap earlier layers.

use super::{health, middleware};
use crate::configs::Config;
use axum::{
    Router, extract::DefaultBodyLimit, http::StatusCode, middleware as axum_middleware,
    response::IntoResponse, routing::get,
};
use tower_http::{catch_panic::CatchPanicLayer, cors::CorsLayer, limit::RequestBodyLimitLayer};

/// Apply shared policies to all resource routes, including fallback responses.
/// Completion logging wraps CORS, guards, panic handling, and body limits.
pub fn router(app: Router, config: &Config) -> Router {
    // Share quota and concurrency state across all requests handled by this router.
    let state = middleware::state(config);

    // Use the validated environment policy for browser cross-origin requests.
    let cors = CorsLayer::new()
        .allow_origin(config.cors.origins.clone())
        .allow_methods(config.cors.methods.clone())
        .allow_headers(config.cors.allowed_headers.clone())
        .expose_headers(config.cors.exposed_headers.clone())
        .allow_credentials(config.cors.allow_credentials)
        .max_age(config.cors.max_age);

    // Register probes and a fallback before layering policies over every route.
    // Requests enter the layers below in reverse order of their registration.
    app.route("/health/live", get(health::check))
        .route("/health/ready", get(health::check))
        .fallback(|| async { StatusCode::NOT_FOUND })
        // Bound extractor buffering and streamed request bodies.
        .layer(DefaultBodyLimit::max(config.body_limit))
        .layer(RequestBodyLimitLayer::new(config.body_limit))
        // Convert handler panics into safe errors for the response normalizer.
        .layer(CatchPanicLayer::custom(|_| {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }))
        // Enforce per-peer quotas, concurrency limits, and handler deadlines.
        .layer(axum_middleware::from_fn_with_state(
            state,
            middleware::guard,
        ))
        // Handle browser preflights before entering the resource guards.
        .layer(cors)
        // Outermost: rejection responses and preflights also receive IDs and logs.
        .layer(axum_middleware::from_fn(middleware::observe))
}
