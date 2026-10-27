---
title: "Static files"
description: "Serve explicitly public assets without exposing application files."
---

Serve files that are intentionally public. Start with the generated public directory and /static/ prefix, then review path and fallback behavior before changing them.

Newly generated applications include `public/index.html`, served at `/static` and
`/static/`, plus `public/hello.txt`, served at
`http://127.0.0.1:3000/static/hello.txt`. Static serving is enabled by default,
without environment settings. Add files to `public/` and access them at `/static/<filename>`.
For example, `public/images/logo.png` is served at `/static/images/logo.png`.
To customize the directory, prefix, or enabled state, edit `StaticConfig` in
`src/configs/static_files.rs`. Relative directories use the application's working
directory; the public directory must exist and cannot be the application root or a symlink.

Files support GET, HEAD, MIME types, conditional requests, and byte ranges through
Tower HTTP `ServeDir`. Directory listings and nested directory indexes are disabled. The mount root
serves `public/index.html`; other assets use explicit filenames. Hidden path components, parent traversal, and
symlinked assets are rejected. Keep public directories limited to intended public
assets; static files share the application's CORS, request IDs, security headers,
logging, rate limits, and global middleware. HTML retains the restrictive API CSP;
this is asset serving, not a generated SPA hosting configuration.

The Docker image copies `public/` into the runtime image. For another static directory,
adapt the Dockerfile or mount the asset directory read-only. Change files in the
public directory directly or mount updated assets; there is no binary embedding.

## Keep learning

[Choose your next guide](/docs). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#static-files).
