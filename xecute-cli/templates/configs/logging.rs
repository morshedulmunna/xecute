//! Logging filter configuration from RUST_LOG, with an info-level default.

use tracing_subscriber::EnvFilter;

/// The validated tracing filter used by the server logger.
pub struct LoggingConfig {
    pub filter: EnvFilter,
}

impl LoggingConfig {
    /// Parse RUST_LOG directives; invalid directives return a startup error.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let filter = match std::env::var("RUST_LOG") {
            Ok(value) => EnvFilter::try_new(value)?,
            Err(std::env::VarError::NotPresent) => EnvFilter::new("info"),
            Err(error) => return Err(error.into()),
        };
        Ok(Self { filter })
    }
}
