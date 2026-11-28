"""scripts/ci_plan.py (what each CI run does), the workflows that run it, and the screen-test shard script."""

from __future__ import annotations

import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
NSH = (ROOT / ".github/workflows/nsh.yml").read_text(encoding="utf-8")

spec = importlib.util.spec_from_file_location("ci_plan", ROOT / "scripts/ci_plan.py")
ci_plan = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(ci_plan)


def job(workflow: str, name: str) -> str:
    match = re.search(rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)", workflow)
    if match is None:
        raise AssertionError(f"missing job {name}")
    return match.group(0)


def pr(*changed: str, **kw):
    return ci_plan.decide(event="pull_request", ref="refs/pull/1/merge", changed=list(changed), **kw)


def push_to(branch: str, *changed: str):
    return ci_plan.decide(event="push", ref=f"refs/heads/{branch}", branch=branch, changed=list(changed))


def manual(ref: str = "refs/heads/feat/x", **inputs):
    return ci_plan.decide(event="workflow_dispatch", ref=ref, **inputs)


# The condition ci.yml's entry jobs carry: a closed or draft pull request takes no runner, except that a labeled
# event reaches the planner (a ci:suite* label is a draft's explicit request; any other label plans nothing there).
NOT_CLOSED_OR_DRAFT = ("(github.event_name != 'pull_request' || (github.event.pull_request.state != 'closed' "
                       "&& github.event.action != 'closed' && (!github.event.pull_request.draft || "
                       "github.event.action == 'labeled')))")

NOTHING_HEAVY = dict(check_tests=False, installer=False, screens=False, build=False, nsh_rust=False, core_plain=False,
                     nsh_systems=[], nsh_agent_check=False)

NON_SUITE_LABEL = ("github.event_name == 'pull_request' && github.event.action == 'labeled' "
                   "&& github.event.label.name != 'ci:suite' && github.event.label.name != 'ci:suite-linux' "
                   "&& github.event.label.name != 'ci:suite-windows' && github.event.label.name != 'ci:suite-macos' "
                   "&& github.event.label.name != 'ci:suite-all'")
SUITE_LABEL = ("github.event_name == 'pull_request' && github.event.action == 'labeled' "
               "&& (github.event.label.name == 'ci:suite' || github.event.label.name == 'ci:suite-linux' "
               "|| github.event.label.name == 'ci:suite-windows' || github.event.label.name == 'ci:suite-macos' "
               "|| github.event.label.name == 'ci:suite-all')")
DRAFT_NOT_READY = ("github.event_name == 'pull_request' && github.event.pull_request.draft "
                   "&& github.event.action != 'labeled'")
CLOSED_PULL_REQUEST = "github.event_name == 'pull_request' && github.event.action == 'closed'"
CI_OK_LANE_NONE = "needs.changes.outputs.lane == 'none' && 'label-ignored' || "
CI_OK_CLOSED = CLOSED_PULL_REQUEST + " && 'closed' || "
CI_OK_NAME = ("    name: ${{ " + CI_OK_LANE_NONE + CI_OK_CLOSED + "(" + NON_SUITE_LABEL + ") && 'label-ignored' || ("
              + DRAFT_NOT_READY + ") && 'not-ready' || "
              "((github.event_name == 'pull_request' || (github.event_name == 'push' && "
              "!startsWith(github.ref, 'refs/heads/test/'))) && 'ci-ok' || 'tests-ok') }}\n")
CI_TESTS_OK_LANE = "needs.changes.outputs.lane != 'suite' && 'tests-not-requested' || "
NSH_TESTS_OK_LANE = "needs.plan.outputs.lane != 'suite' && 'tests-not-requested' || "
CI_TESTS_OK_NAME = "    name: ${{ " + CI_TESTS_OK_LANE + "(" + SUITE_LABEL + ") && 'tests-ok' || 'tests-not-requested' }}\n"
NSH_TESTS_OK_NAME = "    name: ${{ " + NSH_TESTS_OK_LANE + "(" + SUITE_LABEL + ") && 'tests-ok' || 'tests-not-requested' }}\n"


class AgentGatePolicyTests(unittest.TestCase):
    def test_every_agent_gated_nsh_rust_file_is_an_agent_compile_input(self):
        missing = []
        for base in ("nsh/src", "nsh/tests"):
            for path in sorted((ROOT / base).rglob("*.rs")):
                if 'feature = "agent"' not in path.read_text(encoding="utf-8"):
                    continue
                relative = path.relative_to(ROOT).as_posix()
                if not ci_plan.NSH_AGENT_CHECK_INPUTS.match(relative):
                    missing.append(relative)
        self.assertEqual(missing, [])


