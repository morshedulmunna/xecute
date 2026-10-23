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
