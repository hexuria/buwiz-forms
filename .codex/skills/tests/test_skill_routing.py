#!/usr/bin/env python3
"""Forward smoke tests for the two canonical eBIRForms skill boundaries."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
CONVERSION = ROOT / ".codex/skills/ebirforms-convert-form-to-html/SKILL.md"
MAINTENANCE = ROOT / ".codex/skills/ebirforms-print-preview/SKILL.md"
VALIDATION_RULES = ROOT / "rules/agent-boundaries/SKILL.md"
ARTWORK = (
    ROOT
    / ".codex/skills/ebirforms-convert-form-to-html/references/discrete-artwork.md"
)


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
    if any(
        word in normalized
        for word in ("validation rule", "rules corpus", "rule set", "bir-rules", "rule evidence")
    ):
        return "ebirforms-validation-rules"
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
        self.assertEqual(
            expected_route("Audit the validation rules corpus for 2550Q"),
            "ebirforms-validation-rules",
        )
        # Would have routed to the renderer skill on the bare verb "fix".
        self.assertEqual(
            expected_route("Fix the bir-rules source-set digest"),
            "ebirforms-validation-rules",
        )

    def test_discovery_descriptions_express_same_boundary(self) -> None:
        conversion = description(CONVERSION).lower()
        maintenance = description(MAINTENANCE).lower()
        self.assertIn("missing html renderer", conversion)
        self.assertIn("already html-enabled", conversion)
        self.assertIn("fileable form from xml/savefile samples", conversion)
        self.assertIn("existing ebirforms semantic html print preview", maintenance)
        self.assertIn("renderer is missing", maintenance)
        rules = description(VALIDATION_RULES).lower()
        self.assertIn("validation rules corpus", rules)
        self.assertIn("do not use for html print preview", rules)

    def test_validation_rules_skill_states_the_promotion_boundary(self) -> None:
        """The rule that must survive every future edit of this skill."""
        rules = VALIDATION_RULES.read_text(encoding="utf-8").lower()
        self.assertIn("test-only", rules)
        self.assertIn("must never be promoted", rules)
        self.assertIn("never weaken a validator", rules)

    def test_conversion_skill_explicitly_fails_closed_on_one_sample(self) -> None:
        conversion = CONVERSION.read_text(encoding="utf-8").lower()
        self.assertIn("only one xml/savefile sample", conversion)
        self.assertIn("stop and report the missing evidence", conversion)

    def test_artwork_policy_is_strict_and_routes_no_symbol_forms(self) -> None:
        conversion = CONVERSION.read_text(encoding="utf-8").lower()
        maintenance = MAINTENANCE.read_text(encoding="utf-8").lower()
        artwork = ARTWORK.read_text(encoding="utf-8").lower()

        for policy in (conversion, maintenance):
            self.assertIn("exact embedded", policy)
            self.assertIn("rendered-page crop", policy)
            self.assertIn("zero-difference logical", policy)
            self.assertIn("bundled-font", policy)
            self.assertIn("static text", policy)
            self.assertIn("never fabricate", policy)

        self.assertIn("0605:1999", artwork)
        self.assertIn('"status": "absent_in_official_pdf"', artwork)
        self.assertIn("shape-rendering: crispedges", artwork)
        for forbidden_operation in (
            "download",
            "threshold",
            "resample",
            "resize",
            "recolor",
            "sharpen",
        ):
            self.assertIn(forbidden_operation, artwork)

    def test_plain_fields_never_inherit_character_guides(self) -> None:
        conversion = " ".join(CONVERSION.read_text(encoding="utf-8").lower().split())
        maintenance = " ".join(MAINTENANCE.read_text(encoding="utf-8").lower().split())

        for policy in (conversion, maintenance):
            self.assertIn("plain field", policy)
            self.assertIn("absence of guides", policy)
            self.assertIn("comb cells", policy)
            self.assertIn("guide ticks", policy)
            self.assertIn("repeating guide background", policy)


if __name__ == "__main__":
    unittest.main()
