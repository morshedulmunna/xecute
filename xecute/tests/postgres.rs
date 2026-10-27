//! Unit tests run normally. Real PostgreSQL tests are explicit and require a
//! dedicated TEST_DATABASE_URL whose user can CREATE DATABASE.
#![cfg(feature = "postgres")]

use std::{path::Path, sync::Arc, time::Duration};
use xecute::{
    Container, InjectError, ModuleDefinition, ModuleGraph,
    database::{
        DatabaseError, PostgresConfig, PostgresDatabase,
        sqlx::{self, migrate::Migrator},
    },
};

#[path = "../examples/postgres/repository.rs"]
mod repository;
use repository::UserRepository;

#[test]
fn validates_configuration_and_redacts_connection_details() {
    for url in [
        "",
        "sqlite://db",
        "mysql://root:secret@host/db",
        "postgres://host:bad/db",
    ] {
        let error = PostgresConfig::new(url).unwrap_err();
        assert!(matches!(error, DatabaseError::InvalidConfiguration(_)));
        assert!(!format!("{error:?} {error}").contains("secret"));
    }
    let config =
        PostgresConfig::new("postgres://private_user:private_password@private_host/private_db")
            .unwrap();
    let debug = format!("{config:?}");
    for secret in [
        "private_user",
        "private_password",
        "private_host",
        "private_db",
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(config.clone().pool_limits(0, 0).is_err());
    assert!(config.clone().pool_limits(3, 2).is_err());
    assert!(config.clone().pool_limits(0, 1).is_ok());
    assert!(config.acquire_timeout(Duration::ZERO).is_err());
}

#[tokio::test]
async fn connection_attempt_is_bounded_and_error_does_not_expose_url() {
    // Accept TCP but never complete the PostgreSQL handshake.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "postgres://test:secret_password@{}/test",
        listener.local_addr().unwrap()
    );
    let config = PostgresConfig::new(&url)
        .unwrap()
        .acquire_timeout(Duration::from_millis(50))
        .unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), PostgresDatabase::connect(config))
        .await
        .unwrap();
    assert!(matches!(result, Err(DatabaseError::Connection)));
    let error = result.err().unwrap();
    assert!(!format!("{error:?} {error}").contains("secret_password"));
}

struct Fixture {
    admin: sqlx::PgPool,
    name: String,
    config: PostgresConfig,
}

impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("TEST_DATABASE_URL").expect(
            "set TEST_DATABASE_URL to a disposable PostgreSQL database; role requires CREATEDB",
        );
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .expect("test PostgreSQL connection failed");
        // Identifier consists only of a fixed prefix and generated hexadecimal digits.
        let name = format!("xecute_test_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
            .execute(&admin)
            .await
            .unwrap();
        let mut url = url::Url::parse(&url).expect("invalid TEST_DATABASE_URL");
        url.set_path(&format!("/{name}"));
        let config = PostgresConfig::new(url.as_str())
            .unwrap()
            .pool_limits(0, 2)
            .unwrap();
        Self {
            admin,
            name,
            config,
        }
    }

    async fn cleanup(self) {
        // Drop only the database created by this fixture, never the supplied URL's database.
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE {}", self.name)))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}

async fn migrations() -> Migrator {
    Migrator::new(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/postgres/migrations"
    )))
    .await
    .unwrap()
}

