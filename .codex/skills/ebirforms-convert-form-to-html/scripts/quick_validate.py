#!/usr/bin/env python3
"""Validate a Codex skill deterministically using only the Python standard library."""

from __future__ import annotations

import ast
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


MAX_SKILL_NAME_LENGTH = 64
MAX_DESCRIPTION_LENGTH = 1024
MAX_SKILL_LINES = 500
NAME_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
KEY_RE = re.compile(r"^([A-Za-z][A-Za-z0-9_-]*):(?:[ \t]*(.*))?$")
LINK_RE = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
ALLOWED_FRONTMATTER_KEYS = frozenset({"name", "description"})
REQUIRED_INTERFACE_FIELDS = frozenset(
    {"display_name", "short_description", "default_prompt"}
)


def _scalar(value: str, key: str) -> str:
    value = value.strip()
    if not value:
        raise ValueError(f"frontmatter {key!r} must be a non-empty scalar")
    if value in {"|", ">", "|-", ">-", "|+", ">+"}:
        raise ValueError("multiline frontmatter values are not supported")
    if value[0] in {'"', "'"}:
        try:
            parsed = ast.literal_eval(value)
        except (SyntaxError, ValueError) as error:
            raise ValueError(f"frontmatter {key!r} has invalid quoting") from error
        if not isinstance(parsed, str):
            raise ValueError(f"frontmatter {key!r} must be a string")
        return parsed.strip()
    return value


def _frontmatter(content: str) -> tuple[dict[str, str], str]:
    lines = content.splitlines()
    if not lines or lines[0] != "---":
        raise ValueError("SKILL.md must begin with YAML frontmatter")
    try:
        closing = lines.index("---", 1)
    except ValueError as error:
        raise ValueError("SKILL.md frontmatter has no closing delimiter") from error
    if closing == 1:
        raise ValueError("SKILL.md frontmatter is empty")

    values: dict[str, str] = {}
    for line_number, line in enumerate(lines[1:closing], start=2):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith((" ", "\t")):
            raise ValueError(
                f"unsupported nested frontmatter at SKILL.md line {line_number}"
            )
        match = KEY_RE.fullmatch(line)
        if match is None:
            raise ValueError(f"invalid frontmatter at SKILL.md line {line_number}")
        key, raw_value = match.groups()
        if key in values:
            raise ValueError(f"duplicate frontmatter key: {key}")
        values[key] = _scalar(raw_value or "", key)

    unexpected = sorted(set(values) - ALLOWED_FRONTMATTER_KEYS)
    if unexpected:
        raise ValueError("unexpected frontmatter key(s): " + ", ".join(unexpected))
    missing = sorted(ALLOWED_FRONTMATTER_KEYS - set(values))
    if missing:
        raise ValueError("missing frontmatter key(s): " + ", ".join(missing))
    return values, "\n".join(lines[closing + 1 :]).strip()


def _local_link_errors(skill_path: Path, body: str) -> list[str]:
    errors: list[str] = []
    root = skill_path.resolve()
    for raw_target in LINK_RE.findall(body):
        target = raw_target.strip().strip("<>")
        if not target or target.startswith("#"):
            continue
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc:
            continue
        relative_target = unquote(parsed.path)
        if not relative_target:
            continue
        resolved = (root / relative_target).resolve()
        try:
            resolved.relative_to(root)
        except ValueError:
            errors.append(f"local link escapes skill directory: {target}")
            continue
        if not resolved.is_file():
            errors.append(f"missing local link target: {relative_target}")
    return errors


def _python_errors(skill_path: Path) -> list[str]:
    scripts = skill_path / "scripts"
    if not scripts.is_dir():
        return []
    errors: list[str] = []
    for script in sorted(scripts.glob("*.py")):
        try:
            source = script.read_text(encoding="utf-8")
            compile(source, script.as_posix(), "exec")
        except (OSError, SyntaxError, UnicodeError) as error:
            errors.append(f"invalid Python helper {script.name}: {error}")
    return errors


