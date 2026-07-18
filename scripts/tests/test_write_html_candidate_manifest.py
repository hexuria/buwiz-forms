from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = REPOSITORY_ROOT / "scripts/write_html_candidate_manifest.py"
SPEC = importlib.util.spec_from_file_location("write_html_candidate_manifest", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
candidate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(candidate)


class HtmlCandidateManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.revision = "a" * 40
        self.artifact = self.root / "candidate.zip"
        self.artifact.write_bytes(b"exact candidate bytes\n")
        self.identity = self.root / "form-renderer-build-identity.json"
        self.identity.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": "build_time_non_promotional_identity",
                    "promotion_eligible": False,
                    "offline_verification_passed": True,
                    "renderer_bundle_relative_path": "form-renderer",
                    "renderer_bundle_sha256": "b" * 64,
                    "source_revision": {
                        "status": "observed",
                        "value": self.revision,
                    },
                }
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_manifest_binds_candidate_without_claiming_promotion(self) -> None:
        manifest = candidate.build_manifest(
            platform="macos",
            architecture="universal",
            source_revision=self.revision,
            artifact_path=self.artifact,
            renderer_identity_path=self.identity,
        )

        self.assertFalse(manifest["promotion_eligible"])
        self.assertFalse(manifest["trusted_producer"])
        self.assertEqual(manifest["form"], {"code": "2551Q", "revision": "2018"})
        self.assertEqual(
            manifest["artifact"]["sha256"], candidate.sha256_file(self.artifact)
        )
        self.assertFalse(
            manifest["release_policy"]["candidate_build_requires_release_ready"]
        )
        self.assertTrue(
            manifest["release_policy"]["tagged_release_still_requires_release_ready"]
        )

    def test_manifest_rejects_identity_without_clean_source_revision(self) -> None:
        identity = json.loads(self.identity.read_text(encoding="utf-8"))
        identity["source_revision"] = {
            "status": "unavailable",
            "reason": "dirty source",
        }
        self.identity.write_text(json.dumps(identity), encoding="utf-8")

        with self.assertRaisesRegex(
            candidate.CandidateManifestError,
            "not bound to a clean source revision",
        ):
            candidate.build_manifest(
                platform="linux",
                architecture="x86_64",
                source_revision=self.revision,
                artifact_path=self.artifact,
                renderer_identity_path=self.identity,
            )

    def test_manifest_rejects_source_revision_mismatch(self) -> None:
        with self.assertRaisesRegex(
            candidate.CandidateManifestError,
            "differs from the candidate",
        ):
            candidate.build_manifest(
                platform="windows",
                architecture="x86_64",
                source_revision="c" * 40,
                artifact_path=self.artifact,
                renderer_identity_path=self.identity,
            )


if __name__ == "__main__":
    unittest.main()
