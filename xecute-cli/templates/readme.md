# __PROJECT_NAME__

```sh
cp .env.example .env
cargo run
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

Resource generation includes `dto/`, `entity/`, `repository/`, and an HTTP test in `src/tests/`. DTOs use `validator` to reject invalid JSON payloads with HTTP 422 before saving. The included repository is bounded in-memory example storage; replace it with a database for durable production data.

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
