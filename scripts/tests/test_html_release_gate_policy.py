from __future__ import annotations

import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW = REPOSITORY_ROOT / ".github/workflows/ci.yml"
RELEASE_WORKFLOW = REPOSITORY_ROOT / ".github/workflows/release.yml"
CANDIDATE_WORKFLOW = (
    REPOSITORY_ROOT / ".github/workflows/html-candidate-certification.yml"
)
JUSTFILE = REPOSITORY_ROOT / "justfile"
PACKAGE_JSON = REPOSITORY_ROOT / "package.json"
REQUIRED_AUDIT = (
    "npm run audit:forms:migration -- --require-release-ready 2551Q:2018"
)


class HtmlReleaseGatePolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.ci = CI_WORKFLOW.read_text(encoding="utf-8")
        cls.release = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.candidate = CANDIDATE_WORKFLOW.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE.read_text(encoding="utf-8")
        cls.package_json = PACKAGE_JSON.read_text(encoding="utf-8")

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

    def test_candidate_workflow_builds_without_weakening_tagged_release(self) -> None:
        self.assertIn("workflow_dispatch:", self.candidate)
        self.assertIn("contents: read", self.candidate)
        self.assertNotIn("push:", self.candidate)
        self.assertNotIn("softprops/action-gh-release", self.candidate)
        self.assertNotIn("--require-release-ready", self.candidate)
        self.assertNotIn("--features dev-tools", self.candidate)
        self.assertNotIn("form-release-evidence.json", self.candidate)
        self.assertIn(
            "python3 scripts/audit_html_form_migration.py --require-clean-source",
            self.candidate,
        )
        self.assertGreaterEqual(
            self.candidate.count("npm run audit:forms:migration"),
            3,
        )
        self.assertEqual(
            self.candidate.count("cargo build --locked --release"),
            4,
        )
        self.assertEqual(
            self.candidate.count("uses: actions/upload-artifact@v4"),
            3,
        )
        self.assertIn("runs-on: macos-14", self.candidate)
        self.assertIn("runs-on: windows-latest", self.candidate)
        self.assertIn("runs-on: ubuntu-22.04", self.candidate)
        self.assertIn("cargo fmt --all -- --check", self.candidate)
        self.assertIn("cargo check --locked --workspace", self.candidate)
        self.assertIn(
            "cargo clippy --locked --workspace --all-targets -- -D warnings",
            self.candidate,
        )
        self.assertIn("cargo test --locked --workspace", self.candidate)

        release_audit_count = self.release.count("npm run audit:forms:migration")
        self.assertGreater(release_audit_count, 0)
        self.assertEqual(self.release.count(REQUIRED_AUDIT), release_audit_count)

    def test_ci_visual_gate_is_strict_and_blocking(self) -> None:
        step = self.workflow_step(
            self.ci,
            "Enforce strict complete-page visual parity (<= 1%)",
        )

        self.assertEqual(self.ci.count("npm run test:forms:visual"), 1)
        self.assertIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '1'", step)
        self.assertNotIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '100'", self.ci)
        self.assertNotIn("continue-on-error: true", step)

    def test_macos_development_observation_path_remains_non_promotional(self) -> None:
        recipe = self.justfile.split("native-evidence-macos:", maxsplit=1)[1]
        recipe = recipe.split("# Install a built package", maxsplit=1)[0]

        self.assertIn("--require-clean-source", recipe)
        self.assertIn("just _package-mac --native-evidence", recipe)
        self.assertIn("verify:native-output:observation", recipe)
        self.assertIn("non-promotional", recipe)
        self.assertNotIn("release_ready", recipe)
        self.assertNotIn("audit:forms:migration -- --require-release-ready", recipe)
        self.assertIn('"verify:native-output:observation"', self.package_json)

        package_recipe = self.justfile.split('_package-mac args="":', maxsplit=1)[1]
        package_recipe = package_recipe.split("_package-mac-appstore", maxsplit=1)[0]
        self.assertIn(
            '--native-evidence) FEATURES="${FEATURES:+$FEATURES,}dev-tools"',
            package_recipe,
        )

    def test_external_macos_driver_remains_non_promotional_and_untrusted(self) -> None:
        recipe = self.justfile.split(
            "native-evidence-macos-external", maxsplit=1
        )[1]
        recipe = recipe.split("# Install a built package", maxsplit=1)[0]

        self.assertIn("macos_native_evidence_driver.py", recipe)
        self.assertIn("--network-denied", recipe)
        self.assertIn("non-promotional", recipe)
        self.assertNotIn("release_ready", recipe)
        self.assertNotIn("form-release-evidence.json", recipe)

        driver = (
            REPOSITORY_ROOT / "scripts/macos_native_evidence_driver.py"
        ).read_text(encoding="utf-8")
        self.assertIn('"promotion_eligible": False', driver)
        self.assertIn('"trusted_producer": False', driver)
        self.assertNotIn("form-release-evidence.json\"", driver)


if __name__ == "__main__":
    unittest.main()
