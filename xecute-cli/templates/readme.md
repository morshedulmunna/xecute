# __PROJECT_NAME__

```sh
# Optional credentials: cp .env.example .env
xecute run dev
# Single run without watching: cargo run
# Swagger UI: http://127.0.0.1:3000/docs/
# Example collection: GET http://127.0.0.1:3000/example
cargo test --locked
```

## Server defaults

The generated server validates TOML configuration at startup and provides:

- JSON logs through `tracing`, configurable with `logging.filter`. Each request has
  a UUID in its `X-Request-Id` response header. One flat JSON completion log
  records request ID, method, route, status, precise duration in milliseconds,
  and response body size when known. Target and span metadata are omitted.
  Client routing errors are INFO, quota rejections WARN, and server failures ERROR.
- Per-socket-IP rate limits: 60 requests per 60-second fixed window, bounded
  to 10,000 tracked IPs per server process. Rejections return 429 and Retry-After.
- At most 256 concurrent handlers; excess requests receive 503 and Retry-After.
- A 30-second handler deadline returning 504, and a 1 MiB request body limit
  returning 413. Axum extractors enforce streamed body limits during consumption.
- Generic JSON error envelopes with request IDs, including validation, routing,
  overload, timeout, and panic errors. Framework errors are normalized without
  exposing internal details. Customize `server/error.rs` for application error messages.
- Explicit CORS origin allowlists; cross-origin browser access is disabled by
  default. Credentialed cross-origin access is opt-in through validated configuration.
- `nosniff`, frame denial, no-referrer, and restrictive API CSP headers.
- `/health/live` and `/health/ready`, exempt from quotas so probes work under
  load. Readiness currently indicates successful startup; add dependency checks
  when introducing databases or external services.
- Ctrl-C/SIGTERM shutdown with a configurable 10-second request drain deadline.

Logs contain HTTP methods, matched route templates (or the requested path for
unmatched routes), response status, and duration. They omit payloads, authorization
headers, cookies, query strings, and full URLs.
Response latency measures time until response headers; streaming-body completion
and streaming failures are not tracked. Handler concurrency limits and deadlines
also end at response headers. Timeout cancels the handler future; detached tasks
and side effects already performed require application-specific cleanup.

## Configuration

Ordinary settings live in the generated `config/AppConfig.toml`. Commit this file;
keep credentials in `.env` or inject them through your deployment's secret store.
`.env` is optional and ignored by Git/Docker. Process variables take precedence
over `.env` for secrets. Ordinary settings have no environment fallback or override.

```toml
[server]
bind_address = "127.0.0.1:3000"
request_timeout_seconds = 30
shutdown_grace_seconds = 10
body_limit_bytes = 1048576
max_in_flight_requests = 256

[rate_limit]
requests = 60
window_seconds = 60
max_clients = 10000

[cors]
origins = ["https://example.com", "http://localhost:5173"]
allowed_methods = ["GET", "POST"]
allowed_headers = ["content-type", "authorization"]
exposed_headers = ["x-request-id", "retry-after"]
allow_credentials = false
max_age_seconds = 600

[docs]
enabled = true

[logging]
filter = "info"
stdout_enabled = true
file_dir = "logs"
```

The generated CORS origin list is empty by default. Origins must be exact HTTP(S)
origins without paths, credentials, wildcards, or trailing slashes. Credentials
require a nonempty origin list. Arrays are native TOML arrays, without JSON or
dotenv quoting. Invalid types, unknown keys, invalid protocol values, and numeric
bounds fail startup before logging/listening. Missing individual values use typed
defaults, but the file itself must exist. Request limits must be positive; the
CORS cache duration may be zero.

`Config::from_file()` reads `config/AppConfig.toml` relative to the working directory.
`Config::from_path(path)` supports an explicit file chosen in bootstrap code.
The file is parsed once into a validated snapshot shared with logger startup;
there is no automatic production reload. Development hot reload watches TOML
files and restarts the process when settings change.

