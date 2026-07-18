from __future__ import annotations

import json
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


class LinuxCandidateCertificationPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = (
            REPOSITORY_ROOT / "scripts/linux_candidate_certification.py"
        ).read_text(encoding="utf-8")
        cls.workflow = (
            REPOSITORY_ROOT / ".github/workflows/html-candidate-certification.yml"
        ).read_text(encoding="utf-8")
        cls.release = (REPOSITORY_ROOT / ".github/workflows/release.yml").read_text(
            encoding="utf-8"
        )
        cls.rust_pdf = (
            REPOSITORY_ROOT
            / "crates/bir-print/src/bin/verify_certification_pdf.rs"
        ).read_text(encoding="utf-8")

    def test_foundation_cannot_promote_or_register_trust(self) -> None:
        self.assertIn('"promotion_eligible": False', self.verifier)
        self.assertIn('"trusted_producer": False', self.verifier)
        self.assertIn('"promotion_satisfied": False', self.verifier)
        self.assertNotIn("form-release-evidence.json", self.verifier)
        self.assertNotIn("TRUSTED_PLATFORM_EVIDENCE_PRODUCERS", self.verifier)
        self.assertNotIn("TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS", self.verifier)

    def test_closed_schemas_lock_non_promotional_package_boundary(self) -> None:
        schema_root = REPOSITORY_ROOT / "packages/form-specs/schema"
        attestation = json.loads(
            (schema_root / "linux-candidate-certification-attestation-v1.schema.json").read_text()
        )
        report = json.loads(
            (schema_root / "linux-candidate-certification-report-v1.schema.json").read_text()
        )
        self.assertFalse(attestation["additionalProperties"])
        self.assertFalse(report["additionalProperties"])
        self.assertEqual(attestation["properties"]["promotion_eligible"], {"const": False})
        self.assertEqual(report["properties"]["trusted_producer"], {"const": False})
        self.assertEqual(report["properties"]["promotion_satisfied"], {"const": False})
        for name in (
            "final_release_deb_verified",
            "final_release_tarball_verified",
            "release_package_signature_verified",
        ):
            self.assertEqual(
                attestation["properties"]["package_boundary"]["properties"][name],
                {"const": False},
            )

    def test_both_linux_hosts_and_owned_pdf_scope_are_mandatory(self) -> None:
        schema = json.loads(
            (
                REPOSITORY_ROOT
                / "packages/form-specs/schema/linux-candidate-certification-attestation-v1.schema.json"
            ).read_text()
        )
        runs = schema["properties"]["display_runs"]
        self.assertEqual(runs["required"], ["x11", "wayland"])
        self.assertIn("GpuiWryChild", self.verifier)
        self.assertIn("GtkTopLevel", self.verifier)
        self.assertIn("Xvfb", self.verifier)
        self.assertIn("Weston", self.verifier)
        self.assertIn('"linux"', self.rust_pdf)
        self.assertIn("owned_linux_candidate_pdf_validation", self.rust_pdf)

    def test_portable_candidate_is_not_mistaken_for_final_release_packages(self) -> None:
        linux_job = self.workflow.split("  linux-candidate:", maxsplit=1)[1]
        self.assertIn("Assemble portable candidate application", linux_job)
        self.assertIn("*.tar.gz", linux_job)
        self.assertNotIn("cargo deb", linux_job)
        self.assertIn("cargo deb --locked", self.release)
        self.assertIn('"final_release_deb_verified": False', self.verifier)
        self.assertIn('"final_release_tarball_verified": False', self.verifier)
        self.assertIn('"release_package_signature_verified": False', self.verifier)


if __name__ == "__main__":
    unittest.main()
