#!/usr/bin/env python3
"""Refuse absolute local paths in commit messages."""

from __future__ import annotations

import re
import string
import subprocess
import sys
from pathlib import Path
from typing import Callable, Iterable

REASON = "commit message carries an absolute local path"
REWORDING = (
    "Reword the path as $HOME, ~, or /Users/<name>; --no-verify skips the local hook, "
    "while the pull-request check still applies."
)
ZERO_SHA = "0" * 40

# A path starts at a boundary: not in the middle of a relative path (src/home/…). A local file URL is a boundary
# too, but only when its case-insensitive "file" starts a word: xfile:// and profile:// are not file URLs.
POSIX_HOME_PATTERN = re.compile(r"(?P<root>/Users/|/home/|/var/home/)(?P<name>[A-Za-z0-9._-]+)(?![A-Za-z0-9._-])")
WINDOWS_MOUNT_HOME_START_PATTERN = re.compile(r"/c/Users/|/mnt/c/Users/|/cygdrive/c/Users/", re.IGNORECASE)
WINDOWS_HOME_START_PATTERN = re.compile(r"(?:\\\\\?\\)?c:\\Users\\", re.IGNORECASE)
WINDOWS_SPACED_NAME_PATTERN = re.compile(r"(?P<name>[A-Za-z0-9._-]+(?: [A-Za-z0-9._-]+)+)(?=\\|[\"'])")
WINDOWS_NAME_TOKEN_PATTERN = re.compile(r"[A-Za-z0-9._~-]+")
VOLUME_PATTERN = re.compile(r"/Volumes/(?!…)")
ALLOWED_POSIX_HOME_NAMES = {
    "/Users/": {"runner", "devin", "you"},
    "/home/": {"runner", "ubuntu"},
    "/var/home/": {"runner", "ubuntu"},
}
ALLOWED_WINDOWS_HOME_NAMES = {"runneradmin"}
PATH_BODY_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._~/")
WINDOWS_PATH_BODY_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_\\/")
ASCII_ALNUM = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789")

Runner = Callable[..., subprocess.CompletedProcess]


def file_url_boundary(message: str, index: int) -> bool:
    prefix_text = message[:index].lower()
    for prefix in ("file://localhost", "file://"):
        if not prefix_text.endswith(prefix):
            continue
        start = index - len(prefix)
        return start == 0 or message[start - 1] not in ASCII_ALNUM
    return False


def posix_path_boundary(message: str, index: int) -> bool:
    if index == 0 or message[index - 1] not in PATH_BODY_CHARS:
        return True
    return file_url_boundary(message, index)


def windows_path_boundary(message: str, index: int) -> bool:
    return index == 0 or message[index - 1] not in WINDOWS_PATH_BODY_CHARS


def home_name_for_compare(raw: str, message: str, end: int) -> str:
    if end < len(message) and message[end] in "/\\":
        return raw
    return raw.rstrip(".")


def windows_home_name(message: str, start: int) -> tuple[str, int]:
    rest = message[start:]
    spaced = WINDOWS_SPACED_NAME_PATTERN.match(rest)
    if spaced:
        return spaced.group("name"), start + spaced.end("name")
    token = WINDOWS_NAME_TOKEN_PATTERN.match(rest)
    if token:
        return token.group(0), start + token.end()
    return "", start


def allowed_windows_home_name(name: str) -> bool:
    lowered = name.lower()
    if lowered in ALLOWED_WINDOWS_HOME_NAMES:
        return True
    for allowed in ALLOWED_WINDOWS_HOME_NAMES:
        if len(allowed) > 6 and re.fullmatch(re.escape(allowed[:6]) + r"~[0-9]+", lowered):
            return True
    return False


def windows_home_is_disallowed(message: str, start: int) -> bool:
    raw_name, end = windows_home_name(message, start)
    if not raw_name:
        return False
    name = home_name_for_compare(raw_name, message, end)
    return not allowed_windows_home_name(name)


def has_absolute_local_path(message: str) -> bool:
    for match in VOLUME_PATTERN.finditer(message):
        if posix_path_boundary(message, match.start()):
            return True
    for match in POSIX_HOME_PATTERN.finditer(message):
        if not posix_path_boundary(message, match.start()):
            continue
        name = home_name_for_compare(match.group("name"), message, match.end("name"))
        if name not in ALLOWED_POSIX_HOME_NAMES[match.group("root")]:
            return True
    for pattern in (WINDOWS_MOUNT_HOME_START_PATTERN, WINDOWS_HOME_START_PATTERN):
        for match in pattern.finditer(message):
            boundary = posix_path_boundary if pattern is WINDOWS_MOUNT_HOME_START_PATTERN else windows_path_boundary
            if boundary(message, match.start()) and windows_home_is_disallowed(message, match.end()):
                return True
    return False


def first_offending_line(message: str) -> str | None:
    for line in message.splitlines():
        if has_absolute_local_path(line):
            return line
    return None


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


