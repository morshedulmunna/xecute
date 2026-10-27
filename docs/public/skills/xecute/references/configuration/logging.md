Start with console logs, then configure file rotation or an exporter when you need them. This guide shows which logging behavior belongs to your generated application.

Logging outputs use these settings in `config/AppConfig.toml`:

```toml
[logging]
stdout_enabled = true
file_dir = "logs"
```

`logging.stdout_enabled` controls JSON console output. A nonempty `logging.file_dir`
enables JSON file output and creates the private directory. Remove it or leave it
empty to disable file output. Generated projects include both settings above.
`logging.filter` continues to filter events across all outputs.

Files use the `app` prefix, daily rotation, and retention of seven matching files.
These defaults live in `src/configs/logging.rs` and can be changed in code.
Setting stdout false and omitting the file directory allows an exporter-only
configuration. An unwritable enabled file destination fails startup.

The file sink uses a dedicated writer thread and an 8192-entry bounded queue.
A full queue applies backpressure rather than silently dropping events. The
`LoggerGuard` returned from initialization is retained in `server/bootstrap.rs`
until normal server shutdown so pending writes are flushed. Forced termination
cannot guarantee pending log delivery. Built-in request events continue to omit
payloads, cookies, authorization headers, and query strings.

The default `/logs/` path is ignored by Git and Docker. Configured log storage
must remain outside the public static directory, even if file output is currently
disabled. For a read-only container with file output enabled, mount private log
storage writable by the application's UID 65532. Console logging can instead be
collected by the deployment's log collector.

## Sentry, Grafana/Loki, OpenTelemetry, and custom exporters

Application modules emit standard `tracing` events, so business code does not need
to change when a new destination is added:

```rust
tracing::info!(operation = "create_user", "User created");
tracing::warn!(operation = "create_user", "Capacity reached");
```

`src/server/logging.rs` exposes `init_with_layer(&config, exporter_layer)` for a
`tracing_subscriber::Layer<Registry>`. Replace the default bootstrap initialization
with this call after configuring your chosen SDK/collector. Standard console
and file sinks remain independently configurable and continue to receive the
same events. A test exporter verifies that file logging and an extra layer can
receive an event together.

For a future Sentry or OpenTelemetry integration, add its SDK dependencies and
initialize its layer/provider before installing the subscriber. Keep SDK guards
alive for the server lifetime and shut exporters down according to that SDK's
requirements. Grafana/Loki deployments can also ingest the existing JSON stdout
or files through a log collector. No cloud SDK, credentials, or outbound exporter
is configured by default. Per-backend transport/retries/authentication belong in
the selected SDK or collector, not in controllers or services.

Static serving, shared environment access, and configurable file/exporter logging
are available starting with CLI 0.1.11. Existing applications need the corresponding source
and dependency updates; updating the CLI does not rewrite an existing app.

## Shared configuration readers

`src/configs/environment.rs` re-exports the shared runtime readers.
`configs::load_env()` loads optional credentials once; `Env::process()` and
`configs::read_env(key)` read secrets with process-over-dotenv precedence.
Ordinary settings use `ConfigFile::load("config/AppConfig.toml")` and typed dotted paths.
Generated server and logger use the same validated configuration snapshot.

```rust
let token = xecute::config::Env::process().secret("API_TOKEN")?;
let file = xecute::config::ConfigFile::load("config/AppConfig.toml")?;
let timeout = file.range("server.request_timeout_seconds", 30, 1, 3600)?;
```

## Keep learning

[Server defaults](server-defaults.md). Return to the [learning path](../index.md) whenever you need a different topic.