def _agent_metadata_errors(skill_path: Path, skill_name: str) -> list[str]:
    metadata = skill_path / "agents/openai.yaml"
    if not metadata.is_file():
        return ["agents/openai.yaml not found"]
    try:
        lines = metadata.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        return [f"cannot read agents/openai.yaml: {error}"]

    errors: list[str] = []
    interface_lines = [
        index
        for index, line in enumerate(lines)
        if line == "interface:"
    ]
    if not interface_lines:
        return ["agents/openai.yaml requires a top-level interface mapping"]
    if len(interface_lines) > 1:
        errors.append("agents/openai.yaml has duplicate interface mappings")
    interface_index = interface_lines[0]

    values: dict[str, str] = {}
    for line_number, line in enumerate(
        lines[interface_index + 1 :], start=interface_index + 2
    ):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if not line.startswith((" ", "\t")):
            break
        if not line.startswith("  ") or line.startswith(("   ", "\t")):
            errors.append(
                "agents/openai.yaml contains unsupported interface structure at "
                f"line {line_number}"
            )
            continue
        match = KEY_RE.fullmatch(line[2:])
        if match is None:
            errors.append(
                f"invalid agents/openai.yaml interface field at line {line_number}"
            )
            continue
        key, raw_value = match.groups()
        if key in values:
            errors.append(f"duplicate agents/openai.yaml interface field: {key}")
            continue
        raw = (raw_value or "").strip()
        if not raw.startswith(('"', "'")):
            errors.append(
                f"agents/openai.yaml interface.{key} must be a quoted string"
            )
            continue
        try:
            values[key] = _scalar(raw, f"interface.{key}")
        except ValueError as error:
            errors.append(f"agents/openai.yaml {error}")

    missing = sorted(REQUIRED_INTERFACE_FIELDS - set(values))
    if missing:
        errors.append(
            "agents/openai.yaml missing interface field(s): " + ", ".join(missing)
        )
    display_name = values.get("display_name", "")
    if "display_name" in values and not display_name:
        errors.append("agents/openai.yaml interface.display_name must not be empty")
    short_description = values.get("short_description", "")
    if "short_description" in values and not 25 <= len(short_description) <= 64:
        errors.append(
            "agents/openai.yaml interface.short_description must be 25-64 characters"
        )
    default_prompt = values.get("default_prompt", "")
    if "default_prompt" in values and f"${skill_name}" not in default_prompt:
        errors.append(
            "agents/openai.yaml interface.default_prompt must mention "
            f"${skill_name}"
        )
    return errors


def validate_skill(skill_directory: str | Path) -> tuple[bool, str]:
    skill_path = Path(skill_directory)
    if not skill_path.is_dir():
        return False, f"skill directory not found: {skill_path}"
    skill_md = skill_path / "SKILL.md"
    if not skill_md.is_file():
        return False, "SKILL.md not found"

    try:
        content = skill_md.read_text(encoding="utf-8")
        metadata, body = _frontmatter(content)
    except (OSError, UnicodeError, ValueError) as error:
        return False, str(error)

    errors: list[str] = []
    name = metadata["name"]
    description = metadata["description"]
    if not NAME_RE.fullmatch(name):
        errors.append(
            "name must use lowercase letters, digits, and single hyphens only"
        )
    if len(name) > MAX_SKILL_NAME_LENGTH:
        errors.append(
            f"name is {len(name)} characters; maximum is {MAX_SKILL_NAME_LENGTH}"
        )
    if name != skill_path.name:
        errors.append(
            f"frontmatter name {name!r} does not match directory {skill_path.name!r}"
        )
    if not description:
        errors.append("description must not be empty")
    if len(description) > MAX_DESCRIPTION_LENGTH:
        errors.append(
            "description is "
            f"{len(description)} characters; maximum is {MAX_DESCRIPTION_LENGTH}"
        )
    if "<" in description or ">" in description:
        errors.append("description cannot contain angle brackets")
    if not body:
        errors.append("SKILL.md body must not be empty")
    if len(content.splitlines()) > MAX_SKILL_LINES:
        errors.append(f"SKILL.md exceeds {MAX_SKILL_LINES} lines")
    errors.extend(_local_link_errors(skill_path, body))
    errors.extend(_python_errors(skill_path))
    errors.extend(_agent_metadata_errors(skill_path, name))

    if errors:
        return False, "Skill validation failed:\n- " + "\n- ".join(sorted(errors))
    return True, "Skill is valid!"


def main(argv: list[str] | None = None) -> int:
    arguments = sys.argv[1:] if argv is None else argv
    if len(arguments) != 1:
        print("Usage: python quick_validate.py <skill_directory>", file=sys.stderr)
        return 2
    valid, message = validate_skill(arguments[0])
    print(message, file=sys.stdout if valid else sys.stderr)
    return 0 if valid else 1


if __name__ == "__main__":
    raise SystemExit(main())
