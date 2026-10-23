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
