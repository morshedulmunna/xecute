//! Behavioral regression coverage for configuration, errors, and provider lifecycles.

#![cfg(feature = "testing")]
use axum::{
    Json, Router,
    extract::ConnectInfo,
    http::{Method, StatusCode},
    routing::get,
};
use std::{net::SocketAddr, sync::Arc};
use xecute::{
    InjectError, ModuleDefinition, ProviderDefinition, Scope,
    testing::{TestApp, TestingModule},
};

trait Greeting: Send + Sync {
    fn greet(&self) -> &'static str;
}
struct Live;
impl Greeting for Live {
    fn greet(&self) -> &'static str {
        "live"
    }
}
struct Fake;
impl Greeting for Fake {
    fn greet(&self) -> &'static str {
        "mock"
    }
}
struct Gateway(Arc<dyn Greeting>);
struct Service(Arc<Gateway>);
fn module() -> ModuleDefinition {
    ModuleDefinition::new("Example")
        .provider(ProviderDefinition::value(Gateway(Arc::new(Live))))
        .provider(ProviderDefinition::new::<Service, _>(
            Scope::Singleton,
            vec![xecute::ProviderKey::of::<Gateway>()],
            |c| Ok(Service(c.resolve()?)),
        ))
}
#[test]
fn mock_overrides_apply_before_initialization_and_are_isolated() {
    let fake = TestingModule::new()
        .module(module())
        .override_value(Gateway(Arc::new(Fake)))
        .compile()
        .unwrap();
    let live = TestingModule::new().module(module()).compile().unwrap();
    assert_eq!(fake.resolve::<Service>().unwrap().0.0.greet(), "mock");
    assert_eq!(live.resolve::<Service>().unwrap().0.0.greet(), "live");
    assert!(matches!(
        TestingModule::new()
            .module(module())
            .override_value(12_u32)
            .compile(),
        Err(InjectError::Missing(_))
    ));
    assert!(matches!(
        TestingModule::new()
            .module(module())
            .override_value(Gateway(Arc::new(Fake)))
            .override_value(Gateway(Arc::new(Fake)))
            .compile(),
        Err(InjectError::DuplicateProvider(_))
    ));
}
#[tokio::test]
async fn real_router_receives_json_headers_peers_and_middleware() {
    let router = Router::new().route(
        "/",
        get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move {
            Json(serde_json::json!({"ip": peer.ip().to_string()}))
        })
        .post(|Json(value): Json<serde_json::Value>| async move {
            (StatusCode::CREATED, Json(value))
        }),
    );
    let app = TestApp::new(xecute::exceptions::exception_boundary(router));
    let response = app
        .get("/")
        .peer("192.0.2.1:4000".parse().unwrap())
        .send()
        .await
        .unwrap()
        .assert_status(StatusCode::OK)
        .assert_json(&serde_json::json!({"ip":"192.0.2.1"}));
    assert!(response.headers.contains_key("x-request-id"));
    let _ = app
        .post("/")
        .json(&serde_json::json!({"value":42}))
        .send()
        .await
        .unwrap()
        .assert_status(StatusCode::CREATED)
        .assert_json(&serde_json::json!({"value":42}));
    let _ = app
        .post("/")
        .header("content-type", "application/json")
        .body("invalid-json")
        .send()
        .await
        .unwrap()
        .assert_status(StatusCode::BAD_REQUEST);
    let response = app
        .request(Method::HEAD, "/")
        .send()
        .await
        .unwrap()
        .assert_status(StatusCode::OK);
    assert!(response.body.is_empty());
    assert!(
        app.get("/")
            .header("bad\nheader", "value")
            .send()
            .await
            .is_err()
    );
    assert!(app.get("/invalid uri").send().await.is_err());
    assert!(
        TestApp::new(Router::new().route("/", get(|| async { "too many bytes" })))
            .response_limit(3)
            .get("/")
            .send()
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stalled_handlers_are_bounded_by_the_test_timeout() {
    let router = Router::new().route("/", get(|| async { std::future::pending::<()>().await }));
    let app = TestApp::new(router).timeout(std::time::Duration::from_millis(10));
    assert!(
        app.get("/")
            .send()
            .await
            .unwrap_err()
            .to_string()
            .contains("timeout")
    );
}

#[tokio::test]
async fn cancelled_requests_release_their_scoped_instances() {
    struct RequestValue;
    let observed = Arc::new(std::sync::Mutex::new(std::sync::Weak::<RequestValue>::new()));
    let container = Arc::new(
        xecute::Container::builder()
            .provider(ProviderDefinition::request::<RequestValue, _>(
                vec![],
                move |_| Ok(RequestValue),
            ))
            .build()
            .unwrap(),
    );
    let entered = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handler_entered = entered.clone();
    let handler_observed = observed.clone();
    let router = Router::new().route(
        "/",
        get(move |xecute::Inject(value): xecute::Inject<RequestValue>| {
            handler_entered.store(true, std::sync::atomic::Ordering::SeqCst);
            *handler_observed.lock().unwrap() = Arc::downgrade(&value);
            async move {
                let _keep_alive = value;
                std::future::pending::<()>().await
            }
        }),
    );
    let app = TestApp::new(xecute::with_container(router, container))
        .timeout(std::time::Duration::from_millis(10));
    assert!(app.get("/").send().await.is_err());
    assert!(entered.load(std::sync::atomic::Ordering::SeqCst));
    assert!(observed.lock().unwrap().upgrade().is_none());
}
