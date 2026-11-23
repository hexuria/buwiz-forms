#!/usr/bin/env python3
"""Publish the NSH signed update feed from archives already on a release."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def run(args: list[str]) -> str:
    return subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE).stdout


def is_archive(name: str, version: str) -> bool:
    return re.fullmatch(
        rf"eBIRForms-{re.escape(version)}-(?:linux|macos|windows)-(?:x86_64|aarch64)\.(?:tar\.gz|zip)",
        name,
    ) is not None


def publish(tag: str, repo: str) -> None:
    key = os.environ.get("NSH_UPDATE_KEY")
    if not key:
        print("::warning::NSH_UPDATE_KEY is absent; skipping latest.json and latest.json.sig; downloads remain published.")
        return
    version = tag.removeprefix("v")
    assets = json.loads(run(["gh", "api", f"repos/{repo}/releases/tags/{tag}", "--jq", ".assets"]))
    names = sorted(asset["name"] for asset in assets if is_archive(asset["name"], version))
    if not names:
        raise RuntimeError("No NSH updater archives on the release")
    with tempfile.TemporaryDirectory(prefix="nsh-update-") as tmp:
        root = Path(tmp)
        secret = root / "update.secret"
        secret.touch(mode=0o600)
        secret.write_text(key)
        archives = []
        for name in names:
            run(["gh", "release", "download", tag, "--repo", repo, "--pattern", name, "--dir", tmp])
            archives.append(str(root / name))
        run(["nsh", "update-manifest", *archives, "--entry", "nsh",
             "--key", str(secret), "--base-url",
             f"https://github.com/{repo}/releases/download/{tag}", "--out", tmp])
        run(["gh", "release", "upload", tag, "--repo", repo, "--clobber",
             str(root / "latest.json"), str(root / "latest.json.sig")])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repo", default=os.environ.get("GH_REPO", "hexuria/buwiz-forms"))
    args = parser.parse_args()
    publish(args.tag, args.repo)
