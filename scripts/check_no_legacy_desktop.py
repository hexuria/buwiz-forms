#!/usr/bin/env python3
"""Reject new references to the retired painted crate."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import Iterable

REFERENCE = re.compile(r"bir[-_]desktop", re.IGNORECASE)
SELF_REFERENCING_FILES = {
    "scripts/check_no_legacy_desktop.py",
    "scripts/tests/test_check_no_legacy_desktop.py",
}
NEWS_FETCHER_DATA_DIR = "crates/bir-core/src/news_fetcher.rs"


def find_references(paths: Iterable[Path], root: Path) -> list[tuple[str, int, str]]:
    findings = []
    for path in paths:
        relative = path.relative_to(root).as_posix()
        if relative in SELF_REFERENCING_FILES:
            continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        for line_number, line in enumerate(lines, 1):
            if REFERENCE.search(line) and not (
                relative == NEWS_FETCHER_DATA_DIR
                and 'ProjectDirs::from("com", "Goldcoders", "bir-desktop")' in line
            ):
                findings.append((relative, line_number, line.strip()))
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
        for path, line, text in findings:
            print(f"{path}:{line}: {text}", file=sys.stderr)
        return 1
    print("no retired-crate references found")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
