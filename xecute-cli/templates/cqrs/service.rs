//! Shared business operations used by command and query handlers.

use std::sync::Arc;
use xecute::prelude::*;
use super::super::{
    entity::__TYPE__,
    repository::{__TYPE__Repository, CapacityExceeded},
};

/// Singleton business service shared by this feature's handlers.
#[derive(Injectable)]
pub struct __TYPE__Service {
    repository: Arc<__TYPE__Repository>,
}

impl __TYPE__Service {
    /// Retrieve the current collection.
    pub async fn list(&self) -> Vec<__TYPE__> {
        self.repository.list().await
    }

    /// Persist a name after controller validation, applying business normalization.
    pub async fn create(&self, name: String) -> Result<__TYPE__, CapacityExceeded> {
        self.repository.create(name.trim().to_owned()).await
    }
}
