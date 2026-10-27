# Coming from NestJS

Xecute uses Rust and Axum with explicit registration. Several architectural ideas
will be familiar, but NestJS decorator names do not imply matching runtime APIs.

| NestJS concept | Xecute implementation |
| --- | --- |
| Module imports/exports | `#[module(imports = [...], providers = [...], exports = [...])]` plus all definitions in `ModuleGraph` |
| Injectable provider | `#[derive(Injectable)]` or `#[injectable]` on a named-field struct |
| Constructor injection | Typed fields such as `Arc<UserRepository>` |
| Controller and HTTP methods | `#[controller]`, `#[routes]`, `#[get]`, `#[post]` and other verbs |
| Body/query/path inputs | `#[body]`, `#[query]`, singular `#[param]`; other Axum extractors directly |
| Validation pipe | Automatic JSON/query DTO decoding plus `validator` validation |
| Guards / UseGuards | `#[guard(...)]` on a module, controller, or route; async functions return `Result<Request, E>` |
| Authentication/authorization | Optional `auth` verifier, `AuthenticatedUser`, roles/permissions |
| Request scope | `Scope::Request`, `RequestScope` and scoped controller routers |
| Lifecycle | `singleton_async`, `on_init`, `on_shutdown`, explicit async startup/shutdown |
| CQRS buses | Optional typed `CommandBus`, `QueryBus`, `EventBus` and `Behavior` |
| Queues | Optional bounded in-process `JobQueue<T>`; not BullMQ/durable broker storage |
| Cache | Explicit TTL cache API; memory/LRU or Redis; no automatic HTTP interceptor |

## Choose the generated architecture

MVC uses `Controller → Service → Repository`. CQRS uses
`Controller → Command/Query Handler → Repository`. A focused domain service can be
used by handlers when reusable logic needs it; a forwarding service is not
mandatory. Buses are an additional explicit choice, not a requirement for CQRS.

## Important differences

Module imports establish visibility but do not auto-discover definitions or mount
routes. The CLI registers resource definitions and controller routers through
markers in `AppModule`; manual modules need both kinds of registration.

Guards follow nested Axum scopes: app middleware/guards wrap controller
middleware/guards, then route middleware/guards and extraction. This is documented
Xecute ordering, not a promise of NestJS's universal request lifecycle.

Lifecycle attribute markers such as `#[on_module_init]` remain placeholders.
Use the provider registration methods described in the lifecycle guide. There is
no built-in dynamic module registration convention, full HTTP interceptor system,
ORM entity decorators, session/login scaffold or persisted saga engine.

Rust APIs use concrete types and typed errors rather than reflection-based token
lookup. `Command` and `Query` traits declare Output/Error types; event subscribers
receive a shared `Arc<E>`. Explicit configuration and feature flags make startup
and deployment behavior inspectable.

Read [feature setup](/docs/getting-started/status) before enabling integrations
and the [adoption guide](/docs/getting-started/adopting-existing) when updating an
existing generated application.
