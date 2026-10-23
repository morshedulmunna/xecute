//! Verify macro-generated factories and module registrations together.

use std::sync::Arc;

use xecute::{Container, ModuleGraph};

#[xecute::injectable]
struct Repository {}

#[xecute::injectable]
struct Service {
    repository: Arc<Repository>,
    optional: Option<Arc<MissingOptional>>,
}

#[xecute::injectable]
struct MissingOptional {}

#[xecute::module(
    imports = [],
    providers = [Repository, Service],
    controllers = [],
    exports = [Service],
)]
struct TestModule;

#[test]
fn macros_generate_resolvable_provider_and_module_metadata() {
    let container = Container::builder()
        .injectable::<Repository>()
        .injectable::<Service>()
        .build()
        .unwrap();
    let service = container.resolve::<Service>().unwrap();
    assert!(Arc::strong_count(&service.repository) >= 1);
    assert!(service.optional.is_none());

    let graph = ModuleGraph::new(vec![TestModule::definition()]);
    assert!(graph.build().is_ok());
}

#[derive(xecute::Injectable)]
struct StatefulRepository {
    #[inject(default)]
    records: std::sync::Mutex<Vec<String>>,
}

#[derive(xecute::Injectable)]
struct RepositoryService {
    repository: Arc<StatefulRepository>,
}

#[test]
fn injectable_repository_owns_default_state_and_is_shared_by_services() {
    let container = Container::builder()
        .injectable::<StatefulRepository>()
        .injectable::<RepositoryService>()
        .build()
        .unwrap();
    let service = container.resolve::<RepositoryService>().unwrap();
    let repository = container.resolve::<StatefulRepository>().unwrap();
    assert!(Arc::ptr_eq(&service.repository, &repository));
    assert!(<StatefulRepository as xecute::Provider>::dependencies().is_empty());
    repository.records.lock().unwrap().push("Persisted".into());
    assert_eq!(service.repository.records.lock().unwrap()[0], "Persisted");

    let legacy = xecute::Injector::new()
        .resolve::<StatefulRepository>()
        .unwrap();
    assert!(legacy.records.lock().unwrap().is_empty());
}
