# Cargo feature reference

The runtime's default feature set is empty. Enable only the optional modules used
by your application. Async lifecycle, DI, controllers, custom guards, middleware,
configuration, error envelopes and versioning are part of the base runtime.

| Flag | Enables | Required setup |
| --- | --- | --- |
| `auth` | JWT verification, verified principal and role/permission policies | Trusted key/issuer/audience and guards |
| `postgres` | SQLx PostgreSQL pool/migrations/transactions | URL, migrations and repository/provider registration |
| `jobs` | In-process workers and interval scheduling | Queue limits, handlers and lifecycle draining |
| `cache` | Bounded memory TTL/LRU cache | Explicit keys, value limits and TTLs |
| `cache-redis` | Redis backend; includes `cache` | Redis URL, namespace, timeout and provider |
| `cqrs` | Typed command/query/event buses and behaviors | Typed messages/handlers and configured registries |
| `testing` | Isolated TestingModule and in-process HTTP client | Usually enable in dev-dependencies |
| `upload` | Streaming multipart and private local storage | Upload policy/storage and HTTP body limits |
| `upload-s3` | S3 storage adapter; includes `upload` | Bucket/region and trusted AWS configuration |

## Enable features in your application

Add the flags you need to the runtime dependency. Keep the same dependency version
in normal and dev dependencies; enable `testing` for test helpers.

```toml
[dependencies]
xecute = { version = "0.1", features = ["auth", "cqrs"] }

[dev-dependencies]
xecute = { version = "0.1", features = ["testing"] }
```

Cargo unifies feature selections for the same dependency source. Keep runtime and
dev-dependency sources consistent. In generated workspaces, declare the source in
`[workspace.dependencies]` and select app/test features through inherited entries.

## Unsupported assumptions

Feature flags expose runtime APIs; they do not automatically add configured
modules, change controller flow, create database tables or protect endpoints.
There are no ORM-specific Diesel/SeaORM/MongoDB flags, WebSocket gateways, remote
message transports, automatic HTTP cache decorators or persisted cron/saga
features in this implementation. Compose other Rust libraries explicitly when
those capabilities are required.

The docs validation script compares this table with the actual runtime manifest
so stale or invented flag names are caught during documentation checks.
