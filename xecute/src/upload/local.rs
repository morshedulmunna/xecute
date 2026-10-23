use super::{
    StorageFuture, StoredFile, UploadError, UploadStorage, ValidatedUpload, private_directory,
    staging_file, valid_key,
};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Private filesystem storage with generated names, exclusive creation, and atomic publication.
#[derive(Clone)]
pub struct LocalStorage {
    directory: PathBuf,
}
impl LocalStorage {
    /// Create a dedicated real directory. The application must keep it outside public assets.
    pub async fn new(directory: impl AsRef<Path>) -> Result<Self, UploadError> {
        Ok(Self {
            directory: private_directory(directory.as_ref()).await?,
        })
    }
}
impl UploadStorage for LocalStorage {
    fn put<'a>(&'a self, upload: &'a ValidatedUpload) -> StorageFuture<'a, StoredFile> {
        Box::pin(async move {
            if !valid_key(&upload.metadata.key) {
                return Err(UploadError::InvalidFilename);
            }
            let (mut output, path) = staging_file(&self.directory).await?;
            let mut input = tokio::fs::File::open(upload.path())
                .await
                .map_err(|_| UploadError::Storage)?;
            tokio::io::copy(&mut input, &mut output)
                .await
                .map_err(|_| UploadError::Storage)?;
            output.flush().await.map_err(|_| UploadError::Storage)?;
            output.sync_all().await.map_err(|_| UploadError::Storage)?;
            drop(output);
            let target = self.directory.join(&upload.metadata.key);
            tokio::task::spawn_blocking(move || path.persist_noclobber(target))
                .await
                .map_err(|_| UploadError::Storage)?
                .map_err(|_| UploadError::Storage)?;
            Ok(upload.metadata.clone())
        })
    }
    fn delete<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if !valid_key(key) {
                return Err(UploadError::InvalidFilename);
            }
            match tokio::fs::remove_file(self.directory.join(key)).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(UploadError::Storage),
            }
        })
    }
}
