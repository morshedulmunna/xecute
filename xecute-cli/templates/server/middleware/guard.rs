//! Per-peer quotas, global handler concurrency, and request deadlines.
//! Limits cover handler work through response headers, not streaming body completion.

use super::request_log::RequestId;
use crate::{
    configs::Config,
    server::{error, rate_limit::RateLimiter},
};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Semaphore;

/// Shared quotas and permits; cloned routers enforce the same process-wide limits.
pub(crate) struct GuardState {
    limiter: RateLimiter,
    permits: Arc<Semaphore>,
    timeout: std::time::Duration,
}

/// Reject excess work before invoking handlers and cancel handlers past their deadline.
pub(crate) async fn guard(
    State(state): State<Arc<GuardState>>,
    request: Request,
    next: Next,
) -> Response {
    let healthy = request.method() == Method::GET
        && matches!(request.uri().path(), "/health/live" | "/health/ready");
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .map(|id| id.0.as_str())
        .unwrap_or("");
    let permit = if healthy {
        None
    } else {
        // Use the actual socket peer. Never trust arbitrary X-Forwarded-For headers.
        let Some(ConnectInfo(peer)) = request.extensions().get::<ConnectInfo<SocketAddr>>() else {
            tracing::error!("socket peer missing; use into_make_service_with_connect_info");
            return error::response(StatusCode::INTERNAL_SERVER_ERROR, request_id);
        };
        if let Err(retry) = state.limiter.check(peer.ip()) {
            let mut response = error::response(StatusCode::TOO_MANY_REQUESTS, request_id);
            response.headers_mut().insert(
                header::RETRY_AFTER,
                HeaderValue::from_str(&retry.to_string()).unwrap(),
            );
            return response;
        }
        match state.permits.clone().try_acquire_owned() {
            Ok(permit) => Some(permit),
            Err(_) => {
                let mut response = error::response(StatusCode::SERVICE_UNAVAILABLE, request_id);
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
                return response;
            }
        }
    };
    // The permit covers handler work up to response headers, not streaming bodies.
    let id = request_id.to_owned();
    let response = match tokio::time::timeout(state.timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => error::response(StatusCode::GATEWAY_TIMEOUT, &id),
    };
    drop(permit);
    response
}

/// Build shared policy state once when the application router is assembled.
pub(crate) fn state(config: &Config) -> Arc<GuardState> {
    Arc::new(GuardState {
        limiter: RateLimiter::new(
            config.rate_limit_requests,
            config.rate_limit_window,
            config.rate_limit_max_clients,
        ),
        permits: Arc::new(Semaphore::new(config.max_in_flight)),
        timeout: config.request_timeout,
    })
}
