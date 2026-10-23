//! Static files share HTTP policies and never expose hidden paths or symlinks.

use crate::{configs::{Config, StaticConfig}, server};
use axum::{Router, body::to_bytes, http::{header, StatusCode}};
use tower::ServiceExt;
use super::support::request;

#[tokio::test]
async fn public_assets_support_get_head_ranges_and_reject_private_paths() {
    let root = std::env::temp_dir().join(format!("xecute-assets-{}", uuid::Uuid::new_v4()));
    let public = root.join("public");
    std::fs::create_dir_all(&public).unwrap();
    std::fs::write(public.join("asset.txt"), "hello world").unwrap();
    std::fs::write(public.join(".env"), "PRIVATE").unwrap();
    std::fs::write(root.join("outside.txt"), "PRIVATE").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("outside.txt"), public.join("link.txt")).unwrap();
    let mut config = Config { static_files: StaticConfig { enabled: true, directory: public, mount: "/assets".into() }, ..Config::default() };
    server::static_files::validate(&config.static_files).unwrap();
    let app = server::router(Router::new(), &config);
    let response = app.clone().oneshot(request("/assets/asset.txt", "127.0.0.1")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/plain"));
    assert!(response.headers().contains_key("x-request-id"));
    assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), "hello world");
    let mut head = request("/assets/asset.txt", "127.0.0.1");
    *head.method_mut() = axum::http::Method::HEAD;
    let response = app.clone().oneshot(head).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 1024).await.unwrap().is_empty());
    let mut range = request("/assets/asset.txt", "127.0.0.1");
    range.headers_mut().insert(header::RANGE, "bytes=0-4".parse().unwrap());
    let response = app.clone().oneshot(range).await.unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), "hello");
    for path in ["/assets/.env", "/assets/%2eenv", "/assets/%2e%2e/outside.txt", "/assets/missing.txt", "/assets/link.txt"] {
        assert_eq!(app.clone().oneshot(request(path, "127.0.0.1")).await.unwrap().status(), StatusCode::NOT_FOUND, "{path}");
    }
    config.static_files.enabled = false;
    assert_eq!(server::router(Router::new(), &config).oneshot(request("/assets/asset.txt", "127.0.0.1")).await.unwrap().status(), StatusCode::NOT_FOUND);
    std::fs::remove_dir_all(root).unwrap();
}
