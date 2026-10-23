//! Runtime configuration boundary: environment loading and typed settings.

mod cors;
mod docs;
mod logging;
mod server;

pub use cors::CorsConfig;
pub use docs::DocsConfig;
pub use logging::LoggingConfig;
pub use server::Config;

/// Load optional .env settings without overriding existing process variables.
/// A missing file is valid; malformed files stop startup.
pub fn load_env() -> Result<(), dotenvy::Error> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(error),
    }
}
