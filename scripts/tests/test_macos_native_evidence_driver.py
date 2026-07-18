from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from unittest import mock
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "scripts/macos_native_evidence_driver.py"
SPEC = importlib.util.spec_from_file_location("macos_native_evidence_driver", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)


class MacosNativeEvidenceDriverTests(unittest.TestCase):
    def test_tree_hash_matches_the_sorted_renderer_manifest_algorithm(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "nested").mkdir()
            (root / "z.txt").write_bytes(b"z")
            (root / "nested/a.txt").write_bytes(b"a")

            digest = hashlib.sha256()
            for relative, payload in (
                ("nested/a.txt", b"a"),
                ("z.txt", b"z"),
            ):
                digest.update(relative.encode())
                digest.update(b"\0file\0")
                digest.update(hashlib.sha256(payload).hexdigest().encode())
                digest.update(b"\n")

            self.assertEqual(driver.tree_hash(root), digest.hexdigest())

    def test_tree_hash_rejects_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "file").write_text("value", encoding="utf-8")
            (root / "link").symlink_to(root / "file")
            with self.assertRaisesRegex(driver.EvidenceError, "symlink"):
                driver.tree_hash(root)

    def test_envelope_requires_exact_2551q_revision(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.json"
            path.write_text(
                json.dumps(
                    {
                        "schema_version": "1.0",
                        "form": {"code": "2550Q", "version": "2024"},
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(driver.EvidenceError, "2551Q:2018"):
                driver.validate_envelope(path)

    def test_build_identity_must_match_independently_hashed_renderer(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "identity.json"
            expected = "a" * 64
            path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "scope": "build_time_non_promotional_identity",
                        "promotion_eligible": False,
                        "offline_verification_passed": True,
                        "renderer_bundle_relative_path": "assets/form-renderer",
                        "renderer_bundle_sha256": expected,
                        "source_revision": {
                            "status": "unavailable",
                            "reason": "test fixture",
                        },
                    }
                ),
                encoding="utf-8",
            )
            self.assertEqual(
                driver.validate_build_identity(path, expected)["renderer_bundle_sha256"],
                expected,
            )
            with self.assertRaisesRegex(driver.EvidenceError, "differs"):
                driver.validate_build_identity(path, "b" * 64)

    def test_transcript_can_never_claim_promotion_or_trust(self) -> None:
        for forbidden_key in ("promotion_eligible", "trusted_producer"):
            with self.subTest(forbidden_key=forbidden_key), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "transcript.json"
                transcript = {
                    "schema_version": driver.SCHEMA_VERSION,
                    "scope": driver.SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": {"code": "2551Q", "revision": "2018"},
                    "strict_verifier_gaps": ["test gap"],
                }
                transcript[forbidden_key] = True
                path.write_text(json.dumps(transcript), encoding="utf-8")
                with self.assertRaisesRegex(driver.EvidenceError, "never"):
                    driver.verify_transcript(path)

    def test_gpui_export_requeries_exact_pid_geometry_after_activation(self) -> None:
        # GPUI paints the toolbar but does not expose its controls as AXButton
        # children. Bind both measurements to the exact launched process and
        # recalculate the second click after the pinned backend reflows.
        source = driver.NATIVE_EXPORT_SWIFT
        self.assertIn("kCGWindowOwnerPID", source)
        self.assertIn('name.contains("2551Q HTML Form Preview")', source)
        self.assertIn("let initial = try waitForPreview(pid: pid)", source)
        self.assertIn("let active = try waitForPreview(pid: pid, attempts: 20)", source)
        self.assertIn("let secondPoint = exportPoint(active)", source)
        self.assertIn("try click(secondPoint)", source)
        self.assertIn("geometry.width - 183.0", source)
        self.assertNotIn("System Events", source)

    @mock.patch.object(driver.subprocess, "run")
    def test_native_export_passes_pid_and_destination_out_of_band(
        self, run: mock.Mock
    ) -> None:
        run.return_value = mock.Mock(
            returncode=0,
            stdout=json.dumps(
                {
                    "initial": {"x": 40, "y": 940, "width": 1200, "height": 932},
                    "active": {"x": 900, "y": 39, "width": 900, "height": 1129},
                    "firstClickX": 1057,
                    "firstClickY": 998,
                    "secondClickX": 1617,
                    "secondClickY": 97,
                }
            ),
            stderr="",
        )
        destination = Path("/tmp/reviewed export.pdf")

        record = driver.run_native_export(4321, destination, timeout=15.0)

        self.assertEqual(record["active"]["width"], 900)
        command = run.call_args.args[0]
        environment = run.call_args.kwargs["env"]
        self.assertEqual(command[:3], ["/usr/bin/xcrun", "swift", "-e"])
        self.assertEqual(environment["EBIR_NATIVE_EVIDENCE_PID"], "4321")
        self.assertEqual(
            environment["EBIR_NATIVE_EVIDENCE_DESTINATION"], str(destination)
        )

    def test_system_print_uses_the_same_window_relative_toolbar_contract(self) -> None:
        self.assertIn("set windowPosition to position of targetWindow", driver.PRINT_CANCEL_SCRIPT)
        self.assertIn("set windowSize to size of targetWindow", driver.PRINT_CANCEL_SCRIPT)
        self.assertIn("- 210", driver.PRINT_CANCEL_SCRIPT)
        self.assertNotIn("first button of targetWindow", driver.PRINT_CANCEL_SCRIPT)

    def test_failure_observation_binds_and_preserves_the_existing_destination(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            destination = root / "preserved.pdf"
            destination.write_bytes(b"existing destination")
            before = driver.file_record(destination)
            snapshot = {"state": "file", "sha256": before["sha256"]}
            observation_path = root / "native-output-failure-2.failure.json"
            observation_path.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "scope": "development_diagnostic",
                        "promotion_eligible": False,
                        "outcome": "export_failed",
                        "form_code": "2551Q",
                        "form_revision": "2018",
                        "nonce": 2,
                        "destination": str(destination),
                        "destination_before": snapshot,
                        "destination_after": snapshot,
                        "temporary_file_remaining": False,
                        "error": "reviewed induced failure",
                    }
                ),
                encoding="utf-8",
            )

            observation = driver.validate_failure_observation(
                observation_path,
                destination=destination,
                destination_before=before,
            )

            self.assertEqual(observation["outcome"], "export_failed")

            observation["destination_after"] = {"state": "absent"}
            observation_path.write_text(json.dumps(observation), encoding="utf-8")
            with self.assertRaisesRegex(driver.EvidenceError, "preserve"):
                driver.validate_failure_observation(
                    observation_path,
                    destination=destination,
                    destination_before=before,
                )


if __name__ == "__main__":
    unittest.main()