class QuickLaneTests(unittest.TestCase):
    def assert_quick(self, plan, *, check: bool, nsh_check: bool, scripts: bool = False, validation: bool = False,
                     nsh_agent_check: bool = False):
        self.assertEqual(plan["lane"], "quick")
        self.assertEqual(plan["check_systems"], ["linux"] if check else [])
        self.assertEqual(plan["nsh_check"], nsh_check)
        self.assertEqual(plan["nsh_agent_check"], nsh_agent_check)
        self.assertEqual(plan["scripts"], scripts)
        self.assertEqual(plan["validation_systems"], ["linux"] if validation else [])
        for name, value in NOTHING_HEAVY.items():
            if name != "nsh_agent_check":
                self.assertEqual(plan[name], value, name)

    def test_docs_only_runs_only_the_cheapest_jobs(self):
        self.assert_quick(pr("docs/plans/x.md", "README.md", "CONTRIBUTING.md", "docs/agent.md", "nsh/AGENTS.md",
                             "assets/brand/buwiz-forms.svg", "assets/badges/forms.svg", ".github/PULL_REQUEST.md"),
                          check=False, nsh_check=False)

    def test_the_readme_and_its_pictures_are_docs(self):
        # #494: README.md with assets/readme/showcase.png ran every quick check as app code.
        self.assert_quick(pr("README.md", "assets/readme/showcase.png", "assets/readme/showcase/README.md",
                             "assets/readme/showcase/iphone.jpg"), check=False, nsh_check=False)
        self.assertEqual(ci_plan.classify("assets/readme/showcase/iphone.jpg"), "docs")

    def test_app_change_runs_the_quick_checks_on_linux(self):
        self.assert_quick(pr("crates/bir-cli/src/main.rs"), check=True, nsh_check=False)

    def test_a_crate_the_app_is_built_on_also_runs_the_nsh_type_check(self):
        for path in ("crates/bir-core/src/lib.rs", "crates/bir-app-api/src/lib.rs", "Cargo.lock", "forms/1601C.json",
                     "justfile", "nsh/src/print.rs", "nsh/Cargo.toml"):
            with self.subTest(path=path):
                self.assert_quick(pr(path), check=True, nsh_check=True, nsh_agent_check=(path == "nsh/Cargo.toml"))

    def test_nsh_ui_and_screen_tests_run_only_the_nsh_type_check(self):
        for path in ("nsh/screens/home/Home.tsx", "nsh/screens/home/Home.test.tsx", "nsh/tests/agent.rs"):
            with self.subTest(path=path):
                self.assert_quick(pr(path), check=False, nsh_check=True, nsh_agent_check=(path == "nsh/tests/agent.rs"))

    def test_installer_and_packaging_run_only_the_script_tests(self):
        for path in ("packaging/windows/nsh.wxs", "assets/windows/x.png", "scripts/package_msix.ps1",
                     "packaging/linux/build-packages.sh", "assets/macos/x.png", "entitlements.plist",
                     ".github/workflows/release.yml", "scripts/setup-linux.sh", "scripts/tests/test_ci_plan.py"):
            with self.subTest(path=path):
                self.assert_quick(pr(path), check=False, nsh_check=False, scripts=True)

    def test_nsh_manifest_and_workflow_run_the_script_tests_and_the_type_check(self):
        for path in ("nsh/nsh.json", ".github/workflows/nsh.yml"):
            with self.subTest(path=path):
                self.assert_quick(pr(path), check=False, nsh_check=True, scripts=True)

    def test_look_alike_and_retired_paths_are_app_code(self):
        for path in ("installer.iss", "installer.iss.bak", "assets/windowsfoo/x.png", "packaging/linux2/x.sh",
                     "scripts/tests_helper.py", "scripts/setup-linux.sh.orig", ".github/workflows/cirelease.yml",
                     "crates/bir-shell/src/host.rs", "app/main.js", "assets/linux/dev.goldcoders.bir.desktop",
                     "scripts/linux_candidate_certification.md", "crates/bir-core/README.md", "assets/brandx/a.svg",
                     "assets/readmes/a.png", "assets/readme.png"):
            with self.subTest(path=path):
                self.assertEqual(pr(path)["check_systems"], ["linux"])

    def test_validation_audits_run_on_linux_only_for_their_paths(self):
        for path in ("rules/forms/x/gaps.md", "crates/bir-rules-codegen/src/lib.rs", ".github/workflows/ci.yml",
                     "scripts/ci_plan.py", "rust-toolchain.toml"):
            with self.subTest(path=path):
                self.assert_quick(pr(path), check=True, nsh_check=ci_plan.NSH_CHECK_INPUTS.match(path) is not None,
                                  validation=True)

    def test_validation_label_and_ebirforms_branch_run_them_on_linux(self):
        self.assert_quick(pr("docs/x.md", labels=("run-validation",)), check=False, nsh_check=False, validation=True)
        self.assert_quick(pr("docs/x.md", branch="ebirforms-sync"), check=False, nsh_check=False, validation=True)

    def test_the_validation_label_matches_case_insensitively_like_github_expressions(self):
        for label in ("Run-Validation", "RUN-VALIDATION", "run-validatıon"):
            with self.subTest(label=label):
                self.assert_quick(pr("docs/x.md", labels=(label,)), check=False, nsh_check=False, validation=True)
        self.assert_quick(pr("docs/x.md", labels=("run-validation-extra",)), check=False, nsh_check=False)

    def test_unknown_range_runs_every_quick_check_but_not_the_audits(self):
        for changed in (None, []):
            with self.subTest(changed=changed):
                plan = ci_plan.decide(event="pull_request", ref="refs/pull/1/merge", changed=changed)
                self.assert_quick(plan, check=True, nsh_check=True, nsh_agent_check=True)

    def test_agent_compile_check_follows_the_plug_pin_and_agent_only_files(self):
        for path in ("nsh/Cargo.toml", "nsh/Cargo.lock", "nsh/src/agent.rs", "nsh/tests/agent.rs",
                     "nsh/tests/agent_send.rs", "nsh/tests/window_plane.rs"):
            with self.subTest(path=path):
                self.assertTrue(pr(path)["nsh_agent_check"])
        for path in ("nsh/screens/home/Home.tsx", "nsh/tests/other.rs", "crates/bir-core/src/lib.rs"):
            with self.subTest(path=path):
                self.assertFalse(pr(path)["nsh_agent_check"])

    def test_agent_compile_check_is_a_pull_request_check_only(self):
        self.assertFalse(push_to("main", "nsh/Cargo.lock")["nsh_agent_check"])
        self.assertFalse(ci_plan.decide(event="schedule", ref="refs/heads/main")["nsh_agent_check"])

    def test_push_to_main_and_codex_run_the_quick_lane(self):
        for ref in ("refs/heads/main", "refs/heads/codex/x"):
            with self.subTest(ref=ref):
                plan = ci_plan.decide(event="push", ref=ref, changed=["crates/bir-core/src/lib.rs"])
                self.assert_quick(plan, check=True, nsh_check=True)
                self.assert_quick(ci_plan.decide(event="push", ref=ref, changed=["docs/x.md"]),
                                  check=False, nsh_check=False)


