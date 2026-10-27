---
title: "MVC and CQRS"
description: "Choose service-based MVC or direct-handler CQRS generation."
---

MVC uses a service to coordinate operations. CQRS gives each read or write its own handler. Both use DTOs and repositories; start with MVC if you are unsure.

MVC is the default resource layout:

```text
Request → DTO validation → Controller → Service → Repository → Response
```

Create a CQRS project with:

```sh
xecute new my-api --cqrs
cd my-api
xecute g resource user
```

Its resource uses command/query handlers that inject the repository:

```text
src/modules/user/
├── controller.rs
├── module.rs
├── mod.rs
├── commands/
│   ├── mod.rs
│   └── create.rs    # CreateUserCommand + CreateUserHandler
├── queries/
│   ├── mod.rs
│   └── list.rs      # ListUserQuery + ListUserHandler
├── dto/mod.rs
├── entity/mod.rs
└── repository/
    ├── mod.rs
    └── store.rs
```

```text
POST → DTO validation → Controller → CreateUserHandler → Repository write
GET  → Controller → ListUserHandler → Repository read
```

Controllers construct command/query inputs and invoke handlers through direct
dependency injection. The command handler coordinates writes and normalizes input;
the query handler reads records. Both inject the same singleton repository, so
writes are visible to subsequent reads. The module registers the repository and
exports its handlers. Add a focused business service when an operation needs
reusable domain logic. MVC keeps its existing `service.rs` layout.

Existing CQRS features are preserved; migrate their handlers and DI registrations
manually to adopt this layout.

Initial resources implement collection GET and create POST. Update, delete,
get-by-ID, pagination, and their handlers are application extensions. CQRS does
not require separate databases, event sourcing, events, or a message bus; these
are not generated automatically.

## Keep learning

[Providers and modules](/docs/architecture/modules). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#mvc-and-cqrs).
