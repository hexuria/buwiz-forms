"""scripts/ci/apt_update.sh: which apt source lists it sets aside, and how often it is safe to run."""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/ci/apt_update.sh"


class AptSourceFilterTests(unittest.TestCase):
    def run_filter(self, directory: Path, dry_run: str = "1", env_extra: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        command = 'source "$1"\nset_aside_third_party_source_lists "$2" "$3"\n'
        env = dict(os.environ)
        if env_extra:
            env.update(env_extra)
        return subprocess.run(
            ["bash", "-c", command, "_", str(SCRIPT), str(directory), dry_run],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_ubuntu_sources_are_kept_and_third_party_lists_are_set_aside(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "ubuntu.sources").write_text(
                "Types: deb deb-src\n"
                "URIs: mirror+file:/etc/apt/apt-mirrors.txt\n"
                "Suites: noble noble-updates noble-backports noble-security\n"
                "Components: main restricted universe multiverse\n"
                "Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg\n",
                encoding="utf-8",
            )
            (root / "microsoft.list").write_text(
                "deb [arch=amd64 signed-by=/usr/share/keyrings/microsoft.gpg] "
                "https://packages.microsoft.com/ubuntu/24.04/prod noble main\n",
                encoding="utf-8",
            )
            (root / "third-party.sources").write_text(
                "Types: deb\n"
                "URIs: https://download.example.invalid/ubuntu\n"
                "  https://backup.example.invalid/ubuntu\n"
                "Suites: stable\n"
                "Components: main\n",
                encoding="utf-8",
            )

            result = self.run_filter(root, "1")

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("microsoft.list", result.stdout)
            self.assertIn("third-party.sources", result.stdout)
            self.assertNotIn("ubuntu.sources", result.stdout)
            self.assertTrue((root / "ubuntu.sources").exists())
            self.assertTrue((root / "microsoft.list").exists(), "a dry run moves nothing")
            self.assertTrue((root / "third-party.sources").exists(), "a dry run moves nothing")

    def test_empty_directory_is_quietly_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = self.run_filter(Path(tmp), "1")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")

    def test_ubuntu_hosts_and_the_runner_mirror_list_are_kept(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            mirrors = root / "apt-mirrors.txt"
            mirrors.write_text(
                "http://azure.archive.ubuntu.com/ubuntu/\tpriority:1\n"
                "https://runner-mirror.example.invalid/ubuntu/\tpriority:2\n",
                encoding="utf-8",
            )
            (root / "ubuntu-mirrors.list").write_text(
                "deb http://azure.archive.ubuntu.com/ubuntu noble main\n"
                "deb https://runner-mirror.example.invalid/ubuntu noble main\n"
                "deb http://ports.ubuntu.com/ubuntu-ports noble main\n"
                "deb https://security.ubuntu.com/ubuntu noble-security main\n",
                encoding="utf-8",
            )
            result = self.run_filter(root, "1", {"APT_MIRRORS_FILE": str(mirrors)})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "")

    def test_actual_run_is_safe_twice_and_never_overwrites_what_it_set_aside(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "sources"
            backup = Path(tmp) / "set-aside"
            root.mkdir()
            source = "deb https://packages.microsoft.com/ubuntu/24.04/prod noble main\n"
            microsoft = root / "microsoft.list"
            microsoft.write_text(source, encoding="utf-8")
            env = {"APT_SOURCE_SET_ASIDE_DIR": str(backup)}

            first = self.run_filter(root, "0", env)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertFalse(microsoft.exists())
            self.assertEqual((backup / "microsoft.list").read_text(encoding="utf-8"), source)

            microsoft.write_text(source, encoding="utf-8")
            second = self.run_filter(root, "0", env)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertFalse(microsoft.exists())
            self.assertEqual((backup / "microsoft.list.1").read_text(encoding="utf-8"), source)

            third = self.run_filter(root, "0", env)
            self.assertEqual(third.returncode, 0, third.stderr)


class AptUpdateRetryDecisionTests(unittest.TestCase):
    def run_decision(self, status: int, output: str) -> subprocess.CompletedProcess[str]:
        command = 'source "$1"\napt_update_needs_retry "$2" "$3"\n'
        return subprocess.run(
            ["bash", "-c", command, "_", str(SCRIPT), str(status), output],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_a_nonzero_exit_retries(self) -> None:
        result = self.run_decision(100, "")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_clean_zero_exit_does_not_retry(self) -> None:
        output = "Hit:1 http://archive.ubuntu.com/ubuntu noble InRelease\nReading package lists...\n"
        result = self.run_decision(0, output)
        self.assertEqual(result.returncode, 1, result.stderr)

    def test_a_zero_exit_with_a_failed_fetch_retries(self) -> None:
        for level in ("E", "W"):
            with self.subTest(level=level):
                output = (
                    "Hit:1 http://archive.ubuntu.com/ubuntu noble InRelease\n"
                    f"{level}: Failed to fetch http://security.ubuntu.com/ubuntu/dists/noble-security/InRelease 403\n"
                    "Reading package lists...\n"
                )
                result = self.run_decision(0, output)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_failed_fetch_must_start_the_line(self) -> None:
        output = "Reading package lists...\nSome index files failed to download; W: Failed to fetch is later in the line\n"
        result = self.run_decision(0, output)
        self.assertEqual(result.returncode, 1, result.stderr)

    def test_the_update_retries_a_zero_exit_failed_fetch(self) -> None:
        command = """
source "$1"
apt-get() {
  printf '%s\\n' 'W: Failed to fetch http://archive.ubuntu.com/ubuntu/dists/noble/InRelease 403'
  return 0
}
sleep() { :; }
if apt_update_with_retries; then
  status=0
else
  status=$?
fi
printf 'status=%s\\n' "$status"
"""
        result = subprocess.run(
            ["bash", "-c", command, "_", str(SCRIPT)],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("status=1", result.stdout)
        self.assertEqual(result.stdout.count("W: Failed to fetch"), 3)


class WorkflowAptPolicyTests(unittest.TestCase):
    def test_no_workflow_runs_a_bare_apt_get_update(self) -> None:
        offenders: list[str] = []
        for workflow in sorted((ROOT / ".github/workflows").glob("*.yml")):
            for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), 1):
                if re.fullmatch(r"\s*(?:sudo\s+)?apt-get\s+update\s*", line):
                    offenders.append(f"{workflow.relative_to(ROOT)}:{number}")
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
