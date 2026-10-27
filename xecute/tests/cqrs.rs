//! Typed buses, pipelines, fan-out failure semantics, DI scopes and cancellation.
#![cfg(feature = "cqrs")]
use std::{
    convert::Infallible,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
use xecute::{
    Container, InjectError, ModuleDefinition, ModuleGraph, ProviderDefinition, Scope, cqrs::*,
};

#[derive(Debug, Clone, Eq, PartialEq)]
enum DomainError {
    Invalid,
}
struct Add(u32);
impl Command for Add {
    type Output = u32;
    type Error = DomainError;
}
struct Read;
impl Query for Read {
    type Output = String;
    type Error = Infallible;
}
struct Added(u32);
impl Event for Added {}
struct Other;
impl Event for Other {}
struct Offset(u32);
impl CommandHandler<Add> for Offset {
    async fn handle(&self, message: Add, _: DispatchContext) -> Result<u32, DomainError> {
        Ok(message.0 + self.0)
    }
}

#[tokio::test]
async fn command_query_and_closure_handlers_keep_typed_outputs_and_metadata() {
    let commands = CommandBus::builder()
        .handler::<Add, _>(Arc::new(Offset(10)))
        .build()
        .unwrap();
    let queries = QueryBus::builder()
        .handler_fn::<Read, _, _>(|_, context| async move {
            Ok(context.value::<String>().cloned().unwrap_or_default())
        })
        .build()
        .unwrap();
    assert_eq!(commands.execute(Add(2)).await, Ok(12));
    assert_eq!(
        queries
            .execute_with(
                Read,
                DispatchContext::default().with_value("tenant-7".to_owned())
            )
            .await
            .unwrap(),
        "tenant-7"
    );
}
#[test]
fn duplicate_request_handlers_and_provider_context_requirements_fail_early() {
    let duplicate = CommandBus::builder()
        .handler::<Add, _>(Arc::new(Offset(1)))
        .handler::<Add, _>(Arc::new(Offset(2)))
        .build();
    assert!(matches!(duplicate, Err(BusError::DuplicateHandler(_))));
    assert!(matches!(
        QueryBus::builder()
            .handler_fn::<Read, _, _>(|_, _| async { Ok(String::new()) })
            .handler_fn::<Read, _, _>(|_, _| async { Ok(String::new()) })
            .build(),
        Err(BusError::DuplicateHandler(_))
    ));
    assert!(matches!(
        CommandBus::builder()
            .handler_provider::<Add, Offset>()
            .build(),
        Err(BusError::ProviderContextRequired(_))
    ));
}
#[tokio::test]
async fn domain_errors_are_distinct_from_missing_handler_and_panic_errors() {
    let bus = CommandBus::builder()
        .handler_fn::<Add, _, _>(|_, _| async { Err(DomainError::Invalid) })
        .build()
        .unwrap();
    assert_eq!(
        bus.execute(Add(0)).await,
        Err(DispatchError::Handler(DomainError::Invalid))
    );
    assert!(matches!(
        CommandBus::default().execute(Add(0)).await,
        Err(DispatchError::Bus(BusError::MissingHandler(_)))
    ));
    let panic = CommandBus::builder()
        .handler_fn::<Add, _, _>(|_, _| {
            panic!("test factory panic");
            #[allow(unreachable_code)]
            async {
                Ok(0)
            }
        })
        .build()
        .unwrap();
    assert!(matches!(
        panic.execute(Add(0)).await,
        Err(DispatchError::Bus(BusError::Panicked(_)))
    ));
    let panic = CommandBus::builder()
        .handler_fn::<Add, _, _>(|_, _| async {
            panic!("test future panic");
            #[allow(unreachable_code)]
            Ok(0)
        })
        .build()
        .unwrap();
    assert!(matches!(
        panic.execute(Add(0)).await,
        Err(DispatchError::Bus(BusError::Panicked(_)))
    ));
}

struct Trace {
    log: Arc<Mutex<Vec<String>>>,
    name: &'static str,
}
impl Behavior for Trace {
    fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async move {
            self.log.lock().unwrap().push(format!("{} in", self.name));
            let response = next.run().await?;
            self.log
                .lock()
                .unwrap()
                .push(format!("{} out {}", self.name, response.is_success()));
            Ok(response)
        })
    }
}
struct Reject;
impl Behavior for Reject {
    fn handle<'a>(&'a self, _: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async { Err(BusError::Rejected("access_denied")) })
    }
}
struct WrongResponse;
impl Behavior for WrongResponse {
    fn handle<'a>(&'a self, _: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async {
            Ok(DispatchResponse::from_result::<String, ()>(Ok(
                "wrong".into()
            )))
        })
    }
}
struct Cached;
impl Behavior for Cached {
    fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a> {
        Box::pin(async move {
            assert_eq!(next.info().kind, MessageKind::Command);
            assert!(next.info().is::<Add>());
            assert_eq!(next.message::<Add>().unwrap().0, 42);
            assert_eq!(next.context().value::<u64>(), Some(&7));
            Ok(DispatchResponse::from_result::<u32, DomainError>(Ok(99)))
        })
    }
}
#[tokio::test]
async fn behaviors_wrap_in_order_and_observe_typed_domain_failure() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let events = log.clone();
    let bus = CommandBus::builder()
        .behavior(Arc::new(Trace {
            log: log.clone(),
            name: "first",
        }))
        .behavior(Arc::new(Trace {
            log: log.clone(),
            name: "second",
        }))
        .handler_fn::<Add, _, _>(move |_, _| {
            let events = events.clone();
            async move {
                events.lock().unwrap().push("handler".into());
                Err(DomainError::Invalid)
            }
        })
        .build()
        .unwrap();
    assert_eq!(
        bus.execute(Add(1)).await,
        Err(DispatchError::Handler(DomainError::Invalid))
    );
    assert_eq!(
        *log.lock().unwrap(),
        [
            "first in",
            "second in",
            "handler",
            "second out false",
            "first out false"
        ]
    );
}
#[tokio::test]
async fn pipeline_can_reject_or_return_typed_response_without_invoking_handler() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let bus = CommandBus::builder()
        .behavior(Arc::new(Reject))
        .handler_fn::<Add, _, _>(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok(0) }
        })
        .build()
        .unwrap();
    assert_eq!(
        bus.execute(Add(0)).await,
        Err(DispatchError::Bus(BusError::Rejected("access_denied")))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let cached = CommandBus::builder()
        .behavior(Arc::new(Cached))
        .handler_fn::<Add, _, _>(|_, _| async { panic!("cached handler ran") })
        .build()
        .unwrap();
    assert_eq!(
        cached
            .execute_with(Add(42), DispatchContext::default().with_value(7_u64))
            .await,
        Ok(99)
    );
    let bad = CommandBus::builder()
        .behavior(Arc::new(WrongResponse))
        .handler_fn::<Add, _, _>(|_, _| async { Ok(0) })
        .build()
        .unwrap();
    assert!(matches!(
        bad.execute(Add(0)).await,
        Err(DispatchError::Bus(BusError::InvalidResponse(_)))
    ));
}
#[tokio::test]
async fn pipeline_panics_are_reported_without_exposing_panic_payload() {
    struct Panics;
    impl Behavior for Panics {
        fn handle<'a>(&'a self, _: Next<'a>) -> DispatchFuture<'a> {
            panic!("private test panic");
        }
    }
    let bus = CommandBus::builder()
        .behavior(Arc::new(Panics))
        .handler::<Add, _>(Arc::new(Offset(0)))
        .build()
        .unwrap();
    let error = bus.execute(Add(1)).await.unwrap_err();
    assert!(matches!(error, DispatchError::Bus(BusError::Panicked(_))));
    assert!(!format!("{error:?}").contains("private"));
}
#[tokio::test]
async fn event_fanout_preserves_order_and_collects_failures_and_panics() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let tail = seen.clone();
    let bus = EventBus::builder()
        .handler_fn::<Added, _, _>("first", move |event, _| {
            let log = log.clone();
            async move {
                log.lock()
                    .unwrap()
                    .push(("first", event.0, Arc::as_ptr(&event) as usize));
                Ok(())
            }
        })
        .handler_fn::<Added, _, _>("failed", |_, _| async {
            Err(EventError::Handler("delivery_failed"))
        })
        .handler_fn::<Added, _, _>("panic", |_, _| async {
            panic!("private event panic");
            #[allow(unreachable_code)]
            Ok(())
        })
        .handler_fn::<Added, _, _>("last", move |event, _| {
            let tail = tail.clone();
            async move {
                tail.lock()
                    .unwrap()
                    .push(("last", event.0, Arc::as_ptr(&event) as usize));
                Ok(())
            }
        })
        .build()
        .unwrap();
    let report = bus.publish(Added(42)).await.unwrap();
    assert_eq!(report.attempted, 4);
    assert_eq!(report.succeeded, 2);
    assert!(!report.is_success());
    assert_eq!(
        report.failures,
        [
            EventFailure {
                handler: "failed",
                error: EventError::Handler("delivery_failed")
            },
            EventFailure {
                handler: "panic",
                error: EventError::Panicked
            }
        ]
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen[0].0, "first");
    assert_eq!(seen[1].0, "last");
    assert_eq!(seen[0].2, seen[1].2);
    assert_eq!(seen[1].1, 42);
}
#[tokio::test]
async fn unrelated_events_and_zero_subscriber_publications_are_valid() {
    let bus = EventBus::builder()
        .handler_fn::<Added, _, _>("only-added", |_, _| async {
            panic!("wrong event dispatched");
            #[allow(unreachable_code)]
            Ok(())
        })
        .build()
        .unwrap();
    assert_eq!(bus.publish(Other).await.unwrap(), EventReport::default());
    assert!(
        EventBus::default()
            .publish(Added(0))
            .await
            .unwrap()
            .is_success()
    );
}
#[test]
fn event_subscriber_names_are_unique_per_type_and_nonblank() {
    assert!(matches!(
        EventBus::builder()
            .handler_fn::<Added, _, _>("same", |_, _| async { Ok(()) })
            .handler_fn::<Added, _, _>("same", |_, _| async { Ok(()) })
            .build(),
        Err(BusError::DuplicateSubscriber { .. })
    ));
    assert!(matches!(
        EventBus::builder()
            .handler_fn::<Added, _, _>(" ", |_, _| async { Ok(()) })
            .build(),
        Err(BusError::InvalidRegistration(_))
    ));
    EventBus::builder()
        .handler_fn::<Added, _, _>("same", |_, _| async { Ok(()) })
        .handler_fn::<Other, _, _>("same", |_, _| async { Ok(()) })
        .build()
        .unwrap();
}
#[tokio::test]
async fn event_pipelines_observe_partial_failure_and_can_reject_publication() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let bus = EventBus::builder()
        .behavior(Arc::new(Trace {
            log: log.clone(),
            name: "events",
        }))
        .handler_fn::<Added, _, _>("failed", |_, _| async {
            Err(EventError::Handler("failed"))
        })
        .build()
        .unwrap();
    assert!(!bus.publish(Added(1)).await.unwrap().is_success());
    assert_eq!(*log.lock().unwrap(), ["events in", "events out false"]);
    let bus = EventBus::builder()
        .behavior(Arc::new(Reject))
        .handler_fn::<Added, _, _>("never", |_, _| async { panic!("rejected event ran") })
        .build()
        .unwrap();
    assert_eq!(
        bus.publish(Added(1)).await,
        Err(BusError::Rejected("access_denied"))
    );
}
#[tokio::test]
async fn query_pipeline_uses_query_metadata_and_retains_typed_result() {
    struct Inspect;
    impl Behavior for Inspect {
        fn handle<'a>(&'a self, next: Next<'a>) -> DispatchFuture<'a> {
            Box::pin(async move {
                assert_eq!(next.info().kind, MessageKind::Query);
                let response = next.run().await?;
                assert!(
                    response
                        .value::<Result<String, Infallible>>()
                        .unwrap()
                        .is_ok()
                );
                Ok(response)
            })
        }
    }
    let bus = QueryBus::builder()
        .behavior(Arc::new(Inspect))
        .handler_fn::<Read, _, _>(|_, _| async { Ok("read".into()) })
        .build()
        .unwrap();
    assert_eq!(bus.execute(Read).await.unwrap(), "read");
}
#[tokio::test]
async fn di_provider_handlers_and_behaviors_bind_with_module_visibility() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let definition = CommandBus::builder()
        .handler_provider::<Add, Offset>()
        .behavior_provider::<Trace>()
        .into_provider(Scope::Singleton)
        .unwrap();
    let modules = vec![
        ModuleDefinition::new("handlers")
            .provider(ProviderDefinition::value(Offset(7)))
            .export::<Offset>(),
        ModuleDefinition::new("application")
            .import("handlers")
            .provider(ProviderDefinition::value(Trace {
                log: log.clone(),
                name: "DI",
            }))
            .provider(definition),
    ];
    let c = ModuleGraph::new(modules).build().unwrap();
    c.initialize().unwrap();
    assert_eq!(
        c.resolve::<CommandBus>().unwrap().execute(Add(1)).await,
        Ok(8)
    );
    assert_eq!(*log.lock().unwrap(), ["DI in", "DI out true"]);
    let private = ModuleGraph::new(vec![
        ModuleDefinition::new("handlers").provider(ProviderDefinition::value(Offset(0))),
        ModuleDefinition::new("app").provider(
            CommandBus::builder()
                .handler_provider::<Add, Offset>()
                .into_provider(Scope::Singleton)
                .unwrap(),
        ),
    ])
    .build();
    assert!(matches!(
        private,
        Err(InjectError::PrivateProviderAccess { .. })
    ));
}
struct Count;
impl Command for Count {
    type Output = usize;
    type Error = Infallible;
}
struct Counter(AtomicUsize);
impl CommandHandler<Count> for Counter {
    async fn handle(&self, _: Count, _: DispatchContext) -> Result<usize, Infallible> {
        Ok(self.0.fetch_add(1, Ordering::SeqCst))
    }
}
fn counter() -> ProviderDefinition {
    ProviderDefinition::request::<Counter, _>(vec![], |_| Ok(Counter(AtomicUsize::new(0))))
}
#[tokio::test]
async fn request_scoped_bus_captures_only_its_requests_handler_instances() {
    let c = Container::builder()
        .provider(counter())
        .provider(
            CommandBus::builder()
                .handler_provider::<Count, Counter>()
                .into_provider(Scope::Request)
                .unwrap(),
        )
        .build()
        .unwrap();
    let one = c.request();
    let two = c.request();
    let a = one.resolve::<CommandBus>().unwrap();
    let b = two.resolve::<CommandBus>().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert_eq!(a.execute(Count).await.unwrap(), 0);
    assert_eq!(a.execute(Count).await.unwrap(), 1);
    assert_eq!(b.execute(Count).await.unwrap(), 0);
    assert!(matches!(
        c.resolve::<CommandBus>(),
        Err(InjectError::RequestScopeRequired(_))
    ));
}
#[test]
fn di_rejects_missing_handlers_invalid_scope_capture_and_circular_bus_dependencies() {
    assert!(matches!(
        Container::builder()
            .provider(
                CommandBus::builder()
                    .handler_provider::<Add, Offset>()
                    .into_provider(Scope::Singleton)
                    .unwrap()
            )
            .build(),
        Err(InjectError::Missing(_))
    ));
    assert!(matches!(
        Container::builder()
            .provider(counter())
            .provider(
                CommandBus::builder()
                    .handler_provider::<Count, Counter>()
                    .into_provider(Scope::Singleton)
                    .unwrap()
            )
            .build(),
        Err(InjectError::InvalidScopeDependency { .. })
    ));
    let handler = ProviderDefinition::singleton::<Offset, _>(
        vec![xecute::ProviderKey::of::<CommandBus>()],
        |_| Ok(Offset(0)),
    );
    assert!(matches!(
        Container::builder()
            .provider(handler)
            .provider(
                CommandBus::builder()
                    .handler_provider::<Add, Offset>()
                    .into_provider(Scope::Singleton)
                    .unwrap()
            )
            .build(),
        Err(InjectError::Circular(_))
    ));
}
#[tokio::test]
async fn manual_di_binding_supports_root_and_request_scopes_without_context_retention() {
    let c = Container::builder()
        .value(Offset(3))
        .provider(counter())
        .build()
        .unwrap();
    let bus = CommandBus::builder()
        .handler_provider::<Add, Offset>()
        .build_with(&c)
        .unwrap();
    assert_eq!(bus.execute(Add(4)).await, Ok(7));
    assert!(matches!(
        CommandBus::builder()
            .handler_provider::<Count, Counter>()
            .build_with(&c),
        Err(BusError::Provider(InjectError::RequestScopeRequired(_)))
    ));
    let scoped = CommandBus::builder()
        .handler_provider::<Count, Counter>()
        .build_in(&c.request())
        .unwrap();
    assert_eq!(scoped.execute(Count).await.unwrap(), 0);
}
#[tokio::test]
async fn bus_does_not_create_a_container_cache_reference_cycle() {
    struct Owned(Arc<AtomicUsize>);
    impl Drop for Owned {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    impl CommandHandler<Add> for Owned {
        async fn handle(&self, message: Add, _: DispatchContext) -> Result<u32, DomainError> {
            Ok(message.0)
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let c = Container::builder()
        .value(Owned(drops.clone()))
        .provider(
            CommandBus::builder()
                .handler_provider::<Add, Owned>()
                .into_provider(Scope::Singleton)
                .unwrap(),
        )
        .build()
        .unwrap();
    let bus = c.resolve::<CommandBus>().unwrap();
    let weak = Arc::downgrade(&bus);
    drop(c);
    assert_eq!(bus.execute(Add(1)).await, Ok(1));
    drop(bus);
    assert!(weak.upgrade().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn concurrent_dispatch_keeps_context_values_isolated() {
    let bus = Arc::new(
        CommandBus::builder()
            .handler_fn::<Add, _, _>(|message, context| async move {
                tokio::task::yield_now().await;
                Ok(message.0 + context.value::<u32>().copied().unwrap())
            })
            .build()
            .unwrap(),
    );
    let base = DispatchContext::default().with_value(999_u32);
    let mut tasks = Vec::new();
    for i in 0..32 {
        let bus = bus.clone();
        let context = base.clone().with_value(i);
        tasks.push(tokio::spawn(async move {
            assert_eq!(bus.execute_with(Add(1), context).await, Ok(i + 1));
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(base.value::<u32>(), Some(&999));
}
struct DropSignal(Arc<tokio::sync::Notify>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.notify_one();
    }
}
#[tokio::test]
async fn cancelling_command_dispatch_drops_the_handler_future() {
    let entered = Arc::new(tokio::sync::Notify::new());
    let dropped = Arc::new(tokio::sync::Notify::new());
    let (signal, drop_signal) = (entered.clone(), dropped.clone());
    let bus = CommandBus::builder()
        .handler_fn::<Add, _, _>(move |_, _| {
            let (signal, drop_signal) = (signal.clone(), drop_signal.clone());
            async move {
                let _guard = DropSignal(drop_signal);
                signal.notify_one();
                std::future::pending().await
            }
        })
        .build()
        .unwrap();
    let task = tokio::spawn(async move { bus.execute(Add(0)).await });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    dropped.notified().await;
}
#[tokio::test]
async fn cancelled_event_publication_leaves_completed_side_effects_and_skips_later_subscribers() {
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let later = count.clone();
    let entered = Arc::new(tokio::sync::Notify::new());
    let signal = entered.clone();
    let dropped = Arc::new(tokio::sync::Notify::new());
    let d = dropped.clone();
    let bus = EventBus::builder()
        .handler_fn::<Added, _, _>("done", move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok(()) }
        })
        .handler_fn::<Added, _, _>("blocked", move |_, _| {
            let (signal, d) = (signal.clone(), d.clone());
            async move {
                let _guard = DropSignal(d);
                signal.notify_one();
                std::future::pending().await
            }
        })
        .handler_fn::<Added, _, _>("later", move |_, _| {
            later.fetch_add(100, Ordering::SeqCst);
            async { Ok(()) }
        })
        .build()
        .unwrap();
    let task = tokio::spawn(async move { bus.publish(Added(0)).await });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    dropped.notified().await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[cfg(feature = "testing")]
#[tokio::test]
async fn testing_module_overrides_handlers_before_bus_binding() {
    let c = xecute::testing::TestingModule::new()
        .provider(ProviderDefinition::value(Offset(1)))
        .provider(
            CommandBus::builder()
                .handler_provider::<Add, Offset>()
                .into_provider(Scope::Singleton)
                .unwrap(),
        )
        .override_value(Offset(100))
        .compile_async()
        .await
        .unwrap();
    assert_eq!(
        c.resolve::<CommandBus>().unwrap().execute(Add(1)).await,
        Ok(101)
    );
    c.shutdown_async().await.unwrap();
}

impl QueryHandler<Read> for Counter {
    async fn handle(&self, _: Read, _: DispatchContext) -> Result<String, Infallible> {
        Ok(self.0.load(Ordering::SeqCst).to_string())
    }
}
impl EventHandler<Added> for Counter {
    async fn handle(&self, event: Arc<Added>, _: DispatchContext) -> Result<(), EventError> {
        self.0.fetch_add(event.0 as usize, Ordering::SeqCst);
        Ok(())
    }
}
#[tokio::test]
async fn provider_backed_event_and_query_handlers_share_only_one_request_scope() {
    let c = Container::builder()
        .provider(counter())
        .provider(
            EventBus::builder()
                .handler_provider::<Added, Counter>()
                .into_provider(Scope::Request)
                .unwrap(),
        )
        .provider(
            QueryBus::builder()
                .handler_provider::<Read, Counter>()
                .into_provider(Scope::Request)
                .unwrap(),
        )
        .build()
        .unwrap();
    let one = c.request();
    let two = c.request();
    let events = one.resolve::<EventBus>().unwrap();
    let reads = one.resolve::<QueryBus>().unwrap();
    assert_eq!(events.publish(Added(5)).await.unwrap().succeeded, 1);
    assert_eq!(reads.execute(Read).await.unwrap(), "5");
    assert_eq!(
        two.resolve::<QueryBus>()
            .unwrap()
            .execute(Read)
            .await
            .unwrap(),
        "0"
    );
}
#[tokio::test]
async fn direct_typed_event_and_query_handlers_work_and_legacy_injector_requires_bus_value() {
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    let events = EventBus::builder()
        .handler::<Added, _>(counter.clone())
        .build()
        .unwrap();
    let queries = QueryBus::builder()
        .handler::<Read, _>(counter)
        .build()
        .unwrap();
    events.publish(Added(7)).await.unwrap();
    assert_eq!(queries.execute(Read).await.unwrap(), "7");
    let bus = CommandBus::builder()
        .handler::<Add, _>(Arc::new(Offset(1)))
        .build()
        .unwrap();
    let injector = xecute::Injector::new().value(bus);
    let bound = injector.resolve::<CommandBus>().unwrap();
    assert_eq!(bound.execute(Add(1)).await, Ok(2));
    assert!(matches!(
        xecute::Injector::new().resolve::<CommandBus>(),
        Err(InjectError::Missing(_))
    ));
}

#[tokio::test]
async fn async_provider_initialization_finishes_before_bus_handler_binding() {
    let ready = Arc::new(AtomicUsize::new(0));
    let state = ready.clone();
    let handler = ProviderDefinition::singleton_async::<Offset, _, _>(vec![], move |_| {
        let state = state.clone();
        async move {
            tokio::task::yield_now().await;
            state.store(1, Ordering::SeqCst);
            Ok(Offset(42))
        }
    });
    let c = Container::builder()
        .provider(handler)
        .provider(
            CommandBus::builder()
                .handler_provider::<Add, Offset>()
                .into_provider(Scope::Singleton)
                .unwrap(),
        )
        .build()
        .unwrap();
    assert!(matches!(
        c.resolve::<CommandBus>(),
        Err(InjectError::AsyncInitializationRequired(_))
    ));
    c.initialize_async().await.unwrap();
    assert_eq!(ready.load(Ordering::SeqCst), 1);
    assert_eq!(
        c.resolve::<CommandBus>().unwrap().execute(Add(0)).await,
        Ok(42)
    );
    c.shutdown_async().await.unwrap();
}
