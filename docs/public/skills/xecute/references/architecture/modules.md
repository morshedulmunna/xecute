Learn how Xecute creates dependencies and shares them between features. First understand the controller → service → repository flow; then read registration, imports, and exports below.

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
| `request` | Reused within one HTTP request or an explicit request container |

Select a lifecycle with `#[scope("transient")]` or `#[scope("request")]` below
`#[derive(Injectable)]`. Singleton-to-request dependencies are rejected, including
transitive relationships. Generated routing creates request contexts automatically;
explicit `container.request()` contexts are available for jobs and tests.

## Keep learning

[Feature folders](feature-folders.md). Return to the [learning path](../index.md) whenever you need a different topic.