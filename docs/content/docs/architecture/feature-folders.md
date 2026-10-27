---
title: "Feature folders"
description: "Keep DTOs, entities, repositories and tests together by feature."
---

Keep one feature’s request types, records, storage, and tests together. Use this guide when your feature starts growing beyond its controller.

All newly generated resources and standalone modules, controllers, and services
live under `src/modules/<feature>/`. Each resource keeps its controller, DI module,
DTOs, entities, repository, and HTTP tests together. CQRS resources also contain
commands and queries. `src/modules/mod.rs` declares each feature;
`AppModule` imports and mounts it through `crate::modules::<feature>`.

```text
src/
├── app_module.rs
├── modules/
│   ├── mod.rs
│   └── user/
│       ├── mod.rs
│       ├── module.rs
│       ├── controller.rs
│       ├── commands/
│       ├── queries/
│       ├── dto/
│       ├── entity/
│       ├── repository/
│       └── tests/
│           ├── mod.rs
│           └── http.rs
└── tests/                 # Shared server/infrastructure tests
```

MVC resources use `service.rs` inside their feature folder instead of CQRS
commands/queries/services directories. This layout is available starting with
CLI 0.1.9; existing root-level features are preserved rather than moved.

## Keep learning

[Provider scopes](/docs/architecture/scopes). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#feature-module-folders).
