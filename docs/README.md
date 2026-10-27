# Xecute documentation

Documentation website for this repository: Vite, React Router, Fumadocs and
Tailwind. Content describes the actual CLI/runtime/macros and distinguishes
published behavior from source-only opt-in features. Nothing is published by
building or previewing this site.

## Run and verify

```sh
cd docs
bun install --frozen-lockfile
bun run dev
# Open http://localhost:3002
```

```sh
bun run sync:content
bun run check:docs
bun run check:render
bun run check-types
bun run lint
bun run build
bun run preview
```

The production output is `dist/`, including the SPA `404.html` fallback.
Deploy it to a static host with history fallback for direct `/docs/...` links.
For a subpath deployment, set `VITE_CDN_URL=/your-base/` when building. Asset URLs
and the React router use that base. Set `SITE_URL` to the actual public origin
and base path to emit a sitemap and matching robots entry. Without `SITE_URL`,
the build makes no deployment-domain assumption and omits the sitemap.

## Maintained sources

| Source | Purpose |
| --- | --- |
| `../README.md` | Current CLI, generated architecture, HTTP and operational reference |
| `../xecute/README.md` | Base runtime and async lifecycle guide |
| `../xecute/docs/*.md` | Detailed PostgreSQL, auth, jobs, cache and CQRS guides |
| `../CHANGELOG.md` | Unreleased work and published version history |
| `handbook/*.md` | Learning path, first-API and first-feature tutorials, request flow, availability, adoption and examples |
| `content-manifest.json` | Website routes, source sections and sidebar order |
| `content/docs/` | Generated Markdown/frontmatter/navigation; do not edit directly |

`sync:content` extracts maintained sections, rewrites internal/source links, adds
local-feature availability notices and generates sidebar metadata. Edit the source
identified in the manifest, then sync. This avoids separately maintained copies
of runtime integration guides. Generated Markdown remains inspectable in Git.

`check:docs` fails on stale generated pages, broken internal routes, missing
navigation entries, foreign template content, invalid frontmatter/code fences,
undocumented Cargo flags and CLI version drift. Run it after any source guide or
feature/version change. New topics require a manifest entry and sidebar placement.

`check:render` server-renders every compiled MDX article and the homepage, verifies
rendered link anchors, checks runtime sidebar coverage and ensures search indexes
both pages and section headings. It requires no browser or external service.

## Navigation

- Getting started: installation, availability, existing-app adoption and NestJS mapping.
- CLI: commands, watcher, workspaces, updating/uninstalling.
- Architecture: layouts, MVC/CQRS, modules, scopes and lifecycle.
- HTTP: controllers/extractors, DTOs, middleware, OpenAPI, errors, versions and files.
- Configuration: TOML/environment, typed config, logging and server defaults.
- Integrations: PostgreSQL, guards/JWT, jobs, caching and CQRS buses.
- Testing: quality checks, overrides/HTTP tests and complete examples.
- Operations/reference: deployment, troubleshooting, limits, feature flags and releases.

## Writing conventions

Start a tutorial with its outcome and prerequisites. Say which directory a command
runs in, whether the server should stay running, and what status or result to expect.
Explain new terms where they first appear. Mark code fragments as fragments; do not
imply that an endpoint or integration already exists. End with a concrete next step.

Reference pages can set `intro` and `next` in `content-manifest.json` to give readers
context and a follow-up guide without duplicating their maintained API reference.
The first learning path is quick start → first feature → request flow → controllers
→ modules. Keep published commands distinct from unpublished source behavior.

Use real `xecute` APIs and exact CLI flags from this checkout. State required Cargo
features, external services, provider registration and lifecycle setup. Separate
implemented source behavior from published packages and generated defaults.
Distinguish fragments from runnable programs, and link to the complete tested
example when a full application is needed. Include meaningful failure behavior
and material limits, such as in-process job/event delivery or explicit auth setup.

Keep internal links as `/docs/<route>` in handbook sources. Relative repository
links in maintained guides are converted to website routes where available and
GitHub source links otherwise. The repository README remains useful directly on
GitHub, and runtime guides remain available inside the runtime package.

The documentation shell's retained third-party license is in `LICENSE`; Xecute
runtime/CLI/macros use the license declared in their Cargo manifests.
