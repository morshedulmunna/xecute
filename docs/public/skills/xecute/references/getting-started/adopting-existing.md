Enable and integrate one capability at a time, keeping the application compiling
and its existing HTTP tests passing. Installing a newer CLI does not rewrite app
files; this guide covers the explicit migration work.

## 1. Enable the runtime features

In a standalone app, merge the required flags into the existing runtime dependency:

```toml
[dependencies]
xecute = { version = "0.1", features = ["auth", "postgres", "jobs", "cache-redis", "cqrs"] }
```

Choose only the flags you use. For memory caching select `cache` rather than
`cache-redis`. Async lifecycle needs no extra feature. Keep the app's test-only
`testing` feature in dev-dependencies where appropriate.

For a generated workspace, update the shared dependency once:

```toml
# Workspace Cargo.toml
[workspace.dependencies]
xecute = { version = "0.1" }

# apps/my-api/Cargo.toml
[dependencies]
xecute = { workspace = true, features = ["auth", "cqrs"] }
```

Merge these fields with the existing manifests rather than replacing unrelated
dependencies/lints. Run `cargo generate-lockfile` and commit the resulting lockfile
with your application's dependency changes. `--runtime-path` affects generated
projects; it does not install or select a different CLI executable.

## 2. Move resource initialization to an async path

Generated `AppModule::container()` is synchronous. Add an async helper alongside
it using the existing `definitions()` registry:

```rust
pub async fn container_async() -> Result<Container, InjectError> {
    let container = ModuleGraph::new(Self::definitions()).build()?;
    container.initialize_async().await?;
    Ok(container)
}
```

Use `AppModule::container_async().await?` in server bootstrap, retain the returned
`Arc<Container>`, and call `router_with_container(container.clone())` after startup
completes. Router assembly remains synchronous once resources are ready.

Retain the generated configuration, shared policies, version/docs routers and
request draining. Refactor the existing post-initialization work into an async
block returning a Result; afterward await `container.shutdown_async()` on success
or failure, then propagate the results. The complete
[async lifecycle guide](../architecture/lifecycle.md) and runnable examples show
cleanup ownership. Account for early router/listener setup failures too.

Application tests calling the old sync helper need an async setup path when their
registrations include async factories or lifecycle hooks. Use
`TestingModule::compile_async().await` for isolated provider tests.

## 3. Register resources and enforce module visibility

A generated `#[module]` provides static type registrations. Build its definition,
then append configured provider definitions where needed:

```rust
let definition = UserModule::definition().provider(configured_provider);
```

Here `configured_provider` is a `ProviderDefinition` built from your validated
configuration. Include this definition once in `AppModule::definitions()`. For a
shared database/cache module, export the concrete provider type, import its owning
module in consumers, and include all module definitions in the graph.

Do not register a configured provider twice or re-register an imported service in
every module. Factories must declare the dependencies they resolve. The container
validates duplicates, cycles, module visibility and unsafe lifetime capture.

## 4. Adopt each capability

| Capability | App changes | Verification |
| --- | --- | --- |
| Lifecycle | Async factories and `on_init`/`on_shutdown` registration methods | Startup order, failed setup cleanup and shutdown |
| PostgreSQL | Pool config, migration provider, SQL repository, readiness | Run against dedicated PostgreSQL; verify commit/rollback |
| Authentication | Trusted verifier, controller/route guards and explicit policies | Missing/invalid JWT → 401; insufficient permissions → 403 |
| Jobs | Typed payload, bounded queue and handler; dependency-aware cleanup | Retry, overload handling and drain accepted work |
| Cache | Cache-aside reads and post-write invalidation | TTL, tenant/user key separation, storage failures |
| CQRS buses | Message/handler traits, bus registration and controller mapping | One command/query handler, event failures and pipeline ordering |

Read the detailed [PostgreSQL](../integrations/postgres.md),
[auth](../integrations/auth.md), [jobs](../integrations/jobs.md),
[cache](../integrations/cache.md), and [CQRS](../integrations/cqrs.md) guides.
Use compiled example code as a reference; adapt repository and policy logic to
your own application's contracts.

## 5. Preserve the application boundary

Keep body/query/path/form DTOs under `dto/`; controller methods import them and
map requests to service calls, commands or queries. Put SQL in repositories,
workflow logic in services/handlers, and configured resources in DI definitions.
Protect private endpoints explicitly before using authorization-sensitive cache
keys. Queues and event buses are process-local; use durable delivery/outbox
infrastructure for work that must survive a process crash.

## Verify before deployment

Run the app's existing tests, add tests for each enabled capability, and check
formatting/Clippy. Verify generated-app tests against your selected runtime version, not only
the library's unit tests. The [example catalog](../testing/examples.md) provides
working commands and external-service requirements.