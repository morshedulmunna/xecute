//! Provider registrations and exports for this feature.
//!
//! Import another feature module to inject its exported providers (NestJS style):
//! ```ignore
//! use crate::modules::auth::AuthModule;
//! // Add AuthModule to imports below. AuthModule must export AuthService.
//! // Then add `auth: Arc<AuthService>` to your service/controller.
//! ```
//! Register AuthModule::definition() in AppModule::definitions() as well;
//! generated resources are registered there automatically. Keep each provider in
//! its owning module instead of registering the imported service again.

use super::repository::__TYPE__Repository;
use super::{__TYPE__Controller, __TYPE__Service};
use xecute::module;

/// Resource dependency-injection module.
#[module(
    imports = [
        // Other feature modules, e.g. AuthModule (see import guide above).
    ],
    providers = [__TYPE__Repository, __TYPE__Service, __TYPE__Controller],
    controllers = [__TYPE__Controller],
    exports = [__TYPE__Service],
)]
pub struct __TYPE__Module;
