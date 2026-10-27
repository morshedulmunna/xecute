//! Async startup, dependency validation, rollback, cancellation and shutdown contracts.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use xecute::{
    Container, InjectError, ModuleDefinition, ModuleGraph, ProviderDefinition, ProviderKey,
};

type Events = Arc<Mutex<Vec<&'static str>>>;
struct Database(Events);
struct Service(Arc<Database>);

fn database(events: Events) -> ProviderDefinition {
    ProviderDefinition::singleton_async::<Database, _, _>(vec![], move |_| {
        let events = events.clone();
        async move {
            tokio::task::yield_now().await;
            events.lock().unwrap().push("connect");
            Ok(Database(events))
        }
    })
    .on_init::<Database, _, _>(|db| async move {
        tokio::task::yield_now().await;
        db.0.lock().unwrap().push("db.init");
        Ok(())
    })
    .on_shutdown::<Database, _, _>(|db| async move {
        db.0.lock().unwrap().push("db.close");
        Ok(())
    })
}

fn service() -> ProviderDefinition {
    ProviderDefinition::singleton::<Service, _>(vec![ProviderKey::of::<Database>()], |c| {
        Ok(Service(c.resolve()?))
    })
    .on_init::<Service, _, _>(|service| async move {
        service.0.0.lock().unwrap().push("service.init");
        Ok(())
    })
    .on_shutdown::<Service, _, _>(|service| async move {
        service.0.0.lock().unwrap().push("service.close");
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_clones_start_once_and_shutdown_in_reverse_dependency_order() {
    let events = Events::default();
    let container = ModuleGraph::new(vec![
        ModuleDefinition::new("database")
            .provider(database(events.clone()))
            .export::<Database>(),
        ModuleDefinition::new("app")
            .import("database")
            .provider(service()),
    ])
    .build()
    .unwrap();
    // Early sync resolution must not poison the dependent singleton's cache.
    assert!(matches!(
        container.resolve::<Service>(),
        Err(InjectError::AsyncInitializationRequired(_))
    ));
    assert!(matches!(
        container.initialize(),
        Err(InjectError::AsyncInitializationRequired(_))
    ));
    let first = container.clone();
    let second = container.clone();
    let (a, b) = tokio::join!(
        tokio::spawn(async move { first.initialize_async().await }),
        tokio::spawn(async move { second.initialize_async().await }),
    );
    a.unwrap().unwrap();
    b.unwrap().unwrap();
    container.initialize().unwrap();
    let service = container.resolve::<Service>().unwrap();
    assert!(Arc::ptr_eq(
        &service.0,
        &container.resolve::<Database>().unwrap()
    ));
    assert!(Arc::ptr_eq(
        &service.0,
        &container.request().resolve::<Database>().unwrap()
    ));
    assert_eq!(
        *events.lock().unwrap(),
        ["connect", "db.init", "service.init"]
    );
    let (a, b) = tokio::join!(container.shutdown_async(), container.shutdown_async());
    a.unwrap();
    b.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.init",
            "service.close",
            "db.close"
        ]
    );
    assert_eq!(
        container.initialize_async().await,
        Err(InjectError::LifecycleStopped)
    );
}

#[tokio::test]
async fn async_factories_can_await_then_resolve_declared_dependencies() {
    let c = Container::builder()
        .value(41_u32)
        .provider(ProviderDefinition::singleton_async::<u64, _, _>(
            vec![ProviderKey::of::<u32>()],
            |c| async move {
                tokio::task::yield_now().await;
                Ok(u64::from(*c.resolve::<u32>()?) + 1)
            },
        ))
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    assert_eq!(*c.resolve::<u64>().unwrap(), 42);
}

#[tokio::test]
async fn init_failure_rolls_back_including_failing_provider_and_caches_original_error() {
    let events = Events::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let broken = service().on_init::<Service, _, _>(move |service| {
        let counter = counter.clone();
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            service.0.0.lock().unwrap().push("service.fail");
            Err(InjectError::ConstructionFailed("startup failed"))
        }
    });
    let c = Container::builder()
        .provider(broken)
        .provider(database(events.clone()))
        .build()
        .unwrap();
    for _ in 0..2 {
        assert_eq!(
            c.initialize_async().await,
            Err(InjectError::ConstructionFailed("startup failed"))
        );
    }
    c.shutdown_async().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.fail",
            "service.close",
            "db.close"
        ]
    );
}

