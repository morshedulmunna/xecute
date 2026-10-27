//! DATABASE_URL=postgres://... cargo run -p xecute --example postgres --features postgres
//! Applies bundled migrations and inserts one user + audit entry. Use a development database.
mod repository;

use repository::UserRepository;
use std::{path::Path, sync::Arc};
use xecute::{
    ModuleDefinition, ModuleGraph,
    database::{PostgresConfig, PostgresDatabase, sqlx::migrate::Migrator},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Do not log connection strings or return raw driver errors to HTTP clients.
    let url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required")?;
    let config = PostgresConfig::new(&url)?.pool_limits(0, 5)?;
    // Directory migration sources must be shipped with the deployed application.
    let migrations = Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/postgres/migrations"
    )))
    .await?;
    let database = ModuleDefinition::new("database")
        .provider(PostgresDatabase::provider_with_migrations(
            config, migrations,
        ))
        .export::<PostgresDatabase>();
    let users = ModuleDefinition::new("users")
        .import("database")
        .provider(UserRepository::provider());
    let container = Arc::new(ModuleGraph::new(vec![database, users]).build()?);
    container.initialize_async().await?;
    let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let repository = container.resolve::<UserRepository>()?;
        let id = repository
            .create_with_audit("Munna")
            .await
            .map_err(|_| "user creation failed")?;
        let name = repository
            .find_name(id)
            .await
            .map_err(|_| "user lookup failed")?;
        println!("Created user {id}: {name:?}");
        Ok(())
    }
    .await;
    // Cleanup also runs if application work fails. In an HTTP server, drain requests first.
    let cleanup = container.shutdown_async().await;
    result?;
    cleanup?;
    Ok(())
}
