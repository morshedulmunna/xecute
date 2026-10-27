---
title: "Controllers and extractors"
description: "Route handlers, JSON bodies, path parameters and query DTOs."
---

A controller connects an HTTP request to an application operation. Use the example below inside a generated user feature, then choose body, path, or query inputs from the extractor table.

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

## Keep learning

[DTO validation](/docs/http/validation). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#controllers-and-routing).
