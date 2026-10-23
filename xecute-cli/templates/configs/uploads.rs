//! Ordinary upload limits and storage settings; credentials remain outside TOML.

use super::{ConfigError, ConfigFile};
use std::path::PathBuf;
use xecute::upload::{S3Options, UploadPolicy};

/// Explicitly enabled private file upload example.
#[derive(Clone, Debug)]
pub struct UploadConfig {
    /// Expose the multipart endpoint only when enabled.
    pub enabled: bool,
    /// local or s3; the latter requires the generated s3 Cargo feature.
    pub storage: String,
    /// Private local object/staging directory, never the public static tree.
    pub directory: PathBuf,
    /// Number of repeated file fields accepted per request.
    pub max_files: usize,
    /// Actual content bytes allowed in one file.
    pub max_file_bytes: u64,
    /// Aggregate content bytes allowed across all files.
    pub max_total_bytes: u64,
    /// Exact MIME allowlist, verified against detected content signatures.
    pub allowed_types: Vec<String>,
    /// Non-sensitive S3 bucket, region, endpoint and key prefix.
    pub s3: S3Options,
}
impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            enabled: false, storage: "local".into(), directory: "storage/uploads".into(),
            max_files: 4, max_file_bytes: 524288, max_total_bytes: 524288,
            allowed_types: vec!["image/png".into(), "image/jpeg".into(), "application/pdf".into()],
            s3: S3Options { bucket: String::new(), region: "us-east-1".into(), endpoint: None, force_path_style: false, allow_http: false, prefix: "uploads".into() },
        }
    }
}
impl UploadConfig {
    pub(crate) fn from_config(file: &ConfigFile) -> Result<Self, ConfigError> {
        let defaults = Self::default();
        let endpoint = file.get_or("uploads.s3.endpoint", String::new())?;
        let config = Self {
            enabled: file.get_or("uploads.enabled", false)?,
            storage: file.get_or("uploads.storage", defaults.storage)?,
            directory: file.get_or::<String>("uploads.directory", defaults.directory.to_string_lossy().into_owned())?.into(),
            max_files: file.range("uploads.max_files", defaults.max_files as u64, 1, 32)? as usize,
            max_file_bytes: file.range("uploads.max_file_bytes", defaults.max_file_bytes, 1, 1_073_741_824)?,
            max_total_bytes: file.range("uploads.max_total_bytes", defaults.max_total_bytes, 1, 4_294_967_296)?,
            allowed_types: file.get_or("uploads.allowed_types", defaults.allowed_types)?,
            s3: S3Options {
                bucket: file.get_or("uploads.s3.bucket", String::new())?,
                region: file.get_or("uploads.s3.region", defaults.s3.region)?,
                endpoint: (!endpoint.is_empty()).then_some(endpoint),
                prefix: file.get_or("uploads.s3.prefix", defaults.s3.prefix)?,
                force_path_style: file.get_or("uploads.s3.force_path_style", false)?,
                allow_http: file.get_or("uploads.s3.allow_http", false)?,
            },
        };
        if !matches!(config.storage.as_str(), "local" | "s3") || config.directory.as_os_str().is_empty() {
            return Err(ConfigError::invalid("uploads", "invalid storage backend or directory"));
        }
        config.policy()?;
        if config.storage == "s3" { config.s3.validate().map_err(|_| ConfigError::invalid("uploads.s3", "invalid S3 settings"))?; }
        Ok(config)
    }
    /// Construct the immutable runtime validation policy.
    pub fn policy(&self) -> Result<UploadPolicy, ConfigError> {
        UploadPolicy::new(self.max_files, self.max_file_bytes, self.max_total_bytes, self.allowed_types.clone())
            .map_err(|_| ConfigError::invalid("uploads", "invalid upload limits or MIME allowlist"))
    }
    pub(crate) fn validate_location(&self, config: &super::Config) -> Result<(), ConfigError> {
        if !self.enabled { return Ok(()); }
        if self.max_total_bytes.saturating_add(65536) > config.body_limit as u64 {
            return Err(ConfigError::invalid("server.body_limit_bytes", "must cover uploads.max_total_bytes plus 65536 bytes of multipart overhead"));
        }
        let directory = super::logging::absolute_path(&self.directory).map_err(|_| ConfigError::invalid("uploads.directory", "cannot resolve private directory"))?;
        let public = super::logging::absolute_path(&config.static_files.directory).map_err(|_| ConfigError::invalid("uploads.directory", "cannot resolve public directory"))?;
        let root = std::env::current_dir().and_then(|path| path.canonicalize()).map_err(|_| ConfigError::invalid("uploads.directory", "cannot resolve application root"))?;
        if directory == root || directory.starts_with(public) {
            return Err(ConfigError::invalid("uploads.directory", "must be a dedicated directory outside the public static tree"));
        }
        Ok(())
    }
}
