# Xecute CLI

Install from the repository root:

```sh
cargo install --path xecute-cli
xecute new my-api
cd my-api
xecute g resource user
cargo run
```

Install the published release with `cargo install xecute-cli --locked`.

New projects include a pinned Rust toolchain, formatted files, lockfile,
Clippy/EditorConfig configuration, environment examples, CI checks, a non-root
multi-stage Dockerfile with a static musl binary and a non-root distroless runtime.
The server includes one JSON completion log per request without target or span
metadata, UUID request IDs, per-IP rate limits, concurrency limits,
request deadlines, body limits, error envelopes, explicit CORS origins, security
headers, health probes, and bounded graceful shutdown. Generated middleware tests
cover the defaults. Application authentication, shared rate limiting, dependency
readiness checks, and TLS ingress still depend on the deployment.

Resources expose GET and POST endpoints. POST validates a `name` payload with
`validator` before invoking the command handler (CQRS) or service (MVC).

Commands:

- `run dev [--bin <name>] [--watch <path>] [-- <app arguments>]`: run the debug
  application with automatic rebuild/restart on source and configuration changes.

- `new <project|.> [--name <package-name>] [--mvc | --cqrs] [--runtime-path <path>]`: create an independent Axum project.
- `generate resource <name>` / `g res <name>`: create and register a module,
  controller, DTO, entity, repository, and HTTP test, with a service
  by default or command/query handlers for CQRS projects.
- `generate module <name>` / `g mo <name>`: create and register an empty module.
- `generate controller <name>` / `g co <name>`: create a standalone injectable
  controller with a GET endpoint; register it in your module manually.
- `generate service <name>` / `g s <name>`: create a standalone injectable
  service; register it in your module manually.

Names such as `user-profile` become `src/modules/user_profile/` and `UserProfileModule`.
No `#[path]` annotations are generated. Existing component folders are never
overwritten. Run generation from the project root. Automatic registration uses
the comments in `app_module.rs`; keep these markers when editing that file.

Generated projects use `xecute = "0.1.6"` from crates.io by default.
Use `xecute new my-api --runtime-path /path/to/xecute/xecute` for local development.

Generated CQRS resource folders contain `module.rs`, `controller.rs`,
`commands/create.rs`, `queries/list.rs`, `services/<feature>_service.rs`, `dto/`,
`entity/`, and `repository/`.
New projects use MVC with `service.rs` by default.
Use `xecute new my-api --cqrs` to generate command/query handlers instead.
`--mvc` also explicitly selects MVC.
The selection is saved in `.xecute/architecture`; commit this file. Subsequent
resource generation follows that selection. Existing projects without the file
keep the legacy MVC layout. Invalid configuration stops generation.
Command and query handlers inject a shared singleton service from `services/`.
The service contains business operations and injects the repository. The feature
module registers/exports the service and its handlers. MVC retains `service.rs`.
The CQRS services directory is available starting with CLI 0.1.9; existing
features are not rewritten automatically.
Separate databases, event sourcing, and message buses are optional. The repository is a bounded in-memory
example; replace it with durable storage before deployment.

Public API documentation uses `///`; modules use `//!`. The compiler warns on
missing public documentation, and CI checks formatting and denies warnings.
Run `cargo fmt --all` to apply consistent spacing. Rust cannot evaluate the
quality of prose comments or require useful comments on every private line.

New projects also include a working `ExampleModule` and Swagger UI at `/docs/`
with OpenAPI JSON at `/docs/openapi.json`. Set `docs.enabled = false` in `AppConfig.toml` to
disable documentation. Assets are embedded, with remote validation disabled.

Generated controllers use `xecute::prelude::*`, `#[controller]` on the struct,
`#[derive(Injectable)]`, `#[routes]`, and `&self` handlers. `#[body]` and `#[query]`
validate DTOs automatically. `#[api]` and `#[resp]` feed the OpenAPI document.

Generated repositories also use `#[derive(Injectable)]`. Provider-owned storage
uses `#[inject(default)]`; dependency fields are resolved automatically. The
feature module registers the repository as a singleton, and services inject
it through `Arc<FeatureRepository>`.

Middleware is declared with `#[middleware(auth, audit)]` below `#[controller]`
on a controller struct, below `#[routes]` on its impl, or on an individual route
inside that impl. Register global middleware in generated `src/middleware.rs`
with `app.use_middleware(auth)`. Execution follows app → controller → route →
validation → handler, with declaration order within each scope. Middleware can
return early or call `next.run(request).await` to continue.

