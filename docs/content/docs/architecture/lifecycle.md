---
title: "Async startup and shutdown"
description: "Initialize resources in dependency order and clean up after draining."
---

> **Source checkout feature:** this API is implemented locally and is not in the published runtime. Enable/configure it explicitly; generators do not wire it automatically.

Use lifecycle hooks when resources must connect before requests start and close after work drains. Follow the async startup path below; attribute markers alone do not execute hooks.

Register asynchronous resources with `ProviderDefinition::singleton_async`, then
await `container.initialize_async()` before binding or serving HTTP. Factories
receive an owned `Container`, so they can await connection setup and resolve
**declared** dependencies after await points. Dependencies and their startup hooks
finish before dependent factories run. Existing synchronous factories and derived
providers can depend on these initialized resources without changing their APIs.

```rust
use xecute::{Container, InjectError, ProviderDefinition};

struct Connection;
impl Connection {
    async fn connect() -> Result<Self, InjectError> { Ok(Self) }
    async fn close(&self) -> Result<(), InjectError> { Ok(()) }
}

async fn example() -> Result<(), InjectError> {
let container = Container::builder()
    .provider(
        ProviderDefinition::singleton_async::<Connection, _, _>(vec![], |_| async {
            Connection::connect().await
        })
        .on_init::<Connection, _, _>(|connection| async move {
            // Optional readiness checks or resource warm-up.
            let _ = connection;
            Ok(())
        })
        .on_shutdown::<Connection, _, _>(|connection| async move {
            connection.close().await
        }),
    )
    .build()?;

container.initialize_async().await?;
let connection = container.resolve::<Connection>()?;
// Serve requests, then stop accepting work and drain outstanding requests.
container.shutdown_async().await?;
let _ = connection;
Ok(())
}
```

Use `.on_init::<YourType, _, _>(...)` and `.on_shutdown::<YourType, _, _>(...)`
on a synchronous provider definition as well, including `YourType::definition()`.
The hook type must match the registered provider type. Hooks are singleton-only;
request and transient factories keep their existing resolution behavior. Sync
`initialize()` reports `AsyncInitializationRequired` when async factories or
hooks need startup. It remains usable after async startup completes.

Clones share lifecycle state. Concurrent initialization runs factories and hooks
once; shutdown runs cleanup in reverse dependency order. Startup failures clean
up already constructed resources, including a resource whose init hook failed,
then return and cache the original startup error. Cleanup continues if a hook
returns an error; explicit shutdown returns the first cleanup error. A factory
that fails before returning owns cleanup of its partially constructed resources.

Shutdown is explicit: dropping a container does not execute async hooks. After
startup cancellation, call `shutdown_async()` to clean constructed resources;
startup cannot be retried on that container. Cancelled shutdown can be resumed,
but the interrupted hook is not retried. Keep cleanup hooks bounded and avoid
cancelling them when possible. Hooks and factories must not call lifecycle
methods recursively on the same container. Existing `Arc` references remain
accessible after cleanup; finish application work before shutting down.

For module-based applications, attach definitions with
`ModuleDefinition::provider(...)`, then build with `ModuleGraph` as usual.
For generated applications using async resources, build the container from
`AppModule::definitions()` and await initialization before calling
`AppModule::router_with_container(container.clone())`; the synchronous
`AppModule::container()` helper cannot initialize async resources. Retain a
container clone for cleanup after graceful request draining, including startup
errors after initialization (for example, a failed listener bind).

With the `testing` feature, use `TestingModule::compile_async().await` and call
`shutdown_async()` when the test finishes. Provider overrides replace factories
and hooks together. Existing lifecycle attribute markers such as
`#[on_module_init]` remain placeholders; use the executable registration methods
above.

Run `cargo run -p xecute --example async_lifecycle` for a complete example with
async connection setup, a synchronous dependent service, and explicit cleanup.

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/xecute/README.md#async-providers-and-application-lifecycle).
