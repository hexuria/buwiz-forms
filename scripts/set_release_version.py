#!/usr/bin/env python3
"""Set the desktop release version in every release manifest."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

VERSION_RE = re.compile(r"\d+\.\d+\.\d+")
CARGO_VERSION_RE = re.compile(r'(?m)^(version = ")[^"]+(")')
NSH_JSON_VERSION_RE = re.compile(r'(?m)^(  "version": ")[^"]+(")')
INTERNAL_DEPENDENCY_VERSION_RE = re.compile(
    r'(?m)^(bir-(?:core|rules)\s*=\s*\{[^\n]*version = ")[^"]+(")'
)
LOCKFILE_MANIFESTS = (
    Path("Cargo.toml"),
    Path("nsh/Cargo.toml"),
)
LOCKFILE_PATHS = (
    Path("Cargo.lock"),
    Path("nsh/Cargo.lock"),
)
# The rules generator stamps its own version (the workspace's) into this manifest, and CI's
# `bir-rules-codegen check` compares the two. A bump that leaves it behind fails the first
# pull request that touches the core crates (it happened with 0.2.1).
RULES_CODEGEN_MANIFEST = Path("crates/bir-rules-codegen/Cargo.toml")
GENERATED_RULES_MANIFEST = Path("crates/bir-rules/src/generated/manifest.json")
ROLLBACK_PATHS = (
    Path("Cargo.toml"),
    Path("crates/bir-core/Cargo.toml"),
    Path("crates/bir-print/Cargo.toml"),
    Path("nsh/Cargo.toml"),
    Path("nsh/nsh.json"),
    *LOCKFILE_PATHS,
    GENERATED_RULES_MANIFEST,
)


def set_version(root: Path, version: str) -> None:
    if VERSION_RE.fullmatch(version) is None:
        raise ValueError(f"release version must be MAJOR.MINOR.PATCH: {version}")

    paths = [
        (root / "Cargo.toml", CARGO_VERSION_RE),
        (root / "crates/bir-core/Cargo.toml", INTERNAL_DEPENDENCY_VERSION_RE),
        (root / "crates/bir-print/Cargo.toml", CARGO_VERSION_RE),
        (root / "crates/bir-print/Cargo.toml", INTERNAL_DEPENDENCY_VERSION_RE),
        (root / "nsh/Cargo.toml", CARGO_VERSION_RE),
        (root / "nsh/nsh.json", NSH_JSON_VERSION_RE),
    ]

    # bir-print has two independent replacements in one file. Group their
    # validation before any write so a failure leaves every file unchanged.
    grouped: dict[Path, list[re.Pattern[str]]] = {}
    for path, pattern in paths:
        grouped.setdefault(path, []).append(pattern)
    updates = []
    for path, patterns in grouped.items():
        text = path.read_bytes().decode("utf-8")
        for pattern in patterns:
            if len(pattern.findall(text)) != 1:
                raise ValueError(f"{path}: expected exactly one release version")
            text = pattern.sub(
                lambda match: f"{match.group(1)}{version}{match.group(2)}",
                text,
                count=1,
            )
        updates.append((path, text.encode("utf-8")))

    for path, content in updates:
        path.write_bytes(content)


def refresh_lockfiles(root: Path) -> None:
    for relative in LOCKFILE_MANIFESTS:
        manifest = root / relative
        if not manifest.exists():
            continue
        command = [
            "cargo",
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            str(manifest),
        ]
        offline = subprocess.run(
            [*command, "--offline"],
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        if offline.returncode == 0:
            continue
        online = subprocess.run(
            command,
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        if online.returncode != 0:
            detail = online.stderr.strip() or offline.stderr.strip()
            missing = ("failed to read", "failed to load", "missing", "no such file", "does not exist")
            if relative.as_posix() == "nsh/Cargo.toml" and any(
                marker in detail.lower() for marker in missing
            ):
                detail = (
                    f"{detail}; needs ../nsh checked out at NSH_PINNED_REF "
                    "from .github/workflows/nsh.yml"
                )
            raise RuntimeError(f"cargo metadata failed for {relative}: {detail}")


def refresh_generated_rules(root: Path) -> None:
    """Regenerate the rules crate's manifest so it names the new version, then check it."""
    if not (root / RULES_CODEGEN_MANIFEST).exists():
        return
    for step in ("generate", "check"):
        result = subprocess.run(
            ["cargo", "run", "--locked", "-p", "bir-rules-codegen", "--", step],
            cwd=root,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            detail = (result.stderr or "").strip().splitlines()[-1:] or ["no output"]
            raise RuntimeError(f"bir-rules-codegen {step} failed: {detail[0]}")


def current_version(root: Path) -> str | None:
    match = CARGO_VERSION_RE.search((root / "Cargo.toml").read_bytes().decode("utf-8"))
    return match.group(0).split('"')[1] if match else None


def tests_naming(root: Path, version: str) -> list[str]:
    """Test files under nsh/ that still spell out `version` (they compare it with the running app)."""
    found = []
    nsh = root / "nsh"
    if not nsh.is_dir():
        return found
    needle = re.compile(rf"(?<![\d.]){re.escape(version)}(?![\d.])")
    for path in sorted(nsh.rglob("*.test.ts*")):
        if ".nsh" in path.parts or "node_modules" in path.parts:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
            if needle.search(line):
                found.append(f"{path.relative_to(root).as_posix()}:{number}")
    return found


def snapshot_files(root: Path) -> dict[Path, bytes | None]:
    snapshot = {}
    for relative in ROLLBACK_PATHS:
        path = root / relative
        snapshot[relative] = path.read_bytes() if path.exists() else None
    return snapshot


def restore_files(root: Path, snapshot: dict[Path, bytes | None]) -> None:
    for relative, content in snapshot.items():
        path = root / relative
        if content is None:
            if path.exists() or path.is_symlink():
                path.unlink()
        else:
            path.write_bytes(content)


def update_version(root: Path, version: str) -> None:
    snapshot = snapshot_files(root)
    try:
        set_version(root, version)
        refresh_lockfiles(root)
        refresh_generated_rules(root)
    except BaseException as error:
        try:
            restore_files(root, snapshot)
        except OSError as rollback_error:
            raise RuntimeError(f"{error}; rollback failed: {rollback_error}") from error
        raise


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: set_release_version.py MAJOR.MINOR.PATCH", file=sys.stderr)
        return 2
    root = Path(__file__).resolve().parents[1]
    try:
        previous = current_version(root)
        update_version(root, argv[1])
    except (OSError, RuntimeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if previous and previous != argv[1]:
        stale = tests_naming(root, previous)
        if stale:
            print(f"These tests still name {previous}; change them to {argv[1]} before you commit:", file=sys.stderr)
            for place in stale:
                print(f"  {place}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
