from __future__ import annotations

import hashlib
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "packaging/linux/smoke-test.sh"


@unittest.skipUnless(sys.platform == "linux", "uses Linux smoke test utilities")
class LinuxSmokeArchitectureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.downloads = self.root / "downloads"
        self.downloads.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.calls = self.root / "docker-calls"
        docker = self.bin / "docker"
        docker.write_text(
            "#!/usr/bin/env python3\n"
            "import os, sys\n"
            "from pathlib import Path\n"
            "kind = next(x[5:] for x in sys.argv if x.startswith('KIND='))\n"
            "with open(os.environ['DOCKER_CALLS'], 'a') as f: f.write(kind + '\\n')\n"
            "if kind == 'installer':\n"
            "    assert (Path(os.environ['DOWNLOADS']) / 'install-linux.sh').is_file()\n"
            "sys.exit(1 if os.environ.get('FAIL_KIND') == kind else 0)\n"
        )
        docker.chmod(0o755)

    def run_smoke(self, arch: str, fail_kind: str = "") -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["bash", str(SMOKE), "--out", str(self.downloads), "--arch", arch],
            env={
                **os.environ,
                "PATH": f"{self.bin}:{os.environ['PATH']}",
                "SMOKE_EVIDENCE": str(self.root / "evidence"),
                "DOCKER_CALLS": str(self.calls),
                "DOWNLOADS": str(self.downloads),
                "FAIL_KIND": fail_kind,
            },
            text=True, capture_output=True, check=False,
        )

    def prepare_downloads(self, arch: str) -> None:
        archive = self.downloads / f"eBIRForms-Linux-{arch}-0.2.0.tar.gz"
        archive.write_bytes(b"fixture")
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        (self.downloads / f"SHA256SUMS-linux-{arch}.txt").write_text(
            f"{digest}  {archive.name}\n"
        )
        if arch == "x64":
            (self.downloads / "install-linux.sh").touch()

    def test_cases_match_architecture_outputs(self) -> None:
        for arch, expected in (
            ("x64", ["deb", "rpm", "arch", "tarball", "installer"]),
            ("arm64", ["deb", "rpm", "tarball"]),
        ):
            with self.subTest(arch=arch):
                for path in self.downloads.iterdir():
                    path.unlink()
                self.calls.unlink(missing_ok=True)
                self.prepare_downloads(arch)
                result = self.run_smoke(arch)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.calls.read_text().splitlines(), expected)

    def test_empty_downloads_do_not_launch_containers(self) -> None:
        result = self.run_smoke("arm64")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.calls.exists())

    def test_container_failure_stops_remaining_cases(self) -> None:
        self.prepare_downloads("arm64")
        result = self.run_smoke("arm64", fail_kind="rpm")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.calls.read_text().splitlines(), ["deb", "rpm"])

    def test_tarball_cases_do_not_use_x64_installer(self) -> None:
        source = (ROOT / "packaging/linux/smoke-container.sh").read_text()
        cases = re.findall(r"^\s+tarball\)(.*?);;", source, re.MULTILINE | re.DOTALL)
        self.assertEqual(len(cases), 2)
        self.assertIn('tar -xzf "/packages/$BASE.tar.gz"', cases[0])
        self.assertIn('rm -rf "$app"', cases[1])
        for case in cases:
            self.assertNotIn("install-linux.sh", case)
