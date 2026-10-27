---
title: "CQRS buses and events"
description: "Register typed command/query handlers, event subscribers and shared pipelines."
---

Use typed buses when you want central command/query dispatch or event subscribers. Generated CQRS already works through direct handler injection; adding buses is a separate application choice.

Xecute's optional `cqrs` runtime feature provides typed `CommandBus`, `QueryBus`,
and `EventBus` registries. Commands/queries dispatch to one handler; events fan
out to zero or more subscribers. All three share ordered pipeline behaviors,
explicit dispatch metadata, ordinary DI module validation, and caller-owned async
execution.

Enable the feature in your application's Cargo.toml:

```toml
[dependencies]
xecute = { version = "0.1", features = ["cqrs"] }
```

The feature does not start a worker, open a database, require a broker, or spawn
hidden tasks. These are in-process application buses, not a distributed messaging
transport. CQRS does not require separate databases, event sourcing, or buses for
every small application. Existing direct-handler CQRS generation remains valid.

## Run the HTTP example

From the repository root:

```sh
cargo run -p xecute --example cqrs --features cqrs

# In another terminal:
curl -i -X POST http://127.0.0.1:3000/users \
  -H 'Content-Type: application/json' -d '{"name":"  Munna  "}'
curl -i http://127.0.0.1:3000/users
```

