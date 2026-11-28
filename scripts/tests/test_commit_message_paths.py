import os
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "commit_message_paths.py"

sys.path.insert(0, str(ROOT))
from scripts.commit_message_paths import has_absolute_local_path  # noqa: E402
from scripts.install_git_hooks import install_hook  # noqa: E402


def git(repo: Path, hooks: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", f"core.hooksPath={hooks}", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=True,
    )


def make_git_repo(root: Path) -> tuple[Path, Path]:
    repo = root / "repo"
    hooks = root / "empty-hooks"
    repo.mkdir()
    hooks.mkdir()
    git(repo, hooks, "init", "-q")
    git(repo, hooks, "symbolic-ref", "HEAD", "refs/heads/topic")
    git(repo, hooks, "config", "user.name", "Test User")
    git(repo, hooks, "config", "user.email", "test@example.com")
    return repo, hooks


def commit(repo: Path, hooks: Path, message: str) -> str:
    git(repo, hooks, "commit", "-q", "--allow-empty", "-m", message)
    return git(repo, hooks, "rev-parse", "HEAD").stdout.strip()


class LocalPathPatternTests(unittest.TestCase):
    def test_absolute_local_paths_are_the_positive_control(self):
        messages = [
            "built in /Users/alice/project",
            "built in /Users/alice",
            "built in /home/buz/project",
            "built in /home/buz",
            r"built in C:\Users\Carol\project",
            r"built in c:\users\carol",
            "built in /Volumes/example/project",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))

    def test_placeholders_and_relative_paths_pass(self):
        messages = [
            "use /Users/<name>",
            "use /Users/<name>/project",
            r"use C:\Users\<name>\project",
            "use $HOME/project",
            "use ~/project",
            "use /Users/",
            "use /home/",
            "use /var/tmp/project",
            "use src/project",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))


