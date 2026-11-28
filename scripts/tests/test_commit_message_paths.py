from __future__ import annotations

import ast
import io
import os
import re
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "commit_message_paths.py"
INSTALLER = ROOT / "scripts" / "install_git_hooks.py"

sys.path.insert(0, str(ROOT))
from scripts.commit_message_paths import has_absolute_local_path  # noqa: E402
import scripts.install_git_hooks as install_git_hooks  # noqa: E402
from scripts.install_git_hooks import install_hook  # noqa: E402


def git(repo: Path, hooks: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", f"core.hooksPath={hooks}", "-C", str(repo), *args],
        capture_output=True,
        text=True,
        check=True,
    )


def git_bytes(repo: Path, hooks: Path, *args: str, input_bytes: bytes | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", f"core.hooksPath={hooks}", "-C", str(repo), *args],
        input=input_bytes,
        capture_output=True,
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
            "built in '/home/buz/project'",
            'built in "/Users/alice/project"',
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
            "-/home/buz",
            r"-C:\Users\carol",
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

    def test_a_public_log_report_names_the_commit_but_never_repeats_the_line(self):
        import contextlib
        import io as _io

        from scripts import commit_message_paths as module

        bad = [("a" * 40, "built in /Users/alice/project")]
        quiet = _io.StringIO()
        with contextlib.redirect_stderr(quiet):
            module.report_bad_commits(bad, public_log=True)
        text = quiet.getvalue()
        self.assertIn("a" * 40, text)
        self.assertNotIn("/Users/alice", text)
        self.assertNotIn("offending line:", text)
        self.assertIn("git log -1 --format=%B", text)
        loud = _io.StringIO()
        with contextlib.redirect_stderr(loud):
            module.report_bad_commits(bad)
        self.assertIn("offending line: built in /Users/alice/project", loud.getvalue())

    def test_file_urls_with_a_home_or_volume_path_are_refused(self):
        for message in (
            "See file:///Users/alice/notes/plan.md",
            "See file://localhost/home/alice/notes/plan.md",
            "Open file:///Volumes/work/repo/README.md",
            "Log at <file:///home/alice/run.log>",
        ):
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))
        for message in (
            "See file:///Users/runner/work/repo/README.md",
            "See file:///Volumes/…",
            "See https://example.com/Users/alice/profile",
            "See https://example.com/home/alice",
        ):
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))

    def test_file_url_scheme_is_case_insensitive_and_file_must_start_a_word(self):
        for message in (
            "See FILE:///Users/alice/notes/plan.md",
            "See FiLe://LOCALHOST/home/alice/notes/plan.md",
        ):
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))
        for message in (
            "See https://x.com/profile://localhost/home/alice",
            "See xfile:///Users/alice",
        ):
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))

    def test_mounted_and_extended_home_paths_are_refused(self):
        messages = [
            "built in /c/Users/alice/x",
            "built in /mnt/c/Users/alice/x",
            "built in /cygdrive/c/Users/alice",
            "built in /var/home/alice",
            r"built in \\?\C:\Users\alice\x",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))

    def test_home_path_segments_inside_relative_repository_paths_pass(self):
        messages = [
            "Update nsh/screens/home/Home.tsx to clarify filing status",
            "Update nsh/screens/users/home/Home.tsx",
            "use src/home/ubuntu/project",
            "use src/Volumes/project",
            "use ~/Volumes/project",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))

    def test_allowed_home_names_keep_sentence_punctuation(self):
        messages = [
            "Validated on /home/ubuntu. The build passed.",
            "Validated on /Users/runner. The build passed.",
            r"Validated on C:\Users\runneradmin. The build passed.",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))

    def test_windows_profile_paths_with_spaces_are_refused(self):
        messages = [
            r"built in C:\Users\Carol Smith\project",
            r"built in c:\users\carol smith",
            r"built in 'C:\Users\Carol Smith\project'",
            r"built in 'C:\Users\runneradmin Smith'",
            r"built in C:\Users\runneradmin Smith\project",
        ]
        for message in messages:
            with self.subTest(message=message):
                self.assertTrue(has_absolute_local_path(message))

    def test_windows_short_names_of_allowed_ci_users_pass(self):
        for message in (
            r"built in C:\Users\RUNNER~1\AppData\Local\Temp",
            r"built in c:\users\runner~2\AppData\Local\Temp",
        ):
            with self.subTest(message=message):
                self.assertFalse(has_absolute_local_path(message))
        self.assertTrue(has_absolute_local_path(r"built in C:\Users\ALICE~1\AppData\Local\Temp"))


