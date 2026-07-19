#!/usr/bin/env python3
"""Dedicated tests for the repo-local skill validator."""

from __future__ import annotations

import importlib.util
import io
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
SKILL = ROOT / ".codex/skills/ebirforms-convert-form-to-html"
SCRIPT = SKILL / "scripts/quick_validate.py"
SPEC = importlib.util.spec_from_file_location("conversion_quick_validate", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
quick_validate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quick_validate)


def write_skill(root: Path, name: str, frontmatter: str | None = None) -> Path:
    skill = root / name
    skill.mkdir()
    content = frontmatter or (
        "---\n"
        f"name: {name}\n"
        "description: Deterministic test skill for validation coverage.\n"
        "---\n\n"
        "# Test Skill\n\n"
        "Read [policy](references/policy.md).\n"
    )
    (skill / "SKILL.md").write_text(content, encoding="utf-8")
    references = skill / "references"
    references.mkdir()
    (references / "policy.md").write_text("# Policy\n", encoding="utf-8")
    agents = skill / "agents"
    agents.mkdir()
    (agents / "openai.yaml").write_text(
        "interface:\n"
        f'  display_name: "{name} display"\n'
        '  short_description: "Deterministic skill validation"\n'
        f'  default_prompt: "Use ${name} for this test."\n',
        encoding="utf-8",
    )
    return skill


class QuickValidateTests(unittest.TestCase):
    def test_repository_conversion_skill_is_valid(self) -> None:
        valid, message = quick_validate.validate_skill(SKILL)
        self.assertTrue(valid, message)
        self.assertEqual(message, "Skill is valid!")

    def test_valid_quoted_frontmatter_and_local_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(
                Path(directory),
                "quoted-skill",
                "---\n"
                'name: "quoted-skill"\n'
                "description: 'A quoted deterministic description.'\n"
                "---\n\n"
                "# Quoted\n\n"
                "Read [policy](references/policy.md).\n",
            )
            self.assertEqual(
                quick_validate.validate_skill(skill), (True, "Skill is valid!")
            )

    def test_name_must_match_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "folder-name")
            text = (skill / "SKILL.md").read_text(encoding="utf-8")
            (skill / "SKILL.md").write_text(
                text.replace("name: folder-name", "name: different-name"),
                encoding="utf-8",
            )
            valid, message = quick_validate.validate_skill(skill)
            self.assertFalse(valid)
            self.assertIn("does not match directory", message)

    def test_unexpected_frontmatter_and_broken_links_fail(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            extra = write_skill(root, "extra-key")
            text = (extra / "SKILL.md").read_text(encoding="utf-8")
            (extra / "SKILL.md").write_text(
                text.replace("description:", "license: MIT\ndescription:"),
                encoding="utf-8",
            )
            valid, message = quick_validate.validate_skill(extra)
            self.assertFalse(valid)
            self.assertIn("unexpected frontmatter key", message)

            broken = write_skill(root, "broken-link")
            (broken / "references/policy.md").unlink()
            valid, message = quick_validate.validate_skill(broken)
            self.assertFalse(valid)
            self.assertIn("missing local link target", message)

    def test_python_helpers_must_compile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "bad-helper")
            scripts = skill / "scripts"
            scripts.mkdir()
            (scripts / "bad.py").write_text("def broken(:\n", encoding="utf-8")
            valid, message = quick_validate.validate_skill(skill)
            self.assertFalse(valid)
            self.assertIn("invalid Python helper bad.py", message)

    def test_agent_metadata_is_required_and_bound_to_skill_name(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "agent-metadata")
            (skill / "agents/openai.yaml").unlink()
            valid, message = quick_validate.validate_skill(skill)
            self.assertFalse(valid)
            self.assertIn("agents/openai.yaml not found", message)

        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "agent-metadata")
            metadata = skill / "agents/openai.yaml"
            metadata.write_text(
                "interface:\n"
                '  display_name: "Agent metadata"\n'
                '  short_description: "Deterministic skill validation"\n'
                '  default_prompt: "Use $different-skill for this test."\n',
                encoding="utf-8",
            )
            valid, message = quick_validate.validate_skill(skill)
            self.assertFalse(valid)
            self.assertIn("must mention $agent-metadata", message)

    def test_agent_metadata_strings_must_be_quoted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "unquoted-agent")
            metadata = skill / "agents/openai.yaml"
            metadata.write_text(
                "interface:\n"
                "  display_name: Unquoted Agent\n"
                '  short_description: "Deterministic skill validation"\n'
                '  default_prompt: "Use $unquoted-agent for this test."\n',
                encoding="utf-8",
            )
            valid, message = quick_validate.validate_skill(skill)
            self.assertFalse(valid)
            self.assertIn("interface.display_name must be a quoted string", message)

    def test_agent_metadata_allows_documented_top_level_policy(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            skill = write_skill(Path(directory), "metadata-policy")
            metadata = skill / "agents/openai.yaml"
            metadata.write_text(
                "interface:\n"
                '  display_name: "Metadata policy"\n'
                '  short_description: "Deterministic skill validation"\n'
                '  default_prompt: "Use $metadata-policy for this test."\n'
                "policy:\n"
                "  allow_implicit_invocation: true\n",
                encoding="utf-8",
            )
            valid, message = quick_validate.validate_skill(skill)
            self.assertTrue(valid, message)

    def test_cli_usage_is_an_error(self) -> None:
        stderr = io.StringIO()
        with redirect_stderr(stderr):
            self.assertEqual(quick_validate.main([]), 2)
        self.assertIn("Usage:", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
