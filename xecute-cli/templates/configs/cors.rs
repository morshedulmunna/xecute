//! Browser cross-origin policy parsed from typed arrays in config/AppConfig.toml.

use super::{ConfigError, ConfigFile};
use axum::http::{HeaderName, HeaderValue, Method, Uri};
use std::time::Duration;

/// Exact allowlists; wildcards are rejected so credentialed policies remain explicit.
#[derive(Clone, Debug)]
pub struct CorsConfig {
    /// Origins allowed to read responses. Empty disables cross-origin browser access.
    pub origins: Vec<HeaderValue>,
    /// HTTP methods accepted during browser preflight checks.
    pub methods: Vec<Method>,
    /// Request headers browsers may include in cross-origin requests.
    pub allowed_headers: Vec<HeaderName>,
    /// Response headers browsers may read from cross-origin responses.
    pub exposed_headers: Vec<HeaderName>,
    /// Allow browser credentials only with an explicit nonempty origin allowlist.
    pub allow_credentials: bool,
    /// How long browsers may cache successful preflight policy, up to one day.
    pub max_age: Duration,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            origins: Vec::new(),
            methods: vec![
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
                Method::HEAD,
                Method::OPTIONS,
            ],
            allowed_headers: vec![
                HeaderName::from_static("content-type"),
                HeaderName::from_static("authorization"),
            ],
            exposed_headers: vec![
                HeaderName::from_static("x-request-id"),
                HeaderName::from_static("retry-after"),
            ],
            allow_credentials: false,
            max_age: Duration::from_secs(600),
        }
    }
}

impl CorsConfig {
    /// Validate list shapes and protocol values before any middleware is constructed.
    pub(crate) fn from_config(file: &ConfigFile) -> Result<Self, ConfigError> {
        let strings = |key: &str, default: &str| -> Result<Vec<String>, ConfigError> {
            let defaults: Vec<String> = serde_json::from_str(default).expect("valid built-in CORS list");
            let values: Vec<String> = file.get_or(key, defaults)?;
            if values.iter().any(|value| value.is_empty() || value.trim() != value || value == "*") {
                return Err(ConfigError::invalid(key, "entries must be nonempty exact values without wildcards or surrounding whitespace"));
            }
            Ok(values)
        };
        let mut origins = Vec::new();
        for origin in strings("cors.origins", "[]")? {
            let uri: Uri = origin.parse().map_err(|_| ConfigError::invalid("cors.origins", "invalid origin"))?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none()
                || uri.authority().is_some_and(|a| a.as_str().contains('@'))
                || uri.path() != "/"
                || uri.query().is_some()
                || origin.ends_with('/')
            {
                return Err(ConfigError::invalid("cors.origins", "must contain HTTP(S) origins without paths or credentials"));
            }
            origins.push(origin.parse().map_err(|_| ConfigError::invalid("cors.origins", "invalid origin header"))?);
        }
        let methods = strings(
            "cors.allowed_methods",
            r#"["GET","POST","PUT","PATCH","DELETE","HEAD","OPTIONS"]"#,
        )?
        .into_iter()
        .map(|method| {
            let method: Method = method
                .parse()
                .map_err(|_| ConfigError::invalid("cors.allowed_methods", "invalid method"))?;
            if !matches!(
                method,
                Method::GET
                    | Method::POST
                    | Method::PUT
                    | Method::PATCH
                    | Method::DELETE
                    | Method::HEAD
                    | Method::OPTIONS
            ) {
                return Err(ConfigError::invalid("cors.allowed_methods", "must contain supported uppercase HTTP methods"));
            }
            Ok(method)
        })
        .collect::<Result<Vec<_>, ConfigError>>()?;
        let headers = |key: &str, default: &str| -> Result<Vec<HeaderName>, ConfigError> {
            strings(key, default)?
                .into_iter()
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| ConfigError::invalid(key, "invalid HTTP header name"))
                })
                .collect()
        };
        let allow_credentials = file.get_or("cors.allow_credentials", false)?;
        if allow_credentials && origins.is_empty() {
            return Err(ConfigError::invalid("cors.allow_credentials", "requires at least one explicit cors.origins entry"));
        }
        let max_age = file.range("cors.max_age_seconds", 600, 0, 86400)?;
        Ok(Self {
            origins,
            methods,
            allowed_headers: headers(
                "cors.allowed_headers",
                r#"["content-type","authorization"]"#,
            )?,
            exposed_headers: headers("cors.exposed_headers", r#"["x-request-id","retry-after"]"#)?,
            allow_credentials,
            max_age: Duration::from_secs(max_age),
        })
    }
}
