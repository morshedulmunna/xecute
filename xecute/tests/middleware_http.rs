//! Verify middleware scopes, ordering, early returns, and validation boundaries.

use axum::{
    Extension,
    body::{Body, to_bytes},
    http::{Request as HttpRequest, StatusCode},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use tower::ServiceExt;
use xecute::{
    middleware::{Next, Request, Response},
    prelude::*,
};

#[derive(Clone, Default)]
struct Trace(Arc<Mutex<Vec<&'static str>>>);

async fn record(request: Request, next: Next, enter: &'static str, exit: &'static str) -> Response {
    let trace = request.extensions().get::<Trace>().unwrap().clone();
    trace.0.lock().unwrap().push(enter);
    let response = next.run(request).await;
    trace.0.lock().unwrap().push(exit);
    response
}
async fn app_first(request: Request, next: Next) -> Response {
    record(request, next, "app1 in", "app1 out").await
}
async fn app_second(request: Request, next: Next) -> Response {
    record(request, next, "app2 in", "app2 out").await
}
async fn controller_first(request: Request, next: Next) -> Response {
    record(request, next, "controller1 in", "controller1 out").await
}
async fn controller_second(request: Request, next: Next) -> Response {
    record(request, next, "controller2 in", "controller2 out").await
}
async fn route_first(request: Request, next: Next) -> Response {
    record(request, next, "route1 in", "route1 out").await
}
async fn route_second(request: Request, next: Next) -> Response {
    record(request, next, "route2 in", "route2 out").await
}
async fn deny(_: Request, _: Next) -> StatusCode {
    StatusCode::FORBIDDEN
}

#[derive(serde::Deserialize, validator::Validate, schemars::JsonSchema)]
struct Payload {
    #[validate(length(min = 3))]
    name: String,
}

#[controller("/pipeline")]
#[middleware(controller_first, controller_second)]
#[derive(Injectable)]
struct PipelineController {
    #[inject(default)]
    calls: AtomicUsize,
}

#[routes]
impl PipelineController {
    #[get("")]
    #[middleware(route_first, route_second)]
    async fn get(&self, trace: Extension<Trace>) -> &'static str {
        self.calls.fetch_add(1, Ordering::SeqCst);
        trace.0.0.lock().unwrap().push("handler");
        "ok"
    }

    #[post("")]
    async fn create(&self, #[body] payload: Payload) -> String {
        self.calls.fetch_add(1, Ordering::SeqCst);
        payload.name
    }

    #[get("/denied")]
    #[middleware(deny)]
    async fn denied(&self) -> &'static str {
        self.calls.fetch_add(1, Ordering::SeqCst);
        "must never run"
    }
}

fn app(trace: Trace) -> (axum::Router, Arc<PipelineController>) {
    let controller = xecute::Container::builder()
        .injectable::<PipelineController>()
        .build()
        .unwrap()
        .resolve::<PipelineController>()
        .unwrap();
    let app = Application::new(PipelineController::router(controller.clone()))
        .use_middleware(app_first)
        .use_middleware(app_second)
        .into_router()
        .layer(Extension(trace));
    (app, controller)
}

#[tokio::test]
async fn scopes_run_in_declaration_order_and_unwind_in_reverse() {
    let trace = Trace::default();
    let (app, controller) = app(trace.clone());
    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/pipeline")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), "ok");
    assert_eq!(controller.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *trace.0.lock().unwrap(),
        vec![
            "app1 in",
            "app2 in",
            "controller1 in",
            "controller2 in",
            "route1 in",
            "route2 in",
            "handler",
            "route2 out",
            "route1 out",
            "controller2 out",
            "controller1 out",
            "app2 out",
            "app1 out",
        ]
    );
}

#[tokio::test]
async fn route_scope_is_method_specific_and_validation_still_short_circuits() {
    let trace = Trace::default();
    let (app, controller) = app(trace.clone());
    for (body, expected) in [
        (r#"{"name":"valid"}"#, 200),
        (r#"{"name":"a"}"#, 422),
        ("broken", 400),
    ] {
        let response = app
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/pipeline")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
    }
    assert_eq!(controller.calls.load(Ordering::SeqCst), 1);
    assert!(!trace.0.lock().unwrap().contains(&"route1 in"));
    assert_eq!(
        trace
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|s| **s == "controller1 in")
            .count(),
        3
    );
}

#[tokio::test]
async fn early_response_skips_handler_and_unrelated_routes_skip_controller_middleware() {
    let trace = Trace::default();
    let (app, controller) = app(trace.clone());
    let response = app
        .clone()
        .oneshot(
            HttpRequest::builder()
                .uri("/pipeline/denied")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(controller.calls.load(Ordering::SeqCst), 0);
    trace.0.lock().unwrap().clear();
    let response = app
        .oneshot(
            HttpRequest::builder()
                .uri("/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        *trace.0.lock().unwrap(),
        ["app1 in", "app2 in", "app2 out", "app1 out"]
    );
}

#[tokio::test]
async fn app_middleware_accepts_captured_state_and_can_return_early() {
    let (router, controller) = app(Trace::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let router = Application::new(router)
        .use_middleware(move |_: Request, _: Next| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                StatusCode::UNAUTHORIZED
            }
        })
        .into_router();
    let response = router
        .oneshot(
            HttpRequest::builder()
                .uri("/pipeline")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(controller.calls.load(Ordering::SeqCst), 0);
}

#[controller("/impl")]
#[derive(Injectable)]
struct ImplController {}

#[routes]
#[middleware(controller_first)]
impl ImplController {
    #[get("")]
    #[middleware(route_first)]
    #[middleware(route_second)]
    async fn get(&self) -> &'static str {
        "ok"
    }
}

#[derive(Injectable)]
struct LegacyPipelineController {}

#[controller("/legacy-pipeline")]
#[middleware(controller_first)]
impl LegacyPipelineController {
    #[get("")]
    #[middleware(route_first)]
    async fn get() -> &'static str {
        "ok"
    }
}

#[tokio::test]
async fn impl_annotations_and_legacy_controller_syntax_preserve_order() {
    for (router, uri, expected) in [
        (
            ImplController::router(Arc::new(ImplController {})),
            "/impl",
            vec![
                "controller1 in",
                "route1 in",
                "route2 in",
                "route2 out",
                "route1 out",
                "controller1 out",
            ],
        ),
        (
            LegacyPipelineController::router(Arc::new(LegacyPipelineController {})),
            "/legacy-pipeline",
            vec![
                "controller1 in",
                "route1 in",
                "route1 out",
                "controller1 out",
            ],
        ),
    ] {
        let trace = Trace::default();
        let response = router
            .layer(Extension(trace.clone()))
            .oneshot(HttpRequest::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(*trace.0.lock().unwrap(), expected);
    }
}
