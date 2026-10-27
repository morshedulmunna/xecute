//! Queue provider and dependency-injection registration for the __FEATURE__ worker.

use std::time::Duration;
use xecute::{
    ModuleDefinition, ProviderDefinition,
    jobs::{JobQueue, QueueConfig},
};

use super::job::{__TYPE__Job, handle};

/// Dependency-injection module owning the __FEATURE__ background queue.
pub struct __TYPE__Module;

impl __TYPE__Module {
    /// Return the unique module name used for registration.
    pub fn module_name() -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Build the queue provider using the generated capacity and retry defaults.
    pub fn provider() -> ProviderDefinition {
        let config = QueueConfig::new(64, 4)
            .and_then(|config| config.retry(3, Duration::from_millis(250)))
            .expect("generated worker configuration is valid");
        JobQueue::provider(config, handle)
    }

    /// Describe this module and export its queue for dependency injection.
    pub fn definition() -> ModuleDefinition {
        ModuleDefinition::new(Self::module_name())
            .provider(Self::provider())
            .export::<JobQueue<__TYPE__Job>>()
    }
}