The Dockerfile includes this file and adapts only the generated loopback bind to
`0.0.0.0:3000`. Custom bind addresses remain unchanged. To supply deployment
settings, mount a replacement `/app/config/AppConfig.toml` read-only. The default private
`/app/logs` directory is owned by UID 65532; custom log mounts must also be writable
by this user. A read-only root filesystem requires writable log storage or an
empty `logging.file_dir`.

For custom settings, use `xecute::config::ConfigFile` with dotted paths and extend
the allowed-key schema in `src/configs/server.rs`. Use `Env::process().secret(key)`
for required credentials and tokens. The template `.env.example` contains only
commented credential examples, with no active configuration or fake secrets.

## Structure

- `main.rs`: minimal application entrypoint.
- `configs/`: validated runtime configuration, re-exported through `configs/mod.rs`.
- `server/logging.rs`: structured logger.
- `server/bootstrap.rs`: configuration, listener, and bounded request draining.
- `server/shutdown.rs`: Ctrl-C and SIGTERM handling.
- `server/router.rs`: routes and middleware ordering.
- `server/health.rs`: health probes.
- `server/middleware/request_log.rs`: request IDs, tracing spans, completion logs.
- `server/middleware/guard.rs`: rate limits, concurrency limits, and deadlines.
- `server/middleware/response.rs`: error normalization and security headers.
- `server/rate_limit.rs`: bounded per-IP quotas.
- `server/error.rs`: safe response envelopes.
- `tests/`: configuration, rate-limit, middleware, and log tests plus shared helpers.
- `app_module.rs`: generated module and route registrations.

## Reproducible builds and deployment

The project pins Rust 1.98.1 in `rust-toolchain.toml`, includes rustfmt, Clippy,
EditorConfig, and a dependency lockfile. Commit `Cargo.lock`; use `--locked` in
CI and releases. After changing dependencies, run `cargo generate-lockfile` and
review the updated lockfile. Projects using `--runtime-path` must regenerate
their lockfile locally before running locked builds.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
docker build --tag __PROJECT_NAME__:local .
docker run --rm --name __PROJECT_NAME__ --read-only --cap-drop=ALL \
  --security-opt=no-new-privileges --pids-limit=128 --memory=512m --cpus=1 \
  --stop-timeout=15 -p 127.0.0.1:3000:3000 __PROJECT_NAME__:local
```

CI runs these Rust checks and builds the container. The multi-stage Dockerfile
uses Rust 1.98.1 to build a stripped, statically linked musl binary for amd64 or
arm64. The final image uses `gcr.io/distroless/static-debian13:nonroot` and
contains the application binary plus the minimal distroless runtime. Compiler
tools and build-stage packages are excluded from the final image.

Distroless has no shell, package manager, or curl. Configure HTTP liveness and
readiness probes in your orchestrator using `/health/live` and `/health/ready`.
Use a stop timeout longer than `SHUTDOWN_GRACE_SECONDS`; the example uses 15
seconds. `.dockerignore` excludes secrets. Actual image size depends on your
compiled dependencies; native dependencies must support static musl linking.
Local path runtime dependencies need a custom Docker build context.
Dependabot opens weekly Cargo, GitHub Actions, and Docker dependency updates.

CI uses `deny.toml` for dependency source, license, and advisory checks.
To run locally, install `cargo-deny` and run `cargo deny --locked check`.
The application is private (`publish = false`); dependency licenses are checked.
Review your dependency policies
and update pinned toolchain and base image versions regularly. The server
retains Rust's unwind panic strategy so the HTTP panic boundary can return 500.

Keep `// xecute:` registration comments in `app_module.rs` when editing it.
Resources generate collection GET skeletons with injected services, not database
CRUD implementations. Generated projects use the crates.io runtime by default;
use `--runtime-path` with `xecute new` for local runtime development.

Generated feature folders use `controller.rs`, `service.rs`, and `module.rs`.
Types keep feature names such as `AuthController`, `AuthService`, and `AuthModule`.
Macros are imported once and used as `#[controller]`, `#[injectable]`, and
`#[module]`; method attributes such as `#[get]` are consumed by `#[controller]`.

