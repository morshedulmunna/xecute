# xecute

A NestJS-inspired Rust framework for Axum with dependency injection, modular
architecture, automatic payload validation, and CLI scaffolding.

```sh
cargo install xecute-cli --locked
xecute new my-app
cd my-app
xecute g resource user
cargo run
```

New projects use MVC (controller → service → repository) by default.
Use `xecute new my-app --cqrs` for separate command and query handlers.
The architecture is saved in `.xecute/architecture` for subsequent generation.
Invalid request payloads are automatically rejected before business logic runs.
Generated applications include Swagger UI at `/docs/` and HTTP boundary tests.

## Packages

- [`xecute`](xecute): runtime, dependency injection, validation, and Axum integration.
- [`xmacros`](xmacros): controller, route, module, and injection macros.
- [`xecute-cli`](xecute-cli): the `xecute` command and project/resource scaffolding.

## Development

```sh
cargo test --workspace
cargo install --path xecute-cli
xecute new my-app --runtime-path /absolute/path/to/xecute/xecute
```

Repository: <https://github.com/morshedulmunna/xecute>

Licensed under Apache-2.0.
