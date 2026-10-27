Build a Rust API with a clear home for routes, input validation, application logic,
and data access. Xecute adds a CLI, modules, and dependency injection on top of
Axum, so you can start with a connected feature instead of wiring everything first.

You do not need a database to get started. You should be comfortable running
terminal commands and reading basic Rust structs and async functions.

## New to Xecute? Follow this path

| Step | What you will do | Guide |
| --- | --- | --- |
| 1 | Install the CLI, run an API, and send a valid and invalid request | [Build your first API](getting-started/quickstart.md) |
| 2 | Generate `/user`, change a validation rule, and run tests | [Your first feature](getting-started/first-feature.md) |
| 3 | Follow input from the HTTP route to storage | [How a request works](getting-started/request-flow.md) |
| 4 | Add your own body, URL, and query inputs | [Controllers and extractors](http/controllers.md) |
| 5 | Share providers between features | [Providers and modules](architecture/modules.md) |

Start with MVC, the default. Learn [CQRS](architecture/mvc-cqrs.md) when you want
to separate write operations from read operations. You can use the same concepts
without adding a message bus or a second database.

## Already building something?

| I need to… | Go to |
| --- | --- |
| Understand generated files | [Project structure](architecture/project-structure.md) |
| Share Rust code across applications | [Shared libraries](cli/libraries.md) |
| Find a CLI command | [CLI reference](cli/commands.md) |
| Move from NestJS | [Coming from NestJS](getting-started/coming-from-nestjs.md) |
| Configure a port, limits, or environment | [Application configuration](configuration/application.md) |
| Test a route or replace a dependency in tests | [Testing modules and HTTP](testing/modules.md) |
| Connect PostgreSQL | [PostgreSQL](integrations/postgres.md) |
| Protect routes | [Guards and JWT](integrations/auth.md) |
| Fix a setup or startup problem | [Troubleshooting](operations/troubleshooting.md) |
| Update an existing generated application | [Adoption guide](getting-started/adopting-existing.md) |

## Add capabilities when you need them

Lifecycle management is part of the runtime. Database, JWT authentication, jobs,
caching, and CQRS buses are enabled through Cargo features and explicit provider
configuration. Start with the generated API, then follow the integration guide for
each capability. The [feature setup guide](getting-started/status.md) explains
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