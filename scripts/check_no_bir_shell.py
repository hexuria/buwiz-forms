#!/usr/bin/env python3
"""Reject the retired JS-shell crate coming back.

Three checks over ``git ls-files``:

1. No tracked path may live under ``crates/bir-shell/`` or the retired
   top-level ``app/`` tree.
2. No build-context file — Cargo manifests and lockfiles, the justfile,
   CI workflows, shell and PowerShell scripts, installer sources — may
   name ``bir-shell``/``bir_shell``. The name survives only where it is
   not a build reference: Rust sources and docs keep it as the app's
   published agent id (``agent::APP``, the bir-mcp discovery strings),
   and comments/scripts carry it as provenance.
3. No tracked file may spell out the two retired paths, ``crates/bir-shell``
   or the old screens folder under ``app/``. The exceptions are this guard
   and the tests that feed it those paths.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Iterable

RETIRED_PREFIXES = (PurePosixPath("crates/bir-shell"), PurePosixPath("app"))

BUILD_CONTEXT_SUFFIXES = {".toml", ".lock", ".yml", ".yaml", ".sh", ".ps1", ".iss", ".wxs"}
BUILD_CONTEXT_NAMES = {"justfile"}

REFERENCE = re.compile(r"bir[-_]shell", re.IGNORECASE)

RETIRED_PATH_MENTION = re.compile(r"crates/bir-shell|(?<![\w-])app/shell")
RETIRED_PATH_MENTION_ALLOWED = {
    "scripts/check_no_bir_shell.py",
    "scripts/tests/test_check_no_bir_shell.py",
    "scripts/tests/test_ci_change_detection.py",
    "scripts/tests/test_release_workflow_policy.py",
}


def find_references(paths: Iterable[Path], root: Path) -> list[str]:
    findings = []
    for path in paths:
        relative = path.relative_to(root).as_posix()
        if any(PurePosixPath(relative).is_relative_to(prefix) for prefix in RETIRED_PREFIXES):
            findings.append(f"{relative}: retired path is tracked again")
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        if relative not in RETIRED_PATH_MENTION_ALLOWED:
            for line_number, line in enumerate(lines, 1):
                if RETIRED_PATH_MENTION.search(line):
                    findings.append(f"{relative}:{line_number}: names a retired path: {line.strip()[:160]}")
        if PurePosixPath(relative).suffix not in BUILD_CONTEXT_SUFFIXES and (
            PurePosixPath(relative).name not in BUILD_CONTEXT_NAMES
        ):
            continue
        for line_number, line in enumerate(lines, 1):
            # The guard's own name contains the retired name: every file
            # may invoke it without tripping the check.
            if REFERENCE.search(line.replace("check_no_bir_shell", "")):
                findings.append(f"{relative}:{line_number}: {line.strip()}")
    return findings


def tracked_paths(root: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    )
    return [root / item.decode() for item in result.stdout.split(b"\0") if item]


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    findings = find_references(tracked_paths(root), root)
    if findings:
        for finding in findings:
            print(finding, file=sys.stderr)
        return 1
    print("no retired shell references found")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
