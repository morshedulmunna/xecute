Keep a development server running while you edit Rust files. Start with a generated application; this runner rebuilds and restarts it when watched files change.

With CLI 0.1.7 or later installed, run this from your application's directory:

```sh
xecute run dev
```

The CLI starts `cargo run` in the debug profile and monitors project files. Saving
source or configuration changes automatically stops the previous process, rebuilds,
and restarts the application. It uses Cargo's incremental build cache; there is
no separate watcher executable to install.

```sh
# Choose a binary in a project with several binary targets:
xecute run dev --bin server

# Also watch a local runtime or another source directory (repeatable):
xecute run dev --watch ../xecute/xecute --watch ../xecute/xmacros

# Pass arguments to the application, preserving individual argument boundaries:
xecute run dev -- --port 4000
```

The application must implement any arguments passed after `--`; the generated
server configures its port with `server.bind_address` in `config/AppConfig.toml` instead.

The watcher polls file metadata every 200 ms and groups edits with a 300 ms quiet
period. It detects additions, changes, removals, and rename-save operations for
Rust, TOML, JSON, YAML, SQL, HTML, CSS, JavaScript, TypeScript, and GraphQL files,
plus `Cargo.lock`, `.env`, and `.env.*`. An explicitly watched file can have any
extension. Additional watch paths must exist when the command starts. Files in
symlinked subdirectories are not followed; pass a real path with `--watch` if needed.

Cargo's resolved target directory is excluded, including custom
`CARGO_TARGET_DIR` settings. Directories named `target`, `.git`, `.hg`, `.svn`,
`node_modules`, `dist`, `build`, `.idea`, and `.vscode` are also excluded. Runtime
logs and ordinary data files are not watched unless their extension is in the
list above or they are explicitly selected. Keep generated JSON/configuration
output outside the watched source tree to avoid restart loops.

Compiler diagnostics and application logs remain visible in the terminal. A
failed build or exited app leaves the watcher running; saving another change
retries it. An edit during compilation cancels the old process before starting
the next build. If the project has multiple runnable binaries, select one with
`--bin`. Dependencies outside the project tree need explicit `--watch` paths.

Ctrl-C stops the watcher and its managed Cargo/application processes. On Unix,
restarts and shutdown send SIGTERM to the process group, allow up to five seconds
for cleanup, then force termination if necessary. On Windows, the process tree
is terminated with `taskkill /T /F`. Dev shutdown can therefore be shorter than
the generated server's normal production drain deadline. Application stdin is
closed; this command is intended for development servers, not interactive programs.

This is **rebuild-and-restart hot reload**: it restarts the server process, resets
in-memory data, and does not automatically refresh the browser or patch a running
process. Use `cargo run` for a single run and `cargo build --release --locked`
for a production binary. Hot reload runs only when explicitly using `xecute run dev`.

## Keep learning

[Cargo workspaces](workspaces.md). Return to the [learning path](../index.md) whenever you need a different topic.