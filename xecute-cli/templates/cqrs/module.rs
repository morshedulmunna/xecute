//! Command and query handler registrations for this feature.

use super::commands::Create__TYPE__Handler;
use super::queries::List__TYPE__Handler;
use super::repository::__TYPE__Repository;
use super::services::__TYPE__Service;
use super::__TYPE__Controller;
use xecute::module;

/// Resource dependency-injection module.
#[module(
    providers = [__TYPE__Repository, __TYPE__Service, Create__TYPE__Handler, List__TYPE__Handler, __TYPE__Controller],
    controllers = [__TYPE__Controller],
    exports = [__TYPE__Service, Create__TYPE__Handler, List__TYPE__Handler],
)]
pub struct __TYPE__Module;
