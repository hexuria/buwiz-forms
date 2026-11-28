#!/usr/bin/env python3
"""Refuse absolute user-home paths in tracked files."""

from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

# Angle-bracketed or interpolated names such as <name> are placeholders, not
# homes. Concrete account names, including CI account names, are never allowed.
PATH_BODY_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._~/}")
WINDOWS_PATH_BODY_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_\\/")
ASCII_ALNUM = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789")

POSIX_HOME_PATTERN = re.compile(r"(?P<root>/Users/|/home/|/var/home/)(?P<name>[A-Za-z0-9._-]+)(?![A-Za-z0-9._-])")
WINDOWS_MOUNT_HOME_PATTERN = re.compile(r"(?:/c/Users/|/mnt/c/Users/|/cygdrive/c/Users/)(?P<name>[A-Za-z0-9._-]+)(?![A-Za-z0-9._-])", re.IGNORECASE)
WINDOWS_HOME_PATTERN = re.compile(r"(?:\\\\\?\\)?c:\\Users\\(?P<name>[A-Za-z0-9._ -]+)(?=\\|$)", re.IGNORECASE)
WINDOWS_FORWARD_HOME_PATTERN = re.compile(r"c:/Users/(?P<name>[A-Za-z0-9._ -]+)(?=/|$)", re.IGNORECASE)


@dataclass(frozen=True)
class HomePathHit:
    path: str
    kind: str


def posix_path_boundary(text: str, index: int) -> bool:
    if index == 0 or text[index - 1] not in PATH_BODY_CHARS:
        return True
    prefix_text = text[:index].lower()
    for prefix in ("file://localhost", "file://"):
        if prefix_text.endswith(prefix):
            start = index - len(prefix)
            return start == 0 or text[start - 1] not in ASCII_ALNUM
    return False


def windows_path_boundary(text: str, index: int) -> bool:
    return index == 0 or text[index - 1] not in WINDOWS_PATH_BODY_CHARS


def placeholder_name(name: str) -> bool:
    stripped = name.strip().rstrip('.,;:"\')')
    return (stripped.startswith("<") and stripped.endswith(">")) or (stripped.startswith("{") and stripped.endswith("}")) or stripped in {"", ".", "..", "_", "x"}


def disallowed_home_names(text: str) -> set[str]:
    """Return hit kinds, never the path or account name."""
    kinds: set[str] = set()
    # JSON files store a Windows backslash doubled. Scan that spelling as one
    # separator too; the scanner never reports the text it matched.
    variants = (("path", text.replace("\\\\", "\\")),)
    for spelling, scan_text in variants:
        for match in POSIX_HOME_PATTERN.finditer(scan_text):
            name = match.group("name")
            if posix_path_boundary(scan_text, match.start()) and not placeholder_name(name):
                kinds.add("posix-home")
        for match in WINDOWS_MOUNT_HOME_PATTERN.finditer(scan_text):
            name = match.group("name")
            if posix_path_boundary(scan_text, match.start()) and not placeholder_name(name):
                kinds.add("windows-mount-home")
        for match in WINDOWS_HOME_PATTERN.finditer(scan_text):
            name = match.group("name")
            if windows_path_boundary(scan_text, match.start()) and not placeholder_name(name):
                kinds.add("windows-home")
        for match in WINDOWS_FORWARD_HOME_PATTERN.finditer(scan_text):
            name = match.group("name")
            if windows_path_boundary(scan_text, match.start()) and not placeholder_name(name):
                kinds.add("windows-forward-home")
    return kinds


def scan_text(text: str) -> bool:
    return bool(disallowed_home_names(text))


def tracked_files(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files"],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return result.stdout.decode("utf-8", errors="replace").splitlines()


def scan_tracked_files(root: Path) -> list[HomePathHit]:
    hits: list[HomePathHit] = []
    for name in tracked_files(root):
        path = root / name
        try:
            data = path.read_bytes()
        except OSError:
            continue
        if b"\0" in data:
            continue
        kinds = disallowed_home_names(data.decode("utf-8", errors="replace"))
        for kind in sorted(kinds):
            hits.append(HomePathHit(name, kind))
    return hits


def scan_paths(paths: list[Path]) -> list[HomePathHit]:
    hits: list[HomePathHit] = []
    for path in paths:
        data = path.read_bytes()
        kinds = disallowed_home_names(data.decode("utf-8", errors="replace"))
        for kind in sorted(kinds):
            hits.append(HomePathHit(str(path), kind))
    return hits


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    root = Path.cwd()
    if args[:1] == ["--root"]:
        root = Path(args[1])
        args = args[2:]
    if args:
        hits = scan_paths([Path(arg) for arg in args])
    else:
        hits = scan_tracked_files(root)
    if hits:
        print("tracked files carry absolute user-home paths:", file=sys.stderr)
        for hit in hits:
            print(f"{hit.path}: {hit.kind}", file=sys.stderr)
        print("Use %USERPROFILE%, $HOME, a relative evidence path, or an angle-bracketed placeholder.", file=sys.stderr)
        return 1
    print("no absolute user-home paths in tracked files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
