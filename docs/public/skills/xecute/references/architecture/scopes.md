Choose how long an injected provider lives. Start with the generated singleton behavior; use request or transient scopes only when your application needs a different lifetime.

| Scope | Lifetime |
| --- | --- |
| Singleton (default) | One instance per application container, shared across requests |
| Request | One instance per request context, shared across nested dependencies |
| Transient | A new instance on every resolution |

```rust
use std::sync::Arc;
use xecute::{Injectable, controller, routes};

#[derive(Injectable)]
#[scope("request")]
struct RequestContext {
    #[inject(default)]
    state: std::sync::Mutex<Vec<String>>,
}

#[controller("/example-scope")]
#[derive(Injectable)]
#[scope("request")]
struct ExampleController {
    context: Arc<RequestContext>,
}

#[routes]
impl ExampleController {
    #[get("")]
    async fn get(&self) -> &'static str {
        let _ = &self.context;
        "request scope is active"
    }
}
```

Register both types in a module's providers. Generated `AppModule` uses
`Controller::scoped_router(container.clone())`, creating a request context before
controller middleware and extractors. Nested factories resolve against that same
context. Use a named parameter `context: xecute::Inject<RequestContext>` in a
controller method, or `Inject<T>` in a plain Axum handler, to resolve explicitly.
Plain routers need `xecute::with_container(router, Arc<Container>)`.
To resolve providers in app-level middleware, wrap the fully assembled middleware
pipeline with `with_container` as well. Matching nested boundaries reuse the same
request cache; mixing different container registries in one boundary returns a
safe 500 instead of resolving from the wrong application.
`Controller::router(Arc<Controller>)` remains available for explicitly supplied
controller instances; it cannot recreate their captured dependencies per request.

Use `container.request()` for jobs or tests requiring an explicit scope. Its
clones share a request cache; a second call creates an independent cache. Request
cache ownership is released at completion/cancellation, but any Arc retained by
application code keeps its value alive. `RequestContainer<'a>` remains a
compatibility alias for the owned `RequestScope`.

Startup rejects missing required registrations, duplicate providers/modules,
cycles, private module dependencies, and singleton-to-request dependencies even
through transient providers. Optional providers may be absent, but failures
constructing a registered optional provider propagate. Factories must declare
all dependencies; undeclared resolutions fail instead of risking recursive cache
deadlocks. Custom definitions can mark a declared dependency optional with
`.optional_dependency::<T>()`.

Concurrent resolutions initialize each singleton/request provider once. Factory
errors are cached for that scope; recover external connectivity inside a shared
client instead of expecting factory retries. `container.initialize_async()`
eagerly constructs asynchronous and synchronous singletons before generated
applications listen. Generated servers call `shutdown_async()` after the
listener stops so queues drain and lifecycle hooks run. A transient injected into
a singleton is created when that singleton is built, then retained by it.

## Keep learning

[Async startup and shutdown](lifecycle.md). Return to the [learning path](../index.md) whenever you need a different topic.