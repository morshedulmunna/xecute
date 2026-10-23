# xecute

`xecute` is a small dependency-injection runtime and macro facade for Axum applications.
It provides typed providers, application/request scopes, module metadata, and
startup validation. It is intentionally independent of any larger framework.

```rust
use xecute::{Container, Provider, InjectError};

struct Clock;
impl Provider for Clock {
    fn provide(_: &Container) -> Result<Self, InjectError> { Ok(Self) }
}

let container = Container::builder().injectable::<Clock>().build()?;
let clock = container.resolve::<Clock>()?;
# Ok::<(), xecute::InjectError>(())
```

It re-exports `#[injectable]`, `#[module]`, and the routing macros, so applications
need only this one dependency.

## Controller methods and validated payloads

Import `xecute::prelude::*`. Annotate a controller struct with
`#[controller("/v1/auth")]` and `#[derive(Injectable)]`, then put `#[routes]` on
its implementation. HTTP handlers can take `&self`, `#[body] request: Dto`, and
`#[query] request: Dto`. DTOs derive `serde::Deserialize`, `validator::Validate`,
and `schemars::JsonSchema`. The generated adapters decode and validate before
calling the handler. Invalid values return 422; submitted values are omitted
from public validation errors. JSON syntax/content-type errors retain their
HTTP status.

`#[api(summary = "...", tag = "...", operation_id = "...")]`,
`#[body(json = RequestDto)]`, and `#[resp(200, "Description", json = ReplyDto)]`
populate `Controller::api_operations()`. Pass these operations to
`xecute::openapi_document` to build OpenAPI 3.1 with DTO/response schemas.
Generated CLI applications register this document with their default Swagger UI.

## Injectable repositories

Repositories can derive `Injectable` like controllers. Normal dependency fields,
such as `database: Arc<Database>`, are resolved from the container. Mark local
state with `#[inject(default)]` to initialize it using `Default` without treating
it as a dependency. Register the repository in the module’s `providers` list;
the default singleton scope shares one instance across injected services.

## Middleware

Middleware functions accept `Request` and `Next` from `xecute::middleware` and
return any `IntoResponse` value. Call `next.run(request).await` to continue;
return a response directly to stop the pipeline. Register functions in request
execution order; response processing unwinds in reverse.

```rust,ignore
use xecute::prelude::*;
use xecute::middleware::{Request, Next, Response};

async fn auth(request: Request, next: Next) -> Response {
    // Authenticate and insert identity into request.extensions_mut() here.
    next.run(request).await
}

#[controller("/users")]
#[middleware(auth)]
#[derive(Injectable)]
struct UserController {}

#[routes]
impl UserController {
    #[get("")]
    #[middleware(audit)]
    async fn list(&self) -> &'static str { "users" }
}

// Global middleware; closures can capture Arc-backed application dependencies.
let app = Application::new(UserController::router(controller))
    .use_middleware(request_context)
    .use_middleware(logging)
    .into_router();
```

Place controller `#[middleware(...)]` below `#[controller(...)]` on the struct,
or below `#[routes]` on its impl. Route middleware belongs on a handler inside
that impl. Multiple functions and repeated middleware attributes preserve their
written order. Execution is app → controller struct → controller impl → route →
extractors/validation → handler. Controller middleware applies only to matched
controller routes; route middleware applies only to its HTTP method. App
middleware wraps existing routes and fallback responses. Add all routes before
building the application pipeline.

These are HTTP middleware; command/query handler pipelines are separate.


## Typed configuration

These APIs are prepared for runtime 0.1.7/macros 0.1.6 and the next CLI release.
Until those versions are published, generate with `--runtime-path /path/to/xecute`
pointing to this repository's runtime crate and run `cargo test` to create the
local dependency lockfile. Subsequent checks can use `--locked`.

The common configuration boundary separates `ConfigFile` for ordinary settings
from `Env` for secrets. Both report value-redacted validation errors. TOML parsing
preserves native types: `false` is a boolean, while `"false"` is an invalid type
for a boolean setting. `get_or` defaults only absent keys; supplied bad values
remain errors. `range` enforces numeric bounds; `required` rejects missing keys.
Generated configuration rejects unknown keys so typos cannot silently disable policies.

```rust
use xecute::config::{ConfigError, ConfigFile, Env, SecretString};

struct DatabaseConfig {
    url: SecretString,
    pool_size: u64,
}
impl DatabaseConfig {
    fn load(file: &ConfigFile) -> Result<Self, ConfigError> {
        Ok(Self {
            url: Env::process().secret("DATABASE_URL")?,
            pool_size: file.range("database.pool_size", 10, 1, 100)?,
        })
    }
}
```

Add `database.pool_size` to the generated allowed-key schema when introducing
this custom setting. Initialize custom configuration before binding the listener.
`SecretString` redacts Debug output and requires `.expose()` for SDK/client access;
it does not promise memory zeroization. `.env` and TOML parse errors omit source
contents. Tests can use `ConfigFile::parse(source)` and map-backed `Env::new`
without changing process variables. Configuration is a startup snapshot.

## Exception handling and domain mappings

Handlers and services can return typed domain errors. Map expected failures
explicitly using `From<DomainError> for HttpError`; controllers returning
`Result<T, HttpError>` can then use `?`.

```rust
use xecute::{HttpError, HttpStatus};

struct EmailTaken;
impl From<EmailTaken> for HttpError {
    fn from(_: EmailTaken) -> Self {
        HttpError::public(
            HttpStatus::CONFLICT,
            "email_taken",
            "Email is already registered",
        )
    }
}
```

