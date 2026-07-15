#!/usr/bin/env python3
"""Install the exact Typst CLI release used by packaged legacy print fallback.

Release packaging must not copy an arbitrary host installation or follow a
mutable ``latest`` URL.  This helper downloads a target-specific archive from a
versioned upstream release, verifies its SHA-256 digest, and extracts only the
expected executable member.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import stat
import subprocess
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path


TYPST_VERSION = "0.13.1"
RELEASE_BASE_URL = (
    f"https://github.com/typst/typst/releases/download/v{TYPST_VERSION}"
)
MAX_BINARY_BYTES = 128 * 1024 * 1024


@dataclass(frozen=True)
class PinnedArchive:
    filename: str
    sha256: str
    member: str

    @property
    def url(self) -> str:
        return f"{RELEASE_BASE_URL}/{self.filename}"


PINNED_ARCHIVES: dict[str, PinnedArchive] = {
    "aarch64-apple-darwin": PinnedArchive(
        filename="typst-aarch64-apple-darwin.tar.xz",
        sha256="541e4f9eaca3f34ee865f81fc663e4839cb84d6253f71a372cd855b0a7283213",
        member="typst-aarch64-apple-darwin/typst",
    ),
    "x86_64-apple-darwin": PinnedArchive(
        filename="typst-x86_64-apple-darwin.tar.xz",
        sha256="4dabfe647f7f01ed9cc13ad8196a6c7f5e16f0732821b522d50740d3a9f5207b",
        member="typst-x86_64-apple-darwin/typst",
    ),
    "x86_64-pc-windows-msvc": PinnedArchive(
        filename="typst-x86_64-pc-windows-msvc.zip",
        sha256="44170d0632298ba68cbabc43dbfb6908b17ca9236859e0767b0e5d54b2d19f48",
        member="typst-x86_64-pc-windows-msvc/typst.exe",
    ),
    "x86_64-unknown-linux-musl": PinnedArchive(
        filename="typst-x86_64-unknown-linux-musl.tar.xz",
        sha256="7d214bfeffc2e585dc422d1a09d2b144969421281e8c7f5d784b65fc69b5673f",
        member="typst-x86_64-unknown-linux-musl/typst",
    ),
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_archive(path: Path, expected_sha256: str) -> None:
    actual = sha256_file(path)
    if actual != expected_sha256:
        raise ValueError(
            f"Typst archive checksum mismatch for {path.name}: "
            f"expected {expected_sha256}, got {actual}"
        )


def download_archive(spec: PinnedArchive, cache_dir: Path) -> Path:
    cache_dir.mkdir(parents=True, exist_ok=True)
    destination = cache_dir / spec.filename
    if destination.is_file():
        try:
            verify_archive(destination, spec.sha256)
            return destination
        except ValueError:
            destination.unlink()

    request = urllib.request.Request(
        spec.url,
        headers={"User-Agent": "eBIRForms-release-packager/1"},
    )
    temporary_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            prefix=f".{spec.filename}.",
            suffix=".download",
            dir=cache_dir,
            delete=False,
        ) as temporary:
            temporary_path = Path(temporary.name)
            try:
                with urllib.request.urlopen(request, timeout=60) as response:
                    shutil.copyfileobj(response, temporary)
            except urllib.error.URLError:
                # Some developer Python installations do not inherit the OS
                # trust store. Use the platform curl client as a validating TLS
                # fallback; the pinned SHA-256 remains mandatory afterward.
                curl = shutil.which("curl")
                if curl is None:
                    raise
                temporary.close()
                subprocess.run(
                    [
                        curl,
                        "--fail",
                        "--location",
                        "--retry",
                        "3",
                        "--silent",
                        "--show-error",
                        "--output",
                        str(temporary_path),
                        spec.url,
                    ],
                    check=True,
                )
        verify_archive(temporary_path, spec.sha256)
        os.replace(temporary_path, destination)
    finally:
        if temporary_path is not None and temporary_path.exists():
            temporary_path.unlink()
    return destination


def read_binary(archive_path: Path, spec: PinnedArchive) -> bytes:
    if spec.filename.endswith(".zip"):
        with zipfile.ZipFile(archive_path) as archive:
            info = archive.getinfo(spec.member)
            if info.is_dir() or info.file_size > MAX_BINARY_BYTES:
                raise ValueError(f"invalid Typst executable member: {spec.member}")
            return archive.read(info)

    if spec.filename.endswith(".tar.xz"):
        with tarfile.open(archive_path, mode="r:xz") as archive:
            member = archive.getmember(spec.member)
            if not member.isfile() or member.size > MAX_BINARY_BYTES:
                raise ValueError(f"invalid Typst executable member: {spec.member}")
            extracted = archive.extractfile(member)
            if extracted is None:
                raise ValueError(f"missing Typst executable member: {spec.member}")
            return extracted.read()

    raise ValueError(f"unsupported Typst archive format: {spec.filename}")


def install_binary(archive_path: Path, spec: PinnedArchive, output: Path) -> None:
    verify_archive(archive_path, spec.sha256)
    binary = read_binary(archive_path, spec)
    if not binary:
        raise ValueError(f"Typst executable is empty in {archive_path.name}")

    output.parent.mkdir(parents=True, exist_ok=True)
    file_descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.",
        dir=output.parent,
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(file_descriptor, "wb") as handle:
            handle.write(binary)
        temporary_path.chmod(
            temporary_path.stat().st_mode
            | stat.S_IXUSR
            | stat.S_IXGRP
            | stat.S_IXOTH
        )
        os.replace(temporary_path, output)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", choices=sorted(PINNED_ARCHIVES), required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=Path("target/typst-downloads"),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    spec = PINNED_ARCHIVES[args.target]
    archive_path = download_archive(spec, args.cache_dir)
    install_binary(archive_path, spec, args.output)
    print(
        f"Installed Typst {TYPST_VERSION} for {args.target} at {args.output} "
        f"from verified archive {spec.sha256}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