class CommitRangeTests(unittest.TestCase):
    def test_a_bad_commit_fails_with_its_id_and_never_echoes_the_path(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, hooks = make_git_repo(Path(directory))
            base = commit(repo, hooks, "clean base")
            bad_path = "/Users/alice/project"
            bad = commit(repo, hooks, f"built in {bad_path}")
            clean = commit(repo, hooks, "use $HOME/project")

            result = subprocess.run(
                [sys.executable, str(SCRIPT), "range", base, clean],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn(bad, result.stderr)
            self.assertNotIn(bad_path, result.stderr)
            self.assertNotIn(clean, result.stderr)



class HookTests(unittest.TestCase):
    def run_hook(self, hook: str, message: str, *args: str) -> subprocess.CompletedProcess:
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            message_file = Path(directory) / "message"
            message_file.write_text(message, encoding="utf-8")
            return subprocess.run(
                [sh, str(ROOT / ".githooks" / hook), str(message_file), *args],
                cwd=ROOT,
                capture_output=True,
                text=True,
                check=False,
            )

    def test_commit_msg_hook_refuses_the_positive_control_with_one_line(self):
        bad_path = "/Users/alice/project"
        result = self.run_hook("commit-msg", f"built in {bad_path}\n")

        self.assertEqual(result.returncode, 1)
        lines = result.stderr.strip().splitlines()
        self.assertEqual(len(lines), 1, result.stderr)
        self.assertIn("absolute local path", lines[0])
        self.assertNotIn(bad_path, result.stderr)

    def test_commit_msg_hook_accepts_placeholders(self):
        result = self.run_hook("commit-msg", "use /Users/<name>/project, $HOME/project or ~/project\n")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_pre_push_hook_refuses_a_new_branch_commit(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, empty_hooks = make_git_repo(Path(directory))
            (repo / "scripts").mkdir()
            shutil.copy(ROOT / "scripts" / "commit_message_paths.py", repo / "scripts")
            base = commit(repo, empty_hooks, "clean base")
            bad_path = "/Users/alice/project"
            bad = commit(repo, empty_hooks, f"built in {bad_path}")
            git(repo, empty_hooks, "branch", "main", base)
            input_line = f"refs/heads/topic {bad} refs/heads/topic {'0' * 40}\n"
            result = subprocess.run(
                [sh, str(ROOT / ".githooks" / "pre-push"), "origin", "unused"],
                cwd=repo,
                input=input_line,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn(bad, result.stderr)
            self.assertNotIn(bad_path, result.stderr)


class InstallerTests(unittest.TestCase):
    def make_real_checkout(self, root: Path) -> tuple[Path, Path]:
        repo, empty_hooks = make_git_repo(root)
        shutil.copytree(ROOT / ".githooks", repo / ".githooks")
        (repo / "scripts").mkdir()
        shutil.copy(ROOT / "scripts" / "commit_message_paths.py", repo / "scripts")
        return repo, empty_hooks

    def test_the_installed_commit_msg_chain_refuses_the_positive_control(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, _ = self.make_real_checkout(Path(directory))
            hooks = repo / ".git" / "hooks"
            install_hook(repo, hooks, "commit-msg")
            install_hook(repo, hooks, "commit-msg")
            self.assertTrue(os.stat(hooks / "commit-msg").st_mode & stat.S_IXUSR)
            bad_path = "/Users/alice/project"
            message = Path(directory) / "message"
            message.write_text(f"built in {bad_path}\n", encoding="utf-8")

            result = subprocess.run(
                [sh, str(hooks / "commit-msg"), str(message)],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("absolute local path", result.stderr)
            self.assertNotIn(bad_path, result.stderr)

    def test_the_installed_pre_push_chain_scans_and_preserves_the_local_hooks_input(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, empty_hooks = self.make_real_checkout(Path(directory))
            hooks = repo / ".git" / "hooks"
            seen = hooks / "local-input"
            original = hooks / "pre-push"
            original.write_text(f"#!/bin/sh\ncat >{shlex.quote(str(seen))}\n", encoding="utf-8")
            original.chmod(original.stat().st_mode | stat.S_IXUSR)
            install_hook(repo, hooks, "pre-push")
            install_hook(repo, hooks, "pre-push")
            self.assertTrue(os.stat(hooks / "pre-push").st_mode & stat.S_IXUSR)

            base = commit(repo, empty_hooks, "clean base")
            head = commit(repo, empty_hooks, "use $HOME/project")
            git(repo, empty_hooks, "branch", "main", base)
            input_line = f"refs/heads/topic {head} refs/heads/topic {'0' * 40}\n"
            result = subprocess.run(
                [sh, str(hooks / "pre-push"), "origin", "unused"],
                cwd=repo,
                input=input_line,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(seen.read_text(encoding="utf-8"), input_line)


class RepositoryPolicyTests(unittest.TestCase):
    def test_the_quick_lane_scans_pull_request_commit_messages_before_planning(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        match = re.search(r"(?ms)^  changes:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)", workflow)
        self.assertIsNotNone(match)
        changes = match.group(0)
        step = "- name: Check pull-request commit messages for local paths"
        self.assertIn(step, changes)
        self.assertLess(changes.index(step), changes.index("- name: Decide"))
        self.assertIn("if: github.event_name == 'pull_request'", changes)
        self.assertIn("PR_BASE: ${{ github.event.pull_request.base.sha }}", changes)
        self.assertIn("PR_HEAD: ${{ github.event.pull_request.head.sha }}", changes)
        self.assertIn('python3 -I scripts/commit_message_paths.py range "$PR_BASE" "$PR_HEAD"', changes)

    def test_setup_scripts_install_the_hooks(self):
        for name in ("setup-linux.sh", "setup-macos.sh", "setup-windows.ps1"):
            with self.subTest(name=name):
                self.assertIn("install_git_hooks.py", (ROOT / "scripts" / name).read_text(encoding="utf-8"))

    def test_crew_rules_forbid_absolute_local_paths_in_public_text(self):
        rules = (ROOT / "crew/RULES.md").read_text(encoding="utf-8")
        self.assertIn("no absolute local paths in commit messages, pull request text or issues", rules)


if __name__ == "__main__":
    unittest.main()
