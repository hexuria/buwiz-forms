#!/usr/bin/env python3
"""The checks `just play-bundle` makes before it builds a store bundle.

    play_bundle_guard.py dotenv ROOT [--allow-dotenv]
        Refuses when a `.env` is in ROOT or a folder above it, unless --allow-dotenv. `just` loads that file into every
        recipe (`set dotenv-load`), and the app bakes some variables into the binary at compile time
        (`option_env!("BUWIZ_SERVER_URL")`, `BUWIZ_ACCOUNT_URL`, the Google client), so a store build must not take
        whatever a developer's .env holds by accident. App Store Connect's next build number (`just play-bundle` with
        no number) needs that file, hence the flag.

    play_bundle_guard.py wrapper WRAPPER APP_NSH TOOLS HOME
        On Linux the nsh CLI runs in the Ubuntu 24.04 image through ~/BuwizNsh/nsh2404.sh, which scripts/setup-linux-nsh.sh
        writes for one checkout: it changes to the `app_dir` it names and mounts only $HOME, that checkout and the NSH
        tools. Refuses a wrapper written for another checkout, or NSH tools the container cannot see.

    ENV-DUMP | play_bundle_guard.py container-env
        Reads `env` as printed inside that container and refuses when the release profile the recipe set
        (CARGO_PROFILE_RELEASE_LTO, _CODEGEN_UNITS, _OPT_LEVEL, read from this process's environment) did not reach it,
        or when it carries what would make the store binary a development one (BUWIZ_ANDROID_DEV) or bake a server
        address into it (BUWIZ_SERVER_URL, BUWIZ_ACCOUNT_URL). Without the profile Cargo builds its default release
        silently: a 123 MB arm64 library instead of 88 MB, and no error.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import sys
from pathlib import Path

PROFILE = ("CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "CARGO_PROFILE_RELEASE_OPT_LEVEL")
# Set in the container, each would end up in the store binary.
NEVER_IN_RELEASE = ("BUWIZ_ANDROID_DEV", "BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL")


def dotenv_files(root: Path) -> list[Path]:
    """Every `.env` file in `root` or a folder above it."""
    root = root.resolve()
    return [folder / ".env" for folder in (root, *root.parents) if (folder / ".env").is_file()]


def dotenv_refusal(root: Path, allow: bool) -> str | None:
    found = dotenv_files(root)
    if not found or allow:
        return None
    return (f"{found[0]} would be loaded into the build (`set dotenv-load`), and the app bakes some of its variables into "
            "the binary: move it away, or pass --allow-dotenv when it is the one you mean (App Store Connect's next "
            "build number needs it)")


def wrapper_refusal(wrapper: Path, app_nsh: Path, tools: Path, home: Path) -> str | None:
    try:
        text = wrapper.read_text(encoding="utf-8")
    except OSError as error:
        return f"cannot read {wrapper} ({error}): run scripts/setup-linux-nsh.sh"
    match = re.search(r"(?m)^app_dir=(.*)$", text)
    if not match:
        return f"{wrapper} names no app_dir: run scripts/setup-linux-nsh.sh"
    # setup-linux-nsh.sh writes the path with printf %q; a path it had to quote is compared as written.
    written = " ".join(shlex.split(match.group(1)))
    if Path(written) != app_nsh:
        return (f"{wrapper} is for {written}, not this checkout's {app_nsh}: run scripts/setup-linux-nsh.sh from this "
                "checkout")
    mounted = [home, *_wrapper_mounts(text)]
    tools = tools.resolve()
    if not any(tools == m.resolve() or m.resolve() in tools.parents for m in mounted):
        return f"the NSH tools in {tools} are outside $HOME and the wrapper's mounts: the container cannot see them"
    return None


def _wrapper_mounts(text: str) -> list[Path]:
    """The host folders the wrapper's `mounts=(-v SRC:DST ...)` line mounts (written with printf %q)."""
    match = re.search(r"(?m)^mounts=\((.*)\)$", text)
    if not match:
        return []
    words = shlex.split(match.group(1))
    return [Path(spec.split(":", 1)[0]) for flag, spec in zip(words, words[1:]) if flag == "-v"]


def container_env_refusals(dump: str, expected: dict[str, str]) -> list[str]:
    seen = {}
    for line in dump.splitlines():
        key, sep, value = line.partition("=")
        if sep:
            seen[key] = value
    problems = []
    for key in PROFILE:
        want = expected.get(key)
        if not want:
            problems.append(f"{key} is not set for the build: the recipe sets the release profile before this check")
        elif seen.get(key) != want:
            problems.append(f"{key} is {seen.get(key)!r} in the container, {want!r} in the recipe: the release profile "
                            "did not reach the build (re-run scripts/setup-linux-nsh.sh to rewrite nsh2404.sh)")
    for key in NEVER_IN_RELEASE:
        if seen.get(key):
            problems.append(f"{key} is set in the container: a store build would bake it in")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    d = sub.add_parser("dotenv")
    d.add_argument("root", type=Path)
    d.add_argument("--allow-dotenv", action="store_true")
    w = sub.add_parser("wrapper")
    for name in ("wrapper", "app_nsh", "tools", "home"):
        w.add_argument(name, type=Path)
    sub.add_parser("container-env")
    args = parser.parse_args(argv)
    if args.command == "dotenv":
        problems = [p for p in [dotenv_refusal(args.root, args.allow_dotenv)] if p]
    elif args.command == "wrapper":
        problems = [p for p in [wrapper_refusal(args.wrapper, args.app_nsh, args.tools, args.home)] if p]
    else:
        problems = container_env_refusals(sys.stdin.read(), {key: os.environ.get(key, "") for key in PROFILE})
    for problem in problems:
        print(f"❌ play-bundle: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
