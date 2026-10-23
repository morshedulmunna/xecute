//! Browser cross-origin policy parsed from JSON arrays in environment variables.

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
    pub(crate) fn from_lookup(
        lookup: &impl Fn(&str) -> Result<Option<String>, String>,
    ) -> Result<Self, String> {
        let strings = |key: &str, default: &str| -> Result<Vec<String>, String> {
            let raw = lookup(key)?.unwrap_or_else(|| default.to_owned());
            let values: Vec<String> = serde_json::from_str(&raw)
                .map_err(|_| format!("{key} must be a JSON array of strings"))?;
            if values
                .iter()
                .any(|value| value.is_empty() || value.trim() != value || value == "*")
            {
                return Err(format!(
                    "{key} entries must be nonempty exact values without wildcards or surrounding whitespace"
                ));
            }
            Ok(values)
        };
        let mut origins = Vec::new();
        for origin in strings("CORS_ORIGINS", "[]")? {
            let uri: Uri = origin.parse().map_err(|_| "invalid CORS_ORIGINS entry")?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || uri.authority().is_none()
                || uri.authority().is_some_and(|a| a.as_str().contains('@'))
                || uri.path() != "/"
                || uri.query().is_some()
                || origin.ends_with('/')
            {
                return Err("CORS_ORIGINS must contain origins such as https://example.com without paths or credentials".into());
            }
            origins.push(origin.parse().map_err(|_| "invalid CORS origin header")?);
        }
        let methods = strings(
            "CORS_ALLOWED_METHODS",
            r#"["GET","POST","PUT","PATCH","DELETE","HEAD","OPTIONS"]"#,
        )?
        .into_iter()
        .map(|method| {
            let method: Method = method
                .parse()
                .map_err(|_| "invalid CORS_ALLOWED_METHODS entry")?;
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
                return Err(
                    "CORS_ALLOWED_METHODS must contain supported uppercase HTTP methods".to_owned(),
                );
            }
            Ok(method)
        })
        .collect::<Result<Vec<_>, String>>()?;
        let headers = |key: &str, default: &str| -> Result<Vec<HeaderName>, String> {
            strings(key, default)?
                .into_iter()
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| format!("{key} contains an invalid HTTP header name"))
                })
                .collect()
        };
        let allow_credentials = lookup("CORS_ALLOW_CREDENTIALS")?
            .unwrap_or_else(|| "false".into())
            .parse::<bool>()
            .map_err(|_| "CORS_ALLOW_CREDENTIALS must be true or false")?;
        if allow_credentials && origins.is_empty() {
            return Err(
                "CORS_ALLOW_CREDENTIALS requires at least one explicit CORS_ORIGINS entry".into(),
            );
        }
        let max_age = lookup("CORS_MAX_AGE_SECONDS")?
            .unwrap_or_else(|| "600".into())
            .parse::<u64>()
            .map_err(|_| "CORS_MAX_AGE_SECONDS must be an integer from 0 to 86400")?;
        if max_age > 86400 {
            return Err("CORS_MAX_AGE_SECONDS must be from 0 to 86400".into());
        }
        Ok(Self {
            origins,
            methods,
            allowed_headers: headers(
                "CORS_ALLOWED_HEADERS",
                r#"["content-type","authorization"]"#,
            )?,
            exposed_headers: headers("CORS_EXPOSE_HEADERS", r#"["x-request-id","retry-after"]"#)?,
            allow_credentials,
            max_age: Duration::from_secs(max_age),
        })
    }
}
