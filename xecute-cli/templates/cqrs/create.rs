//! Create operation and its handler.

use std::sync::Arc;
use xecute::prelude::*;
use super::super::{entity::__TYPE__, repository::CapacityExceeded, services::__TYPE__Service};

/// Intent to create a record, constructed after HTTP DTO validation.
pub struct Create__TYPE__Command {
    /// Display name to persist.
    pub name: String,
}

/// Executes write requests through the shared business service.
#[derive(Injectable)]
pub struct Create__TYPE__Handler {
    service: Arc<__TYPE__Service>,
}

impl Create__TYPE__Handler {
    /// Persist the requested record.
    pub async fn execute(&self, command: Create__TYPE__Command) -> Result<__TYPE__, CapacityExceeded> {
        self.service.create(command.name).await
    }
}
