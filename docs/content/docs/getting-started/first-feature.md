---
title: "Your first feature"
description: "Generate a user feature, find the right files, change a rule, and run tests."
---

You already have a running example API. Now add a `user` feature and learn which
file to change for each kind of work. This guide follows an MVC project created
in the [quick start](/docs/getting-started/quickstart).

## 1. Generate the feature

In a terminal at your **application root**, beside `Cargo.toml`, run:

```sh
xecute g resource user
```

`g` is short for `generate`. A resource creates the connected files and registers
the module and its routes. You do not need to add the generated router manually.
If the development server is running, wait for its rebuild to finish.

The route is singular: `/user`, not `/users`.

## 2. Find the files you will use

```text
src/modules/user/
├── module.rs           # Registers providers and exports the service
├── controller.rs       # HTTP routes and request/response handling
├── service.rs          # Coordinates application operations
├── dto/mod.rs          # Accepted request fields and validation
├── entity/mod.rs       # The record returned by your feature
├── repository/store.rs # Stores and retrieves records
└── tests/http.rs       # Tests requests through the feature router
```

A DTO describes input. An entity represents a record. A repository owns data
access. A service coordinates the work. The controller connects HTTP to that work.
For a complete directory map, see [Project structure](/docs/architecture/project-structure).

## 3. Create and list a user

With the server running:

```sh
curl -i -X POST http://127.0.0.1:3000/user \
  -H 'Content-Type: application/json' \
  -d '{"name":"Munna"}'

curl -i http://127.0.0.1:3000/user
```

Expect `201 Created` for POST and `200 OK` for GET. The collection should contain
the new user. The generated repository is in memory, so restarting clears it.

The generator provides **list and create**. Get-by-ID, update, delete, pagination,
login, and database persistence are not generated endpoints.

## 4. Follow one request

```text
POST /user
  → dto/mod.rs: decode and validate CreateUserDto
  → controller.rs: call UserService
  → service.rs: trim the name and call UserRepository
  → repository/store.rs: save the record
  → controller.rs: return 201 and the record
```

Open these files in this order. Notice that the controller has an
`Arc<UserService>` field: Xecute injects the registered service. The service has
an `Arc<UserRepository>` field for the same reason.

## 5. Change a validation rule

Open `src/modules/user/dto/mod.rs`. The generated name rule currently allows
3–100 characters. Change the maximum from `100` to `50` in **both** attributes:

```rust
#[schemars(length(min = 3, max = 50))]
#[validate(length(min = 3, max = 50), custom(function = "non_blank"))]
```

This is an attribute fragment: replace the two attributes on the existing `name`
field, keeping its type and the generated `non_blank` function.

`validate` enforces the rule at runtime. `schemars` describes it in the API schema.
Keeping both in sync makes Swagger match what the server accepts.

Save the file and wait for the rebuild. Send a 51-character name:

```sh
curl -i -X POST http://127.0.0.1:3000/user \
  -H 'Content-Type: application/json' \
  -d '{"name":"123456789012345678901234567890123456789012345678901"}'
```

Expect `422`. A valid name such as `Munna` should still return `201`.

## 6. Run the tests

From the application root:

```sh
cargo test user
cargo fmt --all -- --check
```

The generated HTTP tests cover the feature router. Add a test for your new
51-character rejection beside those tests; check the status and verify that
invalid input did not create a record. See [Testing modules and HTTP](/docs/testing/modules)
for request assertions and provider overrides.

## Where should my next change go?

| I want to… | Start here |
| --- | --- |
| Accept another JSON field | `dto/mod.rs`; then pass it through the service and storage as needed |
| Add a URL parameter or query filter | `controller.rs` and a DTO in `dto/mod.rs` |
| Add an application operation | `service.rs` |
| Change how data is saved | `repository/store.rs` |
| Use a provider from another feature | `module.rs` imports and the owning module’s exports |
| Protect a route | [Guards and JWT](/docs/integrations/auth), checking availability first |

## Using CQRS instead?

In a new project, select CQRS before generating resources:

```sh
xecute new query-api --cqrs
cd query-api
xecute g resource user
```

The controller calls `CreateUserHandler` and `ListUserHandler` instead of a
`UserService`. Look in `commands/create.rs` and `queries/list.rs`. DTOs, repositories,
and HTTP validation follow the same ideas. Read [MVC and CQRS](/docs/architecture/mvc-cqrs)
when you want to understand that choice.

Next, learn [How a request works](/docs/getting-started/request-flow) and then
[Controllers and extractors](/docs/http/controllers).

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/docs/handbook/first-feature.md).
