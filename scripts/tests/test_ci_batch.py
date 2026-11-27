"""scripts/ci_batch.py (several pull requests proven together), against throwaway local git repositories.

No network: "origin" is a bare repository in a temporary folder, its refs/pull/<n>/head refs are set by hand, and
GitHub is a fake that returns the JSON shapes `gh` returns.
"""

from __future__ import annotations

import importlib.util
import io
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]

spec = importlib.util.spec_from_file_location("ci_batch", ROOT / "scripts/ci_batch.py")
ci_batch = importlib.util.module_from_spec(spec)
assert spec.loader is not None
sys.modules["ci_batch"] = ci_batch  # dataclasses look their module up here
spec.loader.exec_module(ci_batch)


class FakeGitHub:
    repo = "hexuria/buwiz-forms"

    def __init__(self, origin: Path):
        self.origin = origin
        self.overrides: dict = {}
        self.run_list: list = []
        self.job_lists: dict = {}
        self.check_runs_by_sha: dict = {}
        self.check_run_reads: list = []

    def sha(self, ref: str) -> str:
        return subprocess.run(["git", "rev-parse", ref], cwd=self.origin, capture_output=True, text=True,
                              check=True).stdout.strip()

    def pull(self, number: int) -> dict:
        data = dict(number=number, headRefOid=self.sha(f"refs/pull/{number}/head"), baseRefName="main",
                    isDraft=False, state="OPEN", title=f"Change {number}")
        data.update(self.overrides.get(number, {}))
        return data

    def check_runs(self, sha: str):
        self.check_run_reads.append(sha)
        if sha in self.check_runs_by_sha:
            return self.check_runs_by_sha[sha]
        return [dict(name="ci-ok", head_sha=sha, status="completed", conclusion="success", id=1,
                     started_at="2026-11-27T10:00:00Z")]

    def main(self):
        return self.sha("refs/heads/main"), self.sha("refs/heads/main^{tree}")

    def runs(self, branch: str):
        return self.run_list

    def jobs(self, run_id: int):
        return self.job_lists.get(run_id, [])


class Repos:
    """origin (bare) and a clone to write commits in."""

    def __init__(self, temp: Path):
        self.origin, self.work = temp / "origin.git", temp / "work"
        git(temp, "init", "-q", "--bare", "-b", "main", str(self.origin))
        git(temp, "clone", "-q", str(self.origin), str(self.work))
        git(self.work, "symbolic-ref", "HEAD", "refs/heads/main")
        self.write({"a.txt": "one\ntwo\nthree\n", "b.txt": "b\n", "c.txt": "c\n"}, "start")
        git(self.work, "push", "-q", "origin", "main")

    def write(self, files: dict, message: str) -> str:
        for name, text in files.items():
            path = self.work / name
            if text is None:
                path.unlink()
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text, encoding="utf-8")
        git(self.work, "add", "-A")
        git(self.work, "commit", "-q", "-m", message)
        return git(self.work, "rev-parse", "HEAD")

    def pull_request(self, number: int, start: str, *commits: dict) -> str:
        """A branch from `start` with one commit per dict, published as refs/pull/<number>/head."""
        git(self.work, "checkout", "-q", "--detach", start)
        head = start
        for i, files in enumerate(commits):
            head = self.write(files, f"pr {number} step {i}")
        git(self.work, "push", "-q", "origin", f"+{head}:refs/pull/{number}/head")
        git(self.work, "checkout", "-q", "main")
        return head

    def advance_main(self, files: dict) -> str:
        git(self.work, "checkout", "-q", "main")
        sha = self.write(files, "main moves")
        git(self.work, "push", "-q", "origin", "main")
        return sha


