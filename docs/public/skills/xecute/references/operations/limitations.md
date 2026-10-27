Check this page before planning around a capability. It separates generated behavior from features you must implement or configure in your application.

Xecute provides DI, module metadata, controller adapters, validation, HTTP
middleware, API documentation, and starter application infrastructure. It does
not currently generate database-backed repositories, complete CRUD operations,
login/session scaffolding, broker transports, or event sourcing. Optional
SQLx PostgreSQL integration supports lifecycle-managed pools, migrations, and native
transactions; see the [PostgreSQL guide](../integrations/postgres.md).

The middleware API implements app, controller, and route scopes. `#[module]`
describes DI boundaries and does not automatically attach HTTP middleware.
Executable guards support application, controller, and route policies. Optional
JWT authentication and role/permission checks are documented in the
[authentication guide](../integrations/auth.md). Optional [background jobs](../integrations/jobs.md)
provide bounded process-local workers and interval schedules; [TTL caching](../integrations/cache.md)
supports memory and Redis backends. Optional [CQRS buses](../integrations/cqrs.md)
provide typed command/query dispatch, event fan-out and shared pipelines; direct
handler CQRS remains supported. Reserved attribute markers, including
interceptors, lifecycle attributes, and exception decorators, are not complete
execution systems. Async lifecycle
behavior is available through explicit provider registration methods:
`singleton_async`, `on_init`, and `on_shutdown`; await `initialize_async` before
serving and `shutdown_async` after request draining. See the
[runtime lifecycle guide](../architecture/lifecycle.md).

Licensed under **Apache-2.0**.

## Keep learning

[Choose your next guide](../index.md). Return to the [learning path](../index.md) whenever you need a different topic.