# __PROJECT_NAME__

```sh
cp .env.example .env
xecute run dev
# Single run without watching: cargo run
# Swagger UI: http://127.0.0.1:3000/docs/
# Example collection: GET http://127.0.0.1:3000/example
cargo test --locked
```

## Server defaults

The generated server validates environment configuration at startup and provides:

- JSON logs through `tracing`, configurable with `RUST_LOG`. Each request has
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

Copy `.env.example` to `.env`. Existing process environment variables take
precedence. Invalid or zero limits fail startup rather than silently disabling
protection. `BIND_ADDRESS` defaults to `127.0.0.1:3000`; use `0.0.0.0:3000`
inside a container. CORS lists use JSON arrays of strings, quoted for dotenv:

```dotenv
CORS_ORIGINS='["https://example.com","http://localhost:5173"]'
CORS_ALLOWED_METHODS='["GET","POST"]'
CORS_ALLOWED_HEADERS='["content-type","authorization"]'
CORS_EXPOSE_HEADERS='["x-request-id","retry-after"]'
CORS_ALLOW_CREDENTIALS=false
CORS_MAX_AGE_SECONDS=600
```

Use `[]` to disable allowed origins. Origins must have no trailing slash or path.
Credentials require explicit nonempty origins. Invalid arrays, methods, header
names, or scalar settings fail startup before CORS middleware is constructed.

The rate limiter uses the direct socket peer and deliberately ignores forwarding
headers. Behind a reverse proxy, all requests from that proxy share a quota.
Configure a trusted-proxy policy before adopting forwarded IPs. Multi-instance
deployments need a shared limiter such as Redis or rate limiting at the gateway.
Terminate TLS at your trusted ingress. Authentication and authorization depend
on your application and must be added before exposing protected resources.

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

Swagger UI is available at `/docs/`; OpenAPI JSON is served at `/docs/openapi.json`. The assets are embedded in the application. Set `SWAGGER_ENABLED=false` to disable both routes. Resource generation registers new controllers and their documentation automatically.

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
message, and `documentation: "/docs/"`. With `SWAGGER_ENABLED=false`, the
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

Newly generated applications include `public/hello.txt`, served at
`http://127.0.0.1:3000/static/hello.txt`. Static serving is enabled by default,
without environment settings. Add files to `public/` and access them at `/static/<filename>`.
For example, `public/images/logo.png` is served at `/static/images/logo.png`.
To customize the directory, prefix, or enabled state, edit `StaticConfig` in
`src/configs/static_files.rs`. Relative directories use the application's working
directory; the public directory must exist and cannot be the application root or a symlink.

Files support GET, HEAD, MIME types, conditional requests, and byte ranges through
Tower HTTP `ServeDir`. Directory listings and implicit `index.html` responses are
disabled; use explicit filenames. Hidden path components, parent traversal, and
symlinked assets are rejected. Keep public directories limited to intended public
assets; static files share the application's CORS, request IDs, security headers,
logging, rate limits, and global middleware. HTML retains the restrictive API CSP;
this is asset serving, not a generated SPA hosting configuration.

The Docker image copies `public/` into the runtime image. For another static directory,
adapt the Dockerfile or mount the asset directory read-only. Change files in the
public directory directly or mount updated assets; there is no binary embedding.

## File logging and external logging backends

Logging uses two environment settings:

```dotenv
LOG_STDOUT_ENABLED=true
LOG_FILE_DIR=logs
```

`LOG_STDOUT_ENABLED` controls JSON console output. A nonempty `LOG_FILE_DIR`
enables JSON file output and creates the private directory. Remove it or leave it
empty to disable file output. Generated projects include both settings above.
`RUST_LOG` continues to filter events across all outputs.

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

### Shared environment reader

`src/configs/environment.rs` owns `.env` loading and environment reads.
Startup calls `configs::load_env()` once; server and logging settings both reuse
`configs::read_env(key)`. Existing process variables take precedence over `.env`.
Missing values return `None`, empty values remain empty, and invalid UTF-8 returns
an error without exposing the value. Custom configuration can use the same helper:

```rust
let region = crate::configs::read_env("APP_REGION")?
    .unwrap_or_else(|| "local".to_owned());
```

Configuration tests can still inject a lookup through `from_lookup` without
changing process environment variables.
