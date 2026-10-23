//! Streaming multipart validation and pluggable private file storage.

mod local;
#[cfg(feature = "upload-s3")]
mod s3;
use crate::{HttpError, HttpStatus};
pub use axum::extract::Multipart;
pub use local::LocalStorage;
#[cfg(feature = "upload-s3")]
pub use s3::S3Storage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use tempfile::TempPath;
use tokio::io::AsyncWriteExt;

/// Safe upload failure; storage SDK/OS errors and file contents are never exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadError {
    /// Unsupported limits, MIME types, or storage settings.
    InvalidConfiguration,
    /// Malformed multipart framing, missing filename, or unexpected field.
    InvalidMultipart,
    /// An unsafe filename was supplied.
    InvalidFilename,
    /// The configured number of files was exceeded.
    TooManyFiles,
    /// Per-file or aggregate content limit was exceeded.
    TooLarge,
    /// Declared MIME type and detected signature do not match the allowlist.
    UnsupportedType,
    /// No nonempty file was submitted.
    Empty,
    /// A filesystem or storage provider operation failed.
    Storage,
}
impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "upload failed: {self:?}")
    }
}
impl std::error::Error for UploadError {}
impl From<UploadError> for HttpError {
    fn from(error: UploadError) -> Self {
        match error {
            UploadError::InvalidConfiguration | UploadError::Storage => {
                Self::new(HttpStatus::INTERNAL_SERVER_ERROR)
            }
            UploadError::TooLarge => Self::public(
                HttpStatus::PAYLOAD_TOO_LARGE,
                "upload_too_large",
                "Upload exceeds the configured size limit",
            ),
            UploadError::UnsupportedType => Self::public(
                HttpStatus::UNSUPPORTED_MEDIA_TYPE,
                "upload_type_not_allowed",
                "File type is not allowed or does not match its content",
            ),
            UploadError::TooManyFiles => Self::public(
                HttpStatus::UNPROCESSABLE_ENTITY,
                "upload_count_exceeded",
                "Too many uploaded files",
            ),
            UploadError::Empty => Self::public(
                HttpStatus::UNPROCESSABLE_ENTITY,
                "empty_upload",
                "A nonempty file is required",
            ),
            _ => Self::public(
                HttpStatus::BAD_REQUEST,
                "invalid_upload",
                "Invalid multipart upload",
            ),
        }
    }
}
impl axum::response::IntoResponse for UploadError {
    fn into_response(self) -> axum::response::Response {
        HttpError::from(self).into_response()
    }
}

/// Immutable resource limits and accepted signature-backed MIME types.
#[derive(Clone, Debug)]
pub struct UploadPolicy {
    max_files: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
    allowed_types: Vec<String>,
}
impl UploadPolicy {
    /// Validate limits once. Content is streamed to disk, never buffered as one large body.
    /// An outer HTTP body limit must also bound multipart headers, framing, and unused bytes.
    pub fn new(
        max_files: usize,
        max_file_bytes: u64,
        max_total_bytes: u64,
        allowed_types: Vec<String>,
    ) -> Result<Self, UploadError> {
        if !(1..=32).contains(&max_files)
            || max_file_bytes == 0
            || max_file_bytes > 1_073_741_824
            || max_total_bytes < max_file_bytes
            || max_total_bytes > 4_294_967_296
            || allowed_types.is_empty()
            || allowed_types
                .iter()
                .any(|mime| !infer::is_mime_supported(mime))
        {
            return Err(UploadError::InvalidConfiguration);
        }
        Ok(Self {
            max_files,
            max_file_bytes,
            max_total_bytes,
            allowed_types,
        })
    }
}

/// Metadata for a privately stored object. Keys never contain a client-provided path.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StoredFile {
    /// Opaque generated UUID filename; storage-specific prefixes are private implementation details.
    pub key: String,
    /// Original display name only, never used as a storage path or response header.
    pub original_name: String,
    /// Signature-verified MIME type.
    pub content_type: String,
    /// Actual streamed content length.
    pub size: u64,
    /// Lowercase SHA-256 checksum of the complete content.
    pub sha256: String,
}

