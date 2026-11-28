#!/usr/bin/env python3
"""The early refusals `just play-bundle` makes before it builds a store bundle.

These are refusals, not the proof: what a build really became is read from the bundle itself afterwards
(`scripts/android_bundle.py check`: the production addresses present, no development marker, the library under its
size ceiling). These stop the wrong build before it costs half an hour.

    play_bundle_guard.py args [ARG...]
        Reads the recipe's arguments: at most one build number (1 to 2100000000, no leading zero) and --allow-dotenv.
        Prints `<build or -> <0|1>`.

    play_bundle_guard.py dotenv ROOT [--allow-dotenv]
        Refuses when a `.env` is in ROOT or a folder above it (where `just` finds one for `set dotenv-load`), unless
        --allow-dotenv. With the flag, prints to stderr the NAMES (never the values) of the variables in it that the
        build bakes into the binary: nothing by default (the recipe unsets the server addresses and the Google client),
        and GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET only with STORE_GMAIL=1 (Gmail Connect, which needs Google's
        verification of the app first).

    play_bundle_guard.py wrapper WRAPPER APP_NSH TOOLS HOME
        On Linux the nsh CLI runs in the Ubuntu 24.04 image through the nsh2404.sh that scripts/setup-linux-nsh.sh writes
        for one checkout. Refuses a wrapper written for another checkout, or NSH tools the container cannot see.

    ENV-DUMP | play_bundle_guard.py container-env
        Reads `env` as printed where the build runs and refuses: the recipe's release profile missing or changed there
        (CARGO_PROFILE_RELEASE_LTO, _CODEGEN_UNITS, _OPT_LEVEL, compared with this process's), any other
        CARGO_PROFILE_RELEASE_* (debug assertions would make the store binary a debug build), any RUSTFLAGS form, and
        what would make the binary a development one or bake a server address in.

    play_bundle_guard.py cargo-config DIR... [--cargo-home DIR]
        Reads the Cargo configuration files Cargo would read for a build under each DIR (DIR and every folder above
        it, then $CARGO_HOME) and refuses what `env` cannot show: a [profile.release] setting, rustflags in [build] or
        [target.*], or an [env] entry for a variable the app bakes in.
"""

from __future__ import annotations

import argparse
import os
import re
import shlex
import sys
from pathlib import Path

