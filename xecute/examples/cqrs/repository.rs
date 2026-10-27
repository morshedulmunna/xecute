//! Bounded demonstration storage; replace with the PostgreSQL adapter for persistence.
use super::entity::User;
use xecute::Injectable;
#[derive(Injectable)]
pub struct UserRepository {
    #[inject(default)]
    users: tokio::sync::RwLock<Vec<User>>,
}
impl UserRepository {
    pub async fn create(&self, name: String) -> Option<User> {
        let mut users = self.users.write().await;
        if users.len() >= 1000 {
            return None;
        }
        let user = User {
            id: uuid::Uuid::new_v4().to_string(),
            name,
        };
        users.push(user.clone());
        Some(user)
    }
    pub async fn list(&self) -> Vec<User> {
        self.users.read().await.clone()
    }
}
