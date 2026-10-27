//! Typed, in-process CQRS dispatch. Immutable registrations bind DI handlers without
//! retaining containers. Futures are caller-owned, with explicit context, ordered
//! pipelines and observable event failures. No transport or durable delivery is implied.

use crate::{Container, InjectError, ProviderDefinition, ProviderKey, RequestScope, Scope};
use futures_util::FutureExt;
use std::{
    any::{Any, TypeId, type_name},
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
    panic::AssertUnwindSafe,
    pin::Pin,
    sync::Arc,
};

type DynMessage = dyn Any + Send + Sync;
type Handler = dyn Fn(Box<DynMessage>, DispatchContext) -> DispatchFuture<'static> + Send + Sync;
type Binder<T> = dyn Fn(Option<&Container>) -> Result<T, BusError> + Send + Sync;

/// Registration, routing, policy, or execution infrastructure failure.
/// Messages identify types/static diagnostic codes, never request payloads.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BusError {
    /// No handler is registered for a command/query type.
    MissingHandler(&'static str),
    /// More than one handler was registered for the same command/query type.
    DuplicateHandler(&'static str),
    /// An event subscriber name was registered twice for the same event type.
    DuplicateSubscriber {
        /// Event type name.
        event: &'static str,
        /// Duplicate subscriber name.
        handler: &'static str,
    },
    /// A provider-backed registration needs an explicit DI binding context.
    ProviderContextRequired(&'static str),
    /// Existing DI validation or resolution failure.
    Provider(InjectError),
    /// A middleware returned a response incompatible with the message's typed contract.
    InvalidResponse(&'static str),
    /// A pipeline rejected dispatch with a safe static diagnostic code.
    Rejected(&'static str),
    /// A handler/pipeline panicked. The ordinary Rust panic hook still applies.
    Panicked(&'static str),
    /// Invalid subscriber registration metadata.
    InvalidRegistration(&'static str),
}
impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CQRS bus: {self:?}")
    }
}
impl std::error::Error for BusError {}

/// Keep application handler errors distinct from infrastructure/pipeline errors.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DispatchError<E> {
    /// Typed domain/application error returned by a handler.
    Handler(E),
    /// Registration, dispatch, or pipeline failure.
    Bus(BusError),
}
impl<E: fmt::Display> fmt::Display for DispatchError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handler(error) => write!(f, "CQRS handler: {error}"),
            Self::Bus(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DispatchError<E> {}

/// Which kind of operation a shared pipeline is executing.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MessageKind {
    /// Exactly one command handler.
    Command,
    /// Exactly one query handler.
    Query,
    /// Zero or more ordered event subscribers.
    Event,
}
/// Payload-free dispatch metadata for policies and instrumentation.
#[derive(Debug, Clone, Copy)]
pub struct MessageInfo {
    /// Operation category.
    pub kind: MessageKind,
    /// Concrete Rust message type name.
    pub name: &'static str,
    id: TypeId,
}
impl MessageInfo {
    fn of<M: 'static>(kind: MessageKind) -> Self {
        Self {
            kind,
            name: type_name::<M>(),
            id: TypeId::of::<M>(),
        }
    }
    /// Check the exact Rust message type without matching diagnostic strings.
    pub fn is<M: 'static>(self) -> bool {
        self.id == TypeId::of::<M>()
    }
}

/// Explicit per-dispatch typed metadata. Clones share values, not mutable maps.
/// No HTTP request, principal, transaction or DI context is propagated implicitly.
#[derive(Clone, Default)]
pub struct DispatchContext {
    values: HashMap<TypeId, Arc<DynMessage>>,
}
impl fmt::Debug for DispatchContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DispatchContext")
            .field("value_count", &self.values.len())
            .finish()
    }
}
impl DispatchContext {
    /// Add/replace a typed metadata value, such as a correlation ID or verified principal.
    #[must_use]
    pub fn with_value<T: Send + Sync + 'static>(mut self, value: T) -> Self {
        self.values.insert(TypeId::of::<T>(), Arc::new(value));
        self
    }
    /// Borrow previously supplied typed metadata.
    pub fn value<T: Send + Sync + 'static>(&self) -> Option<&T> {
        self.values
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref())
    }
}