## Development hot reload

Run `xecute run dev` from an application root containing `Cargo.toml`. It starts
`cargo run`, polls source/configuration metadata every 200 ms, and debounces edits
for 300 ms. Build failures keep the watcher alive for the next edit. Ctrl-C stops
the managed Cargo/server processes; no external watcher installation is required.

Use `--bin server` for multiple binaries, repeat `--watch ../path` for external
source dependencies, and put application arguments after `--`. The generated
server uses `server.bind_address` in `AppConfig.toml` for its port; it does not parse `--port` automatically.

Watched extensions: rs, toml, json, yaml, yml, sql, html, css, js, ts, graphql;
also `Cargo.lock`, `.env`, and `.env.*`. Explicit files can have any extension.
Build output, VCS directories, node_modules, dist, build, and editor directories
are excluded. The resolved Cargo target directory is excluded even when customized.
Nested symlinks are skipped; additional paths must exist at startup.

Unix shutdown signals the process group, waits up to five seconds, then kills
remaining processes. Windows uses `taskkill /T /F`. Application stdin is closed.
Reload restarts the process and resets in-memory state; browser live refresh is
not included. This command is available starting with CLI 0.1.7.

## CLI management and current-directory initialization

The following commands are available starting with CLI 0.1.8:

```sh
xecute update
xecute uninstall
xecute new .
xecute new . --name my-app --cqrs
```

Update installs the latest crates.io `xecute-cli` with `--locked --force` into the
running executable's Cargo installation root. Uninstall removes only that Cargo
package from the same root, preserving application files and Rust/Cargo itself.
Both require a Cargo-tracked installation, including custom `--root` installs;
source executables in `target/debug` must first be installed through Cargo.
Update replaces local builds with the registry release and does not upgrade app
dependencies. Windows launches a PowerShell helper after the executable exits;
Cargo's output reports its result. Unix commands wait and propagate failures.

`new .` scaffolds directly into the current directory. Its basename supplies the
package name unless `--name` is specified. MVC/CQRS and local runtime options work
as usual. All generated destinations are checked first; existing scaffold files
are never overwritten. Unrelated files and `.git` are preserved. Symlinked parent
directories are rejected. Use a fresh directory or move conflicting files before
retrying. A named destination also accepts `--name` to choose its package name.

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

## Root welcome and startup documentation links

CLI 0.1.10 generates a welcome JSON response at `GET /` with HTTP 200, the app
name, status, and a `/docs/` documentation path when Swagger is enabled. Startup
logs show the actual server, Swagger UI, and OpenAPI URLs. Disabled Swagger is
reported without links. Other unknown routes retain the normal 404 envelope.
Existing applications require the server changes; CLI updates do not rewrite them.

## Application configuration

Ordinary settings live in the generated root `AppConfig.toml`. Commit this file;
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

`Config::from_file()` reads `AppConfig.toml` relative to the working directory.
`Config::from_path(path)` supports an explicit file chosen in bootstrap code.
The file is parsed once into a validated snapshot shared with logger startup;
there is no automatic production reload. Development hot reload watches TOML
files and restarts the process when settings change.

The Dockerfile includes this file and adapts only the generated loopback bind to
`0.0.0.0:3000`. Custom bind addresses remain unchanged. To supply deployment
settings, mount a replacement `/app/AppConfig.toml` read-only. The default private
`/app/logs` directory is owned by UID 65532; custom log mounts must also be writable
by this user. A read-only root filesystem requires writable log storage or an
empty `logging.file_dir`.

For custom settings, use `xecute::config::ConfigFile` with dotted paths and extend
the allowed-key schema in `src/configs/server.rs`. Use `Env::process().secret(key)`
for required credentials and tokens. The template `.env.example` contains only
commented credential examples, with no active configuration or fake secrets.

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

Logging outputs use these settings in `AppConfig.toml`:

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
Ordinary settings use `ConfigFile::load("AppConfig.toml")` and typed dotted paths.
Generated server and logger use the same validated configuration snapshot.

```rust
let token = xecute::config::Env::process().secret("API_TOKEN")?;
let file = xecute::config::ConfigFile::load("AppConfig.toml")?;
let timeout = file.range("server.request_timeout_seconds", 30, 1, 3600)?;
```

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
