---
title: "Application configuration"
description: "Configure generated servers through validated TOML and environment secrets."
---

Start here to change generated server settings. Edit config/AppConfig.toml for application settings and use environment variables for secrets.

Ordinary settings live in the generated `config/AppConfig.toml`. Commit this file;
keep credentials in `.env` or inject them through your deployment's secret store.
`.env` is optional and ignored by Git/Docker. Process variables take precedence
over `.env` for secrets. Ordinary settings have no environment fallback or override.

```toml
[server]
bind_address = "127.0.0.1:3000"
request_timeout_seconds = 30
shutdown_grace_seconds = 10
body_limit_bytes = 1048576
max_in_flight_requests = 256

[rate_limit]
requests = 60
window_seconds = 60
max_clients = 10000

[cors]
origins = ["https://example.com", "http://localhost:5173"]
allowed_methods = ["GET", "POST"]
allowed_headers = ["content-type", "authorization"]
exposed_headers = ["x-request-id", "retry-after"]
allow_credentials = false
max_age_seconds = 600

[docs]
enabled = true

[logging]
filter = "info"
stdout_enabled = true
file_dir = "logs"
```

The generated CORS origin list is empty by default. Origins must be exact HTTP(S)
origins without paths, credentials, wildcards, or trailing slashes. Credentials
require a nonempty origin list. Arrays are native TOML arrays, without JSON or
dotenv quoting. Invalid types, unknown keys, invalid protocol values, and numeric
bounds fail startup before logging/listening. Missing individual values use typed
defaults, but the file itself must exist. Request limits must be positive; the
CORS cache duration may be zero.

`Config::from_file()` reads `config/AppConfig.toml` relative to the working directory.
`Config::from_path(path)` supports an explicit file chosen in bootstrap code.
The file is parsed once into a validated snapshot shared with logger startup;
there is no automatic production reload. Development hot reload watches TOML
files and restarts the process when settings change.

The Dockerfile includes this file and adapts only the generated loopback bind to
`0.0.0.0:3000`. Custom bind addresses remain unchanged. To supply deployment
settings, mount a replacement `/app/config/AppConfig.toml` read-only. The default private
`/app/logs` directory is owned by UID 65532; custom log mounts must also be writable
by this user. A read-only root filesystem requires writable log storage or an
empty `logging.file_dir`.

For custom settings, use `xecute::config::ConfigFile` with dotted paths and extend
the allowed-key schema in `src/configs/server.rs`. Use `Env::process().secret(key)`
for required credentials and tokens. The template `.env.example` contains only
commented credential examples, with no active configuration or fake secrets.

## Keep learning

[Typed configuration](/docs/configuration/typed). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#application-configuration).
