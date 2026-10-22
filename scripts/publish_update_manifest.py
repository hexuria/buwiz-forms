#!/usr/bin/env python3
"""Publish ``latest.json`` for the desktop auto-updater.

The manifest lists one asset per platform key, with the sha256 taken from the
release's checksum sidecars (``SHA256SUMS-*.txt`` and ``*.sha256``). Both the
tag's release workflow and the macOS publish job run this after they upload,
so whichever finishes last rewrites a manifest that includes every asset
already on the release.

Usage:
    python3 scripts/publish_update_manifest.py --tag v0.1.0 --repo hexuria/buwiz-forms
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def classify_asset(name: str) -> str | None:
    """Map a release file name to a manifest platform key."""
    lower = name.lower()
    if lower.endswith(".dmg") and "macos" in lower:
        return "darwin-universal"
    if lower.endswith("-setup.exe") and "windows" in lower and "x64" in lower:
        return "windows-x64-setup"
    if lower.endswith(".msi") and "windows" in lower and "x64" in lower:
        return "windows-x64-msi"
    if "linux" not in lower:
        return None
    arch = "arm64" if "arm64" in lower or "aarch64" in lower else "x64" if "x64" in lower else None
    if arch is None:
        return None
    if lower.endswith(".deb"):
        kind = "deb"
    elif lower.endswith(".rpm"):
        kind = "rpm"
    elif lower.endswith(".pkg.tar.zst"):
        kind = "arch"
    elif lower.endswith(".tar.gz"):
        kind = "tarball"
    else:
        return None
    if kind == "arch" and arch != "x64":
        return None
    return f"linux-{arch}-{kind}"


def parse_checksum_text(text: str) -> dict[str, str]:
    """``shasum`` / ``sha256sum`` lines, or one bare hex digest."""
    found: dict[str, str] = {}
    bare: list[str] = []
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        digest = parts[0].lower()
        if len(digest) != 64 or any(ch not in "0123456789abcdef" for ch in digest):
            continue
        if len(parts) == 1:
            bare.append(digest)
            continue
        filename = parts[1][1:] if parts[1].startswith("*") else parts[1]
        found[Path(filename).name] = digest
    if not found and len(bare) == 1:
        found[""] = bare[0]
    return found


def build_manifest(
    version: str,
    repo: str,
    tag: str,
    assets: list[dict[str, str]],
    checksums: dict[str, str],
) -> dict:
    """``assets`` entries have ``name``. Checksums are keyed by file name."""
    platforms: dict[str, dict[str, str]] = {}
    checksum_names = {
        name: name
        for name in checksums
        if name.endswith(".sha256") or name.startswith("SHA256SUMS")
    }
    for asset in assets:
        name = asset["name"]
        key = classify_asset(name)
        if key is None or key in platforms:
            continue
        digest = checksums.get(name) or checksums.get("")
        if not digest:
            continue
        sidecar = _sidecar_for(name, list(checksum_names))
        entry = {
            "url": f"https://github.com/{repo}/releases/download/{tag}/{name}",
            "sha256": digest,
            "name": name,
        }
        if sidecar:
            entry["checksum_url"] = (
                f"https://github.com/{repo}/releases/download/{tag}/{sidecar}"
            )
        platforms[key] = entry
    return {"version": version, "platforms": platforms}


def _sidecar_for(asset_name: str, checksum_names: list[str]) -> str | None:
    direct = f"{asset_name}.sha256"
    if direct in checksum_names:
        return direct
    lower = asset_name.lower()
    if "linux" in lower:
        arch = "arm64" if "arm64" in lower or "aarch64" in lower else "x64"
        candidate = f"SHA256SUMS-linux-{arch}.txt"
        if candidate in checksum_names:
            return candidate
    if "windows" in lower:
        candidate = "SHA256SUMS-windows-x64.txt"
        if candidate in checksum_names:
            return candidate
    return None


def _gh(args: list[str]) -> str:
    result = subprocess.run(
        ["gh", *args],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result.stdout


def publish(tag: str, repo: str, dry_run: bool) -> dict:
    version = tag[1:] if tag.startswith("v") else tag
    payload = json.loads(
        _gh(
            [
                "api",
                f"repos/{repo}/releases/tags/{tag}",
                "--jq",
                "{assets: [.assets[] | {name: .name, url: .url}]}",
            ]
        )
    )
    assets = payload["assets"]
    checksums: dict[str, str] = {}
    checksum_assets = [
        asset["name"]
        for asset in assets
        if asset["name"].endswith(".sha256") or asset["name"].startswith("SHA256SUMS")
    ]
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        for name in checksum_assets:
            dest = root / name
            _gh(
                [
                    "release",
                    "download",
                    tag,
                    "--repo",
                    repo,
                    "--pattern",
                    name,
                    "--dir",
                    str(root),
                    "--clobber",
                ]
            )
            parsed = parse_checksum_text(dest.read_text(encoding="utf-8", errors="replace"))
            if "" in parsed and name.endswith(".sha256"):
                checksums[name[: -len(".sha256")]] = parsed[""]
            checksums.update({key: value for key, value in parsed.items() if key})
            checksums[name] = name
    manifest = build_manifest(version, repo, tag, assets, checksums)
    if not manifest["platforms"]:
        raise SystemExit(f"no update assets with checksums on {repo}@{tag}")
    text = json.dumps(manifest, indent=2) + "\n"
    if dry_run:
        sys.stdout.write(text)
        return manifest
    path = Path(tempfile.mkdtemp()) / "latest.json"
    path.write_text(text, encoding="utf-8")
    _gh(
        [
            "release",
            "upload",
            tag,
            "--repo",
            repo,
            "--clobber",
            str(path),
        ]
    )
    print(f"uploaded latest.json with {len(manifest['platforms'])} platform(s)")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", default=os.environ.get("GH_REPO", "hexuria/buwiz-forms"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    publish(args.tag, args.repo, args.dry_run)


if __name__ == "__main__":
    main()
