# Xecute reference index

Read by task, or use [the complete manual](full.md).

## Getting started

- [Build your first API](getting-started/quickstart.md): Install Xecute, run an API, and understand your first successful and invalid requests.
- [Your first feature](getting-started/first-feature.md): Generate a user feature, find the right files, change a rule, and run tests.
- [How a request works](getting-started/request-flow.md): Understand DTOs, controllers, services, repositories, and module imports.
- [Coming from NestJS](getting-started/coming-from-nestjs.md): Map NestJS concepts to the APIs actually implemented in Xecute.
- [Features and setup](getting-started/status.md): Choose runtime features and understand what the generator configures for you.
- [Upgrade an existing application](getting-started/adopting-existing.md): Enable the five new runtime capabilities without assuming files are regenerated.

## CLI and workspaces

- [CLI reference](cli/commands.md): Project creation, resource generators and supported flags.
- [Development runner](cli/development.md): Watch, rebuild, recover from errors and shut down development processes.
- [Cargo workspaces](cli/workspaces.md): Generate multiple applications with shared dependencies and tooling.
- [Shared libraries](cli/libraries.md): Generate a reusable Rust crate and share it between workspace applications.
- [Update and uninstall](cli/update.md): Manage CLI installation without rewriting application source.

## Architecture and DI

- [Project structure](architecture/project-structure.md): Understand generated modules, server infrastructure and feature folders.
- [MVC and CQRS](architecture/mvc-cqrs.md): Choose service-based MVC or direct-handler CQRS generation.
- [Providers and modules](architecture/modules.md): Register providers and enforce imports, exports and dependency boundaries.
- [Feature folders](architecture/feature-folders.md): Keep DTOs, entities, repositories and tests together by feature.
- [Provider scopes](architecture/scopes.md): Use singleton, transient and request lifetimes safely.
- [Async startup and shutdown](architecture/lifecycle.md): Initialize resources in dependency order and clean up after draining.

## HTTP APIs

- [Controllers and extractors](http/controllers.md): Route handlers, JSON bodies, path parameters and query DTOs.
- [DTO validation](http/validation.md): Reject malformed and invalid requests before application work.
- [Middleware](http/middleware.md): Compose app, controller and route middleware in documented order.
- [Swagger and OpenAPI](http/openapi.md): Generate OpenAPI schemas and serve local Swagger documentation.
- [Errors and domain mappings](http/errors.md): Map failures to safe correlated HTTP error envelopes.
- [API version groups](http/versioning.md): Mount explicit versioned routes and separate API documents.
- [Multipart uploads](http/uploads.md): Validate streaming uploads and use private local or optional S3 storage.
- [Static files](http/static-files.md): Serve explicitly public assets without exposing application files.

## Configuration and operations

- [Application configuration](configuration/application.md): Configure generated servers through validated TOML and environment secrets.
- [Typed configuration](configuration/typed.md): Read reusable typed configuration with redacted validation errors.
- [Logging and exporters](configuration/logging.md): Configure structured logs, file rotation and custom exporter layers.
- [Server defaults](configuration/server-defaults.md): Understand request limits, health endpoints and graceful shutdown policies.
- [Welcome and startup URLs](configuration/welcome.md): Discover enabled documentation, static assets and route version groups.

## Opt-in runtime features

- [PostgreSQL](integrations/postgres.md): Configure lifecycle-managed pools, migrations, repositories and transactions.
- [Guards and JWT authentication](integrations/auth.md): Protect routes with verified users and role/permission policies.
- [Jobs and scheduling](integrations/jobs.md): Choose process-local jobs or broker-backed workers, with retries and interval schedules.
- [Memory and Redis caching](integrations/cache.md): Use explicit TTL caching with bounded memory or namespaced Redis.
- [CQRS buses and events](integrations/cqrs.md): Register typed command/query handlers, event subscribers and shared pipelines.

## Testing

- [Quality checks](testing/checks.md): Run runtime, generator and generated-application checks.
- [Testing modules and HTTP](testing/modules.md): Override providers and exercise real routers in isolated tests.
- [Runnable examples](testing/examples.md): Find feature flags, commands and external services for each example.

## Deployment and maintenance

- [Deployment](operations/deployment.md): Build, containerize and operate generated applications.
- [Development and packages](operations/contributing.md): Work on the CLI, runtime and macro crates.
- [Troubleshooting](operations/troubleshooting.md): Resolve common generation and application startup issues.
- [Scope and limitations](operations/limitations.md): Separate implemented capabilities from application extensions.

## Reference

- [Cargo feature reference](reference/features.md): Choose the Cargo flags and setup required for each runtime capability.
- [Release process](reference/releases.md): Understand crate versions, tags and generated GitHub release notes.
- [Changelog](reference/changelog.md): Review changes across Xecute releases.

## AI agent skills

- [AI agent skills](skills/ai-agent.md): Download the Xecute skill and complete machine-readable documentation for your coding agent.

