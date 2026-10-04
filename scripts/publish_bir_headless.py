#!/usr/bin/env python3
"""Tag bir-headless-vMAJOR.MINOR.PATCH so CI uploads the binary.

Does not cargo-publish, does not change the desktop Cargo version, and does
not read a registry token.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION_FILE = ROOT / "crates" / "bir-desktop" / "BIR_HEADLESS_VERSION"
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
TAG_PREFIX = "bir-headless-v"


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=True,
    )


def read_version() -> str:
    text = VERSION_FILE.read_text().strip()
    if not SEMVER.match(text):
        raise SystemExit(f"{VERSION_FILE} is not MAJOR.MINOR.PATCH: {text!r}")
    return text


def bump_patch(version: str) -> str:
    major, minor, patch = version.split(".")
    return f"{major}.{minor}.{int(patch) + 1}"


def ensure_main_clean() -> None:
    branch = git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if branch != "main":
        raise SystemExit(f"just publish-headless must be run on main (currently {branch})")
    dirty = git("status", "--porcelain").stdout.strip()
    if dirty:
        raise SystemExit("working tree is dirty; commit or stash before publish\n" + dirty)


def tag_exists(tag: str) -> bool:
    local = git("rev-parse", "-q", "--verify", f"refs/tags/{tag}", check=False)
    if local.returncode == 0:
        return True
    remote = git("ls-remote", "--tags", "origin", f"refs/tags/{tag}", check=False)
    if remote.returncode != 0:
        raise SystemExit(remote.stderr.strip() or "git ls-remote failed")
    return bool(remote.stdout.strip())


def run_visible(args: list[str]) -> None:
    result = subprocess.run(args, cwd=ROOT)
    if result.returncode != 0:
        raise SystemExit(f"{' '.join(args)} failed ({result.returncode})")


def main() -> None:
    if len(sys.argv) > 2:
        raise SystemExit("usage: publish_bir_headless.py [MAJOR.MINOR.PATCH]")
    requested = sys.argv[1] if len(sys.argv) == 2 else ""
    ensure_main_clean()
    current = read_version()
    new = requested or bump_patch(current)
    if not SEMVER.match(new):
        raise SystemExit(f"version must be MAJOR.MINOR.PATCH, got {new}")
    tag = f"{TAG_PREFIX}{new}"
    if tag_exists(tag):
        raise SystemExit(f"tag {tag} already exists; not moving it")
    if new != current:
        VERSION_FILE.write_text(new + "\n")
        git("add", "--", str(VERSION_FILE.relative_to(ROOT)))
        run_visible(["git", "commit", "-m", f"release: {tag}"])
    run_visible(["git", "push", "origin", "main"])
    run_visible(["git", "tag", "-a", tag, "-m", f"Release {tag}"])
    run_visible(["git", "push", "origin", tag])
    print(f"tagged {tag}; CI builds bir-headless. Not a crates.io publish.")


if __name__ == "__main__":
    main()
