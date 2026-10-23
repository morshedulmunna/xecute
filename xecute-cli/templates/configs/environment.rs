//! Shared environment access for application configuration.

/// Load optional .env settings without overriding existing process variables.
/// A missing file is valid; malformed files stop startup.
pub fn load_env() -> Result<(), dotenvy::Error> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(error),
    }
}

/// Read a setting after `load_env`, preserving empty strings and reporting invalid UTF-8.
/// Missing settings return `None` so each configuration can supply its own default.
pub fn read_env(key: &str) -> Result<Option<String>, String> {
    match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{key} must be valid UTF-8")),
    }
}
