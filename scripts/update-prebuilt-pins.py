#!/usr/bin/env python3
"""Refresh one prebuilt tool's version and release-archive digest pins."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from typing import Dict, List, Tuple


PIN_FILE = Path(__file__).resolve().with_name("impeccable.pin")
HEX64 = re.compile(r"[0-9a-f]{64}\Z")


@dataclass(frozen=True)
class Tool:
    name: str
    repo: str
    prefix: str
    targets: Tuple[str, ...]

    def archive_name(self, target: str, version: str) -> str:
        if self.name == "gpui-agent-cli":
            return f"gpui-agent-cli-{target}-v{version}.tgz"
        if target.endswith("-pc-windows-msvc"):
            return f"impeccable-{target}-v{version}.zip"
        return f"impeccable-{target}-v{version}.tar.gz"


TOOLS = {
    "gpui-agent-cli": Tool(
        name="gpui-agent-cli",
        repo="hexuria/gpui-agent",
        prefix="GPUI_AGENT_CLI",
        targets=(
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
        ),
    ),
    "impeccable": Tool(
        name="impeccable",
        repo="hexuria/impeccable-rust",
        prefix="IMPECCABLE",
        targets=(
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
            "x86_64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
        ),
    ),
}


class PinUpdateError(Exception):
    pass


def digest_pin_key(tool: Tool, target: str) -> str:
    return f"{tool.prefix}_SHA256_{target.replace('-', '_')}"


def fetch_release(tool: Tool, version: str) -> dict:
    endpoint = f"repos/{tool.repo}/releases/tags/v{version}"
    try:
        result = subprocess.run(
            ["gh", "api", endpoint],
            text=True,
            capture_output=True,
        )
    except OSError as exc:
        raise PinUpdateError(f"gh api {endpoint} could not start: {exc}") from exc
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}"
        raise PinUpdateError(f"gh api {endpoint} failed: {detail}")
    try:
        document = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise PinUpdateError(f"gh api {endpoint} did not return JSON: {exc}") from exc
    if not isinstance(document, dict) or not isinstance(document.get("assets"), list):
        raise PinUpdateError(f"gh api {endpoint} did not return a release asset list")
    return document


def valid_digest(asset: dict) -> str:
    digest = asset.get("digest")
    if not isinstance(digest, str):
        return ""
    digest = digest.strip().lower()
    if digest.startswith("sha256:"):
        digest = digest[len("sha256:"):]
    return digest if HEX64.fullmatch(digest) else ""


def collect_digests(tool: Tool, version: str, release: dict) -> Dict[str, str]:
    assets = {}
    for asset in release["assets"]:
        if isinstance(asset, dict) and isinstance(asset.get("name"), str):
            assets.setdefault(asset["name"], asset)

    problems: List[str] = []
    digests: Dict[str, str] = {}
    for target in tool.targets:
        asset_name = tool.archive_name(target, version)
        asset = assets.get(asset_name)
        if asset is None:
            problems.append(f"{target}: no asset {asset_name}")
            continue
        digest = valid_digest(asset)
        if not digest:
            problems.append(f"{target}: asset {asset_name} has no sha256 digest")
            continue
        digests[digest_pin_key(tool, target)] = digest
    if problems:
        raise PinUpdateError(
            f"release v{version} does not publish every {tool.name} target: "
            + "; ".join(problems)
        )
    return digests


def atomic_write(path: Path, text: str) -> None:
    fd, temporary = tempfile.mkstemp(prefix=path.name + ".", dir=str(path.parent), text=True)
    try:
        with os.fdopen(fd, "w") as handle:
            os.chmod(temporary, os.stat(path).st_mode & 0o777)
            handle.write(text)
        os.replace(temporary, path)
    except Exception:
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass
        raise


def rewrite_pin_file(pin_file: Path, tool: Tool, version: str, digests: Dict[str, str]) -> None:
    try:
        original = pin_file.read_text()
    except OSError as exc:
        raise PinUpdateError(f"cannot read {pin_file}: {exc}") from exc

    updates = {
        f"{tool.prefix}_VERSION": version,
        f"{tool.prefix}_SHA256_FOR": version,
    }
    updates.update(digests)
    missing = set(updates)
    output = []
    for line in original.splitlines():
        key = line.split("=", 1)[0] if "=" in line and not line.lstrip().startswith("#") else ""
        if key in updates:
            output.append(f"{key}={updates[key]}")
            missing.remove(key)
        else:
            output.append(line)
    if missing:
        raise PinUpdateError(
            f"{pin_file} is missing {tool.name} pin lines: " + ", ".join(sorted(missing))
        )

    updated = "\n".join(output) + "\n"
    try:
        atomic_write(pin_file, updated)
    except OSError as exc:
        raise PinUpdateError(f"cannot write {pin_file}: {exc}") from exc


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description="rewrite one prebuilt tool's version and release digest pins"
    )
    parser.add_argument("tool", choices=sorted(TOOLS))
    parser.add_argument("version")
    return parser.parse_args(argv)


def main(argv=None) -> int:
    args = parse_args(argv)
    tool = TOOLS[args.tool]
    try:
        release = fetch_release(tool, args.version)
        digests = collect_digests(tool, args.version, release)
        rewrite_pin_file(PIN_FILE, tool, args.version, digests)
    except PinUpdateError as exc:
        print(f"update-prebuilt-pins: error: {exc}", file=sys.stderr)
        return 1
    print(f"{tool.name} prebuilt pins updated to {args.version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
