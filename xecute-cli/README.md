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

Names such as `user-profile` become `user_profile/` and `UserProfileModule`.
No `#[path]` annotations are generated. Existing component folders are never
overwritten. Run generation from the project root. Automatic registration uses
the comments in `app_module.rs`; keep these markers when editing that file.

Generated projects use `xecute = "0.1.6"` from crates.io by default.
Use `xecute new my-api --runtime-path /path/to/xecute/xecute` for local development.

Generated CQRS resource folders contain `module.rs`, `controller.rs`,
`commands/create.rs`, `queries/list.rs`, `dto/`, `entity/`, and `repository/`.
New projects use MVC with `service.rs` by default.
Use `xecute new my-api --cqrs` to generate command/query handlers instead.
`--mvc` also explicitly selects MVC.
The selection is saved in `.xecute/architecture`; commit this file. Subsequent
resource generation follows that selection. Existing projects without the file
keep the legacy MVC layout. Invalid configuration stops generation.
Command and query handlers use direct dependency injection and share one repository.
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
