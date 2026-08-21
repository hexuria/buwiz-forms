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
FREEZE_CHECK = "stamp_frozen_names.py --check-all"


class HtmlReleaseGatePolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.ci = CI_WORKFLOW.read_text(encoding="utf-8")
        cls.release = RELEASE_WORKFLOW.read_text(encoding="utf-8")
        cls.candidate = CANDIDATE_WORKFLOW.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE.read_text(encoding="utf-8")
        cls.package_json = PACKAGE_JSON.read_text(encoding="utf-8")

    def test_ci_and_release_gate_frozen_html_not_the_react_renderer(self) -> None:
        self.assertNotIn("\n  renderer:", self.ci)
        self.assertNotIn("npm run build:forms", self.ci)
        self.assertNotIn("npm run test:forms:visual", self.ci)
        self.assertIn("freeze_html.py --verify", self.ci)
        self.assertIn(FREEZE_CHECK, self.ci)
        self.assertIn("freeze_html.py --verify", self.release)
        self.assertIn(FREEZE_CHECK, self.release)
        self.assertNotIn("npm run build:forms", self.release)
        self.assertNotIn("npm run test:forms:visual", self.release)

    def test_candidate_workflow_builds_without_weakening_tagged_release(self) -> None:
        self.assertIn("workflow_dispatch:", self.candidate)
        self.assertIn("contents: read", self.candidate)
        self.assertNotIn("push:", self.candidate)
        self.assertNotIn("softprops/action-gh-release", self.candidate)
        self.assertNotIn("--require-release-ready", self.candidate)
        self.assertNotIn("form-release-evidence.json", self.candidate)
        self.assertIn("freeze_html.py --verify", self.candidate)
        self.assertIn(FREEZE_CHECK, self.candidate)
        self.assertNotIn("npm run build:forms", self.candidate)
        self.assertGreaterEqual(self.candidate.count("cargo build --locked --release"), 3)
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

    def test_macos_development_observation_path_remains_non_promotional(self) -> None:
        recipe = self.justfile.split("native-evidence-macos:", maxsplit=1)[1]
        recipe = recipe.split("# Install a built package", maxsplit=1)[0]
        self.assertIn("non-promotional", recipe)
        self.assertIn("form-release-evidence.json", recipe)
        self.assertNotIn("release_ready", recipe)

    def test_external_macos_driver_remains_non_promotional_and_untrusted(self) -> None:
        recipe = self.justfile.split(
            "native-evidence-macos-external", maxsplit=1
        )[1]
        recipe = recipe.split("# Install a built package", maxsplit=1)[0]
        self.assertIn("non-promotional", recipe)
        self.assertNotIn("release_ready", recipe)

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
