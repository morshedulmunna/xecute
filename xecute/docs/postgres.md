# PostgreSQL integration

Xecute's optional `postgres` feature integrates SQLx 0.9 with dependency injection
and async application lifecycle. It provides a shared connection pool, validated
configuration, startup migrations, readiness checks, and graceful pool cleanup.
Repositories use native SQLx queries and transactions; Xecute does not add an ORM.

Enable the runtime's `postgres` feature:

```toml
[dependencies]
xecute = { version = "0.1", features = ["postgres"] }
```

`postgres` is disabled by default. You do not need SQLx as a separate dependency:
use `xecute::database::sqlx`. No database URL is needed at compile time because
this integration uses parameterized runtime queries, not schema-checked macros.
MVC and CQRS generators still produce in-memory repositories; database adoption
is explicit and existing generated application files are not rewritten.

## Run the complete example

From the repository root, start a disposable development PostgreSQL instance:

```sh
docker run --detach --rm --name xecute-postgres-demo \
  -e POSTGRES_USER=xecute \
  -e POSTGRES_PASSWORD=xecute_dev_only \
  -e POSTGRES_DB=xecute_demo \
  -p 127.0.0.1:55432:5432 postgres:17

docker exec xecute-postgres-demo pg_isready -U xecute -d xecute_demo

# Wait for pg_isready to report "accepting connections", then:
export DATABASE_URL='postgres://xecute:xecute_dev_only@127.0.0.1:55432/xecute_demo'
cargo run -p xecute --example postgres --features postgres

# Re-running creates another user; existing records remain in the database.
# Remove the disposable container and its database after the demo:
docker stop xecute-postgres-demo
```

The example applies `examples/postgres/migrations/202610270001_create_users.sql`,
creates a user and audit entry in one transaction, reads the user back, and closes
the pool. Its separate `repository.rs` is reusable as a persistence-layer example.
For durable local development, mount a named Docker volume at PostgreSQL's data
directory instead of using this disposable setup.

## Configuration and secrets

Read `DATABASE_URL` from the environment or a secret manager. Do not put it into
ordinary application TOML, source control, log messages, or HTTP error responses.
Xecute does not automatically load `.env`; generated applications already have an
environment loader, while standalone examples use the process environment.

```rust
use std::time::Duration;
use xecute::database::{PostgresConfig, sqlx::postgres::PgSslMode};

let url = std::env::var("DATABASE_URL")?;
let config = PostgresConfig::new(&url)?
    .pool_limits(0, 10)?
    .acquire_timeout(Duration::from_secs(5))?
    .ssl_mode(PgSslMode::VerifyFull);
```

Defaults are zero minimum connections, ten maximum connections, and a five-second
pool acquisition timeout. Maximum must be positive, minimum cannot exceed maximum,
and acquisition timeout must be positive. Acquisition includes establishing a
connection; it is not a SQL statement execution timeout. Configure PostgreSQL
`statement_timeout` separately when needed. Budget pool sizes across all replicas
against your database's connection limit.

TLS uses SQLx's PostgreSQL settings. For production, use `VerifyFull` with a
trusted server certificate and matching hostname; supply an appropriate CA using
SQLx's connection URL options, such as `sslrootcert`. Local Docker PostgreSQL does
not offer TLS by default, so the local example uses the driver's default settings.
SQL statement logging is disabled by this wrapper. `PostgresConfig` debug output
redacts connection options, and `DatabaseError` exposes only static messages.

## Modules, startup, and shutdown

```rust
use std::path::Path;
use xecute::{ModuleDefinition, ModuleGraph};
use xecute::database::{PostgresConfig, PostgresDatabase, sqlx::migrate::Migrator};

async fn setup() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = PostgresConfig::new(&std::env::var("DATABASE_URL")?)?;
    let migrations = Migrator::new(Path::new("./migrations")).await?;
    let database_module = ModuleDefinition::new("database")
        .provider(PostgresDatabase::provider_with_migrations(config, migrations))
        .export::<PostgresDatabase>();

    // Add your feature's repository/service/handler definitions to this module.
    let users_module = ModuleDefinition::new("users").import("database");
    let container = ModuleGraph::new(vec![database_module, users_module]).build()?;
    container.initialize_async().await?;
    let work: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let database = container.resolve::<PostgresDatabase>()?;
        database.health_check().await?;
        // Serve traffic, then stop accepting work and drain in-flight requests.
        Ok(())
    }.await;
    let cleanup = container.shutdown_async().await;
    work?;
    cleanup?;
    Ok(())
}
```

Register the database once, export it from its owning module, and import that
module wherever a repository requires it. Repository definitions must declare
`ProviderKey::of::<PostgresDatabase>()` as a dependency. Dependent factories run
only after the pool connects and migrations finish. Failed startup prevents
serving and closes the pool if it was constructed. `PostgresDatabase::provider`
omits migrations when your deployment runs them separately.

Generated applications currently use synchronous `AppModule::container()`. For
async resources, build a `ModuleGraph` from `AppModule::definitions()` instead,
await `initialize_async()`, then pass a retained `Arc<Container>` to
`AppModule::router_with_container(container.clone())`. That routing helper remains
usable after async startup. Ensure the exported database module definition and
importing feature definitions are included in the graph.

