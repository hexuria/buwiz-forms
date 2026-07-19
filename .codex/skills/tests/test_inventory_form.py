#!/usr/bin/env python3
"""Dedicated tests for the conversion skill's inventory helper."""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
SCRIPT = (
    ROOT
    / ".codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py"
)
SPEC = importlib.util.spec_from_file_location("conversion_inventory_form", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
inventory_form = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(inventory_form)


class InventoryFormTests(unittest.TestCase):
    def test_identity_normalizes_code_and_rejects_unsafe_values(self) -> None:
        self.assertEqual(
            inventory_form.identity(" 1702rt ", "2018C"),
            ("1702RT", "2018C", "1702rt", "1702RTv2018C"),
        )
        with self.assertRaises(ValueError):
            inventory_form.identity("17/02", "2018")
        with self.assertRaises(ValueError):
            inventory_form.identity("1702RT", "../2018")

    def test_source_pack_inventory_is_sorted_hashed_and_classified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "b.xml").write_bytes(b"\x00encrypted")
            (source / "a.pdf").write_bytes(b"%PDF-1.7\n")
            nested = source / "nested"
            nested.mkdir()
            (nested / "plain.xml").write_text(
                "<?xml version='1.0'?><div>one</div><div>two</div>",
                encoding="utf-8",
            )

            records = inventory_form.inventory_source_directory(source)

            self.assertEqual(
                [record["relative_path"] for record in records],
                ["a.pdf", "b.xml", "nested/plain.xml"],
            )
            self.assertEqual(
                [record["kind"] for record in records],
                ["pdf", "encrypted_bir_xml", "plain_bir_xml"],
            )
            self.assertEqual(records[2]["field_div_count_hint"], 2)
            self.assertEqual(records[0]["sha256"], inventory_form.sha256_file(source / "a.pdf"))

    def test_registry_identity_mismatch_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            support = root / "crates/bir-core/src/forms/support_level.rs"
            support.parent.mkdir(parents=True)
            support.write_text(
                'FormCapabilityRecord { code: "0605", revision: "1999", '
                'form_id: "0605v1999", capabilities: SCAFFOLD, '
                'release_ready: false }',
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "pin the exact revision"):
                inventory_form.build_inventory(root, "0605", "2018")

    def test_inventory_records_present_artifacts_and_missing_categories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            model = root / "crates/bir-core/src/forms/form_1601c.rs"
            model.parent.mkdir(parents=True)
            model.write_text("// model\n", encoding="utf-8")
            migration = root / "packages/form-specs/form-migration-status.json"
            migration.parent.mkdir(parents=True)
            migration.write_text(
                json.dumps({"forms": [{"code": "1601C", "revision": "2018"}]}),
                encoding="utf-8",
            )

            result = inventory_form.build_inventory(root, "1601c", "2018")

            self.assertEqual(result["form"]["form_id"], "1601Cv2018")
            self.assertEqual(result["migration_entry"]["code"], "1601C")
            self.assertEqual(
                result["artifacts"]["core_model"][0]["sha256"],
                inventory_form.sha256_file(model),
            )
            self.assertIn("html_component", result["missing_expected"])

    def test_write_json_is_stable_and_ends_with_newline(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "nested/inventory.json"
            inventory_form.write_json({"z": 1, "a": 2}, str(output))
            self.assertEqual(output.read_text(encoding="utf-8"), '{\n  "a": 2,\n  "z": 1\n}\n')


if __name__ == "__main__":
    unittest.main()
