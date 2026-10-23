//! Ordered HTTP middleware for applications, controllers, and routes.
//!
//! Middleware functions accept a request and `Next`, returning an HTTP response.
//! Call `next.run(request).await` to continue or return a response to stop execution.

use axum::{Router, response::IntoResponse};
pub use axum::{extract::Request, middleware::Next, response::Response};
use std::future::Future;

/// Attach one middleware function to the existing routes and fallback.
/// Used by controller macros; application code can use [`Application`].
pub fn apply<F, Fut, R>(router: Router, middleware: F) -> Router
where
    F: Fn(Request, Next) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse + 'static,
{
    router.layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let middleware = middleware.clone();
            async move { middleware(request, next).await }
        },
    ))
}

/// Attach controller middleware only to matching routes, preserving unrelated 404s.
#[doc(hidden)]
pub fn apply_routes<F, Fut, R>(router: Router, middleware: F) -> Router
where
    F: Fn(Request, Next) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse + 'static,
{
    router.route_layer(axum::middleware::from_fn(
        move |request: Request, next: Next| {
            let middleware = middleware.clone();
            async move { middleware(request, next).await }
        },
    ))
}

/// Application HTTP pipeline. Middleware runs in registration order.
///
/// Register all application routes first, then finalize with `into_router`.
/// Middleware also wraps the supplied router's fallback responses.
pub struct Application {
    router: Router,
    middleware: Vec<Box<dyn FnOnce(Router) -> Router + Send>>,
}

impl Application {
    /// Create an application pipeline around an assembled router.
    pub fn new(router: Router) -> Self {
        Self {
            router,
            middleware: Vec::new(),
        }
    }

    /// Register a global middleware function or a closure capturing shared state.
    /// The first registered middleware receives the request first.
    pub fn use_middleware<F, Fut, R>(mut self, middleware: F) -> Self
    where
        F: Fn(Request, Next) -> Fut + Clone + Send + Sync + 'static,
        Fut: Future<Output = R> + Send + 'static,
        R: IntoResponse + 'static,
    {
        self.middleware
            .push(Box::new(move |router| apply(router, middleware)));
        self
    }

    /// Build the router with all registered middleware in declaration order.
    pub fn into_router(self) -> Router {
        self.middleware
            .into_iter()
            .rev()
            .fold(self.router, |router, wrap| wrap(router))
    }
}