/// A validated staging file. Drop removes the temporary file on error or cancellation.
pub struct ValidatedUpload {
    path: TempPath,
    metadata: StoredFile,
}
impl ValidatedUpload {
    /// Read-only staging path for storage adapters. Do not retain it after this upload is dropped.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Verified object metadata with a generated key.
    pub fn metadata(&self) -> &StoredFile {
        &self.metadata
    }
}

/// Sendable asynchronous adapter result, usable without an async-trait dependency.
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, UploadError>> + Send + 'a>>;
/// Private storage adapter; custom implementations can target other providers.
pub trait UploadStorage: Send + Sync {
    /// Store a validated file without trusting its original display name.
    fn put<'a>(&'a self, upload: &'a ValidatedUpload) -> StorageFuture<'a, StoredFile>;
    /// Remove an object by its generated key; used for best-effort batch rollback.
    fn delete<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()>;
}

/// Validates all multipart files before storing any of them.
#[derive(Clone)]
pub struct UploadService {
    policy: UploadPolicy,
    staging_directory: PathBuf,
    storage: Arc<dyn UploadStorage>,
}
impl UploadService {
    /// Prepare a private staging directory at startup, outside publicly served assets.
    pub async fn new(
        policy: UploadPolicy,
        staging_directory: impl AsRef<Path>,
        storage: Arc<dyn UploadStorage>,
    ) -> Result<Self, UploadError> {
        let directory = private_directory(staging_directory.as_ref()).await?;
        Ok(Self {
            policy,
            staging_directory: directory,
            storage,
        })
    }
    /// Accept repeated `file` fields, validate their complete content, then store the batch.
    /// Validation errors leave no committed objects. Storage failures trigger best-effort
    /// rollback; cancellations/network uncertainty can leave committed objects for reconciliation.
    pub async fn receive(&self, mut multipart: Multipart) -> Result<Vec<StoredFile>, UploadError> {
        let mut files = Vec::new();
        let mut total = 0_u64;
        while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
            if files.len() >= self.policy.max_files {
                return Err(UploadError::TooManyFiles);
            }
            if field.name() != Some("file") {
                return Err(UploadError::InvalidMultipart);
            }
            let name = field
                .file_name()
                .ok_or(UploadError::InvalidMultipart)?
                .to_owned();
            if name.is_empty()
                || name.len() > 255
                || name.starts_with('.')
                || name
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
            {
                return Err(UploadError::InvalidFilename);
            }
            let declared = field
                .content_type()
                .ok_or(UploadError::UnsupportedType)?
                .to_owned();
            if !self.policy.allowed_types.contains(&declared) {
                return Err(UploadError::UnsupportedType);
            }
            let (mut output, path) = staging_file(&self.staging_directory).await?;
            let mut size = 0_u64;
            let mut sample = Vec::with_capacity(8192);
            let mut digest = Sha256::new();
            while let Some(chunk) = field.chunk().await.map_err(multipart_error)? {
                let count = chunk.len() as u64;
                size = size.checked_add(count).ok_or(UploadError::TooLarge)?;
                total = total.checked_add(count).ok_or(UploadError::TooLarge)?;
                if size > self.policy.max_file_bytes || total > self.policy.max_total_bytes {
                    return Err(UploadError::TooLarge);
                }
                let take = (8192 - sample.len()).min(chunk.len());
                sample.extend_from_slice(&chunk[..take]);
                digest.update(&chunk);
                output
                    .write_all(&chunk)
                    .await
                    .map_err(|_| UploadError::Storage)?;
            }
            if size == 0 {
                return Err(UploadError::Empty);
            }
            let kind = infer::get(&sample).ok_or(UploadError::UnsupportedType)?;
            if kind.mime_type() != declared {
                return Err(UploadError::UnsupportedType);
            }
            output.flush().await.map_err(|_| UploadError::Storage)?;
            drop(output);
            files.push(ValidatedUpload {
                path,
                metadata: StoredFile {
                    key: format!("{}.{}", uuid::Uuid::new_v4(), kind.extension()),
                    original_name: name,
                    content_type: declared,
                    size,
                    sha256: digest
                        .finalize()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                },
            });
        }
        if files.is_empty() {
            return Err(UploadError::Empty);
        }
        let mut stored: Vec<StoredFile> = Vec::new();
        for file in &files {
            match self.storage.put(file).await {
                Ok(file) => stored.push(file),
                Err(error) => {
                    for committed in &stored {
                        let _ = self.storage.delete(&committed.key).await;
                    }
                    return Err(error);
                }
            }
        }
        Ok(stored)
    }
}
fn multipart_error(error: axum::extract::multipart::MultipartError) -> UploadError {
    if error.status() == HttpStatus::PAYLOAD_TOO_LARGE {
        UploadError::TooLarge
    } else {
        UploadError::InvalidMultipart
    }
}
pub(super) async fn private_directory(path: &Path) -> Result<PathBuf, UploadError> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|_| UploadError::Storage)?;
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|_| UploadError::Storage)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(UploadError::InvalidConfiguration);
    }
    tokio::fs::canonicalize(path)
        .await
        .map_err(|_| UploadError::Storage)
}
pub(super) async fn staging_file(
    directory: &Path,
) -> Result<(tokio::fs::File, TempPath), UploadError> {
    let directory = directory.to_owned();
    let file = tokio::task::spawn_blocking(move || {
        tempfile::Builder::new()
            .prefix(".upload-")
            .tempfile_in(directory)
    })
    .await
    .map_err(|_| UploadError::Storage)?
    .map_err(|_| UploadError::Storage)?;
    let (file, path) = file.into_parts();
    Ok((tokio::fs::File::from_std(file), path))
}
pub(super) fn valid_key(key: &str) -> bool {
    key.split_once('.').is_some_and(|(id, extension)| {
        uuid::Uuid::parse_str(id).is_ok_and(|parsed| id == parsed.to_string())
            && !extension.is_empty()
            && extension.len() <= 16
            && extension.bytes().all(|c| c.is_ascii_alphanumeric())
    })
}