The response contains `error.code`, `error.message`, and a server-generated
`error.request_id` matching `X-Request-Id`. Custom mappings survive generated
response normalization. Public custom messages and codes are static API text;
never include user input or secrets. Codes accept lowercase ASCII letters,
digits, and underscores, up to 64 characters. Invalid codes use default details.
All 5xx errors use safe default messages even if custom text is supplied. Invalid
success statuses passed to `HttpError::new` become 500. Log an underlying failure
explicitly before mapping it; the framework does not serialize error Display text.

`HttpError` produces a JSON envelope on its own, with an empty request ID when
there is no request boundary. Generated applications already normalize errors,
preserve protocol/CORS/retry headers, discard stale representation headers, and
attach IDs. For a plain Axum router, install
`xecute::exceptions::exception_boundary(router)` once after assembling routes.
This helper normalizes error responses; generated servers also have their
existing panic-catching and deadline middleware. A panic in an independent
background task is outside the HTTP boundary.

## Provider lifecycles and HTTP requests

| Scope | Lifetime |
| --- | --- |
| Singleton (default) | One instance per application container, shared across requests |
| Request | One instance per request context, shared across nested dependencies |
| Transient | A new instance on every resolution |

```rust
use std::sync::Arc;
use xecute::{Injectable, controller, routes};

#[derive(Injectable)]
#[scope("request")]
struct RequestContext {
    #[inject(default)]
    state: std::sync::Mutex<Vec<String>>,
}

#[controller("/example-scope")]
#[derive(Injectable)]
#[scope("request")]
struct ExampleController {
    context: Arc<RequestContext>,
}

#[routes]
impl ExampleController {
    #[get("")]
    async fn get(&self) -> &'static str {
        let _ = &self.context;
        "request scope is active"
    }
}
```

Register both types in a module's providers. Generated `AppModule` uses
`Controller::scoped_router(container.clone())`, creating a request context before
controller middleware and extractors. Nested factories resolve against that same
context. Use a named parameter `context: xecute::Inject<RequestContext>` in a
controller method, or `Inject<T>` in a plain Axum handler, to resolve explicitly.
Plain routers need `xecute::with_container(router, Arc<Container>)`.
To resolve providers in app-level middleware, wrap the fully assembled middleware
pipeline with `with_container` as well. Matching nested boundaries reuse the same
request cache; mixing different container registries in one boundary returns a
safe 500 instead of resolving from the wrong application.
`Controller::router(Arc<Controller>)` remains available for explicitly supplied
controller instances; it cannot recreate their captured dependencies per request.

Use `container.request()` for jobs or tests requiring an explicit scope. Its
clones share a request cache; a second call creates an independent cache. Request
cache ownership is released at completion/cancellation, but any Arc retained by
application code keeps its value alive. `RequestContainer<'a>` remains a
compatibility alias for the owned `RequestScope`.

Startup rejects missing required registrations, duplicate providers/modules,
cycles, private module dependencies, and singleton-to-request dependencies even
through transient providers. Optional providers may be absent, but failures
constructing a registered optional provider propagate. Factories must declare
all dependencies; undeclared resolutions fail instead of risking recursive cache
deadlocks. Custom definitions can mark a declared dependency optional with
`.optional_dependency::<T>()`.

Concurrent resolutions initialize each singleton/request provider once. Factory
errors are cached for that scope; recover external connectivity inside a shared
client instead of expecting factory retries. `container.initialize()` eagerly
constructs singletons and is called before generated applications listen.
Factories are synchronous: initialize asynchronous SDK/database clients at
startup and register their handles as values. A transient injected into a
singleton is created when that singleton is built, then retained by it.

## Testing modules and HTTP assertions

Enable the runtime's optional test helpers only for development:

```toml
[dev-dependencies]
xecute = { version = "0.1.7", features = ["testing"] }
```

Generated manifests already include this entry. `TestingModule` compiles isolated
containers, applies overrides before initialization, and validates the final
dependency graph. Unknown or repeated overrides are errors, preventing a silently
unused mock. Values replace the same registered Rust type; for trait-based mocks,
use a concrete adapter containing `Arc<dyn YourTrait>` and replace that adapter.
Production containers remain unchanged.

```rust
use std::sync::Arc;
use xecute::testing::{TestApp, TestingModule};

let mut module = TestingModule::new();
for definition in crate::app_module::AppModule::definitions() {
    module = module.module(definition);
}
// Example: module = module.override_value(DatabaseGateway(Arc::new(FakeDatabase)));
let container = Arc::new(module.compile()?);
let routes = crate::app_module::AppModule::router_with_container(container)?;
let app = TestApp::new(crate::server::router(routes, &crate::configs::Config::default()));

let response = app.post("/example")
    .json(&serde_json::json!({"name": "Example"}))
    .send().await?
    .assert_status(axum::http::StatusCode::CREATED);
let body: serde_json::Value = response.json()?;
```

Run these async helpers inside a Tokio runtime, such as `#[tokio::test]`.
The client runs the actual Router and middleware without a socket. It supports
arbitrary HTTP methods through `.request`, common method helpers, raw bodies,
headers, JSON payloads, and `.peer(SocketAddr)` for limiter tests. Loopback peer
information is supplied by default. Responses expose status, headers, bounded
bytes, typed JSON, and chainable `.assert_status`/`.assert_json` methods. Invalid
requests return test errors rather than panicking. Request and body collection
have a five-second timeout and a 1 MiB response limit; adjust with
`TestApp::timeout` and `response_limit`. This tests HTTP behavior in-process;
network/TLS/deployment behavior still belongs in deployment smoke tests.

The generated `src/tests/application.rs` demonstrates isolated container startup
and safe mapped errors. Resource tests demonstrate JSON validation and storage
through the same test client. The runtime test suite also covers trait adapters,
mock isolation, concurrent initialization, nested request scopes, and cleanup.
