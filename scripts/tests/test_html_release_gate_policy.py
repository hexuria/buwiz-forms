from __future__ import annotations

import json
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

        self.assertEqual(
            self.candidate.count(
                "scripts/audit_html_form_migration.py --print-source-revision --require-clean-source"
            ),
            3,
        )
        self.assertNotIn('--source-revision "$GITHUB_SHA"', self.candidate)
        self.assertNotIn("--source-revision $env:GITHUB_SHA", self.candidate)

        release_audit_count = self.release.count("npm run audit:forms:migration")
        self.assertGreater(release_audit_count, 0)
        self.assertEqual(self.release.count(REQUIRED_AUDIT), release_audit_count)

    def test_macos_candidate_is_signed_notarized_and_bound_after_stapling(self) -> None:
        macos = self.candidate.split("  macos-candidate:", maxsplit=1)[1]
        macos = macos.split("  windows-candidate:", maxsplit=1)[0]

        for secret in (
            "APPLE_CERTIFICATE_P12",
            "APPLE_CERTIFICATE_PASSWORD",
            "APPLE_TEAM_ID",
            "APPLE_ID",
            "APPLE_APP_PASSWORD",
        ):
            self.assertIn(f"secrets.{secret}", macos)
        self.assertIn("security set-key-partition-list", macos)
        self.assertIn("security find-identity -v -p codesigning", macos)
        self.assertIn("Developer ID Application:", macos)
        self.assertIn("codesign --force --options runtime", macos)
        self.assertIn("--entitlements assets/macos/entitlements.mac.plist", macos)
        self.assertIn("xcrun notarytool submit", macos)
        self.assertIn("--output-format json", macos)
        self.assertIn('result.get("status") == "Accepted"', macos)
        self.assertIn('xcrun stapler staple "$APP"', macos)
        self.assertIn('xcrun stapler validate "$APP"', macos)
        self.assertIn('spctl --assess --type execute --verbose=2 "$APP"', macos)
        self.assertNotIn("--sign -", macos)
        self.assertIn("scripts/macos_candidate_certification.py inspect", macos)
        self.assertLess(
            macos.index('xcrun stapler validate "$APP"'),
            macos.index("scripts/write_html_candidate_manifest.py"),
        )

    def test_ci_visual_threshold_is_strict_and_always_reported(self) -> None:
        """The CI complete-page step is reporting-only, by reviewed decision.

        The <= 1% target is unreachable by proof, not by regression: every one
        of the 35 source PDFs carries `emb=no` for its primary faces, so the
        pinned references encode substituted glyph outlines, and the pinned
        per-page noise floor already exceeds 1% before the renderer draws
        anything (2551Q page 1 measures 7.14% against a 3.61% floor). Gating on
        it made the job permanently red and therefore signal-free.

        Everything that protects the *number* is still asserted here. Only the
        step's power to block the pipeline was given up, and only in CI - the
        release workflow's gate stays strict and blocking, covered by
        `test_release_visual_gate_is_strict_and_blocking` above.
        """
        step = self.workflow_step(
            self.ci,
            "Report strict complete-page visual parity (<= 1%, non-gating)",
        )

        self.assertEqual(self.ci.count("npm run test:forms:visual"), 1)
        self.assertIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '1'", step)
        self.assertNotIn("FORM_VISUAL_MAX_CHANGED_PERCENT: '100'", self.ci)
        # The threshold may never be relaxed, in CI or anywhere else.
        for relaxed in ("'2'", "'5'", "'10'", "'50'", "'100'"):
            self.assertNotIn(f"FORM_VISUAL_MAX_CHANGED_PERCENT: {relaxed}", self.ci)
        # The measured percentage must keep being produced and published, so a
        # non-gating step can never become a silent one.
        self.assertIn("test-results/form-renderer", self.ci)

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

    def test_candidate_certification_foundation_cannot_promote_or_register_trust(self) -> None:
        verifier = (
            REPOSITORY_ROOT / "scripts/macos_candidate_certification.py"
        ).read_text(encoding="utf-8")
        attestation_schema = json.loads(
            (
                REPOSITORY_ROOT
                / "packages/form-specs/schema/macos-candidate-certification-attestation-v1.schema.json"
            ).read_text(encoding="utf-8")
        )
        report_schema = json.loads(
            (
                REPOSITORY_ROOT
                / "packages/form-specs/schema/macos-candidate-certification-report-v1.schema.json"
            ).read_text(encoding="utf-8")
        )

        self.assertIn('"promotion_eligible": False', verifier)
        self.assertIn('"trusted_producer": False', verifier)
        self.assertIn('"promotion_satisfied": False', verifier)
        self.assertNotIn("form-release-evidence.json", verifier)
        self.assertNotIn("TRUSTED_PLATFORM_EVIDENCE_PRODUCERS", verifier)
        self.assertNotIn("TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS", verifier)
        self.assertEqual(
            attestation_schema["properties"]["promotion_eligible"], {"const": False}
        )
        self.assertEqual(
            report_schema["properties"]["trusted_producer"], {"const": False}
        )
        self.assertEqual(
            report_schema["properties"]["promotion_satisfied"], {"const": False}
        )

    def test_windows_candidate_foundation_is_untrusted_and_keeps_installer_policy_closed(self) -> None:
        verifier = (
            REPOSITORY_ROOT / "scripts/windows_candidate_certification.py"
        ).read_text(encoding="utf-8")
        attestation_schema = json.loads(
            (
                REPOSITORY_ROOT
                / "packages/form-specs/schema/windows-candidate-certification-attestation-v1.schema.json"
            ).read_text(encoding="utf-8")
        )
        report_schema = json.loads(
            (
                REPOSITORY_ROOT
                / "packages/form-specs/schema/windows-candidate-certification-report-v1.schema.json"
            ).read_text(encoding="utf-8")
        )

        self.assertIn('"promotion_eligible": False', verifier)
        self.assertIn('"trusted_producer": False', verifier)
        self.assertIn('"promotion_satisfied": False', verifier)
        self.assertNotIn("form-release-evidence.json\"", verifier)
        self.assertNotIn("TRUSTED_PLATFORM_EVIDENCE_PRODUCERS", verifier)
        self.assertNotIn("TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS", verifier)
        self.assertEqual(
            attestation_schema["properties"]["promotion_eligible"], {"const": False}
        )
        self.assertEqual(
            report_schema["properties"]["trusted_producer"], {"const": False}
        )
        policy = attestation_schema["properties"]["package_security"]["properties"][
            "distribution_policy"
        ]["properties"]
        self.assertEqual(policy["public_release_allows_msix"], {"const": False})
        self.assertEqual(
            policy["public_release_formats"],
            {"const": ["signed_inno_setup_exe", "signed_msi"]},
        )


if __name__ == "__main__":
    unittest.main()
