//! Run with `cargo run -p xecute --example file_upload --features upload`.

use axum::{Router, extract::DefaultBodyLimit};
use std::{path::Path, sync::Arc};
use xecute::prelude::*;
use xecute::upload::{LocalStorage, Multipart, StoredFile, UploadPolicy, UploadService};

const MAX_FILE_BYTES: u64 = 1_048_576;
const MAX_TOTAL_BYTES: u64 = 2 * MAX_FILE_BYTES;
const HTTP_BODY_LIMIT: usize = MAX_TOTAL_BYTES as usize + 65_536;

/// Accepts repeated `file` fields and returns private object metadata.
#[controller("/uploads")]
pub struct UploadController {
    service: Arc<UploadService>,
}

#[routes]
impl UploadController {
    /// Validate every file before committing the batch to private storage.
    #[post("")]
    #[multipart]
    #[api(
        summary = "Upload private files",
        tag = "Uploads",
        operation_id = "uploadFiles"
    )]
    #[resp(201, "Files stored privately", json = Vec<StoredFile>)]
    #[resp(400, "Invalid multipart upload")]
    #[resp(413, "File or request too large")]
    #[resp(415, "File type is not allowed")]
    #[resp(422, "Empty upload or too many files")]
    async fn upload(
        &self,
        multipart: Multipart,
    ) -> Result<(HttpStatus, Json<Vec<StoredFile>>), HttpError> {
        let files = self.service.receive(multipart).await?;
        Ok((HttpStatus::CREATED, Json(files)))
    }
}

async fn app(directory: &Path) -> Result<Router, Box<dyn std::error::Error + Send + Sync>> {
    let policy = UploadPolicy::new(
        4,
        MAX_FILE_BYTES,
        MAX_TOTAL_BYTES,
        vec![
            "image/png".into(),
            "image/jpeg".into(),
            "application/pdf".into(),
        ],
    )?;
    let storage = Arc::new(LocalStorage::new(directory.join("objects")).await?);
    let service = UploadService::new(policy, directory.join("staging"), storage).await?;
    let controller = Arc::new(UploadController {
        service: Arc::new(service),
    });
    let document = xecute::openapi_document(
        "File upload example",
        "1",
        UploadController::api_operations(),
    )?;
    let router = UploadController::router(controller)
        .route(
            "/openapi.json",
            axum::routing::get(move || async move { Json(document) }),
        )
        .layer(DefaultBodyLimit::max(HTTP_BODY_LIMIT));
    Ok(xecute::exceptions::exception_boundary(router))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Private target data stays outside public assets and is ignored by Git.
    let router = app(Path::new("target/file-upload-example")).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("Upload: http://127.0.0.1:3000/uploads");
    println!("OpenAPI: http://127.0.0.1:3000/openapi.json");
    axum::serve(listener, router).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    fn request(mime: &str, bytes: &[u8]) -> Request<Body> {
        let mut body = format!("--example\r\nContent-Disposition: form-data; name=\"file\"; filename=\"report.pdf\"\r\nContent-Type: {mime}\r\n\r\n").into_bytes();
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n--example--\r\n");
        Request::builder()
            .method("POST")
            .uri("/uploads")
            .header("content-type", "multipart/form-data; boundary=example")
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn controller_persists_valid_files_and_rejects_mismatched_content() {
        let directory = tempfile::tempdir().unwrap();
        let router = app(directory.path()).await.unwrap();
        let pdf = b"%PDF-1.7\nexample\n%%EOF";
        let response = router
            .clone()
            .oneshot(request("application/pdf", pdf))
            .await
            .unwrap();
        assert_eq!(response.status(), HttpStatus::CREATED);
        let files: Vec<StoredFile> =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            tokio::fs::read(directory.path().join("objects").join(&files[0].key))
                .await
                .unwrap(),
            pdf
        );
        let response = router
            .clone()
            .oneshot(request("image/png", pdf))
            .await
            .unwrap();
        assert_eq!(response.status(), HttpStatus::UNSUPPORTED_MEDIA_TYPE);
        let request_id = response.headers()["x-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let error: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(error["error"]["request_id"], request_id);
        assert_eq!(
            std::fs::read_dir(directory.path().join("objects"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            std::fs::read_dir(directory.path().join("staging"))
                .unwrap()
                .count(),
            0
        );
    }

    #[tokio::test]
    async fn oversized_file_is_rejected_without_committing_objects() {
        let directory = tempfile::tempdir().unwrap();
        let router = app(directory.path()).await.unwrap();
        let mut bytes = vec![b' '; MAX_FILE_BYTES as usize + 1];
        bytes[..9].copy_from_slice(b"%PDF-1.7\n");
        let response = router
            .oneshot(request("application/pdf", &bytes))
            .await
            .unwrap();
        assert_eq!(response.status(), HttpStatus::PAYLOAD_TOO_LARGE);
        assert_eq!(
            std::fs::read_dir(directory.path().join("objects"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(
            std::fs::read_dir(directory.path().join("staging"))
                .unwrap()
                .count(),
            0
        );
    }
}
