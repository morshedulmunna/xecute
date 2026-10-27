//! Queue provider and dependency-injection registration for the __FEATURE__ worker.

use xecute::{ModuleDefinition, ProviderDefinition};

use super::broker::{__TYPE__Queue, start};

/// Dependency-injection module owning the __FEATURE__ distributed queue.
pub struct __TYPE__Module;

impl __TYPE__Module {
    /// Return the unique module name used for registration.
    pub fn module_name() -> &'static str {
        std::any::type_name::<Self>()
    }

    /// Build the broker-backed queue provider.
    pub fn provider() -> ProviderDefinition {
        ProviderDefinition::singleton_async::<__TYPE__Queue, _, _>(vec![], |_| start())
            .on_shutdown::<__TYPE__Queue, _, _>(|queue| async move {
                queue.shutdown().await;
                Ok(())
            })
    }

    /// Describe this module and export its queue for dependency injection.
    pub fn definition() -> ModuleDefinition {
        ModuleDefinition::new(Self::module_name())
            .provider(Self::provider())
            .export::<__TYPE__Queue>()
    }
}
