//! Streaming upload limits, content validation, private local storage, and rollback.
#![cfg(feature = "upload")]
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::DefaultBodyLimit,
    http::{Request, StatusCode},
    routing::post,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use xecute::upload::{
    LocalStorage, Multipart, StorageFuture, StoredFile, UploadError, UploadPolicy, UploadService,
    UploadStorage, ValidatedUpload,
};
const PDF: &[u8] = b"%PDF-1.7\nexample content\n%%EOF";
fn multipart(parts: &[(&str, &str, &str, &[u8])]) -> Body {
    let mut body = Vec::new();
    for (field, name, mime, bytes) in parts {
        body.extend_from_slice(format!("--boundary\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{name}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(b"--boundary--\r\n");
    Body::from(body)
}
fn request(body: Body) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/")
        .header("content-type", "multipart/form-data; boundary=boundary")
        .body(body)
        .unwrap()
}
fn router(service: UploadService) -> Router {
    Router::new().route(
        "/",
        post(move |multipart: Multipart| {
            let service = service.clone();
            async move { service.receive(multipart).await.map(Json) }
        }),
    )
}
fn policy(files: usize, file: u64, total: u64) -> UploadPolicy {
    UploadPolicy::new(files, file, total, vec!["application/pdf".into()]).unwrap()
}
#[tokio::test]
async fn local_upload_uses_generated_keys_persists_exact_content_and_removes_staging_files() {
    let root = tempfile::tempdir().unwrap();
    let objects = root.path().join("objects");
    let staging = root.path().join("staging");
    let storage = Arc::new(LocalStorage::new(&objects).await.unwrap());
    let service = UploadService::new(policy(2, 128, 256), &staging, storage.clone())
        .await
        .unwrap();
    let response = router(service)
        .oneshot(request(multipart(&[(
            "file",
            "report.pdf",
            "application/pdf",
            PDF,
        )])))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let files: Vec<StoredFile> =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_ne!(file.key, "report.pdf");
    assert!(file.key.ends_with(".pdf"));
    assert_eq!(file.original_name, "report.pdf");
    assert_eq!(file.size, PDF.len() as u64);
    assert_eq!(file.sha256.len(), 64);
    assert_eq!(std::fs::read(objects.join(&file.key)).unwrap(), PDF);
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
    assert!(storage.delete("../outside").await.is_err());
    storage.delete(&file.key).await.unwrap();
    storage.delete(&file.key).await.unwrap();
    assert_eq!(std::fs::read_dir(&objects).unwrap().count(), 0);
}
#[tokio::test]
async fn invalid_batches_commit_nothing_and_cleanup_all_staged_files() {
    let root = tempfile::tempdir().unwrap();
    let objects = root.path().join("objects");
    let staging = root.path().join("staging");
    let storage = Arc::new(LocalStorage::new(&objects).await.unwrap());
    let app = router(
        UploadService::new(policy(2, 64, 80), &staging, storage)
            .await
            .unwrap(),
    );
    for (parts, status) in [
        (
            vec![("file", "../unsafe.pdf", "application/pdf", PDF)],
            StatusCode::BAD_REQUEST,
        ),
        (
            vec![("other", "report.pdf", "application/pdf", PDF)],
            StatusCode::BAD_REQUEST,
        ),
        (
            vec![("file", "report.pdf", "image/png", PDF)],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            vec![(
                "file",
                "report.pdf",
                "application/pdf",
                b"fake pdf".as_slice(),
            )],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            vec![("file", "report.pdf", "application/pdf", b"".as_slice())],
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (vec![], StatusCode::UNPROCESSABLE_ENTITY),
        (
            vec![
                ("file", "one.pdf", "application/pdf", PDF),
                ("file", "two.pdf", "application/pdf", b"bad".as_slice()),
            ],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            vec![
                ("file", "one.pdf", "application/pdf", PDF),
                ("file", "two.pdf", "application/pdf", PDF),
                ("file", "three.pdf", "application/pdf", PDF),
            ],
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(request(multipart(&parts)))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(std::fs::read_dir(&objects).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
    }
    let large = [b'%'; 65];
    assert_eq!(
        app.clone()
            .oneshot(request(multipart(&[(
                "file",
                "large.pdf",
                "application/pdf",
                &large
            )])))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let total_app = router(
        UploadService::new(
            policy(2, 32, 40),
            &staging,
            Arc::new(LocalStorage::new(&objects).await.unwrap()),
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        total_app
            .oneshot(request(multipart(&[
                ("file", "one.pdf", "application/pdf", PDF),
                ("file", "two.pdf", "application/pdf", PDF)
            ])))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
}
struct FailSecond {
    count: AtomicUsize,
    deleted: Mutex<Vec<String>>,
}
impl UploadStorage for FailSecond {
    fn put<'a>(&'a self, file: &'a ValidatedUpload) -> StorageFuture<'a, StoredFile> {
        Box::pin(async move {
            if self.count.fetch_add(1, Ordering::SeqCst) == 1 {
                Err(UploadError::Storage)
            } else {
                Ok(file.metadata().clone())
            }
        })
    }
    fn delete<'a>(&'a self, key: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            self.deleted.lock().unwrap().push(key.into());
            Ok(())
        })
    }
}
#[tokio::test]
async fn storage_failures_rollback_previously_committed_files_and_http_limits_cover_framing() {
    let root = tempfile::tempdir().unwrap();
    let backend = Arc::new(FailSecond {
        count: AtomicUsize::new(0),
        deleted: Mutex::new(vec![]),
    });
    let service = UploadService::new(policy(2, 64, 128), root.path(), backend.clone())
        .await
        .unwrap();
    let app = router(service);
    let response = app
        .clone()
        .oneshot(request(multipart(&[
            ("file", "one.pdf", "application/pdf", PDF),
            ("file", "two.pdf", "application/pdf", PDF),
        ])))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(backend.deleted.lock().unwrap().len(), 1);
    let limited = app.layer(DefaultBodyLimit::max(32));
    assert_eq!(
        limited
            .oneshot(request(multipart(&[(
                "file",
                "one.pdf",
                "application/pdf",
                PDF
            )])))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
#[test]
fn upload_policy_rejects_unsafe_or_ambiguous_limits_and_mime_allowlists() {
    assert!(UploadPolicy::new(0, 1, 1, vec!["application/pdf".into()]).is_err());
    assert!(UploadPolicy::new(1, 10, 5, vec!["application/pdf".into()]).is_err());
    assert!(UploadPolicy::new(1, 10, 10, vec!["application/*".into()]).is_err());
}

#[cfg(feature = "upload-s3")]
#[tokio::test]
async fn s3_adapter_sends_signed_conditional_streamed_objects_and_deletes_by_generated_key() {
    use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
    use xecute::upload::S3Storage;
    let captured = Arc::new(Mutex::new(Vec::new()));
    let handler_capture = captured.clone();
    let mock = Router::new().fallback(move |request: Request<Body>| {
        let capture = handler_capture.clone();
        async move {
            let (parts, body) = request.into_parts();
            let status = if parts.method == axum::http::Method::DELETE {
                StatusCode::NO_CONTENT
            } else {
                StatusCode::OK
            };
            let bytes = to_bytes(body, 1024 * 1024).await.unwrap();
            capture.lock().unwrap().push((
                parts.method,
                parts.uri.path().to_owned(),
                parts.headers,
                bytes,
            ));
            (status, [("etag", "\"example-etag\"")])
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, mock).await.unwrap();
    });
    let client = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("us-east-1"))
            .credentials_provider(Credentials::new(
                "test-access",
                "test-secret",
                None,
                None,
                "test",
            ))
            .endpoint_url(format!("http://{address}"))
            .force_path_style(true)
            .build(),
    );
    let storage = Arc::new(S3Storage::new(client, "test-bucket", "private").unwrap());
    let staging = tempfile::tempdir().unwrap();
    let service = UploadService::new(policy(1, 64, 64), staging.path(), storage.clone())
        .await
        .unwrap();
    let response = router(service)
        .oneshot(request(multipart(&[(
            "file",
            "report.pdf",
            "application/pdf",
            PDF,
        )])))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let files: Vec<StoredFile> =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    storage.delete(&files[0].key).await.unwrap();
    let calls = captured.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, axum::http::Method::PUT);
    assert_eq!(calls[0].1, format!("/test-bucket/private/{}", files[0].key));
    assert_eq!(calls[0].2["if-none-match"], "*");
    assert_eq!(calls[0].2["content-type"], "application/pdf");
    assert_eq!(calls[0].2["x-amz-meta-sha256"], files[0].sha256);
    assert!(
        calls[0].2["authorization"]
            .to_str()
            .unwrap()
            .starts_with("AWS4-HMAC-SHA256")
    );
    assert!(calls[0].3.windows(PDF.len()).any(|bytes| bytes == PDF));
    assert_eq!(calls[1].0, axum::http::Method::DELETE);
    assert_eq!(std::fs::read_dir(staging.path()).unwrap().count(), 0);
    server.abort();
}

#[test]
fn s3_options_reject_credentials_in_endpoints_insecure_defaults_and_unsafe_prefixes() {
    use xecute::upload::S3Options;
    let options = || S3Options {
        bucket: "test-bucket".into(),
        region: "us-east-1".into(),
        endpoint: None,
        force_path_style: false,
        allow_http: false,
        prefix: "private/files".into(),
    };
    options().validate().unwrap();
    for endpoint in [
        "http://localhost:9000",
        "https://user:password@example.com",
        "https://example.com/private",
        "https://example.com?secret=value",
    ] {
        let mut invalid = options();
        invalid.endpoint = Some(endpoint.into());
        assert!(invalid.validate().is_err());
    }
    let mut local = options();
    local.endpoint = Some("http://localhost:9000".into());
    local.allow_http = true;
    local.validate().unwrap();
    let mut invalid = options();
    invalid.prefix = "../public".into();
    assert!(invalid.validate().is_err());
}

struct NeverFinish(Arc<tokio::sync::Notify>);
impl UploadStorage for NeverFinish {
    fn put<'a>(&'a self, _: &'a ValidatedUpload) -> StorageFuture<'a, StoredFile> {
        Box::pin(async move {
            self.0.notify_one();
            std::future::pending().await
        })
    }
    fn delete<'a>(&'a self, _: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn cancelling_a_request_releases_validated_staging_files() {
    let root = tempfile::tempdir().unwrap();
    let entered = Arc::new(tokio::sync::Notify::new());
    let service = UploadService::new(
        policy(1, 64, 64),
        root.path(),
        Arc::new(NeverFinish(entered.clone())),
    )
    .await
    .unwrap();
    let task = tokio::spawn(router(service).oneshot(request(multipart(&[(
        "file",
        "report.pdf",
        "application/pdf",
        PDF,
    )]))));
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}