class CommitRangeTests(unittest.TestCase):
    def test_a_bad_commit_fails_with_its_id_the_offending_line_and_bypass_note(self):
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
            self.assertIn(f"offending line: built in {bad_path}", result.stderr)
            self.assertIn("reword", result.stderr.lower())
            self.assertIn("--no-verify skips the local hook", result.stderr)
            self.assertIn("pull-request check still applies", result.stderr)
            self.assertNotIn(clean, result.stderr)

    def make_signed_commit(self, repo: Path, hooks: Path, message: str) -> str:
        unsigned = commit(repo, hooks, message)
        raw = git_bytes(repo, hooks, "cat-file", "commit", unsigned).stdout
        headers, body = raw.split(b"\n\n", 1)
        signature = (
            b"gpgsig -----BEGIN PGP SIGNATURE-----\n"
            b" \n"
            b" fake-signature\n"
            b" -----END PGP SIGNATURE-----"
        )
        signed_object = headers + b"\n" + signature + b"\n\n" + body
        signed = git_bytes(
            repo,
            hooks,
            "hash-object",
            "-t",
            "commit",
            "-w",
            "--stdin",
            input_bytes=signed_object,
        ).stdout.decode("ascii").strip()
        git(repo, hooks, "update-ref", "refs/heads/topic", signed)
        return signed

    def test_a_signed_commit_is_scanned_when_git_shows_signatures(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, hooks = make_git_repo(Path(directory))
            base = commit(repo, hooks, "clean base")
            bad_path = "/Users/alice/project"
            signed = self.make_signed_commit(repo, hooks, f"built in {bad_path}")
            git(repo, hooks, "config", "log.showSignature", "true")
            raw_log = git(repo, hooks, "log", "-1", "--format=%H%x00%B%x00", signed).stdout
            if raw_log.split("\0")[0].strip() == signed:
                self.skipTest("git did not emit signature verification output")

            result = subprocess.run(
                [sys.executable, str(SCRIPT), "range", base, signed],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn(signed, result.stderr)
            self.assertIn(f"offending line: built in {bad_path}", result.stderr)
            self.assertIn("--no-verify skips the local hook", result.stderr)

    def test_merged_origin_main_commits_are_not_scanned_again(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, hooks = make_git_repo(Path(directory))
            base = commit(repo, hooks, "clean base")
            git(repo, hooks, "branch", "main", base)
            old_topic_tip = commit(repo, hooks, "topic work")
            git(repo, hooks, "checkout", "-q", "main")
            public_bad_path = "/Users/alice/project"
            public_main_tip = commit(repo, hooks, f"published in {public_bad_path}")
            git(repo, hooks, "update-ref", "refs/remotes/origin/main", public_main_tip)
            git(repo, hooks, "checkout", "-q", "topic")
            git(repo, hooks, "merge", "--no-edit", "main")
            head = git(repo, hooks, "rev-parse", "HEAD").stdout.strip()

            range_result = subprocess.run(
                [sys.executable, str(SCRIPT), "range", old_topic_tip, head],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )
            push_input = f"refs/heads/topic {head} refs/heads/topic {old_topic_tip}\n"
            push_result = subprocess.run(
                [sys.executable, str(SCRIPT), "pre-push"],
                cwd=repo,
                input=push_input,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(range_result.returncode, 0, range_result.stderr)
            self.assertEqual(push_result.returncode, 0, push_result.stderr)
            self.assertNotIn(public_bad_path, range_result.stderr)
            self.assertNotIn(public_bad_path, push_result.stderr)

    def test_pre_push_falls_back_when_the_remote_tip_object_is_missing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            empty_hooks = root / "empty-hooks"
            empty_hooks.mkdir()
            remote = root / "remote.git"
            one = root / "one"
            two = root / "two"

            def run_git(cwd: Path, *args: str) -> subprocess.CompletedProcess:
                return subprocess.run(
                    ["git", "-c", f"core.hooksPath={empty_hooks}", "-C", str(cwd), *args],
                    capture_output=True,
                    text=True,
                    check=True,
                )

            subprocess.run(["git", "init", "-q", "--bare", str(remote)], capture_output=True, text=True, check=True)
            subprocess.run(
                ["git", "--git-dir", str(remote), "symbolic-ref", "HEAD", "refs/heads/main"],
                capture_output=True,
                text=True,
                check=True,
            )
            subprocess.run(
                ["git", "-c", f"core.hooksPath={empty_hooks}", "clone", "--no-local", str(remote), str(one)],
                capture_output=True,
                text=True,
                check=True,
            )
            run_git(one, "symbolic-ref", "HEAD", "refs/heads/main")
            run_git(one, "config", "user.name", "Test User")
            run_git(one, "config", "user.email", "test@example.com")
            (one / "base.txt").write_text("base\n", encoding="utf-8")
            run_git(one, "add", "base.txt")
            run_git(one, "commit", "-q", "-m", "clean base")
            run_git(one, "push", "-q", "-u", "origin", "main")

            subprocess.run(
                ["git", "-c", f"core.hooksPath={empty_hooks}", "clone", "--no-local", str(remote), str(two)],
                capture_output=True,
                text=True,
                check=True,
            )
            run_git(two, "config", "user.name", "Test User")
            run_git(two, "config", "user.email", "test@example.com")
            run_git(two, "checkout", "-q", "-b", "topic")
            (two / "remote.txt").write_text("remote\n", encoding="utf-8")
            run_git(two, "add", "remote.txt")
            run_git(two, "commit", "-q", "-m", "remote edit made elsewhere")
            remote_tip = run_git(two, "rev-parse", "HEAD").stdout.strip()
            run_git(two, "push", "-q", "origin", "HEAD:refs/heads/topic")

            run_git(one, "checkout", "-q", "-b", "topic")
            (one / "local.txt").write_text("local\n", encoding="utf-8")
            run_git(one, "add", "local.txt")
            run_git(one, "commit", "-q", "-m", "clean local work")
            clean_tip = run_git(one, "rev-parse", "HEAD").stdout.strip()
            missing = subprocess.run(
                ["git", "-C", str(one), "cat-file", "-e", remote_tip],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(missing.returncode, 0, "the test requires the remote tip to be unfetched")

            clean_input = f"refs/heads/topic {clean_tip} refs/heads/topic {remote_tip}\n"
            clean_result = subprocess.run(
                [sys.executable, str(SCRIPT), "pre-push"],
                cwd=one,
                input=clean_input,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(clean_result.returncode, 0, clean_result.stderr)
            self.assertNotIn("bad object", clean_result.stderr)

            bad_path = "/Users/alice/project"
            run_git(one, "commit", "-q", "--allow-empty", "-m", f"built in {bad_path}")
            bad_tip = run_git(one, "rev-parse", "HEAD").stdout.strip()
            bad_input = f"refs/heads/topic {bad_tip} refs/heads/topic {remote_tip}\n"
            bad_result = subprocess.run(
                [sys.executable, str(SCRIPT), "pre-push"],
                cwd=one,
                input=bad_input,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(bad_result.returncode, 1, bad_result.stderr)
            self.assertIn(bad_tip, bad_result.stderr)
            self.assertIn(f"offending line: built in {bad_path}", bad_result.stderr)
            self.assertIn("--no-verify skips the local hook", bad_result.stderr)
            self.assertNotIn("bad object", bad_result.stderr)


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

    def make_verbose_commit_repo(self, root: Path, comment_char: str | None = None) -> tuple[Path, Path]:
        repo, empty_hooks = make_git_repo(root)
        (repo / "scripts").mkdir()
        shutil.copy(ROOT / "scripts" / "commit_message_paths.py", repo / "scripts")
        git(repo, empty_hooks, "config", "core.hooksPath", str(ROOT / ".githooks"))
        git(repo, empty_hooks, "config", "commit.cleanup", "scissors")
        git(repo, empty_hooks, "config", "commit.gpgsign", "false")
        if comment_char is not None:
            git(repo, empty_hooks, "config", "core.commentChar", comment_char)
        editor = root / "editor.py"
        editor.write_bytes(
            b"import sys\n"
            b"from pathlib import Path\n"
            b"path = Path(sys.argv[1])\n"
            b"path.write_bytes(b'subject\\n\\n' + path.read_bytes())\n"
        )
        return repo, editor

    def run_real_verbose_commit(self, repo: Path, editor: Path) -> subprocess.CompletedProcess:
        env = os.environ.copy()
        command = (Path(sys.executable), editor)
        env["GIT_EDITOR"] = " ".join(shlex.quote(path.as_posix()) for path in command)
        return subprocess.run(
            ["git", "-C", str(repo), "commit", "-v", "-q"],
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )

    def stage_six_files_with_a_local_path_in_the_diff(self, repo: Path) -> None:
        for index in range(6):
            text = "/Users/alice/project\n" if index == 0 else f"clean {index}\n"
            (repo / f"file-{index}.txt").write_text(text, encoding="utf-8")
        subprocess.run(["git", "-C", str(repo), "add", "."], capture_output=True, text=True, check=True)

    def test_real_git_commit_verbose_scissors_with_six_files_ignores_status_and_diff(self):
        if shutil.which("sh") is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, editor = self.make_verbose_commit_repo(Path(directory))
            self.stage_six_files_with_a_local_path_in_the_diff(repo)

            result = self.run_real_verbose_commit(repo, editor)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("absolute local path", result.stderr)

    def test_real_git_commit_verbose_scissors_reads_core_commentchar(self):
        if shutil.which("sh") is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, editor = self.make_verbose_commit_repo(Path(directory), comment_char=";")
            self.stage_six_files_with_a_local_path_in_the_diff(repo)

            result = self.run_real_verbose_commit(repo, editor)

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("absolute local path", result.stderr)

    def test_commit_msg_hook_refusal_prints_the_offending_line_and_bypass_note(self):
        bad_path = "/Users/alice/project"
        result = self.run_hook("commit-msg", f"built in {bad_path}\n")

        self.assertEqual(result.returncode, 1)
        lines = result.stderr.strip().splitlines()
        self.assertEqual(len(lines), 3, result.stderr)
        self.assertIn("absolute local path", lines[0])
        self.assertEqual(lines[1], f"offending line: built in {bad_path}")
        self.assertIn("reword", lines[2].lower())
        self.assertIn("--no-verify skips the local hook", lines[2])
        self.assertIn("pull-request check still applies", lines[2])

    def test_commit_msg_hook_accepts_placeholders(self):
        result = self.run_hook("commit-msg", "use /Users/<name>/project, $HOME/project or ~/project\n")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_commit_msg_hook_checks_comment_lines_git_may_keep(self):
        result = self.run_hook("commit-msg", "clean subject\n# a comment naming /Users/alice/project\n")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("absolute local path", result.stderr)

    def test_git_commit_m_refuses_a_comment_line_default_cleanup_keeps(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo, _ = make_git_repo(root)
            (repo / "scripts").mkdir()
            shutil.copy(ROOT / "scripts" / "commit_message_paths.py", repo / "scripts")
            bad_path = "/Users/alice/project"

            result = subprocess.run(
                [
                    "git",
                    "-c",
                    f"core.hooksPath={ROOT / '.githooks'}",
                    "-C",
                    str(repo),
                    "commit",
                    "-q",
                    "--allow-empty",
                    "-m",
                    "clean subject",
                    "-m",
                    f"# a comment naming {bad_path}",
                ],
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("absolute local path", result.stderr)
            self.assertIn(f"offending line: # a comment naming {bad_path}", result.stderr)
            self.assertIn("--no-verify skips the local hook", result.stderr)

    def test_commit_msg_hook_checks_text_after_a_lookalike_scissors_line_git_may_keep(self):
        lookalike = "# ----------------------- >8 ------------------------"
        result = self.run_hook(
            "commit-msg",
            f"clean subject\n{lookalike}\nordinary text naming /Users/alice/project\n",
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("absolute local path", result.stderr)

    def test_commit_msg_hook_checks_a_pasted_diff_after_a_lookalike_scissors_line(self):
        lookalike = "# ------------------------ >7 ------------------------"
        result = self.run_hook(
            "commit-msg",
            f"clean subject\n{lookalike}\ndiff --git a/x b/x\n+/Users/alice/project\n",
        )
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("absolute local path", result.stderr)

    def test_commit_msg_hook_ignores_the_generated_verbose_diff(self):
        scissors = "# ------------------------ >8 ------------------------"
        result = self.run_hook(
            "commit-msg",
            f"clean subject\n{scissors}\n"
            "# Do not modify or remove the line above.\n"
            "# Everything below it will be ignored.\n"
            "diff --git a/x b/x\n+/Users/alice/project\n",
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_commit_msg_hook_still_refuses_a_bad_path_before_the_verbose_diff(self):
        scissors = "# ------------------------ >8 ------------------------"
        result = self.run_hook(
            "commit-msg",
            f"built in /Users/alice/project\n{scissors}\nclean diff\n",
        )
        self.assertEqual(result.returncode, 1)
        self.assertIn("absolute local path", result.stderr)

    def test_message_file_outside_a_repository_accepts_any_punctuation_comment_char(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            message = root / "message"
            scissors = "; ------------------------ >8 ------------------------"
            message.write_text(
                f"clean subject\n{scissors}\ndiff --git a/x b/x\n+/Users/alice/project\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [sys.executable, str(SCRIPT), "message-file", str(message)],
                cwd=root,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)

    def test_message_file_inside_a_repository_uses_only_the_configured_comment_char(self):
        with tempfile.TemporaryDirectory() as directory:
            repo, _ = make_git_repo(Path(directory))
            message = repo / "message"
            scissors = "; ------------------------ >8 ------------------------"
            message.write_text(
                f"clean subject\n{scissors}\nordinary text naming /Users/alice/project\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [sys.executable, str(SCRIPT), "message-file", str(message)],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
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
            python_stub(stubs, "py", working=False)
            marker = Path(directory) / "python3-ran"
            write_executable(
                stubs / "python3",
                "#!/bin/sh\n"
                f"printf python3 >>{shlex.quote(str(marker))}\n"
                f"exec {shlex.quote(sys.executable)} \"$@\"\n",
            )
            env = path_with_stubs(stubs)

            clean = self.run_hook("commit-msg", "a plain message\n", env=env)
            refused = self.run_hook("commit-msg", "built in /Users/alice/project\n", env=env)
            python3_ran = marker.exists()

        self.assertEqual(clean.returncode, 0, clean.stderr)
        self.assertNotIn("SyntaxError", clean.stderr)
        self.assertEqual(refused.returncode, 1, refused.stderr)
        self.assertIn("absolute local path", refused.stderr)
        self.assertTrue(python3_ran)

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
            self.assertIn(f"offending line: built in {bad_path}", result.stderr)
            self.assertIn("--no-verify skips the local hook", result.stderr)


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
            global_hooks = root / "global-hooks" if os.name == "nt" else root / "global\\hooks"
            global_hooks.mkdir()
            sentinel = b"#!/bin/sh\nexit 0\n"
            (global_hooks / "commit-msg").write_bytes(sentinel)
            global_config = root / "global-config"
            subprocess.run(
                ["git", "config", "-f", str(global_config), "core.hooksPath", str(global_hooks)],
                capture_output=True,
                text=True,
                check=True,
            )
            configured = subprocess.run(
                ["git", "config", "-f", str(global_config), "--get", "core.hooksPath"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(configured.returncode, 0, configured.stderr)
            self.assertEqual(configured.stdout.strip(), str(global_hooks))
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

            def failing_git(args, **_kwargs):
                return subprocess.CompletedProcess(
                    args,
                    128,
                    stdout="",
                    stderr="fatal: detected dubious ownership in repository at '<repo>'",
                )

            output = io.StringIO()
            with mock.patch.object(install_git_hooks.subprocess, "run", side_effect=failing_git):
                with redirect_stdout(output):
                    status = install_git_hooks.main([str(repo)])

            self.assertEqual(status, 0)
            lines = output.getvalue().strip().splitlines()
            self.assertEqual(len(lines), 1, output.getvalue())
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
            if os.name != "nt":
                self.assertTrue(os.stat(hooks / "commit-msg").st_mode & stat.S_IXUSR)
            bad_path = "/Users/alice/project"
            message = Path(directory) / "message"
            message.write_text(f"built in {bad_path}\n", encoding="utf-8")

            result = subprocess.run(
                [sh, (hooks / "commit-msg").as_posix(), message.as_posix()],
                cwd=repo,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("absolute local path", result.stderr)
            self.assertIn(f"offending line: built in {bad_path}", result.stderr)
            self.assertIn("--no-verify skips the local hook", result.stderr)

    def test_the_installed_pre_push_chain_scans_and_preserves_the_local_hooks_input(self):
        sh = shutil.which("sh")
        if sh is None:
            self.skipTest("sh is not on PATH")
        with tempfile.TemporaryDirectory() as directory:
            repo, empty_hooks = self.make_real_checkout(Path(directory))
            hooks = repo / ".git" / "hooks"
            seen = hooks / "local-input"
            original = hooks / "pre-push"
            original.write_bytes(f"#!/bin/sh\ncat >{shlex.quote(seen.as_posix())}\n".encode("utf-8"))
            original.chmod(original.stat().st_mode | stat.S_IXUSR)
            install_hook(repo, hooks, "pre-push")
            install_hook(repo, hooks, "pre-push")
            if os.name != "nt":
                self.assertTrue(os.stat(hooks / "pre-push").st_mode & stat.S_IXUSR)

            base = commit(repo, empty_hooks, "clean base")
            head = commit(repo, empty_hooks, "use $HOME/project")
            git(repo, empty_hooks, "branch", "main", base)
            input_line = f"refs/heads/topic {head} refs/heads/topic {'0' * 40}\n"
            result = subprocess.run(
                [sh, (hooks / "pre-push").as_posix(), "origin", "unused"],
                cwd=repo,
                input=input_line,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(seen.read_text(encoding="utf-8"), input_line)


class RepositoryPolicyTests(unittest.TestCase):
    def test_the_hooks_accept_python_37_which_the_scanner_supports(self):
        for name in ("commit-msg", "pre-push"):
            hook = (ROOT / ".githooks" / name).read_text(encoding="utf-8")
            with self.subTest(hook=name):
                self.assertIn("sys.version_info < (3, 7)", hook)
                self.assertNotIn("sys.version_info < (3, 8)", hook)
        ast.parse(SCRIPT.read_text(encoding="utf-8"), feature_version=(3, 7))

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
        self.assertIn('python3 -I scripts/commit_message_paths.py range --public-log "$PR_BASE" "$PR_HEAD"', changes)

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
