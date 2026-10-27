//! Executable guards without requiring JWT support.
use axum::{
    Extension, Router,
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
fn record(request: &Request, name: &'static str) {
    request
        .extensions()
        .get::<Trace>()
        .unwrap()
        .0
        .lock()
        .unwrap()
        .push(name);
}
async fn app_middleware(request: Request, next: Next) -> Response {
    record(&request, "app middleware");
    next.run(request).await
}
async fn controller_middleware(request: Request, next: Next) -> Response {
    record(&request, "controller middleware");
    next.run(request).await
}
async fn route_middleware(request: Request, next: Next) -> Response {
    record(&request, "route middleware");
    next.run(request).await
}
async fn app_guard(request: Request) -> Result<Request, HttpError> {
    record(&request, "app guard");
    Ok(request)
}
async fn controller_guard(request: Request) -> Result<Request, HttpError> {
    record(&request, "controller guard");
    Ok(request)
}
async fn route_first(request: Request) -> Result<Request, HttpError> {
    record(&request, "route first");
    Ok(request)
}
async fn route_second(request: Request) -> Result<Request, HttpError> {
    record(&request, "route second");
    Ok(request)
}
async fn deny(_: Request) -> Result<Request, HttpError> {
    Err(HttpError::new(StatusCode::FORBIDDEN))
}

#[derive(serde::Deserialize, validator::Validate, schemars::JsonSchema)]
struct Payload {
    #[validate(length(min = 3))]
    name: String,
}

#[controller("/guards")]
#[middleware(controller_middleware)]
#[guard(controller_guard)]
#[derive(Injectable)]
struct TestController {
    #[inject(default)]
    calls: AtomicUsize,
}
#[routes]
impl TestController {
    #[post("")]
    #[middleware(route_middleware)]
    #[guard(route_first, route_second)]
    async fn create(&self, trace: Extension<Trace>, #[body] body: Payload) -> String {
        self.calls.fetch_add(1, Ordering::SeqCst);
        trace.0.0.lock().unwrap().push("handler");
        body.name
    }
    #[post("/denied")]
    #[guard(deny)]
    async fn denied(&self, #[body] body: Payload) -> String {
        self.calls.fetch_add(1, Ordering::SeqCst);
        body.name
    }
    #[get("/same")]
    #[guard(deny)]
    async fn denied_get(&self) -> &'static str {
        "unreachable"
    }
    #[post("/same")]
    async fn public_post(&self) -> &'static str {
        "ok"
    }
}

