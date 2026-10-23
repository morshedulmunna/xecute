//! Register application middleware in request execution order.
//!
//! Add async functions accepting `xecute::middleware::{Request, Next}` and returning
//! a response, then chain `.use_middleware(your_function)` below. Return early to
//! reject a request, or call `next.run(request).await` to continue.

use xecute::Application;

/// Configure middleware for all application routes, probes, docs, and fallback.
/// The server's request logging and response normalization wrap this pipeline.
pub fn configure(app: Application) -> Application {
    app
}
