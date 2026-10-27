---
title: "API version groups"
description: "Mount explicit versioned routes and separate API documents."
---

Use explicit version groups when clients need different API contracts at the same time. Each group can have its own routes and API document.

Generated applications register only v1 in
`src/server/versions.rs`:

| URL | Contract |
| --- | --- |
| `/example` | Existing unversioned resource alias |
| `/v1/example` | Version-one resources, using the same v1 service/container |
| `/docs/v1/` | Version-one Swagger UI |
| `/docs/v1/openapi.json` | Version-one contract, with `/v1/...` paths |

`/docs/` and `/docs/openapi.json` continue to describe the unversioned aliases.
No v2 module, routes, or documentation are generated. Additional version groups
can be registered explicitly using the runtime API when needed.

```rust
use xecute::{ApiVersion, ApiVersions};

let versions = ApiVersions::new()
    .with(ApiVersion::new("v1", v1_router, V1Controller::api_operations())?)?;
let router = versions.mount(axum::Router::new());
let documents = versions.documents("My API")?;
```

Supply controller paths relative to the group; the router and documented paths
receive the same prefix. Canonical labels are `v1`, `v2`, etc, with positive u32
version numbers. Invalid labels, duplicate groups, and conflicting contracts fail
before serving. The same operation ID/schema name may appear in different
version documents. Global application policies wrap all groups.

```toml
[api]
versioning_enabled = true

[docs]
enabled = true
```

Set `api.versioning_enabled = false` to keep only unversioned aliases. Set
`docs.enabled = false` to hide all Swagger/OpenAPI routes while keeping API routes
active. `GET /` lists each enabled version's Swagger and OpenAPI paths.
The registration pattern is explicit URL versioning; header-based versioning and
automatic deprecation/sunset behavior are not implied.

## Keep learning

[Multipart uploads](/docs/http/uploads). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#api-version-groups-and-separate-openapi-documents).
