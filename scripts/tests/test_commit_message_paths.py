from __future__ import annotations

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
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "commit_message_paths.py"
INSTALLER = ROOT / "scripts" / "install_git_hooks.py"

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


def write_executable(path: Path, body: str) -> None:
    path.write_bytes(body.encode("utf-8"))
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def python_stub(directory: Path, name: str, working: bool) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    if not working:
        body = "#!/bin/sh\nexit 9009\n"
    elif name == "py":
        body = (
            "#!/bin/sh\n"
            "if [ \"$1\" = \"-3\" ]; then shift; fi\n"
            f"exec {shlex.quote(sys.executable)} \"$@\"\n"
        )
    else:
        body = f"#!/bin/sh\nexec {shlex.quote(sys.executable)} \"$@\"\n"
    write_executable(directory / name, body)


def path_with_stubs(directory: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["PATH"] = str(directory) + os.pathsep + env.get("PATH", "")
    return env


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

    def test_bare_home_paths_are_refused_before_punctuation_or_space(self):
        messages = [
            "built in /Users/alice\nnext line",
            "built in /Users/alice.",
            "built in (/Users/alice)",
            "built in `/Users/alice`",
            r"built in C:\Users\carol and then stopped",
            r"built in C:\Users\runneradmin and C:\Users\carol",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))

    def test_ci_users_and_placeholder_names_pass(self):
        messages = [
            "use /Users/runner",
            "use /Users/devin",
            "use /home/runner",
            "use /home/ubuntu",
            "use /Users/<name>",
            "use /Users/you",
            r"use C:\Users\runneradmin",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))

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
    def run_hook(
        self,
        hook: str,
        message: str,
        *args: str,
        env: dict[str, str] | None = None,
        input_text: str | None = None,
    ) -> subprocess.CompletedProcess:
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            message_file = Path(directory) / "message"
            message_file.write_text(message, encoding="utf-8")
            return subprocess.run(
                [sh, str(ROOT / ".githooks" / hook), str(message_file), *args],
                cwd=ROOT,
                env=env,
                input=input_text,
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

    def test_commit_msg_hook_ignores_comments_and_the_verbose_diff(self):
        scissors = "# ------------------------ >8 ------------------------"
        messages = [
            "clean subject\n# a comment naming /Users/alice/project\n",
            f"clean subject\n{scissors}\ndiff --git a/x b/x\n+/Users/alice/project\n",
        ]
        for message in messages:
            with self.subTest(message=message):
                result = self.run_hook("commit-msg", message)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_commit_msg_hook_still_refuses_a_bad_path_before_the_verbose_diff(self):
        scissors = "# ------------------------ >8 ------------------------"
        result = self.run_hook(
            "commit-msg",
            f"built in /Users/alice/project\n{scissors}\nclean diff\n",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("absolute local path", result.stderr)

    def test_commit_msg_hook_uses_python_before_a_broken_python3(self):
        with tempfile.TemporaryDirectory() as directory:
            stubs = Path(directory) / "bin"
            python_stub(stubs, "python", working=True)
            python_stub(stubs, "python3", working=False)

            result = self.run_hook(
                "commit-msg",
                "built in /Users/alice/project\n",
                env=path_with_stubs(stubs),
            )

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("absolute local path", result.stderr)

    def test_commit_msg_hook_uses_py_minus_3_before_a_broken_python3(self):
        with tempfile.TemporaryDirectory() as directory:
            stubs = Path(directory) / "bin"
            python_stub(stubs, "python", working=False)
            python_stub(stubs, "py", working=True)
            python_stub(stubs, "python3", working=False)

            result = self.run_hook(
                "commit-msg",
                "built in /Users/alice/project\n",
                env=path_with_stubs(stubs),
            )

        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("absolute local path", result.stderr)

    def test_a_python_2_named_python_is_skipped_not_run(self):
        # An old `python` (Python 2) answers `-c "import sys"`, then fails the script with a SyntaxError, which would
        # refuse every commit on that machine. The hook must ask for the version and go on to the next candidate.
        with tempfile.TemporaryDirectory() as directory:
            stubs = Path(directory) / "bin"
            stubs.mkdir(parents=True)
            write_executable(
                stubs / "python",
                "#!/bin/sh\n"
                'if [ "$1" = "-c" ]; then case "$2" in *version_info*) exit 1 ;; *) exit 0 ;; esac; fi\n'
                'echo "SyntaxError: invalid syntax" >&2\nexit 1\n',
            )
            python_stub(stubs, "python3", working=True)
            env = path_with_stubs(stubs)

            clean = self.run_hook("commit-msg", "a plain message\n", env=env)
            refused = self.run_hook("commit-msg", "built in /Users/alice/project\n", env=env)

        self.assertEqual(clean.returncode, 0, clean.stderr)
        self.assertNotIn("SyntaxError", clean.stderr)
        self.assertEqual(refused.returncode, 1, refused.stderr)
        self.assertIn("absolute local path", refused.stderr)

    def test_hooks_without_a_working_python_print_one_line_and_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            stubs = Path(directory) / "bin"
            python_stub(stubs, "python", working=False)
            python_stub(stubs, "py", working=False)
            python_stub(stubs, "python3", working=False)
            env = path_with_stubs(stubs)

            for hook in ("commit-msg", "pre-push"):
                with self.subTest(hook=hook):
                    result = self.run_hook(
                        hook,
                        "built in /Users/alice/project\n",
                        env=env,
                        input_text="",
                    )
                    self.assertEqual(result.returncode, 0, result.stderr)
                    lines = result.stderr.strip().splitlines()
                    self.assertEqual(len(lines), 1, result.stderr)
                    self.assertIn("python", lines[0].lower())

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
            stubs = Path(directory) / "bin"
            python_stub(stubs, "python", working=True)
            python_stub(stubs, "python3", working=False)
            input_line = f"refs/heads/topic {bad} refs/heads/topic {'0' * 40}\n"
            result = subprocess.run(
                [sh, str(ROOT / ".githooks" / "pre-push"), "origin", "unused"],
                cwd=repo,
                env=path_with_stubs(stubs),
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

    def run_installer(
        self,
        repo: Path,
        global_config: Path,
        cwd: Path | None = None,
        env_override: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        env["GIT_CONFIG_GLOBAL"] = str(global_config)
        env["GIT_CONFIG_NOSYSTEM"] = "1"
        if env_override:
            env.update(env_override)
        return subprocess.run(
            [sys.executable, str(INSTALLER), str(repo)],
            cwd=cwd or repo,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def worktree_files(self, repo: Path) -> dict[str, bytes]:
        files = {}
        for path in repo.rglob("*"):
            relative = path.relative_to(repo)
            if path.is_file() and ".git" not in relative.parts:
                files[str(relative)] = path.read_bytes()
        return files

    def test_the_installed_chain_is_lf_even_when_text_writes_use_crlf(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, _ = self.make_real_checkout(Path(directory))
            hooks = repo / ".git" / "hooks"
            real_write_text = Path.write_text

            def write_crlf(self: Path, data: str, encoding: str | None = None, errors: str | None = None) -> int:
                return real_write_text(self, data.replace("\n", "\r\n"), encoding=encoding, errors=errors)

            with mock.patch.object(Path, "write_text", write_crlf):
                install_hook(repo, hooks, "commit-msg")

            self.assertNotIn(b"\r", (hooks / "commit-msg").read_bytes())

    def test_installer_without_hookspath_installs_idempotently_and_prints_chained_hooks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")

            first = self.run_installer(repo, global_config)
            self.assertEqual(first.returncode, 0, first.stderr)
            hooks = repo / ".git" / "hooks"
            first_bytes = {name: (hooks / name).read_bytes() for name in ("commit-msg", "pre-push")}
            second = self.run_installer(repo, global_config)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first_bytes, {name: (hooks / name).read_bytes() for name in first_bytes})

            for output in (first.stdout, second.stdout):
                self.assertIn("commit-msg", output)
                self.assertIn("pre-push", output)
                self.assertIn("chain", output.lower())
            self.assertFalse((repo / ".githooks" / "commit-msg.local").exists())
            self.assertFalse((repo / ".githooks" / "pre-push.local").exists())

    def test_installer_keeps_an_existing_hook_running_and_prints_the_move(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")
            hooks = repo / ".git" / "hooks"
            marker = root / "local-ran"
            write_executable(
                hooks / "commit-msg",
                f"#!/bin/sh\nprintf local >>{shlex.quote(str(marker))}\n",
            )

            result = self.run_installer(repo, global_config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("commit-msg.local", result.stdout)
            self.assertIn("commit-msg", result.stdout)
            self.assertTrue((hooks / "commit-msg.local").is_file())

            message = root / "message"
            message.write_text("use ~/project\n", encoding="utf-8")
            hook_result = subprocess.run(
                [sh, str(hooks / "commit-msg"), str(message)],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(hook_result.returncode, 0, hook_result.stderr)
            self.assertEqual(marker.read_text(encoding="utf-8"), "local")

    def test_installer_preserves_an_existing_hooks_failure_and_prints_the_move(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")
            hooks = repo / ".git" / "hooks"
            write_executable(hooks / "pre-push", "#!/bin/sh\nexit 7\n")

            result = self.run_installer(repo, global_config)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("pre-push.local", result.stdout)
            self.assertTrue((hooks / "pre-push.local").is_file())

            hook_result = subprocess.run(
                [sh, str(hooks / "pre-push"), "origin", "unused"],
                cwd=repo,
                input="",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(hook_result.returncode, 7)

    def test_installer_refuses_a_local_hookspath_without_touching_the_worktree(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, empty_hooks = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")
            git(repo, empty_hooks, "config", "core.hooksPath", ".githooks")
            before = self.worktree_files(repo)

            result = self.run_installer(repo, global_config)

            self.assertEqual(result.returncode, 0, result.stderr)
            lines = result.stdout.strip().splitlines()
            self.assertEqual(len(lines), 1, result.stdout)
            self.assertIn("core.hooksPath", lines[0])
            self.assertIn("by hand", lines[0])
            self.assertEqual(before, self.worktree_files(repo))
            self.assertFalse((repo / ".git" / "hooks" / "commit-msg").exists())
            self.assertFalse((repo / ".git" / "hooks" / "pre-push").exists())

    def test_installer_refuses_a_global_hookspath_without_moving_the_users_hooks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_hooks = root / "global-hooks"
            global_hooks.mkdir()
            sentinel = b"#!/bin/sh\nexit 0\n"
            (global_hooks / "commit-msg").write_bytes(sentinel)
            global_config = root / "global-config"
            global_config.write_text(f"[core]\n\thooksPath = {global_hooks}\n", encoding="utf-8")
            before = self.worktree_files(repo)

            result = self.run_installer(repo, global_config)

            self.assertEqual(result.returncode, 0, result.stderr)
            lines = result.stdout.strip().splitlines()
            self.assertEqual(len(lines), 1, result.stdout)
            self.assertIn("core.hooksPath", lines[0])
            self.assertIn("by hand", lines[0])
            self.assertEqual((global_hooks / "commit-msg").read_bytes(), sentinel)
            self.assertFalse((global_hooks / "commit-msg.local").exists())
            self.assertFalse((global_hooks / "pre-push").exists())
            self.assertEqual(before, self.worktree_files(repo))

    def test_installer_uses_the_repository_root_passed_on_the_command_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")
            outside = root / "outside"
            outside.mkdir()

            result = self.run_installer(repo, global_config, cwd=outside)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue((repo / ".git" / "hooks" / "commit-msg").is_file())
            self.assertTrue((repo / ".git" / "hooks" / "pre-push").is_file())

    def test_installer_prints_one_line_and_continues_when_git_reports_dubious_ownership(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = self.make_real_checkout(root)
            global_config = root / "empty-global-config"
            global_config.write_text("", encoding="utf-8")
            stubs = root / "bin"
            stubs.mkdir()
            write_executable(
                stubs / "git",
                "#!/bin/sh\n"
                "echo \"fatal: detected dubious ownership in repository at '<repo>'\" >&2\n"
                "exit 128\n",
            )

            result = self.run_installer(repo, global_config, env_override={"PATH": str(stubs)})

            self.assertEqual(result.returncode, 0, result.stderr)
            lines = result.stdout.strip().splitlines()
            self.assertEqual(len(lines), 1, result.stdout)
            self.assertIn("dubious ownership", lines[0])
            self.assertIn("not installing", lines[0])
            self.assertFalse((repo / ".git" / "hooks" / "commit-msg").exists())

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

    def test_setup_scripts_install_the_hooks_for_an_explicit_repository_root(self):
        linux = (ROOT / "scripts/setup-linux.sh").read_text(encoding="utf-8")
        macos = (ROOT / "scripts/setup-macos.sh").read_text(encoding="utf-8")
        windows = (ROOT / "scripts/setup-windows.ps1").read_text(encoding="utf-8")
        self.assertIn('python3 "$SCRIPT_DIR/install_git_hooks.py" "$REPO_ROOT"', linux)
        self.assertIn('python3 "$REPO_ROOT/scripts/install_git_hooks.py" "$REPO_ROOT"', macos)
        self.assertIn(
            "python (Join-Path $PSScriptRoot 'install_git_hooks.py') (Split-Path $PSScriptRoot -Parent)",
            windows,
        )

    def test_crew_rules_forbid_absolute_local_paths_in_public_text(self):
        rules = (ROOT / "crew/RULES.md").read_text(encoding="utf-8")
        self.assertIn("no absolute local paths in commit messages, pull request text or issues", rules)


if __name__ == "__main__":
    unittest.main()
