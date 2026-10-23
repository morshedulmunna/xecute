# xmacros

Implementation crate for the `xecute` runtime and Axum route registration.

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
