# xecute

`xecute` provides architecture tools and macros for applications built on Axum.
It is not a framework; Axum is the underlying web framework. Use these tools
alongside Axum to organize your application's architecture.

This crate supplies the dependency-injection runtime and macro facade, including
typed providers, application/request scopes, module metadata, and startup validation.

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

These APIs are available with CLI 0.1.13, runtime 0.1.8, and macros 0.1.6.
Update the CLI before generating a new project. Existing applications require
corresponding dependency and source changes; updating the CLI does not rewrite them.

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
explicitly before mapping it; Xecute does not serialize error Display text.

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
xecute = { version = "0.1.8", features = ["testing"] }
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

## Multipart file uploads

A runnable controller example is included at
[`xecute/examples/file_upload.rs`](https://github.com/morshedulmunna/xecute/blob/main/xecute/examples/file_upload.rs).
Run from the repository root:

```sh
cargo run -p xecute --example file_upload --features upload
curl -F 'file=@report.pdf;type=application/pdf' http://127.0.0.1:3000/uploads
curl -F 'file=@photo.png;type=image/png' -F 'file=@report.pdf;type=application/pdf' \
  http://127.0.0.1:3000/uploads
```

The controller returns HTTP 201 with an array of `key`, `original_name`,
`content_type`, `size`, and `sha256`. It accepts PNG/JPEG/PDF content, at most four
files, 1 MiB per file, and 2 MiB total; HTTP framing has an additional 64 KiB
allowance. MIME signatures must match declared types. Files are private under
`target/file-upload-example/objects/`, with temporary files in `staging/`.
`/openapi.json` describes the multipart endpoint; the example has no download route.
Invalid multipart requests return 400, oversized uploads 413, invalid types 415,
and missing/empty/excess files 422, with a consistent error envelope and request ID.

This standalone example binds loopback and is separate from generated projects.
Before deploying it, add authentication and application rate/time limits, use a
persistent private storage directory, and define cleanup/retention. Content
signature validation is not malware scanning. Batch rollback is best effort;
crashes or cancellation can leave objects requiring reconciliation.


New projects do not include a built-in uploads module or upload configuration.
To implement uploads in your own feature module, enable the runtime `upload`
feature (and `upload-s3` for S3). The runtime provides `UploadPolicy`,
`UploadService`, `LocalStorage`, `UploadStorage`, and optional `S3Storage`.
Use `#[multipart]` on the upload handler for multipart OpenAPI metadata.

```toml
[dependencies]
xecute = { version = "0.1.8", features = ["upload"] }
```

Apply HTTP body limits and authentication to your own upload route, keep storage
outside `public/`, and configure its policy and backend explicitly.

## API version groups and separate OpenAPI documents

Generated applications register only v1 in
`src/server/versions.rs`:

| URL | Contract |
| --- | --- |
| `/example` | Existing unversioned resource alias |
| `/v1/example` | Version-one resources, using the same v1 service/container |
| `/docs/v1/` | Version-one Swagger UI |
| `/docs/v1/openapi.json` | Version-one contract, with `/v1/...` paths |

`/docs/` and `/docs/openapi.json` continue to describe the unversioned aliases.
No v2 module, routes, or documentation are generated. Additional version groups
can be registered explicitly using the runtime API when needed.

```rust
use xecute::{ApiVersion, ApiVersions};

let versions = ApiVersions::new()
    .with(ApiVersion::new("v1", v1_router, V1Controller::api_operations())?)?;
let router = versions.mount(axum::Router::new());
let documents = versions.documents("My API")?;
```

Supply controller paths relative to the group; the router and documented paths
receive the same prefix. Canonical labels are `v1`, `v2`, etc, with positive u32
version numbers. Invalid labels, duplicate groups, and conflicting contracts fail
before serving. The same operation ID/schema name may appear in different
version documents. Global application policies wrap all groups.

```toml
[api]
versioning_enabled = true

[docs]
enabled = true
```

Set `api.versioning_enabled = false` to keep only unversioned aliases. Set
`docs.enabled = false` to hide all Swagger/OpenAPI routes while keeping API routes
active. Startup logs include each enabled version's Swagger and OpenAPI URLs.
The registration pattern is explicit URL versioning; header-based versioning and
automatic deprecation/sunset behavior are not implied.

### Module imports and controller request inputs

Run `cargo run -p xecute --example module_imports` for a complete example.
[`modules.rs`](examples/module_imports/modules.rs) exports `GreetingService` from
`GreetingModule`, then imports that module into `ExampleModule` so its controller
can inject the service. Both definitions are registered in `ModuleGraph`.

[`controller.rs`](examples/module_imports/controller.rs) includes curl commands
and handlers for validated JSON (`#[body]`), path parameters (`#[param]`), validated
query strings (`#[query]`), headers (`HeaderMap`), URL-encoded forms (`Form<T>`),
raw text (`String`), and binary data (`Bytes`). Its comments also explain middleware
extensions, connection info, and the separate file upload example. Use one
body-consuming extractor per handler, placed last.

### Async providers and application lifecycle

Register asynchronous resources with `ProviderDefinition::singleton_async`, then
await `container.initialize_async()` before binding or serving HTTP. Factories
receive an owned `Container`, so they can await connection setup and resolve
**declared** dependencies after await points. Dependencies and their startup hooks
finish before dependent factories run. Existing synchronous factories and derived
providers can depend on these initialized resources without changing their APIs.

```rust
use xecute::{Container, InjectError, ProviderDefinition};

struct Connection;
impl Connection {
    async fn connect() -> Result<Self, InjectError> { Ok(Self) }
    async fn close(&self) -> Result<(), InjectError> { Ok(()) }
}

# async fn example() -> Result<(), InjectError> {
let container = Container::builder()
    .provider(
        ProviderDefinition::singleton_async::<Connection, _, _>(vec![], |_| async {
            Connection::connect().await
        })
        .on_init::<Connection, _, _>(|connection| async move {
            // Optional readiness checks or resource warm-up.
            let _ = connection;
            Ok(())
        })
        .on_shutdown::<Connection, _, _>(|connection| async move {
            connection.close().await
        }),
    )
    .build()?;

container.initialize_async().await?;
let connection = container.resolve::<Connection>()?;
// Serve requests, then stop accepting work and drain outstanding requests.
container.shutdown_async().await?;
# let _ = connection;
# Ok(())
# }
```

Use `.on_init::<YourType, _, _>(...)` and `.on_shutdown::<YourType, _, _>(...)`
on a synchronous provider definition as well, including `YourType::definition()`.
The hook type must match the registered provider type. Hooks are singleton-only;
request and transient factories keep their existing resolution behavior. Sync
`initialize()` reports `AsyncInitializationRequired` when async factories or
hooks need startup. It remains usable after async startup completes.

Clones share lifecycle state. Concurrent initialization runs factories and hooks
once; shutdown runs cleanup in reverse dependency order. Startup failures clean
up already constructed resources, including a resource whose init hook failed,
then return and cache the original startup error. Cleanup continues if a hook
returns an error; explicit shutdown returns the first cleanup error. A factory
that fails before returning owns cleanup of its partially constructed resources.

Shutdown is explicit: dropping a container does not execute async hooks. After
startup cancellation, call `shutdown_async()` to clean constructed resources;
startup cannot be retried on that container. Cancelled shutdown can be resumed,
but the interrupted hook is not retried. Keep cleanup hooks bounded and avoid
cancelling them when possible. Hooks and factories must not call lifecycle
methods recursively on the same container. Existing `Arc` references remain
accessible after cleanup; finish application work before shutting down.

For module-based applications, attach definitions with
`ModuleDefinition::provider(...)`, then build with `ModuleGraph` as usual.
For generated applications using async resources, build the container from
`AppModule::definitions()` and await initialization before calling
`AppModule::router_with_container(container.clone())`; the synchronous
`AppModule::container()` helper cannot initialize async resources. Retain a
container clone for cleanup after graceful request draining, including startup
errors after initialization (for example, a failed listener bind).

With the `testing` feature, use `TestingModule::compile_async().await` and call
`shutdown_async()` when the test finishes. Provider overrides replace factories
and hooks together. Existing lifecycle attribute markers such as
`#[on_module_init]` remain placeholders; use the executable registration methods
above.

Run `cargo run -p xecute --example async_lifecycle` for a complete example with
async connection setup, a synchronous dependent service, and explicit cleanup.


### PostgreSQL pools, migrations, and transactions

Enable the optional `postgres` feature to use `database::PostgresConfig` and
`database::PostgresDatabase`. Register a pool with `provider_with_migrations`,
await `initialize_async()` before serving, and await `shutdown_async()` after
request draining. Repositories use the re-exported `database::sqlx` APIs and
native transactions. Database support is opt-in; resource generators continue
to produce their in-memory examples.

See the [PostgreSQL guide](docs/postgres.md) for local setup, pool/TLS settings,
module registration, generated-app integration, migration deployment,
repository/transaction examples, error handling, and live-test commands.
Run `cargo run -p xecute --example postgres --features postgres` with
`DATABASE_URL` set to a development PostgreSQL instance for the complete example.


### Guards, JWT authentication, and permissions

Executable `#[guard(...)]` annotations protect controllers and routed handlers;
`Application::use_guard` protects an application router. Custom guards work
without extra features. Enable `auth` for pinned HS256/RS256 JWT verification,
`AuthenticatedUser`, `#[roles(...)]`, and `#[permissions(...)]`. Authentication
installs a verified principal before DTO extraction and authorization checks.

See the [authentication and guard guide](docs/auth.md) for configuration, DI,
public routes, execution order, safe errors, limitations, and test commands.
Run `cargo run -p xecute --example auth --features auth` with a random `JWT_SECRET`
of at least 32 bytes to try the local example.


### Background jobs and TTL caching

Enable `jobs` for bounded in-process queues with retries, per-attempt timeouts,
receipts, interval schedules and lifecycle draining. Enable `cache` for bounded
memory TTL/LRU storage, or `cache-redis` for shared namespaced Redis entries.
Jobs are process-local and do not survive a restart; Redis is a cache backend,
not a durable job broker in this implementation.

See the [jobs guide](docs/jobs.md) and [cache guide](docs/cache.md) for limits,
DI/resource ordering, retry/idempotency semantics, Redis configuration,
authorization-aware cache keys, invalidation and test commands.
Run `cargo run -p xecute --example jobs_cache --features jobs,cache` for a complete
worker → cache example with a retry and explicit lifecycle cleanup.


### CQRS command/query buses, event handlers, and pipelines

Enable `cqrs` for typed `CommandBus` / `QueryBus` dispatch and ordered `EventBus`
fan-out. Builders reject duplicate registrations, bind DI handlers with ordinary
scope/module validation, and support shared `Behavior` pipelines and explicit
`DispatchContext` metadata. Domain errors remain typed; event publication reports
subscriber failures instead of silently dropping them. Buses are in-process and
caller-owned, with no durable broker or automatic event sourcing.

See the [CQRS guide](docs/cqrs.md) for API examples, error/transaction/cancellation
semantics, request scopes, workflow limitations and migration from generated direct
handlers. Run `cargo run -p xecute --example cqrs --features cqrs` for a complete
HTTP example with separate DTOs, commands, queries, repository and event subscribers.
