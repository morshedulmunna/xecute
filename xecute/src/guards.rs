//! Guards inspect or enrich a request before extraction and handler execution.
//! A guard is an async function returning `Result<Request, E>` where E implements
//! IntoResponse + Send. It has no Next argument and cannot accidentally call the handler.
//! Guards must leave the request body intact for subsequent extraction.

use crate::middleware::{Next, Request, Response};
use axum::response::IntoResponse;
use axum::{Router, routing::MethodRouter};
use std::future::Future;

async fn run<F, Fut, E>(guard: F, request: Request, next: Next) -> Response
where
    F: Fn(Request) -> Fut,
    Fut: Future<Output = Result<Request, E>>,
    E: IntoResponse + Send,
{
    match guard(request).await {
        Ok(request) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

/// Protect an application router, including fallback responses.
pub fn apply<F, Fut, E>(router: Router, guard: F) -> Router
where
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<Request, E>> + Send + 'static,
    E: IntoResponse + Send + 'static,
{
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| run(guard.clone(), request, next),
    ))
}

/// Protect matching controller routes while preserving unrelated 404 responses.
#[doc(hidden)]
pub fn apply_routes<F, Fut, E>(router: Router, guard: F) -> Router
where
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<Request, E>> + Send + 'static,
    E: IntoResponse + Send + 'static,
{
    router.route_layer(axum::middleware::from_fn(
        move |request: Request, next: Next| run(guard.clone(), request, next),
    ))
}

/// Protect one declared HTTP method before body extraction.
#[doc(hidden)]
pub fn apply_method<S, F, Fut, E>(router: MethodRouter<S>, guard: F) -> MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
    F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Result<Request, E>> + Send + 'static,
    E: IntoResponse + Send + 'static,
{
    router.route_layer(axum::middleware::from_fn(
        move |request: Request, next: Next| run(guard.clone(), request, next),
    ))
}
