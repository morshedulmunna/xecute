---
title: "DTO validation"
description: "Reject malformed and invalid requests before application work."
---

Put request rules in DTO structs under dto/mod.rs. Read the example, then use the status table to understand which requests are rejected before your handler runs.

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

## Keep learning

[Middleware](/docs/http/middleware). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#dto-validation-and-errors).
