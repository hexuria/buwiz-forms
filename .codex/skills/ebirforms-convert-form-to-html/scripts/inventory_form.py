#!/usr/bin/env python3
"""Create a deterministic, read-only inventory for one exact BIR form revision."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import tempfile
from pathlib import Path
from typing import Any


FORM_CODE_RE = re.compile(r"^[A-Za-z0-9]+$")
REVISION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


def identity(form_code: str, revision: str) -> tuple[str, str, str, str]:
    code = form_code.strip().upper()
    rev = revision.strip()
    if not FORM_CODE_RE.fullmatch(code):
        raise ValueError("form code must contain only letters and digits")
    if not REVISION_RE.fullmatch(rev):
        raise ValueError("revision contains unsupported characters")
    return code, rev, code.lower(), f"{code}v{rev}"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def display_path(path: Path, root: Path) -> tuple[str, bool]:
    resolved = path.resolve()
    try:
        return resolved.relative_to(root).as_posix(), False
    except ValueError:
        return f"external:{path.name}", True


def artifact(path: Path, root: Path) -> dict[str, Any]:
    shown, external = display_path(path, root)
    return {
        "path": shown,
        "external": external,
        "size_bytes": path.stat().st_size,
        "sha256": sha256_file(path),
    }


def existing(paths: list[Path], root: Path) -> list[dict[str, Any]]:
    return [artifact(path, root) for path in sorted(paths) if path.is_file()]


def containing(path: Path, tokens: tuple[str, ...]) -> bool:
    if not path.is_file():
        return False
    text = path.read_text(encoding="utf-8", errors="replace")
    return all(token in text for token in tokens)


def load_migration_entry(root: Path, code: str, revision: str) -> Any:
    path = root / "packages/form-specs/form-migration-status.json"
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    matches = [
        item
        for item in data.get("forms", [])
        if str(item.get("code", "")).upper() == code
        and str(item.get("revision", "")) == revision
    ]
    if len(matches) > 1:
        raise ValueError(f"duplicate migration entries for {code}:{revision}")
    return matches[0] if matches else None


def canonical_form_identity(root: Path, code: str) -> tuple[str, str] | None:
    """Read the exact filing identity already owned by the Rust registry.

    File names such as ``form_0605.rs`` intentionally omit the revision, so
    their existence cannot prove that an arbitrary requested revision matches.
    """
    support = root / "crates/bir-core/src/forms/support_level.rs"
    if not support.is_file():
        return None
    text = support.read_text(encoding="utf-8", errors="replace")
    records = re.findall(
        r"FormCapabilityRecord\s*\{\s*"
        r'code:\s*"([A-Za-z0-9]+)"\s*,\s*'
        r'revision:\s*"([A-Za-z0-9._-]+)"\s*,\s*'
        r'form_id:\s*"([A-Za-z0-9._-]+)"',
        text,
        flags=re.DOTALL,
    )
    matches = [
        (revision, form_id)
        for item_code, revision, form_id in records
        if item_code == code
    ]
    if len(matches) > 1:
        raise ValueError(f"Rust registry contains duplicate capability records for {code}")
    return matches[0] if matches else None


def classify_source_artifact(path: Path) -> tuple[str, int | None]:
    suffix = path.suffix.lower()
    if suffix == ".pdf":
        return "pdf", None
    if suffix == ".xml":
        payload = path.read_bytes()
        if b"<div" in payload and payload.lstrip().startswith(b"<?xml"):
            return "plain_bir_xml", payload.count(b"<div")
        return "encrypted_bir_xml", None
    if suffix in {".png", ".jpg", ".jpeg", ".webp"}:
        return "screenshot_or_reference", None
    if suffix in {".json", ".xsd", ".html", ".htm"}:
        return "schema_or_manifest_evidence", None
    if suffix in {".md", ".txt", ".doc", ".docx"}:
        return "guide_or_notes", None
    return "supporting_artifact", None


def inventory_source_directory(source_directory: Path) -> list[dict[str, Any]]:
    source_directory = source_directory.resolve()
    if not source_directory.is_dir():
        raise NotADirectoryError(f"source directory not found: {source_directory}")
    records = []
    for path in sorted(item for item in source_directory.rglob("*") if item.is_file()):
        kind, field_div_count = classify_source_artifact(path)
        record: dict[str, Any] = {
            "relative_path": path.relative_to(source_directory).as_posix(),
            "kind": kind,
            "size_bytes": path.stat().st_size,
            "sha256": sha256_file(path),
        }
        if field_div_count is not None:
            record["field_div_count_hint"] = field_div_count
        records.append(record)
    return records


def build_inventory(
    root: Path,
    form_code: str,
    revision: str,
    official_pdf: Path | None = None,
    source_directory: Path | None = None,
) -> dict[str, Any]:
    root = root.resolve()
    code, rev, slug, form_id = identity(form_code, revision)
    registered_identity = canonical_form_identity(root, code)
    if registered_identity is not None and registered_identity != (rev, form_id):
        registered_revision, registered_form_id = registered_identity
        raise ValueError(
            f"requested identity {form_id} conflicts with Rust registry "
            f"identity {registered_form_id} revision {registered_revision}; "
            "pin the exact revision before inventory"
        )
    component = f"Form{code}"

    paths = {
        "core_model": [root / f"crates/bir-core/src/forms/form_{slug}.rs"],
        "xml_mapping": [root / f"crates/bir-core/src/forms/form_{slug}_xml.rs"],
        "desktop_editor": [root / f"crates/bir-desktop/src/views/form_{slug}_view.rs"],
        "html_component": [root / f"packages/form-renderer/src/forms/{component}.tsx"],
        "renderer_tests": list(
            (root / "packages/form-renderer/tests").glob(f"*{slug}*.test.ts")
        ),
        "contract_fixtures": list(
            (root / "packages/form-contracts/fixtures").glob(f"{slug}-*.json")
        ),
        "visual_references": list(
            (root / "packages/form-renderer/references").glob(
                f"{slug}-{rev.lower()}-page-*.png"
            )
        ),
    }

    generic_candidates = {
        "core_registry": (
            root / "crates/bir-core/src/forms/registry.rs",
            (code,),
        ),
        "render_provider_registry": (
            root / "crates/bir-print/src/html_forms/mod.rs",
            (f"form_{slug}",),
        ),
        "contract_generator": (
            root / "crates/bir-print/src/bin/generate_render_contract.rs",
            (code,),
        ),
        "html_dispatch": (
            root / "packages/form-renderer/src/forms/registry.ts",
            (f"{code}:{rev}",),
        ),
        "form_spec": (
            root / "packages/form-specs/src/index.ts",
            (code, rev),
        ),
        "reference_manifest": (
            root / "packages/form-renderer/references/manifest.json",
            (code, rev),
        ),
    }
    for category, (path, tokens) in generic_candidates.items():
        paths[category] = [path] if containing(path, tokens) else []
    provider = root / f"crates/bir-print/src/html_forms/form_{slug}.rs"
    paths["render_provider"] = [provider] if provider.is_file() else []

    artifacts = {
        category: existing(candidates, root)
        for category, candidates in sorted(paths.items())
    }
    expected = (
        "core_model",
        "xml_mapping",
        "desktop_editor",
        "core_registry",
        "render_provider",
        "render_provider_registry",
        "contract_fixtures",
        "html_component",
        "html_dispatch",
        "form_spec",
        "renderer_tests",
        "visual_references",
        "reference_manifest",
    )

    source_record = None
    if official_pdf is not None:
        if not official_pdf.is_file():
            raise FileNotFoundError(f"official PDF not found: {official_pdf}")
        source_record = artifact(official_pdf, root)

    source_pack_artifacts = (
        inventory_source_directory(source_directory)
        if source_directory is not None
        else None
    )

    return {
        "schema_version": 1,
        "form": {"code": code, "revision": rev, "form_id": form_id},
        "official_source_pdf": source_record,
        "source_pack_directory": (
            str(source_directory.resolve()) if source_directory is not None else None
        ),
        "source_pack_artifacts": source_pack_artifacts,
        "migration_entry": load_migration_entry(root, code, rev),
        "artifacts": artifacts,
        "missing_expected": [name for name in expected if not artifacts[name]],
    }


def write_json(value: Any, destination: str) -> None:
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    if destination == "-":
        print(payload, end="")
        return
    output = Path(destination)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=output.parent, delete=False
    ) as handle:
        handle.write(payload)
        temp_path = Path(handle.name)
    temp_path.replace(output)


def self_test() -> None:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        model = root / "crates/bir-core/src/forms/form_1601c.rs"
        model.parent.mkdir(parents=True)
        model.write_text("// 1601C\n", encoding="utf-8")
        inventory = build_inventory(root, "1601c", "2018")
        assert inventory["form"]["form_id"] == "1601Cv2018"
        assert inventory["artifacts"]["core_model"][0]["sha256"] == sha256_file(model)
        assert "html_component" in inventory["missing_expected"]

        support = root / "crates/bir-core/src/forms/support_level.rs"
        support.write_text(
            'const FORMS: &[FormCapabilityRecord] = &[\n'
            '  FormCapabilityRecord { code: "0605", revision: "1999", '
            'form_id: "0605v1999", capabilities: SCAFFOLD, release_ready: false },\n'
            '];\n',
            encoding="utf-8",
        )
        try:
            build_inventory(root, "0605", "2018")
        except ValueError as error:
            assert "0605v1999" in str(error)
        else:
            raise AssertionError("revision mismatch must fail closed")

        source = root / "source"
        source.mkdir()
        (source / "blank.pdf").write_bytes(b"%PDF-1.7\n")
        (source / "plain.xml").write_text(
            "<?xml version='1.0'?><div>field=valuefield=</div>",
            encoding="utf-8",
        )
        (source / "submitted.xml").write_bytes(b"\x00encrypted")
        source_records = inventory_source_directory(source)
        assert [record["kind"] for record in source_records] == [
            "pdf",
            "plain_bir_xml",
            "encrypted_bir_xml",
        ]
        assert source_records[1]["field_div_count_hint"] == 1
    print("inventory_form.py self-test: ok")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".", help="repository root")
    parser.add_argument("--form-code")
    parser.add_argument("--revision")
    parser.add_argument("--official-pdf", type=Path)
    parser.add_argument(
        "--source-dir",
        type=Path,
        help="exact form source-pack directory to inventory recursively",
    )
    parser.add_argument("--output", default="-", help="JSON path or '-' for stdout")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if not args.self_test and (not args.form_code or not args.revision):
        parser.error("--form-code and --revision are required")
    return args


def main() -> int:
    args = parse_args()
    if args.self_test:
        self_test()
        return 0
    try:
        result = build_inventory(
            Path(args.repo),
            args.form_code,
            args.revision,
            args.official_pdf,
            args.source_dir,
        )
        write_json(result, args.output)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise SystemExit(f"inventory failed: {error}") from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
