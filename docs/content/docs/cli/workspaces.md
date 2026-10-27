---
title: "Cargo workspaces"
description: "Generate multiple applications with shared dependencies and tooling."
---

Choose a workspace when several applications should share dependency declarations and repository tooling. A single application does not need this setup.

Workspace scaffolding is available starting with CLI 0.1.14. MVC is the default:

```sh
xecute new my-project --workspace
cd my-project
xecute g apps admin
xecute g apps billing --cqrs
xecute g apps dashboard --mvc
cargo build --workspace
cargo test --workspace
```

The first application is created at `apps/my-project/`. Each app keeps its own
configuration, HTTP server, feature modules, and MVC or CQRS architecture.
Applications share the root `Cargo.lock`, `target/`, and dependency definitions
in `[workspace.dependencies]`. App dependencies use `workspace = true`, with
features selected per app. New applications inherit
the workspace architecture; use `--mvc` or `--cqrs` to override it.
`xecute new my-project --workspace --cqrs` selects CQRS for the workspace.

Run resource generators and the development server inside an app:

```sh
cd apps/admin
xecute g resource user
xecute run dev
```

Configure distinct bind addresses in the apps' `config/AppConfig.toml` files
when running them together. See [the CLI workspace guide](https://github.com/morshedulmunna/xecute/blob/main/xecute-cli/README.md#multiple-applications-in-a-workspace)
for local runtime paths, container builds, and in-place initialization.

## Keep learning

[Update and uninstall](/docs/cli/update). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#multiple-applications-with-cargo-workspaces).
