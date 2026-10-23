//! HTTP runtime boundary. Exposes startup and router construction to the application.

mod bootstrap;
pub(crate) mod docs;
mod error;
mod health;
pub(crate) mod logging;
mod middleware;
pub(crate) mod rate_limit;
mod router;
mod shutdown;
pub(crate) mod static_files;

pub use bootstrap::run;
pub use router::router;
