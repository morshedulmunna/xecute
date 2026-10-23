//! Behavioral regression coverage for configuration, errors, and provider lifecycles.

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::get,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use xecute::{Container, Inject, Injectable, controller, routes};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Unique(usize);
impl Default for Unique {
    fn default() -> Self {
        Self(NEXT.fetch_add(1, Ordering::SeqCst))
    }
}
#[derive(Injectable)]
#[scope("request")]
struct RequestState {
    #[inject(default)]
    id: Unique,
}
#[derive(Injectable)]
#[scope("transient")]
struct Worker {
    state: Arc<RequestState>,
}
#[controller("/scoped")]
#[derive(Injectable)]
#[scope("request")]
struct Controller {
    worker: Arc<Worker>,
    state: Arc<RequestState>,
}
#[routes]
impl Controller {
    #[get("")]
    async fn get(&self, other: Inject<RequestState>) -> Json<serde_json::Value> {
        Json(
            serde_json::json!({"id": self.state.id.0, "same": Arc::ptr_eq(&self.worker.state, &other.0) && Arc::ptr_eq(&self.state, &other.0)}),
        )
    }
}
#[tokio::test]
async fn generated_scoped_router_resolves_nested_providers_per_request() {
    let container = Arc::new(
        Container::builder()
            .injectable::<RequestState>()
            .injectable::<Worker>()
            .injectable::<Controller>()
            .build()
            .unwrap(),
    );
    let app = Controller::scoped_router(container);
    let mut ids = Vec::new();
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/scoped")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
        assert_eq!(value["same"], true);
        ids.push(value["id"].clone());
    }
    assert_ne!(ids[0], ids[1]);
}
#[tokio::test]
async fn missing_request_scope_returns_safe_internal_error() {
    let app = Router::new().route("/", get(|Inject(_): Inject<RequestState>| async { "ok" }));
    let response = app.oneshot(Request::new(Body::empty())).await.unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(response.into_body(), 1024).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"]["message"], "Internal server error");
}

#[tokio::test]
async fn outer_middleware_and_controller_share_the_same_request_context() {
    let container = Arc::new(
        Container::builder()
            .injectable::<RequestState>()
            .injectable::<Worker>()
            .injectable::<Controller>()
            .build()
            .unwrap(),
    );
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = seen.clone();
    let app = Controller::scoped_router(container.clone()).layer(axum::middleware::from_fn(
        move |Inject(state): Inject<RequestState>,
              request: axum::extract::Request,
              next: axum::middleware::Next| {
            captured.lock().unwrap().push(state.id.0);
            async move { next.run(request).await }
        },
    ));
    let app = xecute::with_container(app, container);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/scoped")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let value: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap()).unwrap();
    assert_eq!(value["id"], seen.lock().unwrap()[0]);
}
