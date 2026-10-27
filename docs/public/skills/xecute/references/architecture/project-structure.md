Use this directory map to find the file you need to edit. Start in src/modules for feature work; use src/server for shared HTTP behavior.

A generated MVC project contains:

```text
my-app/
├── .xecute/architecture
├── config/
│   └── AppConfig.toml
├── .env.example
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── Dockerfile
├── .github/workflows/ci.yml
└── src/
    ├── main.rs
    ├── app_module.rs
    ├── middleware.rs           # Global middleware hook
    ├── configs/               # Validated TOML settings and secret readers
    ├── server/
    │   ├── bootstrap.rs       # Listener and graceful shutdown
    │   ├── router.rs          # Shared HTTP policies and probes
    │   ├── docs.rs            # Swagger/OpenAPI endpoints
    │   ├── error.rs           # Public error envelope
    │   ├── health.rs
    │   ├── logging.rs
    │   ├── rate_limit.rs
    │   ├── shutdown.rs
    │   └── middleware/        # Built-in guards, logging, response handling
    ├── modules/
    │   ├── mod.rs
    │   └── example/
    │       ├── mod.rs
    │       ├── module.rs
    │       ├── controller.rs
    │       ├── service.rs
    │       ├── dto/mod.rs
    │       ├── entity/mod.rs
    │       ├── repository/
    │       │   ├── mod.rs
    │       │   └── store.rs
    │       └── tests/
    │           ├── mod.rs
    │           └── http.rs
    └── tests/                 # Shared server HTTP tests
```

Feature filenames are `controller.rs`, `module.rs`, and `service.rs`; the Rust
type names retain the feature prefix. Resource HTTP tests live in
`src/modules/<feature>/tests/http.rs`.

## Keep learning

[MVC and CQRS](mvc-cqrs.md). Return to the [learning path](../index.md) whenever you need a different topic.