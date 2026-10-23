#!/usr/bin/env python3
"""Extract one stable CLI release from CHANGELOG.md for GitHub Releases."""

import argparse
import os
from pathlib import Path
import re
import subprocess

HEADING = re.compile(r"^## \[(\d+\.\d+\.\d+)\](?: - \d{4}-\d{2}-\d{2})?\s*$", re.MULTILINE)
TAG = re.compile(r"v\d+\.\d+\.\d+\Z")


def release_notes(changelog: str, tag: str | None) -> tuple[str, str, bool]:
    headings = list(HEADING.finditer(changelog))
    if not headings:
        raise ValueError("CHANGELOG.md has no versioned release sections")
    selected = tag or f"v{headings[0].group(1)}"
    if not TAG.fullmatch(selected):
        raise ValueError("expected a stable CLI tag such as v0.1.10")
    heading = next((h for h in headings if f"v{h.group(1)}" == selected), None)
    if heading is None:
        raise ValueError(f"CHANGELOG.md has no section for {selected}")
    remainder = changelog[heading.end():]
    boundary = re.search(r"^## |^\[[^\]]+\]:", remainder, re.MULTILINE)
    body = remainder[:boundary.start()] if boundary else remainder
    body = body.strip()
    if not body:
        raise ValueError(f"empty changelog section for {selected}")
    version = selected[1:]
    body += (f"\n\n## Install or update\n\n```sh\n"
             f"cargo install xecute-cli --version {version} --locked --force\n```\n\n"
             "Existing applications are not regenerated when the CLI is updated.\n\n"
             "[Full changelog](https://github.com/morshedulmunna/xecute/blob/main/CHANGELOG.md)\n")
    return selected, body, selected == f"v{headings[0].group(1)}"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", default="", help="Existing CLI tag; defaults to newest changelog version")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        tag, notes, latest = release_notes(Path("CHANGELOG.md").read_text(), args.tag or None)
    except ValueError as error:
        parser.error(str(error))
    exists = subprocess.run(["git", "rev-parse", "--verify", f"refs/tags/{tag}"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0
    args.output.write_text(notes)
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a") as file:
            file.write(f"tag={tag}\nexists={str(exists).lower()}\nlatest={str(latest).lower()}\n")
    print(f"Prepared notes for {tag}; existing tag: {exists}")


if __name__ == "__main__":
    main()