class ClosedAndDraftTests(unittest.TestCase):
    EVERYTHING = ("crates/bir-core/src/lib.rs", "rules/x.json", "nsh/screens/x.tsx", "packaging/windows/x",
                  "scripts/tests/test_ci_plan.py")

    def assert_nothing(self, plan):
        self.assertEqual(plan["lane"], "none")
        out = ci_plan.outputs(plan)
        for name in ("check", "nsh_check", "nsh_agent_check", "scripts", "installer", "validation", "nsh_macos", "nsh_linux",
                     "nsh_windows", "screens", "build", "nsh_rust", "core_plain", "linux_packages"):
            self.assertEqual(out[name], "false", name)
        self.assertEqual(json.loads(out["check_matrix"]), {"include": []})
        self.assertEqual(json.loads(out["validation_matrix"]), [])

    def test_a_closed_pull_request_plans_nothing(self):
        for draft in (False, True):
            with self.subTest(draft=draft):
                self.assert_nothing(pr(*self.EVERYTHING, action="closed", draft=draft, branch="ebirforms-sync",
                                       labels=("run-validation",)))
        self.assert_nothing(ci_plan.decide(event="pull_request", ref="refs/heads/main", action="closed", changed=None))

    def test_a_draft_plans_nothing(self):
        for action in ("opened", "synchronize", "reopened"):
            with self.subTest(action=action):
                self.assert_nothing(pr(*self.EVERYTHING, action=action, draft=True, labels=("run-validation",)))

    def test_ready_for_review_and_a_push_to_a_ready_pull_request_run_the_quick_lane(self):
        for action in ("ready_for_review", "opened", "synchronize", "reopened"):
            with self.subTest(action=action):
                plan = pr("crates/bir-core/src/lib.rs", action=action, draft=False)
                self.assertEqual(plan["lane"], "quick")
                self.assertEqual((plan["check_systems"], plan["nsh_check"]), (["linux"], True))

    def test_only_pull_requests_have_drafts(self):
        self.assertEqual(ci_plan.decide(event="push", ref="refs/heads/main", action="closed", draft=True,
                                        changed=["crates/bir-core/src/lib.rs"])["lane"], "quick")

    def test_main_reads_the_action_and_the_draft_flag(self):
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / "out"
            base = {**os.environ, "EVENT": "pull_request", "REF": "refs/pull/1/merge", "PR_NUMBER": "",
                    "PR_BASE": "", "PR_HEAD": "", "GITHUB_OUTPUT": str(out), "GITHUB_STEP_SUMMARY": ""}
            for extra, lane in ((dict(ACTION="closed", DRAFT="false"), "none"),
                                (dict(ACTION="synchronize", DRAFT="true"), "none"),
                                (dict(ACTION="ready_for_review", DRAFT="false"), "quick")):
                with self.subTest(**extra):
                    out.write_text("")
                    result = subprocess.run([sys.executable, "-I", str(ROOT / "scripts/ci_plan.py")],
                                            env={**base, **extra}, capture_output=True, text=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    values = dict(line.split("=", 1) for line in out.read_text().splitlines())
                    self.assertEqual(values["lane"], lane)


class SuiteLabelTests(unittest.TestCase):
    def labeled(self, label: str, *changed: str, **kw):
        return pr(*changed, action="labeled", label=label, **kw)

    def assert_nothing(self, plan):
        self.assertEqual(plan["lane"], "none")
        self.assertEqual((plan["check_systems"], plan["nsh_systems"], plan["validation_systems"]), ([], [], []))
        self.assertFalse(plan["check_tests"] or plan["nsh_check"] or plan["scripts"] or plan["installer"]
                         or plan["screens"] or plan["build"] or plan["nsh_rust"] or plan["core_plain"])

    def test_ci_suite_plans_the_quick_checks_and_the_linux_and_windows_suites(self):
        plan = self.labeled("ci:suite", "crates/bir-core/src/lib.rs")

        self.assertEqual(plan["lane"], "suite")
        self.assertEqual(plan["check_systems"], ["linux", "windows"])
        self.assertTrue(plan["check_tests"])
        self.assertEqual(plan["nsh_systems"], ["linux", "windows"])
        self.assertTrue(plan["nsh_check"])
        self.assertTrue(plan["screens"] and plan["build"] and plan["nsh_rust"] and plan["core_plain"])
        out = ci_plan.outputs(plan)
        self.assertEqual(out["chosen_systems"], "linux,windows")
        self.assertEqual(json.loads(out["check_matrix"])["include"], [
            {"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu", "tests": True},
            {"os": "windows-latest", "target": "x86_64-pc-windows-msvc", "tests": True},
        ])

    def test_one_system_and_all_system_labels_pick_their_systems(self):
        windows = self.labeled("ci:suite-windows", "nsh/screens/form/Form.tsx")
        self.assertEqual(windows["lane"], "suite")
        self.assertEqual(windows["check_systems"], [])
        self.assertEqual(windows["nsh_systems"], ["windows"])
        self.assertTrue(windows["nsh_check"])
        self.assertFalse(windows["core_plain"])

        every = self.labeled("ci:suite-all", "crates/bir-core/src/lib.rs")
        self.assertEqual(every["check_systems"], ["macos", "linux", "windows"])
        self.assertEqual(every["nsh_systems"], ["macos", "linux", "windows"])

    def test_ci_suite_linux_and_macos_pick_their_systems(self):
        self.assertEqual(self.labeled("ci:suite-linux", "crates/bir-core/src/lib.rs")["check_systems"], ["linux"])
        self.assertEqual(self.labeled("ci:suite-macos", "crates/bir-core/src/lib.rs")["check_systems"], ["macos"])

    def test_suite_labels_match_case_insensitively_like_github_expressions_and_log_their_spelling(self):
        for label, systems in (("CI:Suite", ["linux", "windows"]), ("ci:SUITE-windows", ["windows"]),
                               ("cı:suite", ["linux", "windows"])):
            with self.subTest(label=label):
                plan = self.labeled(label, "crates/bir-core/src/lib.rs")
                self.assertEqual(plan["lane"], "suite")
                self.assertEqual(plan["chosen_systems"], systems)
                self.assertEqual(plan["check_systems"], systems)
                self.assertEqual(plan["nsh_systems"], systems)
                self.assertIn(f"suite label: {label}", plan["reasons"])

    def test_the_dotless_i_suite_label_plans_the_suite_lane(self):
        plan = self.labeled("cı:suite", "crates/bir-core/src/lib.rs")

        self.assertEqual(plan["lane"], "suite")
        self.assertEqual(plan["chosen_systems"], ["linux", "windows"])

    def test_any_other_label_plans_nothing_even_when_it_starts_with_ci_suite(self):
        for label in ("bug", "ci:suiteful", ""):
            with self.subTest(label=label):
                self.assert_nothing(self.labeled(label, "crates/bir-core/src/lib.rs"))

    def test_a_push_to_a_pull_request_carrying_the_label_still_plans_only_the_quick_checks(self):
        plan = pr("crates/bir-core/src/lib.rs", action="synchronize", labels=("ci:suite",))

        self.assertEqual(plan["lane"], "quick")
        self.assertEqual(plan["check_systems"], ["linux"])
        self.assertFalse(plan["check_tests"])
        self.assertEqual(plan["nsh_systems"], [])

    def test_a_synchronize_can_plan_nothing_for_the_nsh_cancel_only_run(self):
        plan = pr("crates/bir-core/src/lib.rs", action="synchronize", labels=("ci:suite",), sync_lane="none")

        self.assert_nothing(plan)
        self.assertIn("synchronize", " ".join(plan["reasons"]))

    def test_a_suite_label_on_a_draft_is_an_explicit_request_and_runs(self):
        plan = self.labeled("ci:suite", "crates/bir-core/src/lib.rs", draft=True)

        self.assertEqual(plan["lane"], "suite")
        self.assertEqual(plan["nsh_systems"], ["linux", "windows"])

    def test_a_closed_pull_request_with_a_label_event_plans_nothing(self):
        self.assert_nothing(self.labeled("ci:suite", "crates/bir-core/src/lib.rs", state="closed"))


class TestLaneTests(unittest.TestCase):
    def test_the_branch_name_picks_the_systems(self):
        for branch, systems in (("test/linux/x", ["linux"]), ("test/windows/x", ["windows"]),
                                ("test/macos/x", ["macos"]), ("test/all/x", ["macos", "linux", "windows"]),
                                ("test/linux-windows/x", ["linux", "windows"]), ("test/ci-fast-lane", ["linux", "windows"])):
            with self.subTest(branch=branch):
                plan = push_to(branch, "crates/bir-core/src/lib.rs")
                self.assertEqual(plan["lane"], "tests")
                self.assertEqual(plan["check_systems"], systems)
                self.assertEqual(plan["nsh_systems"], systems)
                self.assertTrue(plan["check_tests"])

    def test_app_change_runs_every_suite_but_validation(self):
        plan = push_to("test/linux/x", "crates/bir-core/src/lib.rs")
        self.assertEqual((plan["screens"], plan["build"], plan["nsh_rust"], plan["core_plain"]), (True,) * 4)
        self.assertEqual(plan["validation_systems"], [])
        self.assertEqual((plan["shards"], plan["shard_count"]), ([0, 1, 2], 3))

    def test_only_screen_tests_run_only_the_screen_tests(self):
        plan = push_to("test/linux/x", "nsh/screens/form/Form.test.tsx", "nsh/lib/dates.test.ts", "docs/x.md")
        self.assertEqual(plan["check_systems"], [])
        self.assertEqual((plan["screens"], plan["build"], plan["nsh_rust"], plan["core_plain"]),
                         (True, False, False, False))
        self.assertEqual(plan["shards"], [0, 1, 2])

    def test_nsh_ui_runs_the_app_suites_but_not_the_workspace_or_core_tests(self):
        plan = push_to("test/linux/x", "nsh/screens/form/Form.tsx")
        self.assertEqual(plan["check_systems"], [])
        self.assertEqual((plan["screens"], plan["build"], plan["nsh_rust"], plan["core_plain"]), (True, True, True, False))

    def test_an_app_file_outside_the_nsh_inputs_runs_only_the_workspace_tests(self):
        plan = push_to("test/linux/x", "crates/bir-cli/src/main.rs")
        self.assertEqual(plan["check_systems"], ["linux"])
        self.assertEqual(plan["nsh_systems"], [])

    def test_installer_runs_only_where_windows_is_tested(self):
        self.assertTrue(push_to("test/windows/x", "packaging/windows/nsh.wxs")["installer"])
        self.assertTrue(push_to("test/x", "nsh/nsh.json")["installer"])
        self.assertFalse(push_to("test/linux/x", "packaging/windows/nsh.wxs")["installer"])
        self.assertEqual(push_to("test/windows/x", "packaging/windows/nsh.wxs")["check_systems"], [])

    def test_docs_only_runs_nothing(self):
        plan = push_to("test/all/x", "docs/x.md", "README.md")
        self.assertEqual(plan["check_systems"], [])
        self.assertEqual(plan["nsh_systems"], [])
        self.assertFalse(plan["installer"])

    def test_unknown_range_runs_every_suite_on_the_chosen_systems(self):
        plan = ci_plan.decide(event="push", ref="refs/heads/test/windows/x", changed=None)
        self.assertEqual(plan["check_systems"], ["windows"])
        self.assertEqual(plan["nsh_systems"], ["windows"])
        self.assertTrue(plan["installer"])
        self.assertFalse(plan["core_plain"])
        self.assertEqual(plan["validation_systems"], [])


class ManualAndNightlyTests(unittest.TestCase):
    def test_defaults_are_linux_and_windows_and_every_suite(self):
        plan = manual()
        self.assertEqual(plan["check_systems"], ["linux", "windows"])
        self.assertEqual(plan["validation_systems"], ["linux", "windows"])
        self.assertTrue(plan["installer"] and plan["screens"] and plan["build"] and plan["core_plain"])
        self.assertEqual((plan["shards"], plan["shard_count"]), ([0, 1, 2], 3))

    def test_suites_filter_and_shard(self):
        plan = manual(systems="macos", suites="screens", screens="screens/form/**", shard="1")
        self.assertEqual(plan["check_systems"], [])
        self.assertEqual(plan["nsh_systems"], ["macos"])
        self.assertEqual((plan["screens"], plan["build"], plan["nsh_rust"]), (True, False, False))
        self.assertEqual((plan["shards"], plan["shard_count"], plan["screens_filter"]), ([1], 3, "screens/form/**"))
        out = ci_plan.outputs(plan)
        self.assertEqual((out["shards"], out["shard_count"], out["lead_shard"]), ("[1]", "3", "1"))

    def test_without_screen_tests_one_runner_does_the_rest(self):
        plan = manual(systems="linux", suites="build,rust", shard="2")
        self.assertEqual((plan["shards"], plan["shard_count"]), ([0], 1))

    def test_installer_needs_windows_and_validation_follows_the_systems(self):
        self.assertFalse(manual(systems="linux", suites="installer")["installer"])
        self.assertEqual(manual(systems="all", suites="validation")["validation_systems"], ["macos", "linux", "windows"])
        self.assertEqual(manual(systems="all", suites="validation")["nsh_systems"], [])

    def test_main_ignores_narrowing_so_its_caches_stay_complete(self):
        plan = manual(ref="refs/heads/main", systems="all", suites="screens", screens="screens/x", shard="2")
        self.assertEqual((plan["screens"], plan["build"], plan["nsh_rust"], plan["installer"]), (True,) * 4)
        self.assertEqual((plan["shards"], plan["shard_count"], plan["screens_filter"]), ([0], 1, ""))
        self.assertEqual(plan["check_systems"], ["macos", "linux", "windows"])

    def test_bad_inputs_fail(self):
        for inputs in (dict(systems="beos"), dict(suites="lint"), dict(shard="3"), dict(systems=",")):
            with self.subTest(inputs=inputs), self.assertRaises(ci_plan.PlanError):
                manual(**inputs)

    def test_nightly_runs_everything_on_one_runner_per_system(self):
        plan = ci_plan.decide(event="schedule", ref="refs/heads/main")
        self.assertEqual(plan["check_systems"], ["macos", "linux", "windows"])
        self.assertEqual(plan["validation_systems"], ["macos", "linux", "windows"])
        self.assertEqual(plan["nsh_systems"], ["macos", "linux", "windows"])
        self.assertTrue(plan["installer"] and plan["screens"] and plan["build"] and plan["nsh_rust"] and plan["core_plain"])
        self.assertEqual((plan["shards"], plan["shard_count"]), ([0], 1))

    def test_linux_release_packages_only_nightly_or_when_asked_for(self):
        self.assertTrue(ci_plan.decide(event="schedule", ref="refs/heads/main")["linux_packages"])
        self.assertTrue(manual(systems="linux", suites="build")["linux_packages"])
        self.assertTrue(manual(ref="refs/heads/main", systems="all")["linux_packages"])
        self.assertFalse(manual(systems="linux", suites="screens")["linux_packages"])
        self.assertFalse(manual(systems="windows", suites="build")["linux_packages"])
        self.assertFalse(push_to("test/all/x", "crates/bir-core/src/lib.rs")["linux_packages"])
        self.assertFalse(pr("nsh/screens/x.tsx")["linux_packages"])

    def test_macos_runs_only_when_asked_for(self):
        everything = ["crates/bir-core/src/lib.rs", "rules/x.json", "nsh/screens/x.tsx", "packaging/windows/x"]
        asked = [ci_plan.decide(event="schedule", ref="refs/heads/main"), push_to("test/macos/x", *everything),
                 push_to("test/all/x", *everything), manual(systems="macos")]
        not_asked = [pr(*everything, labels=("run-validation",)), pr(), push_to("test/x", *everything),
                     push_to("test/linux/x", *everything), push_to("test/windows/x", *everything),
                     ci_plan.decide(event="push", ref="refs/heads/main", changed=None), manual()]
        for plan in asked:
            self.assertIn("macos", plan["check_systems"] + plan["nsh_systems"] + plan["validation_systems"])
        for plan in not_asked:
            self.assertNotIn("macos", plan["check_systems"] + plan["nsh_systems"] + plan["validation_systems"])


class OutputTests(unittest.TestCase):
    def test_check_matrix_keeps_each_host_triple_and_lane(self):
        out = ci_plan.outputs(ci_plan.decide(event="schedule", ref="refs/heads/main"))
        self.assertEqual(json.loads(out["check_matrix"]), {"include": [
            {"os": "macos-15", "target": "aarch64-apple-darwin", "tests": True},
            {"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu", "tests": True},
            {"os": "windows-latest", "target": "x86_64-pc-windows-msvc", "tests": True},
        ]})
        self.assertEqual(json.loads(out["validation_matrix"]), ["macos-15", "ubuntu-22.04", "windows-latest"])
        quick = ci_plan.outputs(pr("crates/bir-core/src/lib.rs"))
        self.assertEqual(json.loads(quick["check_matrix"])["include"],
                         [{"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu", "tests": False}])
        self.assertEqual((quick["check"], quick["nsh_check"], quick["nsh_agent_check"], quick["validation"]),
                         ("true", "true", "false", "false"))

    def test_main_writes_the_outputs_and_fails_on_bad_inputs(self):
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / "out"
            env = {**os.environ, "EVENT": "workflow_dispatch", "REF": "refs/heads/feat/x",
                   "IN_SYSTEMS": "linux", "IN_SUITES": "rust", "IN_SCREENS": "", "IN_SHARD": "",
                   "GITHUB_OUTPUT": str(out), "GITHUB_STEP_SUMMARY": ""}
            result = subprocess.run([sys.executable, "-I", str(ROOT / "scripts/ci_plan.py")], env=env,
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in out.read_text().splitlines())
            self.assertEqual((values["check"], values["nsh_linux"], values["core_plain"]), ("true", "true", "true"))
            env["IN_SUITES"] = "lint"
            result = subprocess.run([sys.executable, "-I", str(ROOT / "scripts/ci_plan.py")], env=env,
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 1)
            self.assertIn("::error::unknown suite", result.stdout)

    @unittest.skipIf(os.name == "nt", "the git stub is a shell script; the plan runs on Linux")
    def test_changed_files_reads_the_right_range(self):
        with tempfile.TemporaryDirectory() as temp:
            stub = Path(temp) / "git"
            stub.write_text('#!/bin/sh\necho "$3" >> "$GIT_LOG"\necho a.rs\n')
            stub.chmod(0o755)
            log = Path(temp) / "log"
            saved = dict(os.environ)
            os.environ.update(PATH=f"{temp}{os.pathsep}{saved['PATH']}", GIT_LOG=str(log))
            try:
                cases = [
                    (dict(EVENT="pull_request", PR_BASE="b", PR_HEAD="h"), "b...h"),
                    (dict(EVENT="push", REF="refs/heads/test/linux/x", PUSH_BEFORE="0" * 40, PUSH_AFTER="a"),
                     "origin/main...HEAD"),
                    (dict(EVENT="push", REF="refs/heads/main", PUSH_BEFORE="p", PUSH_AFTER="a"), "p..a"),
                ]
                for env, span in cases:
                    with self.subTest(span=span):
                        self.assertEqual(ci_plan.changed_files(env), ["a.rs"])
                        self.assertEqual(log.read_text().splitlines()[-1], span)
                self.assertIsNone(ci_plan.changed_files(dict(EVENT="push", REF="refs/heads/main",
                                                             PUSH_BEFORE="0" * 40, PUSH_AFTER="a")))
            finally:
                os.environ.clear()
                os.environ.update(saved)


class WorkflowTests(unittest.TestCase):
    def test_both_workflows_plan_with_the_same_script_and_concurrency(self):
        # A non-suite labeled event gets its own group (its run id) and cancels nothing: it must not cancel the pull
        # request's quick run. A ci:suite* labeled event stays in the pull request's group, so it replaces that quick
        # run with quick + suites, and a later push replaces it in turn. The main and test/** rules are unchanged.
        non_suite_label = ("github.event_name == 'pull_request' && github.event.action == 'labeled' "
                           "&& github.event.label.name != 'ci:suite' && github.event.label.name != 'ci:suite-linux' "
                           "&& github.event.label.name != 'ci:suite-windows' && github.event.label.name != 'ci:suite-macos' "
                           "&& github.event.label.name != 'ci:suite-all'")
        suite_label_or_other_event = ("github.event_name != 'pull_request' || github.event.action != 'labeled' "
                                      "|| github.event.label.name == 'ci:suite' || github.event.label.name == 'ci:suite-linux' "
                                      "|| github.event.label.name == 'ci:suite-windows' "
                                      "|| github.event.label.name == 'ci:suite-macos' "
                                      "|| github.event.label.name == 'ci:suite-all'")
        concurrency = (
            "  group: ${{ github.workflow }}-${{ " + non_suite_label + " && github.run_id || github.event.pull_request.number || "
            "(github.ref == 'refs/heads/main' && github.run_id) || github.ref }}\n"
            "  cancel-in-progress: ${{ (" + suite_label_or_other_event + ") && "
            "(github.event_name == 'pull_request' || github.ref != 'refs/heads/main') }}\n"
        )
        for workflow in (CI, NSH):
            self.assertIn("run: python3 -I scripts/ci_plan.py", workflow)
            self.assertIn(concurrency, workflow)
            self.assertIn('cron: "0 18 * * *"', workflow)
            for name in ("systems", "suites", "screens", "shard"):
                self.assertRegex(workflow, rf"(?m)^      {name}:\n        description:")

    def test_triggers(self):
        self.assertIn('    branches: [ "main", "codex/**", "test/**" ]\n  pull_request:\n', CI)
        # `closed` is there to cancel the pull request's runs; the plan and the jobs' `if` make it run nothing.
        # `labeled` is the suite request; ci_plan.py decides which label names count.
        self.assertIn("    types: [ opened, synchronize, reopened, ready_for_review, closed, labeled ]\n", CI)
        self.assertIn("        ACTION: ${{ github.event.action }}\n        LABEL: ${{ github.event.label.name }}\n",
                      job(CI, "changes"))
        self.assertIn('  push:\n    branches: [ "test/**" ]\n  pull_request:\n', NSH)
        self.assertIn('  pull_request:\n    branches: [ "main", "gol/**", "codex/**", "feat/**" ]\n', NSH)
        self.assertIn('    types: [ labeled, synchronize ]\n', NSH)
        self.assertNotIn("paths:", NSH)

    def test_nsh_synchronize_is_a_cancel_only_run_with_no_jobs(self):
        plan = job(NSH, "plan")
        expected_if = f"    if: ${{{{ {NOT_CLOSED_OR_DRAFT} && github.event.action != 'synchronize' }}}}\n"
        self.assertIn(expected_if, plan)
        self.assertIn("          PR_SYNC_LANE: none\n", plan)
        for name in ("nsh-macos", "nsh-linux", "nsh-windows", "core-plain-test", "linux-packages", "tests-ok"):
            self.assertIn("needs.plan.outputs", job(NSH, name), name)

    def test_non_suite_labels_never_create_checks_named_ci_ok_or_tests_ok(self):
        gate = job(CI, "ci-ok")
        self.assertIn(CI_OK_NAME, gate)
        self.assertLess(gate.index(CI_OK_LANE_NONE), gate.index(CI_OK_CLOSED))
        self.assertLess(gate.index(CI_OK_LANE_NONE), gate.index("&& 'not-ready'"))
        self.assertLess(gate.index(CI_OK_LANE_NONE), gate.index("&& 'ci-ok'"))

        ci_proof = job(CI, "tests-ok")
        self.assertIn(CI_TESTS_OK_NAME, ci_proof)
        self.assertLess(ci_proof.index(CI_TESTS_OK_LANE), ci_proof.index("&& 'tests-ok'"))
        nsh_proof = job(NSH, "tests-ok")
        self.assertIn(NSH_TESTS_OK_NAME, nsh_proof)
        self.assertLess(nsh_proof.index(NSH_TESTS_OK_LANE), nsh_proof.index("&& 'tests-ok'"))

    def test_a_drafts_empty_run_is_named_not_ready_until_a_suite_label_or_ready_for_review(self):
        gate = job(CI, "ci-ok")
        self.assertIn(CI_OK_NAME, gate)
        self.assertIn("github.event.pull_request.draft", DRAFT_NOT_READY)
        self.assertLess(gate.index("&& 'not-ready'"), gate.index("&& 'ci-ok'"))

    def test_a_closed_pull_requests_empty_run_is_named_closed(self):
        gate = job(CI, "ci-ok")
        self.assertIn(CI_OK_CLOSED, gate)
        self.assertLess(gate.index(CI_OK_CLOSED), gate.index("&& 'not-ready'"))
        self.assertLess(gate.index(CI_OK_CLOSED), gate.index("&& 'ci-ok'"))

    def test_workflow_label_comparisons_and_the_planner_follow_githubs_case_insensitive_equality(self):
        for workflow in (CI, NSH):
            compared = re.findall(r"github\.event\.label\.name\s*[!=]=\s*'([^']+)'", workflow)
            self.assertEqual({name.upper() for name in compared}, {name.upper() for name in ci_plan.SUITE_LABEL_SYSTEMS})
            for label in ("CI:Suite", "ci:SUITE-windows", "cı:suite"):
                self.assertTrue(any(label.upper() == name.upper() for name in compared), label)
        planner_source = (ROOT / "scripts/ci_plan.py").read_text(encoding="utf-8")
        self.assertNotIn(".casefold()", planner_source)
        self.assertIn("label.upper()", planner_source)
        self.assertIn("wanted.upper()", planner_source)
        for label, systems in (("CI:Suite", ["linux", "windows"]), ("ci:SUITE-windows", ["windows"]),
                               ("cı:suite", ["linux", "windows"])):
            with self.subTest(label=label):
                plan = pr("crates/bir-core/src/lib.rs", action="labeled", label=label)
                self.assertEqual(plan["lane"], "suite")
                self.assertEqual(plan["chosen_systems"], systems)

    def test_both_workflows_pass_the_label_event_to_the_planner_only_through_env(self):
        for workflow, job_name in ((CI, "changes"), (NSH, "plan")):
            with self.subTest(workflow=job_name):
                text = job(workflow, job_name)
                decide = text.split("      run: python3 -I scripts/ci_plan.py", 1)[0]
                self.assertIn("ACTION: ${{ github.event.action }}\n", decide)
                self.assertIn("LABEL: ${{ github.event.label.name }}\n", decide)
                self.assertIn("PR_STATE: ${{ github.event.pull_request.state }}\n", decide)
                self.assertIn("DRAFT: ${{ github.event.pull_request.draft }}\n", decide)
                self.assertIn("PR_BASE: ${{ github.event.pull_request.base.sha }}\n", decide)
                self.assertIn("PR_HEAD: ${{ github.event.pull_request.head.sha }}\n", decide)
                run_script = text.split("      run: python3 -I scripts/ci_plan.py", 1)[1]
                self.assertNotIn("github.event", run_script)

    def test_check_job_serves_both_lanes(self):
        check = job(CI, "check")
        header, steps = check.split("    steps:\n", maxsplit=1)
        self.assertRegex(header, r"(?m)^    needs: changes\s*$")
        self.assertRegex(header, r"(?m)^    if: needs\.changes\.outputs\.check == 'true'\s*$")
        self.assertIn("matrix: ${{ fromJSON(needs.changes.outputs.check_matrix) }}", header)
        self.assertNotIn("needs.changes.outputs", steps)
        for step in ("Test", "Page-versus-print audit (all 43 forms)"):
            self.assertRegex(steps, rf"(?m)^    - name: {re.escape(step)}\n      if: matrix\.tests\n")
        # Only a complete (test-lane) run on main writes the shared workspace cache.
        self.assertIn("save-if: ${{ github.ref == 'refs/heads/main' && matrix.tests }}", steps)
        # The quick lane keeps formatting, clippy, the direct-bir checks and the production boundary.
        for command in ("cargo fmt --all -- --check", "cargo clippy --locked --workspace -- -D warnings",
                        "cargo clippy --locked -p bir-core --features direct-bir -- -D warnings",
                        "cargo run -q --locked -p bir-rules-codegen -- status --boundaries-only"):
            run = steps.index(command)
            self.assertNotIn("matrix.tests", steps[steps.rindex("    - name:", 0, run):run])

    def test_ci_ok_is_the_single_gate(self):
        gate = job(CI, "ci-ok")
        # Not always(): in a run being cancelled (a newer push, or the pull request closing) it would run and report a
        # red ci-ok on the commit.
        self.assertIn(f"    if: ${{{{ !cancelled() && needs.changes.outputs.lane != 'none' && {NOT_CLOSED_OR_DRAFT} }}}}\n",
                      gate)
        needs = re.search(r"(?m)^    needs: \[(.*)\]\s*$", gate)
        assert needs is not None
        self.assertEqual([name.strip() for name in needs.group(1).split(",")],
                         ["changes", "check", "nsh-check", "nsh-agent-check", "windows-setup", "scripts", "validation-rules-v1",
                          "validation-rules-v2", "legacy-guard"])
        self.assertIn('[[ "$CHANGES" == success ]]', gate)
        self.assertIn(CI_OK_NAME, gate)
        for name in ("check", "nsh-check", "nsh-agent-check", "windows-setup", "scripts", "validation-rules-v1", "validation-rules-v2",
                     "legacy-guard"):
            self.assertIn(f"expect {name} ", gate)
        # Folded into ci-ok; nothing waits for a runner just to read two results.
        self.assertNotIn("\n  validation-rules:\n", CI)

    def test_tests_ok_gates_suite_label_runs_in_both_workflows(self):
        ci_gate = job(CI, "tests-ok")
        self.assertIn(CI_TESTS_OK_NAME, ci_gate)
        self.assertIn("always() && github.event_name == 'pull_request' && needs.changes.outputs.lane == 'suite'", ci_gate)
        self.assertIn("needs: [ changes, check, nsh-check, nsh-agent-check, windows-setup, scripts, validation-rules-v1, "
                      "validation-rules-v2, legacy-guard ]", ci_gate)
        for name in ("check", "nsh-check", "nsh-agent-check", "windows-setup", "scripts", "validation-rules-v1",
                     "validation-rules-v2", "legacy-guard"):
            self.assertIn(f"expect {name} ", ci_gate)

        self.assertIn("lane: ${{ steps.decide.outputs.lane }}", job(NSH, "plan"))
        nsh_gate = job(NSH, "tests-ok")
        self.assertIn(NSH_TESTS_OK_NAME, nsh_gate)
        self.assertIn("always() && github.event_name == 'pull_request' && needs.plan.outputs.lane == 'suite'", nsh_gate)
        self.assertIn("needs: [ plan, nsh-macos, nsh-linux, nsh-windows, core-plain-test, linux-packages ]", nsh_gate)
        for name in ("nsh-macos", "nsh-linux", "nsh-windows", "core-plain-test", "linux-packages"):
            self.assertIn(f"expect {name} ", nsh_gate)

    def test_every_heavy_job_waits_for_the_plan(self):
        for name, flag in (("windows-setup", "installer"), ("nsh-check", "nsh_check"),
                           ("nsh-agent-check", "nsh_agent_check"), ("scripts", "scripts"),
                           ("validation-rules-v1", "validation"), ("validation-rules-v2", "validation")):
            self.assertIn(f"    if: needs.changes.outputs.{flag} == 'true'\n", job(CI, name))
        for name, flag in (("nsh-macos", "nsh_macos"), ("nsh-linux", "nsh_linux"), ("nsh-windows", "nsh_windows"),
                           ("core-plain-test", "core_plain")):
            self.assertIn(f"    needs: plan\n    if: needs.plan.outputs.{flag} == 'true'\n", job(NSH, name))

    def test_a_closed_or_draft_pull_request_takes_no_runner(self):
        """Every job's `if` is false for a closed or draft pull request: it carries the condition itself, needs a job
        that does (with no status function in its `if`), runs only for a push or the schedule, or is the suite-label
        gate. That gate may run for a draft only when the planner has already decided the label is a suite label."""
        names = re.findall(r"(?m)^  ([a-z][a-z0-9-]*):\n", CI.split("\njobs:\n", 1)[1])
        self.assertIn("ci-ok", names)
        guarded: set[str] = set()
        for _ in names:
            for name in names:
                text = job(CI, name)
                condition = (re.search(r"(?m)^    if: (.*)$", text) or [None, ""])[1]
                needs = re.search(r"(?m)^    needs: (.*)$", text)
                needed = re.findall(r"[a-z][a-z0-9-]*", needs.group(1)) if needs else []
                if (NOT_CLOSED_OR_DRAFT in condition
                        or (needed and not re.search(r"\b(always|cancelled|failure)\(\)", condition)
                            and set(needed) <= guarded)
                        or "(github.event_name == 'schedule' || github.event_name == 'push')" in condition
                        or ("needs.changes.outputs.lane == 'suite'" in condition
                            and "github.event_name == 'pull_request'" in condition)):
                    guarded.add(name)
        self.assertEqual(sorted(set(names) - guarded), [])
        for name in ("changes", "legacy-guard", "ci-ok"):
            self.assertIn(NOT_CLOSED_OR_DRAFT, job(CI, name), name)

    def test_quick_and_test_jobs_build_without_incremental_data(self):
        self.assertIn('env:\n  CARGO_TERM_COLOR: always\n', CI)
        self.assertRegex(CI.split("\njobs:\n", 1)[0], r'(?m)^  CARGO_INCREMENTAL: "0"$')
        self.assertIn('      CARGO_INCREMENTAL: "0"\n      CARGO_PROFILE_DEV_DEBUG: "0"\n', job(CI, "nsh-check"))

    def test_branches_never_save_caches(self):
        for workflow in (CI, NSH):
            for save in re.finditer(r"(?ms)^ +- name: Save [^\n]*\n +if: ([^\n]*)\n", workflow):
                self.assertIn("github.ref == 'refs/heads/main'", save.group(1))
            self.assertNotIn("uses: actions/cache@v4", workflow)

    def test_linux_packages_are_called_without_secrets_or_a_tag(self):
        call = job(NSH, "linux-packages")
        self.assertIn("    needs: plan\n    if: needs.plan.outputs.linux_packages == 'true'\n", call)
        self.assertIn("uses: ./.github/workflows/release-linux.yml", call)
        self.assertIn('      tag: ""\n', call)
        self.assertNotIn("secrets", call)
        linux = (ROOT / ".github/workflows/release-linux.yml").read_text(encoding="utf-8")
        self.assertNotIn("contents: write", linux)
        self.assertNotIn("gh release", linux)

    def test_nsh_check_reads_the_pin_from_nsh_yml(self):
        self.assertEqual(CI.count("8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf"), 0)
        self.assertIn("NSH_PINNED_REF", job(CI, "nsh-check"))
        self.assertIn("run: just nsh-check -v", job(CI, "nsh-check"))

    def test_agent_compile_check_compiles_the_linux_plug_and_counts_in_ci_ok(self):
        agent = job(CI, "nsh-agent-check")
        self.assertIn("cargo test --locked --manifest-path nsh/Cargo.toml --features agent --no-run", agent)
        self.assertIn("sudo bash buwiz-forms/scripts/ci/apt_update.sh", agent)
        gate = job(CI, "ci-ok")
        self.assertIn("NSH_AGENT_CHECK: ${{ needs.nsh-agent-check.result }}", gate)
        self.assertIn("WANT_NSH_AGENT_CHECK: ${{ needs.changes.outputs.nsh_agent_check }}", gate)
        self.assertIn('expect nsh-agent-check "$NSH_AGENT_CHECK" "$WANT_NSH_AGENT_CHECK"', gate)

    def test_main_failures_open_an_issue_but_cancellations_do_not(self):
        report = job(CI, "main-report")
        self.assertRegex(report, r"(?m)^    needs: \[ ci-ok \]\s*$")
        self.assertIn("needs.ci-ok.result == 'failure'", report)
        self.assertIn("github.ref == 'refs/heads/main'", report)


@unittest.skipIf(os.name == "nt" or shutil.which("bash") is None, "the shard script runs on macOS and Linux")
class ScreenShardTests(unittest.TestCase):
    FILES = ["a.test.ts", "b.test.tsx", "lib/c.test.ts", "screens/form/d.test.tsx", "screens/form/e.test.tsx",
             "screens/form2/f.test.tsx", "screens/home/g.test.tsx"]
    IGNORED = ["verify/v.test.tsx", "node_modules/n.test.ts", ".nsh/h.test.ts", "target/t.test.ts", "lib/plain.ts"]

    def run_shard(self, shard: int, shards: int, screens_filter: str = "", files=None):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            app = root / "nsh"
            for name in (files if files is not None else self.FILES) + self.IGNORED:
                (app / name).parent.mkdir(parents=True, exist_ok=True)
                (app / name).write_text("")
            stub = root / "bin" / "just"
            stub.parent.mkdir()
            stub.write_text('#!/bin/sh\necho "$2" >> "$RAN"\n')
            stub.chmod(0o755)
            ran = root / "ran"
            ran.write_text("")
            env = dict(os.environ, PATH=f"{stub.parent}{os.pathsep}{os.environ['PATH']}", RAN=str(ran),
                       SHARD=str(shard), SHARDS=str(shards), SCREENS_FILTER=screens_filter)
            result = subprocess.run(["bash", str(ROOT / "scripts/nsh_screen_tests.sh")], cwd=app, env=env,
                                    capture_output=True, text=True, check=False)
            return result, [line.removeprefix("./") for line in ran.read_text().splitlines()]

    def test_without_a_filter_every_file_runs_exactly_once(self):
        for shards in (1, 2, 3):
            seen = []
            for shard in range(shards):
                result, ran = self.run_shard(shard, shards)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                seen += ran
            self.assertEqual(sorted(seen), sorted(self.FILES))
            self.assertEqual(len(seen), len(set(seen)))
        # Line k of the sorted list belongs to shard k % SHARDS.
        _, ran = self.run_shard(1, 3)
        self.assertEqual(ran, sorted(self.FILES)[1::3])

    def test_a_glob_or_a_folder_keeps_only_those_files(self):
        for screens_filter in ("screens/form/**", "screens/form/*", "screens/form", "screens/form/"):
            with self.subTest(filter=screens_filter):
                result, ran = self.run_shard(0, 1, screens_filter)
                self.assertEqual(result.returncode, 0, result.stdout)
                self.assertEqual(ran, ["screens/form/d.test.tsx", "screens/form/e.test.tsx"])
        _, ran = self.run_shard(0, 1, "a.test.ts")
        self.assertEqual(ran, ["a.test.ts"])
        seen = []
        for shard in range(3):
            result, ran = self.run_shard(shard, 3, "screens/**")
            self.assertEqual(result.returncode, 0, result.stdout)
            seen += ran
        self.assertEqual(sorted(seen), [name for name in sorted(self.FILES) if name.startswith("screens/")])

    def test_empty_results_fail_where_they_mean_a_mistake(self):
        result, _ = self.run_shard(0, 1, "screens/nothing/**")
        self.assertEqual(result.returncode, 1)
        self.assertIn("no test files match", result.stdout)
        # Two files over three shards: without a filter an empty shard is an error, with one it is not.
        result, _ = self.run_shard(2, 3, files=["a.test.ts", "b.test.ts"])
        self.assertEqual(result.returncode, 1)
        result, _ = self.run_shard(2, 3, "*.test.ts", files=["a.test.ts", "b.test.ts"])
        self.assertEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main()