Resource generation includes `dto/`, `entity/`, `repository/`, and an HTTP test inside each feature’s `tests/` directory. CQRS projects add `commands/`, `queries/`, and `services/`; MVC projects add `service.rs`. DTOs use `validator` to reject invalid JSON payloads with HTTP 422 before saving. The included repository is bounded in-memory example storage; replace it with a database for durable production data.

```sh
curl -X POST http://localhost:3000/user -H 'content-type: application/json' -d '{"name":"Example"}'
```

Document public items with `///` and modules with `//!`. Missing documentation produces compiler warnings. Run `cargo fmt --all` before committing; CI checks formatting and treats Clippy warnings as errors.

## Controllers and API documentation

Every new project includes `ExampleModule`, with GET and POST `/example` routes, an injected service and repository, DTO/entity folders, and an HTTP test.

Swagger UI is available at `/docs/`; OpenAPI JSON is served at `/docs/openapi.json`. The assets are embedded in the application. Set `docs.enabled = false` in `config/AppConfig.toml` to disable both routes. Resource generation registers new controllers and their documentation automatically.

Controllers use `#[controller("/example")]` on the struct, `#[derive(Injectable)]`, and `#[routes]` on the impl. Handlers use `&self` with `#[body]` or `#[query]` DTO parameters. Extraction runs `validator` validation before entering the handler; invalid payloads return 422. DTOs derive `JsonSchema` to describe payloads and response entities in Swagger. `#[api]`, `#[body(json = ...)]`, and `#[resp]` describe each operation.

```sh
curl -X POST http://127.0.0.1:3000/example -H 'content-type: application/json' -d '{"name":"Example"}'
xecute g resource user
```

Repositories use `#[derive(Injectable)]` and are registered in the module’s
`providers` list. `#[inject(default)]` initializes provider-owned storage;
other dependency fields are auto-resolved. The default singleton scope lets
services share the same repository state.

## Architecture

This project uses **__ARCHITECTURE__** architecture. `.xecute/architecture` records
this choice; commit it so subsequent `xecute g resource <name>` commands use the
same layout. New projects default to MVC with the
controller → service → repository layout. Use `xecute new my-api --cqrs` for
command/query handlers; `--mvc` explicitly selects MVC. Projects without the architecture file
retain the legacy MVC layout. Invalid configuration stops generation.

CQRS resources contain `commands/create.rs` and `queries/list.rs`, each with its
input type and injectable handler. Controllers validate DTOs automatically, map
them to commands, and invoke handlers through dependency injection. Both handlers
inject the shared business service in `services/<feature>_service.rs`, which holds
business operations and injects the singleton repository. The module registers
and exports the service alongside the handlers. Writes are visible to subsequent
reads. The flow is controller → command/query handler → service → repository.
Separate databases, event sourcing,
and command/query buses are optional extensions. The initial endpoints are GET
(collection) and POST (create); add further operations as the feature grows.

Standalone module, controller, and service generators remain available in both
architectures; standalone services are opt-in building blocks.

## Middleware

Register global middleware in `src/middleware.rs`:

```rust,ignore
pub fn configure(app: xecute::Application) -> xecute::Application {
    app.use_middleware(auth).use_middleware(audit)
}
```

Middleware functions accept `(Request, Next)` from `xecute::middleware` and return
an HTTP response. `next.run(request).await` continues; returning early stops the
pipeline. Closures can capture shared application dependencies through `Arc`.
Global middleware covers routes, health probes, documentation, and unmatched
requests; request logging and response normalization remain outside this pipeline.
Handle public routes explicitly in authentication policies.

For controller middleware, add `#[middleware(auth, audit)]` immediately below
`#[controller("/example")]` on its struct. For route middleware, add
`#[middleware(check_permission)]` to a handler inside `#[routes]`.
Middleware also works on an entire impl immediately below `#[routes]`.

Request order is app → controller → route → validation → handler. Within each
scope, middleware runs in declaration order; responses unwind in reverse.
Controller middleware only covers matching routes, and route middleware only
covers its own HTTP method. Keep `#[controller]` / `#[routes]` above their
middleware annotations so the owning macros can process them.

