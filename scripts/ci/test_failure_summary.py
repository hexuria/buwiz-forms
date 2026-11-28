#!/usr/bin/env python3
"""Append the failing test names in CI's captured Python and Rust test logs to the job summary."""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]")
PYTHON_FAILURE = re.compile(r"^(?:FAIL|ERROR): (.+?)\s*$")
RUST_FAILURE = re.compile(r"^test (.+?)\s+\.\.\.\s+FAILED\s*$")


def without_color(text: str) -> str:
    return ANSI.sub("", text)


def unique(names: list[str]) -> list[str]:
    return list(dict.fromkeys(names))


def python_failures(text: str) -> list[str]:
    """The qualified unittest names from the failure/error summary, without duplicates."""
    return unique(match.group(1) for line in without_color(text).splitlines()
                  if (match := PYTHON_FAILURE.match(line)))


def rust_failures(text: str) -> list[str]:
    """The test names libtest prints with FAILED, without duplicates."""
    return unique(match.group(1) for line in without_color(text).splitlines()
                  if (match := RUST_FAILURE.match(line)))


def section(title: str, path: Path, failures) -> list[str]:
    lines = [f"### {title}", ""]
    if not path.exists():
        lines.append(f"- log missing: `{path}`")
    else:
        names = failures(path.read_text(encoding="utf-8", errors="replace"))
        if names:
            lines.extend(f"- `{name}`" for name in names)
        else:
            lines.append("- none found in the captured output")
    return lines + [""]


def markdown(python_log: Path, rust_log: Path) -> str:
    lines = ["## Failing test names", ""]
    lines += section("Python script tests", python_log, python_failures)
    lines += section("Rust workspace tests", rust_log, rust_failures)
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(f"usage: {argv[0]} PYTHON_TEST_LOG RUST_TEST_LOG", file=sys.stderr)
        return 2
    text = markdown(Path(argv[1]), Path(argv[2]))
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as out:
            out.write(text)
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
