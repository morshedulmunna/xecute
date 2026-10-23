//! Shared environment access for application configuration.

/// Load optional .env settings without overriding existing process variables.
/// A missing file is valid; malformed files stop startup.
pub fn load_env() -> Result<(), String> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(_) => Err("Could not load .env; check file syntax and permissions".into()),
    }
}

// Reuse the runtime boundary for all environment access and typed parsing.
pub use xecute::config::{ConfigError, ConfigFile, Env, read_env};
