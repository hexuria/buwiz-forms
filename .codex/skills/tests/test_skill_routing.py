#!/usr/bin/env python3
"""Forward smoke tests for the two canonical eBIRForms skill boundaries."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
CONVERSION = ROOT / ".codex/skills/ebirforms-convert-form-to-html/SKILL.md"
MAINTENANCE = ROOT / ".codex/skills/ebirforms-print-preview/SKILL.md"


def description(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    match = re.match(r"---\n.*?^description:\s*(.+)$.*?\n---", text, re.M | re.S)
    if not match:
        raise AssertionError(f"missing skill description: {path}")
    return match.group(1).strip()


def expected_route(prompt: str) -> str:
    """Executable acceptance policy; Codex discovery metadata mirrors it."""

    normalized = " ".join(prompt.lower().split())
    if "fileable" in normalized and (
        "one xml" in normalized or "one savefile" in normalized
    ):
        return "fail_closed"
    if any(word in normalized for word in ("convert", "create", "missing")) and "html" in normalized:
        return "ebirforms-convert-form-to-html"
    if any(word in normalized for word in ("fix", "align", "alignment", "calibrate")):
        return "ebirforms-print-preview"
    return "unresolved"


class SkillRoutingTests(unittest.TestCase):
    def test_forward_prompts(self) -> None:
        self.assertEqual(
            expected_route("Convert 1601C 2018 to HTML"),
            "ebirforms-convert-form-to-html",
        )
        self.assertEqual(
            expected_route("Fix 2551Q barcode alignment"),
            "ebirforms-print-preview",
        )
        self.assertEqual(
            expected_route("Generate a fileable form from one XML sample"),
            "fail_closed",
        )

    def test_discovery_descriptions_express_same_boundary(self) -> None:
        conversion = description(CONVERSION).lower()
        maintenance = description(MAINTENANCE).lower()
        self.assertIn("missing html renderer", conversion)
        self.assertIn("already html-enabled", conversion)
        self.assertIn("existing ebirforms semantic html print preview", maintenance)
        self.assertIn("renderer is missing", maintenance)

    def test_conversion_skill_explicitly_fails_closed_on_one_sample(self) -> None:
        conversion = CONVERSION.read_text(encoding="utf-8").lower()
        self.assertIn("only one xml/savefile sample", conversion)
        self.assertIn("stop and report the missing evidence", conversion)


if __name__ == "__main__":
    unittest.main()