Retain the container for cleanup on every exit path after successful startup,
including router construction, listener binding, serving, and graceful-shutdown
errors. Capture application work in an async block returning a `Result`, await
`shutdown_async()` afterward, then return the result; the complete example shows
this pattern. There are no async cleanup hooks in `Drop`. Release checked-out
connections and transactions before closing: pool closure waits for them. If
bounded shutdown is required, apply your application's timeout policy explicitly.

## Migration workflow

Use SQLx migration filenames such as `202610270001_create_users.sql`. Version
numbers must be unique and ordered. Applied migrations are recorded in
`_sqlx_migrations`; re-running validates checksums and applies only pending files.
SQLx's PostgreSQL migration locking serializes concurrent migration runners.
Do not edit an applied migration: add a new version instead.

The example loads migration files at runtime. Deploy the migration directory and
resolve its path explicitly, or run migrations as a deployment step before starting
applications. You can also enable SQLx's `macros` feature in your application to
embed migrations with `sqlx::migrate!`; follow SQLx's build-script guidance so new
SQL files trigger recompilation. Xecute does not enable SQLx macros by default.

Run schema-changing migrations with an appropriately privileged deployment role.
Runtime access can use a separate restricted application role with
`PostgresDatabase::provider`. There is no automatic schema rollback on application
shutdown. Migration SQL is trusted application code; backup and rollback strategy
remain deployment responsibilities.

## Repository and transaction boundaries

Keep the application flow:

```text
MVC:  Controller → Service → Repository → PostgreSQL
CQRS: Controller → Command/Query Handler → Repository → PostgreSQL
```

Bind user input with `$1`, `$2`, and `.bind(value)`. Never interpolate user input
into a SQL string. Methods on the repository return application entities or DTOs;
controllers should not contain SQL.

```rust
use xecute::database::{PostgresDatabase, sqlx};

async fn create_user(db: &PostgresDatabase, name: &str) -> Result<i64, sqlx::Error> {
    let mut tx = db.begin().await?;
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO xecute_example_users (name) VALUES ($1) RETURNING id",
    ).bind(name).fetch_one(&mut *tx).await?;

    sqlx::query(
        "INSERT INTO xecute_example_audit (user_id, action) VALUES ($1, $2)",
    ).bind(id).bind("created").execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(id)
}
```

All participating writes must execute against `&mut *tx`, not `db.pool()`, or they
will not share the transaction. Call `commit()` on success or `rollback()` when
handling an error explicitly. Dropping an uncommitted SQLx transaction schedules
rollback. Avoid holding a transaction across unrelated network calls.

For a use case spanning multiple repositories, the handler/service should own the
transaction and pass `&mut PgConnection` (obtained via `&mut *tx`) to each repository
method. Starting a new transaction inside every repository cannot make multiple
repositories' writes atomic. Automatic transaction propagation, distributed
transactions, and automatic retries are not provided.

## Readiness and error handling

Use `health_check().await` in a readiness endpoint and return HTTP 503 when the
database is unavailable. Keep liveness independent of PostgreSQL connectivity.
The generated readiness route is not changed automatically.

`DatabaseError` redacts configuration, connection, migration, and readiness errors.
Lifecycle registration maps setup failures to static `InjectError` messages.
`begin()` and native SQLx repository queries retain SQLx errors for application
handling, including SQLSTATE codes for unique/foreign-key violations. Map these
to domain errors and safe HTTP responses; never serialize raw driver errors.

## Tests and CI

Normal feature-enabled tests include configuration validation, redaction, and a
bounded connection-attempt test that requires no database:

```sh
cargo test -p xecute --features postgres,testing --test postgres
```

Live integration tests are explicitly ignored in the default suite. They fail
with a clear message if explicitly invoked without `TEST_DATABASE_URL`; they do
not silently pass when no database is available.

```sh
# Use the disposable instance from the example setup; its role can CREATE DATABASE.
export TEST_DATABASE_URL="$DATABASE_URL"
cargo test -p xecute --features postgres,testing --test postgres -- --ignored
```

Each live test creates its own database with a generated name, closes its pools,
and drops only that database. The configured role requires `CREATEDB`; use a
dedicated local/CI instance. The supplied database is used only for provisioning.
If a test panics, discard the disposable PostgreSQL instance afterward.

Coverage includes persistent repository writes, parameter binding, audit atomicity,
commit, explicit and dropped rollback, migration ordering, idempotency, checksum
mismatch, concurrent migration locking, failed-startup cleanup, acquisition limits,
readiness, closed-pool behavior, module DI, and HTTP injection. The repository's
`postgres` CI job runs the live suite against a PostgreSQL service.

SQLx reference: [pool options](https://docs.rs/sqlx/latest/sqlx/pool/struct.PoolOptions.html),
[migrations](https://docs.rs/sqlx/latest/sqlx/migrate/struct.Migrator.html),
[transactions](https://docs.rs/sqlx/latest/sqlx/struct.Transaction.html), and
[parameter binding](https://docs.rs/sqlx/latest/sqlx/fn.query.html).