Existing projects can wrap their assembled router with
`xecute::Application::new(router).use_middleware(auth).into_router()`.

## Development hot reload

Run `xecute run dev` from this project root to automatically rebuild and restart
the debug server on source/configuration changes. A failed compilation keeps the
watcher alive; the next edit retries it. Compiler output and server logs remain
visible. Ctrl-C stops the watcher and its managed application processes.

```sh
xecute run dev
xecute run dev --bin your-binary
xecute run dev --watch ../shared-library
```

Use CLI 0.1.7 or later. No separate watcher is required. The watcher polls every
200 ms and debounces for 300 ms. It monitors Rust/configuration/web source files,
`Cargo.lock`, `.env`, and `.env.*`; target output, VCS, dependency, and build
folders are excluded. External path dependencies need explicit `--watch` paths.

Reload restarts the process, so in-memory repository data resets. Browser refresh
is not automatic. Unix allows five seconds for managed processes to stop before
forcing termination; Windows terminates the process tree. Use `cargo run` for a
single run and a release build for production. Hot reload is development-only.

## CLI management

With CLI 0.1.8 or later, use `xecute update` to
install the latest registry CLI and `xecute uninstall` to remove the installed
CLI. These target the running executable's Cargo installation root and preserve
this application's files. Updating the CLI does not update this project's runtime
or regenerate its source. Development/source binaries must be installed through
Cargo before using self-management commands.

To create another project directly inside a fresh existing directory, run
`xecute new .`; use `--name my-app` to override its package name or `--cqrs` to
select CQRS. Existing scaffold files are rejected before any files are written.

## Feature module folders

All newly generated resources and standalone modules, controllers, and services
live under `src/modules/<feature>/`. Each resource keeps its controller, DI module,
DTOs, entities, repository, and HTTP tests together. CQRS resources also contain
commands, queries, and shared services. `src/modules/mod.rs` declares each feature;
`AppModule` imports and mounts it through `crate::modules::<feature>`.

```text
src/
├── app_module.rs
├── modules/
│   ├── mod.rs
│   └── user/
│       ├── mod.rs
│       ├── module.rs
│       ├── controller.rs
│       ├── commands/
│       ├── queries/
│       ├── services/
│       │   ├── mod.rs
│       │   └── user_service.rs
│       ├── dto/
│       ├── entity/
│       ├── repository/
│       └── tests/
│           ├── mod.rs
│           └── http.rs
└── tests/                 # Shared server/infrastructure tests
```

MVC resources use `service.rs` inside their feature folder instead of CQRS
commands/queries/services directories. This layout is available starting with
CLI 0.1.9; existing root-level features are preserved rather than moved.

## Welcome route and startup URLs

`GET /` returns HTTP 200 with the application name, `status: "ok"`, a welcome
message, and `documentation: "/docs/"`. With `docs.enabled = false` in `config/AppConfig.toml`, the
`documentation` field is null. Other unregistered paths still return the normal
404 envelope. The root route is defined in `src/server/router.rs` and can be
customized there.

Startup logs include the server URL, Swagger UI URL (`/docs/`), and OpenAPI URL
(`/docs/openapi.json`). The actual bound port is used, including when binding port
zero. Wildcard bindings are displayed as local loopback URLs; IPv6 addresses are
bracketed. These are local HTTP URLs; configure the public URL separately when
running behind a TLS ingress or reverse proxy. Disabled documentation is logged
without advertising Swagger links.

Available starting with CLI 0.1.10. Existing projects need the corresponding
server changes; updating the CLI does not rewrite them. The runtime crate is unchanged.

## Static files

Newly generated applications include `public/index.html`, served at `/static` and
`/static/`, plus `public/hello.txt`, served at
`http://127.0.0.1:3000/static/hello.txt`. Static serving is enabled by default,
without environment settings. Add files to `public/` and access them at `/static/<filename>`.
For example, `public/images/logo.png` is served at `/static/images/logo.png`.
To customize the directory, prefix, or enabled state, edit `StaticConfig` in
`src/configs/static_files.rs`. Relative directories use the application's working
directory; the public directory must exist and cannot be the application root or a symlink.

