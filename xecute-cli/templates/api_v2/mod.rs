//! Independent version-two routes and metadata.
mod controller;
use axum::Router;
use std::sync::Arc;
use controller::StatusController;

/// Version-two example boundary; it does not inherit version-one routes.
pub struct ApiV2Module;
impl ApiV2Module {
    /// Build this version's own controllers.
    pub fn router() -> Router { StatusController::router(Arc::new(StatusController)) }
    /// Return only this version's declared API contracts.
    pub fn api_operations() -> Vec<xecute::ApiOperation> { StatusController::api_operations() }
}
