//! Parameterized SQL and a multi-write transaction; application logic stays in handlers/services.
use std::sync::Arc;
use xecute::{
    ProviderDefinition, ProviderKey,
    database::{PostgresDatabase, sqlx},
};

pub struct UserRepository {
    database: Arc<PostgresDatabase>,
}

impl UserRepository {
    pub fn provider() -> ProviderDefinition {
        ProviderDefinition::singleton::<Self, _>(
            vec![ProviderKey::of::<PostgresDatabase>()],
            |container| {
                Ok(Self {
                    database: container.resolve()?,
                })
            },
        )
    }

    // All writes share ONE transaction connection, never the pool executor.
    pub async fn create_with_audit(&self, name: &str) -> Result<i64, sqlx::Error> {
        let mut transaction = self.database.begin().await?;
        let id: i64 =
            sqlx::query_scalar("INSERT INTO xecute_example_users (name) VALUES ($1) RETURNING id")
                .bind(name)
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::query("INSERT INTO xecute_example_audit (user_id, action) VALUES ($1, $2)")
            .bind(id)
            .bind("created")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(id)
    }

    pub async fn find_name(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT name FROM xecute_example_users WHERE id = $1")
            .bind(id)
            .fetch_optional(self.database.pool())
            .await
    }
}
