---
title: "Errors and domain mappings"
description: "Map failures to safe correlated HTTP error envelopes."
---

Turn application failures into safe HTTP responses. Choose a status and public message deliberately; keep internal errors out of the response.

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

## Keep learning

[API version groups](/docs/http/versioning). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#exception-handling-and-domain-mappings).
