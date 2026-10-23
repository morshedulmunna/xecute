//! Validated server settings; invalid limits fail startup rather than disable protection.

use super::{ConfigError, ConfigFile, CorsConfig, DocsConfig, LoggingConfig, StaticConfig, UploadConfig};
use std::{net::SocketAddr, time::Duration};

/// Typed, validated HTTP server configuration. Durations use monotonic clocks.
#[derive(Clone, Debug)]
pub struct Config {
    /// Listener IP and port; loopback by default, 0.0.0.0 for containers.
    pub bind_address: SocketAddr,
    /// Maximum handler duration before returning 504.
    pub request_timeout: Duration,
    /// Maximum time allowed for in-flight work to drain after shutdown.
    pub shutdown_grace: Duration,
    /// Maximum consumed request body size, in bytes.
    pub body_limit: usize,
    /// Maximum concurrent handlers; excess work receives 503.
    pub max_in_flight: usize,
    /// Allowed requests per peer IP within one fixed window.
    pub rate_limit_requests: u32,
    /// Monotonic duration of each per-IP quota window.
    pub rate_limit_window: Duration,
    /// Memory bound on tracked peer IPs; active counters are not evicted.
    pub rate_limit_max_clients: usize,
    /// Browser cross-origin policy validated from TOML arrays.
    pub cors: CorsConfig,
    /// Whether API documentation is exposed.
    pub docs: DocsConfig,
    /// Dedicated public asset directory and mount prefix.
    pub static_files: StaticConfig,
    /// Validated local logging destinations and event filter.
    pub logging: LoggingConfig,
    /// Enable explicitly registered /v1 and /v2 URL groups.
    pub versioning_enabled: bool,
    /// Private upload backend and streaming validation limits.
    pub uploads: UploadConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:3000".parse().unwrap(),
            request_timeout: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(10),
            body_limit: 1_048_576,
            max_in_flight: 256,
            rate_limit_requests: 60,
            rate_limit_window: Duration::from_secs(60),
            rate_limit_max_clients: 10_000,
            cors: CorsConfig::default(),
            docs: DocsConfig::default(),
            static_files: StaticConfig::default(),
            logging: LoggingConfig::default(),
            versioning_enabled: true,
            uploads: UploadConfig::default(),
        }
    }
}

impl Config {
    /// Read ordinary settings from the required AppConfig.toml in the working directory.
    pub fn from_file() -> Result<Self, ConfigError> {
        Self::from_path("AppConfig.toml")
    }

    /// Load an explicitly selected configuration file, without environment overrides.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        Self::from_config(&ConfigFile::load(path)?)
    }

    /// Validate a parsed snapshot before constructing policies or opening a listener.
    pub(crate) fn from_config(file: &ConfigFile) -> Result<Self, ConfigError> {
        file.check_keys(&[
            "server.bind_address", "server.request_timeout_seconds", "server.shutdown_grace_seconds",
            "server.body_limit_bytes", "server.max_in_flight_requests",
            "rate_limit.requests", "rate_limit.window_seconds", "rate_limit.max_clients",
            "cors.origins", "cors.allowed_methods", "cors.allowed_headers", "cors.exposed_headers",
            "cors.allow_credentials", "cors.max_age_seconds", "docs.enabled",
            "logging.filter", "logging.stdout_enabled", "logging.file_dir", "api.versioning_enabled",
            "uploads.enabled", "uploads.storage", "uploads.directory", "uploads.max_files",
            "uploads.max_file_bytes", "uploads.max_total_bytes", "uploads.allowed_types",
            "uploads.s3.bucket", "uploads.s3.region", "uploads.s3.endpoint", "uploads.s3.prefix",
            "uploads.s3.force_path_style", "uploads.s3.allow_http",
        ])?;
        let defaults = Self::default();
        let number = |key, default, max| file.range(key, default, 1, max);
        let bind_address = file.get_or("server.bind_address", defaults.bind_address.to_string())?
            .parse().map_err(|_| ConfigError::invalid("server.bind_address", "must be an IP address and port"))?;
        let config = Self {
            bind_address,
            request_timeout: Duration::from_secs(number("server.request_timeout_seconds", 30, 3600)?),
            shutdown_grace: Duration::from_secs(number("server.shutdown_grace_seconds", 10, 300)?),
            body_limit: number(
                "server.body_limit_bytes",
                defaults.body_limit as u64,
                1_073_741_824,
            )? as usize,
            max_in_flight: number("server.max_in_flight_requests", 256, 100_000)? as usize,
            rate_limit_requests: number("rate_limit.requests", 60, u32::MAX as u64)? as u32,
            rate_limit_window: Duration::from_secs(number("rate_limit.window_seconds", 60, 86400)?),
            rate_limit_max_clients: number("rate_limit.max_clients", 10_000, 1_000_000)? as usize,
            cors: CorsConfig::from_config(file)?,
            docs: DocsConfig::from_config(file)?,
            static_files: StaticConfig::default(),
            logging: LoggingConfig::from_config(file)?,
            versioning_enabled: file.get_or("api.versioning_enabled", true)?,
            uploads: UploadConfig::from_config(file)?,
        };
        config.uploads.validate_location(&config)?;
        Ok(config)
    }
}
