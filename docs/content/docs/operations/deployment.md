---
title: "Deployment"
description: "Build, containerize and operate generated applications."
---

Deploy a generated application after it builds and passes tests locally. Follow the release/container instructions and supply your own production configuration and secrets.

From a generated application using registry dependencies:

```sh
docker build --tag my-app:local .
docker run --rm --name my-app \
  --read-only --cap-drop=ALL --security-opt=no-new-privileges \
  --pids-limit=128 --memory=512m --cpus=1 --stop-timeout=15 \
  -p 127.0.0.1:3000:3000 my-app:local
```

The Dockerfile builds a stripped static musl binary for amd64 or arm64 and runs
it in `gcr.io/distroless/static-debian13:nonroot`. Its container listener defaults
to `0.0.0.0:3000`. The final image contains no build tools, shell, package manager,
or curl. Configure HTTP health probes in the orchestrator.

Pass runtime configuration through environment variables or the deployment's
secret mechanism. `.env` is excluded from source control and the Docker context.
Allow a stop timeout longer than `SHUTDOWN_GRACE_SECONDS`. Local runtime path
dependencies require a custom Docker build context. Native dependencies must
support static musl linking if you retain the generated build configuration.

Replace in-memory repositories with durable storage, add application-specific
authentication/authorization, and terminate TLS at your ingress. Add readiness
checks for external dependencies when introducing them.

## Keep learning

[Development and packages](/docs/operations/contributing). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#deployment).
