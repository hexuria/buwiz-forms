#!/usr/bin/env python3
"""Install this repository's git commit-message hooks, preserving any hook already present."""

from __future__ import annotations

import os
import stat
import subprocess
import sys
from pathlib import Path

MARKER = "buwiz-forms repository hook chain"
HOOKS = ("commit-msg", "pre-push")


class HookInstallError(RuntimeError):
    pass


class HooksPathConfigured(RuntimeError):
    pass


class GitDubiousOwnership(RuntimeError):
    pass


def chain_script(name: str) -> str:
    if name == "commit-msg":
        return f"""#!/bin/sh
# {MARKER}
hook_dir=$(CDPATH= cd "$(dirname "$0")" 2>/dev/null && pwd) || exit 1
local_hook="$hook_dir/{name}.local"
status=0
root=$(git rev-parse --show-toplevel 2>/dev/null)
if [ -n "$root" ] && [ -f "$root/.githooks/{name}" ]; then
    sh "$root/.githooks/{name}" "$@" || status=$?
fi
if [ "$status" -eq 0 ] && [ -x "$local_hook" ]; then
    "$local_hook" "$@" || status=$?
fi
exit "$status"
"""
    if name == "pre-push":
        return f"""#!/bin/sh
# {MARKER}
hook_dir=$(CDPATH= cd "$(dirname "$0")" 2>/dev/null && pwd) || exit 1
local_hook="$hook_dir/{name}.local"
input=$(mktemp "${{TMPDIR:-/tmp}}/buwiz-pre-push.XXXXXX") || exit 1
trap 'rm -f "$input"' EXIT
cat >"$input"
status=0
root=$(git rev-parse --show-toplevel 2>/dev/null)
if [ -n "$root" ] && [ -f "$root/.githooks/{name}" ]; then
    sh "$root/.githooks/{name}" "$@" <"$input" || status=$?
fi
if [ "$status" -eq 0 ] && [ -x "$local_hook" ]; then
    "$local_hook" "$@" <"$input" || status=$?
fi
exit "$status"
"""
    raise HookInstallError(f"unknown hook {name}")


def install_hook(root: Path, hooks_dir: Path, name: str) -> tuple[Path, Path | None]:
    source = root / ".githooks" / name
    if not source.is_file():
        raise HookInstallError(f"missing repository hook .githooks/{name}")
    hooks_dir.mkdir(parents=True, exist_ok=True)
    target = hooks_dir / name
    local = hooks_dir / f"{name}.local"
    moved = None
    if target.exists() or target.is_symlink():
        installed = ""
        if not target.is_symlink():
            try:
                installed = target.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError) as exc:
                raise HookInstallError(f"cannot read the existing {name} hook") from exc
        if MARKER not in installed:
            if local.exists():
                raise HookInstallError(f"both {name} and {name}.local already exist")
            os.replace(target, local)
            moved = local
    target.write_bytes(chain_script(name).encode("utf-8"))
    target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return target, moved


def raise_for_git_failure(result: subprocess.CompletedProcess, fallback: str) -> None:
    message = result.stderr.strip() or fallback
    if "dubious ownership" in message.lower():
        raise GitDubiousOwnership()
    raise HookInstallError(message)


def git_output(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise_for_git_failure(result, "git rev-parse failed")
    return result.stdout.strip()


def hooks_path_configured(root: Path) -> bool:
    result = subprocess.run(
        ["git", "config", "--get", "core.hooksPath"],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode == 0:
        return True
    if result.returncode == 1 and not result.stderr.strip():
        return False
    raise_for_git_failure(result, "git config could not read core.hooksPath")
    return False


def install_for_checkout(cwd: Path) -> list[tuple[Path, Path | None]]:
    root = Path(git_output(cwd, "rev-parse", "--show-toplevel")).resolve()
    if hooks_path_configured(root):
        raise HooksPathConfigured()
    raw_hooks = git_output(root, "rev-parse", "--git-path", "hooks")
    hooks_dir = Path(raw_hooks)
    if not hooks_dir.is_absolute():
        hooks_dir = root / hooks_dir
    return [install_hook(root, hooks_dir, name) for name in HOOKS]


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) > 1:
        print("usage: install_git_hooks.py [REPOSITORY_ROOT]", file=sys.stderr)
        return 2
    checkout = Path(args[0]).expanduser() if args else Path.cwd()
    try:
        installed = install_for_checkout(checkout)
    except HooksPathConfigured:
        print(
            "core.hooksPath is set; not installing hooks: "
            "chain 'sh .githooks/commit-msg \"$@\"' and 'sh .githooks/pre-push \"$@\"' by hand"
        )
        return 0
    except GitDubiousOwnership:
        print("git reports dubious ownership; not installing hooks: mark the checkout safe or add the hooks by hand")
        return 0
    except HookInstallError as error:
        print(f"could not install the git hooks: {error}", file=sys.stderr)
        return 1
    for target, moved in installed:
        if moved is not None:
            print(f"moved existing hook aside: {moved}")
        print(f"chained git hook: {target}")
    print("git commit-message hooks installed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
