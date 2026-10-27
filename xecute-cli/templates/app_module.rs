//! Root dependency-injection module and generated resource registrations.
//! Keep the xecute markers intact so CLI generators can register new modules.

use axum::Router;
use std::sync::Arc;
use xecute::{Container, InjectError, ModuleDefinition, ModuleGraph, module};
// xecute:modules

/// Root application dependency-injection boundary.
#[module(imports = [
    // xecute:imports
])]
pub struct AppModule;

impl AppModule {
    /// Collect documentation from each registered resource controller.
    pub fn api_operations() -> Vec<xecute::ApiOperation> {
        let operations = Vec::new();
        // xecute:api
        operations
    }

    /// Application module metadata, reusable by isolated test modules.
    pub fn definitions() -> Vec<ModuleDefinition> {
        vec![
            Self::definition(),
            // xecute:definitions
        ]
    }

    /// Validate and eagerly construct singleton providers before listening.
    pub async fn container() -> Result<Container, InjectError> {
        let container = ModuleGraph::new(Self::definitions()).build()?;
        container.initialize_async().await?;
        Ok(container)
    }

    /// Resolve controllers using the configured provider lifecycles.
    pub async fn router() -> Result<Router, InjectError> {
        Self::router_with_container(Arc::new(Self::container().await?)).await
    }

    /// Assemble the real HTTP routes with an application or isolated testing container.
    pub async fn router_with_container(container: Arc<Container>) -> Result<Router, InjectError> {
        container.initialize_async().await?;
        let _ = &container;
        let router = Router::new();
        // xecute:routes
        Ok(xecute::with_container(Self::apply_guards(router), container))
    }
}
