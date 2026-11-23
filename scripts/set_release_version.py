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
    Path("crates/bir-shell/Cargo.toml"),
)
LOCKFILE_PATHS = (
    Path("Cargo.lock"),
    Path("nsh/Cargo.lock"),
    Path("crates/bir-shell/Cargo.lock"),
)
ROLLBACK_PATHS = (
    Path("Cargo.toml"),
    Path("crates/bir-core/Cargo.toml"),
    Path("crates/bir-print/Cargo.toml"),
    Path("nsh/Cargo.toml"),
    Path("nsh/nsh.json"),
    *LOCKFILE_PATHS,
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
        update_version(root, argv[1])
    except (OSError, RuntimeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
