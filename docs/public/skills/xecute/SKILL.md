---
name: xecute
description: Build, modify, test, or explain Rust APIs using Xecute and Axum, including generated MVC/CQRS features, modules, dependency injection, guards, and optional runtime integrations. Use when working in a Xecute application or when the user asks to use Xecute.
---

# Xecute application development

Use this skill to work with the application's selected architecture and installed
APIs. Follow the user's scope and repository instructions. This skill does not
authorize publishing packages, deploying, or changing external services.

## Read the right documentation

The skill bundle contains the full documentation, not just links to a website.
Start with [the reference index](references/topics.md). To read the complete manual
in one pass, use [the full documentation](references/full.md). For ordinary tasks,
read only the relevant topic files and follow their links.

- First application: [quick start](references/getting-started/quickstart.md),
  [first feature](references/getting-started/first-feature.md),
  [request flow](references/getting-started/request-flow.md).
- Generation and architecture: [CLI commands](references/cli/commands.md),
  [MVC/CQRS](references/architecture/mvc-cqrs.md),
  [modules](references/architecture/modules.md),
  [provider scopes](references/architecture/scopes.md).
- HTTP: [controllers](references/http/controllers.md),
  [DTO validation](references/http/validation.md),
  [errors](references/http/errors.md), [OpenAPI](references/http/openapi.md).
- Integrations: [feature flags](references/reference/features.md),
  [lifecycle](references/architecture/lifecycle.md),
  [PostgreSQL](references/integrations/postgres.md),
  [guards/JWT](references/integrations/auth.md),
  [jobs](references/integrations/jobs.md), [cache](references/integrations/cache.md),
  [CQRS buses](references/integrations/cqrs.md).
- Verification: [HTTP/provider tests](references/testing/modules.md),
  [quality checks](references/testing/checks.md),
  [troubleshooting](references/operations/troubleshooting.md).

If only this SKILL.md was downloaded, obtain the complete bundle from the Skills
page on the same documentation site. Do not assume the references are available
without checking. The site's `/llms.txt` lists machine-readable documents;
`/llms-full.txt` contains the whole manual. Respect any hosting base path.

## Orient to the application

Inspect Cargo.toml, Cargo.lock, .xecute/architecture, src/app_module.rs, and one
existing feature before editing. In workspaces, inspect the root manifest and the
selected app under apps/. Keep the dependency source and versions consistent.
Use installed source/API definitions to resolve differences from these reference
docs; do not assume every older application has the latest generator wiring.

For new features, run `xecute g resource <name>` from the app root when the CLI is
available. Preserve generator markers in AppModule and feature registrations.
Generation defaults to MVC; `.xecute/architecture` determines subsequent resources.
If the CLI is unavailable, follow the documented generated structure manually
and register both module definitions and routers.

## Preserve the architectural boundaries

- MVC: Controller → Service → Repository.
- CQRS: Controller → Command/Query Handler → Repository. Add a domain service only
  for actual shared logic. Typed buses are optional, not automatically generated.
- Put input contracts in dto/, records in entity/, data access in repository/,
  operations in services/handlers, and feature HTTP tests in tests/http.rs.
- Generated resources provide collection GET and create POST, with in-memory
  storage. Add get-by-ID, update/delete, pagination, persistence, and login only
  when the task calls for them.

## Apply Xecute-specific rules

Modules explicitly list providers, controllers, imports, and exports. Controllers
must be providers too. Imports grant provider visibility; they do not discover
module definitions or mount routes. Register every definition in the graph.
Use Arc<T> for injected shared dependencies and #[inject(default)] for local state.
Choose provider scope deliberately; do not capture request dependencies in singletons.

Use #[controller] on the struct and #[routes] on its impl. Parameter #[body] and
#[query] decode and validate DTOs deriving Deserialize, Validate, and JsonSchema.
Use singular #[param] for path values. Other Axum extractors work directly. Use
one body-consuming extractor, last; forms do not get automatic validator checks.
Keep schema constraints aligned with runtime validation. HTTP extraction does
not validate direct service/handler calls, so enforce domain invariants there too.

Declare #[guard(...)] below #[module] or #[controller], or on a route inside
#[routes]. Module declarations also accept guards = [...]. Policies are additive:
application → module → controller → route. Mount FeatureModule::scoped_router to
apply module guards; direct controller routers do not discover module policies.
Older apps need the routing migration in the auth guide. Authenticate before
roles/permissions; configure a trusted verifier. Headers alone are not identity.

Read the integration guide before wiring a resource. Optional features need
explicit configuration and provider registration; they do not rewrite the app.
Use async container initialization and shutdown when resources have async
factories/hooks; lifecycle attribute markers alone do not execute hooks. Preserve
draining and cleanup on startup failure. Jobs and event buses are process-local;
use durable infrastructure if the requested work must survive a crash. Give
cache entries explicit keys, expiration, ownership scope, and invalidation rules.

## Verify the requested behavior

Use the app's established checks. For generated apps, run formatting, relevant
HTTP/provider tests, and Clippy. Test meaningful boundaries: validation before
persistence, guard denial before handlers, cross-module visibility, and cleanup
when introducing managed resources. Integration tests need their documented
external services; do not substitute a successful unit test for a database test.

For dependency changes, resolve the lockfile with Cargo, review it, and preserve
compatible versions. Do not force breaking upgrades or overwrite existing
configuration just because a newer template exists. Report changed behavior,
checks run, and any remaining limitations.
