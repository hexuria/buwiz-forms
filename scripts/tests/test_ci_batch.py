"""scripts/ci_batch.py (several pull requests proven together), against throwaway local git repositories.

No network: "origin" is a bare repository in a temporary folder, its refs/pull/<n>/head refs are set by hand, and
GitHub is a fake that returns the JSON shapes `gh` returns.
"""

from __future__ import annotations

import importlib.util
import io
import json
import os
import re
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

plan_spec = importlib.util.spec_from_file_location("ci_plan_for_batch_test", ROOT / "scripts/ci_plan.py")
ci_plan = importlib.util.module_from_spec(plan_spec)
assert plan_spec.loader is not None
plan_spec.loader.exec_module(ci_plan)


class FakeGitHub(ci_batch.GitHub):
    repo = "hexuria/buwiz-forms"

    def __init__(self, origin: Path):
        super().__init__(self.repo)
        self.origin = origin
        self.overrides: dict = {}
        self.run_list: list = []
        self.job_lists: dict = {}
        self.check_runs_by_sha: dict = {}
        self.check_run_reads: list = []
        self.workflow_runs_by_sha: dict = {}
        self.job_pages: dict = {}
        self.job_logs: dict = {}
        self.api_reads: list = []

    def sha(self, ref: str) -> str:
        return subprocess.run(["git", "rev-parse", ref], cwd=self.origin, capture_output=True, text=True,
                              check=True).stdout.strip()

    def _gh(self, *args: str) -> str:
        if len(args) >= 2 and args[0] == "api":
            endpoint = args[1]
            self.api_reads.append(endpoint)
            runs_prefix = f"repos/{self.repo}/actions/workflows/nsh.yml/runs?head_sha="
            if endpoint.startswith(runs_prefix) and endpoint.endswith("&per_page=100"):
                sha = endpoint[len(runs_prefix):-len("&per_page=100")]
                return json.dumps({"workflow_runs": self.workflow_runs_by_sha.get(sha, [])})
            jobs = re.fullmatch(rf"repos/{re.escape(self.repo)}/actions/runs/(\d+)/jobs\?per_page=100&page=(\d+)",
                                endpoint)
            if jobs:
                run_id, page = int(jobs.group(1)), int(jobs.group(2))
                pages = self.job_pages.get(run_id, [self.job_lists.get(run_id, [])])
                entries = pages[page - 1] if page <= len(pages) else []
                total = sum(len(part) for part in pages)
                return json.dumps({"total_count": total, "jobs": entries})
            log = re.fullmatch(rf"repos/{re.escape(self.repo)}/actions/jobs/(\d+)/logs", endpoint)
            if log:
                return self.job_logs[int(log.group(1))]
        raise AssertionError(f"the fake GitHub does not know: {' '.join(args)}")

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
    LABELS = {"macos": "macOS", "linux": "Linux", "windows": "Windows"}

    def suite_run(self, head: str, run_id: int, status: str = "completed", conclusion: str = "success",
                  name: str = "NSH app", event: str = "push", branch: str = "test/linux-windows/change",
                  path: str = ".github/workflows/nsh.yml"):
        return dict(id=run_id, name=name, head_sha=head, status=status, conclusion=conclusion, event=event,
                    head_branch=branch, path=path)

    def shard_job(self, run_id: int, system: str, shard: int, status: str = "completed",
                  conclusion: str = "success"):
        job_id = run_id * 1000 + shard * 10 + ("macos", "linux", "windows").index(system) + 1
        return dict(id=job_id, name=f"NSH app ({self.LABELS[system]}, shard {shard})", status=status,
                    conclusion=conclusion, steps=[])

    def green_jobs(self, run_id: int, systems=("linux", "windows"), shards=None):
        jobs = [dict(id=run_id * 1000 + 999, name="plan", status="completed", conclusion="success", steps=[])]
        wanted = list(range(ci_plan.SHARDS)) if shards is None else list(shards)
        for system in systems:
            jobs.extend(self.shard_job(run_id, system, shard) for shard in wanted)
        return jobs

    def branch_for(self, systems) -> str:
        chosen = tuple(systems)
        if set(chosen) == {"linux", "windows"}:
            return "test/linux-windows/change"
        if set(chosen) == {"macos", "linux", "windows"}:
            return "test/all/change"
        return f"test/{chosen[0]}/change"

    def add_suite_run(self, head: str, run_id: int, systems=("linux", "windows"), branch: str = None,
                      event: str = "push", status: str = "completed", conclusion: str = "success",
                      jobs=None, log: str = None, name: str = "NSH app",
                      path: str = ".github/workflows/nsh.yml"):
        branch = branch or self.branch_for(systems)
        run = self.suite_run(head, run_id, status=status, conclusion=conclusion, event=event, branch=branch,
                             name=name, path=path)
        self.github.workflow_runs_by_sha.setdefault(head, []).append(run)
        if jobs is None:
            jobs = self.green_jobs(run_id, systems) if status == "completed" and conclusion == "success" else []
        self.github.job_lists[run_id] = jobs
        if log is not None:
            self.github.job_logs[run_id * 1000 + 999] = log
        return run

    def manual_log(self, systems=("linux", "windows"), suites: str = "all", screens: str = "",
                   shard: str = "all") -> str:
        """The plan job's real log shape: its summary lines, without the workflow's inputs."""
        plan = ci_plan.decide(event="workflow_dispatch", ref="refs/heads/feat/change",
                              systems=",".join(systems), suites=suites, screens=screens, shard=shard)
        values = ci_plan.outputs(plan)
        lines = [f"lane: {plan['lane']}", *plan["reasons"]]
        lines += [f"{name}: {value}" for name, value in values.items()]
        return "\n".join("2026-11-28T00:00:00.0000000Z " + line for line in lines) + "\n"

    def test_workflow_runs_reads_the_nsh_workflow_file_runs_for_the_exact_head_sha(self):
        head = self.repos.pull_request(30, self.base, {"nsh/src/app.tsx": "app\n"})
        run = self.suite_run(head, 456)
        self.github.workflow_runs_by_sha[head] = [run]

        self.assertEqual(self.github.workflow_runs(head), [run])
        self.assertIn(f"repos/hexuria/buwiz-forms/actions/workflows/nsh.yml/runs?head_sha={head}&per_page=100",
                      self.github.api_reads)

    def test_suite_jobs_are_read_past_the_first_page(self):
        first = [dict(id=1000 + i, name=f"job {i}", status="completed", conclusion="success", steps=[])
                 for i in range(100)]
        second = [dict(id=2000, name="job 100", status="completed", conclusion="success", steps=[])]
        self.github.job_pages[777] = [first, second]

        jobs = self.github.suite_jobs(777)

        self.assertEqual(len(jobs), 101)
        self.assertEqual(jobs[-1]["name"], "job 100")
        self.assertIn("repos/hexuria/buwiz-forms/actions/runs/777/jobs?per_page=100&page=1", self.github.api_reads)
        self.assertIn("repos/hexuria/buwiz-forms/actions/runs/777/jobs?per_page=100&page=2", self.github.api_reads)

    def test_needs_suite_agrees_with_the_planner(self):
        cases = [
            (["forms/2556.json"], True),
            (["rust-toolchain.toml"], True),
            ([".github/workflows/nsh.yml"], True),
            (["scripts/ci_plan.py"], True),
            (["scripts/nsh_screen_tests.sh"], True),
            (["crates/bir-core/src/lib.rs"], True),
            (["crates/bir-app-api/src/lib.rs"], True),
            (["nsh/src/app.tsx"], True),
            (["nsh/screens/form.test.tsx"], True),
            (["justfile"], True),
            (["crates/bir-cli/src/main.rs"], False),
            (["crates/bir-mcp/src/main.rs"], False),
            (["nsh/README.md"], False),
            (["nsh/docs/guide.md"], False),
            (["scripts/ci_batch.py"], False),
            (["agent-recipes/smoke.json"], False),
            (["docs/CI.md"], False),
        ]
        for files, expected in cases:
            with self.subTest(files=files):
                plan = ci_plan.decide(event="push", ref="refs/heads/test/linux-windows/x", changed=list(files))
                self.assertEqual(bool(plan["nsh_systems"]), expected)
                self.assertIs(ci_batch.needs_suite(files), expected)

    def test_changed_files_come_from_the_git_diff_and_count_both_names_of_a_rename(self):
        content = "# just\n"
        base = self.repos.advance_main({"justfile": content})
        head = self.repos.pull_request(47, base, {"justfile": None, "x/justfile": content})

        names = self.git.changed_files(base, head)

        self.assertIn("x/justfile", names)
        self.assertIn("justfile", names)
        self.assertTrue(ci_batch.needs_suite(names))
        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(47)
        self.assertIn("missing", str(caught.exception))

    def test_suite_runs_are_chosen_by_workflow_file_not_display_name(self):
        head = self.repos.pull_request(52, self.base, {"nsh/src/app.tsx": "app\n"})
        wrong = self.suite_run(head, 450, name="NSH app", path=".github/workflows/ci.yml")
        right = self.suite_run(head, 451, name="Renamed workflow")
        self.github.workflow_runs_by_sha[head] = [wrong, right]
        self.github.job_lists[451] = self.green_jobs(451)

        code, output = self.main("build", "--dry-run", "52")

        self.assertEqual(code, 0)
        self.assertIn(f"#52      {head}  suite 451  Change 52", output)

    def test_suite_green_at_the_head_passes_and_the_dry_run_prints_the_run_id(self):
        head = self.repos.pull_request(35, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 456)

        code, output = self.main("build", "--dry-run", "35")

        self.assertEqual(code, 0)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")
        self.assertIn(f"#35      {head}  suite 456  Change 35", output)

    def test_suite_missing_refuses_with_the_ci_test_pr_command(self):
        head = self.repos.pull_request(36, self.base, {"nsh/src/app.tsx": "app\n"})

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(36)

        text = str(caught.exception)
        self.assertIn("refused by the NSH app suite gate", text)
        self.assertIn(f"#36 at {head}", text)
        self.assertIn("linux missing (no run)", text)
        self.assertIn("windows missing (no run)", text)
        self.assertIn("start it with: just ci-test-pr 36 linux-windows", text)
        self.assertIn("nothing was pushed", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_suite_newest_failed_while_an_older_one_passed_refuses(self):
        head = self.repos.pull_request(37, self.base, {"crates/bir-core/src/lib.rs": "core\n"})
        self.add_suite_run(head, 10)
        failed_jobs = self.green_jobs(11)
        failed_jobs[1]["conclusion"] = "failure"
        self.add_suite_run(head, 11, conclusion="failure", jobs=failed_jobs)

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(37)

        text = str(caught.exception)
        self.assertIn("#37", text)
        self.assertIn("failed", text)
        self.assertIn("run 11", text)
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_suite_in_progress_refuses_as_running(self):
        head = self.repos.pull_request(38, self.base, {"Cargo.toml": "[workspace]\n"})
        self.add_suite_run(head, 12, status="in_progress", conclusion=None, jobs=[])

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(38)

        self.assertIn("running", str(caught.exception))
        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

    def test_only_scripts_changed_passes_with_no_suite_run(self):
        head = self.repos.pull_request(39, self.base, {"scripts/ci_batch.py": "# batch\n"})

        code, output = self.main("build", "--dry-run", "39")

        self.assertEqual(code, 0)
        self.assertIn(f"#39      {head}  Change 39", output)
        self.assertNotIn("suite", output)
        self.assertFalse(any("actions/workflows/nsh.yml" in endpoint for endpoint in self.github.api_reads))

    def test_allow_unsuited_lets_one_through_prints_it_and_records_it(self):
        head = self.repos.pull_request(40, self.base, {"nsh/src/app.tsx": "app\n"})

        code, output = self.main("build", "40", "--allow-unsuited", "40")

        branch = "test/linux-windows/batch-40"
        pushed = git(self.repos.origin, "rev-parse", f"refs/heads/{branch}")
        self.assertEqual(code, 0)
        self.assertRegex(pushed, r"^[0-9a-f]{40}$")
        self.assertIn(f"*** ALLOW-UNSUITED: #40 at {head}; skipping the NSH app suite gate ***", output)
        message = git(self.repos.work, "log", "-1", "--format=%B", pushed)
        self.assertIn(f"Batch-Allow-Unsuited: 40 {head}", message)
        self.assertFalse(any("actions/workflows/nsh.yml" in endpoint for endpoint in self.github.api_reads))

    def test_allow_unsuited_outside_the_batch_warns_and_the_build_goes_on(self):
        self.repos.pull_request(41, self.base, {"scripts/ci_batch.py": "# batch\n"})

        code, output = self.main("build", "41", "--allow-unsuited", "42")

        self.assertEqual(code, 0)
        self.assertIn("*** WARNING: --allow-unsuited #42 is not in the batch; the build goes on ***", output)

    def test_suite_refusals_name_every_pull_request_in_one_message(self):
        missing = self.repos.pull_request(44, self.base, {"nsh/src/app.tsx": "app\n"})
        failed = self.repos.pull_request(45, self.base, {"justfile": "# just\n"})
        failed_jobs = self.green_jobs(99)
        failed_jobs[1]["conclusion"] = "failure"
        self.add_suite_run(failed, 99, conclusion="failure", jobs=failed_jobs)

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(44, 45)

        text = str(caught.exception)
        self.assertEqual(text.count("refused by the NSH app suite gate"), 1)
        self.assertIn(f"#44 at {missing}", text)
        self.assertIn(f"#45 at {failed}", text)
        self.assertIn("missing", text)
        self.assertIn("failed", text)
        self.assertIn("just ci-test-pr 44 linux-windows", text)
        self.assertIn("just ci-test-pr 45 linux-windows", text)

    def test_a_newer_other_system_run_does_not_hide_a_red_needed_system(self):
        head = self.repos.pull_request(60, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 10, systems=("linux",))
        windows_jobs = self.green_jobs(11, ("windows",))
        windows_jobs[1]["conclusion"] = "failure"
        self.add_suite_run(head, 11, systems=("windows",), conclusion="failure", jobs=windows_jobs)
        self.add_suite_run(head, 12, systems=("macos",))

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(60)

        text = str(caught.exception)
        self.assertIn("windows failed (run 11)", text)
        self.assertNotIn("linux failed", text)
        self.assertNotIn("macos failed", text)

    def test_a_newer_other_system_run_does_not_hide_a_missing_needed_system(self):
        head = self.repos.pull_request(61, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 20, systems=("linux",))
        self.add_suite_run(head, 21, systems=("macos",))

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(61)

        self.assertIn("windows missing (no run)", str(caught.exception))

    def test_a_success_with_every_app_job_skipped_is_missing(self):
        head = self.repos.pull_request(62, self.base, {"nsh/src/app.tsx": "app\n"})
        jobs = [dict(job, conclusion="skipped") for job in self.green_jobs(30)]
        self.add_suite_run(head, 30, jobs=jobs)

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(62)

        text = str(caught.exception)
        self.assertIn("linux missing (run 30)", text)
        self.assertIn("windows missing (run 30)", text)

    def test_a_partial_manual_run_is_refused_as_partial(self):
        head = self.repos.pull_request(63, self.base, {"nsh/src/app.tsx": "app\n"})
        log = self.manual_log(screens="screens/form/**")
        self.add_suite_run(head, 40, event="workflow_dispatch", branch="feat/change", log=log)

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(63)

        text = str(caught.exception)
        self.assertIn("linux partial (run 40)", text)
        self.assertIn("windows partial (run 40)", text)

    def test_a_full_manual_run_passes(self):
        head = self.repos.pull_request(64, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 41, event="workflow_dispatch", branch="feat/change", log=self.manual_log())

        code, output = self.main("build", "--dry-run", "64")

        self.assertEqual(code, 0)
        self.assertIn(f"#64      {head}  suite 41  Change 64", output)

    def test_two_runs_together_covering_both_systems_pass(self):
        head = self.repos.pull_request(65, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 42, systems=("linux",))
        self.add_suite_run(head, 43, systems=("windows",))
        self.add_suite_run(head, 44, systems=("macos",))

        code, output = self.main("build", "--dry-run", "65")

        self.assertEqual(code, 0)
        self.assertIn("suite 42,43", output)

    def test_a_missing_shard_is_missing(self):
        head = self.repos.pull_request(66, self.base, {"nsh/src/app.tsx": "app\n"})
        jobs = [job for job in self.green_jobs(50)
                if job["name"] != f"NSH app (Windows, shard {ci_plan.SHARDS - 1})"]
        self.add_suite_run(head, 50, jobs=jobs)

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(66)

        self.assertIn("windows missing (run 50)", str(caught.exception))

    def test_a_completed_run_without_the_systems_jobs_is_not_the_systems_newest_run(self):
        head = self.repos.pull_request(68, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 70)
        self.add_suite_run(head, 71, jobs=[])

        code, output = self.main("build", "--dry-run", "68")

        self.assertEqual(code, 0)
        self.assertIn("suite 70", output)
        self.assertNotIn("suite 70,71", output)

    def test_a_newer_running_run_names_the_older_green_one(self):
        head = self.repos.pull_request(67, self.base, {"nsh/src/app.tsx": "app\n"})
        self.add_suite_run(head, 60)
        self.add_suite_run(head, 61, systems=("windows",), status="in_progress", conclusion=None, jobs=[])

        with self.assertRaises(ci_batch.BatchError) as caught:
            self.build(67)

        self.assertIn("windows running (run 61, older green run 60)", str(caught.exception))

    def test_ci_test_pr_pushes_the_exact_github_head_and_prints_the_actions_url(self):
        head = self.repos.pull_request(70, self.base, {"x70.txt": "x\n"})

        ci_batch.test_pr(self.git, self.github, 70, say=self.lines.append)

        branch = "test/linux-windows/pr-70"
        self.assertEqual(git(self.repos.origin, "rev-parse", f"refs/heads/{branch}"), head)
        self.assertIn("https://github.com/hexuria/buwiz-forms/actions?query=branch%3Atest%2Flinux-windows%2Fpr-70",
                      "\n".join(self.lines))

    def test_ci_test_pr_refuses_when_the_fetched_sha_is_not_the_github_head(self):
        self.repos.pull_request(71, self.base, {"x71.txt": "x\n"})
        self.github.overrides[71] = {"headRefOid": self.base}

        with self.assertRaisesRegex(ci_batch.BatchError, "not the head GitHub reports"):
            ci_batch.test_pr(self.git, self.github, 71, say=self.lines.append)

        self.assertEqual(git(self.repos.origin, "for-each-ref", "refs/heads/test"), "")

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
        self.assertEqual(ci_batch.parse_build_args(["12", "#34"]), ("linux-windows", [12, 34], False, [], []))
        self.assertEqual(ci_batch.parse_build_args(["all", "12"]), ("all", [12], False, [], []))
        self.assertEqual(ci_batch.parse_build_args(["--dry-run", "all", "12", "34", "--allow-unchecked", "12",
                                                    "--allow-unchecked", "#34", "--allow-unsuited", "12",
                                                    "--allow-unsuited=#34"]),
                         ("all", [12, 34], True, [12, 34], [12, 34]))
        for bad in ([], ["linux"], ["12", "12"], ["mac", "12"], ["12", "x"], ["12", "--allow-unchecked"],
                    ["12", "--allow-unchecked", "x"], ["12", "--allow-unsuited"],
                    ["12", "--allow-unsuited", "x"], ["12", "--allow-unsuited="], ["12", "--what"]):
            with self.subTest(bad=bad), self.assertRaises(ci_batch.BatchError):
                ci_batch.parse_build_args(bad)

    def test_record_round_trips(self):
        record = ci_batch.Record("a" * 40, [ci_batch.Pull(1, "b" * 40), ci_batch.Pull(22, "c" * 40)], "d" * 40,
                                 "linux-windows", allow_unsuited=[22])
        self.assertIn(f"Batch-Allow-Unsuited: 22 {'c' * 40}", ci_batch.format_record(record))
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
                                ("ci-batch-merge branch", "merge"), ("ci-batch-verify branch", "verify"),
                                ("ci-test-pr number system=\"\"", "test-pr")):
            with self.subTest(recipe=recipe):
                self.assertIn(f"\n{recipe}:\n    {{{{PYTHON}}}} scripts/ci_batch.py {command} ", justfile)


class BatchDocsTests(unittest.TestCase):
    def test_the_batch_docs_list_both_gates_and_both_escape_hatches(self):
        docs = (ROOT / "docs/CI.md").read_text(encoding="utf-8")
        self.assertIn("### What a pull request needs before a batch", docs)
        section = docs.split("### What a pull request needs before a batch", 1)[1].split("\n## ", 1)[0]
        for wording in ("ci-ok", "NSH app", "just ci-test-pr", "--allow-unchecked", "--allow-unsuited",
                        "per system", "every shard"):
            self.assertIn(wording, section)


if __name__ == "__main__":
    unittest.main()
