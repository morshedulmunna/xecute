---
title: "CLI reference"
description: "Project creation, resource generators and supported flags."
---

Use this page when you know what you want to generate and need the exact command or flag. Run resource generators beside your application’s Cargo.toml.

```text
xecute new <project|.> [--name <package-name>] [--mvc | --cqrs] [--runtime-path <path>]
xecute update
xecute uninstall
xecute run dev [--bin <name>] [--features <list>] [--watch <path>] [-- <app arguments>]
xecute generate <resource|module|controller|service> <name>
xecute generate worker <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]
xecute g <res|mo|co|s> <name>
xecute g wo <name> [--distributed] [--broker <postgres-outbox|rabbitmq|nats>]
xecute --help
xecute --version
```

| Command | Result |
| --- | --- |
| `xecute new my-app` | MVC application with a working example resource |
| `xecute new .` | Initialize the current directory using its name as the package name |
| `xecute new . --name my-app` | Initialize the current directory with an explicit package name |
| `xecute update` | Install the latest crates.io CLI into the running CLI's Cargo installation root |
| `xecute uninstall` | Remove the Cargo-installed CLI; application projects are retained |
| `xecute new my-app --mvc` | Explicit MVC selection |
| `xecute new my-app --cqrs` | Application with command/query handlers |
| `xecute new my-project --workspace` | Cargo workspace with its first MVC application under `apps/` |
| `xecute g apps admin [--mvc \| --cqrs]` | Add an app from the workspace root; inherit architecture unless overridden |
| `xecute g library shared-utils` or `xecute g lib shared-utils` | Create a reusable crate under `libs/` from the workspace root |
| `xecute run dev` | Rebuild and restart the debug application when watched files change |
| `xecute g resource user` or `xecute g res user` | Resource, module registration, routes, schemas, repository, and HTTP test |
| `xecute g module billing` or `xecute g mo billing` | Registered module with an empty provider list |
| `xecute g controller status` or `xecute g co status` | Standalone injectable controller with a GET endpoint |
| `xecute g service mailer` or `xecute g s mailer` | Standalone injectable service |
| `xecute g worker email` or `xecute g wo email` | Registered process-local job queue with payload, handler, and `jobs` feature |
| `xecute g wo email --distributed` | Durable worker backed by the default PostgreSQL outbox |
| `xecute g wo email --broker rabbitmq` | Durable RabbitMQ worker; `--broker` implies `--distributed` |
| `xecute g wo email --distributed --broker nats` | NATS queue-group worker with at-most-once delivery |

Run feature generators from the generated project root. Run `g apps` and
`g library` from the workspace root. Standalone controllers and
services require manual provider/route registration when you decide to use them.
Standalone generators remain available in both architecture modes.

Workers are registered automatically and enable the runtime's `jobs` feature.
The default in-process queue provides 64 waiting slots and four workers with
three attempts, a 250 ms retry delay, and a 30-second attempt timeout. Queues
drain when the generated server shuts down. Jobs are process-local and are not
persisted or shared across machines.

Add `--distributed` to generate a durable producer and consumer. `--broker`
accepts `postgres-outbox` (the default), `rabbitmq`, or `nats`, and implies
`--distributed`:

- `postgres-outbox` stores jobs in `xecute_<feature>_jobs`, claims them with
  `FOR UPDATE SKIP LOCKED`, and reads `DATABASE_URL`.
- `rabbitmq` consumes a durable `<feature>_jobs` queue and reads `RABBITMQ_URL`
  (default `amqp://127.0.0.1:5672/%2f`). Failed deliveries are retried up to
  three times.
- `nats` uses a `<feature>-workers` queue group on `<feature>.jobs` and reads
  `NATS_URL` (default `nats://127.0.0.1:4222`). Core NATS delivery is
  at-most-once; handler failures are logged without redelivery.

Distributed workers expose `<Feature>Queue::enqueue(...)` and `shutdown()`
instead of `JobQueue`. Generate workers from an application using the current
async container startup.

Names must start with a lowercase ASCII letter and contain lowercase letters,
digits, hyphens, or underscores. Rust keywords and empty name segments are
rejected. `user-profile` becomes the module `user_profile` and types such as
`UserProfileController`. Existing feature paths are not overwritten.

The CLI saves the architecture as `mvc` or `cqrs` in `.xecute/architecture`.
Commit this file. Subsequent resource generation follows that selection; projects
without it retain the legacy MVC layout. Invalid values stop generation.
Changing the file changes future generation, not existing feature code.

Keep the `// xecute:` registration markers in `src/app_module.rs`. Generators
use them to add module imports, container definitions, routes, and API metadata.

## Keep learning

[Development runner](/docs/cli/development). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#cli-reference).
