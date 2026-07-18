from __future__ import annotations

import importlib.util
import json
import os
import plistlib
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "scripts/macos_candidate_certification.py"
SPEC = importlib.util.spec_from_file_location("macos_candidate_certification", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
certification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(certification)


class MacosCandidateCertificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_revision = "a" * 40
        self.app = self.root / "source/eBIRForms.app"
        renderer = self.app / certification.common.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.app / "Contents/MacOS/bir"
        binary.parent.mkdir(parents=True)
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        info = self.app / "Contents/Info.plist"
        info.parent.mkdir(parents=True, exist_ok=True)
        info.write_bytes(plistlib.dumps({"CFBundleExecutable": "bir"}))

        self.renderer_hash = certification.common.tree_hash(renderer)
        self.identity = self.root / "form-renderer-build-identity.json"
        identity = {
            "schema_version": 1,
            "scope": "build_time_non_promotional_identity",
            "promotion_eligible": False,
            "offline_verification_passed": True,
            "renderer_bundle_relative_path": "assets/form-renderer",
            "renderer_bundle_sha256": self.renderer_hash,
            "source_revision": {"status": "observed", "value": self.source_revision},
        }
        identity_bytes = (json.dumps(identity, sort_keys=True) + "\n").encode()
        self.identity.write_bytes(identity_bytes)
        bundled_identity = self.app / certification.common.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(identity_bytes)

        self.archive = self.root / "ebirforms-macos-universal.zip"
        self._write_archive(self.archive)
        self.manifest = self.root / "candidate-manifest.json"
        archive_record = certification.common.file_record(self.archive)
        identity_record = certification.common.file_record(self.identity)
        self.manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": certification.CANDIDATE_SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": certification.FORM,
                    "source_revision": self.source_revision,
                    "platform": "macos",
                    "architecture": "universal",
                    "artifact": {
                        "name": self.archive.name,
                        "byte_count": archive_record["byte_count"],
                        "sha256": archive_record["sha256"],
                    },
                    "renderer_identity": {
                        "name": self.identity.name,
                        "sha256": identity_record["sha256"],
                        "renderer_bundle_sha256": self.renderer_hash,
                    },
                    "release_policy": {
                        "candidate_build_requires_release_ready": False,
                        "tagged_release_still_requires_release_ready": True,
                    },
                },
                sort_keys=True,
            )
            + "\n"
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def _write_archive(self, archive: Path) -> None:
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(candidate for candidate in self.app.rglob("*") if candidate.is_file()):
                relative = path.relative_to(self.app.parent).as_posix()
                info = zipfile.ZipInfo(relative)
                info.create_system = 3
                info.external_attr = (path.stat().st_mode & 0xFFFF) << 16
                bundle.writestr(info, path.read_bytes())

    def _binding(self) -> dict:
        output = self.root / "inspection"
        return certification.inspect_candidate(
            self.manifest, self.archive, self.identity, output
        )

    def _record(self, path: Path) -> dict:
        return certification.common.file_record(path)

    def _attestation(self, binding: dict) -> tuple[Path, dict]:
        evidence = self.root / "attestation"
        evidence.mkdir()
        generic = evidence / "observation.log"
        generic.write_text("observed\n")
        generic_record = self._record(generic)
        output_pdf = evidence / "toolbar-export.pdf"
        output_pdf.write_bytes(b"fake deterministic export for schema tests\n")
        before_destination = evidence / "destination-before.bin"
        after_destination = evidence / "destination-after.bin"
        before_destination.write_bytes(b"preserved destination\n")
        after_destination.write_bytes(b"preserved destination\n")
        before_draft = evidence / "draft-before.json"
        after_draft = evidence / "draft-after.json"
        before_draft.write_text('{"unchanged":true}\n')
        after_draft.write_text('{"unchanged":true}\n')
        temporary_files = evidence / "temporary-files.json"
        temporary_files.write_text('{"remaining":[]}\n')
        verifier_artifact = evidence / "pdf-verifier.json"
        verifier_artifact.write_text("{}\n")

        pages = [
            {
                "page": page,
                "media_width_pt": 612.0,
                "media_height_pt": 936.0,
                "crop_width_pt": 612.0,
                "crop_height_pt": 936.0,
                "rotation": 0,
                "content_byte_count": page * 10,
            }
            for page in (1, 2)
        ]
        rectangles = [
            {"page": page, "x": 0.0, "y": (page - 1) * 936.0, "width_pt": 612.0, "height_pt": 936.0}
            for page in (1, 2)
        ]
        artifact = lambda: dict(generic_record)
        value = {
            "schema_version": 1,
            "scope": certification.ATTESTATION_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "operator_only": True,
            "attestation_id": "abcdefab-1234-5678-9234-567812345678",
            "form": certification.FORM,
            "candidate": {
                "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
                "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
                "source_revision": binding["source_revision"],
                "app_tree_sha256": binding["packaged_app"]["app_tree_sha256"],
                "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
            },
            "collector": {
                "name": "external macOS operator collector",
                "version": "1",
                "invocation_id": "test-invocation",
                "started_at_utc": "2026-07-18T00:00:00Z",
                "completed_at_utc": "2026-07-18T00:05:00Z",
                "executable_sha256": "b" * 64,
                "host_identifier_sha256": "c" * 64,
            },
            "accessibility": {
                "permission_granted": True,
                "automation_identity": "operator.test",
                "artifact": artifact(),
            },
            "runtime": {
                "non_dev_build": True,
                "dev_tools_enabled": False,
                "launch_argv": [binding["packaged_app"]["binary"]["path"]],
                "pid": 123,
                "network_denial": {
                    "mechanism": "sandbox-exec deny network*",
                    "exercised": True,
                    "enforced_for_launch": True,
                    "passed": True,
                    "artifact": artifact(),
                },
                "artifact": artifact(),
            },
            "preview": {
                "exercised": True,
                "passed": True,
                "window_title": "2551Q HTML Form Preview",
                "document_run_id": "run-1",
                "envelope_sha256": "d" * 64,
                "nonce": 7,
                "page_count": 2,
                "geometry_measurements": [
                    {
                        "measurement_index": index,
                        "page_width_pt": 612.0,
                        "page_height_pt": 936.0,
                        "pages": rectangles,
                        "clipping_count": 0,
                        "overflow_count": 0,
                    }
                    for index in (1, 2)
                ],
                "artifact": artifact(),
            },
            "toolbar_export": {
                "exercised": True,
                "passed": True,
                "control": "Export PDF",
                "save_chooser_exercised": True,
                "destination_path": str(output_pdf.resolve()),
                "nonce": 7,
                "artifact": artifact(),
            },
            "native_print": {
                "exercised": True,
                "passed": True,
                "completed": True,
                "printer_name": "Certification_Printer",
                "job_id": "Certification_Printer-42",
                "artifact": artifact(),
            },
            "pdf_validation": {
                "exercised": True,
                "passed": True,
                "output": self._record(output_pdf),
                "expected_page_count": 2,
                "actual_page_count": 2,
                "pages": pages,
                "content_nonempty": True,
                "validated_by": "bir-print::html_output::validate_pdf_file",
                "verifier_executable_sha256": "f" * 64,
                "artifact": self._record(verifier_artifact),
            },
            "package_security": {
                "codesign": {
                    "passed": True,
                    "developer_id_signed": True,
                    "authority": "Developer ID Application: Example",
                    "team_identifier": "EXAMPLE123",
                    "artifact": artifact(),
                },
                "notarization": {"passed": True, "gatekeeper_accepted": True, "artifact": artifact()},
                "stapling": {"passed": True, "artifact": artifact()},
            },
            "integrity": {
                "app_tree_sha256_before": binding["packaged_app"]["app_tree_sha256"],
                "app_tree_sha256_after": binding["packaged_app"]["app_tree_sha256"],
                "destination_before": self._record(before_destination),
                "destination_after": self._record(after_destination),
                "draft_before": self._record(before_draft),
                "draft_after": self._record(after_draft),
                "temporary_files_manifest": self._record(temporary_files),
            },
            "rollback": {
                "cases": [
                    {"name": name, "passed": True, "artifact": artifact()}
                    for name in sorted(certification.ROLLBACK_CASES)
                ],
                "destination_preserved": True,
                "temporary_files_remaining": 0,
                "draft_unchanged": True,
            },
            "strict_verifier_gaps": [certification.NON_PROMOTIONAL_GAP],
        }
        path = evidence / "macos-attestation.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def test_inspection_binds_exact_archive_identity_and_app(self) -> None:
        binding = self._binding()

        self.assertFalse(binding["promotion_eligible"])
        self.assertFalse(binding["trusted_producer"])
        self.assertFalse(binding["certification_complete"])
        self.assertEqual(
            binding["packaged_app"]["renderer_bundle_sha256"], self.renderer_hash
        )
        self.assertIn(certification.NON_PROMOTIONAL_GAP, binding["strict_verifier_gaps"])

    def test_manifest_hash_mismatch_fails_closed(self) -> None:
        self.archive.write_bytes(self.archive.read_bytes() + b"tampered")
        with self.assertRaisesRegex(certification.EvidenceError, "size differs"):
            certification.validate_candidate_inputs(
                self.manifest, self.archive, self.identity
            )

    def test_symlinked_candidate_input_is_rejected(self) -> None:
        link = self.root / "manifest-link.json"
        link.symlink_to(self.manifest)
        with self.assertRaisesRegex(certification.EvidenceError, "non-symlink"):
            certification.validate_candidate_inputs(link, self.archive, self.identity)

    def test_archive_path_traversal_is_rejected(self) -> None:
        archive = self.root / "traversal.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("../escape", b"bad")
        with self.assertRaisesRegex(certification.EvidenceError, "escapes"):
            certification.extract_candidate_archive(archive, self.root / "unsafe")

    def test_complete_attestation_is_closed_and_non_promotional(self) -> None:
        binding = self._binding()
        path, _ = self._attestation(binding)

        attestation, verified = certification.validate_attestation(path, binding)

        self.assertFalse(attestation["promotion_eligible"])
        self.assertFalse(attestation["trusted_producer"])
        self.assertGreater(len(verified), 20)

    def test_missing_rollback_case_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["rollback"]["cases"].pop()
        path.write_text(json.dumps(value))

        with self.assertRaisesRegex(certification.EvidenceError, "incomplete"):
            certification.validate_attestation(path, binding)

    def test_uuid_must_be_canonical_lowercase(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["attestation_id"] = value["attestation_id"].upper()
        path.write_text(json.dumps(value))

        with self.assertRaisesRegex(certification.EvidenceError, "canonical lowercase"):
            certification.validate_attestation(path, binding)

    def test_collector_time_range_must_move_forward(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["collector"]["completed_at_utc"] = value["collector"]["started_at_utc"]
        path.write_text(json.dumps(value))

        with self.assertRaisesRegex(certification.EvidenceError, "after its start"):
            certification.validate_attestation(path, binding)

    def test_owned_pdf_verifier_is_rerun_and_exact_output_is_compared(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        verifier = self.root / "verify-pdf"
        output_pdf = Path(value["pdf_validation"]["output"]["path"])
        report = {
            "schema_version": 1,
            "scope": "owned_macos_candidate_pdf_validation",
            "promotion_eligible": False,
            "form": certification.FORM,
            "envelope_sha256": value["preview"]["envelope_sha256"],
            "output_sha256": certification.common.file_record(output_pdf)["sha256"],
            "expected_page_count": 2,
            "actual_page_count": 2,
            "width_points": 612.0,
            "height_points": 936.0,
            "content_nonempty": True,
            "validated_by": "bir-print::html_output::validate_pdf_file",
            "pages": value["pdf_validation"]["pages"],
        }
        encoded = (json.dumps(report, separators=(",", ":")) + "\n").encode()
        verifier.write_text(
            "#!/usr/bin/env python3\nimport sys\nsys.stdout.buffer.write(" + repr(encoded) + ")\n"
        )
        verifier.chmod(0o755)
        value["pdf_validation"]["verifier_executable_sha256"] = self._record(verifier)[
            "sha256"
        ]
        artifact = Path(value["pdf_validation"]["artifact"]["path"])
        artifact.write_bytes(encoded)
        value["pdf_validation"]["artifact"] = self._record(artifact)
        path.write_text(json.dumps(value))
        attestation, verified = certification.validate_attestation(path, binding)

        result = certification.verify_owned_pdf_artifact(
            path, attestation, verifier, verified
        )

        self.assertEqual(result["actual_page_count"], 2)
        self.assertEqual(result["verifier_executable_sha256"], self._record(verifier)["sha256"])

    @mock.patch.object(certification.platform, "system", return_value="Darwin")
    @mock.patch.object(certification, "_run_required", return_value="false")
    def test_live_verification_fails_closed_without_accessibility(
        self, _run: mock.Mock, _system: mock.Mock
    ) -> None:
        with self.assertRaisesRegex(certification.EvidenceError, "Accessibility"):
            certification.verify_live_macos_state(
                self.app,
                {"native_print": {"printer_name": "Certification_Printer"}},
            )

    @mock.patch.object(certification.platform, "system", return_value="Darwin")
    @mock.patch.object(certification, "_run_required", side_effect=["true", "printer disabled"])
    def test_live_verification_fails_closed_for_disabled_printer(
        self, _run: mock.Mock, _system: mock.Mock
    ) -> None:
        with self.assertRaisesRegex(certification.EvidenceError, "printer is disabled"):
            certification.verify_live_macos_state(
                self.app,
                {"native_print": {"printer_name": "Certification_Printer"}},
            )

    @mock.patch.object(certification.platform, "system", return_value="Darwin")
    @mock.patch.object(
        certification,
        "_run_required",
        side_effect=["true", "printer Certification_Printer is idle", "Other_Printer-1 user"],
    )
    def test_live_verification_requires_the_completed_print_job(
        self, _run: mock.Mock, _system: mock.Mock
    ) -> None:
        with self.assertRaisesRegex(certification.EvidenceError, "completed CUPS jobs"):
            certification.verify_live_macos_state(
                self.app,
                {
                    "native_print": {
                        "printer_name": "Certification_Printer",
                        "job_id": "Certification_Printer-42",
                    }
                },
            )

    def test_schemas_preserve_the_untrusted_operator_only_boundary(self) -> None:
        schema_root = REPOSITORY_ROOT / "packages/form-specs/schema"
        attestation = json.loads(
            (schema_root / "macos-candidate-certification-attestation-v1.schema.json").read_text()
        )
        report = json.loads(
            (schema_root / "macos-candidate-certification-report-v1.schema.json").read_text()
        )
        self.assertEqual(attestation["properties"]["promotion_eligible"], {"const": False})
        self.assertEqual(report["properties"]["trusted_producer"], {"const": False})
        self.assertEqual(report["properties"]["promotion_satisfied"], {"const": False})
        self.assertNotIn("release_ready", json.dumps(report))


if __name__ == "__main__":
    unittest.main()