def git(cwd: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()


class BatchTestCase(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.temp = Path(temp.name)
        # Nobody's own git settings (signing, hooks, default branch) reach these repositories.
        empty = self.temp / "gitconfig"
        empty.write_text("", encoding="utf-8")
        env = mock.patch.dict(os.environ, {
            "GIT_CONFIG_GLOBAL": str(empty), "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@example.invalid",
            "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@example.invalid",
        })
        env.start()
        self.addCleanup(env.stop)
        self.repos = Repos(self.temp)
        self.base = git(self.repos.work, "rev-parse", "main")
        self.github = FakeGitHub(self.repos.origin)
        self.git = ci_batch.Git(str(self.repos.work), "origin")
        self.lines: list = []

    def build(self, *numbers: int, system: str = "linux-windows"):
        return ci_batch.build(self.git, self.github, system, list(numbers), say=self.lines.append)

    def main(self, *args: str) -> tuple[int, str]:
        output = io.StringIO()
        cwd = os.getcwd()
        os.chdir(self.repos.work)
        try:
            with mock.patch.object(ci_batch, "GitHub", return_value=self.github), \
                    mock.patch.dict(os.environ, {"CI_TEST_REMOTE": "origin"}), redirect_stdout(output):
                code = ci_batch.main(list(args))
        finally:
            os.chdir(cwd)
        return code, output.getvalue()

    def github_squashes(self, *heads: str) -> str:
        """What merging each pull request with `gh pr merge --squash` in turn gives, by porcelain: `git merge
        --squash` takes the same merge base (the head's and the current main's), then one commit on main."""
        check = self.temp / "check"
        git(self.repos.work, "worktree", "add", "-q", "--detach", str(check), self.base)
        for head in heads:
            git(check, "merge", "-q", "--squash", head)
            git(check, "commit", "-q", "--allow-empty", "-m", f"squash {head}")
        tree = git(check, "rev-parse", "HEAD^{tree}")
        git(self.repos.work, "worktree", "remove", "--force", str(check))
        return tree


class BuildTests(BatchTestCase):
    def test_clean_batch_is_the_tree_the_squash_merges_give(self):
        # #1 edits a.txt in two commits; #2 started before main moved and edits another line of a.txt and adds a
        # file; main moved after both started.
        one = self.repos.pull_request(1, self.base, {"a.txt": "ONE\ntwo\nthree\n"}, {"d.txt": "d\n"})
        two = self.repos.pull_request(2, self.base, {"a.txt": "one\ntwo\nTHREE\n", "c.txt": None})
        self.base = self.repos.advance_main({"b.txt": "b moved\n"})

        branch, sha, record = self.build(1, 2)

        self.assertEqual(branch, "test/linux-windows/batch-1-2")
        self.assertEqual(record.main, self.base)
        self.assertEqual([(p.number, p.head) for p in record.pulls], [(1, one), (2, two)])
        self.assertEqual(record.tree, self.github_squashes(one, two))
        self.assertEqual(git(self.repos.work, "show", f"{sha}:a.txt"), "ONE\ntwo\nTHREE")
        # Pushed: the branch tip is the record commit, one squash per pull request on main, nothing else.
        self.assertEqual(git(self.repos.origin, "rev-parse", f"refs/heads/{branch}"), sha)
        self.assertEqual(git(self.repos.work, "rev-list", "--count", f"{self.base}..{sha}"), "3")
        self.assertEqual(git(self.repos.work, "rev-parse", f"{sha}~3"), self.base)
        self.assertEqual(git(self.repos.work, "rev-parse", f"{sha}~1^{{tree}}"), record.tree)
        message = git(self.repos.work, "log", "-1", "--format=%B", sha)
        self.assertIn(f"Batch-Main: {self.base}", message)
        self.assertIn(f"Batch-PR: 1 {one}\nBatch-PR: 2 {two}", message)
        self.assertIn(f"Batch-Tree: {record.tree}", message)
        # The checkout itself is untouched.
        self.assertEqual(git(self.repos.work, "status", "--porcelain"), "")
        self.assertEqual(git(self.repos.work, "rev-parse", "HEAD"), self.base)

    def test_green_ci_ok_at_the_head_passes_and_is_the_check_the_build_reads(self):
        head = self.repos.pull_request(14, self.base, {"x.txt": "x\n"})

        self.build(14)

        self.assertEqual(self.github.check_run_reads, [head])

    def assert_ci_ok_refused(self, number: int, check_runs: list, state: str):
        head = self.repos.pull_request(number, self.base, {f"n{number}.txt": "x\n"})
        self.github.check_runs_by_sha[head] = [dict(run, head_sha=head) for run in check_runs]

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(number)

        text = str(caught.exception)
        self.assertIn(f"#{number}", text)
        self.assertIn(head, text)
        expected = ("ci-ok is not reported at this head (a run still going, or none)" if state == "missing"
                    else f"ci-ok is {state}")
        self.assertIn(expected, text)
        self.assertIn("nothing was pushed", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_pending_ci_ok_refuses_and_pushes_nothing(self):
        self.assert_ci_ok_refused(15, [dict(name="ci-ok", status="in_progress", conclusion=None)], "pending")

    def test_failed_ci_ok_refuses_and_pushes_nothing(self):
        self.assert_ci_ok_refused(16, [dict(name="ci-ok", status="completed", conclusion="failure")], "failure")

    def test_cancelled_ci_ok_refuses_and_pushes_nothing(self):
        self.assert_ci_ok_refused(17, [dict(name="ci-ok", status="completed", conclusion="cancelled")], "cancelled")

    def test_skipped_ci_ok_refuses_and_pushes_nothing(self):
        self.assert_ci_ok_refused(18, [dict(name="ci-ok", status="completed", conclusion="skipped")], "skipped")

    def test_missing_ci_ok_refuses_and_pushes_nothing(self):
        self.assert_ci_ok_refused(19, [], "missing")

    def test_green_then_pending_at_one_head_is_refused_as_pending(self):
        self.assert_ci_ok_refused(23, [
            dict(name="ci-ok", status="completed", conclusion="success", id=1,
                 started_at="2026-11-27T10:00:00Z", completed_at="2026-11-27T10:05:00Z"),
            dict(name="ci-ok", status="queued", conclusion=None, id=2, started_at=None, completed_at=None),
        ], "pending")

    def test_red_then_green_at_one_head_passes_even_when_the_timestamps_invert(self):
        head = self.repos.pull_request(24, self.base, {"n24.txt": "x\n"})
        self.github.check_runs_by_sha[head] = [dict(run, head_sha=head) for run in [
            dict(name="ci-ok", status="completed", conclusion="failure", id=1,
                 started_at="2026-11-27T10:10:00Z", completed_at="2026-11-27T10:15:00Z"),
            dict(name="ci-ok", status="completed", conclusion="success", id=2,
                 started_at="2026-11-27T10:00:00Z", completed_at="2026-11-27T10:05:00Z"),
        ]]

        self.build(24)

        pushed = git(self.repos.origin, "for-each-ref", "--format=%(objectname)",
                     "refs/heads/test/linux-windows/batch-24")
        self.assertRegex(pushed, r"^[0-9a-f]{40}$")

    def test_green_then_cancelled_at_one_head_is_refused(self):
        self.assert_ci_ok_refused(25, [
            dict(name="ci-ok", status="completed", conclusion="success", id=1,
                 started_at="2026-11-27T10:00:00Z", completed_at="2026-11-27T10:05:00Z"),
            dict(name="ci-ok", status="completed", conclusion="cancelled", id=2, started_at=None,
                 completed_at="2026-11-27T10:10:00Z"),
        ], "cancelled")

    def test_green_ci_ok_at_an_older_commit_refuses(self):
        old = self.repos.pull_request(20, self.base, {"n20.txt": "old\n"})
        head = self.repos.pull_request(20, old, {"n20.txt": "new\n"})
        self.github.check_runs_by_sha[head] = [dict(name="ci-ok", head_sha=old, status="completed",
                                                    conclusion="success")]

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(20)

        text = str(caught.exception)
        self.assertIn("#20", text)
        self.assertIn(head, text)
        self.assertIn("ci-ok is not reported at this head (a run still going, or none)", text)
        self.assertIn("nothing was pushed", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_dry_run_prints_the_table_and_pushes_nothing(self):
        head = self.repos.pull_request(21, self.base, {"n21.txt": "x\n"})

        code, output = self.main("build", "--dry-run", "21")

        self.assertEqual(code, 0)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")
        self.assertEqual(git(self.repos.work, "for-each-ref", "refs/ci-batch"), "")
        self.assertIn(f"main     {self.base}", output)
        self.assertIn(f"#21      {head}  Change 21", output)
        self.assertIn("\ntree     ", "\n" + output)
        self.assertIn("dry-run: nothing was pushed", output)
        self.assertNotIn("https://github.com/hexuria/buwiz-forms/actions", output)

    def test_allow_unchecked_lets_one_through_and_prints_the_line(self):
        head = self.repos.pull_request(22, self.base, {"n22.txt": "x\n"})
        self.github.check_runs_by_sha[head] = [dict(name="ci-ok", head_sha=head, status="completed",
                                                    conclusion="failure")]

        code, output = self.main("build", "22", "--allow-unchecked", "22")

        branch = "test/linux-windows/batch-22"
        pushed = git(self.repos.origin, "for-each-ref", "--format=%(objectname)", f"refs/heads/{branch}")
        self.assertEqual(code, 0)
        self.assertRegex(pushed, r"^[0-9a-f]{40}$")
        self.assertIn(f"*** ALLOW-UNCHECKED: #22 at {head}; skipping the ci-ok gate ***", output)

    def test_allow_unchecked_outside_the_batch_warns_and_the_build_goes_on(self):
        self.repos.pull_request(26, self.base, {"n26.txt": "x\n"})

        code, output = self.main("build", "26", "--allow-unchecked", "27")

        branch = "test/linux-windows/batch-26"
        pushed = git(self.repos.origin, "for-each-ref", "--format=%(objectname)", f"refs/heads/{branch}")
        self.assertEqual(code, 0)
        self.assertRegex(pushed, r"^[0-9a-f]{40}$")
        self.assertIn("*** WARNING: --allow-unchecked #27 is not in the batch; the build goes on ***", output)

    def test_order_matters_and_is_kept(self):
        self.repos.pull_request(3, self.base, {"b.txt": "three\n"})
        self.repos.pull_request(4, self.base, {"c.txt": "four\n"})
        branch, _, record = self.build(4, 3, system="linux")
        self.assertEqual(branch, "test/linux/batch-4-3")
        self.assertEqual(record.numbers(), [4, 3])

    def test_conflict_names_the_pull_request_and_files_and_pushes_nothing(self):
        self.repos.pull_request(5, self.base, {"a.txt": "five\ntwo\nthree\n"})
        self.repos.pull_request(6, self.base, {"a.txt": "six\ntwo\nthree\n", "b.txt": "six\n"})
        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(5, 6)
        text = str(caught.exception)
        self.assertIn("#6 conflicts with main plus #5", text)
        self.assertIn("a.txt", text)
        self.assertNotIn("b.txt", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_refuses_a_pull_request_not_on_main_a_draft_or_a_closed_one_and_says_which(self):
        for number in (7, 8, 9, 10, 11):
            self.repos.pull_request(number, self.base, {f"n{number}.txt": "x\n"})
        self.github.overrides = {8: {"baseRefName": "feat/other"}, 9: {"isDraft": True}, 10: {"state": "CLOSED"},
                                 11: {"state": "MERGED"}}
        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(7, 8, 9, 10, 11)
        text = str(caught.exception)
        self.assertNotIn("#7", text)
        self.assertIn("#8 is based on 'feat/other', not main", text)
        self.assertIn("#9 is a draft", text)
        self.assertIn("#10 is closed", text)
        self.assertIn("#11 is merged", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_refuses_a_head_that_is_not_a_full_sha(self):
        self.repos.pull_request(12, self.base, {"x.txt": "x\n"})
        self.github.overrides = {12: {"headRefOid": self.github.sha("refs/pull/12/head")[:12]}}
        with self.assertRaisesRegex(ci_batch.BatchError, "no full head sha"):
            self.build(12)

    def test_refuses_when_github_and_the_remote_disagree(self):
        self.repos.pull_request(13, self.base, {"x.txt": "x\n"})
        self.github.overrides = {13: {"headRefOid": self.base}}
        with self.assertRaisesRegex(ci_batch.BatchError, "moved while reading"):
            self.build(13)

    def test_arguments(self):
        self.assertEqual(ci_batch.parse_build_args(["12", "#34"]), ("linux-windows", [12, 34], False, []))
        self.assertEqual(ci_batch.parse_build_args(["all", "12"]), ("all", [12], False, []))
        self.assertEqual(ci_batch.parse_build_args(["--dry-run", "all", "12", "34", "--allow-unchecked", "12",
                                                    "--allow-unchecked", "#34"]),
                         ("all", [12, 34], True, [12, 34]))
        for bad in ([], ["linux"], ["12", "12"], ["mac", "12"], ["12", "x"], ["12", "--allow-unchecked"],
                    ["12", "--allow-unchecked", "x"], ["12", "--what"]):
            with self.subTest(bad=bad), self.assertRaises(ci_batch.BatchError):
                ci_batch.parse_build_args(bad)

    def test_record_round_trips(self):
        record = ci_batch.Record("a" * 40, [ci_batch.Pull(1, "b" * 40), ci_batch.Pull(22, "c" * 40)], "d" * 40,
                                 "linux-windows")
        self.assertEqual(ci_batch.parse_record(ci_batch.format_record(record)), record)
        with self.assertRaises(ci_batch.BatchError):
            ci_batch.parse_record("just a commit\n")


class AfterTheBuildTests(BatchTestCase):
    def setUp(self):
        super().setUp()
        self.repos.pull_request(1, self.base, {"a.txt": "ONE\ntwo\nthree\n"})
        self.repos.pull_request(2, self.base, {"b.txt": "two\n"})
        self.repos.pull_request(3, self.base, {"c.txt": "three\n"})
        self.branch, self.sha, self.record = self.build(1, 2, 3)

    def runs(self, ci="success", nsh="success", status="completed", sha=None):
        sha = sha or self.sha
        self.github.run_list = [
            dict(databaseId=1, workflowName="CI", status=status, conclusion=ci if status == "completed" else "",
                 headSha=sha, url="u1", event="push", createdAt="2026-11-26T10:00:00Z"),
            dict(databaseId=2, workflowName="NSH app", status="completed", conclusion=nsh, headSha=sha, url="u2",
                 event="push", createdAt="2026-11-26T10:00:00Z"),
        ]

    def status(self) -> str:
        return ci_batch.status(self.git, self.github, self.branch, say=self.lines.append)

    def merge(self):
        return ci_batch.merge_commands(self.git, self.github, self.branch, say=self.lines.append)

    def test_status_green_running_and_red_with_the_failed_job_and_a_bisect(self):
        self.assertEqual(self.status(), "running")  # no runs yet
        self.runs(status="in_progress")
        self.assertEqual(self.status(), "running")
        self.runs()
        self.assertEqual(self.status(), "green")
        self.runs(sha="f" * 40)  # runs of an older push to the branch do not count
        self.assertEqual(self.status(), "running")
        self.runs(nsh="failure")
        self.github.job_lists = {2: [dict(name="screens (linux, 1)", conclusion="failure", url="j"),
                                     dict(name="build (linux)", conclusion="success", url="k")]}
        self.lines.clear()
        self.assertEqual(self.status(), "red")
        text = "\n".join(self.lines)
        self.assertIn("failed job: screens (linux, 1)", text)
        self.assertNotIn("build (linux)", text)
        self.assertIn("does not say which pull request", text)
        self.assertIn("just ci-batch linux-windows 1 2\n", text + "\n")
        self.assertIn("just ci-batch linux-windows 3\n", text + "\n")

    def test_merge_prints_the_commands_in_order_with_the_recorded_heads(self):
        self.runs()
        commands = self.merge()
        self.assertEqual(commands, [
            f"gh pr merge {p.number} --repo hexuria/buwiz-forms --squash --match-head-commit {p.head}"
            for p in self.record.pulls])
        self.assertEqual([c.split()[3] for c in commands], ["1", "2", "3"])

    def test_merge_refuses_unless_green(self):
        self.runs(ci="failure")
        with self.assertRaisesRegex(ci_batch.BatchError, "is red, not green"):
            self.merge()

    def test_merge_refuses_when_main_moved(self):
        self.runs()
        self.repos.advance_main({"e.txt": "e\n"})
        with self.assertRaisesRegex(ci_batch.BatchError, "main moved"):
            self.merge()

    def test_merge_refuses_when_a_head_moved_or_the_pull_request_changed(self):
        self.runs()
        self.repos.pull_request(2, self.base, {"b.txt": "two again\n"})
        with self.assertRaisesRegex(ci_batch.BatchError, "#2 moved"):
            self.merge()
        self.repos.pull_request(2, self.record.pulls[1].head)
        self.github.overrides = {3: {"isDraft": True}}
        with self.assertRaisesRegex(ci_batch.BatchError, "#3 is a draft"):
            self.merge()

    def test_the_record_survives_its_branch_being_deleted(self):
        git(self.repos.origin, "update-ref", "-d", f"refs/heads/{self.branch}")
        self.runs()
        self.assertEqual(len(self.merge()), 3)

    def test_verify_equal_after_the_squash_merges_and_unequal_otherwise(self):
        say = self.lines.append
        self.assertFalse(ci_batch.verify(self.git, self.github, self.branch, say=say))
        self.assertIn("does not cover main", "\n".join(self.lines))
        # The owner merges the three in order; main is then the batch's tree.
        git(self.repos.work, "checkout", "-q", "main")
        for pull in self.record.pulls:
            git(self.repos.work, "merge", "-q", "--squash", pull.head)
            git(self.repos.work, "commit", "-q", "-m", f"#{pull.number}")
        git(self.repos.work, "push", "-q", "origin", "main")
        self.lines.clear()
        self.assertTrue(ci_batch.verify(self.git, self.github, self.branch, say=say))
        self.assertTrue(self.lines[0].startswith("equal: "))
        # Something else merged on top: not covered.
        self.repos.advance_main({"z.txt": "z\n"})
        self.assertFalse(ci_batch.verify(self.git, self.github, self.branch, say=say))


class JustfileTests(unittest.TestCase):
    def test_recipes_call_the_script(self):
        justfile = (ROOT / "justfile").read_text(encoding="utf-8")
        for recipe, command in (("ci-batch *args", "build"), ("ci-batch-status branch", "status"),
                                ("ci-batch-merge branch", "merge"), ("ci-batch-verify branch", "verify")):
            with self.subTest(recipe=recipe):
                self.assertIn(f"\n{recipe}:\n    {{{{PYTHON}}}} scripts/ci_batch.py {command} ", justfile)


if __name__ == "__main__":
    unittest.main()
