from __future__ import annotations

import hashlib
import io
import os
import platform
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
INSTALLER = ROOT / "packaging/linux/install-linux.sh"


@unittest.skipUnless(platform.system() == "Linux", "uses Linux installer utilities")
class LinuxNshInstallerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name) / "home"
        self.home.mkdir()
        self.downloads = Path(self.temp.name) / "downloads"
        self.downloads.mkdir()
        self.arch = "x64" if platform.machine() == "x86_64" else "arm64"
        self.archive = self.downloads / f"eBIRForms-Linux-{self.arch}-0.2.0.tar.gz"
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, data, mode in (
                ("eBIRForms", b"#!/bin/sh\nexit 0\n", 0o755),
                ("nsh/app.json", b"{}", 0o644),
                ("dev.goldcoders.bir.desktop", b"[Desktop Entry]\nExec=eBIRForms\n", 0o644),
                ("dev.goldcoders.bir.png", b"fixture", 0o644),
            ):
                info = tarfile.TarInfo(f"eBIRForms/{name}")
                info.size = len(data)
                info.mode = mode
                archive.addfile(info, io.BytesIO(data))
        checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        (self.downloads / f"SHA256SUMS-linux-{self.arch}.txt").write_text(
            f"{checksum}  {self.archive.name}\n"
        )

    def run_installer(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["sh", str(INSTALLER), *args],
            env={**os.environ, "HOME": str(self.home)},
            text=True, capture_output=True, check=False,
        )

    def install(self) -> subprocess.CompletedProcess[str]:
        return self.run_installer(
            "--tarball", "--from-dir", str(self.downloads), "--version", "0.2.0"
        )

    def test_tarball_install_and_uninstall_preserves_nsh_layout(self) -> None:
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        app = self.home / ".local/share/ebirforms"
        self.assertEqual((self.home / ".local/bin/ebirforms").resolve(), app / "eBIRForms")
        self.assertTrue((app / "nsh/app.json").is_file())
        self.assertFalse((app / "ebirforms-no-self-update").exists())
        desktop = self.home / ".local/share/applications/dev.goldcoders.bir.desktop"
        self.assertIn(f'Exec="{app}/eBIRForms"', desktop.read_text())
        result = self.run_installer("--uninstall", "--tarball")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(app.exists())
        self.assertFalse(desktop.exists())
        self.assertFalse((self.home / ".local/bin/ebirforms").is_symlink())

    def test_empty_download_directory_is_rejected(self) -> None:
        for path in self.downloads.iterdir():
            path.unlink()
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing checksum file", result.stderr)

    def test_corrupt_archive_is_rejected_before_install(self) -> None:
        self.archive.write_bytes(b"corrupt")
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.home / ".local/bin/ebirforms").exists())

    def test_invalid_version_is_rejected(self) -> None:
        result = self.run_installer("--version", "../bad", "--tarball")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Invalid release version", result.stderr)
