//! Typed environment access with value-redacted validation errors.

use serde::de::DeserializeOwned;
use std::{fmt, str::FromStr};

/// A configuration failure identifying the key, never its submitted value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    key: String,
    reason: String,
}
impl ConfigError {
    fn new(key: &str, reason: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            reason: reason.into(),
        }
    }
    /// Describe an application invariant using static public diagnostic text.
    pub fn invalid(key: &str, reason: &'static str) -> Self {
        Self::new(key, reason)
    }
    /// Return the setting that failed validation.
    pub fn key(&self) -> &str {
        &self.key
    }
}
impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.reason)
    }
}
impl std::error::Error for ConfigError {}

/// Read an optional Unicode setting; distinguish missing and empty values.
pub fn read_env(key: &str) -> Result<Option<String>, String> {
    match std::env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{key} must be valid UTF-8")),
    }
}

/// A typed reader over process variables or an injectable configuration source.
/// Source and parser errors are deliberately redacted because they can contain secrets.
pub struct Env<'a> {
    lookup: &'a dyn Fn(&str) -> Result<Option<String>, String>,
}
impl Env<'static> {
    /// Read process variables after the application has loaded its optional .env file.
    pub fn process() -> Self {
        Self::new(&read_env)
    }
}
impl<'a> Env<'a> {
    /// Use a custom source, including a map-backed closure in tests.
    pub fn new(lookup: &'a impl Fn(&str) -> Result<Option<String>, String>) -> Self {
        Self { lookup }
    }
    /// Parse a value, returning None only when the key is absent.
    pub fn optional<T: FromStr>(&self, key: &str) -> Result<Option<T>, ConfigError> {
        (self.lookup)(key)
            .map_err(|_| ConfigError::new(key, "could not read setting"))?
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| ConfigError::new(key, "invalid value or type"))
            })
            .transpose()
    }
    /// Require a present, nonempty value of the requested type.
    pub fn required<T: FromStr>(&self, key: &str) -> Result<T, ConfigError> {
        let raw = self
            .optional::<String>(key)?
            .ok_or_else(|| ConfigError::new(key, "required setting is missing"))?;
        if raw.trim().is_empty() {
            return Err(ConfigError::new(key, "required setting must not be empty"));
        }
        raw.parse()
            .map_err(|_| ConfigError::new(key, "invalid value or type"))
    }
    /// Use the default only when absent; malformed supplied values remain errors.
    pub fn get_or<T: FromStr>(&self, key: &str, default: T) -> Result<T, ConfigError> {
        Ok(self.optional(key)?.unwrap_or(default))
    }
    /// Read an unsigned integer and enforce inclusive startup bounds, including defaults.
    pub fn range(&self, key: &str, default: u64, min: u64, max: u64) -> Result<u64, ConfigError> {
        let value = self.get_or(key, default)?;
        if !(min..=max).contains(&value) {
            return Err(ConfigError::new(
                key,
                format!("must be between {min} and {max}"),
            ));
        }
        Ok(value)
    }
    /// Parse a JSON configuration value, or use the typed default when absent.
    pub fn json<T: DeserializeOwned>(&self, key: &str, default: T) -> Result<T, ConfigError> {
        match self.optional::<String>(key)? {
            None => Ok(default),
            Some(raw) => serde_json::from_str(&raw)
                .map_err(|_| ConfigError::new(key, "invalid JSON value or type")),
        }
    }
    /// Apply an application-specific invariant without including the value in errors.
    pub fn validate<T>(
        &self,
        key: &str,
        value: T,
        predicate: impl FnOnce(&T) -> bool,
    ) -> Result<T, ConfigError> {
        if predicate(&value) {
            Ok(value)
        } else {
            Err(ConfigError::new(key, "failed validation"))
        }
    }
    /// Require a nonempty secret whose Debug representation is redacted.
    pub fn secret(&self, key: &str) -> Result<SecretString, ConfigError> {
        self.required::<String>(key).map(SecretString)
    }
}