PROFILE = ("CARGO_PROFILE_RELEASE_LTO", "CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "CARGO_PROFILE_RELEASE_OPT_LEVEL")
# Every variable the app reads at compile time (option_env!, and bir-core/build.rs for the two addresses);
# scripts/tests/test_play_bundle_guard.py checks this list against the sources.
COMPILE_TIME = ("BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL", "BUWIZ_ANDROID_DEV", "BUWIZ_ANDROID_DEV_ENV",
                "GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET")
# What the recipe keeps out of a store build: set where the build runs, each would end up in the binary.
NEVER_IN_RELEASE = ("BUWIZ_ANDROID_DEV", "BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL")
# What a store build bakes in from a .env when --allow-dotenv lets it, and only with STORE_GMAIL=1: the Google client
# for Gmail Connect. Without the switch the recipe unsets it and nothing from a .env is baked.
GMAIL = ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET")


def store_gmail() -> bool:
    return os.environ.get("STORE_GMAIL") == "1"
RUSTFLAGS = re.compile(r"^(RUSTFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_BUILD_RUSTFLAGS|CARGO_TARGET_[A-Z0-9_]+_RUSTFLAGS)$")
# A compiler wrapper can add any flag to every rustc call.
WRAPPERS = ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
MAX_BUILD = 2_100_000_000


class Refusal(Exception):
    pass


def parse_args(args: list[str]) -> tuple[str | None, bool]:
    build, allow = None, False
    for arg in args:
        if arg == "--allow-dotenv":
            allow = True
        elif re.fullmatch(r"[1-9][0-9]*", arg) and int(arg) <= MAX_BUILD:
            if build is not None:
                raise Refusal(f"two build numbers: {build} and {arg}")
            build = arg
        else:
            raise Refusal(f"unknown argument {arg!r}: a build number from 1 to {MAX_BUILD} with no leading zero, "
                          "or --allow-dotenv")
    return build, allow


def dotenv_files(root: Path) -> list[Path]:
    """Every `.env` file in `root` or a folder above it."""
    root = root.resolve()
    return [folder / ".env" for folder in (root, *root.parents) if (folder / ".env").is_file()]


def dotenv_names(path: Path) -> list[str]:
    names = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        match = re.match(r"\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=", line)
        if match:
            names.append(match.group(1))
    return names


def dotenv_refusal(root: Path, allow: bool) -> str | None:
    found = dotenv_files(root)
    if not found or allow:
        return None
    return (f"{found[0]} would be loaded into the build (`set dotenv-load`). With --allow-dotenv the bundle bakes in "
            f"nothing from it (only {' and '.join(GMAIL)} with STORE_GMAIL=1, for Gmail Connect); pass it when the "
            "file is meant (App Store Connect's next build number reads it), or move the file away")


def dotenv_report(root: Path) -> str | None:
    """With --allow-dotenv: which names from the .env files the build bakes in (never their values)."""
    found = dotenv_files(root)
    if not found:
        return None
    names = sorted({name for path in found for name in dotenv_names(path)})
    baked = [name for name in names if name in GMAIL and store_gmail()]
    kept_out = [name for name in names if name in (*NEVER_IN_RELEASE, "BUWIZ_ANDROID_DEV_ENV")
                or (name in GMAIL and not store_gmail())]
    line = f"--allow-dotenv: from {', '.join(map(str, found))} the bundle bakes in: {', '.join(baked) or 'nothing'}"
    if kept_out:
        line += f"; kept out (unset): {', '.join(kept_out)}"
    return line


def wrapper_refusal(wrapper: Path, app_nsh: Path, tools: Path, home: Path) -> str | None:
    try:
        text = wrapper.read_text(encoding="utf-8")
    except OSError as error:
        return f"cannot read {wrapper} ({error}): run scripts/setup-linux-nsh.sh"
    match = re.search(r"(?m)^app_dir=(.*)$", text)
    if not match:
        return f"{wrapper} names no app_dir: run scripts/setup-linux-nsh.sh"
    # setup-linux-nsh.sh writes the path with printf %q; read it as the shell would.
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
    if not dump.strip():
        return ["the build environment could not be read (see the error above)"]
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
            problems.append(f"{key} is {seen.get(key)!r} where the build runs, {want!r} in the recipe: the release "
                            "profile did not reach the build (re-run scripts/setup-linux-nsh.sh to rewrite nsh2404.sh)")
    for key in sorted(seen):
        if key.startswith("CARGO_PROFILE_RELEASE_") and key not in PROFILE:
            problems.append(f"{key} is set where the build runs: a store build takes the recipe's release profile only "
                            "(debug assertions would make it a debug build)")
        if RUSTFLAGS.match(key) and seen[key].strip():
            problems.append(f"{key} is set where the build runs: rustflags can change what a release build becomes")
        if key in WRAPPERS and seen[key].strip():
            problems.append(f"{key} is set where the build runs: a compiler wrapper can change what a release build "
                            "becomes (unset it for the store build)")
    for key in NEVER_IN_RELEASE:
        if seen.get(key):
            problems.append(f"{key} is set where the build runs: a store build would bake it in")
    if not store_gmail():
        for key in GMAIL:
            if seen.get(key):
                problems.append(f"{key} is set where the build runs without STORE_GMAIL=1: Gmail Connect would be "
                                "compiled in")
    return problems


def cargo_config_files(dirs: list[Path], cargo_home: Path) -> list[Path]:
    """The Cargo configuration files Cargo reads for a build under any of `dirs`, deduplicated, in order."""
    files: list[Path] = []
    folders = [folder for d in dirs for folder in (d.resolve(), *d.resolve().parents)]
    candidates = [folder / ".cargo" / name for folder in folders for name in ("config.toml", "config")]
    candidates += [cargo_home / name for name in ("config.toml", "config")]
    for path in candidates:
        if path.is_file() and path not in files:
            files.append(path)
    return files


def cargo_config_refusals(path: Path, text: str) -> list[str]:
    """What in one Cargo configuration file would change a store build without showing in `env`.

    A line reader, not a TOML parser (Python before 3.11 has none): it reads table headers and the keys under them,
    dotted keys included, which is how these settings are written. Inline tables (`profile = { release = ... }`) are
    refused whole for the tables that matter."""
    problems = []
    table = ""
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        header = re.fullmatch(r"\[\[?\s*([^\]]+?)\s*\]\]?", line)
        if header:
            table = _dotted(header.group(1))
            if table.startswith("profile.release"):
                problems.append(f"{path}:{number}: [{header.group(1)}] changes the release profile")
            continue
        key = re.match(r"([A-Za-z0-9_\-\"'. ]+?)\s*=", line)
        if not key:
            continue
        full = ".".join(part for part in (table, _dotted(key.group(1))) if part)
        if full == "include":
            problems.append(f"{path}:{number}: include brings in configuration this check does not read")
        elif full.startswith("profile.release") or full in ("profile",):
            problems.append(f"{path}:{number}: {full} changes the release profile")
        elif full in ("build.rustc-wrapper", "build.rustc-workspace-wrapper") or (
                full == "build" and "wrapper" in line):
            problems.append(f"{path}:{number}: {full} sets a compiler wrapper, which can add any flag: comment that line "
                            "out for the store build and put it back after (an empty RUSTC_WRAPPER does not reach the "
                            "Linux container, so the file is what counts)")
        elif (full == "build.rustflags" or (full.startswith("target.") and full.endswith(".rustflags"))
              or (full in ("build", "target") and "rustflags" in line)):
            problems.append(f"{path}:{number}: {full} sets rustflags")
        elif full.startswith("env."):
            name = full[len("env."):].split(".", 1)[0]
            if name in COMPILE_TIME:
                problems.append(f"{path}:{number}: [env] {name} would be baked into the binary")
        elif full == "env" and any(name in line for name in COMPILE_TIME):
            problems.append(f"{path}:{number}: [env] names a variable the app bakes in")
    return problems


def _dotted(text: str) -> str:
    return ".".join(part.strip().strip("\"'") for part in re.findall(r"\"[^\"]*\"|'[^']*'|[^.]+", text))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    a = sub.add_parser("args")
    a.add_argument("values", nargs=argparse.REMAINDER)
    d = sub.add_parser("dotenv")
    d.add_argument("root", type=Path)
    d.add_argument("--allow-dotenv", action="store_true")
    w = sub.add_parser("wrapper")
    for name in ("wrapper", "app_nsh", "tools", "home"):
        w.add_argument(name, type=Path)
    sub.add_parser("container-env")
    c = sub.add_parser("cargo-config")
    c.add_argument("dirs", type=Path, nargs="+")
    c.add_argument("--cargo-home", type=Path)
    args = parser.parse_args(argv)
    problems: list[str] = []
    if args.command == "args":
        values = [v for v in args.values if v != "--"] if args.values[:1] == ["--"] else args.values
        try:
            build, allow = parse_args(values)
        except Refusal as refusal:
            problems = [str(refusal)]
        else:
            print(f"{build or '-'} {1 if allow else 0}")
    elif args.command == "dotenv":
        refusal = dotenv_refusal(args.root, args.allow_dotenv)
        if refusal:
            problems = [refusal]
        elif args.allow_dotenv and (report := dotenv_report(args.root)):
            print(f"play-bundle: {report}", file=sys.stderr)
    elif args.command == "wrapper":
        problems = [p for p in [wrapper_refusal(args.wrapper, args.app_nsh, args.tools, args.home)] if p]
    elif args.command == "container-env":
        problems = container_env_refusals(sys.stdin.read(), {key: os.environ.get(key, "") for key in PROFILE})
    else:
        home = args.cargo_home or Path(os.environ.get("CARGO_HOME") or Path.home() / ".cargo")
        for path in cargo_config_files(args.dirs, home):
            problems += cargo_config_refusals(path, path.read_text(encoding="utf-8", errors="replace"))
    for problem in problems:
        print(f"❌ play-bundle: {problem}", file=sys.stderr)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