/// Type-erased response for shared pipelines. Request responses contain
/// Result<Output, Error>; event publication contains an EventReport.
pub struct DispatchResponse {
    value: Box<dyn Any + Send>,
    success: bool,
}
impl fmt::Debug for DispatchResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DispatchResponse")
            .field("success", &self.success)
            .finish()
    }
}
impl DispatchResponse {
    /// Create a typed request response (also useful for deliberate pipeline short circuits).
    pub fn from_result<T: Send + 'static, E: Send + 'static>(result: Result<T, E>) -> Self {
        Self {
            success: result.is_ok(),
            value: Box::new(result),
        }
    }
    /// Create an event response whose success reflects every subscriber outcome.
    pub fn from_event_report(report: EventReport) -> Self {
        Self {
            success: report.failures.is_empty(),
            value: Box::new(report),
        }
    }
    /// Whether the handler's domain result (or complete fan-out report) succeeded.
    pub fn is_success(&self) -> bool {
        self.success
    }
    /// Inspect a known typed result without serializing application data.
    pub fn value<T: 'static>(&self) -> Option<&T> {
        self.value.downcast_ref()
    }
    fn take<T: Send + 'static>(self, name: &'static str) -> Result<T, BusError> {
        self.value
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_| BusError::InvalidResponse(name))
    }
}
/// Object-safe async continuation/result type used by shared behaviors.
pub type DispatchFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DispatchResponse, BusError>> + Send + 'a>>;

/// One shared validation, authorization, logging, caching, or instrumentation behavior.
/// Behaviors run in registration order; consuming Next permits one downstream call.
pub trait Behavior: Send + Sync + 'static {
    /// Inspect metadata/payload, reject, or await next.run() and inspect the result.
    fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a>;
}
/// Owned continuation for exactly one message through the remaining pipeline.
pub struct Next<'a> {
    info: MessageInfo,
    message: Box<DynMessage>,
    context: DispatchContext,
    behaviors: &'a [Arc<dyn Behavior>],
    handler: &'a Arc<Handler>,
}
impl<'a> Next<'a> {
    /// Inspect payload-free operation metadata.
    pub fn info(&self) -> MessageInfo {
        self.info
    }
    /// Inspect a typed payload before moving this continuation into run().
    pub fn message<M: Send + Sync + 'static>(&self) -> Option<&M> {
        self.message.downcast_ref()
    }
    /// Borrow explicit dispatch metadata.
    pub fn context(&self) -> &DispatchContext {
        &self.context
    }
    /// Continue downstream once; returning without calling run short-circuits dispatch.
    pub fn run(self) -> DispatchFuture<'a> {
        if let Some((behavior, remaining)) = self.behaviors.split_first() {
            behavior.handle(Self {
                behaviors: remaining,
                ..self
            })
        } else {
            (self.handler)(self.message, self.context)
        }
    }
}

async fn dispatch(
    info: MessageInfo,
    message: Box<DynMessage>,
    context: DispatchContext,
    behaviors: &[Arc<dyn Behavior>],
    handler: &Arc<Handler>,
) -> Result<DispatchResponse, BusError> {
    AssertUnwindSafe(async {
        Next {
            info,
            message,
            context,
            behaviors,
            handler,
        }
        .run()
        .await
    })
    .catch_unwind()
    .await
    .map_err(|_| BusError::Panicked(info.name))?
}

