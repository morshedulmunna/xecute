//! Version groups route and document independent contracts with safe startup validation.
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::get,
};
use tower::ServiceExt;
use xecute::{ApiOperation, ApiResponse, ApiVersion, ApiVersions};
fn operation(id: &'static str, path: &str) -> ApiOperation {
    ApiOperation {
        method: "GET",
        path: path.into(),
        summary: "Contract",
        tag: "Example",
        operation_id: id,
        body_type: None,
        body_schema: None,
        query_schema: None,
        responses: vec![ApiResponse {
            status: 200,
            description: "OK",
            json_type: None,
            schema: None,
        }],
    }
}
#[tokio::test]
async fn versions_route_independently_and_documents_have_matching_prefixed_paths() {
    let versions = ApiVersions::new()
        .with(
            ApiVersion::new(
                "v1",
                Router::new().route("/items", get(|| async { "old" })),
                vec![operation("list", "/items")],
            )
            .unwrap(),
        )
        .unwrap()
        .with(
            ApiVersion::new(
                "v2",
                Router::new().route("/records", get(|| async { "new" })),
                vec![operation("list", "/records")],
            )
            .unwrap(),
        )
        .unwrap();
    let app = versions.mount(Router::new());
    for (path, status, body) in [
        ("/v1/items", StatusCode::OK, "old"),
        ("/v2/records", StatusCode::OK, "new"),
        ("/v2/items", StatusCode::NOT_FOUND, ""),
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), body);
    }
    let documents = versions.documents("Example").unwrap();
    assert!(documents["v1"]["paths"]["/v1/items"].is_object());
    assert!(documents["v1"]["paths"]["/v2/records"].is_null());
    assert_eq!(documents["v2"]["info"]["version"], "v2");
    assert_eq!(
        documents["v2"]["paths"]["/v2/records"]["get"]["operationId"],
        "list"
    );
}
#[test]
fn version_names_duplicates_and_contract_conflicts_fail_before_serving() {
    for name in ["", "v0", "v01", "V1", "v1/other", "v4294967296", "v1?query"] {
        assert!(ApiVersion::new(name, Router::new(), vec![]).is_err());
    }
    let group = || ApiVersion::new("v1", Router::new(), vec![]).unwrap();
    assert!(
        ApiVersions::new()
            .with(group())
            .unwrap()
            .with(group())
            .is_err()
    );
    assert!(
        ApiVersion::new(
            "v1",
            Router::new(),
            vec![operation("same", "/one"), operation("same", "/two")]
        )
        .is_err()
    );
    assert!(ApiVersion::new("v1", Router::new(), vec![operation("relative", "bad-path")]).is_err());
}
