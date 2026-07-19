from __future__ import annotations

import importlib.util
import json
import os
import plistlib
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/macos_candidate_collector.py"
SPEC = importlib.util.spec_from_file_location("macos_candidate_collector", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
collector = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(collector)


class MacosCandidateCollectorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name).resolve()
        self.source_revision = "a" * 40
        self.app = self.root / "source/eBIRForms.app"
        renderer = self.app / collector.native_driver.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.app / "Contents/MacOS/bir"
        binary.parent.mkdir(parents=True)
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        info = self.app / "Contents/Info.plist"
        info.parent.mkdir(parents=True, exist_ok=True)
        info.write_bytes(plistlib.dumps({"CFBundleExecutable": "bir"}))
        self.renderer_hash = collector.native_driver.tree_hash(renderer)
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
        encoded_identity = (json.dumps(identity, sort_keys=True) + "\n").encode()
        self.identity.write_bytes(encoded_identity)
        bundled_identity = self.app / collector.native_driver.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(encoded_identity)
        self.archive = self.root / "candidate.zip"
        with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(item for item in self.app.rglob("*") if item.is_file()):
                relative = path.relative_to(self.app.parent).as_posix()
                entry = zipfile.ZipInfo(relative)
                entry.create_system = 3
                entry.external_attr = (path.stat().st_mode & 0xFFFF) << 16
                bundle.writestr(entry, path.read_bytes())
        self.manifest = self.root / "candidate-manifest.json"
        archive_record = collector.native_driver.file_record(self.archive)
        identity_record = collector.native_driver.file_record(self.identity)
        self.manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": collector.certification.CANDIDATE_SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": collector.FORM,
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
        self.binding = collector.candidate_binding(
            self.manifest,
            self.archive,
            self.identity,
            self.root / "extracted",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def _record(self, path: Path) -> dict:
        return collector.native_driver.file_record(path)

    def _rollback_bundle(self) -> tuple[Path, dict]:
        evidence = self.root / "rollback"
        evidence.mkdir(exist_ok=True)
        destination_before = evidence / "destination-before.bin"
        destination_after = evidence / "destination-after.bin"
        draft_before = evidence / "draft-before.json"
        draft_after = evidence / "draft-after.json"
        destination_before.write_bytes(b"preserved destination\n")
        destination_after.write_bytes(b"preserved destination\n")
        draft_before.write_text('{"unchanged":true}\n')
        draft_after.write_text('{"unchanged":true}\n')
        temporary = evidence / "temporary-files.json"
        temporary.write_text('{"remaining":[]}\n')
        cases = []
        for name in sorted(collector.certification.ROLLBACK_CASES):
            artifact = evidence / f"{name}.json"
            artifact.write_text(json.dumps({"case": name, "passed": True}) + "\n")
            cases.append({"name": name, "passed": True, "artifact": self._record(artifact)})
        value = {
            "schema_version": 1,
            "scope": collector.ROLLBACK_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "candidate": collector.expected_candidate(self.binding),
            "integrity": {
                "destination_before": self._record(destination_before),
                "destination_after": self._record(destination_after),
                "draft_before": self._record(draft_before),
                "draft_after": self._record(draft_after),
                "temporary_files_manifest": self._record(temporary),
            },
            "cases": cases,
            "strict_verifier_gaps": ["rollback producer is not registered as trusted"],
        }
        path = evidence / "rollback-bundle.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def _runtime_observation(self, destination: Path) -> dict:
        record = self._record(destination)
        geometry_page = {
            "x": 0.0,
            "y": 0.0,
            "width": 816.0,
            "height": 1248.0,
            "client_width": 816.0,
            "client_height": 1248.0,
            "scroll_width": 816.0,
            "scroll_height": 1248.0,
            "descendant_overflow_x": 0,
            "descendant_overflow_y": 0,
            "descendant_clipped_x": 0,
            "descendant_clipped_y": 0,
        }
        report = {
            "page_count": 2,
            "page_width_pt": 612.0,
            "page_height_pt": 936.0,
            "pages": [geometry_page, geometry_page | {"y": 1248.0}],
        }
        return {
            "schema_version": 1,
            "scope": collector.RUNTIME_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "collector_challenge_sha256": "b" * 64,
            "form_code": "2551Q",
            "form_revision": "2018",
            "document_run_id_sha256": "c" * 64,
            "envelope_sha256": "d" * 64,
            "render_epoch": 1,
            "readiness_revision": 1,
            "issued_nonce": 7,
            "preflight_consumptions": [7],
            "backend_completion_nonce": 7,
            "started_at_unix_ms": 1,
            "completed_at_unix_ms": 2,
            "geometry_reports": [report, report],
            "output": {
                "kind": "pdf_export_succeeded",
                "wkpdf_pages": [
                    {"page": 1, "byte_count": 10, "sha256": "e" * 64},
                    {"page": 2, "byte_count": 11, "sha256": "f" * 64},
                ],
                "output_pdf_sha256": record["sha256"],
                "output_pdf_byte_count": record["byte_count"],
                "pdf_validation": {
                    "page_count": 2,
                    "width_points": 612.0,
                    "height_points": 936.0,
                    "content_nonempty": True,
                    "validated_by": "bir-print::html_output::validate_pdf_file",
                },
                "destination_before": {"status": "absent"},
                "destination_after": {"status": "file", "sha256": record["sha256"]},
                "temporary_file_remaining": False,
            },
            "strict_verifier_gaps": collector.RUNTIME_GAPS,
        }

    def test_complete_rollback_bundle_is_candidate_bound_and_closed(self) -> None:
        path, _ = self._rollback_bundle()
        loaded = collector.load_rollback_bundle(path, self.binding)
        self.assertEqual(len(loaded["cases"]), 11)
        self.assertEqual(loaded["integrity"]["destination_before"]["sha256"], loaded[
            "integrity"
        ]["destination_after"]["sha256"])

    def test_missing_rollback_case_and_candidate_drift_fail_closed(self) -> None:
        path, value = self._rollback_bundle()
        value["cases"].pop()
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "incomplete"):
            collector.load_rollback_bundle(path, self.binding)
        path, value = self._rollback_bundle()
        value["candidate"]["source_revision"] = "f" * 40
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "exact candidate"):
            collector.load_rollback_bundle(path, self.binding)

    def test_runtime_observation_binds_challenge_nonce_geometry_and_pdf(self) -> None:
        destination = self.root / "export.pdf"
        destination.write_bytes(b"two-page PDF placeholder")
        observation = self._runtime_observation(destination)
        path = self.root / "observation.json"
        path.write_text(json.dumps(observation))
        accepted = collector.validate_runtime_observation(
            path,
            challenge_sha256="b" * 64,
            expected_kind="pdf_export_succeeded",
            destination=destination,
        )
        self.assertEqual(accepted["issued_nonce"], 7)
        observation["preflight_consumptions"] = [7, 7]
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "one-use nonce"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
            )

    def test_runtime_observation_rejects_wrong_challenge_and_nonfinite_geometry(self) -> None:
        destination = self.root / "export.pdf"
        destination.write_bytes(b"two-page PDF placeholder")
        observation = self._runtime_observation(destination)
        path = self.root / "observation.json"
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "constants or challenge"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="0" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
            )
        observation["geometry_reports"][0]["pages"][0]["width"] = float("inf")
        observation["geometry_reports"][1]["pages"][0]["width"] = float("inf")
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "invalid geometry"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
            )

    def test_runtime_observation_closes_epoch_destination_and_all_geometry(self) -> None:
        destination = self.root / "export.pdf"
        destination.write_bytes(b"two-page PDF placeholder")
        observation = self._runtime_observation(destination)
        destination_before_sha256 = "a" * 64
        observation["output"]["destination_before"] = {
            "status": "file",
            "sha256": destination_before_sha256,
        }
        path = self.root / "observation.json"
        path.write_text(json.dumps(observation))
        collector.validate_runtime_observation(
            path,
            challenge_sha256="b" * 64,
            expected_kind="pdf_export_succeeded",
            destination=destination,
            destination_before_sha256=destination_before_sha256,
        )

        observation["render_epoch"] = 0
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "render_epoch must be positive"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
                destination_before_sha256=destination_before_sha256,
            )

        observation["render_epoch"] = 1
        observation["geometry_reports"][0]["pages"][0]["client_width"] = 0
        observation["geometry_reports"][1]["pages"][0]["client_width"] = 0
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "non-positive geometry"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
                destination_before_sha256=destination_before_sha256,
            )

    def test_runtime_destination_snapshot_is_closed_and_challenge_bound(self) -> None:
        destination = self.root / "export.pdf"
        destination.write_bytes(b"two-page PDF placeholder")
        observation = self._runtime_observation(destination)
        observation["output"]["destination_before"] = {
            "status": "file",
            "sha256": "a" * 64,
            "unexpected": True,
        }
        path = self.root / "observation.json"
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "schema mismatch"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
                destination_before_sha256="a" * 64,
            )

        observation["output"]["destination_before"] = {
            "status": "file",
            "sha256": "a" * 64,
        }
        path.write_text(json.dumps(observation))
        with self.assertRaisesRegex(collector.EvidenceError, "collector challenge"):
            collector.validate_runtime_observation(
                path,
                challenge_sha256="b" * 64,
                expected_kind="pdf_export_succeeded",
                destination=destination,
                destination_before_sha256="0" * 64,
            )

    def test_save_chooser_observation_is_closed_and_exact_window_bound(self) -> None:
        preview = self.root / "preview.png"
        chooser = self.root / "chooser.png"
        value = {
            "preview": {
                "id": 10,
                "title": "2551Q HTML Form Preview",
                "x": 0.0,
                "y": 0.0,
                "width": 900.0,
                "height": 700.0,
            },
            "dialog": {
                "id": 11,
                "title": "Save",
                "x": 50.0,
                "y": 50.0,
                "width": 600.0,
                "height": 400.0,
            },
            "previewScreenshot": str(preview),
            "dialogScreenshot": str(chooser),
        }
        accepted = collector.validate_save_chooser_observation(
            value, preview_screenshot=preview, chooser_screenshot=chooser
        )
        self.assertEqual(accepted["dialog"]["id"], 11)
        value["preview"]["title"] = "Unrelated Preview"
        with self.assertRaisesRegex(collector.EvidenceError, "identify 2551Q"):
            collector.validate_save_chooser_observation(
                value, preview_screenshot=preview, chooser_screenshot=chooser
            )

    def test_exact_pid_toolbar_and_print_dialog_records_are_closed(self) -> None:
        export = {
            "initial": {"x": 10.0, "y": 20.0, "width": 900.0, "height": 700.0},
            "active": {"x": 5.0, "y": 15.0, "width": 1000.0, "height": 750.0},
            "firstClickX": 727.0,
            "firstClickY": 78.0,
            "secondClickX": 822.0,
            "secondClickY": 73.0,
        }
        self.assertEqual(collector.validate_export_automation(export)["secondClickX"], 822.0)
        export["secondClickX"] = 800.0
        with self.assertRaisesRegex(collector.EvidenceError, "target Export PDF"):
            collector.validate_export_automation(export)

        print_dialog = {
            "previewWindowId": 10,
            "previewWindowWidth": 1000.0,
            "previewWindowHeight": 750.0,
            "clickX": 921.0,
            "clickY": 73.0,
            "dialogWindowId": 11,
            "dialogWidth": 700.0,
            "dialogHeight": 500.0,
            "dialogObserved": True,
        }
        self.assertTrue(
            collector.validate_print_dialog_observation(print_dialog)["dialogObserved"]
        )
        print_dialog["dialogWindowId"] = 10
        with self.assertRaisesRegex(collector.EvidenceError, "reused the preview"):
            collector.validate_print_dialog_observation(print_dialog)

    def test_private_output_directory_and_live_print_acknowledgement_fail_closed(self) -> None:
        output = collector.private_output_directory(self.root / "evidence")
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)
        (output / "unexpected").write_text("data")
        with self.assertRaisesRegex(collector.EvidenceError, "absent or an empty"):
            collector.private_output_directory(output)
        arguments = collector.build_parser().parse_args(
            [
                "--candidate-manifest", str(self.manifest),
                "--candidate-archive", str(self.archive),
                "--renderer-identity", str(self.identity),
                "--pdf-verifier", str(self.app / "Contents/MacOS/bir"),
                "--rollback-bundle", str(self.root / "missing.json"),
                "--output-dir", str(self.root / "other"),
                "--printer", "Example",
            ]
        )
        original = collector.platform.system
        collector.platform.system = lambda: "Darwin"
        try:
            with self.assertRaisesRegex(collector.EvidenceError, "allow-live-print"):
                collector.collect(arguments)
        finally:
            collector.platform.system = original

    def test_print_helper_never_cancels_or_activates_the_native_dialog(self) -> None:
        self.assertNotIn("try postEscape()", collector.OPEN_PRINT_DIALOG_SWIFT)
        self.assertIn("dialogObserved", collector.OPEN_PRINT_DIALOG_SWIFT)
        self.assertIn("/usr/sbin/screencapture", collector.DIALOG_WATCHER_SWIFT)

    def test_candidate_pid_matching_excludes_the_sandbox_wrapper(self) -> None:
        binary = self.app / "Contents/MacOS/bir"
        self.assertTrue(collector.command_executes_binary(str(binary.resolve()), binary))
        self.assertTrue(
            collector.command_executes_binary(
                f"{binary.resolve()} --ordinary-runtime-argument", binary
            )
        )
        self.assertFalse(
            collector.command_executes_binary(
                f"/usr/bin/sandbox-exec -p policy {binary.resolve()}", binary
            )
        )


if __name__ == "__main__":
    unittest.main()
