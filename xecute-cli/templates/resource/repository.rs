//! Bounded in-memory example; replace with a durable database for deployment.

use super::super::entity::__TYPE__;
use tokio::sync::RwLock;
use xecute::prelude::*;

/// Singleton repository; stored records last only for this process lifetime.
#[derive(Injectable)]
pub struct __TYPE__Repository {
    // Initialize storage locally; database providers can be injected as Arc<Database>.
    #[inject(default)]
    records: RwLock<Vec<__TYPE__>>,
}

/// Storage capacity has been exhausted.
#[derive(Debug)]
pub struct CapacityExceeded;

impl From<CapacityExceeded> for HttpError {
    fn from(_: CapacityExceeded) -> Self {
        HttpError::new(HttpStatus::SERVICE_UNAVAILABLE)
    }
}

impl __TYPE__Repository {
    /// Read a snapshot without holding the lock while serializing a response.
    pub async fn list(&self) -> Vec<__TYPE__> {
        self.records.read().await.clone()
    }

    /// Store a validated name, limiting memory growth to 10,000 records.
    pub async fn create(&self, name: String) -> Result<__TYPE__, CapacityExceeded> {
        let mut records = self.records.write().await;
        if records.len() >= 10_000 {
            return Err(CapacityExceeded);
        }
        let entity = __TYPE__ {
            id: uuid::Uuid::new_v4().to_string(),
            name,
        };
        records.push(entity.clone());
        Ok(entity)
    }
}
