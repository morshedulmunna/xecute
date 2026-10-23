//! Standalone business logic; register this service in a module before injecting it.

use xecute::injectable;

/// Singleton service constructed by the dependency-injection container.
#[injectable]
pub struct __TYPE__Service {}

impl __TYPE__Service {
    /// Collection skeleton; replace with the feature's business operations.
    pub fn list(&self) -> Vec<String> {
        Vec::new()
    }
}
