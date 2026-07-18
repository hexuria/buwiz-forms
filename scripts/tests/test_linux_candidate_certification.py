from __future__ import annotations

import importlib.util
import json
import os
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "scripts/linux_candidate_certification.py"
SPEC = importlib.util.spec_from_file_location("linux_candidate_certification", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
certification = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(certification)


class LinuxCandidateCertificationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_revision = "a" * 40
        self.application = self.root / "source/eBIRForms-Linux-x64"
        renderer = self.application / certification.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.application / "bir"
        binary.write_text("#!/bin/sh\nwhile :; do sleep 1; done\n")
        binary.chmod(0o755)
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
        bundled_identity = self.application / certification.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(identity_bytes)
        self.archive = self.root / "eBIRForms-Linux-x64-candidate.tar.gz"
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
                    "platform": "linux",
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
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(self.application, arcname=self.application.name, recursive=True)

    def _record(self, path: Path) -> dict:
        return certification.common.file_record(path)

    def _binding(self) -> dict:
        return certification.inspect_candidate(
            self.manifest, self.archive, self.identity, self.root / "inspection"
        )

    def _attestation(self, binding: dict) -> tuple[Path, dict]:
        evidence = self.root / "attestation"
        evidence.mkdir()
        generic = evidence / "observation.log"
        generic.write_text("observed\n")
        generic_record = self._record(generic)
        boundary = evidence / "package-boundary.log"
        boundary.write_text("portable candidate is not the final release package\n")
        runtime = evidence / "runtime.log"
        runtime.write_text("non-dev packaged runtime\n")
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
        pdf_pages = [
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

        runs = {}
        for backend, expected in certification.BACKENDS.items():
            output_pdf = evidence / f"{backend}-toolbar-export.pdf"
            output_pdf.write_bytes(f"fake {backend} export\n".encode())
            destination_before = evidence / f"{backend}-destination-before.bin"
            destination_after = evidence / f"{backend}-destination-after.bin"
            destination_before.write_bytes(b"preserved destination\n")
            destination_after.write_bytes(b"preserved destination\n")
            draft_before = evidence / f"{backend}-draft-before.json"
            draft_after = evidence / f"{backend}-draft-after.json"
            draft_before.write_text('{"unchanged":true}\n')
            draft_after.write_text('{"unchanged":true}\n')
            temporary_files = evidence / f"{backend}-temporary-files.json"
            temporary_files.write_text('{"remaining":[]}\n')
            verifier_artifact = evidence / f"{backend}-pdf-verifier.json"
            verifier_artifact.write_text("{}\n")
            display_artifact = evidence / f"{backend}-display.log"
            display_artifact.write_text(f"{expected['compositor']} ready\n")
            runs[backend] = {
                "exercised": True,
                "passed": True,
                "display_server": backend,
                "host_strategy": expected["host_strategy"],
                "app_owned_window": True,
                "external_browser": False,
                "window_title": expected["window_title"],
                "launch_argv": [binding["installed_candidate"]["binary"]["path"]],
                "pid": 100 if backend == "x11" else 200,
                "display_environment": {
                    "display_variable": expected["display_variable"],
                    "display_value": ":99" if backend == "x11" else "wayland-1",
                    "runtime_directory": None if backend == "x11" else "/run/user/1000",
                    "compositor": expected["compositor"],
                    "compositor_version": "1.0",
                    "gtk_version": "3.24.0",
                    "webkitgtk_version": "2.44.0",
                    "artifact": self._record(display_artifact),
                },
                "network_denial": {
                    "mechanism": "bubblewrap --unshare-net",
                    "exercised": True,
                    "enforced_for_launch": True,
                    "passed": True,
                    "host_namespace_inode": "net:[100]",
                    "candidate_namespace_inode": f"net:[{200 if backend == 'x11' else 300}]",
                    "artifact": dict(generic_record),
                },
                "lifecycle": {
                    "opened": True,
                    "preview_ready": True,
                    "close_reopen": True,
                    "clean_shutdown": True,
                    "artifact": dict(generic_record),
                },
                "preview": {
                    "exercised": True,
                    "passed": True,
                    "document_run_id": f"{backend}-run-1",
                    "envelope_sha256": ("d" if backend == "x11" else "e") * 64,
                    "nonce": 7 if backend == "x11" else 8,
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
                    "artifact": dict(generic_record),
                },
                "toolbar_export": {
                    "exercised": True,
                    "passed": True,
                    "control": "Export PDF",
                    "save_chooser_exercised": True,
                    "destination_path": str(output_pdf.resolve()),
                    "nonce": 7 if backend == "x11" else 8,
                    "artifact": dict(generic_record),
                },
                "native_print": {
                    "exercised": True,
                    "passed": True,
                    "completed": True,
                    "printer_name": f"Certification_{backend}",
                    "job_id": f"Certification_{backend}-42",
                    "artifact": dict(generic_record),
                },
                "pdf_validation": {
                    "exercised": True,
                    "passed": True,
                    "output": self._record(output_pdf),
                    "expected_page_count": 2,
                    "actual_page_count": 2,
                    "pages": pdf_pages,
                    "content_nonempty": True,
                    "validated_by": "bir-print::html_output::validate_pdf_file",
                    "verifier_executable_sha256": "f" * 64,
                    "artifact": self._record(verifier_artifact),
                },
                "integrity": {
                    "installed_root_sha256_before": binding["installed_candidate"][
                        "installed_root_sha256"
                    ],
                    "installed_root_sha256_after": binding["installed_candidate"][
                        "installed_root_sha256"
                    ],
                    "destination_before": self._record(destination_before),
                    "destination_after": self._record(destination_after),
                    "draft_before": self._record(draft_before),
                    "draft_after": self._record(draft_after),
                    "temporary_files_manifest": self._record(temporary_files),
                },
                "rollback": {
                    "cases": [
                        {"name": name, "passed": True, "artifact": dict(generic_record)}
                        for name in sorted(certification.ROLLBACK_CASES)
                    ],
                    "destination_preserved": True,
                    "temporary_files_remaining": 0,
                    "draft_unchanged": True,
                },
                "artifact": dict(generic_record),
            }

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
                "installed_root_sha256": binding["installed_candidate"]["installed_root_sha256"],
                "installed_binary_sha256": binding["installed_candidate"]["binary"]["sha256"],
                "renderer_bundle_sha256": binding["installed_candidate"]["renderer_bundle_sha256"],
                "renderer_identity_sha256": binding["installed_candidate"][
                    "bundled_renderer_identity"
                ]["sha256"],
                "installation_method": "secure_portable_tar_extraction",
            },
            "collector": {
                "name": "external Linux operator collector",
                "version": "1",
                "invocation_id": "test-invocation",
                "started_at_utc": "2026-07-18T00:00:00Z",
                "completed_at_utc": "2026-07-18T00:05:00Z",
                "executable_sha256": "b" * 64,
                "host_identifier_sha256": "c" * 64,
            },
            "runtime": {
                "non_dev_build": True,
                "dev_tools_enabled": False,
                "installed_root_sha256": binding["installed_candidate"]["installed_root_sha256"],
                "installed_binary_sha256": binding["installed_candidate"]["binary"]["sha256"],
                "assets_tree_sha256": binding["installed_candidate"]["assets_tree_sha256"],
                "renderer_bundle_sha256": binding["installed_candidate"]["renderer_bundle_sha256"],
                "renderer_identity_sha256": binding["installed_candidate"][
                    "bundled_renderer_identity"
                ]["sha256"],
                "artifact": self._record(runtime),
            },
            "display_runs": runs,
            "package_boundary": {
                "portable_candidate_verified": True,
                "final_release_deb_verified": False,
                "final_release_tarball_verified": False,
                "release_package_signature_verified": False,
                "artifact": self._record(boundary),
            },
            "strict_verifier_gaps": [
                certification.NON_PROMOTIONAL_GAP,
                certification.RELEASE_PACKAGE_GAP,
            ],
        }
        path = evidence / "linux-attestation.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def test_inspection_binds_archive_identity_binary_and_renderer(self) -> None:
        binding = self._binding()
        self.assertFalse(binding["promotion_eligible"])
        self.assertFalse(binding["trusted_producer"])
        self.assertFalse(binding["certification_complete"])
        self.assertEqual(
            binding["installed_candidate"]["renderer_bundle_sha256"], self.renderer_hash
        )
        self.assertEqual(
            binding["installed_candidate"]["installation_method"],
            "secure_portable_tar_extraction",
        )
        self.assertFalse(binding["package_boundary"]["final_release_deb_verified"])

    def test_manifest_hash_mismatch_fails_closed(self) -> None:
        self.archive.write_bytes(self.archive.read_bytes() + b"tampered")
        with self.assertRaisesRegex(certification.EvidenceError, "size differs"):
            certification.validate_candidate_inputs(self.manifest, self.archive, self.identity)

    def test_symlinked_candidate_input_is_rejected(self) -> None:
        link = self.root / "manifest-link.json"
        link.symlink_to(self.manifest)
        with self.assertRaisesRegex(certification.EvidenceError, "non-symlink"):
            certification.validate_candidate_inputs(link, self.archive, self.identity)

    def test_archive_path_traversal_is_rejected(self) -> None:
        archive = self.root / "traversal.tar.gz"
        with tarfile.open(archive, "w:gz") as bundle:
            info = tarfile.TarInfo("../escape")
            info.size = 3
            bundle.addfile(info, fileobj=__import__("io").BytesIO(b"bad"))
        with self.assertRaisesRegex(certification.EvidenceError, "escapes"):
            certification.extract_candidate_archive(archive, self.root / "unsafe")

    def test_archive_link_is_rejected(self) -> None:
        archive = self.root / "link.tar.gz"
        with tarfile.open(archive, "w:gz") as bundle:
            directory = tarfile.TarInfo("eBIRForms-Linux-x64")
            directory.type = tarfile.DIRTYPE
            bundle.addfile(directory)
            link = tarfile.TarInfo("eBIRForms-Linux-x64/bir")
            link.type = tarfile.SYMTYPE
            link.linkname = "/bin/sh"
            bundle.addfile(link)
        with self.assertRaisesRegex(certification.EvidenceError, "unsafe member"):
            certification.extract_candidate_archive(archive, self.root / "unsafe-link")

    def test_complete_dual_backend_attestation_is_closed_and_non_promotional(self) -> None:
        binding = self._binding()
        path, _ = self._attestation(binding)
        attestation, verified = certification.validate_attestation(path, binding)
        self.assertFalse(attestation["promotion_eligible"])
        self.assertFalse(attestation["trusted_producer"])
        self.assertGreater(len(verified), 50)

    def test_missing_wayland_run_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        del value["display_runs"]["wayland"]
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "schema mismatch"):
            certification.validate_attestation(path, binding)

    def test_wrong_wayland_host_strategy_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["display_runs"]["wayland"]["host_strategy"] = "GpuiWryChild"
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "wrong.*host strategy"):
            certification.validate_attestation(path, binding)

    def test_network_namespace_must_differ_from_host(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        denial = value["display_runs"]["x11"]["network_denial"]
        denial["candidate_namespace_inode"] = denial["host_namespace_inode"]
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "separate network namespace"):
            certification.validate_attestation(path, binding)

    def test_final_release_package_claim_is_rejected(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["package_boundary"]["final_release_deb_verified"] = True
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "must remain false"):
            certification.validate_attestation(path, binding)

    def test_missing_rollback_case_fails_closed_per_backend(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        value["display_runs"]["wayland"]["rollback"]["cases"].pop()
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(certification.EvidenceError, "incomplete"):
            certification.validate_attestation(path, binding)

    def test_tampered_retained_artifact_fails_closed(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        artifact = Path(value["display_runs"]["x11"]["preview"]["artifact"]["path"])
        artifact.write_text("tampered\n")
        with self.assertRaisesRegex(certification.EvidenceError, "changed"):
            certification.validate_attestation(path, binding)

    def test_owned_pdf_verifier_is_replayed_for_linux_and_both_backends(self) -> None:
        binding = self._binding()
        path, value = self._attestation(binding)
        reports = {}
        for backend in certification.BACKENDS:
            pdf = value["display_runs"][backend]["pdf_validation"]
            reports[Path(pdf["output"]["path"]).name] = {
                "schema_version": 1,
                "scope": "owned_linux_candidate_pdf_validation",
                "promotion_eligible": False,
                "form": certification.FORM,
                "envelope_sha256": value["display_runs"][backend]["preview"][
                    "envelope_sha256"
                ],
                "output_sha256": pdf["output"]["sha256"],
                "expected_page_count": 2,
                "actual_page_count": 2,
                "width_points": 612.0,
                "height_points": 936.0,
                "content_nonempty": True,
                "validated_by": "bir-print::html_output::validate_pdf_file",
                "pages": pdf["pages"],
            }
        verifier = self.root / "fake-owned-verifier.py"
        verifier.write_text(
            "#!/usr/bin/env python3\n"
            "import json, pathlib, sys\n"
            f"reports = {reports!r}\n"
            "assert sys.argv[3] == 'linux'\n"
            "print(json.dumps(reports[pathlib.Path(sys.argv[1]).name], separators=(',', ':')))\n"
        )
        verifier.chmod(0o755)
        verifier_hash = self._record(verifier)["sha256"]
        for backend in certification.BACKENDS:
            pdf = value["display_runs"][backend]["pdf_validation"]
            pdf["verifier_executable_sha256"] = verifier_hash
            encoded = (json.dumps(reports[Path(pdf["output"]["path"]).name], separators=(",", ":")) + "\n").encode()
            artifact = Path(pdf["artifact"]["path"])
            artifact.write_bytes(encoded)
            pdf["artifact"] = self._record(artifact)
        path.write_text(json.dumps(value))
        attestation, verified = certification.validate_attestation(path, binding)
        result = certification.verify_owned_pdf_artifacts(path, attestation, verifier, verified)
        self.assertEqual(set(result["backends"]), {"x11", "wayland"})
        self.assertEqual(result["verifier_executable_sha256"], verifier_hash)

    @mock.patch.object(certification.platform, "system", return_value="Darwin")
    def test_live_verification_is_linux_only(self, _system: mock.Mock) -> None:
        with self.assertRaisesRegex(certification.EvidenceError, "must run on Linux"):
            certification.verify_live_linux_state(self.application, {})

    def test_schemas_preserve_untrusted_candidate_and_release_package_boundary(self) -> None:
        schema_root = REPOSITORY_ROOT / "packages/form-specs/schema"
        attestation = json.loads(
            (schema_root / "linux-candidate-certification-attestation-v1.schema.json").read_text()
        )
        report = json.loads(
            (schema_root / "linux-candidate-certification-report-v1.schema.json").read_text()
        )
        self.assertEqual(attestation["properties"]["promotion_eligible"], {"const": False})
        self.assertEqual(report["properties"]["trusted_producer"], {"const": False})
        self.assertEqual(report["properties"]["promotion_satisfied"], {"const": False})
        self.assertEqual(
            attestation["properties"]["package_boundary"]["properties"][
                "final_release_deb_verified"
            ],
            {"const": False},
        )
        self.assertNotIn("release_ready", json.dumps(report))


if __name__ == "__main__":
    unittest.main()
