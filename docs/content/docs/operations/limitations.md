---
title: "Scope and limitations"
description: "Separate implemented capabilities from application extensions."
---

Check this page before planning around a capability. It separates generated behavior from features you must implement or configure in your application.

Xecute provides DI, module metadata, controller adapters, validation, HTTP
middleware, API documentation, and starter application infrastructure. It does
not currently generate database-backed repositories, complete CRUD operations,
login/session scaffolding, broker transports, or event sourcing. Optional
SQLx PostgreSQL integration supports lifecycle-managed pools, migrations, and native
transactions; see the [PostgreSQL guide](/docs/integrations/postgres).

The middleware API implements app, controller, and route scopes. `#[module]`
describes DI boundaries and does not automatically attach HTTP middleware.
Executable guards support application, controller, and route policies. Optional
JWT authentication and role/permission checks are documented in the
[authentication guide](/docs/integrations/auth). Optional [background jobs](/docs/integrations/jobs)
provide bounded process-local workers and interval schedules; [TTL caching](/docs/integrations/cache)
supports memory and Redis backends. Optional [CQRS buses](/docs/integrations/cqrs)
provide typed command/query dispatch, event fan-out and shared pipelines; direct
handler CQRS remains supported. Reserved attribute markers, including
interceptors, lifecycle attributes, and exception decorators, are not complete
execution systems. Async lifecycle
behavior is available through explicit provider registration methods:
`singleton_async`, `on_init`, and `on_shutdown`; await `initialize_async` before
serving and `shutdown_async` after request draining. See the
[runtime lifecycle guide](/docs/architecture/lifecycle).

Licensed under **Apache-2.0**.

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#scope-and-limitations).