/// OpenAPI request schema for repeated multipart `file` fields.
pub fn multipart_schema() -> serde_json::Value {
    serde_json::json!({"type":"object", "required":["file"], "properties": {
        "file": {"type":"array", "minItems":1, "items":{"type":"string", "format":"binary"}}
    }})
}

/// Ordinary S3 settings. Credentials come from the AWS SDK credential chain, not TOML.
#[derive(Clone, Debug)]
pub struct S3Options {
    /// Existing private bucket; the adapter never creates a bucket or changes its ACL.
    pub bucket: String,
    /// Explicit AWS region from application configuration.
    pub region: String,
    /// Optional S3-compatible endpoint, for example a private MinIO service.
    pub endpoint: Option<String>,
    /// Use bucket/path URLs for S3-compatible endpoints.
    pub force_path_style: bool,
    /// Explicitly permit an HTTP endpoint for local development only.
    pub allow_http: bool,
    /// Optional key prefix with simple slash-separated segments.
    pub prefix: String,
}
impl S3Options {
    /// Reject invalid ordinary settings before resolving credentials or sending requests.
    pub fn validate(&self) -> Result<(), UploadError> {
        if self.bucket.len() < 3
            || self.bucket.len() > 63
            || self.bucket.contains("..")
            || self.bucket.contains(".-")
            || self.bucket.contains("-.")
            || !self
                .bucket
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            || !self
                .bucket
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
            || self.bucket.parse::<std::net::Ipv4Addr>().is_ok()
            || !self
                .bucket
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'.')
            || self.region.len() > 64
            || self.prefix.len() > 900
            || self.region.is_empty()
            || !self
                .region
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || (!self.prefix.is_empty()
                && self.prefix.split('/').any(|part| {
                    part.is_empty()
                        || !part
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
                }))
        {
            return Err(UploadError::InvalidConfiguration);
        }
        if let Some(endpoint) = &self.endpoint {
            let uri: axum::http::Uri = endpoint
                .parse()
                .map_err(|_| UploadError::InvalidConfiguration)?;
            if !matches!(uri.scheme_str(), Some("https"))
                && !(self.allow_http && uri.scheme_str() == Some("http"))
            {
                return Err(UploadError::InvalidConfiguration);
            }
            if uri.authority().is_none()
                || uri
                    .authority()
                    .is_some_and(|host| host.as_str().contains('@'))
                || uri.query().is_some()
                || uri.path() != "/"
            {
                return Err(UploadError::InvalidConfiguration);
            }
        }
        Ok(())
    }
}
