---
title: "Learn Xecute"
description: "A guided path from your first Rust API to modules, testing, and integrations."
---

Build a Rust API with a clear home for routes, input validation, application logic,
and data access. Xecute adds a CLI, modules, and dependency injection on top of
Axum, so you can start with a connected feature instead of wiring everything first.

You do not need a database to get started. You should be comfortable running
terminal commands and reading basic Rust structs and async functions.

## New to Xecute? Follow this path

| Step | What you will do | Guide |
| --- | --- | --- |
| 1 | Install the CLI, run an API, and send a valid and invalid request | [Build your first API](/docs/getting-started/quickstart) |
| 2 | Generate `/user`, change a validation rule, and run tests | [Your first feature](/docs/getting-started/first-feature) |
| 3 | Follow input from the HTTP route to storage | [How a request works](/docs/getting-started/request-flow) |
| 4 | Add your own body, URL, and query inputs | [Controllers and extractors](/docs/http/controllers) |
| 5 | Share providers between features | [Providers and modules](/docs/architecture/modules) |

Start with MVC, the default. Learn [CQRS](/docs/architecture/mvc-cqrs) when you want
to separate write operations from read operations. You can use the same concepts
without adding a message bus or a second database.

## Already building something?

| I need to… | Go to |
| --- | --- |
| Understand generated files | [Project structure](/docs/architecture/project-structure) |
| Share Rust code across applications | [Shared libraries](/docs/cli/libraries) |
| Find a CLI command | [CLI reference](/docs/cli/commands) |
| Move from NestJS | [Coming from NestJS](/docs/getting-started/coming-from-nestjs) |
| Configure a port, limits, or environment | [Application configuration](/docs/configuration/application) |
| Test a route or replace a dependency in tests | [Testing modules and HTTP](/docs/testing/modules) |
| Connect PostgreSQL | [PostgreSQL](/docs/integrations/postgres) |
| Protect routes | [Guards and JWT](/docs/integrations/auth) |
| Fix a setup or startup problem | [Troubleshooting](/docs/operations/troubleshooting) |
| Update an existing generated application | [Adoption guide](/docs/getting-started/adopting-existing) |

## Add capabilities when you need them

Lifecycle management is part of the runtime. Database, JWT authentication, jobs,
caching, and CQRS buses are enabled through Cargo features and explicit provider
configuration. Start with the generated API, then follow the integration guide for
each capability. The [feature setup guide](/docs/getting-started/status) explains
what is generated and what you configure yourself.

A generated resource starts with collection GET and create POST, validation, and
an **in-memory** repository. It does not create login, database persistence,
update/delete endpoints, background workers, or cache configuration automatically.

## A few terms you will see

| Term | Plain meaning |
| --- | --- |
| Feature | One part of your application, such as users or orders |
| DTO | A Rust struct describing accepted request data and validation rules |
| Provider | An object registered so Xecute can inject it where needed |
| Module | A group of providers, with explicit imports and exports |
| Repository | The part that reads and writes data |
| Lifecycle | Startup and shutdown work, such as opening and closing a pool |

Tutorials tell you where to run commands and what to expect. Reference pages cover
the available APIs in more detail. The source link at the bottom identifies where
the page is maintained; it is useful when you want to inspect implementation details.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/docs/handbook/index.md).
