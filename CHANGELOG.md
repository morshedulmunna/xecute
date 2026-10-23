# Changelog

Changes are grouped by published CLI version, newest first. Runtime and macro
versions are listed when they changed alongside a CLI release. Updating the CLI
does not rewrite existing application source; generator changes apply to newly
created projects and resources.

## [Unreleased]

### Fixed

- Serve a generated static landing page at both `/static` and `/static/`.

## [0.1.11] - 2026-10-23

### Added

- A shared environment reader reused by generated server and logging configuration.

- Default public static file serving with a generated example, GET/HEAD/range support, and private-path rejection.
- JSON console/file logging using only LOG_STDOUT_ENABLED and LOG_FILE_DIR, with built-in daily rotation, retention, and shutdown flushing.
- A composable tracing exporter layer hook for future cloud logging integrations.
- A repository changelog and GitHub Release automation using the matching version
  section as release notes.

## [0.1.10] - 2026-10-23

### Added

- `GET /` welcome JSON with HTTP 200, the application name, status, and the Swagger
  documentation path when enabled.
- Startup logs with server, Swagger UI, and OpenAPI URLs using the actual bound
  port, including OS-assigned ports.
- Browser-usable loopback URLs for wildcard bindings and bracketed IPv6 addresses.
- Tests for the welcome response, disabled Swagger, and startup URL formatting.

### Changed

- Disabled Swagger is explicitly logged; the welcome response does not advertise
  a documentation link when it is disabled.

### Fixed

- Fresh applications no longer return `Route not found` at `/`. Other unregistered
  paths continue to return the normal 404 error envelope.

## [0.1.9] - 2026-10-23

### Added

- CQRS `services/<feature>_service.rs` for shared business operations. Command and
  query handlers inject the service, which injects the repository.
- Service provider registration and exports in generated CQRS modules.

### Changed

- New resources and standalone building blocks live under `src/modules/<feature>/`.
- `src/modules/mod.rs` declares features; application registration uses
  `crate::modules::<feature>`.
- Resource HTTP tests live inside the feature at `tests/http.rs`; shared server
  tests remain in `src/tests/`.
- Rust scaffolding templates use `.rs` instead of `.tmpl`, preventing their
  classification as Go Template. The manifest template uses `Cargo.toml.in`.

## [0.1.8] - 2026-10-23

### Added

- `xecute update` and `xecute uninstall`, targeting the running executable's
  Cargo installation root, including custom installation roots.
- `xecute new .` for in-place initialization and `--name` for an explicit package name.
- Preflight collision and symlink checks before in-place scaffold writes.
- Cargo failure reporting and Windows helper handling for executable replacement
  or removal after the running CLI exits.

### Changed

- Removed framework-comparison wording from package descriptions and documentation.
- Generated applications use runtime `xecute 0.1.6`; macros remain `xmacros 0.1.5`.
- Existing application files are retained during CLI management operations.

## [0.1.7] - 2026-10-23

### Added

- `xecute run dev` with automatic debug rebuild/restart, debouncing, and recovery
  after compiler errors.
- `--bin`, repeatable `--watch`, and application argument forwarding after `--`.
- Source/configuration and `.env` watching with build-output exclusions, including
  custom Cargo target directories.
- Managed process-tree shutdown on restarts and Ctrl-C.
- App-level `Application::use_middleware` and controller/route `#[middleware(...)]`.
- Ordered middleware execution, early responses, and a global `src/middleware.rs`
  configuration hook.

### Changed

- Generated applications use runtime `xecute 0.1.5` and macros `xmacros 0.1.5`.

## [0.1.6] - 2026-10-23

### Added

- MVC generation by default, explicit `--mvc`, and opt-in CQRS through `--cqrs`.
- Persistent architecture selection in `.xecute/architecture` for later generators.
- CQRS create-command and list-query handlers with automatic DI registration.

### Changed

- Request validation uses `validator` instead of `garde`, automatically rejecting
  invalid body/query DTOs before controller business logic runs.
- Repository metadata points to `morshedulmunna/xecute`.
- Added a root workspace manifest and build documentation.
- Generated applications use runtime `xecute 0.1.4` and macros `xmacros 0.1.4`.

[Unreleased]: https://github.com/morshedulmunna/xecute/compare/v0.1.11...HEAD
[0.1.11]: https://github.com/morshedulmunna/xecute/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/morshedulmunna/xecute/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/morshedulmunna/xecute/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/morshedulmunna/xecute/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/morshedulmunna/xecute/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/morshedulmunna/xecute/releases/tag/v0.1.6