fn controller() -> Arc<TestController> {
    xecute::Container::builder()
        .injectable::<TestController>()
        .build()
        .unwrap()
        .resolve()
        .unwrap()
}
fn request(method: &str, path: &str, body: &'static str) -> HttpRequest<Body> {
    HttpRequest::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn middleware_and_guards_run_in_documented_scope_order_before_body_extraction() {
    let controller = controller();
    let trace = Trace::default();
    let app = Application::new(TestController::router(controller.clone()))
        .use_guard(app_guard)
        .use_middleware(app_middleware)
        .into_router()
        .layer(Extension(trace.clone()));
    let response = app
        .oneshot(request("POST", "/guards", r#"{"name":"Munna"}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), "Munna");
    assert_eq!(
        *trace.0.lock().unwrap(),
        [
            "app middleware",
            "app guard",
            "controller middleware",
            "controller guard",
            "route middleware",
            "route first",
            "route second",
            "handler"
        ]
    );
    assert_eq!(controller.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn denial_precedes_validation_and_does_not_invoke_handler() {
    let controller = controller();
    let app = TestController::router(controller.clone()).layer(Extension(Trace::default()));
    for body in ["malformed", r#"{"name":"x"}"#, r#"{"name":"valid"}"#] {
        assert_eq!(
            app.clone()
                .oneshot(request("POST", "/guards/denied", body))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(controller.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn method_guard_does_not_protect_another_method_or_unrelated_paths() {
    let app = TestController::router(controller()).layer(Extension(Trace::default()));
    for (method, path, expected) in [
        ("GET", "/guards/same", 403),
        ("POST", "/guards/same", 200),
        ("DELETE", "/guards/same", 405),
        ("GET", "/missing", 404),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(request(method, path, ""))
                .await
                .unwrap()
                .status()
                .as_u16(),
            expected,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn global_guards_run_in_order_and_stop_after_first_denial() {
    let trace = Trace::default();
    let app = Application::new(Router::new().route(
        "/",
        axum::routing::get(|| async {
            panic!("handler ran");
            #[allow(unreachable_code)]
            ""
        }),
    ))
    .use_guard(route_first)
    .use_guard(deny)
    .use_guard(route_second)
    .into_router()
    .layer(Extension(trace.clone()));
    assert_eq!(
        app.oneshot(request("GET", "/", "")).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(*trace.0.lock().unwrap(), ["route first"]);
}

#[tokio::test]
async fn scoped_controller_guards_can_resolve_request_dependencies() {
    async fn policy(request: Request) -> Result<Request, HttpError> {
        let scope = request.extensions().get::<xecute::RequestScope>().unwrap();
        assert_eq!(*scope.resolve::<u32>().unwrap(), 42);
        Ok(request)
    }
    #[controller("/scoped")]
    #[guard(policy)]
    #[derive(Injectable)]
    struct ScopedController {}
    #[routes]
    impl ScopedController {
        #[get("")]
        async fn get(&self) -> &'static str {
            "ok"
        }
    }
    let container = Arc::new(
        xecute::Container::builder()
            .value(42_u32)
            .injectable::<ScopedController>()
            .build()
            .unwrap(),
    );
    let app = ScopedController::scoped_router(container);
    assert_eq!(
        app.oneshot(request("GET", "/scoped", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn legacy_impl_controller_guards_execute() {
    struct Legacy;
    #[controller("/legacy")]
    #[guard(deny)]
    impl Legacy {
        #[get("")]
        async fn get(&self) -> &'static str {
            "unreachable"
        }
    }
    let app = Legacy::router(Arc::new(Legacy));
    assert_eq!(
        app.oneshot(request("GET", "/legacy", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
}

async fn module_first(request: Request) -> Result<Request, HttpError> {
    record(&request, "module first");
    let scope = request.extensions().get::<xecute::RequestScope>().unwrap();
    assert_eq!(*scope.resolve::<u32>().unwrap(), 42);
    Ok(request)
}
async fn module_second(request: Request) -> Result<Request, HttpError> {
    record(&request, "module second");
    Ok(request)
}

#[controller("/sibling")]
#[derive(Injectable)]
struct SiblingController {}
#[routes]
impl SiblingController {
    #[get("")]
    async fn get(&self) -> &'static str {
        "sibling"
    }
}

#[xecute::module(
    providers = [TestController, SiblingController],
    controllers = [TestController, SiblingController],
    guards = [module_first],
)]
#[guard(module_second)]
struct GuardedModule;

#[xecute::module(controllers = [TestController, SiblingController])]
#[guard(module_first, deny, module_second)]
struct DeniedModule;

#[tokio::test]
async fn module_guards_cover_every_controller_in_order_with_request_scope() {
    let container = Arc::new(
        xecute::ModuleGraph::new(vec![
            GuardedModule::definition().provider(xecute::ProviderDefinition::value(42_u32)),
        ])
        .build()
        .unwrap(),
    );
    let trace = Trace::default();
    let app = Application::new(GuardedModule::scoped_router(container.clone()))
        .use_guard(app_guard)
        .use_middleware(app_middleware)
        .into_router()
        .layer(Extension(trace.clone()));
    assert_eq!(
        app.clone()
            .oneshot(request("POST", "/guards", r#"{"name":"Munna"}"#))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        *trace.0.lock().unwrap(),
        [
            "app middleware",
            "app guard",
            "module first",
            "module second",
            "controller middleware",
            "controller guard",
            "route middleware",
            "route first",
            "route second",
            "handler"
        ]
    );
    trace.0.lock().unwrap().clear();
    assert_eq!(
        app.oneshot(request("GET", "/sibling", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        *trace.0.lock().unwrap(),
        [
            "app middleware",
            "app guard",
            "module first",
            "module second"
        ]
    );
    assert_eq!(
        container
            .resolve::<TestController>()
            .unwrap()
            .calls
            .load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn module_denial_stops_inner_guards_and_body_extraction_without_leaking() {
    let container = Arc::new(
        xecute::ModuleGraph::new(vec![
            GuardedModule::definition().provider(xecute::ProviderDefinition::value(42_u32)),
        ])
        .build()
        .unwrap(),
    );
    let trace = Trace::default();
    let app = DeniedModule::scoped_router(container.clone())
        .merge(Router::new().route("/public", axum::routing::get(|| async { "public" })))
        .layer(Extension(trace.clone()));
    for (method, path, status) in [
        ("POST", "/guards", 403),
        ("GET", "/sibling", 403),
        ("GET", "/public", 200),
        ("GET", "/missing", 404),
        ("DELETE", "/sibling", 403),
    ] {
        trace.0.lock().unwrap().clear();
        let response = app
            .clone()
            .oneshot(request(method, path, "malformed"))
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "{method} {path}");
        let expected = if status == 403 {
            vec!["module first"]
        } else {
            vec![]
        };
        assert_eq!(*trace.0.lock().unwrap(), expected);
    }
    assert_eq!(
        container
            .resolve::<TestController>()
            .unwrap()
            .calls
            .load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn root_module_wrapper_guards_all_assembled_modules_before_feature_guards() {
    #[xecute::module(imports = [GuardedModule])]
    #[guard(deny)]
    struct RootModule;
    let container = Arc::new(
        xecute::ModuleGraph::new(vec![
            RootModule::definition(),
            GuardedModule::definition().provider(xecute::ProviderDefinition::value(42_u32)),
        ])
        .build()
        .unwrap(),
    );
    let trace = Trace::default();
    let routes = GuardedModule::scoped_router(container.clone());
    let app = xecute::with_container(RootModule::apply_guards(routes), container)
        .layer(Extension(trace.clone()));
    for path in ["/guards/same", "/sibling"] {
        assert_eq!(
            app.clone()
                .oneshot(request("GET", path, ""))
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert!(trace.0.lock().unwrap().is_empty());
}
