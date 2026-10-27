//! PostgreSQL integration. Enable `postgres`; register a lifecycle-managed pool,
//! await container initialization, and drain requests before explicit shutdown.
//! SQLx is re-exported so repositories can use native queries and transactions.

use crate::{InjectError, ProviderDefinition};
use sqlx::{
    ConnectOptions, PgPool, Postgres, Transaction,
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use std::{fmt, str::FromStr, sync::Arc, time::Duration};

/// Native SQLx queries, row types, migrations, and transaction APIs.
pub use sqlx;

/// Redacted setup/readiness errors. No connection strings or driver messages are exposed.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DatabaseError {
    /// An invalid setting, described without its value.
    InvalidConfiguration(&'static str),
    /// A connection could not be established within the configured acquisition timeout.
    Connection,
    /// Migration execution or checksum validation failed.
    Migration,
    /// The readiness probe could not acquire and query a connection.
    Unavailable,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(reason) => {
                write!(f, "invalid PostgreSQL configuration: {reason}")
            }
            Self::Connection => f.write_str("PostgreSQL connection failed"),
            Self::Migration => f.write_str("PostgreSQL migration failed"),
            Self::Unavailable => f.write_str("PostgreSQL unavailable"),
        }
    }
}
impl std::error::Error for DatabaseError {}

/// Validated connection settings. Debug output deliberately excludes connection options.
#[derive(Clone)]
pub struct PostgresConfig {
    options: PgConnectOptions,
    min_connections: u32,
    max_connections: u32,
    acquire_timeout: Duration,
}

impl fmt::Debug for PostgresConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresConfig")
            .field("connection", &"[REDACTED]")
            .field("min_connections", &self.min_connections)
            .field("max_connections", &self.max_connections)
            .field("acquire_timeout", &self.acquire_timeout)
            .finish()
    }
}

impl PostgresConfig {
    /// Parse a PostgreSQL URL without connecting. Defaults: 0–10 connections and
    /// a five-second acquisition timeout. SQL statement logging is disabled.
    pub fn new(url: &str) -> Result<Self, DatabaseError> {
        if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
            return Err(DatabaseError::InvalidConfiguration(
                "expected a PostgreSQL URL",
            ));
        }
        let options = PgConnectOptions::from_str(url)
            .map_err(|_| DatabaseError::InvalidConfiguration("malformed connection URL"))?
            .disable_statement_logging();
        Ok(Self {
            options,
            min_connections: 0,
            max_connections: 10,
            acquire_timeout: Duration::from_secs(5),
        })
    }

    /// Set connection pool limits. Maximum must be positive and at least the minimum.
    pub fn pool_limits(mut self, min: u32, max: u32) -> Result<Self, DatabaseError> {
        if max == 0 || min > max {
            return Err(DatabaseError::InvalidConfiguration(
                "require 0 <= min <= max and max > 0",
            ));
        }
        self.min_connections = min;
        self.max_connections = max;
        Ok(self)
    }

    /// Bound pool acquisition, including establishing a connection. Must be positive.
    pub fn acquire_timeout(mut self, timeout: Duration) -> Result<Self, DatabaseError> {
        if timeout.is_zero() {
            return Err(DatabaseError::InvalidConfiguration(
                "acquisition timeout must be positive",
            ));
        }
        self.acquire_timeout = timeout;
        Ok(self)
    }

    /// Select SQLx TLS behavior; use VerifyFull and an appropriate CA for production.
    #[must_use]
    pub fn ssl_mode(mut self, mode: PgSslMode) -> Self {
        self.options = self.options.ssl_mode(mode);
        self
    }
}

/// One shared pool per container, suitable for injection into MVC services or CQRS repositories.
pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    /// Establish a pool and verify connectivity. Direct callers own migration and cleanup.
    pub async fn connect(config: PostgresConfig) -> Result<Self, DatabaseError> {
        let pool = PgPoolOptions::new()
            .min_connections(config.min_connections)
            .max_connections(config.max_connections)
            .acquire_timeout(config.acquire_timeout)
            .connect_with(config.options)
            .await
            .map_err(|_| DatabaseError::Connection)?;
        Ok(Self { pool })
    }

    /// Register an async singleton and its pool-closing shutdown hook.
    /// Add this definition to a ModuleDefinition or ContainerBuilder.
    #[must_use]
    pub fn provider(config: PostgresConfig) -> ProviderDefinition {
        Self::managed_provider(config, None)
    }

    /// Register a pool whose migrations finish before dependent providers initialize.
    /// Migration failure rolls back startup and closes the newly created pool.
    #[must_use]
    pub fn provider_with_migrations(
        config: PostgresConfig,
        migrations: Migrator,
    ) -> ProviderDefinition {
        Self::managed_provider(config, Some(Arc::new(migrations)))
    }

    fn managed_provider(
        config: PostgresConfig,
        migrations: Option<Arc<Migrator>>,
    ) -> ProviderDefinition {
        ProviderDefinition::singleton_async::<Self, _, _>(vec![], move |_| {
            let config = config.clone();
            async move {
                Self::connect(config)
                    .await
                    .map_err(|_| InjectError::ConstructionFailed("PostgreSQL connection failed"))
            }
        })
        .on_init::<Self, _, _>(move |database| {
            let migrations = migrations.clone();
            async move {
                if let Some(migrations) = migrations {
                    database.migrate(&migrations).await.map_err(|_| {
                        InjectError::ConstructionFailed("PostgreSQL migration failed")
                    })?;
                }
                Ok(())
            }
        })
        .on_shutdown::<Self, _, _>(|database| async move {
            database.close().await;
            Ok(())
        })
    }

    /// Borrow the pool for parameterized queries. Cloned pools share connections and closure.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Start a native SQLx transaction. Commit explicitly; dropping it schedules rollback.
    /// Driver errors are intended for application handling, never direct HTTP responses.
    pub async fn begin(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }

    /// Apply pending migrations and validate applied checksums using SQLx's migration lock.
    pub async fn migrate(&self, migrations: &Migrator) -> Result<(), DatabaseError> {
        migrations
            .run(&self.pool)
            .await
            .map_err(|_| DatabaseError::Migration)
    }

    /// Query the database for readiness; a listening HTTP socket alone does not imply DB readiness.
    pub async fn health_check(&self) -> Result<(), DatabaseError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|_| DatabaseError::Unavailable)?;
        Ok(())
    }

    /// Stop new acquisitions and wait for checked-out connections to return.
    /// Finish requests and release all transactions before awaiting this method.
    pub async fn close(&self) {
        self.pool.close().await;
    }
}
