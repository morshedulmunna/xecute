A library is a reusable Rust crate. Put shared domain types, validation helpers,
or services in it, then import it from one or more applications. It does not run
an HTTP server by itself.

Use a **feature module** for routes owned by one application. Use a **library**
when code needs a separate Cargo boundary and can be shared by applications.

## 1. Create a workspace

```sh
xecute new my-platform --workspace
cd my-platform
```

This creates the first application at `apps/my-platform`. If you already have a
Xecute workspace, open its root directory beside the workspace `Cargo.toml`.
The library command must run there, not inside an app or standalone project.

## 2. Generate a library

```sh
xecute g library shared-utils
# Short form: xecute g lib shared-utils
# Long form: xecute generate library shared-utils
```

You get:

```text
my-platform/
├── Cargo.toml
├── Cargo.lock
├── apps/my-platform/
└── libs/shared-utils/
    ├── Cargo.toml
    ├── README.md
    └── src/lib.rs
```

The CLI adds the library to workspace members (unless a matching member pattern
already covers it) and registers a shared path dependency:

```toml
# Workspace Cargo.toml
[workspace.dependencies]
shared-utils = { path = "libs/shared-utils" }
```

Existing workspace declarations and comments are preserved. The generator does
not add the dependency to every application, modify existing app code, or register
DI modules automatically.

## 3. Use it in an application

Merge the dependency into `apps/my-platform/Cargo.toml`:

```toml
[dependencies]
shared-utils = { workspace = true }
```

Now your application's Rust code can call the generated example:

```rust
let message = shared_utils::greet("  Munna  ");
assert_eq!(message, "Hello, Munna!");
```

The Cargo package is `shared-utils`; the Rust import is `shared_utils`. Hyphens
become underscores in Rust crate names.

The greeting function is only a small working example. Replace it with shared
code your application actually needs. Do not move transport-specific controllers
into a general domain library unless that is its intended purpose.

After adding the app dependency, run `cargo check -p my-platform` to refresh the
workspace lockfile. Commit it with the dependency change.

## 4. Test and check the library

From the workspace root:

```sh
cargo test -p shared-utils --locked
cargo clippy -p shared-utils --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

The generated crate includes unit tests for whitespace, Unicode, and empty input,
plus a runnable documentation example. It shares the workspace lockfile, build
output, toolchain and profiles; it has no separate lockfile or server settings.

When defined by the root, edition, minimum Rust version, license, and lints are
inherited. Otherwise the crate uses edition 2024, Rust 1.98.1, and forbids unsafe
code. It starts with no third-party dependencies and `publish = false`.

## Share a Xecute provider

The library is ordinary Rust and does not depend on Xecute by default. To share
injectable services or modules:

1. Add `xecute = { workspace = true }` to the library's dependencies, using the
   runtime source/version already declared by your workspace.
2. Make the provider and module types public and export the intended providers.
3. Add the library dependency to the consuming app.
4. Register its module definition and import it from the app module that needs
   those providers. Mount routers explicitly if the library includes HTTP code.

A Cargo dependency makes Rust code available. A Xecute module import controls DI
visibility. Neither step replaces the other. Follow
[Providers and modules](../architecture/modules.md) for registration examples.

## Failure and recovery

| Message or situation | What to do |
| --- | --- |
| Command is not recognized | Use a CLI build containing the library generator; `xecute --help` lists it |
| Not a Xecute workspace root | Run from the directory containing `.xecute/workspace` and the workspace manifest |
| Name or destination already exists | Choose a different name; existing libraries and app packages are not overwritten |
| Workspace dependency already uses the name | Use a distinct name; the generator will not replace a dependency |
| Library path is excluded | Review `workspace.exclude`; generation respects those exclusions |
| Formatting failed | Fix rustfmt/toolchain availability and retry; the staged library and manifest edits are discarded |
| Library created, lockfile refresh failed | Keep the generated files, fix Cargo/registry configuration, then run `cargo generate-lockfile` from the workspace root |
| Library generator lock exists | Wait for the other generator. If its process crashed, verify no generator is running before removing `.xecute/library-generator.lock` and retrying |

Names follow the usual Xecute lowercase naming rules. Hyphen/underscore equivalents
are treated as the same Rust crate name for conflict checks. Symlinked workspace
manifests, control directories, and library destinations are rejected.

Next, explore [Cargo workspaces](workspaces.md) or
[Testing modules and HTTP](../testing/modules.md).