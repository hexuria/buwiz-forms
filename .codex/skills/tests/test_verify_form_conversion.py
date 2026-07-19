#!/usr/bin/env python3
"""Dedicated tests for the conversion-evidence verifier."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = (
    ROOT
    / ".codex/skills/ebirforms-convert-form-to-html/scripts/verify_form_conversion.py"
)
SPEC = importlib.util.spec_from_file_location("conversion_verify_form", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
verify_form = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify_form)


class VerifyFormConversionTests(unittest.TestCase):
    def test_identity_and_geometry_predicates_fail_closed(self) -> None:
        self.assertEqual(
            verify_form.identity("1702mx", "2018C")[3], "1702MXv2018C"
        )
        self.assertTrue(verify_form.positive_int_pair([120, 7]))
        self.assertFalse(verify_form.positive_int_pair([120, True]))
        self.assertTrue(verify_form.finite_ctm([1, 0, 0, 1, 10, 20]))
        self.assertFalse(verify_form.finite_ctm([1, 0, 0, 1, 10, float("inf")]))
        self.assertTrue(verify_form.finite_rect([10, 20, 40, 8]))
        self.assertFalse(verify_form.finite_rect([10, 20, 0, 8]))

    def test_exact_entry_rejects_duplicates(self) -> None:
        errors: list[str] = []
        data = {
            "forms": [
                {"code": "1601C", "revision": "2018"},
                {"code": "1601C", "revision": "2018"},
            ]
        }
        self.assertIsNone(
            verify_form.exact_entry(data, "1601C", "2018", "manifest", errors)
        )
        self.assertEqual(errors, ["manifest has duplicate entries for 1601C:2018"])

    def test_model_stage_passes_with_exact_minimum_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in (
                "crates/bir-core/src/forms/form_1601c.rs",
                "crates/bir-core/src/forms/form_1601c_xml.rs",
                "crates/bir-desktop/src/views/form_1601c_view.rs",
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// tested artifact\n", encoding="utf-8")
            migration = root / "packages/form-specs/form-migration-status.json"
            migration.parent.mkdir(parents=True)
            migration.write_text(
                json.dumps(
                    {
                        "forms": [
                            {
                                "code": "1601C",
                                "revision": "2018",
                                "form_id": "1601Cv2018",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            report = verify_form.audit(root, "1601C", "2018", "model")

            self.assertTrue(report["passed"])
            self.assertEqual(report["errors"], [])
            self.assertEqual(len(report["artifacts"]), 4)

    def test_release_stage_does_not_promote_incomplete_form(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in (
                "crates/bir-core/src/forms/form_1601c.rs",
                "crates/bir-core/src/forms/form_1601c_xml.rs",
                "crates/bir-desktop/src/views/form_1601c_view.rs",
            ):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// tested artifact\n", encoding="utf-8")
            migration = root / "packages/form-specs/form-migration-status.json"
            migration.parent.mkdir(parents=True)
            migration.write_text(
                json.dumps(
                    {
                        "forms": [
                            {
                                "code": "1601C",
                                "revision": "2018",
                                "form_id": "1601Cv2018",
                                "capabilities": {},
                                "production_route": "experimental",
                                "release_ready": False,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            report = verify_form.audit(root, "1601C", "2018", "release")

            self.assertFalse(report["passed"])
            self.assertIn("release requires release_ready=true", report["errors"])
            self.assertIn(
                "release requires production_route=html_only", report["errors"]
            )
            self.assertIn(
                "release requires capabilities.visual_parity=true", report["errors"]
            )

    def test_fixture_matrix_checks_identity_and_all_required_kinds(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fixture = root / "packages/form-contracts/fixtures/1601c-normal.json"
            fixture.parent.mkdir(parents=True)
            fixture.write_text(
                json.dumps({"form": {"code": "WRONG", "version": "2018"}}),
                encoding="utf-8",
            )
            provider = root / "crates/bir-print/src/html_forms/form_1601c.rs"
            provider.parent.mkdir(parents=True)
            provider.write_text(
                "\n".join(
                    (
                        "RenderFixtureKind::Minimum",
                        "RenderFixtureKind::Normal",
                        "RenderFixtureKind::LongValues",
                        "RenderFixtureKind::ValidationEdge",
                        "RenderFixtureKind::ScheduleCapacity",
                    )
                ),
                encoding="utf-8",
            )
            errors: list[str] = []
            artifacts: list[str] = []

            verify_form.fixture_matrix(
                root, "1601C", "2018", "1601c", errors, artifacts
            )

            self.assertEqual(len(errors), 1)
            self.assertIn("fixture identity mismatch", errors[0])
            self.assertEqual(len(artifacts), 2)


if __name__ == "__main__":
    unittest.main()
