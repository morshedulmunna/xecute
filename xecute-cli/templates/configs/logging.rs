//! config/AppConfig.toml-driven output, rotation, and filtering settings.

use super::{ConfigError, ConfigFile};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// Validated logger destinations and file retention policy.
#[derive(Clone, Debug)]
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
impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            filter: EnvFilter::new("info"), stdout: true, file_enabled: false,
            directory: "logs".into(), filename: "app".into(), rotation: "daily".into(), max_files: 7,
        }
    }
}
impl LoggingConfig {
    pub(crate) fn from_config(file: &ConfigFile) -> Result<Self, ConfigError> {
        let filter = EnvFilter::try_new(file.get_or("logging.filter", "info".to_owned())?)
            .map_err(|_| ConfigError::invalid("logging.filter", "invalid tracing filter"))?;
        let directory = file.get_or("logging.file_dir", String::new())?;
        let file_enabled = !directory.trim().is_empty();
        Ok(Self {
            filter, stdout: file.get_or("logging.stdout_enabled", true)?, file_enabled,
            directory: if file_enabled { directory.into() } else { "logs".into() },
            ..Self::default()
        })
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
