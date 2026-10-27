---
title: "Quality checks"
description: "Run runtime, generator and generated-application checks."
---

Run these commands from the generated application root before shipping a change. The page distinguishes application checks from checks for contributors working on Xecute itself.

In a generated application:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo build --release --locked
```

New standalone projects resolve dependencies with `cargo generate-lockfile` during
setup, including `--runtime-path` projects. This selects the latest stable versions
within the template’s compatibility ranges, instead of copying a bundled lockfile.
Registry access is required unless Cargo is configured for offline cached resolution.
Resolution failures return an error and preserve generated files; fix registry/network
access and rerun `cargo generate-lockfile` in the project. Commit `Cargo.lock` for
reproducible builds. Breaking major upgrades require updated templates. Existing
workspace apps retain shared dependency versions when adding another app; setup
does not force upgrades across an existing workspace.

Resource HTTP tests verify rejection before persistence and visibility of created
records through subsequent reads. Server tests cover configuration, quota and
concurrency behavior, timeouts, body limits, response envelopes, CORS, request IDs,
logging, and Swagger routes. The workspace middleware tests cover scope ordering,
early returns, validation boundaries, and compatibility with legacy controllers.

The generator also includes CI, rustfmt/Clippy configuration, EditorConfig,
Dependabot configuration, and `deny.toml`. To run the dependency policy check:

```sh
cargo install cargo-deny --locked
cargo deny --locked check
```

Document public items with `///` and modules with `//!`. Missing documentation
produces compiler warnings; Clippy CI treats warnings as errors.

## Keep learning

[Testing modules and HTTP](/docs/testing/modules). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#testing-and-quality-checks).
