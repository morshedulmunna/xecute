//! Validated server settings; invalid limits fail startup rather than disable protection.

use super::{CorsConfig, DocsConfig, StaticConfig};
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
    /// Browser cross-origin policy validated from environment JSON arrays.
    pub cors: CorsConfig,
    /// Whether API documentation is exposed.
    pub docs: DocsConfig,
    /// Dedicated public asset directory and mount prefix.
    pub static_files: StaticConfig,
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
        }
    }
}

impl Config {
    /// Read process settings, supply safe defaults, and reject invalid values.
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(super::read_env)
    }

    /// Accept an injectable source so tests can validate settings without mutating process state.
    pub(crate) fn from_lookup(
        lookup: impl Fn(&str) -> Result<Option<String>, String>,
    ) -> Result<Self, String> {
        let defaults = Self::default();
        let number = |key: &str, default: u64, max: u64| -> Result<u64, String> {
            let value = match lookup(key)? {
                None => default,
                Some(value) => value
                    .parse()
                    .map_err(|_| format!("{key} must be a positive integer"))?,
            };
            if value == 0 || value > max {
                return Err(format!("{key} must be between 1 and {max}"));
            }
            Ok(value)
        };
        let bind_address = lookup("BIND_ADDRESS")?
            .map(|value| {
                value
                    .parse::<SocketAddr>()
                    .map_err(|_| "BIND_ADDRESS must be an IP address and port".to_string())
            })
            .transpose()?
            .unwrap_or(defaults.bind_address);
        Ok(Self {
            bind_address,
            request_timeout: Duration::from_secs(number("REQUEST_TIMEOUT_SECONDS", 30, 3600)?),
            shutdown_grace: Duration::from_secs(number("SHUTDOWN_GRACE_SECONDS", 10, 300)?),
            body_limit: number(
                "BODY_LIMIT_BYTES",
                defaults.body_limit as u64,
                1_073_741_824,
            )? as usize,
            max_in_flight: number("MAX_IN_FLIGHT_REQUESTS", 256, 100_000)? as usize,
            rate_limit_requests: number("RATE_LIMIT_REQUESTS", 60, u32::MAX as u64)? as u32,
            rate_limit_window: Duration::from_secs(number("RATE_LIMIT_WINDOW_SECONDS", 60, 86400)?),
            rate_limit_max_clients: number("RATE_LIMIT_MAX_CLIENTS", 10_000, 1_000_000)? as usize,
            cors: CorsConfig::from_lookup(&lookup)?,
            docs: DocsConfig::from_lookup(&lookup)?,
            static_files: StaticConfig::default(),
        })
    }
}
