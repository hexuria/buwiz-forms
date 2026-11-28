#!/usr/bin/env python3
"""Refuse absolute local paths in commit messages."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from typing import Callable, Iterable

REASON = "commit message carries an absolute local path; use $HOME, ~, or /Users/<name> instead"
ZERO_SHA = "0" * 40

POSIX_HOME_PATTERN = re.compile(r"(?P<root>/Users/|/home/)(?P<name>[A-Za-z0-9._-]+)(?![A-Za-z0-9._-])")
WINDOWS_HOME_PATTERN = re.compile(r"\bC:\\Users\\(?P<name>[A-Za-z0-9._-]+)(?![A-Za-z0-9._-])", re.IGNORECASE)
VOLUME_PATTERN = re.compile(r"/Volumes/")
ALLOWED_POSIX_HOME_NAMES = {
    "/Users/": {"runner", "devin", "you"},
    "/home/": {"runner", "ubuntu"},
}
ALLOWED_WINDOWS_HOME_NAMES = {"runneradmin"}

Runner = Callable[..., subprocess.CompletedProcess]


def has_absolute_local_path(message: str) -> bool:
    if VOLUME_PATTERN.search(message):
        return True
    for match in POSIX_HOME_PATTERN.finditer(message):
        if match.group("name") not in ALLOWED_POSIX_HOME_NAMES[match.group("root")]:
            return True
    for match in WINDOWS_HOME_PATTERN.finditer(message):
        if match.group("name").lower() not in ALLOWED_WINDOWS_HOME_NAMES:
            return True
    return False


def parse_log_output(output: bytes) -> list[tuple[str, str]]:
    parts = output.split(b"\0")
    commits = []
    for index in range(0, len(parts) - 1, 2):
        sha = parts[index].strip().decode("ascii", errors="replace")
        if not re.fullmatch(r"[0-9a-fA-F]{40}", sha):
            continue
        message = parts[index + 1].decode("utf-8", errors="replace")
        commits.append((sha, message))
    return commits


def git_commit_messages(args: list[str], cwd: Path | None, runner: Runner) -> list[tuple[str, str]]:
    result = runner(
        ["git", *args],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        stderr = result.stderr.decode("utf-8", errors="replace") if isinstance(result.stderr, bytes) else str(result.stderr)
        raise RuntimeError(stderr.strip() or "git could not read the commit messages")
    return parse_log_output(result.stdout)


def log_messages(span: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> list[tuple[str, str]]:
    return git_commit_messages(["log", "--format=%H%x00%B%x00", span], cwd, runner)


def commit_message(commit: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> list[tuple[str, str]]:
    return git_commit_messages(["show", "-s", "--format=%H%x00%B%x00", commit], cwd, runner)


def scan_messages(commits: Iterable[tuple[str, str]]) -> list[str]:
    return [sha for sha, message in commits if has_absolute_local_path(message)]


def scan_range(base: str, head: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> list[str]:
    return scan_messages(log_messages(f"{base}..{head}", cwd, runner))


def merge_base(head: str, cwd: Path | None, runner: Runner = subprocess.run) -> str | None:
    for candidate in ("origin/main", "main"):
        result = runner(
            ["git", "merge-base", candidate, head],
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if result.returncode == 0:
            base = result.stdout.strip()
            if base:
                return base.decode() if isinstance(base, bytes) else str(base)
    return None


def scan_pre_push(lines: Iterable[str], cwd: Path | None = None, runner: Runner = subprocess.run) -> list[str]:
    bad: list[str] = []
    seen: set[str] = set()
    for line in lines:
        parts = line.split()
        if len(parts) < 4:
            continue
        _local_ref, local_sha, _remote_ref, remote_sha = parts[:4]
        if not re.fullmatch(r"[0-9a-fA-F]{40}", local_sha) or local_sha == ZERO_SHA:
            continue
        if re.fullmatch(r"[0-9a-fA-F]{40}", remote_sha) and remote_sha != ZERO_SHA:
            key = f"range:{remote_sha}..{local_sha}"
            if key in seen:
                continue
            seen.add(key)
            bad.extend(scan_range(remote_sha, local_sha, cwd, runner))
            continue
        base = merge_base(local_sha, cwd, runner)
        if base:
            key = f"range:{base}..{local_sha}"
            if key in seen:
                continue
            seen.add(key)
            bad.extend(scan_range(base, local_sha, cwd, runner))
        elif local_sha not in seen:
            seen.add(local_sha)
            bad.extend(scan_messages(commit_message(local_sha, cwd, runner)))
    return bad


def report_bad_commits(bad: Iterable[str]) -> None:
    for sha in bad:
        print(f"commit {sha}: commit message carries an absolute local path", file=sys.stderr)


def committed_message_text(message: str) -> str:
    lines = []
    scissors = "# ------------------------ >8 ------------------------"
    for line in message.splitlines(keepends=True):
        if line.startswith(scissors):
            break
        if line.startswith("#"):
            continue
        lines.append(line)
    return "".join(lines)


def read_message_file(name: str) -> str:
    return committed_message_text(Path(name).read_bytes().decode("utf-8", errors="replace"))


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) == 2 and args[0] == "message-file":
        if has_absolute_local_path(read_message_file(args[1])):
            print(REASON, file=sys.stderr)
            return 1
        return 0
    if len(args) == 3 and args[0] == "range":
        try:
            bad = scan_range(args[1], args[2])
        except RuntimeError as error:
            print(f"could not scan pull-request commit messages: {error}", file=sys.stderr)
            return 2
        if bad:
            report_bad_commits(bad)
            return 1
        print("no absolute local paths in pull-request commit messages")
        return 0
    if len(args) == 1 and args[0] == "pre-push":
        try:
            bad = scan_pre_push(sys.stdin)
        except RuntimeError as error:
            print(f"could not scan pushed commit messages: {error}", file=sys.stderr)
            return 2
        if bad:
            report_bad_commits(bad)
            return 1
        return 0
    print("usage: commit_message_paths.py message-file PATH | range BASE HEAD | pre-push", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
