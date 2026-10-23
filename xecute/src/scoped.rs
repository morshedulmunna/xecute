//! Axum integration for one dependency-resolution context per HTTP request.

use crate::{Container, HttpError, HttpStatus, RequestScope};
use axum::{
    Router,
    extract::{FromRequestParts, Request, State},
    http::request::Parts,
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use std::sync::Arc;

/// Extract a provider using the request's shared dependency scope.
/// Install [`with_container`] once on assembled routes, or use a generated `scoped_router`.
pub struct Inject<T>(pub Arc<T>);
impl<S, T> FromRequestParts<S> for Inject<T>
where
    S: Send + Sync,
    T: Send + Sync + 'static,
{
    type Rejection = HttpError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestScope>()
            .ok_or_else(|| HttpError::new(HttpStatus::INTERNAL_SERVER_ERROR))?
            .resolve::<T>()
            .map(Self)
            .map_err(|_| HttpError::new(HttpStatus::INTERNAL_SERVER_ERROR))
    }
}

/// Create a fresh DI cache for each request before running controller middleware/extractors.
/// Singleton caches are shared across requests; dropping a request releases its cached values.
pub fn with_container(router: Router, container: Arc<Container>) -> Router {
    router.layer(middleware::from_fn_with_state(container, attach))
}
async fn attach(
    State(container): State<Arc<Container>>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(scope) = request.extensions().get::<RequestScope>() {
        if !scope.belongs_to(&container) {
            return HttpError::new(HttpStatus::INTERNAL_SERVER_ERROR).into_response();
        }
    } else {
        request.extensions_mut().insert(container.request());
    }
    next.run(request).await
}