struct HandlerSpec {
    info: MessageInfo,
    bind: Arc<Binder<Arc<Handler>>>,
}
struct BehaviorSpec {
    bind: Arc<Binder<Arc<dyn Behavior>>>,
}
#[derive(Default)]
struct RegistryBuilder {
    handlers: Vec<HandlerSpec>,
    behaviors: Vec<BehaviorSpec>,
    dependencies: Vec<ProviderKey>,
}
#[derive(Default)]
struct Registry {
    handlers: HashMap<TypeId, Arc<Handler>>,
    behaviors: Vec<Arc<dyn Behavior>>,
}
impl RegistryBuilder {
    fn validate(&self) -> Result<(), BusError> {
        let mut seen = HashSet::new();
        for spec in &self.handlers {
            if !seen.insert(spec.info.id) {
                return Err(BusError::DuplicateHandler(spec.info.name));
            }
        }
        Ok(())
    }
    fn bind(&self, container: Option<&Container>) -> Result<Registry, BusError> {
        self.validate()?;
        let mut handlers = HashMap::new();
        for spec in &self.handlers {
            handlers.insert(spec.info.id, (spec.bind)(container)?);
        }
        let behaviors = self
            .behaviors
            .iter()
            .map(|spec| (spec.bind)(container))
            .collect::<Result<_, _>>()?;
        Ok(Registry {
            handlers,
            behaviors,
        })
    }
    fn behavior(&mut self, behavior: Arc<dyn Behavior>) {
        self.behaviors.push(BehaviorSpec {
            bind: Arc::new(move |_| Ok(behavior.clone())),
        });
    }
    fn behavior_provider<B: Behavior>(&mut self) {
        self.dependencies.push(ProviderKey::of::<B>());
        self.behaviors.push(BehaviorSpec {
            bind: Arc::new(|container| {
                let container =
                    container.ok_or(BusError::ProviderContextRequired(type_name::<B>()))?;
                container
                    .resolve::<B>()
                    .map(|value| value as Arc<dyn Behavior>)
                    .map_err(BusError::Provider)
            }),
        });
    }
}
fn provider_error(error: BusError) -> InjectError {
    match error {
        BusError::Provider(error) => error,
        _ => InjectError::ConstructionFailed("CQRS registration binding failed"),
    }
}
fn unique_dependencies(dependencies: &[ProviderKey]) -> Vec<ProviderKey> {
    let mut seen = HashSet::new();
    dependencies
        .iter()
        .copied()
        .filter(|key| seen.insert(*key))
        .collect()
}

