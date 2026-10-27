# __NAME__

Reusable Rust code shared by applications in this workspace. The greeting function
is a small, tested starting example; replace it with your application's shared API.
No HTTP server, runtime dependency, or module registration is added automatically.

From the workspace root:

```sh
cargo test -p __NAME__
cargo clippy -p __NAME__ --all-targets -- -D warnings
```

Add this to a consuming application's Cargo.toml:

```toml
[dependencies]
__NAME__ = { workspace = true }
```

Use it from Rust:

```rust
let message = __CRATE__::greet("Munna");
```

Hyphens in a Cargo package name become underscores in Rust imports. The workspace
owns the lockfile, profiles, toolchain, and shared dependency declarations. This
library is private (`publish = false`); publishing is a separate explicit decision.

To share Xecute providers, add the workspace's `xecute` dependency to this crate,
export public providers/module types, and register/import them in the consuming
application. A Cargo dependency alone does not register DI modules or HTTP routes.
