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


def install_hook(root: Path, hooks_dir: Path, name: str) -> Path:
    source = root / ".githooks" / name
    if not source.is_file():
        raise HookInstallError(f"missing repository hook .githooks/{name}")
    hooks_dir.mkdir(parents=True, exist_ok=True)
    target = hooks_dir / name
    local = hooks_dir / f"{name}.local"
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
    target.write_text(chain_script(name), encoding="utf-8")
    target.chmod(target.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return target


def git_output(root: Path, *args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise HookInstallError(result.stderr.strip() or "git rev-parse failed")
    return result.stdout.strip()


def install_for_checkout(cwd: Path) -> list[Path]:
    root = Path(git_output(cwd, "rev-parse", "--show-toplevel")).resolve()
    raw_hooks = git_output(root, "rev-parse", "--git-path", "hooks")
    hooks_dir = Path(raw_hooks)
    if not hooks_dir.is_absolute():
        hooks_dir = root / hooks_dir
    return [install_hook(root, hooks_dir, name) for name in HOOKS]


def main() -> int:
    try:
        install_for_checkout(Path.cwd())
    except HookInstallError as error:
        print(f"could not install the git hooks: {error}", file=sys.stderr)
        return 1
    print("git commit-message hooks installed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
