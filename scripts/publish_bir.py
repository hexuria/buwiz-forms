#!/usr/bin/env python3
"""Tag bir-vMAJOR.MINOR.PATCH so CI publishes the bir crate and binstall artifacts.

Does not cargo-publish, does not read a registry token, and does not tag the
desktop v* release.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "crates" / "bir" / "Cargo.toml"
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
VERSION_LINE = re.compile(r'^version = "([^"]+)"\s*$', re.M)
TAG_PREFIX = "bir-v"


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=True,
    )


def read_version() -> str:
    text = MANIFEST.read_text()
    match = VERSION_LINE.search(text)
    if not match or not SEMVER.match(match.group(1)):
        raise SystemExit(f"{MANIFEST} has no MAJOR.MINOR.PATCH version")
    return match.group(1)


def write_version(version: str) -> None:
    text = MANIFEST.read_text()
    new, count = VERSION_LINE.subn(f'version = "{version}"', text, count=1)
    if count != 1:
        raise SystemExit(f"could not update version in {MANIFEST}")
    MANIFEST.write_text(new)


def bump_patch(version: str) -> str:
    major, minor, patch = version.split(".")
    return f"{major}.{minor}.{int(patch) + 1}"


def ensure_main_clean() -> None:
    branch = git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if branch != "main":
        raise SystemExit(f"just publish-bir must be run on main (currently {branch})")
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
        raise SystemExit("usage: publish_bir.py [MAJOR.MINOR.PATCH]")
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
        write_version(new)
        git("add", "--", str(MANIFEST.relative_to(ROOT)))
        run_visible(["git", "commit", "-m", f"release: {tag}"])
    run_visible(["git", "push", "origin", "main"])
    run_visible(["git", "tag", "-a", tag, "-m", f"Release {tag}"])
    run_visible(["git", "push", "origin", tag])
    print(
        f"tagged {tag}. CI publishes crate bir to crates.io "
        "(needs Actions secret CARGO_REGISTRY_TOKEN on hexuria/buwiz-forms) "
        "and uploads cargo-binstall archives. Not the desktop v* release."
    )


if __name__ == "__main__":
    main()