def log_messages_not_in(head: str, exclusions: Iterable[str], cwd: Path | None, runner: Runner) -> list[tuple[str, str]]:
    revisions = [head, *(f"^{exclusion}" for exclusion in exclusions)]
    return git_commit_messages(["log", "--no-show-signature", "--format=%H%x00%B%x00", *revisions], cwd, runner)


def commit_message(commit: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> list[tuple[str, str]]:
    return git_commit_messages(["show", "-s", "--no-show-signature", "--format=%H%x00%B%x00", commit], cwd, runner)


def scan_messages(commits: Iterable[tuple[str, str]]) -> list[tuple[str, str]]:
    bad = []
    for sha, message in commits:
        offending = first_offending_line(message)
        if offending is not None:
            bad.append((sha, offending))
    return bad


def scan_range(base: str, head: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> list[tuple[str, str]]:
    exclusions = [base, *published_main_refs(cwd, runner)]
    return scan_messages(log_messages_not_in(head, exclusions, cwd, runner))


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


def published_main_refs(cwd: Path | None, runner: Runner) -> list[str]:
    result = runner(
        ["git", "rev-parse", "--verify", "--quiet", "origin/main"],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return ["origin/main"] if result.returncode == 0 else []


def object_exists(sha: str, cwd: Path | None, runner: Runner = subprocess.run) -> bool:
    result = runner(
        ["git", "cat-file", "-e", sha],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result.returncode == 0


def scan_pre_push(lines: Iterable[str], cwd: Path | None = None, runner: Runner = subprocess.run) -> list[tuple[str, str]]:
    bad: list[tuple[str, str]] = []
    seen = set()
    published = published_main_refs(cwd, runner)
    for line in lines:
        parts = line.split()
        if len(parts) < 4:
            continue
        _local_ref, local_sha, _remote_ref, remote_sha = parts[:4]
        if not re.fullmatch(r"[0-9a-fA-F]{40}", local_sha) or local_sha == ZERO_SHA:
            continue
        exclusions = list(published)
        if re.fullmatch(r"[0-9a-fA-F]{40}", remote_sha) and remote_sha != ZERO_SHA and object_exists(remote_sha, cwd, runner):
            exclusions.insert(0, remote_sha)
        else:
            base = merge_base(local_sha, cwd, runner)
            if base:
                exclusions.insert(0, base)
        if exclusions:
            key = (local_sha, tuple(exclusions))
            if key in seen:
                continue
            seen.add(key)
            bad.extend(scan_messages(log_messages_not_in(local_sha, exclusions, cwd, runner)))
        elif local_sha not in seen:
            seen.add(local_sha)
            bad.extend(scan_messages(commit_message(local_sha, cwd, runner)))
    return bad


def report_bad_commits(bad: Iterable[tuple[str, str]]) -> None:
    entries = list(bad)
    for sha, offending in entries:
        print(f"commit {sha}: {REASON}", file=sys.stderr)
        print(f"offending line: {offending}", file=sys.stderr)
    if entries:
        print(REWORDING, file=sys.stderr)


SCISSORS_BODY = "-" * 24 + " >8 " + "-" * 24


def process_output_text(output: bytes | str) -> str:
    if isinstance(output, bytes):
        return output.decode("utf-8", errors="replace")
    return str(output)


def inside_git_repository(cwd: Path | None, runner: Runner = subprocess.run) -> bool:
    result = runner(
        ["git", "rev-parse", "--git-dir"],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    return result.returncode == 0


def comment_characters(cwd: Path | None, runner: Runner = subprocess.run) -> set[str]:
    if not inside_git_repository(cwd, runner):
        return set(string.punctuation)
    result = runner(
        ["git", "config", "--get", "core.commentChar"],
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode == 0:
        configured = process_output_text(result.stdout).strip()
        if len(configured) == 1:
            return {configured}
        if configured.lower() == "auto":
            return set(string.punctuation)
    return {"#"}


def is_git_scissors_line(line: str, characters: set[str]) -> bool:
    text = line.rstrip("\r\n")
    if len(text) != len(SCISSORS_BODY) + 2:
        return False
    return text[0] in characters and text[1] == " " and text[2:] == SCISSORS_BODY


def committed_message_text(message: str, cwd: Path | None = None, runner: Runner = subprocess.run) -> str:
    lines = []
    characters = comment_characters(Path.cwd() if cwd is None else cwd, runner)
    for line in message.splitlines(keepends=True):
        if is_git_scissors_line(line, characters):
            break
        lines.append(line)
    return "".join(lines)


def read_message_file(name: str) -> str:
    return committed_message_text(Path(name).read_bytes().decode("utf-8", errors="replace"))


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) == 2 and args[0] == "message-file":
        offending = first_offending_line(read_message_file(args[1]))
        if offending is not None:
            print(REASON, file=sys.stderr)
            print(f"offending line: {offending}", file=sys.stderr)
            print(REWORDING, file=sys.stderr)
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