macro_rules! request_bus {
    ($message:ident, $handler:ident, $bus:ident, $builder:ident, $kind:ident) => {
        /// Typed request contract: exactly one handler and a statically known output/error.
        pub trait $message: Send + Sync + 'static {
            /// Successful handler output.
            type Output: Send + 'static;
            /// Domain/application failure retained by typed dispatch.
            type Error: Send + 'static;
        }
        /// Async handler implementation for one typed request contract.
        pub trait $handler<M: $message>: Send + Sync + 'static {
            /// Execute using explicit request metadata. The returned future must be Send.
            fn handle(
                &self,
                message: M,
                context: DispatchContext,
            ) -> impl Future<Output = Result<M::Output, M::Error>> + Send;
        }
        /// Cloneable immutable typed dispatch registry and its ordered behaviors.
        #[derive(Clone, Default)]
        pub struct $bus {
            registry: Arc<Registry>,
        }
        impl crate::Injectable for $bus {
            fn inject(injector: &crate::Injector) -> Result<Self, InjectError> {
                injector.value_of::<Self>()
            }
        }
        impl $bus {
            /// Start explicit handler/behavior registration.
            pub fn builder() -> $builder {
                $builder::default()
            }
            /// Execute one typed request with empty dispatch metadata.
            pub async fn execute<M: $message>(
                &self,
                message: M,
            ) -> Result<M::Output, DispatchError<M::Error>> {
                self.execute_with(message, DispatchContext::default()).await
            }
            /// Execute with explicit metadata. Futures remain owned by the caller;
            /// cancellation drops the handler/pipeline future without spawning hidden work.
            pub async fn execute_with<M: $message>(
                &self,
                message: M,
                context: DispatchContext,
            ) -> Result<M::Output, DispatchError<M::Error>> {
                let info = MessageInfo::of::<M>(MessageKind::$kind);
                let handler = self
                    .registry
                    .handlers
                    .get(&info.id)
                    .ok_or(DispatchError::Bus(BusError::MissingHandler(info.name)))?;
                let result = dispatch(
                    info,
                    Box::new(message),
                    context,
                    &self.registry.behaviors,
                    handler,
                )
                .await
                .map_err(DispatchError::Bus)?;
                result
                    .take::<Result<M::Output, M::Error>>(info.name)
                    .map_err(DispatchError::Bus)?
                    .map_err(DispatchError::Handler)
            }
        }
        /// Registration builder. Duplicate message handlers fail before dispatch/startup.
        #[derive(Default)]
        pub struct $builder {
            registry: RegistryBuilder,
        }
        impl $builder {
            /// Register a closure/function as the sole handler for a message type.
            #[must_use]
            pub fn handler_fn<M, F, Fut>(mut self, handler: F) -> Self
            where
                M: $message,
                F: Fn(M, DispatchContext) -> Fut + Send + Sync + 'static,
                Fut: Future<Output = Result<M::Output, M::Error>> + Send + 'static,
            {
                let handler = Arc::new(handler);
                let erased: Arc<Handler> = Arc::new(move |message, context| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let message = *message
                            .downcast::<M>()
                            .map_err(|_| BusError::InvalidResponse(type_name::<M>()))?;
                        Ok(DispatchResponse::from_result(
                            handler(message, context).await,
                        ))
                    })
                });
                self.registry.handlers.push(HandlerSpec {
                    info: MessageInfo::of::<M>(MessageKind::$kind),
                    bind: Arc::new(move |_| Ok(erased.clone())),
                });
                self
            }
            /// Register an existing Arc-backed typed handler.
            #[must_use]
            pub fn handler<M: $message, H: $handler<M>>(self, handler: Arc<H>) -> Self {
                self.handler_fn::<M, _, _>(move |message, context| {
                    let handler = handler.clone();
                    async move { handler.handle(message, context).await }
                })
            }
            /// Bind a registered DI provider when this bus is constructed, not per dispatch.
            /// Handler dependencies and scopes participate in normal ModuleGraph validation.
            #[must_use]
            pub fn handler_provider<M: $message, H: $handler<M>>(mut self) -> Self {
                self.registry.dependencies.push(ProviderKey::of::<H>());
                self.registry.handlers.push(HandlerSpec {
                    info: MessageInfo::of::<M>(MessageKind::$kind),
                    bind: Arc::new(|container| {
                        let handler = container
                            .ok_or(BusError::ProviderContextRequired(type_name::<H>()))?
                            .resolve::<H>()
                            .map_err(BusError::Provider)?;
                        let erased: Arc<Handler> = Arc::new(move |message, context| {
                            let handler = handler.clone();
                            Box::pin(async move {
                                let message = *message
                                    .downcast::<M>()
                                    .map_err(|_| BusError::InvalidResponse(type_name::<M>()))?;
                                Ok(DispatchResponse::from_result(
                                    handler.handle(message, context).await,
                                ))
                            })
                        });
                        Ok(erased)
                    }),
                });
                self
            }
            /// Append an outer-to-inner shared pipeline behavior.
            #[must_use]
            pub fn behavior<B: Behavior>(mut self, behavior: Arc<B>) -> Self {
                self.registry.behavior(behavior);
                self
            }
            /// Bind a DI behavior at bus construction using normal provider scope rules.
            #[must_use]
            pub fn behavior_provider<B: Behavior>(mut self) -> Self {
                self.registry.behavior_provider::<B>();
                self
            }
            /// Build standalone registrations; DI-backed registrations require build_with/build_in.
            pub fn build(self) -> Result<$bus, BusError> {
                Ok($bus {
                    registry: Arc::new(self.registry.bind(None)?),
                })
            }
            /// Bind handlers to an existing root container without retaining that container.
            pub fn build_with(self, container: &Container) -> Result<$bus, BusError> {
                Ok($bus {
                    registry: Arc::new(self.registry.bind(Some(container))?),
                })
            }
            /// Bind handlers to one existing request scope.
            pub fn build_in(self, scope: &RequestScope) -> Result<$bus, BusError> {
                self.build_with(&scope.container)
            }
            /// Register the bus in DI with explicit lifetime and handler/behavior dependencies.
            /// Use Request scope for request-scoped handlers; no Container is retained.
            pub fn into_provider(self, scope: Scope) -> Result<ProviderDefinition, BusError> {
                self.registry.validate()?;
                let dependencies = unique_dependencies(&self.registry.dependencies);
                Ok(ProviderDefinition::new::<$bus, _>(
                    scope,
                    dependencies,
                    move |container| {
                        self.registry
                            .bind(Some(container))
                            .map(|registry| $bus {
                                registry: Arc::new(registry),
                            })
                            .map_err(provider_error)
                    },
                ))
            }
        }
    };
}
request_bus!(
    Command,
    CommandHandler,
    CommandBus,
    CommandBusBuilder,
    Command
);
request_bus!(Query, QueryHandler, QueryBus, QueryBusBuilder, Query);