async fn broken_migrations() -> Migrator {
    let path = std::env::temp_dir().join(format!(
        "xecute-broken-migrations-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir(&path).unwrap();
    std::fs::write(
        path.join("1_broken.sql"),
        "CREATE TABLE must_rollback (id BIGINT); THIS IS NOT VALID SQL;",
    )
    .unwrap();
    let migrator = Migrator::new(path.as_path()).await.unwrap();
    std::fs::remove_dir_all(path).unwrap();
    migrator
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn migrations_precede_repository_and_parameterized_writes_persist_across_reconnect() {
    let fixture = Fixture::new().await;
    let container = ModuleGraph::new(vec![
        ModuleDefinition::new("database")
            .provider(PostgresDatabase::provider_with_migrations(
                fixture.config.clone(),
                migrations().await,
            ))
            .export::<PostgresDatabase>(),
        ModuleDefinition::new("users")
            .import("database")
            .provider(UserRepository::provider()),
    ])
    .build()
    .unwrap();
    container.initialize_async().await.unwrap();
    let database = container.resolve::<PostgresDatabase>().unwrap();
    let repository = container.resolve::<UserRepository>().unwrap();
    // SQL-like text must be stored as data, not interpolated into SQL.
    let name = "Munna'); DROP TABLE xecute_example_users; --";
    let id = repository.create_with_audit(name).await.unwrap();
    assert_eq!(
        repository.find_name(id).await.unwrap().as_deref(),
        Some(name)
    );
    assert!(repository.find_name(-1).await.unwrap().is_none());
    let audit: i64 =
        sqlx::query_scalar("SELECT count(*) FROM xecute_example_audit WHERE user_id = $1")
            .bind(id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(audit, 1);
    // Re-running migrations validates checksums and does not recreate tables.
    database.migrate(&migrations().await).await.unwrap();
    container.shutdown_async().await.unwrap();
    assert!(database.pool().is_closed());
    let reopened = PostgresDatabase::connect(fixture.config.clone())
        .await
        .unwrap();
    let persisted: String =
        sqlx::query_scalar("SELECT name FROM xecute_example_users WHERE id = $1")
            .bind(id)
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert_eq!(persisted, name);
    reopened.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn transactions_commit_rollback_and_drop_without_partial_writes() {
    let fixture = Fixture::new().await;
    let db = PostgresDatabase::connect(fixture.config.clone().pool_limits(0, 1).unwrap())
        .await
        .unwrap();
    db.migrate(&migrations().await).await.unwrap();
    for (name, commit) in [("committed", true), ("rolled back", false)] {
        let mut tx = db.begin().await.unwrap();
        sqlx::query("INSERT INTO xecute_example_users (name) VALUES ($1)")
            .bind(name)
            .execute(&mut *tx)
            .await
            .unwrap();
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
    }
    {
        let mut tx = db.begin().await.unwrap();
        sqlx::query("INSERT INTO xecute_example_users (name) VALUES ('dropped')")
            .execute(&mut *tx)
            .await
            .unwrap();
        // Dropping an uncommitted transaction schedules rollback.
    }
    // A second write failure must also undo the first write in the transaction.
    {
        let mut tx = db.begin().await.unwrap();
        sqlx::query("INSERT INTO xecute_example_users (name) VALUES ('partial')")
            .execute(&mut *tx)
            .await
            .unwrap();
        assert!(
            sqlx::query(
                "INSERT INTO xecute_example_audit (user_id, action) VALUES (-1, 'invalid')"
            )
            .execute(&mut *tx)
            .await
            .is_err()
        );
        tx.rollback().await.unwrap();
    }
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM xecute_example_users ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(names, ["committed"]);
    db.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn failed_migrations_close_pool_and_prevent_dependent_startup() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let fixture = Fixture::new().await;
    let ran = Arc::new(AtomicBool::new(false));
    let called = ran.clone();
    let c = Container::builder()
        .provider(PostgresDatabase::provider_with_migrations(
            fixture.config.clone(),
            broken_migrations().await,
        ))
        .provider(xecute::ProviderDefinition::singleton::<bool, _>(
            vec![xecute::ProviderKey::of::<PostgresDatabase>()],
            move |_| {
                called.store(true, Ordering::SeqCst);
                Ok(true)
            },
        ))
        .build()
        .unwrap();
    assert_eq!(
        c.initialize_async().await,
        Err(InjectError::ConstructionFailed(
            "PostgreSQL migration failed"
        ))
    );
    assert!(!ran.load(Ordering::SeqCst));
    let database = c.resolve::<PostgresDatabase>().unwrap();
    assert!(database.pool().is_closed());
    c.shutdown_async().await.unwrap();
    let reopened = PostgresDatabase::connect(fixture.config.clone())
        .await
        .unwrap();
    let partial: Option<String> = sqlx::query_scalar("SELECT to_regclass('must_rollback')::text")
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert!(
        partial.is_none(),
        "failed migration must roll back its earlier DDL"
    );
    reopened.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn pool_limits_bound_waiters_and_close_rejects_future_queries() {
    let fixture = Fixture::new().await;
    let config = fixture
        .config
        .clone()
        .pool_limits(0, 1)
        .unwrap()
        .acquire_timeout(Duration::from_millis(100))
        .unwrap();
    let database = PostgresDatabase::connect(config).await.unwrap();
    database.health_check().await.unwrap();
    let held = database.pool().acquire().await.unwrap();
    assert_eq!(
        database.health_check().await,
        Err(DatabaseError::Unavailable)
    );
    drop(held);
    database.health_check().await.unwrap();
    database.close().await;
    assert_eq!(
        database.health_check().await,
        Err(DatabaseError::Unavailable)
    );
    assert!(matches!(
        database.begin().await,
        Err(sqlx::Error::PoolClosed)
    ));
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn concurrent_migration_startup_and_checksums_are_enforced() {
    let fixture = Fixture::new().await;
    let a = PostgresDatabase::connect(fixture.config.clone())
        .await
        .unwrap();
    let b = PostgresDatabase::connect(fixture.config.clone())
        .await
        .unwrap();
    let first = migrations().await;
    let second = migrations().await;
    let (left, right) = tokio::join!(a.migrate(&first), b.migrate(&second));
    left.unwrap();
    right.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success")
        .fetch_one(a.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let path =
        std::env::temp_dir().join(format!("xecute-checksum-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("202610270001_create_users.sql"), "SELECT 1;").unwrap();
    let changed = Migrator::new(path.as_path()).await.unwrap();
    std::fs::remove_dir_all(path).unwrap();
    assert_eq!(a.migrate(&changed).await, Err(DatabaseError::Migration));
    a.close().await;
    b.close().await;
    fixture.cleanup().await;
}

#[cfg(feature = "testing")]
#[tokio::test]
#[ignore = "requires disposable PostgreSQL via TEST_DATABASE_URL"]
async fn http_injection_and_testing_module_use_the_live_database() {
    use axum::{Router, routing::get};
    use xecute::{
        Inject,
        testing::{TestApp, TestingModule},
    };
    let fixture = Fixture::new().await;
    let c = Arc::new(
        TestingModule::new()
            .provider(PostgresDatabase::provider_with_migrations(
                fixture.config.clone(),
                migrations().await,
            ))
            .provider(UserRepository::provider())
            .compile_async()
            .await
            .unwrap(),
    );
    let repo = c.resolve::<UserRepository>().unwrap();
    repo.create_with_audit("HTTP user").await.unwrap();
    let app = TestApp::new(xecute::with_container(
        Router::new().route(
            "/ready",
            get(|Inject(db): Inject<PostgresDatabase>| async move {
                db.health_check()
                    .await
                    .map(|()| "ready")
                    .map_err(|_| axum::http::StatusCode::SERVICE_UNAVAILABLE)
            }),
        ),
        c.clone(),
    ));
    let _ = app
        .get("/ready")
        .send()
        .await
        .unwrap()
        .assert_status(axum::http::StatusCode::OK);
    c.shutdown_async().await.unwrap();
    fixture.cleanup().await;
}
