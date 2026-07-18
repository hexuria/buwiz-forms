from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
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


if __name__ == "__main__":
    unittest.main()
