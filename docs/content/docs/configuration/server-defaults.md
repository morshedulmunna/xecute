---
title: "Server defaults"
description: "Understand request limits, health endpoints and graceful shutdown policies."
---

Understand the policies your generated server already applies, including limits and health endpoints. Review this page before changing middleware or deploying.

Generated applications include the following HTTP policies:

| Policy | Default behavior |
| --- | --- |
| Logging | One flat JSON completion log per request, filtered by `logging.filter` |
| Correlation | UUID `X-Request-Id` response header |
| Rate limit | 60 requests per socket peer IP per 60-second fixed window |
| Limiter capacity | At most 10,000 tracked IPs per process |
| Concurrency | At most 256 active handlers; overload returns `503` |
| Deadline | 30 seconds until response headers; expiry returns `504` |
| Body limit | 1 MiB; oversized declared or consumed bodies return `413` |
| CORS | No allowed cross-origin browser origins by default |
| Response headers | `nosniff`, frame denial, no-referrer, restrictive API CSP |
| Errors | Safe JSON envelopes for rejection and failure responses |
| Shutdown | Ctrl-C/SIGTERM handling with a 10-second drain deadline |
| Health | Liveness and startup-readiness probes bypass built-in quotas/concurrency limits |

Logs omit payloads, authorization headers, cookies, query strings, and full URLs.
Matched requests log the route template; unmatched requests may log the requested
path. Streaming-body completion is not included in latency measurements.

The timeout and concurrency boundary ends when response headers are produced.
Timeouts cancel the handler future but cannot undo completed writes or stop
separately spawned tasks. Readiness indicates startup success until you add
checks for databases and other dependencies.

## Keep learning

[Welcome and startup URLs](/docs/configuration/welcome). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#server-defaults).
