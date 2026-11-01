from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"


def extract_decide_script(workflow: str) -> str:
    lines = workflow.splitlines()
    step_id = next(
        (index for index, line in enumerate(lines) if line.strip() == "id: decide"),
        None,
    )
    if step_id is None:
        raise ValueError("ci.yml is missing the Decide step id")
    run_line = next(
        (
            index
            for index in range(step_id + 1, len(lines))
            if lines[index].strip() == "run: |"
        ),
        None,
    )
    if run_line is None:
        raise ValueError("the Decide step is missing its run block")

    body: list[str] = []
    for line in lines[run_line + 1 :]:
        if line.strip() and len(line) - len(line.lstrip()) < 8:
            break
        body.append(line[8:] if line.startswith("        ") else "")
    return "\n".join(body).strip("\n") + "\n"


def extract_job(workflow: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
        workflow,
    )
    if match is None:
        raise ValueError(f"ci.yml is missing the {name} job")
    return match.group(0)


class CiChangeDetectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.decide_script = extract_decide_script(cls.workflow).replace(
            "\r\n",
            "\n",
        )

    def run_decide(
        self,
        *,
        event: str = "pull_request",
        changed: tuple[str, ...] = (),
        git_fails: bool = False,
        labels: str = "",
        force: str = "false",
        pr_base: str = "base",
        pr_head: str = "head",
        push_before: str = "before",
        push_after: str = "after",
    ) -> tuple[dict[str, str], str]:
        # The Decide step only ever runs in the `changes` job on ubuntu-22.04.
        # Git Bash on Windows does not resolve the stub `git`/`gh` below ahead
        # of the real ones, so a Windows run would test a different shell, not
        # the script CI executes.
        if os.name == "nt":
            self.skipTest("the Decide step runs on Linux only")
        bash = shutil.which("bash")
        if bash is None:
            self.skipTest("bash is not available")

        with tempfile.TemporaryDirectory(prefix="ci-change-detection-") as temp:
            root = Path(temp)
            stub_bin = root / "bin"
            stub_bin.mkdir()
            git_stub = stub_bin / "git"
            git_stub.write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"${1-}\" != diff || \"${2-}\" != --name-only ]]; then\n"
                "  printf 'unexpected git arguments: %s\\n' \"$*\" >&2\n"
                "  exit 2\n"
                "fi\n"
                "if [[ \"${STUB_GIT_FAIL:-false}\" == true ]]; then exit 1; fi\n"
                "printf '%s\\n' \"${STUB_CHANGED:-}\"\n",
                encoding="utf-8",
                newline="\n",
            )
            gh_stub = stub_bin / "gh"
            gh_stub.write_text(
                "#!/usr/bin/env bash\n"
                "printf '%s\\n' \"${STUB_LABELS:-}\"\n",
                encoding="utf-8",
                newline="\n",
            )
            os.chmod(git_stub, 0o755)
            os.chmod(gh_stub, 0o755)

            output_path = root / "github-output"
            summary_path = root / "github-step-summary"
            output_path.write_text("", encoding="utf-8", newline="\n")
            summary_path.write_text("", encoding="utf-8", newline="\n")
            environment = os.environ.copy()
            environment.update(
                {
                    "EVENT": event,
                    "BRANCH": "feature",
                    "FORCE": force,
                    "PR_NUMBER": "test-pr",
                    "GH_TOKEN": "",
                    "GH_REPO": "test/repo",
                    "PR_BASE": pr_base,
                    "PR_HEAD": pr_head,
                    "PUSH_BEFORE": push_before,
                    "PUSH_AFTER": push_after,
                    "GITHUB_OUTPUT": output_path.as_posix(),
                    "GITHUB_STEP_SUMMARY": summary_path.as_posix(),
                    "STUB_CHANGED": "\n".join(changed),
                    "STUB_GIT_FAIL": "true" if git_fails else "false",
                    "STUB_LABELS": labels,
                    "PATH": str(stub_bin) + os.pathsep + os.environ.get("PATH", ""),
                },
            )
            result = subprocess.run(
                [bash, "--noprofile", "--norc", "-c", self.decide_script],
                cwd=root,
                env=environment,
                capture_output=True,
                check=False,
                text=True,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"Decide script failed:\n{result.stdout}\n{result.stderr}",
            )
            outputs = dict(
                line.split("=", maxsplit=1)
                for line in output_path.read_text(encoding="utf-8").splitlines()
            )
            return outputs, summary_path.read_text(encoding="utf-8")

    ALL_SYSTEMS = ("macos-14", "ubuntu-22.04", "windows-latest")

    def assert_decisions(
        self,
        *,
        systems: tuple[str, ...],
        check_reason: str,
        validation: str = "false",
        validation_reason: str = "no validation-rules path changed",
        scripts: str = "false",
        app: str | None = None,
        shell: str = "false",
        packaging: tuple[str, ...] = (),
        event: str = "pull_request",
        changed: tuple[str, ...] = (),
        git_fails: bool = False,
        labels: str = "",
        force: str = "false",
        pr_base: str = "base",
        pr_head: str = "head",
        push_before: str = "before",
        push_after: str = "after",
    ) -> None:
        outputs, summary = self.run_decide(
            event=event,
            changed=changed,
            git_fails=git_fails,
            labels=labels,
            force=force,
            pr_base=pr_base,
            pr_head=pr_head,
            push_before=push_before,
            push_after=push_after,
        )
        matrix = json.loads(outputs.pop("check_matrix"))
        self.assertEqual(tuple(entry["os"] for entry in matrix["include"]), systems)
        shell_matrix = json.loads(outputs.pop("shell_matrix"))
        self.assertEqual(
            tuple(entry["os"] for entry in shell_matrix["include"]),
            self.ALL_SYSTEMS if shell == "true" else (),
        )
        if app is None:
            app = "true" if systems == self.ALL_SYSTEMS else "false"
        self.assertEqual(
            outputs,
            {
                "validation": validation,
                "check": "true" if systems else "false",
                "app": app,
                "scripts": scripts,
                "shell": shell,
                "linux_pkg": "true" if "linux" in packaging else "false",
                "windows_pkg": "true" if "windows" in packaging else "false",
                "macos_pkg": "true" if "macos" in packaging else "false",
            },
        )
        self.assertIn(
            f"validation-rules audits: {validation} ({validation_reason})",
            summary,
        )
        self.assertIn(
            f"app check on: {', '.join(systems) or 'none'} ({check_reason})",
            summary,
        )

    def test_docs_only_pull_request_tests_nothing(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="docs-only changes",
            changed=(
                "docs/plans/x.md",
                "README.md",
                "CONTRIBUTING.md",
                "crates/bir-shell/docs/AGENT.md",
            ),
        )

    def test_docs_only_push_tests_nothing(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="docs-only changes",
            event="push",
            changed=("docs/plans/x.md", "README.md"),
        )

    def test_app_change_tests_all_three_systems(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: crates/bir-core/src/lib.rs",
            changed=("docs/plans/x.md", "crates/bir-core/src/lib.rs"),
        )

    def test_shell_workspace_change_runs_shell_check_not_check(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="shell workspace changed (covered by shell-check)",
            app="false",
            shell="true",
            changed=("crates/bir-shell/src/host.rs", "app/main.js"),
        )

    def test_shell_and_app_changes_run_both(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: crates/bir-core/src/lib.rs",
            shell="true",
            changed=("crates/bir-core/src/lib.rs", "app/main.js"),
        )

    def test_rules_markdown_is_app_code_and_validation_input(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: rules/forms/x/gaps.md",
            validation="true",
            validation_reason="validation-rules paths changed",
            changed=("rules/forms/x/gaps.md",),
        )

    def test_script_markdown_is_app_code(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: scripts/linux_candidate_certification.md",
            changed=("scripts/linux_candidate_certification.md",),
        )

    def test_core_manifest_is_app_code(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: crates/bir-core/Cargo.toml",
            changed=("crates/bir-core/Cargo.toml",),
        )

    def test_ci_workflow_change_tests_everything_and_audits(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: .github/workflows/ci.yml",
            validation="true",
            validation_reason="validation-rules paths changed",
            changed=(".github/workflows/ci.yml",),
        )

    def test_windows_installer_change_tests_windows_only(self) -> None:
        self.assert_decisions(
            systems=("windows-latest",),
            check_reason="Windows installer files changed",
            packaging=("windows",),
            changed=(
                "installer.iss",
                "installer.wxs",
                "assets/windows/Square44x44Logo.png",
                "scripts/audit_final_setup.ps1",
                "scripts/audit_installer_sources.py",
                "scripts/package_msix.ps1",
                "docs/platforms/windows.md",
            ),
        )

    def test_windows_installer_with_app_code_tests_all_three(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: crates/bir-core/src/lib.rs",
            app="true",
            packaging=("windows",),
            changed=("installer.iss", "crates/bir-core/src/lib.rs"),
        )

    def test_linux_packaging_runs_only_the_script_tests(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="no app code changed",
            scripts="true",
            packaging=("linux",),
            changed=("packaging/linux/build-packages.sh", "docs/platforms/linux.md"),
        )

    def test_macos_packaging_runs_only_the_script_tests(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="no app code changed",
            scripts="true",
            packaging=("macos",),
            changed=(
                "entitlements.plist",
                "daemon.entitlements.plist",
                "assets/macos/AppIcon.iconset/icon_16x16.png",
                "scripts/dev_bundle_macos.sh",
                "scripts/package_macos_release.sh",
                "scripts/setup_macos_release_secrets.sh",
            ),
        )

    def test_linux_desktop_entry_is_app_code(self) -> None:
        # bir-shell's tests read assets/linux/, so it is not packaging-only.
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="app file changed: assets/linux/dev.goldcoders.bir.desktop",
            changed=("assets/linux/dev.goldcoders.bir.desktop",),
        )

    def test_release_workflows_setup_scripts_and_tests_run_only_the_script_tests(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="no app code changed",
            scripts="true",
            changed=(
                ".github/workflows/release.yml",
                ".github/workflows/release-linux.yml",
                ".github/workflows/bir.yml",
                ".github/workflows/html-candidate-certification.yml",
                "scripts/setup-linux.sh",
                "scripts/setup-windows.ps1",
                "scripts/tests/test_release_workflow_policy.py",
            ),
        )

    def test_windows_and_linux_packaging_together_test_windows_only(self) -> None:
        self.assert_decisions(
            systems=("windows-latest",),
            check_reason="Windows installer files changed",
            packaging=("linux", "windows"),
            changed=("packaging/linux/smoke-test.sh", "installer.iss"),
        )

    def test_look_alike_paths_are_app_code(self) -> None:
        for path in (
            "installer.iss.bak",
            "assets/windowsfoo/x.png",
            "packaging/linux2/x.sh",
            "scripts/tests_helper.py",
            "scripts/setup-linux.sh.orig",
            ".github/workflows/cirelease.yml",
        ):
            with self.subTest(path=path):
                self.assert_decisions(
                    systems=self.ALL_SYSTEMS,
                    check_reason=f"app file changed: {path}",
                    changed=(path,),
                )

    def test_schedule_tests_everything_and_audits(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="event schedule tests everything",
            validation="true",
            validation_reason="nightly run",
            event="schedule",
        )

    def test_unticked_manual_run_tests_everything(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="event workflow_dispatch tests everything",
            validation_reason="manual run, box not ticked",
            event="workflow_dispatch",
        )

    def test_ticked_manual_run_keeps_validation_audit(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="event workflow_dispatch tests everything",
            validation="true",
            validation_reason="manual run",
            event="workflow_dispatch",
            force="true",
        )

    def test_push_with_zero_before_tests_everything(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="change range unknown",
            validation="true",
            validation_reason="change range unknown",
            event="push",
            push_before="0000000000000000000000000000000000000000",
        )

    def test_pull_request_with_unknown_range_tests_everything(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="change range unknown",
            validation="true",
            validation_reason="change range unknown",
            pr_base="",
        )

    def test_diff_failure_tests_everything(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="change range unknown",
            validation="true",
            validation_reason="change range unknown",
            git_fails=True,
        )

    def test_empty_diff_tests_everything(self) -> None:
        self.assert_decisions(
            systems=self.ALL_SYSTEMS,
            check_reason="no changed files",
        )

    def test_docs_only_pull_request_with_validation_label_keeps_audit(self) -> None:
        self.assert_decisions(
            systems=(),
            check_reason="docs-only changes",
            validation="true",
            validation_reason="run-validation label",
            changed=("docs/plans/x.md",),
            labels="run-validation",
        )

    def test_check_runs_only_on_the_decided_systems(self) -> None:
        check = extract_job(self.workflow, "check")
        header, steps_text = check.split("    steps:\n", maxsplit=1)
        self.assertRegex(header, r"(?m)^    needs:\s*changes\s*$")
        self.assertRegex(header, r"(?m)^    if: needs\.changes\.outputs\.check == 'true'\s*$")
        self.assertRegex(
            header,
            r"(?m)^      matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.check_matrix\) \}\}\s*$",
        )
        # The decision is made once, for the whole job: no step re-reads it.
        self.assertNotIn("needs.changes.outputs", steps_text)

    def test_all_systems_matrix_keeps_each_host_triple(self) -> None:
        outputs, _ = self.run_decide(changed=("crates/bir-core/src/lib.rs",))
        self.assertEqual(
            json.loads(outputs["check_matrix"]),
            {
                "include": [
                    {"os": "macos-14", "target": "aarch64-apple-darwin"},
                    {"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu"},
                    {"os": "windows-latest", "target": "x86_64-pc-windows-msvc"},
                ]
            },
        )

    def test_ci_ok_is_the_single_gate(self) -> None:
        gate = extract_job(self.workflow, "ci-ok")
        self.assertRegex(gate, r"(?m)^    if: always\(\)\s*$")
        needs = re.search(r"(?m)^    needs: \[(.*)\]\s*$", gate)
        self.assertIsNotNone(needs)
        if needs is not None:
            self.assertEqual(
                [name.strip() for name in needs.group(1).split(",")],
                ["changes", "check", "shell-check", "scripts", "validation-rules", "legacy-guard"],
            )
        # A skip caused by a failed `changes` must fail the gate.
        self.assertIn('[[ "$CHANGES" == success ]]', gate)
        # windows-smoke is report-only and must not hold the gate.
        self.assertNotIn("windows-smoke", gate.split("    steps:", 1)[0])

    def test_scripts_job_runs_the_python_tests(self) -> None:
        scripts = extract_job(self.workflow, "scripts")
        self.assertRegex(scripts, r"(?m)^    if: needs\.changes\.outputs\.scripts == 'true'\s*$")
        self.assertIn("python -m unittest discover -s scripts/tests -v", scripts)

    def test_painted_windows_smoke_job_is_removed(self) -> None:
        self.assertNotIn("windows-smoke:", self.workflow)

    def test_main_failures_open_an_issue_but_cancellations_do_not(self) -> None:
        report = extract_job(self.workflow, "main-report")
        self.assertRegex(report, r"(?m)^    needs: \[ ci-ok \]\s*$")
        self.assertIn("needs.ci-ok.result == 'failure'", report)
        self.assertIn("github.ref == 'refs/heads/main'", report)

if __name__ == "__main__":
    unittest.main()
