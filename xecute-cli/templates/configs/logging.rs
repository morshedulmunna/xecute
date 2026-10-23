//! Environment-driven output, rotation, and filtering settings.

use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// Validated logger destinations and file retention policy.
pub struct LoggingConfig {
    /// Global filter applied to console, file, and exporter layers.
    pub filter: EnvFilter,
    /// Emit structured JSON to stdout.
    pub stdout: bool,
    /// Enable rolling JSON log files.
    pub file_enabled: bool,
    /// Dedicated private log directory.
    pub directory: PathBuf,
    /// File prefix, without directories or extension.
    pub filename: String,
    /// Time-based rotation policy: daily, hourly, or never.
    pub rotation: String,
    /// Maximum matching files retained by the rolling appender.
    pub max_files: usize,
}
impl LoggingConfig {
    /// Read logging configuration without changing process environment variables.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::from_lookup(super::read_env).map_err(Into::into)
    }
    pub(crate) fn from_lookup(lookup: impl Fn(&str) -> Result<Option<String>, String>) -> Result<Self, String> {
        let boolean = |key: &str, default: bool| match lookup(key)?.as_deref() {
            None => Ok(default), Some("true") => Ok(true), Some("false") => Ok(false),
            _ => Err(format!("{key} must be true or false")),
        };
        let filter = EnvFilter::try_new(lookup("RUST_LOG")?.unwrap_or_else(|| "info".into())).map_err(|e| e.to_string())?;
        let file_directory = lookup("LOG_FILE_DIR")?;
        let file_enabled = file_directory.as_ref().is_some_and(|value| !value.trim().is_empty());
        let directory = file_directory.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| "logs".into());
        let filename = "app".to_owned();
        let rotation = "daily".to_owned();
        let max_files = 7;
        Ok(Self { filter, stdout: boolean("LOG_STDOUT_ENABLED", true)?, file_enabled, directory: directory.into(), filename, rotation, max_files })
    }
}

/// Resolve existing ancestors, including symlinks, before directories are created.
pub(crate) fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    let path = if path.is_absolute() { path.to_owned() } else { std::env::current_dir()?.join(path) };
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part { std::path::Component::ParentDir => { normalized.pop(); }, std::path::Component::CurDir => {}, part => normalized.push(part.as_os_str()) }
    }
    if normalized.exists() { return normalized.canonicalize(); }
    let parent = normalized.parent().ok_or_else(|| std::io::Error::other("path has no parent"))?;
    Ok(absolute_path(parent)?.join(normalized.file_name().ok_or_else(|| std::io::Error::other("path has no filename"))?))
}
