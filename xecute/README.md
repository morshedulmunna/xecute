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

These APIs are available with CLI 0.1.12, runtime 0.1.7, and macros 0.1.6.
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

## Multipart file uploads

Generated projects include `src/modules/uploads/` with a controller and async
startup module. Enable private writes explicitly in `AppConfig.toml`:

```toml
[uploads]
enabled = true
storage = "local"
directory = "storage/uploads"
max_files = 4
max_file_bytes = 524288
max_total_bytes = 524288
allowed_types = ["image/png", "image/jpeg", "application/pdf"]
```

The endpoint accepts repeated `file` parts:

```sh
curl -F 'file=@report.pdf;type=application/pdf' http://127.0.0.1:3000/uploads
curl -F 'file=@photo.png;type=image/png' -F 'file=@report.pdf;type=application/pdf' \
  http://127.0.0.1:3000/v1/uploads
```

A successful request returns 201 with an array of metadata containing `key`,
`original_name`, `content_type`, `size`, and `sha256`. Generated UUID keys never
use client filenames. Local objects and staging files stay outside `public/`;
there is no public download route. Persist returned metadata in your own database
and implement authorized retrieval/deletion according to your application rules.
Attach authentication/permissions using the existing controller/route middleware
before enabling upload writes in a deployed application.

The runtime streams each part to a temporary file and keeps only an 8 KiB sniff
sample plus hashing state in memory. It checks actual per-file and aggregate
bytes, file count, nonempty content, safe display filenames, and an exact MIME
allowlist. Declared MIME must match an `infer`-recognized file signature; unknown
formats and mismatches are rejected. Signature recognition is not a full decoder,
malware scan, or content moderation system. Add those checks before publishing
files when your application needs them. Text form fields are not part of this
example contract; only repeated `file` fields are accepted.

| Failure | Status |
| --- | --- |
| Malformed multipart, unexpected field, unsafe filename | 400 |
| File content, aggregate bytes, or HTTP body limit exceeded | 413 |
| Unrecognized, mismatched, or disallowed MIME type | 415 |
| Missing/empty file or too many files | 422 |
| Storage provider failure | 500 with safe public details |

`server.body_limit_bytes` also bounds multipart framing and headers. When uploads
are enabled, startup requires it to cover `uploads.max_total_bytes + 65536` bytes.
To accept larger files, increase both limits explicitly. HTTP deadlines and global
rate/concurrency policies still apply. The low-level `UploadService` expects its
caller to apply an HTTP body limit as well; payload limits alone do not bound
multipart headers or framing.

All files validate before any storage commit. Invalid batches remove staging
files without writing final objects. Local storage publishes complete files
atomically without replacing an existing key. Handled storage failures trigger
best-effort deletion of previously committed batch objects. A process crash,
cancelled request, or uncertain S3 response can leave staging/committed objects;
use application reconciliation, disk quotas, and retention/cleanup appropriate to
your deployment. Stored objects do not expire automatically. Keep upload storage
and staging directories private and unavailable for untrusted filesystem writes.

### S3 storage

S3 is an optional Cargo feature; local uploads do not compile the AWS SDK.
Ordinary backend settings stay in `AppConfig.toml`:

```toml
[uploads]
enabled = true
storage = "s3"
directory = "storage/uploads" # private streaming staging directory
max_files = 4
max_file_bytes = 524288
max_total_bytes = 524288
allowed_types = ["image/png", "image/jpeg", "application/pdf"]

[uploads.s3]
bucket = "my-private-bucket"
region = "us-east-1"
endpoint = "" # AWS default; set an HTTPS URL for an S3-compatible service
prefix = "uploads"
force_path_style = false
allow_http = false
```

Use workload credentials or the standard AWS credential provider chain. For local
static credentials, put only `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and an
optional `AWS_SESSION_TOKEN` in `.env`; never put them in TOML. Compile/run with:

```sh
xecute run dev --features s3
cargo run --features s3
# Container image with the optional SDK:
docker build --build-arg CARGO_FEATURES=s3 -t my-app .
```

For local MinIO, configure `endpoint`, `force_path_style = true`, and explicitly
set `allow_http = true` only if the development endpoint uses HTTP. Embedded
credentials, query strings, and endpoint paths are rejected. The adapter uses the
official AWS SDK, streams retryable file bodies, supplies MIME/checksum metadata,
and uses conditional `If-None-Match: *` writes. It never creates buckets, changes
ACLs, or makes objects public. Provision a bucket and scoped PutObject/DeleteObject
permissions, plus KMS permissions if required by its encryption policy. Bucket
versioning, lifecycle, encryption, and application metadata retention remain
operator/application choices.

Runtime users enable `xecute/upload` for `UploadPolicy`, `UploadService`,
`LocalStorage`, and the `UploadStorage` adapter trait; `upload-s3` adds `S3Storage`.
`S3Storage::new` accepts an application-configured AWS client; `from_environment`
uses the standard credential chain. `#[multipart]` inside a `#[routes]` method
produces the matching multipart OpenAPI schema and cannot be combined with JSON
body metadata. The example Swagger page can submit repeated file fields.

Tests cover invalid batches, count/size limits, request framing limits, generated
keys, private storage, temporary-file cleanup, and handled-failure rollback. A
local mock S3 endpoint verifies signed conditional PUT, streamed content, metadata,
and DELETE; no live cloud bucket or real credentials are used by the tests.

## API version groups and separate OpenAPI documents

Generated applications explicitly register version groups in
`src/server/versions.rs`:

| URL | Contract |
| --- | --- |
| `/example` | Existing unversioned resource alias |
| `/v1/example` | Version-one resources, using the same v1 service/container |
| `/v1/uploads` | Version-one upload endpoint when uploads are enabled |
| `/v2/status` | Independent v2 example from `src/modules/api_v2/` |
| `/docs/v1/` | Version-one Swagger UI |
| `/docs/v1/openapi.json` | Version-one contract, with `/v1/...` paths |
| `/docs/v2/` | Version-two Swagger UI |
| `/docs/v2/openapi.json` | Version-two contract, with `/v2/...` paths |

`/docs/` and `/docs/openapi.json` continue to describe the unversioned aliases.
Each version has its own routes, operation IDs, and schema components; v2 does
not automatically inherit v1 resources. Add v2-specific modules/controllers to
its router as the contract evolves. Use a separate DI container when a version
needs independently constructed provider instances.

```rust
use xecute::{ApiVersion, ApiVersions};

let versions = ApiVersions::new()
    .with(ApiVersion::new("v1", v1_router, V1Controller::api_operations())?)?
    .with(ApiVersion::new("v2", v2_router, V2Controller::api_operations())?)?;
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
