---
title: "Typed configuration"
description: "Read reusable typed configuration with redacted validation errors."
---

Use typed configuration when your own feature needs validated settings. Define a Rust type and read it at startup rather than parsing environment values inside every handler.

These APIs are available with CLI 0.1.13, runtime 0.1.8, and macros 0.1.6.
Update the CLI before generating a new project. Existing applications require
corresponding dependency and source changes; updating the CLI does not rewrite them.

The common configuration boundary separates `ConfigFile` for ordinary settings
from `Env` for secrets. Both report value-redacted validation errors. TOML parsing
preserves native types: `false` is a boolean, while `"false"` is an invalid type
for a boolean setting. `get_or` defaults only absent keys; supplied bad values
remain errors. `range` enforces numeric bounds; `required` rejects missing keys.
Generated configuration rejects unknown keys so typos cannot silently disable policies.

```rust
use xecute::config::{ConfigError, ConfigFile, Env, SecretString};

struct DatabaseConfig {
    url: SecretString,
    pool_size: u64,
}
impl DatabaseConfig {
    fn load(file: &ConfigFile) -> Result<Self, ConfigError> {
        Ok(Self {
            url: Env::process().secret("DATABASE_URL")?,
            pool_size: file.range("database.pool_size", 10, 1, 100)?,
        })
    }
}
```

Add `database.pool_size` to the generated allowed-key schema when introducing
this custom setting. Initialize custom configuration before binding the listener.
`SecretString` redacts Debug output and requires `.expose()` for SDK/client access;
it does not promise memory zeroization. `.env` and TOML parse errors omit source
contents. Tests can use `ConfigFile::parse(source)` and map-backed `Env::new`
without changing process variables. Configuration is a startup snapshot.

## Keep learning

[Logging and exporters](/docs/configuration/logging). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#typed-configuration).
