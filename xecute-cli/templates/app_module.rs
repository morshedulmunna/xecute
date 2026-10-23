//! Root dependency-injection module and generated resource registrations.
//! Keep the xecute markers intact so CLI generators can register new modules.

use axum::Router;
use xecute::{InjectError, ModuleGraph, module};
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

    /// Resolve registered controllers and compose their HTTP routes.
    pub fn router() -> Result<Router, InjectError> {
        let container = ModuleGraph::new(vec![
            Self::definition(),
            // xecute:definitions
        ])
        .build()?;
        let _ = &container;
        let router = Router::new();
        // xecute:routes
        Ok(router)
    }
}
