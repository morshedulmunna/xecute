//! Create operation and its handler.

use std::sync::Arc;
use xecute::prelude::*;
use super::super::{entity::__TYPE__, repository::{__TYPE__Repository, CapacityExceeded}};

/// Intent to create a record, constructed after HTTP DTO validation.
pub struct Create__TYPE__Command {
    /// Display name to persist.
    pub name: String,
}

/// Coordinates writes and applies business normalization before persistence.
#[derive(Injectable)]
pub struct Create__TYPE__Handler {
    repository: Arc<__TYPE__Repository>,
}

impl Create__TYPE__Handler {
    /// Persist the requested record.
    pub async fn execute(&self, command: Create__TYPE__Command) -> Result<__TYPE__, CapacityExceeded> {
        self.repository.create(command.name.trim().to_owned()).await
    }
}
