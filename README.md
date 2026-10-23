# xecute

A Rust framework for Axum with dependency injection, modules,
automatic request validation, MVC and CQRS scaffolding, and scoped HTTP middleware.

Xecute keeps HTTP controllers thin. MVC resources delegate to an injectable
service; CQRS resources delegate to separate command and query handlers. Both
use an injectable repository and validated request DTOs.

## Contents

- [Release status](#release-status)
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
- [Environment configuration](#environment-configuration)
- [Testing and quality checks](#testing-and-quality-checks)
- [Deployment](#deployment)
- [Development and packages](#development-and-packages)
- [Troubleshooting](#troubleshooting)
- [Scope and limitations](#scope-and-limitations)

## Release status

Published packages are `xecute-cli 0.1.8`, `xecute 0.1.6`, and `xmacros 0.1.5`.
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
cargo install xecute-cli --version 0.1.8 --locked
# Update an existing installation:
cargo install xecute-cli --version 0.1.8 --locked --force
```

Create and run an MVC application:

```sh
xecute new my-app
cd my-app
cp .env.example .env
xecute run dev
# Single run without watching: cargo run
```

The application starts at `http://127.0.0.1:3000` with these endpoints:

| Method | Path | Purpose |
| --- | --- | --- |
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
cargo generate-lockfile
cargo run
```

`--runtime-path` selects the runtime dependency for the new project. It does not
switch which CLI binary is running; install the local CLI as shown above.

## CLI reference

```text
xecute new <project|.> [--name <package-name>] [--mvc | --cqrs] [--runtime-path <path>]
xecute update
xecute uninstall
xecute run dev [--bin <name>] [--watch <path>] [-- <app arguments>]
xecute generate <resource|module|controller|service> <name>
xecute g <res|mo|co|s> <name>
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
| `xecute run dev` | Rebuild and restart the debug application when watched files change |
| `xecute g resource user` or `xecute g res user` | Resource, module registration, routes, schemas, repository, and HTTP test |
| `xecute g module billing` or `xecute g mo billing` | Registered module with an empty provider list |
| `xecute g controller status` or `xecute g co status` | Standalone injectable controller with a GET endpoint |
| `xecute g service mailer` or `xecute g s mailer` | Standalone injectable service |

Run generators from the generated project root. Standalone controllers and
services require manual provider/route registration when you decide to use them.
Standalone generators remain available in both architecture modes.

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
server configures its port with `BIND_ADDRESS` instead.

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
    ├── configs/               # Validated environment settings
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
    ├── example/
    │   ├── mod.rs
    │   ├── module.rs
    │   ├── controller.rs
    │   ├── service.rs
    │   ├── dto/mod.rs
    │   ├── entity/mod.rs
    │   └── repository/
    │       ├── mod.rs
    │       └── store.rs
    └── tests/                 # Resource and server HTTP tests
```

Feature filenames are `controller.rs`, `module.rs`, and `service.rs`; the Rust
type names retain the feature prefix. Resource HTTP tests live in
`src/tests/<feature>.rs`.

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

Its resource replaces `service.rs` with:

```text
user/
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
dependency injection. Both handlers share the singleton repository. There is
no service layer in the generated CQRS resource.

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
values or field-level validation details. Customize `src/server/error.rs` for
application-specific messages. The runtime's `HttpError` itself returns a status;
the envelope comes from generated server middleware.

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
| `request` | Reused within an explicitly created request container |

Select a lifecycle with `#[scope("transient")]` or `#[scope("request")]` below
`#[derive(Injectable)]`. A singleton cannot directly depend on a request-scoped
provider. Request scopes require explicit `container.request()` management;
generated controller routing does not create request containers automatically.

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
operations automatically. Set `SWAGGER_ENABLED=false` to disable both routes.

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
| Logging | One flat JSON completion log per request, filtered by `RUST_LOG` |
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

## Environment configuration

Copy `.env.example` to `.env`. Process environment variables take precedence.
Invalid settings fail startup. Numeric request-protection limits must be positive;
CORS preflight cache duration can be zero.

| Variable | Default | Meaning |
| --- | --- | --- |
| `BIND_ADDRESS` | `127.0.0.1:3000` | Listener address; use `0.0.0.0:3000` in containers |
| `RUST_LOG` | `info` | Tracing filter |
| `REQUEST_TIMEOUT_SECONDS` | `30` | Handler deadline, 1–3600 seconds |
| `SHUTDOWN_GRACE_SECONDS` | `10` | Drain deadline, 1–300 seconds |
| `BODY_LIMIT_BYTES` | `1048576` | Body limit, up to 1 GiB |
| `MAX_IN_FLIGHT_REQUESTS` | `256` | Handler concurrency, up to 100,000 |
| `RATE_LIMIT_REQUESTS` | `60` | Requests allowed per peer per window |
| `RATE_LIMIT_WINDOW_SECONDS` | `60` | Fixed window, up to 86,400 seconds |
| `RATE_LIMIT_MAX_CLIENTS` | `10000` | Tracked IP capacity, up to 1,000,000 |
| `CORS_ORIGINS` | `[]` | Exact allowed browser origins |
| `CORS_ALLOWED_METHODS` | GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS | JSON array of allowed methods |
| `CORS_ALLOWED_HEADERS` | `content-type`, `authorization` | JSON array of allowed request headers |
| `CORS_EXPOSE_HEADERS` | `x-request-id`, `retry-after` | JSON array of exposed response headers |
| `CORS_ALLOW_CREDENTIALS` | `false` | Enable browser credentials for explicit origins |
| `CORS_MAX_AGE_SECONDS` | `600` | Preflight cache duration, 0–86,400 seconds |
| `SWAGGER_ENABLED` | `true` | Enable Swagger UI and OpenAPI endpoints |

CORS arrays use JSON syntax, single-quoted in dotenv files:

```dotenv
CORS_ORIGINS='["https://example.com","http://localhost:5173"]'
CORS_ALLOWED_METHODS='["GET","POST"]'
CORS_ALLOWED_HEADERS='["content-type","authorization"]'
CORS_EXPOSE_HEADERS='["x-request-id","retry-after"]'
CORS_ALLOW_CREDENTIALS=false
CORS_MAX_AGE_SECONDS=600
```

Origins have no path or trailing slash. Credentials require explicit nonempty
origins; wildcards are not accepted. Invalid arrays, methods, and header names
fail startup. `[]` disables allowed origins.

Rate limits use the direct socket peer and ignore forwarding headers. Behind a
proxy, requests from that proxy share a quota. Configure trusted-proxy handling
before using forwarded client addresses. Multiple server instances need a shared
rate limiter or a gateway policy if quotas must apply across instances.

## Testing and quality checks

In a generated application:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
```

Local runtime projects need an initial `cargo generate-lockfile` before locked
builds. Commit the resulting `Cargo.lock`. Refresh it after intentional dependency
changes and review the diff.

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
| Browser cross-origin requests fail | Configure exact `CORS_ORIGINS` plus the required methods and headers |
| Many clients share a rate-limit quota | Check whether a reverse proxy is the direct socket peer |
| Stored records disappear after restart | The generated repository is in memory; add durable storage |
| Container cannot resolve a local runtime dependency | Use registry dependencies or adapt the Docker build context |

## Scope and limitations

Xecute provides DI, module metadata, controller adapters, validation, HTTP
middleware, API documentation, and starter application infrastructure. It does
not currently generate an ORM/database integration, complete CRUD operations,
authentication, message buses, event sourcing, or CQRS handler pipelines.

The middleware API implements app, controller, and route scopes. `#[module]`
describes DI boundaries and does not automatically attach HTTP middleware.
Other reserved marker annotations, including guards, interceptors, lifecycle
hooks, and exception decorators, are not complete execution systems; do not rely
on them as operational middleware or lifecycle behavior.

Licensed under **Apache-2.0**.
