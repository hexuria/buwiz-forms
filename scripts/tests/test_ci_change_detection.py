from __future__ import annotations

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

    def assert_decisions(
        self,
        *,
        code: str,
        validation: str,
        code_reason: str,
        validation_reason: str,
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
        self.assertEqual(outputs, {"validation": validation, "code": code})
        self.assertIn(
            f"validation-rules audits: {validation} ({validation_reason})",
            summary,
        )
        self.assertIn(
            f"full build and tests: {code} ({code_reason})",
            summary,
        )

    def test_docs_only_pull_request_skips_build_and_tests(self) -> None:
        self.assert_decisions(
            code="false",
            validation="false",
            code_reason="docs-only changes",
            validation_reason="no validation-rules path changed",
            changed=(
                "docs/plans/x.md",
                "README.md",
                "CONTRIBUTING.md",
                "crates/bir-desktop/docs/AGENT.md",
            ),
        )

    def test_docs_only_push_skips_build_and_tests(self) -> None:
        self.assert_decisions(
            code="false",
            validation="false",
            code_reason="docs-only changes",
            validation_reason="no validation-rules path changed",
            event="push",
            changed=("docs/plans/x.md", "README.md"),
        )

    def test_code_change_keeps_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="false",
            code_reason="non-docs file changed",
            validation_reason="no validation-rules path changed",
            changed=("docs/plans/x.md", "crates/bir-core/src/lib.rs"),
        )

    def test_rules_markdown_is_full_ci_and_validation_input(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="non-docs file changed",
            validation_reason="validation-rules paths changed",
            changed=("rules/forms/x/gaps.md",),
        )

    def test_script_markdown_keeps_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="false",
            code_reason="non-docs file changed",
            validation_reason="no validation-rules path changed",
            changed=("scripts/linux_candidate_certification.md",),
        )

    def test_crate_readme_keeps_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="false",
            code_reason="non-docs file changed",
            validation_reason="no validation-rules path changed",
            changed=("crates/bir/README.md",),
        )

    def test_workflow_change_keeps_full_ci_and_validation_audit(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="non-docs file changed",
            validation_reason="validation-rules paths changed",
            changed=(".github/workflows/ci.yml",),
        )

    def test_schedule_runs_full_ci_and_validation_audits(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="event schedule requires full CI",
            validation_reason="nightly run",
            event="schedule",
        )

    def test_unticked_manual_run_keeps_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="false",
            code_reason="event workflow_dispatch requires full CI",
            validation_reason="manual run, box not ticked",
            event="workflow_dispatch",
        )

    def test_ticked_manual_run_keeps_validation_audit(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="event workflow_dispatch requires full CI",
            validation_reason="manual run",
            event="workflow_dispatch",
            force="true",
        )

    def test_push_with_zero_before_runs_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="change range unknown",
            validation_reason="change range unknown",
            event="push",
            push_before="0000000000000000000000000000000000000000",
        )

    def test_pull_request_with_unknown_range_runs_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="change range unknown",
            validation_reason="change range unknown",
            pr_base="",
        )

    def test_diff_failure_runs_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="true",
            code_reason="change range unknown",
            validation_reason="change range unknown",
            git_fails=True,
        )

    def test_empty_diff_runs_full_ci(self) -> None:
        self.assert_decisions(
            code="true",
            validation="false",
            code_reason="no changed files",
            validation_reason="no validation-rules path changed",
        )

    def test_docs_only_pull_request_with_validation_label_keeps_audit(self) -> None:
        self.assert_decisions(
            code="false",
            validation="true",
            code_reason="docs-only changes",
            validation_reason="run-validation label",
            changed=("docs/plans/x.md",),
            labels="run-validation",
        )

    def test_check_matrix_reports_and_gates_every_step_after_checkout(self) -> None:
        check = extract_job(self.workflow, "check")
        header, steps_text = check.split("    steps:\n", maxsplit=1)
        self.assertRegex(header, r"(?m)^    needs:\s*changes\s*$")
        self.assertRegex(header, r"(?m)^    if:\s*\$\{\{\s*always\(\)\s*\}\}\s*$")
        self.assertEqual(
            re.findall(r"(?m)^\s+- os: ([^\s]+)\s*$", check),
            ["macos-14", "ubuntu-22.04", "windows-latest"],
        )

        steps = re.split(r"(?m)^    - ", steps_text)
        steps = [step for step in steps if step.strip()]
        checkout_index = next(
            index
            for index, step in enumerate(steps)
            if "uses: actions/checkout@v4" in step
        )
        docs_notice_index = next(
            index
            for index, step in enumerate(steps)
            if step.startswith("name: Docs-only change: skipping build and tests")
        )
        self.assertEqual(docs_notice_index, checkout_index + 1)
        self.assertRegex(
            steps[docs_notice_index],
            r"(?m)^      if: needs\.changes\.outputs\.code == 'false'$",
        )
        self.assertNotRegex(steps[checkout_index], r"(?m)^      if:")

        for index, step in enumerate(steps):
            if index <= checkout_index or index == docs_notice_index:
                continue
            condition = re.search(r"(?m)^      if:\s*(.*)$", step)
            self.assertIsNotNone(condition, msg=f"step {index} has no if guard")
            if condition is not None:
                self.assertIn(
                    "needs.changes.outputs.code != 'false'",
                    condition.group(1),
                    msg=f"step {index} is not gated",
                )


if __name__ == "__main__":
    unittest.main()
