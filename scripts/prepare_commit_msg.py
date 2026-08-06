#!/usr/bin/env -S uv run --script
#
# /// script
# requires-python = ">=3.13"
# dependencies = []
# ///

"""git `prepare-commit-msg` hook.

Regenerates CHANGELOG.md and prefills a "build: bump version to X"
commit message, but only when the staged root Cargo.toml's
`[workspace.package].version` already matches what
`git cliff --bumped-version` computes as the next version from
conventional-commit history. Any other commit is left untouched -- this
runs before the commit object is created, so staging CHANGELOG.md here
lands it in the same commit, no `git commit --amend` needed.

See https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks for the
positional arguments Git passes to this hook.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path


def staged_workspace_version() -> str | None:
    """The `[workspace.package].version` from the staged root Cargo.toml,
    or None if it can't be determined (file not staged, invalid TOML, or
    the key is missing)."""
    try:
        content = subprocess.run(
            ["git", "show", ":Cargo.toml"],
            capture_output=True,
            check=True,
            text=True,
        ).stdout
    except subprocess.CalledProcessError:
        return None
    try:
        data = tomllib.loads(content)
    except tomllib.TOMLDecodeError:
        return None
    return data.get("workspace", {}).get("package", {}).get("version")


def cliff_bumped_version() -> str | None:
    """What `git cliff --bumped-version` would bump to next, based on
    conventional-commit history up to HEAD -- i.e. not counting the
    commit currently in progress. None if `git cliff` is unavailable or
    fails (e.g. no commits to bump from yet)."""
    try:
        result = subprocess.run(
            ["git", "cliff", "--bumped-version"],
            capture_output=True,
            check=True,
            text=True,
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None
    return result.stdout.strip().removeprefix("v") or None


def regenerate_changelog() -> None:
    """Writes CHANGELOG.md and stages it."""
    subprocess.run(
        ["git", "cliff", "--offline", "--bump", "--output", "CHANGELOG.md"],
        check=True,
    )
    subprocess.run(["git", "add", "CHANGELOG.md"], check=True)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "msg_file",
        type=Path,
        help="path to the file holding the commit message so far",
    )
    parser.add_argument(
        "commit_source",
        nargs="?",
        default="",
        help="how the message was produced: message, template, merge, squash, or commit",
    )
    parser.add_argument(
        "sha1",
        nargs="?",
        default="",
        help="commit SHA-1, only set when amending an existing commit",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)

    # Merges, squashes, and amends already carry a message worth keeping
    # as-is -- only plain commits (explicit -m/-F, or the default editor
    # invocation) are candidates for the auto-generated bump message.
    if args.commit_source in ("merge", "squash", "commit"):
        return 0

    staged_version = staged_workspace_version()
    bumped_version = cliff_bumped_version()
    if staged_version is None or bumped_version is None or staged_version != bumped_version:
        return 0

    regenerate_changelog()

    if args.msg_file.read_text().strip():
        return 0  # an explicit message (-m/-F) is already provided

    args.msg_file.write_text(f"build: bump version to {staged_version}\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
