# Changelog

Changes are grouped by published CLI version, newest first. Runtime and macro
versions are listed when they changed alongside a CLI release. Updating the CLI
does not rewrite existing application source; generator changes apply to newly
created projects and resources.

## [Unreleased]

## [0.1.17] - 2026-10-24

Packages: `xecute-cli 0.1.17`, `xecute 0.1.9`, and `xmacros 0.1.7`.
Runtime and macro releases update documentation and package descriptions only.

### Documentation

- Describe Xecute consistently as application architecture tools and macros built
  on Axum, rather than a separate framework, across READMEs, generated project
  documentation, crate descriptions, and crate-level documentation.

### Fixed

- Initialize versioning configuration directly in generated root-route tests so
  Clippy passes with `--all-targets --all-features -- -D warnings`.

## [0.1.16] - 2026-10-24

Packages: `xecute-cli 0.1.16`, unchanged `xecute 0.1.8` and `xmacros 0.1.6`.

### Changed

- Emit only the `server listening` startup log. Serve the bound address, local URL,
  enabled Swagger/OpenAPI and versioned documentation paths, and static mount at
  `GET /` instead of announcing each endpoint in separate logs.

### Fixed

- Generate shared lint policies in `[workspace.lints]` and only
  `[lints] workspace = true` in workspace apps, preserving root lint customizations.

## [0.1.15] - 2026-10-24

Packages: `xecute-cli 0.1.15`, unchanged `xecute 0.1.8` and `xmacros 0.1.6`.

### Changed

- Generate workspace tooling, Git settings, Docker ignore rules, and GitHub
  configuration once at the root rather than duplicating them in each app.

- Centralize generated workspace dependency versions and paths in the root
  `[workspace.dependencies]`. Apps inherit dependencies with `workspace = true`,
  retaining their own feature selections and test-only runtime features.
- Adding apps and generating resources preserves existing shared dependency
  definitions. Explicit app runtime paths update the shared workspace runtime.

## [0.1.14] - 2026-10-24

Packages: `xecute-cli 0.1.14`, unchanged `xecute 0.1.8` and `xmacros 0.1.6`.

### Added

- `xecute new <project|.> --workspace` scaffolds a Cargo workspace with its first
  application under `apps/`, a shared lockfile, tooling, and workspace CI.
- `xecute g apps <app_name>` and `xecute generate apps <app_name>` add applications
  that inherit the workspace architecture, with `--mvc` and `--cqrs` overrides.
- Workspace applications retain feature modules, application configuration,
  static files, resource generators, tests, and Dockerfiles built from the
  workspace root. Local runtime paths can be inherited or overridden.
- Integration coverage for workspace membership, default MVC, CQRS selection,
  resource generation, architecture overrides, in-place creation, and conflicts.

### Changed

- MVC remains the default architecture for new projects and workspaces.
- Resolve newly added workspace members while retaining existing locked
  dependency versions.

## [0.1.13] - 2026-10-23

Packages: `xecute-cli 0.1.13`, `xecute 0.1.8`, and unchanged `xmacros 0.1.6`.

### Added

- Runnable file-upload controller example with private local storage, multipart
  OpenAPI metadata, size/type validation, and HTTP persistence/error tests.

### Changed

- Move generated application settings to config/AppConfig.toml, updating startup,
  Docker, and documentation paths.

- New projects generate only v1; remove the built-in v2 example and uploads module,
  upload configuration, and default upload/S3 features. Runtime upload and versioning
  APIs remain available for custom modules.

## [0.1.12] - 2026-10-23

Packages: `xecute-cli 0.1.12`, `xecute 0.1.7`, and `xmacros 0.1.6`.

### Added

- Streaming multipart uploads with actual size/count limits, MIME signature checks,
  private staging, generated keys, checksums, local atomic storage, optional official
  AWS S3 storage, and handled-failure batch rollback.
- Explicit /v1 and /v2 route groups with independently validated OpenAPI contracts,
  version-specific Swagger pages, and generated upload/version examples.
- Cargo feature selection through `xecute run dev --features`, plus optional S3
  container builds and configuration in AppConfig.toml with credentials kept outside TOML.

- Typed environment configuration with required/optional values, conversion, defaults,
  numeric bounds, JSON parsing, custom invariants, and redacted secret/error handling.
- Safe runtime exception envelopes and explicit domain-error mappings retained by
  generated middleware, with request correlation and protected internal failures.
- Automatic per-request DI contexts and controller routing; nested request/transient
  resolution, concurrent singleton caching, eager startup initialization, and graph validation.
- Optional testing helpers: isolated module/provider overrides, in-process HTTP client,
  JSON assertions, response size bounds, timeouts, and generated usage examples.

### Changed

- Ordinary generated application settings now load from required AppConfig.toml;
  .env is optional and contains only secret examples. TOML types/unknown keys are
  validated at startup, and the same snapshot configures server and logging.

- Generated applications use runtime 0.1.7 and macros 0.1.6 APIs. These package
  versions are published alongside CLI 0.1.12.
- Provider construction errors are cached per scope; undeclared factory dependencies
  and missing required registrations now fail explicitly.


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

[Unreleased]: https://github.com/morshedulmunna/xecute/compare/v0.1.13...HEAD
[0.1.11]: https://github.com/morshedulmunna/xecute/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/morshedulmunna/xecute/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/morshedulmunna/xecute/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/morshedulmunna/xecute/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/morshedulmunna/xecute/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/morshedulmunna/xecute/releases/tag/v0.1.6

[0.1.12]: https://github.com/morshedulmunna/xecute/compare/v0.1.11...v0.1.12

[0.1.13]: https://github.com/morshedulmunna/xecute/compare/v0.1.12...v0.1.13