#[tokio::test]
async fn factory_failure_rolls_back_dependencies_without_calling_its_own_hooks() {
    let events = Events::default();
    let c = Container::builder()
        .provider(database(events.clone()))
        .provider(
            ProviderDefinition::singleton_async::<Service, _, _>(
                vec![ProviderKey::of::<Database>()],
                |_| async { Err(InjectError::ConstructionFailed("connect failed")) },
            )
            .on_shutdown::<Service, _, _>(|_| async { panic!("no service was constructed") }),
        )
        .build()
        .unwrap();
    assert_eq!(
        c.initialize_async().await,
        Err(InjectError::ConstructionFailed("connect failed"))
    );
    assert_eq!(*events.lock().unwrap(), ["connect", "db.init", "db.close"]);
}

#[tokio::test]
async fn shutdown_continues_after_errors_and_does_not_repeat_hooks() {
    let events = Events::default();
    let c = Container::builder()
        .provider(database(events.clone()))
        .provider(
            service().on_shutdown::<Service, _, _>(|service| async move {
                service.0.0.lock().unwrap().push("service.fail_close");
                Err(InjectError::ConstructionFailed("close failed"))
            }),
        )
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    assert_eq!(
        c.shutdown_async().await,
        Err(InjectError::ConstructionFailed("close failed"))
    );
    c.shutdown_async().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.init",
            "service.fail_close",
            "db.close"
        ]
    );
}

#[tokio::test]
async fn dependency_validation_survives_await_points() {
    let c = Container::builder()
        .value(7_u32)
        .provider(ProviderDefinition::singleton_async::<u64, _, _>(
            vec![],
            |c| async move {
                tokio::task::yield_now().await;
                Ok(u64::from(*c.resolve::<u32>()?))
            },
        ))
        .build()
        .unwrap();
    assert_eq!(
        c.initialize_async().await,
        Err(InjectError::UndeclaredDependency {
            owner: std::any::type_name::<u64>(),
            dependency: std::any::type_name::<u32>(),
        })
    );
}

#[tokio::test]
async fn optional_absent_dependency_and_transient_factories_keep_existing_behavior() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let c = Container::builder()
        .provider(
            ProviderDefinition::singleton_async::<u32, _, _>(
                vec![ProviderKey::of::<u64>()],
                |c| async move {
                    assert!(c.resolve_optional::<u64>()?.is_none());
                    Ok(5)
                },
            )
            .optional_dependency::<u64>(),
        )
        .provider(ProviderDefinition::transient::<usize, _>(
            vec![],
            move |_| Ok(count.fetch_add(1, Ordering::SeqCst)),
        ))
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(*c.resolve::<usize>().unwrap(), 0);
    assert_eq!(*c.resolve::<usize>().unwrap(), 1);
}

#[test]
fn rejects_non_singleton_hooks_missing_dependencies_cycles_and_request_capture() {
    assert!(matches!(
        Container::builder()
            .provider(
                ProviderDefinition::transient::<u32, _>(vec![], |_| Ok(1))
                    .on_init::<u32, _, _>(|_| async { Ok(()) })
            )
            .build(),
        Err(InjectError::InvalidLifecycleScope(_))
    ));
    assert!(matches!(
        Container::builder()
            .provider(ProviderDefinition::singleton_async::<u32, _, _>(
                vec![ProviderKey::of::<u64>()],
                |_| async { Ok(1) }
            ))
            .build(),
        Err(InjectError::Missing(_))
    ));
    assert!(matches!(
        Container::builder()
            .provider(ProviderDefinition::singleton_async::<u32, _, _>(
                vec![ProviderKey::of::<u32>()],
                |_| async { Ok(1) }
            ))
            .build(),
        Err(InjectError::Circular(_))
    ));
    assert!(matches!(
        Container::builder()
            .provider(ProviderDefinition::request::<u64, _>(vec![], |_| Ok(1)))
            .provider(ProviderDefinition::singleton_async::<u32, _, _>(
                vec![ProviderKey::of::<u64>()],
                |_| async { Ok(1) }
            ))
            .build(),
        Err(InjectError::InvalidScopeDependency { .. })
    ));
}

