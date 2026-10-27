# xmacros

Procedural macros for Xecute's application architecture tools and Axum route
registration. Xecute adds tools and macros to Axum applications; Axum remains
the underlying web framework.

```rust
use std::sync::Arc;
use xecute::{Container, injectable};

#[injectable]
struct Service { dependency: Arc<Dependency> }
#[injectable]
struct Dependency;

let container = Container::builder()
    .injectable::<Dependency>()
    .injectable::<Service>()
    .build()?;
# Ok::<(), xecute::InjectError>(())
```

The crates follow semantic versioning. Until 1.0, minor releases may include
breaking API changes; after 1.0, breaking changes require a major release.


## Executable guards and authorization

Place `#[guard(check_access)]` below `#[controller("/prefix")]` on a struct,
below `#[routes]` on an impl, or on a routed handler inside that impl. Guards
are async functions returning `Result<xecute::middleware::Request, E>`; the first
error stops request extraction and handler execution. Guards work in both shared
and scoped routers. Empty guard lists and policies on non-route helpers are errors.

With Xecute's `auth` feature, `#[roles("admin")]` and
`#[permissions("users:read")]` require all declared values from the verified
principal. Run authentication first; policies combine across controller and route
scopes. See the [runtime authentication guide](../xecute/docs/auth.md) for complete
examples, middleware ordering, DI setup, and limitations. Use matching local
runtime/macros during development; these APIs are not in prior published releases.
