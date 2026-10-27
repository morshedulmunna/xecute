---
title: "Welcome and startup URLs"
description: "Discover enabled documentation, static assets and route version groups."
---

Use the welcome response and startup output to find your application’s enabled URLs. Only configured routes and documents appear.

`GET /` returns HTTP 200 with the application name, `status: "ok"`, a welcome
message, and `documentation: "/docs/"`. With `docs.enabled = false` in `config/AppConfig.toml`, the
`documentation` field is null. Other unregistered paths still return the normal
404 envelope. The root route is defined in `src/server/router.rs` and can be
customized there.

Startup emits one `server listening` JSON log with the actual bound address and
local URL, including when binding port zero. `GET /` exposes the address, URL,
Swagger and OpenAPI paths, versioned documentation links, and static-file mount.
Disabled endpoints are null or omitted from the version list. Wildcard bindings
use loopback URLs and IPv6 addresses are bracketed. Configure public URLs
separately when running behind a TLS ingress or reverse proxy.

Available starting with CLI 0.1.10. Existing projects need the corresponding
server changes; updating the CLI does not rewrite them. The runtime crate is unchanged.

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#welcome-route-and-startup-urls).
