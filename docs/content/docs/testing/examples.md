---
title: "Runnable examples"
description: "Find feature flags, commands and external services for each example."
---

Run these commands from the repository root. Each example is implemented under
`xecute/examples`; the associated tests compile its application setup. Clone the repository to run
these examples; applications can enable the same APIs through their Cargo features.

| Example | Command | Requirements |
| --- | --- | --- |
| Module imports and request inputs | `cargo run -p xecute --example module_imports` | HTTP port 3000 |
| Async resource lifecycle | `cargo run -p xecute --example async_lifecycle` | No external service |
| PostgreSQL repository/transactions | `cargo run -p xecute --example postgres --features postgres` | Development `DATABASE_URL` |
| JWT guards and permissions | `cargo run -p xecute --example auth --features auth` | Random `JWT_SECRET` of at least 32 bytes; port 3000 |
| Worker with TTL cache | `cargo run -p xecute --example jobs_cache --features jobs,cache` | No external service |
| CQRS buses and event subscribers | `cargo run -p xecute --example cqrs --features cqrs` | HTTP port 3000 |
| Validated private file upload | `cargo run -p xecute --example file_upload --features upload` | Private temporary storage; port 3000 |

Run one HTTP example at a time when using the same listening port. Read each
example's module/controller comments for curl commands and behavioral details.
The PostgreSQL example runs migrations and inserts data; use its disposable setup.

## Focused checks

```sh
cargo test -p xecute --example module_imports
cargo test -p xecute --features testing --test async_lifecycle
cargo test -p xecute --features auth --test auth_http --test guards_http --example auth
cargo test -p xecute --features jobs,cache,testing --test jobs --test cache --example jobs_cache
cargo test -p xecute --features cqrs,testing --test cqrs --example cqrs
```

## External-service suites

The [PostgreSQL guide](/docs/integrations/postgres) and
[Redis cache guide](/docs/integrations/cache) explain disposable services and
credentials. Live tests are ignored in the normal suite and fail clearly when
explicitly invoked without the required environment variable.

```sh
# Dedicated database; role needs permission to create test databases:
cargo test -p xecute --features postgres,testing --test postgres -- --ignored

# Dedicated Redis instance; tests use separate random namespaces:
cargo test -p xecute --features cache-redis --test cache_redis -- --ignored
```

Set `TEST_DATABASE_URL` or `TEST_REDIS_URL` as described in those guides before
running the corresponding command. Repository CI has explicit database/Redis
service jobs. No live credential is stored in the docs.

## Documentation snippets versus complete applications

Guide snippets show the relevant API or integration fragment. They are not always
complete standalone programs. For executable application boundaries, use the
complete examples and generated-app tests. A code block documenting a placeholder
attribute does not make it an operational lifecycle/interceptor feature.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/docs/handbook/examples.md).
