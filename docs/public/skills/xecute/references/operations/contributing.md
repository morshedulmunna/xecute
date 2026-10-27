This page is for changing Xecute itself, rather than building an application with it. It explains the repository packages and contributor checks.

Repository: [morshedulmunna/xecute](https://github.com/morshedulmunna/xecute).

| Package | Role | Documentation |
| --- | --- | --- |
| `xecute` | Runtime, DI container, validation, HTTP middleware, OpenAPI helpers | [Runtime README](https://github.com/morshedulmunna/xecute/blob/main/xecute/README.md) |
| `xmacros` | Injection, module, controller, and route procedural macros | [Macro README](https://github.com/morshedulmunna/xecute/blob/main/xmacros/README.md) |
| `xecute-cli` | `xecute` binary and project/resource templates | [CLI README](https://github.com/morshedulmunna/xecute/blob/main/xecute-cli/README.md) |

From the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo install --path xecute-cli --locked --force
```

Use `--runtime-path /absolute/path/to/xecute/xecute` when exercising local runtime
changes. CLI template changes affect newly generated files; they do not rewrite
existing applications automatically.

## Keep learning

[Troubleshooting](troubleshooting.md). Return to the [learning path](../index.md) whenever you need a different topic.