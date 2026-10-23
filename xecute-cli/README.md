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
with OpenAPI JSON at `/docs/openapi.json`. Set `SWAGGER_ENABLED=false` to
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
server uses `BIND_ADDRESS` for its port; it does not parse `--port` automatically.

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
