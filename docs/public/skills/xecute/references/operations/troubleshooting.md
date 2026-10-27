Find the symptom closest to yours in the table below. Read the full command output first, and check whether you are in the generated application or the Xecute repository.

| Symptom | What to check |
| --- | --- |
| Installed CLI lacks `run dev` or middleware support | Update to CLI 0.1.7 or later and use runtime/macros 0.1.5 or later |
| Generated app cannot find `Application` | Update its runtime/macros to 0.1.5 or later |
| Locked build says the lockfile must change | Run `cargo generate-lockfile`, review it, and commit the result |
| Generator cannot register a resource | Run from the project root and retain the `// xecute:` markers in `app_module.rs` |
| Resource already exists | Choose another feature name or edit the existing feature; generators do not overwrite it |
| Middleware annotation produces an ordering error | Place it below the owning `#[controller]` or `#[routes]` macro |
| Wrong payload reaches a manually invoked handler | Automatic validation occurs through the generated HTTP extractor, not arbitrary Rust method calls |
| Browser cross-origin requests fail | Configure exact `cors.origins` plus the required methods and headers |
| Many clients share a rate-limit quota | Check whether a reverse proxy is the direct socket peer |
| Stored records disappear after restart | The generated repository is in memory; add durable storage |
| Container cannot resolve a local runtime dependency | Use registry dependencies or adapt the Docker build context |

## Keep learning

[Scope and limitations](limitations.md). Return to the [learning path](../index.md) whenever you need a different topic.