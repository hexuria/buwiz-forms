#!/usr/bin/env python3
"""Audit deterministic artifacts for one exact BIR HTML form conversion."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import struct
from pathlib import Path
from typing import Any


FORM_CODE_RE = re.compile(r"^[A-Za-z0-9]+$")
REVISION_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")
FIXTURE_KINDS = (
    "minimum",
    "normal",
    "long_value",
    "validation_edge",
    "maximum_capacity",
)
RELEASE_CAPABILITIES = (
    "typed_model",
    "xml_round_trip",
    "formula_evidence",
    "persistence",
    "queue_submission",
    "editor",
    "render_contract",
    "html_component",
    "html_spec",
    "pagination",
    "visual_parity",
    "native_preview",
    "native_print",
    "pdf_export",
    "packaged_offline",
)
LEGACY_TOKENS = (
    "formtypes/",
    "template.typ",
    "source_svg",
    "data:image/svg+xml",
    "page1.svg",
    "page2.svg",
)


def identity(form_code: str, revision: str) -> tuple[str, str, str, str]:
    code = form_code.strip().upper()
    rev = revision.strip()
    if not FORM_CODE_RE.fullmatch(code):
        raise ValueError("form code must contain only letters and digits")
    if not REVISION_RE.fullmatch(rev):
        raise ValueError("revision contains unsupported characters")
    return code, rev, code.lower(), f"{code}v{rev}"


def load_json(path: Path, errors: list[str]) -> Any:
    if not path.is_file():
        errors.append(f"missing {path.as_posix()}")
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        errors.append(f"invalid JSON in {path.as_posix()}: {error}")
        return None


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def png_size(path: Path) -> tuple[int, int]:
    with path.open("rb") as handle:
        header = handle.read(24)
    if len(header) != 24 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("invalid PNG header")
    return struct.unpack(">II", header[16:24])


def exact_entry(
    data: Any, code: str, revision: str, label: str, errors: list[str]
) -> dict[str, Any] | None:
    if not isinstance(data, dict):
        return None
    matches = [
        value
        for value in data.get("forms", [])
        if isinstance(value, dict)
        and str(value.get("code", "")).upper() == code
        and str(value.get("revision", "")) == revision
    ]
    if not matches:
        errors.append(f"{label} lacks exact entry {code}:{revision}")
        return None
    if len(matches) > 1:
        errors.append(f"{label} has duplicate entries for {code}:{revision}")
        return None
    return matches[0]


def contains_legacy(value: Any, prefix: str = "") -> list[str]:
    findings: list[str] = []
    if isinstance(value, dict):
        for key in sorted(value):
            child = f"{prefix}.{key}" if prefix else str(key)
            if any(token in str(key).lower() for token in LEGACY_TOKENS):
                findings.append(child)
            findings.extend(contains_legacy(value[key], child))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            findings.extend(contains_legacy(item, f"{prefix}[{index}]"))
    elif isinstance(value, str):
        lowered = value.lower()
        if any(token in lowered for token in LEGACY_TOKENS):
            findings.append(prefix or "value")
    return findings


def relative(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def renderer_unit_test_paths(tests_dir: Path, slug: str) -> list[Path]:
    return sorted(
        {
            *tests_dir.glob(f"*{slug}*.test.ts"),
            *tests_dir.glob(f"*{slug}*.test.tsx"),
        }
    )


def reference_pixel_dimensions(
    width_pt: float, height_pt: float, dpi: float
) -> tuple[int, int]:
    return (round(width_pt * dpi / 72.0), round(height_pt * dpi / 72.0))


def require_file(
    path: Path, root: Path, errors: list[str], artifacts: list[str]
) -> None:
    if path.is_file():
        artifacts.append(relative(path, root))
    else:
        errors.append(f"missing {relative(path, root)}")


def fixture_matrix(
    root: Path,
    code: str,
    revision: str,
    slug: str,
    errors: list[str],
    artifacts: list[str],
) -> None:
    fixtures = sorted(
        (root / "packages/form-contracts/fixtures").glob(f"{slug}-*.json")
    )
    artifacts.extend(relative(path, root) for path in fixtures)
    if not fixtures:
        errors.append(f"no generated contract fixtures found for {slug}")
        return
    for fixture in fixtures:
        value = load_json(fixture, errors)
        form = value.get("form") if isinstance(value, dict) else None
        if (
            not isinstance(form, dict)
            or form.get("code") != code
            or str(form.get("version", "")) != revision
        ):
            errors.append(
                f"fixture identity mismatch in {relative(fixture, root)}; "
                f"expected {code}:{revision}"
            )

    provider = root / f"crates/bir-print/src/html_forms/form_{slug}.rs"
    if not provider.is_file():
        errors.append(f"missing {relative(provider, root)}")
        return
    artifacts.append(relative(provider, root))
    provider_text = provider.read_text(encoding="utf-8", errors="replace")
    provider_kinds = {
        "minimum": "RenderFixtureKind::Minimum",
        "normal": "RenderFixtureKind::Normal",
        "long_value": "RenderFixtureKind::LongValues",
        "validation_edge": "RenderFixtureKind::ValidationEdge",
        "maximum_capacity": "RenderFixtureKind::ScheduleCapacity",
    }
    for kind in FIXTURE_KINDS:
        if provider_kinds[kind] not in provider_text:
            errors.append(f"Rust provider lacks required fixture kind {kind}")


def audit(root: Path, form_code: str, revision: str, stage: str) -> dict[str, Any]:
    root = root.resolve()
    code, rev, slug, form_id = identity(form_code, revision)
    component_name = f"Form{code}"
    errors: list[str] = []
    warnings: list[str] = []
    artifacts: list[str] = []

    require_file(
        root / f"crates/bir-core/src/forms/form_{slug}.rs", root, errors, artifacts
    )
    require_file(
        root / f"crates/bir-core/src/forms/form_{slug}_xml.rs", root, errors, artifacts
    )
    require_file(
        root / f"crates/bir-desktop/src/views/form_{slug}_view.rs",
        root,
        errors,
        artifacts,
    )

    migration_path = root / "packages/form-specs/form-migration-status.json"
    migration = load_json(migration_path, errors)
    entry = exact_entry(migration, code, rev, "migration manifest", errors)
    if entry is None:
        entry = {}
    else:
        artifacts.append(relative(migration_path, root))
        if str(entry.get("form_id", form_id)) != form_id:
            errors.append(f"migration form_id must be {form_id}")

    if stage in ("preview", "release"):
        provider_registry = root / "crates/bir-print/src/html_forms/mod.rs"
        require_file(provider_registry, root, errors, artifacts)
        if (
            provider_registry.is_file()
            and f"form_{slug}"
            not in provider_registry.read_text(encoding="utf-8", errors="replace")
        ):
            errors.append(f"Rust provider registry lacks form_{slug}")

        component = root / f"packages/form-renderer/src/forms/{component_name}.tsx"
        require_file(component, root, errors, artifacts)
        if component.is_file():
            text = component.read_text(encoding="utf-8", errors="replace").lower()
            for token in LEGACY_TOKENS:
                if token in text:
                    errors.append(
                        f"HTML component contains forbidden legacy token {token!r}"
                    )

        dispatch = root / "packages/form-renderer/src/forms/registry.ts"
        require_file(dispatch, root, errors, artifacts)
        if dispatch.is_file():
            text = dispatch.read_text(encoding="utf-8", errors="replace")
            for token in (f"{code}:{rev}", component_name):
                if token not in text:
                    errors.append(f"HTML dispatch lacks {token!r} for {code}:{rev}")

        spec = root / "packages/form-specs/src/index.ts"
        require_file(spec, root, errors, artifacts)
        if spec.is_file():
            text = spec.read_text(encoding="utf-8", errors="replace")
            if code not in text or rev not in text:
                errors.append(f"form specs lack exact entry {code}:{rev}")

        tests_dir = root / "packages/form-renderer/tests"
        tests = renderer_unit_test_paths(tests_dir, slug)
        if tests:
            artifacts.extend(relative(path, root) for path in tests)
        else:
            errors.append(f"no renderer unit test found for {code}:{rev}")

        fixture_matrix(root, code, rev, slug, errors, artifacts)

        references_path = root / "packages/form-renderer/references/manifest.json"
        references = load_json(references_path, errors)
        reference = exact_entry(references, code, rev, "reference manifest", errors)
        if reference is not None:
            artifacts.append(relative(references_path, root))
            digest = str(reference.get("official_source_sha256", ""))
            if not SHA256_RE.fullmatch(digest):
                errors.append("reference entry lacks a valid official_source_sha256")
            if reference.get("page_count", 0) < 1:
                errors.append("reference entry has no pages")
            page_width_pt = reference.get("page_width_pt")
            page_height_pt = reference.get("page_height_pt")
            reference_dpi = reference.get("reference_dpi", references.get("dpi"))
            geometry_values = (page_width_pt, page_height_pt, reference_dpi)
            if any(
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(float(value))
                or float(value) <= 0
                for value in geometry_values
            ):
                errors.append(
                    "reference paper geometry and DPI must be finite positive numbers"
                )
                expected_reference_width = None
                expected_reference_height = None
            else:
                expected_reference_width, expected_reference_height = (
                    reference_pixel_dimensions(
                        float(page_width_pt),
                        float(page_height_pt),
                        float(reference_dpi),
                    )
                )

            pages = reference.get("pages")
            if not isinstance(pages, list) or len(pages) != reference.get("page_count"):
                errors.append("reference pages do not match page_count")
                pages = []
            for expected_page, page in enumerate(pages, start=1):
                if not isinstance(page, dict) or page.get("page") != expected_page:
                    errors.append(
                        f"reference page sequence is invalid at page {expected_page}"
                    )
                    continue
                path_value = page.get("reference_png")
                expected_hash = page.get("reference_png_sha256")
                if not isinstance(path_value, str) or not isinstance(
                    expected_hash, str
                ):
                    errors.append(
                        f"reference page {expected_page} lacks path/hash evidence"
                    )
                    continue
                image = root / path_value
                if not image.is_file():
                    errors.append(f"missing reference PNG {path_value}")
                    continue
                artifacts.append(relative(image, root))
                if sha256_file(image) != expected_hash:
                    errors.append(f"reference PNG hash mismatch for {path_value}")
                try:
                    width, height = png_size(image)
                except (OSError, ValueError) as error:
                    errors.append(f"cannot inspect reference PNG {path_value}: {error}")
                    continue
                if width != page.get("reference_width_px") or height != page.get(
                    "reference_height_px"
                ):
                    errors.append(f"reference PNG dimensions mismatch for {path_value}")
                if (
                    expected_reference_width is not None
                    and expected_reference_height is not None
                    and (width, height)
                    != (expected_reference_width, expected_reference_height)
                ):
                    errors.append(
                        f"reference PNG dimensions do not match paper geometry/DPI for {path_value}"
                    )

            fixture_path = reference.get("fixture")
            fixture_hash = reference.get("fixture_sha256")
            if isinstance(fixture_path, str) and isinstance(fixture_hash, str):
                visual_fixture = root / fixture_path
                if not visual_fixture.is_file():
                    errors.append(f"missing visual fixture {fixture_path}")
                elif sha256_file(visual_fixture) != fixture_hash:
                    errors.append(f"visual fixture hash mismatch for {fixture_path}")
            elif stage == "release":
                errors.append("release reference lacks pinned visual fixture evidence")

            if stage == "release":
                legacy = contains_legacy(reference)
                if legacy:
                    errors.append(
                        "release reference retains legacy provenance at: "
                        + ", ".join(legacy)
                    )
                provenance = reference.get("reference_provenance")
                if (
                    not isinstance(provenance, dict)
                    or provenance.get("kind") != "official_pdf_raster"
                ):
                    errors.append(
                        "release reference provenance must be official_pdf_raster"
                    )
                elif provenance.get("replacement_required") is not False:
                    errors.append(
                        "release reference still requires provenance replacement"
                    )
            elif (
                isinstance(reference.get("reference_provenance"), dict)
                and reference["reference_provenance"].get("replacement_required")
                is True
            ):
                warnings.append("official-PDF reference provenance is still required")
        if isinstance(references, dict):
            if references.get("calibration_only") is not True:
                errors.append("reference manifest must be calibration_only")
            if references.get("runtime_background_allowed") is not False:
                errors.append("runtime form-page backgrounds must be forbidden")

    if stage == "release":
        capabilities = entry.get("capabilities")
        if not isinstance(capabilities, dict):
            errors.append("release requires a capabilities object")
            capabilities = {}
        for capability in RELEASE_CAPABILITIES:
            if capabilities.get(capability) is not True:
                errors.append(f"release requires capabilities.{capability}=true")
        route = entry.get("production_route", entry.get("route"))
        if route != "html_only":
            errors.append("release requires production_route=html_only")
        if entry.get("release_ready") is not True:
            errors.append("release requires release_ready=true")

    return {
        "schema_version": 1,
        "form": {"code": code, "revision": rev, "form_id": form_id},
        "stage": stage,
        "passed": not errors,
        "errors": sorted(set(errors)),
        "warnings": sorted(set(warnings)),
        "artifacts": sorted(set(artifacts)),
    }


def self_test() -> None:
    assert identity("1702rt", "2018C")[3] == "1702RTv2018C"
    assert "editor" in RELEASE_CAPABILITIES
    assert reference_pixel_dimensions(612.0, 792.0, 144.0) == (1224, 1584)
    import tempfile

    with tempfile.TemporaryDirectory() as directory:
        tests_dir = Path(directory)
        (tests_dir / "form-1601c.test.tsx").touch()
        (tests_dir / "form-1601c.test.ts").touch()
        (tests_dir / "form-1601c.test.js").touch()
        assert len(renderer_unit_test_paths(tests_dir, "1601c")) == 2
    value = {"pages": [{"source_svg": "formtypes/x/pages/page1.svg"}]}
    findings = contains_legacy(value)
    assert findings
    assert SHA256_RE.fullmatch("a" * 64)
    print("verify_form_conversion.py self-test: ok")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=".")
    parser.add_argument("--form-code")
    parser.add_argument("--revision")
    parser.add_argument(
        "--stage", choices=("model", "preview", "release"), default="preview"
    )
    parser.add_argument("--json", action="store_true", dest="json_output")
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
        report = audit(Path(args.repo), args.form_code, args.revision, args.stage)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise SystemExit(f"conversion verification failed: {error}") from error

    if args.json_output:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        state = "PASS" if report["passed"] else "FAIL"
        print(
            f"{state} {report['form']['code']}:{report['form']['revision']} ({report['stage']})"
        )
        for error in report["errors"]:
            print(f"ERROR: {error}")
        for warning in report["warnings"]:
            print(f"WARN: {warning}")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
