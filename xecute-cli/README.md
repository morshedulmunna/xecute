# Xecute CLI

Install from the repository root:

```sh
cargo install --path cli/xecute-cli
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

- `new <project> [--mvc | --cqrs] [--runtime-path <path>]`: create an independent Axum project.
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

Generated projects use `xecute = "0.1.3"` from crates.io by default.
Use `xecute new my-api --runtime-path /path/to/cli/xecute` for local development.

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
