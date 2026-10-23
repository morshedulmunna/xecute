//! Runtime configuration boundary: environment loading and typed settings.

mod cors;
mod docs;
mod environment;
pub(crate) mod logging;
mod static_files;
mod server;

pub use cors::CorsConfig;
pub use docs::DocsConfig;
pub use environment::{load_env, read_env};
pub use static_files::StaticConfig;
pub use logging::LoggingConfig;
pub use server::Config;
