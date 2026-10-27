---
title: "Update and uninstall"
description: "Manage CLI installation without rewriting application source."
---

Update the installed CLI executable or remove it. Updating the CLI does not rewrite an application you already generated.

For a CLI installed through Cargo:

```sh
xecute update
xecute uninstall
```

`update` runs `cargo install xecute-cli --registry crates-io --locked --force` in
the same installation root as the running executable. It updates the CLI and
its templates; it does not edit application dependencies or regenerate existing
projects. It requires Cargo and access to crates.io. A local/source installation
is replaced by the published registry version, which may not include unreleased
local features. Check the release status above before updating a development build.

`uninstall` runs `cargo uninstall xecute-cli` against that same root. It removes
the installed CLI, including installations created with `cargo install --root`.
It does not remove application directories, source code, or the Cargo toolchain.

The CLI verifies Cargo's `.crates2.json` installation metadata before invoking
these commands. Executables run directly from `target/debug` or copied outside a
registered installation are rejected with guidance to use Cargo directly.
Unix commands wait for Cargo and report failures. On Windows, a PowerShell helper
waits for the running executable to exit before starting Cargo; its terminal
output reports completion or errors after the CLI returns.

To initialize an existing directory without creating a nested folder:

```sh
mkdir my-api
cd my-api
xecute new .
xecute run dev

# Optional package name, architecture, and local runtime:
xecute new . --name my-api --cqrs --runtime-path /absolute/path/to/xecute/xecute
```

The final example is an alternative initialization command for an uninitialized
directory. The default package name comes from the current directory's basename;
use `--name` if the basename contains spaces, uppercase letters, or other invalid
characters. `--name` can also override a named project's Cargo package name.
MVC remains the default, and `--cqrs` selects CQRS.

In-place generation checks all scaffold destinations before writing. Conflicting
files such as `Cargo.toml`, `README.md`, or `.gitignore` cause a clear error with
no scaffold files changed. Move those conflicting files or choose a fresh directory
before retrying. Unrelated files and an existing `.git` directory are preserved.
Symlinked scaffold parents are rejected. Generation does not change the shell's
working directory and does not print an extra `cd` step for `new .`.

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#update-and-uninstall).