The [example](https://github.com/morshedulmunna/xecute/blob/main/xecute/examples/cqrs/main.rs) separates controller, DTO, entity,
repository, command/query handlers, event subscribers, module registrations and a
shared pipeline policy. The write handler normalizes/validates a name, persists a
user, then publishes `UserCreated` to audit and welcome subscribers. The query
handler reads the same repository. Welcome handling records a demo action; it does
not send email. Storage is bounded in-memory demonstration data, not durable user
or audit storage. The HTTP example is public; add the [authentication guards](/docs/integrations/auth)
when protecting a real API.

```text
POST → DTO → Controller → CommandBus → Behaviors → CreateUserHandler → Repository
                                                    └→ EventBus → Audit / Welcome
GET  → Controller → QueryBus → Behaviors → ListUsersHandler → Repository
```

## Typed commands and queries

A command declares its output and domain error; a handler supplies an async
implementation. Queries have the same shape using `Query` / `QueryHandler`.

```rust
use std::{convert::Infallible, sync::Arc};
use xecute::cqrs::{Command, CommandHandler, CommandBus, DispatchContext};

struct Add { value: u64 }
impl Command for Add {
    type Output = u64;
    type Error = Infallible;
}

struct AddHandler;
impl CommandHandler<Add> for AddHandler {
    async fn handle(&self, command: Add, _: DispatchContext)
        -> Result<u64, Infallible>
    {
        Ok(command.value + 1)
    }
}

let commands = CommandBus::builder()
    .handler::<Add, _>(Arc::new(AddHandler))
    .build()?;
let result = commands.execute(Add { value: 41 }).await?;
assert_eq!(result, 42);
```

For closures/functions, use `.handler_fn::<Add, _, _>(|command, context| async move
{ ... })`. Messages must be `Send + Sync + 'static`; outputs/errors must be
`Send + 'static`. A message does not need `Clone`. A concrete Rust message type
identifies its handler; strings and serialized type names are not routing keys.
Command and query registries are separate. Their traits express intent; the query
trait does not technically prevent application code from writing to a database.

Exactly one handler per command/query type is allowed. Duplicates fail `build()`
or `into_provider()` instead of silently replacing a handler. Dispatch of an
unregistered message returns `BusError::MissingHandler` before behaviors run.
Registries are immutable after construction and safe to clone/share concurrently.
A bus clone shares the same registered handler instances.

### Error mapping

`execute` returns `Result<Output, DispatchError<YourError>>`:

- `DispatchError::Handler(error)` preserves the handler's typed domain error.
- `DispatchError::Bus(error)` represents registration/routing/pipeline failure.

Map these deliberately at your HTTP boundary. For example, a known duplicate
resource can become 409, a validation failure 422, and an unexpected bus failure
500. Do not serialize arbitrary domain errors or call their Display output in a
public HTTP response. Bus errors contain static codes/type names, but internal
configuration/type details still should not be exposed as client diagnostics.
There is no automatic HTTP status mapping for arbitrary CQRS errors.

## Dependency injection and module boundaries

Register handlers with the ordinary DI container, then bind them to a bus:

```rust
use xecute::{ModuleDefinition, Scope};
use xecute::cqrs::CommandBus;

let commands = CommandBus::builder()
    .handler_provider::<Add, AddHandler>()
    .into_provider(Scope::Singleton)?;

// Register AddHandler using its Provider definition, or a concrete value:
let module = ModuleDefinition::new("math")
    .provider(xecute::ProviderDefinition::value(AddHandler))
    .provider(commands);
```

Build `ModuleGraph` as usual and initialize the container before serving. Resolve
or inject `Arc<CommandBus>` and call it from a controller/service. The runnable
example uses `#[derive(Injectable)]` for its controller and handlers. Configured
buses are registered with their builder's `into_provider`, not by putting bare
`CommandBus` / `QueryBus` / `EventBus` types into `#[module(providers = [...])]`.
Those types cannot infer application-specific registrations on their own.

`handler_provider` and `behavior_provider` add declared dependencies to the bus's
provider definition. Existing missing-provider, module import/export, cycle and
scope validation applies. A handler or behavior in another module must be exported
and its owning module imported by the bus's module. Missing providers fail graph
construction; resolution/factory errors fail bus construction/startup.

Bindings resolve handlers **once when the bus is constructed**, not on each
execute. The resulting bus retains handler Arcs, not a `Container`; this avoids
container → singleton cache → bus → container reference cycles. Singleton buses
can retain only compatible provider scopes. For request-scoped handlers, register
the bus with `Scope::Request` and resolve it through the active `RequestScope` or
`scoped_router` / `Inject<CommandBus>`. Different request buses then retain their
own request handler instances while sharing permitted singleton dependencies.
A transient handler bound into a singleton bus is retained for that bus's lifetime;
use a request/transient bus when new handler instances per resolution are needed.

For manual binding, `.build_with(&container)` uses an existing root container;
`.build_in(&request_scope)` uses one request scope. `.build()` is for direct
Arc/closure registrations and reports `ProviderContextRequired` if a DI-backed
registration has no binding context. Direct Arc/closure captures are app-owned
objects; their dependency lifetimes cannot be inferred or validated by the bus.

Async handler dependencies from step 1 work normally: call `initialize_async()`
before routing traffic. Stop in-flight dispatches before calling container
shutdown and closing their database/cache dependencies. Buses do not add a
separate lifecycle hook or detach handler work. A handler Arc can outlive its
container when retained elsewhere; do not execute it after its resources close.

The legacy `Injector` can resolve an explicitly registered `.value(bus)`; it does
not synthesize an empty bus. Native `Container` / `ModuleGraph` is preferred for
configured buses and provider lifetimes.

## Explicit dispatch context

```rust
#[derive(Clone)]
struct CorrelationId(String);

let context = DispatchContext::default()
    .with_value(CorrelationId("request-123".into()));
let result = commands.execute_with(Add { value: 41 }, context).await?;
```

`context.value::<CorrelationId>()` reads typed metadata. Context clones share Arc
values but have independent maps, so replacing a value on a cloned context does
not modify another dispatch's metadata map. Values with interior mutability still
follow their own synchronization rules. Context Debug output reports only value
count and does not print payloads.

HTTP principals, request IDs, transactions and cancellation tokens are not
implicitly propagated. Extract the verified `AuthenticatedUser` or server-issued
request ID, then pass the required trusted values explicitly. Do not put raw
bearer tokens or an entire HTTP request into dispatch context. Reusable command
handlers must enforce business invariants for non-HTTP callers too; controller
DTO validation alone does not protect job/CLI/event entry points.

## Shared pipeline behaviors

A `Behavior` wraps a continuation for a registered command/query or complete event
publication. Register outer-to-inner order with `.behavior(Arc::new(...))` or
`.behavior_provider::<YourBehavior>()`.

```rust
use xecute::cqrs::{Behavior, BusError, DispatchFuture, Next};

struct RequireCorrelation;
impl Behavior for RequireCorrelation {
    fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async move {
            if next.context().value::<CorrelationId>().is_none() {
                return Err(BusError::Rejected("correlation_required"));
            }
            let response = next.run().await?;
            let succeeded = response.is_success();
            // Record duration/result metadata without logging submitted payloads.
            let _ = succeeded;
            Ok(response)
        })
    }
}
```

`Next` exposes `info()` (Command/Query/Event and concrete type),
`message::<YourMessage>()`, and `context()`. `next.run()` consumes the continuation,
so downstream execution is invoked at most once through that continuation.
Behaviors can validate/authorize, reject, inspect a typed payload before
continuing, or inspect the result afterward. This order is verified:

```text
first before → second before → handler → second after → first after
```

A handler's domain failure is still a completed pipeline response;
`response.is_success()` is false. For typed inspection, use
`response.value::<Result<Output, DomainError>>()`. Event responses instead contain
`EventReport`, and success is false if any subscriber failed. A pipeline rejection
or panic returns a `BusError` without a normal response. As with ordinary async
code, code after `next.run().await?` does not run when that await returns an error;
match the result explicitly when cleanup/logging must cover failures.

A trusted behavior may intentionally short-circuit with
`DispatchResponse::from_result::<Output, DomainError>(Ok(cached_value))` or
`from_event_report(report)`. The final bus checks the concrete response contract;
returning the wrong result type fails with `InvalidResponse`. Only implement
caching for query types/authorization scopes where it is correct. A typed bus
pipeline is not an HTTP guard and does not automatically apply JWT policies.

Behaviors have no automatic retry or transaction wrapper. Retrying an entire
command or a whole event publication can repeat side effects. A SQL transaction
must be owned/passed explicitly by application handlers or transaction-aware
adapters; there is no implicit transaction propagation through DispatchContext.

## Event fan-out and observable failures

```rust
use std::sync::Arc;
use xecute::cqrs::{Event, EventBus, EventError};

struct UserCreated { id: String }
impl Event for UserCreated {}

let events = EventBus::builder()
    .handler_fn::<UserCreated, _, _>("audit", |event, _| async move {
        let _ = event.id.as_str();
        Ok::<(), EventError>(())
    })
    .handler_fn::<UserCreated, _, _>("welcome", |_, _| async move {
        Err(EventError::Handler("email_unavailable"))
    })
    .build()?;
let report = events.publish(UserCreated { id: "user-42".into() }).await?;
assert_eq!(report.attempted, 2);
assert_eq!(report.succeeded, 1);
assert!(!report.is_success());
```

For structs, implement `EventHandler<E>` and use `.handler::<E, H>(Arc<H>)` or
`.handler_provider::<E, H>()`. Each subscriber receives an `Arc<E>` and an explicit
context; one publication shares the same event payload across subscribers. No
`Clone` implementation is required on the event. Subscriber names must be
nonblank and unique **within one event type**. Direct typed subscribers use the
concrete handler type name; closures use their explicit name. Duplicate names are
startup/build errors.

Subscribers run sequentially in registration order. A returned `EventError` or
unwinding panic is collected in `EventReport.failures`, and later subscribers
still run. Reports contain attempted/succeeded counts and ordered named failures.
A publication with no subscribers is a successful empty report. Behaviors wrap
the whole publication; a rejecting behavior can prevent fan-out entirely.

`publish(...).await` completes only after subscriber work finishes. It does not
mean queued delivery and does not automatically create a background job. There
are no implicit subscriber retries, deadlines, dead-letter records or persistent
subscriptions. If delivery must be asynchronous, explicitly enqueue work using
[jobs](/docs/integrations/jobs), and remember that step 4's queue is also process-local.

Ordinary unwinding panics during handler/behavior execution are converted to safe
bus/event failure results. The default Rust panic hook still runs and may log the
panic text; do not panic with secrets. `panic = "abort"`, process failures and
panicking destructors are not recoverable delivery guarantees. Recovery does not
undo state changes or repair a poisoned/mutated application resource.

## Write consistency, workflows, and sagas

Publish domain events deliberately after successful repository persistence. A
failed subscriber does **not** roll back a committed write or earlier subscriber
side effects. In the example, the user remains created even if event delivery is
incomplete; delivery failure is reported separately instead of returning a false
"creation rolled back" result.

A crash between commit and publish can lose notifications. Use a transactional
outbox in PostgreSQL plus a durable delivery worker when notifications must survive
that gap. Neither the in-process EventBus nor the process-local JobQueue provides
an outbox, exactly-once execution or event sourcing. Persist idempotency state and
use stable event/operation IDs when retrying side effects.

An event subscriber can coordinate a follow-up command through an explicitly
supplied command bus. Keep dependencies acyclic: if a command handler depends on
EventBus and an event subscriber depends on the same CommandBus containing that
handler, ModuleGraph correctly rejects the circular dependency. Use a separate
newtype-wrapped follow-up command registry, explicit orchestration, or a durable
outbox boundary rather than hiding the cycle with a retained Container.
Long-running persisted sagas, compensation state machines, broker transports,
aggregate event stores and replay are not implemented by this feature.

## Cancellation and deadlines

Execution futures remain owned by the caller. Cancelling an execute drops its
current handler/pipeline future; cancelling event publication leaves completed
subscriber side effects intact and drops the current subscriber before later
subscribers run. No internal task continues work in the background. Application
handlers can still spawn their own tasks, so their ownership is your responsibility.

There is no automatic deadline. Use your HTTP request timeout or an explicit
`tokio::time::timeout` when appropriate. Timeout/cancellation is not a transaction
rollback guarantee for already completed database/remote operations. Apply the
same idempotency and cleanup rules as other async application code.

## Adopt in generated MVC/CQRS applications

`xecute g resource user` still emits direct
`Controller → Command/Query Handler → Repository` calls in CQRS mode. Existing
features are not rewritten automatically, and the CLI currently does not enable
this runtime feature or create bus registries by default. Adoption is explicit:

1. Enable the runtime's `cqrs` feature, using the local runtime path until release.
2. Implement `Command`/`Query` and their handler traits for existing command/query
   types. A trait handler can delegate to your existing `.execute(...)` method.
3. Add handler-provider registrations to the configured buses and add their
   `into_provider(...)` definitions to the owning ModuleDefinition. Export/import
   dependencies as required.
4. Inject `Arc<CommandBus>` / `Arc<QueryBus>` into the controller and map
   `DispatchError` to safe HTTP/domain responses.
5. Register event subscribers and explicitly publish at the chosen write boundary.

Use the complete example's module/controller code as the reference. MVC services
may also call buses; choosing a bus does not require moving every operation into
CQRS. Keep HTTP DTOs under `dto/` and domain/application messages under
`commands/`, `queries/` and `events/`.

## Tests

```sh
cargo test -p xecute --features cqrs,testing --test cqrs --example cqrs
cargo check -p xecute --no-default-features
```

Tests cover typed outputs/domain errors, duplicate/missing handlers, pipeline
ordering/short circuits/response validation, command/query/event metadata, event
fan-out and panic continuation, empty/unrelated subscriptions, DI visibility,
request scope isolation, provider overrides, invalid dependency cycles, absence of
container reference cycles, concurrent contexts, caller cancellation, and the HTTP
controller → buses → handlers → repository → subscribers example.

Reference: [NestJS CQRS concepts](https://docs.nestjs.com/recipes/cqrs) and
[FutureExt unwind handling](https://docs.rs/futures-util/latest/futures_util/future/trait.FutureExt.html).

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/xecute/docs/cqrs.md).
