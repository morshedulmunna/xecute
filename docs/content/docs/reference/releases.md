---
title: "Release process"
description: "Understand crate versions, tags and generated GitHub release notes."
---

This page is for maintainers publishing Xecute packages. Application users normally want the CLI update guide instead.

See [CHANGELOG.md](/docs/reference/changelog) for version-by-version changes and
[GitHub Releases](https://github.com/morshedulmunna/xecute/releases) for published
release notes. Git tags alone do not create GitHub Release pages.

Maintain an `## [VERSION] - YYYY-MM-DD` section in `CHANGELOG.md` before releasing.
Publish the crate, then push the matching CLI tag, such as `v0.1.10`. The
[release workflow](https://github.com/morshedulmunna/xecute/blob/main/.github/workflows/release.yml) copies that exact changelog
section into the GitHub Release body and adds the versioned installation command.
It creates a missing release or updates the notes of an existing release without
moving its source tag. Historical CLI tags can be backfilled through the
workflow's **Run workflow** input.

Changelog/workflow changes on `main` also synchronize the latest changelog
version when its tag already exists; an untagged future version is deferred.
This initializes the existing latest release when release automation is first
added. It does not publish crates or create version tags automatically.

Preview the release body locally:

```sh
python3 scripts/release_notes.py --tag v0.1.10 --output /tmp/xecute-release-notes.md
```

When manually using GitHub's **Create a new release** page, **Generate release
notes** uses the PR-label categories in [.github/release.yml](https://github.com/morshedulmunna/xecute/blob/main/.github/release.yml).
For the curated notes in this repository, use the changelog-backed workflow or
paste the corresponding changelog section into the release description.

## Keep learning

[Changelog](/docs/reference/changelog). Return to the [learning path](/docs) whenever you need a different topic.

---

[Maintained source](https://github.com/morshedulmunna/xecute/blob/main/README.md#changelog-and-github-releases).
