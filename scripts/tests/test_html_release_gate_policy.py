from __future__ import annotations

import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPOSITORY_ROOT / ".github/workflows/ci.yml"
RELEASE_WORKFLOW = REPOSITORY_ROOT / ".github/workflows/release.yml"
REQUIRED_AUDIT = (
    "npm run audit:forms:migration -- --require-release-ready 2551Q:2018"
)


class HtmlReleaseGatePolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.ci = CI_WORKFLOW.read_text(encoding="utf-8")
        cls.release = RELEASE_WORKFLOW.read_text(encoding="utf-8")

    def workflow_step(self, workflow: str, name: str) -> str:
        remainder = workflow.split(f"- name: {name}", maxsplit=1)[1]
        return remainder.split("- name:", maxsplit=1)[0]

    def test_every_tagged_release_audit_requires_certified_2551q(self) -> None:
        audit_count = self.release.count("npm run audit:forms:migration")

        self.assertGreater(audit_count, 0)
        self.assertEqual(self.release.count(REQUIRED_AUDIT), audit_count)

    def test_release_visual_gate_is_strict_and_blocking(self) -> None:
        step = self.workflow_step(
            self.release,
            "Enforce strict complete-page visual parity on the tagged source (<= 1%)",
        )

        self.assertIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '1'", step)
        self.assertNotIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '100'", self.release)
        self.assertNotIn("continue-on-error: true", step)

    def test_ci_visual_gate_is_strict_and_blocking(self) -> None:
        step = self.workflow_step(
            self.ci,
            "Enforce strict complete-page visual parity (<= 1%)",
        )

        self.assertEqual(self.ci.count("npm run test:forms:visual"), 1)
        self.assertIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '1'", step)
        self.assertNotIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '100'", self.ci)
        self.assertNotIn("continue-on-error: true", step)


if __name__ == "__main__":
    unittest.main()
