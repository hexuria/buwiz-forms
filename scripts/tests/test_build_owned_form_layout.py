from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "build_owned_form_layout.py"
SPEC = importlib.util.spec_from_file_location("build_owned_form_layout", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
layout = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = layout
SPEC.loader.exec_module(layout)


class BuildOwnedFormLayoutTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.form_dir = Path(self.temporary_directory.name) / "formtypes" / "2551Qv2018"
        self.form_dir.mkdir(parents=True)

    def write_sources(
        self,
        *,
        structure_changes: dict | None = None,
        fields: list[dict] | None = None,
        metadata_changes: dict | None = None,
    ) -> None:
        structure = {
            "form_id": "2551Qv2018",
            "page_dimensions": {"width": 612.0, "height": 936.0, "count": 2},
            "lines": [],
            "rectangles": [
                {
                    "x": 20.0,
                    "y": 10.0,
                    "w": 10.0,
                    "h": 0.4,
                    "fill": "#000000",
                    "stroke": None,
                    "stroke_width": 1.0,
                    "page": 1,
                },
                {
                    "x": 30.1,
                    "y": 10.05,
                    "w": 10.0,
                    "h": 0.4,
                    "fill": "#000000",
                    "stroke": None,
                    "stroke_width": 1.0,
                    "page": 1,
                },
                {
                    "x": 50.0,
                    "y": 20.0,
                    "w": 0.4,
                    "h": 10.0,
                    "fill": "#000000",
                    "stroke": None,
                    "stroke_width": 1.0,
                    "page": 1,
                },
                {
                    "x": 50.05,
                    "y": 30.1,
                    "w": 0.4,
                    "h": 5.0,
                    "fill": "#000000",
                    "stroke": None,
                    "stroke_width": 1.0,
                    "page": 1,
                },
                {
                    "x": 10.0,
                    "y": 40.0,
                    "w": 100.0,
                    "h": 20.0,
                    "fill": "#d8d9d8",
                    "stroke": None,
                    "stroke_width": 1.0,
                    "page": 1,
                },
            ],
            "text_blocks": [
                {
                    "content": "Part I - Background Information",
                    "x": 20.0,
                    "y": 75.0,
                    "font_size": 10.0,
                    "font_name": "Arial",
                    "is_bold": True,
                    "color": "#000000",
                    "page": 1,
                }
            ],
        }
        if structure_changes:
            structure.update(structure_changes)

        if fields is None:
            fields = [
                self.field(
                    "registeredAddress", "char", 10.0, 100.0, 100.0, 10.0, 40
                ),
                self.field(
                    "registeredAddress", "char", 10.0, 112.0, 80.0, 10.0, 31
                ),
                self.field(
                    "amount", "dec", 200.0, 150.0, 100.0, 12.0, 11, dec_x=310.0
                ),
                self.field(
                    "amount", "dec", 312.0, 150.0, 20.0, 12.0, 2, dec_x=310.0
                ),
            ]
        formtype = {
            "form_id": "2551Qv2018",
            "page_width": 612.0,
            "page_height": 936.0,
            "fields": fields,
        }
        metadata = {
            "form_id": "2551Qv2018",
            "title": "Quarterly Percentage Tax Return",
            "official_source": "https://example.test/2551q.pdf",
            "sha256": layout.EXPECTED_OFFICIAL_SOURCE_SHA256,
            "page_width_pt": 612.0,
            "page_height_pt": 936.0,
            "page_count": 2,
        }
        if metadata_changes:
            metadata.update(metadata_changes)

        for filename, value in (
            ("form_structure.json", structure),
            ("formtype.json", formtype),
            ("metadata.json", metadata),
        ):
            (self.form_dir / filename).write_text(
                json.dumps(value, indent=2) + "\n", encoding="utf-8"
            )

    @staticmethod
    def field(
        key: str,
        kind: str,
        x: float,
        y: float,
        width: float,
        height: float,
        char_count: int,
        *,
        dec_x: float | None = None,
        page: int = 1,
    ) -> dict:
        value = {
            "key": key,
            "kind": kind,
            "page": page,
            "x": x,
            "y": y,
            "char_count": char_count,
            "optional": True,
            "widget": {
                "type": "text",
                "width": width,
                "height": height,
                "font_size": 8.5,
            },
        }
        if dec_x is not None:
            value.update({"dec_x": dec_x, "int_cells": 11, "cell_w": 10.0})
        return value

    def test_builds_deterministic_semantic_candidate(self) -> None:
        self.write_sources()

        first = layout.build_candidate(self.form_dir)
        second = layout.build_candidate(self.form_dir)

        self.assertEqual(
            layout.serialize_candidate(first), layout.serialize_candidate(second)
        )
        self.assertEqual(first["coordinate_system"]["origin"], "top_left")
        self.assertEqual(first["coordinate_system"]["page_width_pt"], 612.0)
        self.assertEqual(first["coordinate_system"]["page_height_pt"], 936.0)
        self.assertEqual(first["coordinate_system"]["page_count"], 2)
        self.assertEqual(
            first["source_provenance"]["official_source_sha256"],
            layout.EXPECTED_OFFICIAL_SOURCE_SHA256,
        )
        for filename, digest in first["source_provenance"]["input_sha256"].items():
            self.assertEqual(
                digest,
                hashlib.sha256((self.form_dir / filename).read_bytes()).hexdigest(),
            )

        page_one = first["pages"][0]
        horizontal = [
            rule for rule in page_one["rules"] if rule["orientation"] == "horizontal"
        ]
        vertical = [
            rule for rule in page_one["rules"] if rule["orientation"] == "vertical"
        ]
        self.assertEqual(len(horizontal), 1)
        self.assertEqual(horizontal[0]["source_segments"], 2)
        self.assertEqual(horizontal[0]["start_pt"], 20.0)
        self.assertEqual(horizontal[0]["end_pt"], 40.1)
        self.assertEqual(len(vertical), 1)
        self.assertEqual(vertical[0]["source_segments"], 2)

        self.assertEqual(len(page_one["fill_candidates"]), 1)
        self.assertEqual(
            page_one["fill_candidates"][0]["role_candidate"], "shaded_region"
        )
        self.assertEqual(len(page_one["text_candidates"]), 1)
        self.assertEqual(
            page_one["text_candidates"][0]["role_candidate"], "section_heading"
        )
        self.assertIn("baseline", first["coordinate_system"]["text_y_semantics"])

        bindings = {
            binding["field_key"]: binding for binding in page_one["dynamic_bindings"]
        }
        self.assertEqual(
            [fragment["role"] for fragment in bindings["registeredAddress"]["fragments"]],
            ["primary", "continuation"],
        )
        self.assertEqual(
            [fragment["role"] for fragment in bindings["amount"]["fragments"]],
            ["integer", "decimal"],
        )

    def test_groups_repeated_non_decimal_bindings_instead_of_rejecting_them(self) -> None:
        fields = [
            self.field("choice", "bool", 10.0, 100.0, 10.0, 10.0, 1),
            self.field("choice", "bool", 30.0, 100.0, 10.0, 10.0, 1),
        ]
        self.write_sources(fields=fields)

        candidate = layout.build_candidate(self.form_dir)
        fragments = candidate["pages"][0]["dynamic_bindings"][0]["fragments"]

        self.assertEqual([fragment["order"] for fragment in fragments], [0, 1])
        self.assertEqual([fragment["role"] for fragment in fragments], ["primary", "repeat"])

    def test_rejects_conflicting_dynamic_binding_overlap(self) -> None:
        fields = [
            self.field("left", "char", 10.0, 100.0, 50.0, 12.0, 10),
            self.field("right", "char", 40.0, 105.0, 50.0, 12.0, 10),
        ]
        self.write_sources(fields=fields)

        with self.assertRaisesRegex(layout.LayoutError, "conflicting dynamic binding overlap"):
            layout.build_candidate(self.form_dir)

    def test_rejects_geometry_or_official_source_drift(self) -> None:
        with self.subTest("page geometry"):
            self.write_sources(
                structure_changes={
                    "page_dimensions": {"width": 612.0, "height": 792.0, "count": 2}
                }
            )
            with self.assertRaisesRegex(layout.LayoutError, "height must be 936.0pt"):
                layout.build_candidate(self.form_dir)

        with self.subTest("official source hash"):
            self.write_sources(metadata_changes={"sha256": "0" * 64})
            with self.assertRaisesRegex(layout.LayoutError, "official-source hash drift"):
                layout.build_candidate(self.form_dir)

    def test_writes_only_below_scratch(self) -> None:
        self.write_sources()
        candidate = layout.build_candidate(self.form_dir)
        unsafe_output = Path(self.temporary_directory.name) / "candidate.json"
        with self.assertRaisesRegex(layout.LayoutError, "must remain under a .scratch"):
            layout.write_candidate(candidate, unsafe_output)

        safe_output = (
            Path(self.temporary_directory.name) / ".scratch" / "layout" / "candidate.json"
        )
        written = layout.write_candidate(candidate, safe_output)
        self.assertEqual(written, safe_output.resolve())
        self.assertEqual(
            written.read_text(encoding="utf-8"), layout.serialize_candidate(candidate)
        )


if __name__ == "__main__":
    unittest.main()
