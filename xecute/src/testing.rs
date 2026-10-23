//! Isolated provider overrides and in-process HTTP assertions.
//! Enable the `testing` feature in dev-dependencies; no listening socket is required.

use crate::{
    Container, ContainerBuilder, InjectError, ModuleDefinition, ModuleGraph, Provider,
    ProviderDefinition,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::{self, HeaderMap, Method, Request, StatusCode},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{fmt, net::SocketAddr, time::Duration};
use tower::ServiceExt;

/// Build a validated test container with explicit replacements of registered providers.
#[derive(Default)]
pub struct TestingModule {
    modules: Vec<ModuleDefinition>,
    builder: ContainerBuilder,
    overrides: Vec<ProviderDefinition>,
}
impl TestingModule {
    /// Start an isolated test module. Each compile creates independent singleton caches.
    pub fn new() -> Self {
        Self::default()
    }
    /// Include application module definitions without mutating production registrations.
    #[must_use]
    pub fn module(mut self, module: ModuleDefinition) -> Self {
        self.modules.push(module);
        self
    }
    /// Include a provider factory.
    #[must_use]
    pub fn provider(mut self, provider: ProviderDefinition) -> Self {
        self.builder = self.builder.provider(provider);
        self
    }
    /// Include a macro-generated provider.
    #[must_use]
    pub fn injectable<T: Provider>(self) -> Self {
        self.provider(T::definition())
    }
    /// Replace an existing provider with a mock value of the same registered Rust type.
    /// For trait mocks, register a concrete adapter holding an Arc<dyn YourTrait>.
    #[must_use]
    pub fn override_value<T: Send + Sync + 'static>(self, value: T) -> Self {
        self.override_provider(ProviderDefinition::value(value))
    }
    /// Replace an existing factory and lifecycle; duplicate or unknown overrides are errors.
    #[must_use]
    pub fn override_provider(mut self, replacement: ProviderDefinition) -> Self {
        self.overrides.push(replacement);
        self
    }
    /// Validate the final graph and eagerly initialize singletons before a test sends requests.
    pub fn compile(self) -> Result<Container, InjectError> {
        // Keep the standalone providers in their own synthetic module so the same graph
        // validation and override rules apply to both entry points.
        let mut modules = self.modules;
        let extra = ModuleDefinition::new("xecute::testing::standalone");
        let mut extra = extra;
        for provider in self.builder.into_definitions() {
            extra = extra.provider(provider);
        }
        modules.push(extra);
        let container = ModuleGraph::new(modules).build_with_overrides(self.overrides)?;
        container.initialize()?;
        Ok(container)
    }
}

/// HTTP setup, serialization, or bounded body-read failure.
#[derive(Debug)]
pub struct TestError(String);
impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for TestError {}

/// An in-process client for the actual application Router, including its middleware.
#[derive(Clone)]
pub struct TestApp {
    router: Router,
    body_limit: usize,
    timeout: Duration,
}
impl TestApp {
    /// Use an assembled router; callers decide whether to include generated server policies.
    pub fn new(router: Router) -> Self {
        Self {
            router,
            body_limit: 1_048_576,
            timeout: Duration::from_secs(5),
        }
    }
    /// Set the maximum buffered response size (default 1 MiB).
    #[must_use]
    pub fn response_limit(mut self, bytes: usize) -> Self {
        self.body_limit = bytes;
        self
    }
    /// Bound the entire request and body collection (default five seconds).
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    /// Begin an HTTP request with a loopback peer, accepted by generated rate-limit middleware.
    pub fn request(&self, method: Method, uri: &str) -> TestRequest {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(extensions) = builder.extensions_mut() {
            extensions.insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
        }
        TestRequest {
            router: self.router.clone(),
            builder,
            body: Ok(Body::empty()),
            limit: self.body_limit,
            timeout: self.timeout,
        }
    }
    /// Begin a GET request.
    pub fn get(&self, uri: &str) -> TestRequest {
        self.request(Method::GET, uri)
    }
    /// Begin a POST request.
    pub fn post(&self, uri: &str) -> TestRequest {
        self.request(Method::POST, uri)
    }
    /// Begin a PATCH request.
    pub fn patch(&self, uri: &str) -> TestRequest {
        self.request(Method::PATCH, uri)
    }
    /// Begin a DELETE request.
    pub fn delete(&self, uri: &str) -> TestRequest {
        self.request(Method::DELETE, uri)
    }
}
/// Request builder supporting JSON, headers, body, and explicit peer addresses.
pub struct TestRequest {
    router: Router,
    builder: http::request::Builder,
    body: Result<Body, TestError>,
    limit: usize,
    timeout: Duration,
}
impl TestRequest {
    /// Set a request header; invalid names or values are reported by send().
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.builder = self.builder.header(name, value);
        self
    }
    /// Supply a JSON payload and Content-Type.
    #[must_use]
    pub fn json<T: Serialize>(mut self, value: &T) -> Self {
        self.builder = self
            .builder
            .header(http::header::CONTENT_TYPE, "application/json");
        self.body = serde_json::to_vec(value)
            .map(Body::from)
            .map_err(|_| TestError("could not serialize test JSON".into()));
        self
    }
    /// Supply raw bytes, for example to test malformed payload rejection.
    #[must_use]
    pub fn body(mut self, body: impl Into<Body>) -> Self {
        self.body = Ok(body.into());
        self
    }
    /// Override the socket peer to exercise per-peer policies.
    #[must_use]
    pub fn peer(mut self, peer: SocketAddr) -> Self {
        if let Some(extensions) = self.builder.extensions_mut() {
            extensions.insert(ConnectInfo(peer));
        }
        self
    }
    /// Run the actual router and collect the response with a strict size bound.
    pub async fn send(self) -> Result<TestResponse, TestError> {
        tokio::time::timeout(self.timeout, async move {
            let request = self
                .builder
                .body(self.body?)
                .map_err(|error| TestError(error.to_string()))?;
            let response = self
                .router
                .oneshot(request)
                .await
                .expect("Router is infallible");
            let (parts, body) = response.into_parts();
            let bytes = to_bytes(body, self.limit).await.map_err(|_| {
                TestError("response body failed or exceeded the configured test limit".into())
            })?;
            Ok(TestResponse {
                status: parts.status,
                headers: parts.headers,
                body: bytes.to_vec(),
            })
        })
        .await
        .map_err(|_| TestError("test request exceeded its timeout".into()))?
    }
}
/// Collected HTTP response with typed JSON access and chainable assertions.
#[derive(Debug)]
pub struct TestResponse {
    /// HTTP response status.
    pub status: StatusCode,
    /// Response headers, including request correlation and policy headers.
    pub headers: HeaderMap,
    /// Raw body bytes, bounded by TestApp::response_limit.
    pub body: Vec<u8>,
}
impl TestResponse {
    /// Deserialize the response into a DTO or serde_json::Value.
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
    /// Assert an exact status code without printing potentially private response bodies.
    #[must_use]
    pub fn assert_status(self, expected: StatusCode) -> Self {
        assert_eq!(self.status, expected);
        self
    }
    /// Assert exact JSON equality (object property order is irrelevant).
    #[must_use]
    pub fn assert_json(self, expected: &serde_json::Value) -> Self {
        assert_eq!(
            &self
                .json::<serde_json::Value>()
                .expect("response must be JSON"),
            expected
        );
        self
    }
}
