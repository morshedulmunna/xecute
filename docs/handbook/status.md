# Features and setup

Xecute gives you a working API first. Add database access, authentication,
background processing, caching, and buses as your application needs them.
Most integrations are opt-in so you control dependencies and configuration.

## What is generated and what do I configure?

| Capability | Generated behavior | Your setup |
| --- | --- | --- |
| MVC resources | Controller → service → repository | Default; add your application logic |
| CQRS resources | Controller → handler → repository | Choose `--cqrs` when creating the project |
| Request validation | Create DTO in `dto/mod.rs` | Add fields and validation rules |
| Search/path/form inputs | Input examples in controller comments | Add DTOs and route handlers |
| Module imports | Imports section and usage guide | Export shared providers and register module definitions |
| Lifecycle | Synchronous startup helpers | Use async startup and cleanup for async resources |
| PostgreSQL | In-memory repository | Enable `postgres`; configure pool, migrations, SQL repository |
| JWT authentication | Public resources | Enable `auth`; configure trusted verification and guards |
| Background jobs | No worker created automatically | Enable `jobs`; register queue, handlers, and shutdown |
| Caching | No automatic response caching | Enable `cache` or `cache-redis`; define keys and TTLs |
| CQRS buses/events | Direct handler calls | Enable `cqrs`; register buses and handlers |

## Enable a capability

For example, enable authentication and CQRS buses in your application manifest:

```toml
[dependencies]
xecute = { version = "0.1", features = ["auth", "cqrs"] }

[dev-dependencies]
xecute = { version = "0.1", features = ["testing"] }
```

Merge the flags with your existing dependency declaration. In a Cargo workspace,
set the version in `[workspace.dependencies]` and select features on inherited
application dependencies. Keep `Cargo.lock` committed for reproducible builds.

Feature flags expose APIs. They do not create credentials, connect external
services, replace repositories, or protect routes automatically. Follow the
[feature reference](/docs/reference/features) and the relevant integration guide.

## Updating an existing application

Updating the CLI changes the executable used for future generation. It does not
rewrite existing source files or migrate application startup.

Use the [adoption guide](/docs/getting-started/adopting-existing) to add capabilities
one at a time, including provider registration, startup, shutdown, and tests.