/// A secret that requires explicit access and never exposes its value in Debug.
#[derive(Clone)]
pub struct SecretString(String);
impl SecretString {
    /// Explicitly access the secret for a database client or SDK configuration.
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// A parsed TOML configuration snapshot. Environment variables are never merged into it.
/// Use this for ordinary settings and Env for credentials supplied by .env/secret stores.
pub struct ConfigFile {
    values: toml::Table,
}
impl ConfigFile {
    /// Read a required configuration file. Errors omit file contents and OS error details.
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let source = std::fs::read_to_string(path)
            .map_err(|_| ConfigError::new("AppConfig.toml", "could not read configuration file"))?;
        Self::parse(&source)
    }
    /// Parse TOML without exposing the original source in diagnostics.
    pub fn parse(source: &str) -> Result<Self, ConfigError> {
        source
            .parse::<toml::Table>()
            .map(|values| Self { values })
            .map_err(|_| ConfigError::new("AppConfig.toml", "invalid TOML syntax"))
    }
    fn find(&self, key: &str) -> Result<Option<&toml::Value>, ConfigError> {
        let mut table = &self.values;
        let mut parts = key.split('.').peekable();
        while let Some(part) = parts.next() {
            if part.is_empty() {
                return Err(ConfigError::new(key, "invalid setting path"));
            }
            let Some(value) = table.get(part) else {
                return Ok(None);
            };
            if parts.peek().is_none() {
                return Ok(Some(value));
            }
            table = value
                .as_table()
                .ok_or_else(|| ConfigError::new(key, "expected a configuration table"))?;
        }
        Ok(None)
    }
    /// Deserialize a supplied value with strict TOML types; None means the key is absent.
    pub fn optional<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, ConfigError> {
        self.find(key)?
            .cloned()
            .map(|value| {
                value
                    .try_into()
                    .map_err(|_| ConfigError::new(key, "invalid value or type"))
            })
            .transpose()
    }
    /// Require a setting of the requested type; string values must be nonempty.
    pub fn required<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        let value = self
            .find(key)?
            .ok_or_else(|| ConfigError::new(key, "required setting is missing"))?;
        if value.as_str().is_some_and(|value| value.trim().is_empty()) {
            return Err(ConfigError::new(key, "required setting must not be empty"));
        }
        value
            .clone()
            .try_into()
            .map_err(|_| ConfigError::new(key, "invalid value or type"))
    }
    /// Use a typed default only for an absent key; a supplied wrong type is an error.
    pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> Result<T, ConfigError> {
        Ok(self.optional(key)?.unwrap_or(default))
    }
    /// Read an unsigned integer within inclusive bounds.
    pub fn range(&self, key: &str, default: u64, min: u64, max: u64) -> Result<u64, ConfigError> {
        let value = self.get_or(key, default)?;
        if !(min..=max).contains(&value) {
            return Err(ConfigError::new(
                key,
                format!("must be between {min} and {max}"),
            ));
        }
        Ok(value)
    }
    /// Reject misspelled settings instead of silently falling back to defaults.
    /// List allowed leaf paths, such as `server.bind_address` and `cors.origins`.
    pub fn check_keys(&self, allowed: &[&str]) -> Result<(), ConfigError> {
        fn visit(table: &toml::Table, prefix: &str, allowed: &[&str]) -> Result<(), ConfigError> {
            for (key, value) in table {
                if key.contains('.') {
                    return Err(ConfigError::new(
                        key,
                        "use nested TOML tables instead of literal dotted keys",
                    ));
                }
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if let Some(table) = value.as_table() {
                    if !allowed
                        .iter()
                        .any(|path| path.starts_with(&format!("{key}.")))
                    {
                        return Err(ConfigError::new(&key, "unknown configuration section"));
                    }
                    visit(table, &key, allowed)?;
                } else if !allowed.contains(&key.as_str()) {
                    return Err(ConfigError::new(&key, "unknown setting"));
                }
            }
            Ok(())
        }
        visit(&self.values, "", allowed)
    }
}
