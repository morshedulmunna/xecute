---
title: "Cargo feature reference"
description: "Enable only the runtime capabilities an application needs."
---

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

## Source versus published runtime

The auth, postgres, jobs, cache, cache-redis and cqrs features are currently local
source additions. Async lifecycle is also a local addition without a feature flag.
The manifest version alone does not identify these unreleased changes. Read
[release status](/docs/getting-started/status) and use a local dependency when
trying them.

```toml
[dependencies]
xecute = { path = "/path/to/repo/xecute", features = ["auth", "cqrs"] }

[dev-dependencies]
xecute = { path = "/path/to/repo/xecute", features = ["testing"] }
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

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/docs/handbook/features.md).