/// A typed event, consumed by zero or more subscribers.
pub trait Event: Send + Sync + 'static {}
/// Event handler failure. Use safe static codes for expected application failures.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EventError {
    /// Subscriber returned an expected application failure.
    Handler(&'static str),
    /// Subscriber panicked; the panic payload is not included in the report.
    Panicked,
}
/// Subscriber implementation; events are shared without requiring Clone on E.
pub trait EventHandler<E: Event>: Send + Sync + 'static {
    /// React to an event. Publication awaits this future before the next subscriber.
    fn handle(
        &self,
        event: Arc<E>,
        context: DispatchContext,
    ) -> impl Future<Output = Result<(), EventError>> + Send;
}
/// One subscriber's observable failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EventFailure {
    /// Explicit subscriber name or concrete handler type name.
    pub handler: &'static str,
    /// Static, safe failure detail.
    pub error: EventError,
}
/// Complete ordered fan-out results; failures do not skip later subscribers.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EventReport {
    /// Number of registered subscribers invoked.
    pub attempted: usize,
    /// Number returning success.
    pub succeeded: usize,
    /// Failures in subscriber registration order.
    pub failures: Vec<EventFailure>,
}
impl EventReport {
    /// Whether every invoked subscriber succeeded; no subscribers is also success.
    pub fn is_success(&self) -> bool {
        self.failures.is_empty()
    }
}
type EventFuture = Pin<Box<dyn Future<Output = Result<(), EventError>> + Send>>;
type EventFn = dyn Fn(Arc<DynMessage>, DispatchContext) -> EventFuture + Send + Sync;
struct EventSpec {
    info: MessageInfo,
    name: &'static str,
    bind: Arc<Binder<Arc<EventFn>>>,
}
#[derive(Clone)]
struct Subscriber {
    name: &'static str,
    handler: Arc<EventFn>,
}
#[derive(Default)]
struct EventRegistry {
    handlers: HashMap<TypeId, Vec<Subscriber>>,
    behaviors: Vec<Arc<dyn Behavior>>,
}
/// Immutable in-process event fan-out with explicit completion and failure reporting.
#[derive(Clone, Default)]
pub struct EventBus {
    registry: Arc<EventRegistry>,
}
impl crate::Injectable for EventBus {
    fn inject(injector: &crate::Injector) -> Result<Self, InjectError> {
        injector.value_of::<Self>()
    }
}
impl EventBus {
    /// Start event subscriber and behavior registration.
    pub fn builder() -> EventBusBuilder {
        EventBusBuilder::default()
    }
    /// Publish and await all matching subscribers in registration order.
    /// Nothing is spawned/detached, persisted, or retried automatically.
    pub async fn publish<E: Event>(&self, event: E) -> Result<EventReport, BusError> {
        self.publish_with(event, DispatchContext::default()).await
    }
    /// Publish using explicit metadata. Subscriber failures/panics are collected,
    /// while a pipeline rejection/panic returns BusError before/around fan-out.
    pub async fn publish_with<E: Event>(
        &self,
        event: E,
        context: DispatchContext,
    ) -> Result<EventReport, BusError> {
        let info = MessageInfo::of::<E>(MessageKind::Event);
        let subscribers = self
            .registry
            .handlers
            .get(&info.id)
            .cloned()
            .unwrap_or_default();
        let handler: Arc<Handler> = Arc::new(move |message, context| {
            let subscribers = subscribers.clone();
            Box::pin(async move {
                let message: Arc<DynMessage> = Arc::from(message);
                let mut report = EventReport::default();
                for subscriber in subscribers {
                    report.attempted += 1;
                    let result = AssertUnwindSafe(async {
                        (subscriber.handler)(message.clone(), context.clone()).await
                    })
                    .catch_unwind()
                    .await
                    .unwrap_or(Err(EventError::Panicked));
                    match result {
                        Ok(()) => report.succeeded += 1,
                        Err(error) => report.failures.push(EventFailure {
                            handler: subscriber.name,
                            error,
                        }),
                    }
                }
                Ok(DispatchResponse::from_event_report(report))
            })
        });
        dispatch(
            info,
            Box::new(event),
            context,
            &self.registry.behaviors,
            &handler,
        )
        .await?
        .take(info.name)
    }
}
/// Event subscriptions are ordered and uniquely named within each event type.
#[derive(Default)]
pub struct EventBusBuilder {
    handlers: Vec<EventSpec>,
    behaviors: RegistryBuilder,
}
impl EventBusBuilder {
    /// Register an async function/closure under a stable subscriber name.
    #[must_use]
    pub fn handler_fn<E, F, Fut>(mut self, name: &'static str, handler: F) -> Self
    where
        E: Event,
        F: Fn(Arc<E>, DispatchContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), EventError>> + Send + 'static,
    {
        let handler = Arc::new(handler);
        let erased: Arc<EventFn> = Arc::new(move |message, context| {
            let handler = handler.clone();
            Box::pin(async move {
                let event = message
                    .downcast::<E>()
                    .map_err(|_| EventError::Handler("event type mismatch"))?;
                handler(event, context).await
            })
        });
        self.handlers.push(EventSpec {
            info: MessageInfo::of::<E>(MessageKind::Event),
            name,
            bind: Arc::new(move |_| Ok(erased.clone())),
        });
        self
    }
    /// Register an existing typed subscriber, named by its concrete handler type.
    #[must_use]
    pub fn handler<E: Event, H: EventHandler<E>>(self, handler: Arc<H>) -> Self {
        self.handler_fn::<E, _, _>(type_name::<H>(), move |event, context| {
            let handler = handler.clone();
            async move { handler.handle(event, context).await }
        })
    }
    /// Bind a DI subscriber at bus construction, respecting module visibility/scopes.
    #[must_use]
    pub fn handler_provider<E: Event, H: EventHandler<E>>(mut self) -> Self {
        self.behaviors.dependencies.push(ProviderKey::of::<H>());
        self.handlers.push(EventSpec {
            info: MessageInfo::of::<E>(MessageKind::Event),
            name: type_name::<H>(),
            bind: Arc::new(|container| {
                let handler = container
                    .ok_or(BusError::ProviderContextRequired(type_name::<H>()))?
                    .resolve::<H>()
                    .map_err(BusError::Provider)?;
                let erased: Arc<EventFn> = Arc::new(move |message, context| {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let event = message
                            .downcast::<E>()
                            .map_err(|_| EventError::Handler("event type mismatch"))?;
                        handler.handle(event, context).await
                    })
                });
                Ok(erased)
            }),
        });
        self
    }
    /// Append shared pipeline behavior around the complete publication.
    #[must_use]
    pub fn behavior<B: Behavior>(mut self, behavior: Arc<B>) -> Self {
        self.behaviors.behavior(behavior);
        self
    }
    /// Bind a shared behavior from DI when constructing the bus.
    #[must_use]
    pub fn behavior_provider<B: Behavior>(mut self) -> Self {
        self.behaviors.behavior_provider::<B>();
        self
    }
    fn validate(&self) -> Result<(), BusError> {
        let mut seen = HashSet::new();
        for spec in &self.handlers {
            if spec.name.trim().is_empty() {
                return Err(BusError::InvalidRegistration("subscriber name is required"));
            }
            if !seen.insert((spec.info.id, spec.name)) {
                return Err(BusError::DuplicateSubscriber {
                    event: spec.info.name,
                    handler: spec.name,
                });
            }
        }
        Ok(())
    }
    fn bind(&self, container: Option<&Container>) -> Result<EventBus, BusError> {
        self.validate()?;
        let mut handlers: HashMap<TypeId, Vec<Subscriber>> = HashMap::new();
        for spec in &self.handlers {
            handlers.entry(spec.info.id).or_default().push(Subscriber {
                name: spec.name,
                handler: (spec.bind)(container)?,
            });
        }
        let behaviors = self
            .behaviors
            .behaviors
            .iter()
            .map(|spec| (spec.bind)(container))
            .collect::<Result<_, _>>()?;
        Ok(EventBus {
            registry: Arc::new(EventRegistry {
                handlers,
                behaviors,
            }),
        })
    }
    /// Build a standalone event bus; provider-backed registrations need a container.
    pub fn build(self) -> Result<EventBus, BusError> {
        self.bind(None)
    }
    /// Bind subscriber providers without retaining the root container.
    pub fn build_with(self, container: &Container) -> Result<EventBus, BusError> {
        self.bind(Some(container))
    }
    /// Bind subscribers using one existing request scope.
    pub fn build_in(self, scope: &RequestScope) -> Result<EventBus, BusError> {
        self.build_with(&scope.container)
    }
    /// Register a DI bus with subscriber/behavior dependencies and an explicit lifetime.
    pub fn into_provider(self, scope: Scope) -> Result<ProviderDefinition, BusError> {
        self.validate()?;
        let dependencies = unique_dependencies(&self.behaviors.dependencies);
        Ok(ProviderDefinition::new::<EventBus, _>(
            scope,
            dependencies,
            move |container| self.bind(Some(container)).map_err(provider_error),
        ))
    }
}
