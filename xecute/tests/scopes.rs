//! Behavioral regression coverage for configuration, errors, and provider lifecycles.

use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};
use xecute::{Container, InjectError, ProviderDefinition as Def, ProviderKey as Key, Scope};

struct Shared;
struct RequestValue {
    shared: Arc<Shared>,
}
struct Worker {
    request: Arc<RequestValue>,
}
fn scoped_container() -> Container {
    Container::builder()
        .provider(Def::singleton::<Shared, _>(vec![], |_| Ok(Shared)))
        .provider(Def::new::<RequestValue, _>(
            Scope::Request,
            vec![Key::of::<Shared>()],
            |c| {
                Ok(RequestValue {
                    shared: c.resolve()?,
                })
            },
        ))
        .provider(Def::new::<Worker, _>(
            Scope::Transient,
            vec![Key::of::<RequestValue>()],
            |c| {
                Ok(Worker {
                    request: c.resolve()?,
                })
            },
        ))
        .build()
        .unwrap()
}
#[test]
fn nested_request_factories_share_only_the_current_request() {
    let container = scoped_container();
    assert!(matches!(
        container.resolve::<RequestValue>(),
        Err(InjectError::RequestScopeRequired(_))
    ));
    let first = container.request();
    let same = first.clone();
    let second = container.request();
    let a = first.resolve::<Worker>().unwrap();
    let b = same.resolve::<Worker>().unwrap();
    let other = second.resolve::<Worker>().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert!(Arc::ptr_eq(&a.request, &b.request));
    assert!(!Arc::ptr_eq(&a.request, &other.request));
    assert!(Arc::ptr_eq(&a.request.shared, &other.request.shared));
    let weak = Arc::downgrade(&a.request);
    drop((a, b, first, same));
    assert!(weak.upgrade().is_none());
}
#[test]
fn concurrent_singleton_and_request_initialization_run_once() {
    let count = Arc::new(AtomicUsize::new(0));
    let factory_count = count.clone();
    let container = Arc::new(
        Container::builder()
            .provider(Def::singleton::<Shared, _>(vec![], move |_| {
                factory_count.fetch_add(1, Ordering::SeqCst);
                std::thread::yield_now();
                Ok(Shared)
            }))
            .build()
            .unwrap(),
    );
    let barrier = Arc::new(Barrier::new(16));
    let handles = (0..16)
        .map(|_| {
            let container = container.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                container.resolve::<Shared>().unwrap()
            })
        })
        .collect::<Vec<_>>();
    let values = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));

    let request = scoped_container().request();
    let handles = (0..16)
        .map(|_| {
            let request = request.clone();
            std::thread::spawn(move || request.resolve::<RequestValue>().unwrap())
        })
        .collect::<Vec<_>>();
    let values = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
}
#[test]
fn rejects_transitive_scope_capture_cycles_missing_and_undeclared_dependencies() {
    let result = Container::builder()
        .provider(Def::new::<RequestValue, _>(
            Scope::Request,
            vec![],
            |_| unreachable!(),
        ))
        .provider(Def::new::<Worker, _>(
            Scope::Transient,
            vec![Key::of::<RequestValue>()],
            |_| unreachable!(),
        ))
        .provider(Def::singleton::<Shared, _>(
            vec![Key::of::<Worker>()],
            |_| Ok(Shared),
        ))
        .build();
    assert!(matches!(
        result,
        Err(InjectError::InvalidScopeDependency { .. })
    ));
    let missing = Container::builder()
        .provider(Def::singleton::<Worker, _>(
            vec![Key::of::<Shared>()],
            |_| unreachable!(),
        ))
        .build();
    assert!(matches!(missing, Err(InjectError::Missing(_))));
    let cycle = Container::builder()
        .provider(Def::singleton::<Shared, _>(
            vec![Key::of::<Shared>()],
            |_| Ok(Shared),
        ))
        .build();
    assert!(matches!(cycle, Err(InjectError::Circular(_))));
    let container = Container::builder()
        .value(Shared)
        .provider(Def::new::<Worker, _>(Scope::Transient, vec![], |c| {
            c.resolve::<Shared>()?;
            unreachable!()
        }))
        .build()
        .unwrap();
    assert!(matches!(
        container.resolve::<Worker>(),
        Err(InjectError::UndeclaredDependency { .. })
    ));
}
#[test]
fn optional_resolution_does_not_hide_a_registered_factory_failure() {
    let container = Container::builder()
        .provider(Def::singleton::<Shared, _>(vec![], |_| {
            Err(InjectError::Missing("factory dependency"))
        }))
        .build()
        .unwrap();
    assert!(matches!(
        container.resolve_optional::<Shared>(),
        Err(InjectError::Missing("factory dependency"))
    ));
    assert!(container.initialize().is_err());
    assert!(container.resolve_optional::<Worker>().unwrap().is_none());
}
#[test]
fn request_values_drop_after_the_last_context_and_borrow_are_released() {
    let container = scoped_container();
    let request = container.request();
    let value = request.resolve::<RequestValue>().unwrap();
    let weak = Arc::downgrade(&value);
    drop(request);
    assert!(weak.upgrade().is_some());
    drop(value);
    assert!(weak.upgrade().is_none());
}

#[test]
fn failed_initialization_is_cached_and_cycles_unwind_cleanly() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let factory_attempts = attempts.clone();
    let container = Container::builder()
        .provider(Def::singleton::<Shared, _>(vec![], move |_| {
            factory_attempts.fetch_add(1, Ordering::SeqCst);
            Err(InjectError::ConstructionFailed("configuration unavailable"))
        }))
        .build()
        .unwrap();
    for _ in 0..3 {
        assert!(container.resolve::<Shared>().is_err());
    }
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    // An earlier failure must not leave the thread-local resolution stack populated.
    assert!(scoped_container().resolve::<Shared>().is_ok());
}

#[test]
fn module_boundaries_reject_duplicate_modules_and_private_dependencies() {
    use xecute::{ModuleDefinition, ModuleGraph};
    assert!(matches!(
        ModuleGraph::new(vec![
            ModuleDefinition::new("same"),
            ModuleDefinition::new("same")
        ])
        .build(),
        Err(InjectError::DuplicateModule("same"))
    ));
    let owner = ModuleDefinition::new("owner").provider(Def::value(Shared));
    let consumer = ModuleDefinition::new("consumer")
        .import("owner")
        .provider(Def::singleton::<Worker, _>(
            vec![Key::of::<Shared>()],
            |_| unreachable!(),
        ));
    assert!(matches!(
        ModuleGraph::new(vec![owner, consumer]).build(),
        Err(InjectError::PrivateProviderAccess { .. })
    ));
}
