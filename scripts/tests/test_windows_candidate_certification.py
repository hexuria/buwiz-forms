from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "scripts/windows_candidate_certification.py"
SPEC = importlib.util.spec_from_file_location("windows_candidate_certification", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
certification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(certification)


class WindowsCandidateCertificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_revision = "a" * 40
        self.package = self.root / "source"
        renderer = self.package / certification.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.package / certification.BINARY_RELATIVE_PATH
        binary.write_bytes(b"portable windows release candidate\n")

        self.renderer_hash = certification.artifact_common.tree_hash(renderer)
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
        bundled_identity = self.package / certification.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(identity_bytes)

        self.archive = self.root / "eBIRForms-Windows-x64.zip"
        self._write_archive(self.archive)
        self.manifest = self.root / "candidate-manifest.json"
        archive_record = certification.artifact_common.file_record(self.archive)
        identity_record = certification.artifact_common.file_record(self.identity)
        self.manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": certification.certification_common.CANDIDATE_SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": certification.FORM,
                    "source_revision": self.source_revision,
                    "platform": "windows",
                    "architecture": "x86_64",
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
            for path in sorted(candidate for candidate in self.package.rglob("*") if candidate.is_file()):
                bundle.write(path, path.relative_to(self.package).as_posix())

    def _binding(self) -> dict:
        return certification.inspect_candidate(
            self.manifest, self.archive, self.identity, self.root / "inspection"
        )

    def _record(self, path: Path) -> dict:
        return certification.artifact_common.file_record(path)

    def _attestation(self, binding: dict) -> tuple[Path, dict]:
        evidence = self.root / "attestation"
        evidence.mkdir()
        generic = evidence / "observation.log"
        generic.write_text("observed\n")
        generic_record = self._record(generic)
        webview2_executable = evidence / "msedgewebview2.exe"
        webview2_executable.write_bytes(b"test WebView2 runtime executable\n")
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
            {
                "page": page,
                "x": 0.0,
                "y": (page - 1) * 936.0,
                "width_pt": 612.0,
                "height_pt": 936.0,
            }
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
                "package_tree_sha256": binding["packaged_app"]["package_tree_sha256"],
                "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
                "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
            },
            "collector": {
                "name": "external Windows operator collector",
                "version": "1",
                "invocation_id": "test-invocation",
                "started_at_utc": "2026-07-18T00:00:00Z",
                "completed_at_utc": "2026-07-18T00:05:00Z",
                "executable_sha256": "b" * 64,
                "host_identifier_sha256": "c" * 64,
            },
            "host": {
                "windows_edition": "Windows 11 Pro",
                "windows_build": "26100.4652",
                "os_architecture": "x86_64",
                "process_architecture": "x86_64",
                "session_id": 2,
                "elevated": True,
                "process_integrity_level": "High",
                "artifact": artifact(),
            },
            "ui_automation": {
                "available": True,
                "automation_identity": "operator.test",
                "process_architecture": "x86_64",
                "artifact": artifact(),
            },
            "webview2": {
                "runtime_version": "138.0.3351.83",
                "channel": "stable",
                "architecture": "x86_64",
                "install_scope": "per_machine",
                "core_webview2_7_available": True,
                "core_webview2_16_available": True,
                "executable": self._record(webview2_executable),
                "artifact": artifact(),
            },
            "dependencies": {
                "msvc_runtime_loaded": True,
                "webview2_loader_bound": True,
                "artifact": artifact(),
            },
            "runtime": {
                "non_dev_build": True,
                "dev_tools_enabled": False,
                "launch_argv": [binding["packaged_app"]["binary"]["path"]],
                "pid": 123,
                "network_denial": {
                    "mechanism": (
                        "Windows Defender Firewall outbound block rules for exact bir.exe "
                        "and msedgewebview2.exe"
                    ),
                    "exercised": True,
                    "enforced_for_launch": True,
                    "passed": True,
                    "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
                    "webview2_executable_sha256": self._record(webview2_executable)[
                        "sha256"
                    ],
                    "rule_names": [
                        "eBIRForms candidate deny test app",
                        "eBIRForms candidate deny test webview2",
                    ],
                    "cleanup_verified": True,
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
                "print_to_pdf_hresult": "S_OK",
                "print_to_pdf_result": True,
                "artifact": artifact(),
            },
            "native_print": {
                "exercised": True,
                "passed": True,
                "completed": True,
                "printer_name": "Certification Printer",
                "job_id": "42",
                "event_record_id": 99,
                "document_name": "eBIRForms 2551Q",
                "submitted_at_utc": "2026-07-18T00:02:00Z",
                "completed_at_utc": "2026-07-18T00:03:00Z",
                "total_pages": 2,
                "completion_status": "Completed",
                "output_sha256": None,
                "webview2_print_hresult": "S_OK",
                "webview2_print_status": "Succeeded",
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
                "authenticode": {
                    "passed": True,
                    "status": "Valid",
                    "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
                    "signer_subject": "CN=Goldcoders Corp",
                    "signer_issuer": "CN=Trusted Code Signing CA",
                    "signer_serial_number": "0123456789ABCDEF",
                    "signer_thumbprint": "A" * 40,
                    "code_signing_eku": True,
                    "file_digest_algorithm": "SHA256",
                    "timestamp_signature_present": True,
                    "timestamp_subject": "CN=Timestamp Authority",
                    "timestamp_thumbprint": "B" * 40,
                    "timestamp_time_utc": "2026-07-18T00:01:00Z",
                    "timestamp_digest_algorithm": "SHA256",
                    "chain_trusted": True,
                    "signtool_policy": "/pa /all /v",
                    "artifact": artifact(),
                },
                "distribution_policy": {
                    "candidate_format": "portable_zip",
                    "distribution_track": "portable_candidate",
                    "candidate_contains_installer": False,
                    "public_release_formats": certification.PUBLIC_RELEASE_FORMATS,
                    "public_release_allows_msix": False,
                    "store_msix_policy": certification.STORE_MSIX_POLICY,
                    "msix_certification_claimed": False,
                    "public_installer_certification_claimed": False,
                    "installed_payload_certification_claimed": False,
                    "artifact": artifact(),
                },
            },
            "integrity": {
                "package_tree_sha256_before": binding["packaged_app"]["package_tree_sha256"],
                "package_tree_sha256_after": binding["packaged_app"]["package_tree_sha256"],
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
            "strict_verifier_gaps": [
                certification.NON_PROMOTIONAL_GAP,
                certification.PUBLIC_INSTALLER_GAP,
            ],
        }
        path = evidence / "windows-attestation.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def test_inspection_binds_exact_archive_binary_identity_and_renderer(self) -> None:
        binding = self._binding()

        self.assertFalse(binding["promotion_eligible"])
        self.assertFalse(binding["trusted_producer"])
        self.assertFalse(binding["certification_complete"])
        self.assertEqual(binding["packaged_app"]["renderer_bundle_sha256"], self.renderer_hash)
        self.assertEqual(binding["packaged_app"]["distribution_kind"], "portable_zip")
        self.assertIn(certification.NON_PROMOTIONAL_GAP, binding["strict_verifier_gaps"])

    def test_manifest_hash_mismatch_fails_closed(self) -> None:
        self.archive.write_bytes(self.archive.read_bytes() + b"tampered")
        with self.assertRaisesRegex(certification.EvidenceError, "size differs"):
            certification.validate_candidate_inputs(self.manifest, self.archive, self.identity)

    def test_case_colliding_archive_paths_are_rejected(self) -> None:
        archive = self.root / "collision.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("assets/INDEX.html", b"one")
            bundle.writestr("assets/index.html", b"two")
        with self.assertRaisesRegex(certification.EvidenceError, "duplicate paths"):
            certification.certification_common.extract_portable_zip(
                archive, self.root / "unsafe"
            )

    def test_reserved_windows_device_path_is_rejected(self) -> None:
        archive = self.root / "device.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("assets/CON.txt", b"reserved")
        with self.assertRaisesRegex(certification.EvidenceError, "reserved Windows device"):
            certification.certification_common.extract_portable_zip(
                archive, self.root / "unsafe-device"
            )

    def test_unicode_normalization_colliding_paths_are_rejected(self) -> None:
        archive = self.root / "unicode-collision.zip"
        with zipfile.ZipFile(archive, "w") as bundle:
            bundle.writestr("assets/caf\N{LATIN SMALL LETTER E WITH ACUTE}.txt", b"one")
            bundle.writestr("assets/cafe\N{COMBINING ACUTE ACCENT}.txt", b"two")
        with self.assertRaisesRegex(certification.EvidenceError, "Unicode-normalization"):
            certification.certification_common.extract_portable_zip(
                archive, self.root / "unsafe-unicode"
            )

    def test_portable_candidate_rejects_embedded_msix(self) -> None:
        (self.package / "forbidden.msix").write_bytes(b"not a Store package")
        self.archive.unlink()
        self._write_archive(self.archive)
        archive_record = certification.artifact_common.file_record(self.archive)
        manifest = json.loads(self.manifest.read_text())
        manifest["artifact"].update(
            byte_count=archive_record["byte_count"], sha256=archive_record["sha256"]
        )
        self.manifest.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(certification.EvidenceError, "installer artifacts"):
            self._binding()

    @mock.patch.object(certification.platform, "system", return_value="Windows")
    @mock.patch.object(certification.subprocess, "Popen")
    @mock.patch.object(certification, "_run_powershell")
    def test_probe_blocks_app_and_webview2_then_cleans_both_rules(
        self,
        powershell: mock.Mock,
        popen: mock.Mock,
        _system: mock.Mock,
    ) -> None:
        binding = self._binding()
        output = self.root / "probe"
        output.mkdir()
        webview2 = self.root / "msedgewebview2.exe"
        webview2.write_bytes(b"test WebView2 executable\n")
        process = mock.Mock()
        process.poll.return_value = None
        process.wait.return_value = 0
        popen.return_value = process

        def response(script: str, label: str, **_kwargs: object) -> str:
            if label == "Windows administrator state":
                return "True"
            if label == "Windows Firewall profile state":
                return json.dumps([{"Name": "Domain", "Enabled": True}])
            if label == "Windows Firewall cleanup verification":
                return "0"
            return ""

        powershell.side_effect = response

        probe = certification.probe_nondev_candidate(
            binding, output, 0.0, webview2
        )

        scripts = [call.args[0] for call in powershell.call_args_list]
        self.assertTrue(any("bir.exe" in script for script in scripts))
        self.assertTrue(any("msedgewebview2.exe" in script for script in scripts))
        self.assertEqual(len(probe["network_denial"]["rule_names"]), 2)
        self.assertTrue(probe["network_denial"]["cleanup_verified"])

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

    def test_msix_promotion_claim_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["package_security"]["distribution_policy"]["msix_certification_claimed"] = True
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "must not claim"):
            certification.validate_attestation(path, binding)

    def test_public_installer_claim_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["package_security"]["distribution_policy"][
            "public_installer_certification_claimed"
        ] = True
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "must not claim"):
            certification.validate_attestation(path, binding)

    def test_public_installer_gap_cannot_be_erased(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["strict_verifier_gaps"].remove(certification.PUBLIC_INSTALLER_GAP)
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "installed-tree blocker"):
            certification.validate_attestation(path, binding)

    def test_owned_pdf_verifier_is_platform_scoped_and_rerun(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        verifier = self.root / "verify-pdf"
        output_pdf = Path(value["pdf_validation"]["output"]["path"])
        report = {
            "schema_version": 1,
            "scope": "owned_windows_candidate_pdf_validation",
            "promotion_eligible": False,
            "form": certification.FORM,
            "envelope_sha256": value["preview"]["envelope_sha256"],
            "output_sha256": certification.artifact_common.file_record(output_pdf)["sha256"],
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
            "#!/usr/bin/env python3\n"
            "import sys\n"
            "assert sys.argv[3] == 'windows'\n"
            "sys.stdout.buffer.write(" + repr(encoded) + ")\n"
        )
        verifier.chmod(0o755)
        value["pdf_validation"]["verifier_executable_sha256"] = self._record(verifier)["sha256"]
        artifact = Path(value["pdf_validation"]["artifact"]["path"])
        artifact.write_bytes(encoded)
        value["pdf_validation"]["artifact"] = self._record(artifact)
        path.write_text(json.dumps(value))
        attestation, verified = certification.validate_attestation(path, binding)

        result = certification.verify_owned_pdf_artifact(path, attestation, verifier, verified)

        self.assertEqual(result["actual_page_count"], 2)

    @mock.patch.object(certification.platform, "system", return_value="Windows")
    @mock.patch.object(certification, "_run_powershell", return_value=json.dumps({"Status": "NotSigned"}))
    def test_live_verification_fails_closed_for_unsigned_candidate(
        self, _powershell: mock.Mock, _system: mock.Mock
    ) -> None:
        binding = self._binding()
        _, attestation = self._attestation(binding)
        signtool = self.root / "signtool.exe"
        signtool.write_bytes(b"test tool")
        with self.assertRaisesRegex(certification.EvidenceError, "differs"):
            certification.verify_live_windows_state(
                Path(binding["packaged_app"]["package_path"]), attestation, signtool
            )

    def test_schemas_preserve_untrusted_boundary_and_release_policy(self) -> None:
        schema_root = REPOSITORY_ROOT / "packages/form-specs/schema"
        attestation = json.loads(
            (schema_root / "windows-candidate-certification-attestation-v1.schema.json").read_text()
        )
        report = json.loads(
            (schema_root / "windows-candidate-certification-report-v1.schema.json").read_text()
        )
        self.assertEqual(attestation["properties"]["promotion_eligible"], {"const": False})
        self.assertEqual(report["properties"]["trusted_producer"], {"const": False})
        self.assertEqual(report["properties"]["promotion_satisfied"], {"const": False})
        policy = attestation["properties"]["package_security"]["properties"][
            "distribution_policy"
        ]["properties"]
        self.assertEqual(policy["public_release_allows_msix"], {"const": False})
        self.assertEqual(
            policy["public_release_formats"],
            {"const": ["signed_inno_setup_exe", "signed_msi"]},
        )
        self.assertEqual(policy["distribution_track"], {"const": "portable_candidate"})
        self.assertEqual(
            policy["public_installer_certification_claimed"], {"const": False}
        )
        self.assertEqual(
            policy["installed_payload_certification_claimed"], {"const": False}
        )
        self.assertEqual(
            attestation["properties"]["rollback"]["properties"]["cases"]["minItems"],
            len(certification.ROLLBACK_CASES),
        )
        self.assertIn(
            certification.PUBLIC_INSTALLER_GAP,
            json.dumps(report["properties"]["strict_verifier_gaps"]),
        )
        self.assertNotIn("release_ready", json.dumps(report))


if __name__ == "__main__":
    unittest.main()