#[tokio::test]
async fn cancelled_startup_requires_shutdown_and_cleans_constructed_resources() {
    let events = Events::default();
    let entered = Arc::new(tokio::sync::Notify::new());
    let notify = entered.clone();
    let c = Container::builder()
        .provider(database(events.clone()))
        .provider(service().on_init::<Service, _, _>(move |_| {
            let notify = notify.clone();
            async move {
                notify.notify_one();
                std::future::pending().await
            }
        }))
        .build()
        .unwrap();
    let task_container = c.clone();
    let task = tokio::spawn(async move { task_container.initialize_async().await });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        c.initialize_async().await,
        Err(InjectError::LifecycleStopped)
    );
    c.shutdown_async().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        ["connect", "db.init", "service.close", "db.close"]
    );
}

#[tokio::test]
async fn cancelled_shutdown_can_resume_remaining_cleanup_without_repeating_hook() {
    let events = Events::default();
    let entered = Arc::new(tokio::sync::Notify::new());
    let notify = entered.clone();
    let c = Container::builder()
        .provider(database(events.clone()))
        .provider(service().on_shutdown::<Service, _, _>(move |service| {
            let notify = notify.clone();
            async move {
                service.0.0.lock().unwrap().push("service.close_started");
                notify.notify_one();
                std::future::pending().await
            }
        }))
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    let task_container = c.clone();
    let task = tokio::spawn(async move { task_container.shutdown_async().await });
    entered.notified().await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    c.shutdown_async().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.init",
            "service.close_started",
            "db.close"
        ]
    );
}

#[tokio::test]
async fn failed_rollback_cleanup_preserves_startup_error_and_still_closes_dependencies() {
    let events = Events::default();
    let c = Container::builder()
        .provider(database(events.clone()))
        .provider(
            service()
                .on_init::<Service, _, _>(|_| async {
                    Err(InjectError::ConstructionFailed("init failed"))
                })
                .on_shutdown::<Service, _, _>(|_| async {
                    Err(InjectError::ConstructionFailed("cleanup failed"))
                }),
        )
        .build()
        .unwrap();
    assert_eq!(
        c.initialize_async().await,
        Err(InjectError::ConstructionFailed("init failed"))
    );
    assert_eq!(*events.lock().unwrap(), ["connect", "db.init", "db.close"]);
}

#[tokio::test]
async fn async_providers_keep_module_visibility_and_overrides() {
    let private = ModuleGraph::new(vec![
        ModuleDefinition::new("database").provider(database(Events::default())),
        ModuleDefinition::new("app")
            .import("database")
            .provider(service()),
    ])
    .build();
    assert!(matches!(
        private,
        Err(InjectError::PrivateProviderAccess { .. })
    ));
    let c = Container::builder()
        .provider(ProviderDefinition::singleton_async::<u32, _, _>(
            vec![],
            |_| async { panic!("overridden factory ran") },
        ))
        .override_provider(ProviderDefinition::value(42_u32))
        .build()
        .unwrap();
    c.initialize_async().await.unwrap();
    assert_eq!(*c.resolve::<u32>().unwrap(), 42);
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn testing_module_compiles_async_resources_and_cleans_up() {
    let events = Events::default();
    let c = xecute::testing::TestingModule::new()
        .provider(database(events.clone()))
        .provider(service())
        .compile_async()
        .await
        .unwrap();
    c.resolve::<Service>().unwrap();
    c.shutdown_async().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.init",
            "service.close",
            "db.close"
        ]
    );
}

#[cfg(feature = "testing")]
#[tokio::test]
async fn http_requests_resolve_initialized_async_singletons_before_shutdown() {
    use axum::{Router, routing::get};
    use xecute::{
        Inject,
        testing::{TestApp, TestingModule},
    };
    let events = Events::default();
    let container = Arc::new(
        TestingModule::new()
            .provider(database(events.clone()))
            .provider(service())
            .compile_async()
            .await
            .unwrap(),
    );
    let router = Router::new().route(
        "/ready",
        get(|Inject(service): Inject<Service>| async move {
            service.0.0.lock().unwrap().push("request");
            "ready"
        }),
    );
    let app = TestApp::new(xecute::with_container(router, container.clone()));
    for _ in 0..2 {
        let _ = app
            .get("/ready")
            .send()
            .await
            .unwrap()
            .assert_status(axum::http::StatusCode::OK);
    }
    container.shutdown_async().await.unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        [
            "connect",
            "db.init",
            "service.init",
            "request",
            "request",
            "service.close",
            "db.close"
        ]
    );
}
