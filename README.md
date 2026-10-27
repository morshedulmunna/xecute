# xecute

Xecute is a set of tools and macros that helps structure your application's
architecture. It is not a framework: everything is built on Axum, an established
Rust web framework. Use Xecute's tools alongside Axum in your applications.

The tools include dependency injection, modules, automatic request validation,
MVC and CQRS scaffolding, and scoped HTTP middleware.

Xecute keeps HTTP controllers thin. MVC resources delegate to an injectable
service; CQRS resources delegate to separate command and query handlers. Both
use an injectable repository and validated request DTOs.

## Documentation

The [documentation site source](docs/README.md) organizes this reference into
topic guides, with explicit published/source feature availability and a walkthrough
for adopting the new capabilities in an existing application. Run it locally with
`cd docs && bun install --frozen-lockfile && bun run dev`.

## Contents

- [Release status](#release-status)
- [Changelog and GitHub releases](#changelog-and-github-releases)
- [Installation and quick start](#installation-and-quick-start)
- [CLI reference](#cli-reference)
- [Update and uninstall](#update-and-uninstall)
- [Development server and hot reload](#development-server-and-hot-reload)
- [Project structure](#project-structure)
- [MVC and CQRS](#mvc-and-cqrs)
- [Controllers and routing](#controllers-and-routing)
- [DTO validation and errors](#dto-validation-and-errors)
- [Dependency injection and modules](#dependency-injection-and-modules)
- [Middleware](#middleware)
- [Swagger and OpenAPI](#swagger-and-openapi)
- [Server defaults](#server-defaults)
- [Static files](#static-files)
- [File logging and external logging backends](#file-logging-and-external-logging-backends)
- [Application configuration](#application-configuration)
- [Multipart file uploads](#multipart-file-uploads)
- [API version groups and separate OpenAPI documents](#api-version-groups-and-separate-openapi-documents)
- [Typed configuration](#typed-configuration)
- [Exception handling and domain mappings](#exception-handling-and-domain-mappings)
- [Provider lifecycles and HTTP requests](#provider-lifecycles-and-http-requests)
- [Testing modules and HTTP assertions](#testing-modules-and-http-assertions)
- [Testing and quality checks](#testing-and-quality-checks)
- [Deployment](#deployment)
- [Development and packages](#development-and-packages)
- [Troubleshooting](#troubleshooting)
- [Scope and limitations](#scope-and-limitations)

## Release status

Published packages are `xecute-cli 0.1.20`, `xecute 0.1.10`, and `xmacros 0.1.8`.
MVC is the default; CQRS is selected with `--cqrs`.

This release includes `xecute run dev` with automatic development rebuild/restart,
app/controller/route middleware, the `Application` builder, and the generated
`src/middleware.rs` configuration hook. Middleware requires runtime/macros 0.1.5
or later; the dev runner requires CLI 0.1.7 or later.

CLI 0.1.8 adds `xecute update`, `xecute uninstall`, and `xecute new .`, including
custom package names through `--name` and safe in-place project initialization.

## Installation and quick start

This checkout and generated projects specify Rust **1.98.1** as their minimum
version. Generated projects include a `rust-toolchain.toml` pin for that version.
Use a Rust installation with Cargo, rustfmt, and Clippy.

Install the published CLI:

```sh
cargo install xecute-cli --version 0.1.20 --locked
# Update an existing installation:
cargo install xecute-cli --version 0.1.20 --locked --force
```

Create and run an MVC application:

```sh
xecute new my-app
cd my-app
# Optional credentials: cp .env.example .env
xecute run dev
# Single run without watching: cargo run
```

The application starts at `http://127.0.0.1:3000` with these endpoints:

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/` | Welcome JSON with the documentation path when Swagger is enabled |
| GET | `/static/hello.txt` | Example public asset (static serving enabled) |
| GET | `/example` | List example records |
| POST | `/example` | Create an example record |
| GET | `/health/live` | Liveness probe |
| GET | `/health/ready` | Startup readiness probe |
| GET | `/docs/` | Swagger UI |
| GET | `/docs/openapi.json` | OpenAPI 3.1 document |

```sh
curl http://127.0.0.1:3000/example
curl -X POST http://127.0.0.1:3000/example \
  -H 'content-type: application/json' \
  -d '{"name":"Example"}'
```

The initial collection is empty. A successful POST returns `201` with a generated
UUID and the stored name. Later GET requests return the created records. Storage
is in memory and resets when the process restarts.

To develop against this repository checkout, install its CLI and select its
local runtime:

```sh
cargo install --path xecute-cli --locked --force
# Run from the repository root; the runtime is the nested xecute directory.
xecute new local-app --runtime-path "$PWD/xecute"
cd local-app
cargo run
```

`--runtime-path` selects the runtime dependency for the new project. It does not
switch which CLI binary is running; install the local CLI as shown above.

## CLI reference

```text
xecute new <project|.> [--name <package-name>] [--mvc | --cqrs] [--runtime-path <path>]
xecute update
xecute uninstall
xecute run dev [--bin <name>] [--features <list>] [--watch <path>] [-- <app arguments>]
xecute generate <resource|module|controller|service> <name>
xecute generate worker <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]
xecute g <res|mo|co|s> <name>
xecute g wo <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]
xecute --help
xecute --version
```

| Command | Result |
| --- | --- |
| `xecute new my-app` | MVC application with a working example resource |
| `xecute new .` | Initialize the current directory using its name as the package name |
| `xecute new . --name my-app` | Initialize the current directory with an explicit package name |
| `xecute update` | Install the latest crates.io CLI into the running CLI's Cargo installation root |
| `xecute uninstall` | Remove the Cargo-installed CLI; application projects are retained |
| `xecute new my-app --mvc` | Explicit MVC selection |
| `xecute new my-app --cqrs` | Application with command/query handlers |
| `xecute new my-project --workspace` | Cargo workspace with its first MVC application under `apps/` |
| `xecute g apps admin [--mvc \| --cqrs]` | Add an app from the workspace root; inherit architecture unless overridden |
| `xecute g library shared-utils` or `xecute g lib shared-utils` | Create a reusable crate under `libs/` from the workspace root |
| `xecute run dev` | Rebuild and restart the debug application when watched files change |
| `xecute g resource user` or `xecute g res user` | Resource, module registration, routes, schemas, repository, and HTTP test |
| `xecute g module billing` or `xecute g mo billing` | Registered module with an empty provider list |
| `xecute g controller status` or `xecute g co status` | Standalone injectable controller with a GET endpoint |
| `xecute g service mailer` or `xecute g s mailer` | Standalone injectable service |
| `xecute g worker email` or `xecute g wo email` | Registered process-local job queue with payload, handler, and `jobs` feature |
| `xecute g wo email --distributed` | Durable worker backed by the default PostgreSQL outbox |
| `xecute g wo email --broker rabbitmq` | Durable RabbitMQ worker; `--broker` implies `--distributed` |
| `xecute g wo email --distributed --broker nats` | NATS queue-group worker with at-most-once delivery |

Run feature generators from the generated project root. Run `g apps` and
`g library` from the workspace root. Standalone controllers and
services require manual provider/route registration when you decide to use them.
Standalone generators remain available in both architecture modes.

Workers are registered automatically and enable the runtime's `jobs` feature.
The default in-process queue provides 64 waiting slots and four workers with
three attempts, a 250 ms retry delay, and a 30-second attempt timeout. Queues
drain when the generated server shuts down. Jobs are process-local and are not
persisted or shared across machines.

Add `--distributed` to generate a durable producer and consumer. `--broker`
accepts `postgres-outbox` (the default), `rabbitmq`, or `nats`, and implies
`--distributed`:

- `postgres-outbox` stores jobs in `xecute_<feature>_jobs`, claims them with
  `FOR UPDATE SKIP LOCKED`, and reads `DATABASE_URL`.
- `rabbitmq` consumes a durable `<feature>_jobs` queue and reads `RABBITMQ_URL`
  (default `amqp://127.0.0.1:5672/%2f`). Failed deliveries are retried up to
  three times.
- `nats` uses a `<feature>-workers` queue group on `<feature>.jobs` and reads
  `NATS_URL` (default `nats://127.0.0.1:4222`). Core NATS delivery is
  at-most-once; handler failures are logged without redelivery.

Distributed workers expose `<Feature>Queue::enqueue(...)` and `shutdown()`
instead of `JobQueue`. Generate workers from an application using the current
async container startup.

Names must start with a lowercase ASCII letter and contain lowercase letters,
digits, hyphens, or underscores. Rust keywords and empty name segments are
rejected. `user-profile` becomes the module `user_profile` and types such as
`UserProfileController`. Existing feature paths are not overwritten.

The CLI saves the architecture as `mvc` or `cqrs` in `.xecute/architecture`.
Commit this file. Subsequent resource generation follows that selection; projects
without it retain the legacy MVC layout. Invalid values stop generation.
Changing the file changes future generation, not existing feature code.

Keep the `// xecute:` registration markers in `src/app_module.rs`. Generators
use them to add module imports, container definitions, routes, and API metadata.

## Update and uninstall

For a CLI installed through Cargo:

```sh
xecute update
xecute uninstall
```

`update` runs `cargo install xecute-cli --registry crates-io --locked --force` in
the same installation root as the running executable. It updates the CLI and
its templates; it does not edit application dependencies or regenerate existing
projects. It requires Cargo and access to crates.io. A local/source installation
is replaced by the published registry version, which may not include unreleased
local features. Check the release status above before updating a development build.

`uninstall` runs `cargo uninstall xecute-cli` against that same root. It removes
the installed CLI, including installations created with `cargo install --root`.
It does not remove application directories, source code, or the Cargo toolchain.

The CLI verifies Cargo's `.crates2.json` installation metadata before invoking
these commands. Executables run directly from `target/debug` or copied outside a
registered installation are rejected with guidance to use Cargo directly.
Unix commands wait for Cargo and report failures. On Windows, a PowerShell helper
waits for the running executable to exit before starting Cargo; its terminal
output reports completion or errors after the CLI returns.

To initialize an existing directory without creating a nested folder:

```sh
mkdir my-api
cd my-api
xecute new .
xecute run dev

# Optional package name, architecture, and local runtime:
xecute new . --name my-api --cqrs --runtime-path /absolute/path/to/xecute/xecute
```

The final example is an alternative initialization command for an uninitialized
directory. The default package name comes from the current directory's basename;
use `--name` if the basename contains spaces, uppercase letters, or other invalid
characters. `--name` can also override a named project's Cargo package name.
MVC remains the default, and `--cqrs` selects CQRS.

In-place generation checks all scaffold destinations before writing. Conflicting
files such as `Cargo.toml`, `README.md`, or `.gitignore` cause a clear error with
no scaffold files changed. Move those conflicting files or choose a fresh directory
before retrying. Unrelated files and an existing `.git` directory are preserved.
Symlinked scaffold parents are rejected. Generation does not change the shell's
working directory and does not print an extra `cd` step for `new .`.

## Development server and hot reload

With CLI 0.1.7 or later installed, run this from your application's directory:

```sh
xecute run dev
```

The CLI starts `cargo run` in the debug profile and monitors project files. Saving
source or configuration changes automatically stops the previous process, rebuilds,
and restarts the application. It uses Cargo's incremental build cache; there is
no separate watcher executable to install.

```sh
# Choose a binary in a project with several binary targets:
xecute run dev --bin server

# Also watch a local runtime or another source directory (repeatable):
xecute run dev --watch ../xecute/xecute --watch ../xecute/xmacros

# Pass arguments to the application, preserving individual argument boundaries:
xecute run dev -- --port 4000
```

The application must implement any arguments passed after `--`; the generated
server configures its port with `server.bind_address` in `config/AppConfig.toml` instead.

The watcher polls file metadata every 200 ms and groups edits with a 300 ms quiet
period. It detects additions, changes, removals, and rename-save operations for
Rust, TOML, JSON, YAML, SQL, HTML, CSS, JavaScript, TypeScript, and GraphQL files,
plus `Cargo.lock`, `.env`, and `.env.*`. An explicitly watched file can have any
extension. Additional watch paths must exist when the command starts. Files in
symlinked subdirectories are not followed; pass a real path with `--watch` if needed.

Cargo's resolved target directory is excluded, including custom
`CARGO_TARGET_DIR` settings. Directories named `target`, `.git`, `.hg`, `.svn`,
`node_modules`, `dist`, `build`, `.idea`, and `.vscode` are also excluded. Runtime
logs and ordinary data files are not watched unless their extension is in the
list above or they are explicitly selected. Keep generated JSON/configuration
output outside the watched source tree to avoid restart loops.

Compiler diagnostics and application logs remain visible in the terminal. A
failed build or exited app leaves the watcher running; saving another change
retries it. An edit during compilation cancels the old process before starting
the next build. If the project has multiple runnable binaries, select one with
`--bin`. Dependencies outside the project tree need explicit `--watch` paths.

Ctrl-C stops the watcher and its managed Cargo/application processes. On Unix,
restarts and shutdown send SIGTERM to the process group, allow up to five seconds
for cleanup, then force termination if necessary. On Windows, the process tree
is terminated with `taskkill /T /F`. Dev shutdown can therefore be shorter than
the generated server's normal production drain deadline. Application stdin is
closed; this command is intended for development servers, not interactive programs.

This is **rebuild-and-restart hot reload**: it restarts the server process, resets
in-memory data, and does not automatically refresh the browser or patch a running
process. Use `cargo run` for a single run and `cargo build --release --locked`
for a production binary. Hot reload runs only when explicitly using `xecute run dev`.

## Project structure

A generated MVC project contains:

```text
my-app/
├── .xecute/architecture
├── config/
│   └── AppConfig.toml
├── .env.example
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── Dockerfile
├── .github/workflows/ci.yml
└── src/
    ├── main.rs
    ├── app_module.rs
    ├── middleware.rs           # Global middleware hook
    ├── configs/               # Validated TOML settings and secret readers
    ├── server/
    │   ├── bootstrap.rs       # Listener and graceful shutdown
    │   ├── router.rs          # Shared HTTP policies and probes
    │   ├── docs.rs            # Swagger/OpenAPI endpoints
    │   ├── error.rs           # Public error envelope
    │   ├── health.rs
    │   ├── logging.rs
    │   ├── rate_limit.rs
    │   ├── shutdown.rs
    │   └── middleware/        # Built-in guards, logging, response handling
    ├── modules/
    │   ├── mod.rs
    │   └── example/
    │       ├── mod.rs
    │       ├── module.rs
    │       ├── controller.rs
    │       ├── service.rs
    │       ├── dto/mod.rs
    │       ├── entity/mod.rs
    │       ├── repository/
    │       │   ├── mod.rs
    │       │   └── store.rs
    │       └── tests/
    │           ├── mod.rs
    │           └── http.rs
    └── tests/                 # Shared server HTTP tests
```

Feature filenames are `controller.rs`, `module.rs`, and `service.rs`; the Rust
type names retain the feature prefix. Resource HTTP tests live in
`src/modules/<feature>/tests/http.rs`.

## MVC and CQRS

MVC is the default resource layout:

```text
Request → DTO validation → Controller → Service → Repository → Response
```

Create a CQRS project with:

```sh
xecute new my-api --cqrs
cd my-api
xecute g resource user
```

Its resource uses command/query handlers that inject the repository:

```text
src/modules/user/
├── controller.rs
├── module.rs
├── mod.rs
├── commands/
│   ├── mod.rs
│   └── create.rs    # CreateUserCommand + CreateUserHandler
├── queries/
│   ├── mod.rs
│   └── list.rs      # ListUserQuery + ListUserHandler
├── dto/mod.rs
├── entity/mod.rs
└── repository/
    ├── mod.rs
    └── store.rs
```

```text
POST → DTO validation → Controller → CreateUserHandler → Repository write
GET  → Controller → ListUserHandler → Repository read
```

Controllers construct command/query inputs and invoke handlers through direct
dependency injection. The command handler coordinates writes and normalizes input;
the query handler reads records. Both inject the same singleton repository, so
writes are visible to subsequent reads. The module registers the repository and
exports its handlers. Add a focused business service when an operation needs
reusable domain logic. MVC keeps its existing `service.rs` layout.

Existing CQRS features are preserved; migrate their handlers and DI registrations
manually to adopt this layout.

Initial resources implement collection GET and create POST. Update, delete,
get-by-ID, pagination, and their handlers are application extensions. CQRS does
not require separate databases, event sourcing, events, or a message bus; these
are not generated automatically.

## Controllers and routing

Use `#[controller]` on the struct and `#[routes]` on its inherent impl. An MVC
resource controller can use the generated service and DTO as follows:

```rust
//! HTTP endpoints for the user feature.

use std::sync::Arc;
use xecute::prelude::*;
use super::{dto::CreateUserDto, entity::User, service::UserService};

#[controller("/user")]
#[derive(Injectable)]
/// User collection and creation endpoints.
pub struct UserController {
    service: Arc<UserService>,
}

#[routes]
impl UserController {
    #[get("")]
    #[api(summary = "List users", tag = "Users", operation_id = "listUsers")]
    #[resp(200, "Current users", json = Vec<User>)]
    async fn list(&self) -> Json<Vec<User>> {
        Json(self.service.list().await)
    }

    #[post("")]
    #[body(json = CreateUserDto)]
    #[resp(201, "User created", json = User)]
    #[resp(422, "Invalid payload")]
    #[resp(503, "Repository capacity exhausted")]
    async fn create(
        &self,
        #[body] payload: CreateUserDto,
    ) -> Result<(HttpStatus, Json<User>), HttpError> {
        let user = self.service.create(payload).await
            .map_err(|_| HttpError::new(HttpStatus::SERVICE_UNAVAILABLE))?;
        Ok((HttpStatus::CREATED, Json(user)))
    }
}
```

Supported HTTP annotations are `#[get]`, `#[post]`, `#[put]`, `#[patch]`,
`#[delete]`, `#[head]`, and `#[options]`. A handler has one HTTP annotation.
Prefixes and nonempty route suffixes must start with `/`.

| Parameter | Meaning |
| --- | --- |
| `#[body] payload: CreateUserDto` | Decode JSON and validate the DTO |
| `#[query] filter: SearchDto` | Decode the query string and validate the DTO |
| `#[param] id: String` | Extract an Axum path parameter, e.g. `#[get("/{id}")]` |
| `identity: axum::Extension<Identity>` | Use an Axum extractor directly |

Receiver handlers use `&self`, simple parameter names, and async, non-generic
methods. Only one JSON body parameter is supported. When using ordinary Axum
extractors, place any body-consuming extractor last. `#[body]` is moved to the
last extractor position automatically.

`Controller::router(Arc<Controller>)` builds its Axum router.
`Controller::api_operations()` returns documentation metadata. The older
`#[controller("/prefix")] impl Controller` syntax remains supported.

## DTO validation and errors

Validation uses the `validator` crate directly. Generated request DTOs derive
`Deserialize`, `Validate`, and `JsonSchema`, and reject unknown fields:

```rust
//! User request contracts and validation rules.

use schemars::JsonSchema;
use serde::Deserialize;
use validator::{Validate, ValidationError};

#[derive(Deserialize, Validate, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Validated input for user creation.
pub struct CreateUserDto {
    /// Display name containing 3–100 characters and non-whitespace content.
    #[schemars(length(min = 3, max = 100))]
    #[validate(length(min = 3, max = 100), custom(function = "non_blank"))]
    pub name: String,
}

fn non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("name must not be blank"));
    }
    Ok(())
}
```

`#[body]` and `#[query]` run validation at the controller's extraction boundary.
You do not call `.validate()` manually in the controller. Invalid input returns
an HTTP response before the controller method, service, command handler, or
repository runs. Middleware already entered can still process that response.
Direct calls to services or command handlers bypass HTTP extraction; enforce any
required domain invariants at those entry points too.

| Input failure | HTTP status |
| --- | --- |
| Malformed JSON syntax | `400 Bad Request` |
| Missing JSON fields, wrong field types, unknown fields, failed DTO rules | `422 Unprocessable Entity` |
| Missing/unsupported JSON content type | `415 Unsupported Media Type` |
| Query decoding failure | `400 Bad Request` |
| Query DTO validation failure | `422 Unprocessable Entity` |
| Oversized request body | `413 Payload Too Large` |

The generated server normalizes failures to a safe JSON envelope:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "Invalid request data",
    "request_id": "<request UUID>"
  }
}
```

Responses include `X-Request-Id`. Public validation errors do not expose submitted
values or field-level validation details. Use explicit domain-error mappings to
`HttpError` for safe application messages. The runtime produces JSON errors; the
generated middleware supplies correlation IDs and normalizes other failures.

## Dependency injection and modules

Use `#[derive(Injectable)]` for named-field structs. Dependency fields are resolved
from the container; use `Arc<T>` to share a registered provider. Initialize local
state with `#[inject(default)]` rather than registering that state as a provider:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use xecute::Injectable;

#[derive(Injectable)]
pub struct UserRepository {
    #[inject(default)]
    names: RwLock<Vec<String>>,
}

#[derive(Injectable)]
pub struct UserService {
    repository: Arc<UserRepository>,
}
```

The `#[injectable]` struct attribute is also supported. Modules define provider
registrations, imports, and exports:

```rust
use xecute::module;
use super::{UserController, UserService};
use super::repository::UserRepository;

#[module(
    providers = [UserRepository, UserService, UserController],
    controllers = [UserController],
    exports = [UserService],
)]
pub struct UserModule;
```

Register controllers in `providers`; the `controllers` list does not replace
provider registration. Module metadata alone does not mount HTTP routes. The
resource generator also registers module definitions and controller routers in
`AppModule`. For manual assembly, pass all module definitions to
`ModuleGraph::new(...).build()`, resolve controllers, and merge their routers.

Import an owning module to use its exported providers from another module.
Startup validation detects missing registrations, duplicates, circular
relationships, invalid imports/exports, and inaccessible private providers.

For a small manually assembled container:

```rust
let container = xecute::Container::builder()
    .injectable::<UserRepository>()
    .injectable::<UserService>()
    .build()?;
let service = container.resolve::<UserService>()?;
```

| Scope | Behavior |
| --- | --- |
| `singleton` | Default; one instance for the container lifetime |
| `transient` | A new instance for each resolution |
| `request` | Reused within one HTTP request or an explicit request container |

Select a lifecycle with `#[scope("transient")]` or `#[scope("request")]` below
`#[derive(Injectable)]`. Singleton-to-request dependencies are rejected, including
transitive relationships. Generated routing creates request contexts automatically;
explicit `container.request()` contexts are available for jobs and tests.

## Middleware

This section requires runtime/macros 0.1.5 or later; see [release status](#release-status).

Middleware functions accept `Request` and `Next` from `xecute::middleware` and
return a response or another `IntoResponse` value. Call `next.run(request).await`
to continue. Return directly to stop further middleware, validation, and handler
execution.

### App-level middleware

The generator creates `src/middleware.rs`. Replace its default configuration
with a middleware registration, for example:

```rust
use xecute::{Application, middleware::{Request, Next, Response}};

pub fn configure(app: Application) -> Application {
    app.use_middleware(audit)
}

pub async fn audit(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert("x-audited", "true".parse().unwrap());
    response
}
```

The generated server invokes this hook for application routes, documentation,
health probes, and unmatched URLs. Request logging and response normalization
wrap the hook, including its early error responses. A global authentication
policy must explicitly allow any public endpoints it needs, such as probes.

Existing projects can wrap their assembled router directly:

```rust
let app = xecute::Application::new(router)
    .use_middleware(audit)
    .use_middleware(metrics)
    .into_router();
```

Add routes before constructing/finalizing this pipeline. Only routes and
fallbacks already present are wrapped. Global registrations can be closures
capturing shared state such as `Arc<YourDependency>`.

### Controller-level middleware

Place middleware below the controller attribute:

```rust
#[controller("/user")]
#[middleware(crate::middleware::audit)]
#[derive(Injectable)]
/// User collection and creation endpoints.
pub struct UserController {
    service: Arc<UserService>,
}
```

It runs for every matched route belonging to that controller. It does not run
for unrelated URLs that return 404.

Controller middleware can also be declared on the routing impl:

```rust
#[routes]
#[middleware(crate::middleware::audit)]
impl UserController {
    // HTTP handlers...
}
```

If the controller has both struct and impl middleware, struct middleware wraps
impl middleware. Use the standard `#[controller]` struct plus `#[routes]` impl
form when combining controller metadata and middleware.

### Route-level middleware

Place middleware on a handler inside a `#[routes]` impl:

```rust
#[routes]
impl UserController {
    #[get("")]
    #[middleware(crate::middleware::audit)]
    async fn list(&self) -> Json<Vec<User>> {
        Json(self.service.list().await)
    }
}
```

This middleware applies only to that HTTP method and route. It does not attach
to a POST handler sharing the same path.

An early-return middleware can reject a request without invoking the controller:

```rust
use xecute::middleware::{Request, Next};

async fn temporarily_unavailable(_: Request, _: Next) -> axum::http::StatusCode {
    axum::http::StatusCode::SERVICE_UNAVAILABLE
}
```

### Execution order

```text
Request:
  App middleware → Controller struct middleware → Controller impl middleware
  → Route middleware → Extraction/validation → Handler

Response:
  Handler/rejection → Route → Controller impl → Controller struct → App
```

Within a scope, declaration order is execution order:

```rust
#[middleware(first, second)]
// Equivalent ordering to consecutive #[middleware(first)] and #[middleware(second)].
```

The response unwinds in reverse order. A short-circuit skips later middleware
and the handler; earlier middleware can still inspect or modify its response.

Keep `#[controller]` and `#[routes]` above their middleware annotations so the
owning macros process them. Empty middleware lists and middleware on methods
without an HTTP route attribute are rejected. Controller/route annotations take
function paths; use app-level closures or request extensions for shared state.
These are HTTP middleware, not automatic CQRS handler pipelines.

## Swagger and OpenAPI

Every generated project includes embedded Swagger UI at `/docs/` and an OpenAPI
3.1 document at `/docs/openapi.json`. Resource generation adds each controller's
operations automatically. Set `docs.enabled = false` in `config/AppConfig.toml` to disable both routes.

| Annotation | Purpose |
| --- | --- |
| `#[api(summary = "...", tag = "...", operation_id = "...")]` | Describe an endpoint and its operation ID |
| `#[body(json = RequestDto)]` | Declare the JSON body type; it must match the `#[body]` parameter |
| `#[resp(201, "Created", json = ResponseDto)]` | Describe a response and schema |
| `#[resp(422, "Invalid payload")]` | Describe a response without a JSON schema |

DTOs and response entities derive `schemars::JsonSchema`. Keep schema constraints
such as `#[schemars(length(...))]` aligned with validation attributes. Schema
generation describes data; it does not itself validate requests. API metadata
does not change the HTTP status returned by a handler.

For manual documentation assembly, use `Controller::api_operations()` with
`xecute::openapi_document(title, version, operations)`.

## Server defaults

Generated applications include the following HTTP policies:

| Policy | Default behavior |
| --- | --- |
| Logging | One flat JSON completion log per request, filtered by `logging.filter` |
| Correlation | UUID `X-Request-Id` response header |
| Rate limit | 60 requests per socket peer IP per 60-second fixed window |
| Limiter capacity | At most 10,000 tracked IPs per process |
| Concurrency | At most 256 active handlers; overload returns `503` |
| Deadline | 30 seconds until response headers; expiry returns `504` |
| Body limit | 1 MiB; oversized declared or consumed bodies return `413` |
| CORS | No allowed cross-origin browser origins by default |
| Response headers | `nosniff`, frame denial, no-referrer, restrictive API CSP |
| Errors | Safe JSON envelopes for rejection and failure responses |
| Shutdown | Ctrl-C/SIGTERM handling with a 10-second drain deadline |
| Health | Liveness and startup-readiness probes bypass built-in quotas/concurrency limits |

Logs omit payloads, authorization headers, cookies, query strings, and full URLs.
Matched requests log the route template; unmatched requests may log the requested
path. Streaming-body completion is not included in latency measurements.

The timeout and concurrency boundary ends when response headers are produced.
Timeouts cancel the handler future but cannot undo completed writes or stop
separately spawned tasks. Readiness indicates startup success until you add
checks for databases and other dependencies.

## Application configuration

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

## Testing and quality checks

In a generated application:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
```

New standalone projects resolve dependencies with `cargo generate-lockfile` during
setup, including `--runtime-path` projects. This selects the latest stable versions
within the template’s compatibility ranges, instead of copying a bundled lockfile.
Registry access is required unless Cargo is configured for offline cached resolution.
Resolution failures return an error and preserve generated files; fix registry/network
access and rerun `cargo generate-lockfile` in the project. Commit `Cargo.lock` for
reproducible builds. Breaking major upgrades require updated templates. Existing
workspace apps retain shared dependency versions when adding another app; setup
does not force upgrades across an existing workspace.

Resource HTTP tests verify rejection before persistence and visibility of created
records through subsequent reads. Server tests cover configuration, quota and
concurrency behavior, timeouts, body limits, response envelopes, CORS, request IDs,
logging, and Swagger routes. The workspace middleware tests cover scope ordering,
early returns, validation boundaries, and compatibility with legacy controllers.

The generator also includes CI, rustfmt/Clippy configuration, EditorConfig,
Dependabot configuration, and `deny.toml`. To run the dependency policy check:

```sh
cargo install cargo-deny --locked
cargo deny --locked check
```

Document public items with `///` and modules with `//!`. Missing documentation
produces compiler warnings; Clippy CI treats warnings as errors.

## Deployment

From a generated application using registry dependencies:

```sh
docker build --tag my-app:local .
docker run --rm --name my-app \
  --read-only --cap-drop=ALL --security-opt=no-new-privileges \
  --pids-limit=128 --memory=512m --cpus=1 --stop-timeout=15 \
  -p 127.0.0.1:3000:3000 my-app:local
```

The Dockerfile builds a stripped static musl binary for amd64 or arm64 and runs
it in `gcr.io/distroless/static-debian13:nonroot`. Its container listener defaults
to `0.0.0.0:3000`. The final image contains no build tools, shell, package manager,
or curl. Configure HTTP health probes in the orchestrator.

Pass runtime configuration through environment variables or the deployment's
secret mechanism. `.env` is excluded from source control and the Docker context.
Allow a stop timeout longer than `SHUTDOWN_GRACE_SECONDS`. Local runtime path
dependencies require a custom Docker build context. Native dependencies must
support static musl linking if you retain the generated build configuration.

Replace in-memory repositories with durable storage, add application-specific
authentication/authorization, and terminate TLS at your ingress. Add readiness
checks for external dependencies when introducing them.

## Development and packages

Repository: [morshedulmunna/xecute](https://github.com/morshedulmunna/xecute).

| Package | Role | Documentation |
| --- | --- | --- |
| `xecute` | Runtime, DI container, validation, HTTP middleware, OpenAPI helpers | [Runtime README](xecute/README.md) |
| `xmacros` | Injection, module, controller, and route procedural macros | [Macro README](xmacros/README.md) |
| `xecute-cli` | `xecute` binary and project/resource templates | [CLI README](xecute-cli/README.md) |

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo install --path xecute-cli --locked --force
```

Use `--runtime-path /absolute/path/to/xecute/xecute` when exercising local runtime
changes. CLI template changes affect newly generated files; they do not rewrite
existing applications automatically.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Installed CLI lacks `run dev` or middleware support | Update to CLI 0.1.7 or later and use runtime/macros 0.1.5 or later |
| Generated app cannot find `Application` | Update its runtime/macros to 0.1.5 or later |
| Locked build says the lockfile must change | Run `cargo generate-lockfile`, review it, and commit the result |
| Generator cannot register a resource | Run from the project root and retain the `// xecute:` markers in `app_module.rs` |
| Resource already exists | Choose another feature name or edit the existing feature; generators do not overwrite it |
| Middleware annotation produces an ordering error | Place it below the owning `#[controller]` or `#[routes]` macro |
| Wrong payload reaches a manually invoked handler | Automatic validation occurs through the generated HTTP extractor, not arbitrary Rust method calls |
| Browser cross-origin requests fail | Configure exact `cors.origins` plus the required methods and headers |
| Many clients share a rate-limit quota | Check whether a reverse proxy is the direct socket peer |
| Stored records disappear after restart | The generated repository is in memory; add durable storage |
| Container cannot resolve a local runtime dependency | Use registry dependencies or adapt the Docker build context |

## Scope and limitations

Xecute provides DI, module metadata, controller adapters, validation, HTTP
middleware, API documentation, and starter application infrastructure. It does
not currently generate database-backed repositories, complete CRUD operations,
login/session scaffolding, broker transports, or event sourcing. Optional
SQLx PostgreSQL integration supports lifecycle-managed pools, migrations, and native
transactions; see the [PostgreSQL guide](xecute/docs/postgres.md).

The middleware API implements app, controller, and route scopes. `#[module]`
describes DI boundaries and does not automatically attach HTTP middleware.
Executable guards support application, controller, and route policies. Optional
JWT authentication and role/permission checks are documented in the
[authentication guide](xecute/docs/auth.md). Optional [background jobs](xecute/docs/jobs.md)
provide bounded process-local workers and interval schedules; [TTL caching](xecute/docs/cache.md)
supports memory and Redis backends. Optional [CQRS buses](xecute/docs/cqrs.md)
provide typed command/query dispatch, event fan-out and shared pipelines; direct
handler CQRS remains supported. Reserved attribute markers, including
interceptors, lifecycle attributes, and exception decorators, are not complete
execution systems. Async lifecycle
behavior is available through explicit provider registration methods:
`singleton_async`, `on_init`, and `on_shutdown`; await `initialize_async` before
serving and `shutdown_async` after request draining. See the
[runtime lifecycle guide](xecute/README.md#async-providers-and-application-lifecycle).

Licensed under **Apache-2.0**.

## Feature module folders

All newly generated resources, standalone modules, controllers, services, and
workers live under `src/modules/<feature>/`. Each resource keeps its controller,
DI module, DTOs, entities, repository, and HTTP tests together. CQRS resources
also contain commands and queries. Workers keep their payload, handler, queue
provider, and optional broker transport together. `src/modules/mod.rs` declares
each feature; `AppModule` imports and mounts it through `crate::modules::<feature>`.

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

Startup emits one `server listening` JSON log with the actual bound address and
local URL, including when binding port zero. `GET /` exposes the address, URL,
Swagger and OpenAPI paths, versioned documentation links, and static-file mount.
Disabled endpoints are null or omitted from the version list. Wildcard bindings
use loopback URLs and IPv6 addresses are bracketed. Configure public URLs
separately when running behind a TLS ingress or reverse proxy.

Available starting with CLI 0.1.10. Existing projects need the corresponding
server changes; updating the CLI does not rewrite them. The runtime crate is unchanged.

## Changelog and GitHub releases

See [CHANGELOG.md](CHANGELOG.md) for version-by-version changes and
[GitHub Releases](https://github.com/morshedulmunna/xecute/releases) for published
release notes. Git tags alone do not create GitHub Release pages.

Maintain an `## [VERSION] - YYYY-MM-DD` section in `CHANGELOG.md` before releasing.
Publish the crate, then push the matching CLI tag, such as `v0.1.10`. The
[release workflow](.github/workflows/release.yml) copies that exact changelog
section into the GitHub Release body and adds the versioned installation command.
It creates a missing release or updates the notes of an existing release without
moving its source tag. Historical CLI tags can be backfilled through the
workflow's **Run workflow** input.

Changelog/workflow changes on `main` also synchronize the latest changelog
version when its tag already exists; an untagged future version is deferred.
This initializes the existing latest release when release automation is first
added. It does not publish crates or create version tags automatically.

Preview the release body locally:

```sh
python3 scripts/release_notes.py --tag v0.1.10 --output /tmp/xecute-release-notes.md
```

When manually using GitHub's **Create a new release** page, **Generate release
notes** uses the PR-label categories in [.github/release.yml](.github/release.yml).
For the curated notes in this repository, use the changelog-backed workflow or
paste the corresponding changelog section into the release description.

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
client instead of expecting factory retries. `container.initialize_async()`
eagerly constructs asynchronous and synchronous singletons before generated
applications listen. Generated servers call `shutdown_async()` after the
listener stops so queues drain and lifecycle hooks run. A transient injected into
a singleton is created when that singleton is built, then retained by it.

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
let container = Arc::new(module.compile_async().await?);
let routes = crate::app_module::AppModule::router_with_container(container.clone()).await?;
let app = TestApp::new(crate::server::router(routes, &crate::configs::Config::default()));

let response = app.post("/example")
    .json(&serde_json::json!({"name": "Example"}))
    .send().await?
    .assert_status(axum::http::StatusCode::CREATED);
let body: serde_json::Value = response.json()?;
container.shutdown_async().await?;
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
active. `GET /` lists each enabled version's Swagger and OpenAPI paths.
The registration pattern is explicit URL versioning; header-based versioning and
automatic deprecation/sunset behavior are not implied.

## Multiple applications with Cargo workspaces

Workspace scaffolding is available starting with CLI 0.1.14. MVC is the default:

```sh
xecute new my-project --workspace
cd my-project
xecute g apps admin
xecute g apps billing --cqrs
xecute g apps dashboard --mvc
cargo build --workspace
cargo test --workspace
```

The first application is created at `apps/my-project/`. Each app keeps its own
configuration, HTTP server, feature modules, and MVC or CQRS architecture.
Applications share the root `Cargo.lock`, `target/`, and dependency definitions
in `[workspace.dependencies]`. App dependencies use `workspace = true`, with
features selected per app. New applications inherit
the workspace architecture; use `--mvc` or `--cqrs` to override it.
`xecute new my-project --workspace --cqrs` selects CQRS for the workspace.

Run resource generators and the development server inside an app:

```sh
cd apps/admin
xecute g resource user
xecute run dev
```

Configure distinct bind addresses in the apps' `config/AppConfig.toml` files
when running them together. See [the CLI workspace guide](xecute-cli/README.md#multiple-applications-in-a-workspace)
for local runtime paths, container builds, and in-place initialization.

## Shared libraries

Generate a reusable Cargo library from a Xecute workspace root:

```sh
xecute g library shared-utils
# Alias: xecute g lib shared-utils
cargo test -p shared-utils --locked
```

The library lives in `libs/shared-utils`, is registered as a workspace member and
shared path dependency, and includes a README, sample function, unit tests and
doc test. Add `shared-utils = { workspace = true }` to a consuming application's
`[dependencies]`, then import `shared_utils` in Rust. Libraries are private by
default and do not add HTTP routes or DI registrations automatically. Existing
files/dependencies, package-name collisions, excluded paths and symlink targets
are checked before committing the library. A failed lockfile refresh preserves
generated files and reports how to recover.

Read the [library guide](https://xecute-docs.vercel.app/docs/cli/libraries) for
workspace inheritance, provider sharing, and failure recovery.