Files support GET, HEAD, MIME types, conditional requests, and byte ranges through
Tower HTTP `ServeDir`. Directory listings and nested directory indexes are disabled. The mount root
serves `public/index.html`; other assets use explicit filenames. Hidden path components, parent traversal, and
symlinked assets are rejected. Keep public directories limited to intended public
assets; static files share the application's CORS, request IDs, security headers,
logging, rate limits, and global middleware. HTML retains the restrictive API CSP;
this is asset serving, not a generated SPA hosting configuration.

The Docker image copies `public/` into the runtime image. For another static directory,
adapt the Dockerfile or mount the asset directory read-only. Change files in the
public directory directly or mount updated assets; there is no binary embedding.

## File logging and external logging backends

Logging outputs use these settings in `config/AppConfig.toml`:

```toml
[logging]
stdout_enabled = true
file_dir = "logs"
```

`logging.stdout_enabled` controls JSON console output. A nonempty `logging.file_dir`
enables JSON file output and creates the private directory. Remove it or leave it
empty to disable file output. Generated projects include both settings above.
`logging.filter` continues to filter events across all outputs.

Files use the `app` prefix, daily rotation, and retention of seven matching files.
These defaults live in `src/configs/logging.rs` and can be changed in code.
Setting stdout false and omitting the file directory allows an exporter-only
configuration. An unwritable enabled file destination fails startup.

The file sink uses a dedicated writer thread and an 8192-entry bounded queue.
A full queue applies backpressure rather than silently dropping events. The
`LoggerGuard` returned from initialization is retained in `server/bootstrap.rs`
until normal server shutdown so pending writes are flushed. Forced termination
cannot guarantee pending log delivery. Built-in request events continue to omit
payloads, cookies, authorization headers, and query strings.

The default `/logs/` path is ignored by Git and Docker. Configured log storage
must remain outside the public static directory, even if file output is currently
disabled. For a read-only container with file output enabled, mount private log
storage writable by the application's UID 65532. Console logging can instead be
collected by the deployment's log collector.

### Sentry, Grafana/Loki, OpenTelemetry, and custom exporters

Application modules emit standard `tracing` events, so business code does not need
to change when a new destination is added:

```rust
tracing::info!(operation = "create_user", "User created");
tracing::warn!(operation = "create_user", "Capacity reached");
```

`src/server/logging.rs` exposes `init_with_layer(&config, exporter_layer)` for a
`tracing_subscriber::Layer<Registry>`. Replace the default bootstrap initialization
with this call after configuring your chosen SDK/collector. Standard console
and file sinks remain independently configurable and continue to receive the
same events. A test exporter verifies that file logging and an extra layer can
receive an event together.

For a future Sentry or OpenTelemetry integration, add its SDK dependencies and
initialize its layer/provider before installing the subscriber. Keep SDK guards
alive for the server lifetime and shut exporters down according to that SDK's
requirements. Grafana/Loki deployments can also ingest the existing JSON stdout
or files through a log collector. No cloud SDK, credentials, or outbound exporter
is configured by default. Per-backend transport/retries/authentication belong in
the selected SDK or collector, not in controllers or services.

Static serving, shared environment access, and configurable file/exporter logging
are available starting with CLI 0.1.11. Existing applications need the corresponding source
and dependency updates; updating the CLI does not rewrite an existing app.

### Shared configuration readers

`src/configs/environment.rs` re-exports the shared runtime readers.
`configs::load_env()` loads optional credentials once; `Env::process()` and
`configs::read_env(key)` read secrets with process-over-dotenv precedence.
Ordinary settings use `ConfigFile::load("config/AppConfig.toml")` and typed dotted paths.
Generated server and logger use the same validated configuration snapshot.

```rust
let token = xecute::config::Env::process().secret("API_TOKEN")?;
let file = xecute::config::ConfigFile::load("config/AppConfig.toml")?;
let timeout = file.range("server.request_timeout_seconds", 30, 1, 3600)?;
```

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
