//! Initialize the selected private storage backend once before accepting requests.

use super::controller::UploadController;
use crate::configs::{Config, ConfigError};
use axum::Router;
use std::sync::Arc;
use xecute::upload::{LocalStorage, UploadService, UploadStorage};

/// Async startup boundary for upload providers; it never creates a bucket or changes ACLs.
pub struct UploadModule;
impl UploadModule {
    /// Assemble the enabled upload routes with their shared validation/storage service.
    pub async fn router(config: &Config) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
        let uploads = &config.uploads;
        if !uploads.enabled { return Ok(Router::new()); }
        uploads.validate_location(config)?;
        let storage: Arc<dyn UploadStorage> = match uploads.storage.as_str() {
            "local" => Arc::new(LocalStorage::new(&uploads.directory).await?),
            "s3" => {
                #[cfg(feature = "s3")]
                { Arc::new(xecute::upload::S3Storage::from_environment(uploads.s3.clone()).await?) }
                #[cfg(not(feature = "s3"))]
                { return Err(ConfigError::invalid("uploads.storage", "enable the s3 Cargo feature to use S3 storage").into()); }
            }
            _ => return Err(ConfigError::invalid("uploads.storage", "unsupported storage backend").into()),
        };
        let service = Arc::new(UploadService::new(uploads.policy()?, &uploads.directory, storage).await?);
        Ok(UploadController::router(Arc::new(UploadController { service })))
    }
    /// Share the application DI request context with upload middleware and handlers.
    pub async fn router_with_container(config: &Config, container: Arc<xecute::Container>) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
        Ok(xecute::with_container(Self::router(config).await?, container))
    }
    /// Include upload documentation only when the endpoint is enabled.
    pub fn api_operations(config: &Config) -> Vec<xecute::ApiOperation> {
        if !config.uploads.enabled { return Vec::new(); }
        let mut operations = UploadController::api_operations();
        for operation in &mut operations {
            if let Some(schema) = operation.body_schema.as_mut() {
                let files = &mut schema["properties"]["file"];
                files["maxItems"] = serde_json::json!(config.uploads.max_files);
                files["description"] = serde_json::json!(format!(
                    "Up to {} files; {} bytes per file; {} total bytes. Allowed types: {}.",
                    config.uploads.max_files, config.uploads.max_file_bytes,
                    config.uploads.max_total_bytes, config.uploads.allowed_types.join(", ")
                ));
                schema["x-max-total-bytes"] = serde_json::json!(config.uploads.max_total_bytes);
            }
        }
        operations
    }
}
