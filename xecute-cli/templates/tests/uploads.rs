//! Enabled upload example persists validated files privately and documents multipart input.
use crate::{configs::Config, modules::uploads::UploadModule, server};
use axum::http::StatusCode;
use xecute::{testing::TestApp, upload::StoredFile};

#[tokio::test]
async fn enabled_uploads_are_private_validated_and_documented() {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.uploads.enabled = true;
    config.uploads.max_files = 2;
    config.uploads.directory = root.path().join("objects");
    let routes = UploadModule::router(&config).await.unwrap()
        .merge(server::docs::router(&config, UploadModule::api_operations(&config)).unwrap());
    let app = TestApp::new(server::router(routes, &config));
    let body = "--example\r\nContent-Disposition: form-data; name=\"file\"; filename=\"report.pdf\"\r\nContent-Type: application/pdf\r\n\r\n%PDF-1.7\nexample\n%%EOF\r\n--example--\r\n";
    let response = app.post("/uploads").header("content-type", "multipart/form-data; boundary=example")
        .body(body).send().await.unwrap().assert_status(StatusCode::CREATED);
    let files: Vec<StoredFile> = response.json().unwrap();
    assert_eq!(files.len(), 1);
    assert!(config.uploads.directory.join(&files[0].key).is_file());
    assert!(!config.uploads.directory.join("report.pdf").exists());
    let _ = app.get(&format!("/static/{}", files[0].key)).send().await.unwrap().assert_status(StatusCode::NOT_FOUND);
    let document: serde_json::Value = app.get("/docs/openapi.json").send().await.unwrap().json().unwrap();
    assert!(document["paths"]["/uploads"]["post"]["requestBody"]["content"]["multipart/form-data"].is_object());
    assert_eq!(document["paths"]["/uploads"]["post"]["requestBody"]["content"]["multipart/form-data"]["schema"]["properties"]["file"]["maxItems"], 2);
    assert!(document["paths"]["/uploads"]["post"]["requestBody"]["content"]["application/json"].is_null());
}
#[tokio::test]
async fn disabled_uploads_do_not_create_directories_or_expose_routes() {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::default(); config.uploads.directory = root.path().join("objects");
    let app = TestApp::new(server::router(UploadModule::router(&config).await.unwrap(), &config));
    let _ = app.post("/uploads").send().await.unwrap().assert_status(StatusCode::NOT_FOUND);
    assert!(!config.uploads.directory.exists());
    assert!(UploadModule::api_operations(&config).is_empty());
}
