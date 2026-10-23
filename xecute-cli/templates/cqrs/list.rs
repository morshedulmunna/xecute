//! Collection read operation and its handler.

use std::sync::Arc;
use xecute::prelude::*;
use super::super::{entity::__TYPE__, services::__TYPE__Service};

/// Request a snapshot of the current collection.
pub struct List__TYPE__Query;

/// Executes read requests through the shared business service.
#[derive(Injectable)]
pub struct List__TYPE__Handler {
    service: Arc<__TYPE__Service>,
}

impl List__TYPE__Handler {
    /// Return records without modifying storage.
    pub async fn execute(&self, _query: List__TYPE__Query) -> Vec<__TYPE__> {
        self.service.list().await
    }
}
