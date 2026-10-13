"""Strictly validate the subset of Inno [Files] syntax used by this project."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Iterable


_SECTION_RE = re.compile(r"^\s*\[([^\]]+)\]\s*(?:;.*)?$")
_NAME_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")
_ALLOWED = {"source", "destdir", "flags"}
_STAGE_PREFIX = "{#StageDir}\\"


class InstallerSourceError(ValueError):
    """Raised when installer sources are outside the supported syntax."""


def _split_parameters(line: str, line_number: int) -> list[tuple[str, str, bool]]:
    pieces: list[str] = []
    start = 0
    quoted = False
    index = 0
    while index < len(line):
        char = line[index]
        if char == '"':
            if quoted and index + 1 < len(line) and line[index + 1] == '"':
                index += 2
                continue
            quoted = not quoted
        elif char == ";" and not quoted:
            pieces.append(line[start:index])
            start = index + 1
        index += 1
    if quoted:
        raise InstallerSourceError(f"line {line_number}: unterminated quote")
    pieces.append(line[start:])

    result: list[tuple[str, str, bool]] = []
    for piece in pieces:
        piece = piece.strip()
        if not piece:
            raise InstallerSourceError(f"line {line_number}: empty parameter")
        colon = piece.find(":")
        if colon <= 0:
            raise InstallerSourceError(f"line {line_number}: expected name:value")
        name = piece[:colon].strip()
        if _NAME_RE.fullmatch(name) is None:
            raise InstallerSourceError(f"line {line_number}: invalid directive name")
        raw = piece[colon + 1 :].strip()
        if not raw:
            raise InstallerSourceError(f"line {line_number}: empty value for {name}")
        if raw.startswith('"'):
            if not raw.endswith('"') or len(raw) == 1:
                raise InstallerSourceError(
                    f"line {line_number}: malformed quoted value"
                )
            value = raw[1:-1].replace('""', '"')
            quoted_value = True
        else:
            if '"' in raw:
                raise InstallerSourceError(f"line {line_number}: malformed value")
            value = raw
            quoted_value = False
        result.append((name.casefold(), value, quoted_value))
    return result


def _validate_source(source: str, quoted: bool, stage: Path, line_number: int) -> None:
    if not quoted:
        raise InstallerSourceError(f"line {line_number}: Source must be quoted")
    if not source.startswith(_STAGE_PREFIX):
        raise InstallerSourceError(
            f"line {line_number}: Source must start with {_STAGE_PREFIX!r}"
        )
    relative = source[len(_STAGE_PREFIX) :]
    if not relative or ":" in relative or "{" in relative or "}" in relative:
        raise InstallerSourceError(f"line {line_number}: unsupported Source path")
    normalized = relative.replace("/", "\\")
    segments = normalized.split("\\")
    if any(segment in {".", ".."} for segment in segments):
        raise InstallerSourceError(f"line {line_number}: traversal is not allowed")
    if normalized.startswith(("\\", "/")):
        raise InstallerSourceError(f"line {line_number}: absolute Source path")
    wildcard_positions = [i for i, segment in enumerate(segments) if "*" in segment]
    if wildcard_positions and wildcard_positions != [len(segments) - 1]:
        raise InstallerSourceError(f"line {line_number}: wildcard must be final")
    if "?" in relative or "[" in relative or "]" in relative:
        raise InstallerSourceError(f"line {line_number}: unsupported wildcard")
    candidate = stage.joinpath(*segments)
    try:
        stage_resolved = stage.resolve(strict=True)
        parent = candidate.parent.resolve(strict=True)
    except FileNotFoundError as exc:
        raise InstallerSourceError(
            f"line {line_number}: Source does not exist"
        ) from exc
    try:
        parent.relative_to(stage_resolved)
    except ValueError as exc:
        raise InstallerSourceError(f"line {line_number}: Source escapes stage") from exc
    if wildcard_positions:
        matches = list(candidate.parent.glob(candidate.name))
        if not matches:
            raise InstallerSourceError(
                f"line {line_number}: wildcard Source matches nothing"
            )
        for match in matches:
            try:
                match.resolve(strict=True).relative_to(stage_resolved)
            except (FileNotFoundError, ValueError) as exc:
                raise InstallerSourceError(
                    f"line {line_number}: Source escapes stage"
                ) from exc
    else:
        try:
            candidate.resolve(strict=True).relative_to(stage_resolved)
        except (FileNotFoundError, ValueError) as exc:
            raise InstallerSourceError(
                f"line {line_number}: Source escapes stage"
            ) from exc


def validate_installer_sources(installer: Path, stage_dir: Path) -> int:
    text = installer.read_text(encoding="utf-8")
    if re.search(r"#include\b", text, re.IGNORECASE):
        raise InstallerSourceError("#include is not supported")
    stage = stage_dir.resolve(strict=True)
    in_files = False
    files_seen = False
    sources = 0
    for line_number, raw_line in enumerate(text.splitlines(), 1):
        stripped = raw_line.strip()
        section = _SECTION_RE.match(raw_line)
        if section:
            in_files = section.group(1).casefold() == "files"
            files_seen = files_seen or in_files
            continue
        if not in_files or not stripped or stripped.startswith(";"):
            continue
        parameters = _split_parameters(raw_line, line_number)
        names = [name for name, _, _ in parameters]
        unknown = set(names) - _ALLOWED
        if unknown:
            raise InstallerSourceError(
                f"line {line_number}: unsupported [Files] directive "
                f"{sorted(unknown)[0]}"
            )
        if names.count("source") != 1:
            raise InstallerSourceError(
                f"line {line_number}: exactly one Source is required"
            )
        source, quoted = next(
            (value, is_quoted)
            for name, value, is_quoted in parameters
            if name == "source"
        )
        _validate_source(source, quoted, stage, line_number)
        sources += 1
    if not files_seen:
        raise InstallerSourceError("installer has no [Files] section")
    if not sources:
        raise InstallerSourceError("installer has no valid Source entries")
    return sources


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--installer", type=Path, required=True)
    parser.add_argument("--stage-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        count = validate_installer_sources(args.installer, args.stage_dir)
    except (OSError, InstallerSourceError) as exc:
        print(f"installer source validation failed: {exc}", file=sys.stderr)
        return 1
    print(f"validated {count} installer source entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
