//! Command and query handler registrations for this feature.
//!
//! Import another feature module to inject its exported providers (NestJS style):
//! ```ignore
//! use crate::modules::auth::AuthModule;
//! // Add AuthModule to imports below. AuthModule must export AuthService.
//! // Then add `auth: Arc<AuthService>` to a handler or controller.
//! ```
//! Register AuthModule::definition() in AppModule::definitions() as well;
//! generated resources are registered there automatically. Keep each provider in
//! its owning module instead of registering the imported service again.

use super::commands::Create__TYPE__Handler;
use super::queries::List__TYPE__Handler;
use super::repository::__TYPE__Repository;
use super::__TYPE__Controller;
use xecute::module;

/// Resource dependency-injection module.
/// Add `#[xecute::guard(your_guard)]` below `#[module(...)]` to protect all
/// controllers in this module. The generated app mounts this module's scoped router.
#[module(
    imports = [
        // Other feature modules, e.g. AuthModule (see import guide above).
    ],
    providers = [__TYPE__Repository, Create__TYPE__Handler, List__TYPE__Handler, __TYPE__Controller],
    controllers = [__TYPE__Controller],
    exports = [Create__TYPE__Handler, List__TYPE__Handler],
)]
pub struct __TYPE__Module;
