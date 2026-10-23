//! Multipart transport boundary; storage and validation belong to the runtime service.

use std::sync::Arc;
use xecute::prelude::*;
use xecute::upload::{Multipart, StoredFile, UploadService};

/// Example upload controller, explicitly initialized by its module at startup.
#[controller("/uploads")]
pub struct UploadController {
    pub(super) service: Arc<UploadService>,
}
#[routes]
impl UploadController {
    /// Stream repeated file fields, validate the entire batch, then store it privately.
    #[post("")]
    #[multipart]
    #[api(summary = "Upload private files", tag = "Uploads", operation_id = "uploadFiles")]
    #[resp(201, "Files stored privately", json = Vec<StoredFile>)]
    #[resp(400, "Invalid multipart upload")]
    #[resp(413, "File or request too large")]
    #[resp(415, "File type is not allowed")]
    #[resp(422, "Empty upload or too many files")]
    async fn upload(&self, multipart: Multipart) -> Result<(HttpStatus, Json<Vec<StoredFile>>), HttpError> {
        Ok((HttpStatus::CREATED, Json(self.service.receive(multipart).await?)))
    }
}
