---
title: "Middleware"
description: "Compose app, controller and route middleware in documented order."
---

Middleware wraps requests and responses around a handler. Use it for shared HTTP behavior; follow the execution order below before combining application, controller, and route middleware.

This section requires runtime/macros 0.1.5 or later; see [release status](#release-status).

Middleware functions accept `Request` and `Next` from `xecute::middleware` and
return a response or another `IntoResponse` value. Call `next.run(request).await`
to continue. Return directly to stop further middleware, validation, and handler
execution.

## App-level middleware

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

## Controller-level middleware

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

## Route-level middleware

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

## Execution order

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

## Keep learning

[Swagger and OpenAPI](/docs/http/openapi). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#middleware).
