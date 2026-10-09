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
LOCK = ROOT / "Cargo.lock"
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")
VERSION_LINE = re.compile(r'^version = "([^"]+)"\s*$', re.M)
BIR_LOCK_VERSION = re.compile(
    r'\[\[package\]\]\nname = "bir"\nversion = "([^"]+)"\n',
    re.M,
)
TAG_PREFIX = "bir-v"
ALLOWED_DIRTY = {"crates/bir/Cargo.toml", "Cargo.lock"}


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=True,
    )


def parse_semver(version: str) -> tuple[int, int, int]:
    match = SEMVER.match(version)
    if not match:
        raise SystemExit(f"version must be MAJOR.MINOR.PATCH, got {version}")
    return tuple(int(part) for part in match.groups())


def read_version() -> str:
    text = MANIFEST.read_text()
    match = VERSION_LINE.search(text)
    if not match:
        raise SystemExit(f"{MANIFEST} has no MAJOR.MINOR.PATCH version")
    parse_semver(match.group(1))
    return match.group(1)


def write_version(version: str) -> None:
    parse_semver(version)
    text = MANIFEST.read_text()
    new, count = VERSION_LINE.subn(f'version = "{version}"', text, count=1)
    if count != 1:
        raise SystemExit(f"could not update version in {MANIFEST}")
    MANIFEST.write_text(new)


def bump_patch(version: str) -> str:
    major, minor, patch = parse_semver(version)
    return f"{major}.{minor}.{patch + 1}"


def ensure_not_older(current: str, new: str) -> None:
    if parse_semver(new) < parse_semver(current):
        raise SystemExit(
            f"refusing to tag {new}; crates/bir is already {current}. "
            "Pass the current version to tag it, or a newer MAJOR.MINOR.PATCH."
        )


def lock_bir_version(text: str) -> str | None:
    match = BIR_LOCK_VERSION.search(text)
    return match.group(1) if match else None


def lock_has_bir_version(version: str, text: str | None = None) -> bool:
    body = LOCK.read_text() if text is None else text
    return lock_bir_version(body) == version


def publish_remote(remotes: list[str] | None = None) -> str:
    names = remotes if remotes is not None else git("remote").stdout.split()
    if "origin" in names:
        return "origin"
    if "public" in names:
        return "public"
    raise SystemExit(
        "no git remote named origin or public. "
        f"Have: {', '.join(names) if names else '(none)'}"
    )


def ensure_main_clean() -> None:
    branch = git("rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if branch != "main":
        raise SystemExit(f"just publish-bir must be run on main (currently {branch})")
    dirty = git("status", "--porcelain").stdout.strip()
    if dirty:
        raise SystemExit("working tree is dirty; commit or stash before publish\n" + dirty)


def tag_exists(tag: str, remote: str) -> bool:
    local = git("rev-parse", "-q", "--verify", f"refs/tags/{tag}", check=False)
    if local.returncode == 0:
        return True
    remote_tag = git("ls-remote", "--tags", remote, f"refs/tags/{tag}", check=False)
    if remote_tag.returncode != 0:
        raise SystemExit(remote_tag.stderr.strip() or "git ls-remote failed")
    return bool(remote_tag.stdout.strip())


def run_visible(args: list[str]) -> None:
    result = subprocess.run(args, cwd=ROOT)
    if result.returncode != 0:
        raise SystemExit(f"{' '.join(args)} failed ({result.returncode})")


def refresh_lock(version: str) -> None:
    """Rewrite Cargo.lock so --locked builds see the new path-package version."""
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--offline"],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"cargo metadata failed ({result.returncode}); "
            f"Cargo.lock was not updated for bir {version}"
        )
    if not lock_has_bir_version(version):
        raise SystemExit(f"Cargo.lock does not record bir {version}")


def porcelain_paths() -> set[str]:
    paths: set[str] = set()
    for line in git("status", "--porcelain").stdout.splitlines():
        path = line[3:]
        if " -> " in path:
            path = path.split(" -> ", 1)[1]
        paths.add(path.strip())
    return paths


def main() -> None:
    if len(sys.argv) > 2:
        raise SystemExit("usage: publish_bir.py [MAJOR.MINOR.PATCH]")
    requested = sys.argv[1] if len(sys.argv) == 2 else ""
    ensure_main_clean()
    remote = publish_remote()
    current = read_version()
    new = requested or bump_patch(current)
    parse_semver(new)
    ensure_not_older(current, new)
    tag = f"{TAG_PREFIX}{new}"
    if tag_exists(tag, remote):
        raise SystemExit(f"tag {tag} already exists; not moving it")
    if new != current:
        write_version(new)
        refresh_lock(new)
        unexpected = porcelain_paths() - ALLOWED_DIRTY
        if unexpected:
            raise SystemExit(
                "publish would commit unexpected files: " + ", ".join(sorted(unexpected))
            )
        git("add", "--", "crates/bir/Cargo.toml", "Cargo.lock")
        run_visible(["git", "commit", "-m", f"release: {tag}"])
    run_visible(["git", "push", remote, "main"])
    run_visible(["git", "tag", "-a", tag, "-m", f"Release {tag}"])
    run_visible(["git", "push", remote, tag])
    print(
        f"tagged {tag}. CI publishes crate bir to crates.io "
        "(needs Actions secret CARGO_REGISTRY_TOKEN on hexuria/buwiz-forms) "
        "and uploads cargo-binstall archives. Not the desktop v* release."
    )


if __name__ == "__main__":
    main()
