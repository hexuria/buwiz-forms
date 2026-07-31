#!/usr/bin/env python3
"""The done-condition for tools/formgen/GOAL.md. Exit 0 means finished.

One command, so nobody has to decide whether the work is done by reading a
summary. Every check prints a line and a verdict.

The rule that matters most here: **a check that cannot be evaluated is a
failure, never a pass.** This project has already been burned by the opposite.
The numeric audit reported `rules 100% on 51/51` while 137 real defects were
present -- a black rectangle over a header, a seal printed upside-down, tax
brackets a taxpayer could type over -- because it only compared what it knew to
compare. An unimplemented assertion that silently passes is that same failure
wearing a green tick, so `UNEVALUABLE` is counted with the failures and named in
the summary.

Usage:
    python3 tools/formgen/gate.py                 # the real gate
    python3 tools/formgen/gate.py --only rules    # one check, while iterating
    python3 tools/formgen/gate.py --list          # what the checks are
    python3 tools/formgen/gate.py --self-test     # the gate checks itself
"""

from __future__ import annotations

import argparse
import dataclasses
import enum
import hashlib
import json
import math
import os
import pathlib
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
from decimal import Decimal, InvalidOperation
from typing import Any, Callable, Iterable, Sequence

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

FORMS = REPO / "forms"
BUILD = REPO / "build"
AUDIT_JSON = BUILD / "audit.json"
AUDIT_APPLICATION_ATTESTATION = BUILD / "audit-attested.json"
BATCH_REPORT = BUILD / "batch-report.json"
FINDINGS = HERE / "review-findings.json"
COMB_REFEREE_REPORT = BUILD / "comb-referee.json"
COMB_REFEREE_ATTESTATION = BUILD / "comb-referee-attested.json"
COMB_REFEREE_SOURCE_ROOT = pathlib.Path.home() / "Downloads/forms"

EXPECTED_FORMS = 51
EXPECTED_COMB_SUBJECTS = 4442
COMB_REFEREE_REPORT_VERSION = 2
COMB_REFEREE_ATTESTATION_VERSION = 2
COMB_REFEREE_SCOPE = "formgen-comb-referee-application-v1"
AUDIT_APPLICATION_SCOPE = "formgen-audit-application-v1"
AUDIT_APPLICATION_ATTESTATION_VERSION = 1
COMB_REFEREE_TIMEOUT_SECONDS = 7200
COMB_REFEREE_RUN_COUNT = 2
COMB_REFEREE_CLEANUP_TIMEOUT_SECONDS = 5
COMB_REFEREE_TOTAL_TIMEOUT_SECONDS = (
    COMB_REFEREE_RUN_COUNT * (
        COMB_REFEREE_TIMEOUT_SECONDS
        + 2 * COMB_REFEREE_CLEANUP_TIMEOUT_SECONDS))
ISOLATED_PYTHON_ATTESTED_FLAGS = [
    "-I", "-B", "-X", "pycache_prefix=<fresh-empty-directory>",
]
COMB_REFEREE_PRODUCERS = (
    "tools/formgen/gate.py",
    "tools/formgen/batch.py",
    "tools/formgen/comb_referee.py",
    "tools/formgen/audit.py",
    "tools/formgen/lattice.py",
    "tools/formgen/extract.py",
    "tools/formgen/guides.py",
    "tools/formgen/fonts.py",
    "tools/formgen/emit.py",
    "tools/formgen/index_page.py",
    "tools/formgen/verify.py",
)
COMB_REFEREE_ARTIFACT_TREES = {
    "ir": BUILD / "ir",
    "layout": BUILD / "layout",
    "html": BUILD / "html",
    "guides": BUILD / "guides",
}
COMPARISON_NAMES = (
    "agree", "repair-lattice", "repair-audit", "stale-generation", "stop",
    "unevaluable",
)

# Modules that expose --self-test. lattice and fonts need an --ir argument, so
# they are invoked with one rather than being excused from the check.
SELF_TEST_MODULES = ("extract", "lattice", "fonts", "guides", "emit", "verify",
                     "index_page", "comb_referee", "gate")

# The eight assertions from GOAL.md. gate.py does not implement them: audit.py
# owns them, and the gate's job is to demand them. Each maps to the key audit.py
# must publish per form in its record.
REQUIRED_ASSERTIONS = {
    "inputs_over_printed_text": "No <input> overlaps a pre-printed text run's bbox",
    "comb_slots_match_printed": "Every comb's slot count equals its printed compartment count",
    "money_boxes_have_inputs": "Every printed money box on a form page has an input",
    "rules_below_guide_cut": "No form-side rule extends below that page's guide cut",
    "run_colour_matches_ir": "No emitted run's colour differs from the IR's",
    "reflow_rate_without_description": "No relocated table row has an empty description and a rate",
    "image_transform_applied": "Every non-positive-diagonal image transform is emitted",
    "no_invented_codepoints": "No IR run holds a character the source did not state",
}
AUDIT_DEPENDENT_CHECKS = {
    "rules", "paper", "artwork", "text", "assertions",
}


class Verdict(enum.Enum):
    PASS = "PASS"
    FAIL = "FAIL"
    UNEVALUABLE = "UNEVALUABLE"   # counted as a failure; see the module docstring

    @property
    def ok(self) -> bool:
        return self is Verdict.PASS


@dataclasses.dataclass
class Result:
    name: str
    verdict: Verdict
    detail: str


def run(args: list[str], timeout: int = 5400) -> tuple[int, str]:
    return run_isolated_python(args, timeout)


def run_isolated_python(
        args: list[str], timeout: int = 5400,
        base_environment: dict[str, str] | None = None,
        ) -> tuple[int, str]:
    environment = dict(
        os.environ if base_environment is None else base_environment)
    environment.pop("PYTHONPATH", None)
    environment.pop("PYTHONHOME", None)
    with tempfile.TemporaryDirectory(
            prefix=".gate-python-pycache-") as pycache_prefix:
        # batch.py launches the individual Python generator stages.  These
        # variables give those grandchildren the same source-only cache and
        # safe-path policy even though their argv is constructed by batch.py.
        environment["PYTHONDONTWRITEBYTECODE"] = "1"
        environment["PYTHONPYCACHEPREFIX"] = pycache_prefix
        environment["PYTHONNOUSERSITE"] = "1"
        environment["PYTHONSAFEPATH"] = "1"
        process = subprocess.Popen(
            [
                sys.executable, "-I", "-B", "-X",
                f"pycache_prefix={pycache_prefix}", *args,
            ], cwd=REPO,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
            env=environment, start_new_session=(os.name == "posix"))
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                process.kill()
            try:
                stdout, stderr = process.communicate(
                    timeout=COMB_REFEREE_CLEANUP_TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
            return 124, (
                stdout + stderr
                + f"\nisolated Python process exceeded {timeout}s\n")
        return process.returncode, stdout + stderr


def load(path: pathlib.Path) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001 - absent or unreadable is "cannot evaluate"
        return None


class CombRefereeScopeError(RuntimeError):
    """The application-scoped referee claim cannot be evaluated safely."""


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def canonical_digest(value: Any) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"),
                         ensure_ascii=False).encode("utf-8")
    return sha256_bytes(payload)


def _is_sha256(value: Any) -> bool:
    return (isinstance(value, str) and len(value) == 64
            and all(character in "0123456789abcdef" for character in value))


def _is_count(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value >= 0


def _stable_file_record(path: pathlib.Path, logical: str) -> dict[str, Any]:
    """Read one regular file and reject mutation during the read itself."""
    try:
        if path.is_symlink():
            raise CombRefereeScopeError(f"symlink is outside the scope: {logical}")
        before = path.stat(follow_symlinks=False)
        if not stat.S_ISREG(before.st_mode):
            raise CombRefereeScopeError(f"not a regular file: {logical}")
        payload = path.read_bytes()
        after = path.stat(follow_symlinks=False)
    except OSError as error:
        raise CombRefereeScopeError(f"cannot read {logical}: {error}") from error
    identity = lambda value: (  # noqa: E731 - compact immutable stat identity
        value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns,
        value.st_ctime_ns,
    )
    if identity(before) != identity(after) or len(payload) != after.st_size:
        raise CombRefereeScopeError(f"file changed while hashing: {logical}")
    return {"path": logical, "bytes": len(payload),
            "sha256": sha256_bytes(payload)}


def _file_manifest(records: Sequence[dict[str, Any]]) -> dict[str, Any]:
    ordered = sorted(records, key=lambda record: str(record["path"]))
    if len({str(record["path"]) for record in ordered}) != len(ordered):
        raise CombRefereeScopeError("snapshot contains duplicate logical paths")
    return {
        "file_count": len(ordered),
        "bytes": sum(int(record["bytes"]) for record in ordered),
        "sha256": canonical_digest(ordered),
        "files": ordered,
    }


def _tree_manifest(root: pathlib.Path, logical_root: str) -> dict[str, Any]:
    if root.is_symlink() or not root.is_dir():
        raise CombRefereeScopeError(f"missing or unsafe tree: {logical_root}")
    records: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        logical = f"{logical_root}/{path.relative_to(root).as_posix()}"
        if path.is_symlink():
            raise CombRefereeScopeError(f"symlink is outside the scope: {logical}")
        if path.is_dir():
            continue
        records.append(_stable_file_record(path, logical))
    manifest = _file_manifest(records)
    manifest["root"] = logical_root
    return manifest


def _git(args: Sequence[str]) -> bytes:
    proc = subprocess.run(["git", *args], cwd=REPO, capture_output=True)
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", errors="replace").strip()
        raise CombRefereeScopeError(
            f"git {' '.join(args)} failed: {detail or 'no diagnostic'}")
    return proc.stdout


def _git_text(args: Sequence[str]) -> str:
    return _git(args).decode("utf-8", errors="strict").strip()


def _git_state() -> dict[str, Any]:
    head = _git_text(("rev-parse", "--verify", "HEAD"))
    tree = _git_text(("rev-parse", "--verify", "HEAD^{tree}"))
    status = _git(("status", "--porcelain=v1", "-z", "--untracked-files=all"))
    return {
        "commit": head,
        "tree": tree,
        "worktree_clean": status == b"",
    }


def _tracked_record(relative: str, head: str) -> dict[str, Any]:
    current = _stable_file_record(REPO / relative, relative)
    head_payload = _git(("show", f"{head}:{relative}"))
    current["head_sha256"] = sha256_bytes(head_payload)
    current["equals_head"] = (
        current["bytes"] == len(head_payload)
        and current["sha256"] == current["head_sha256"]
    )
    return current


def _layout_declared_inputs(
        layout_tree: dict[str, Any], head: str,
        source_root: pathlib.Path = COMB_REFEREE_SOURCE_ROOT,
        ) -> tuple[dict[str, Any], dict[str, Any]]:
    layout_paths = sorted((BUILD / "layout").glob("*.layout.json"))
    if len(layout_paths) != EXPECTED_FORMS:
        raise CombRefereeScopeError(
            f"layout corpus has {len(layout_paths)} files, expected {EXPECTED_FORMS}")
    tree_records = {record["path"]: record for record in layout_tree["files"]}
    provenance: list[dict[str, Any]] = []
    sources: list[dict[str, Any]] = []
    seen_slugs: set[str] = set()
    for layout_path in layout_paths:
        slug = layout_path.name.removesuffix(".layout.json")
        if (not slug or slug in seen_slugs
                or any(not (char.isalnum() or char in "-_") for char in slug)):
            raise CombRefereeScopeError(f"invalid or duplicate layout slug: {slug}")
        seen_slugs.add(slug)
        logical_layout = f"build/layout/{layout_path.name}"
        current_layout = _stable_file_record(layout_path, logical_layout)
        if tree_records.get(logical_layout) != current_layout:
            raise CombRefereeScopeError(
                f"layout changed while discovering inputs: {slug}")
        try:
            layout = json.loads(layout_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise CombRefereeScopeError(f"invalid layout for {slug}: {error}") from error
        source = layout.get("source") if isinstance(layout, dict) else None
        if not isinstance(source, dict):
            raise CombRefereeScopeError(f"layout source is missing: {slug}")
        declared = str(source.get("file", "")).split(":", 1)[-1]
        expected_sha = source.get("sha256")
        expected_bytes = source.get("bytes")
        if (not declared or not _is_sha256(expected_sha)
                or not _is_count(expected_bytes)):
            raise CombRefereeScopeError(f"layout source pin is incomplete: {slug}")

        matches = sorted((FORMS).glob(f"**/{slug}/provenance.json"))
        if len(matches) != 1:
            raise CombRefereeScopeError(
                f"expected one provenance file for {slug}, got {len(matches)}")
        relative_provenance = matches[0].relative_to(REPO).as_posix()
        provenance_record = _tracked_record(relative_provenance, head)
        if not provenance_record["equals_head"]:
            raise CombRefereeScopeError(
                f"provenance differs from HEAD: {relative_provenance}")
        provenance.append(provenance_record)

        try:
            candidates = sorted(
                candidate for candidate in source_root.rglob(declared)
                if candidate.is_file())
        except (OSError, ValueError) as error:
            raise CombRefereeScopeError(
                f"cannot resolve source PDF for {slug}: {error}") from error
        candidate_records = [
            _stable_file_record(
                candidate,
                candidate.relative_to(source_root).as_posix(),
            )
            for candidate in candidates
        ]
        matching = [
            record for record in candidate_records
            if (record["sha256"] == expected_sha
                and record["bytes"] == expected_bytes)
        ]
        if len(matching) != 1:
            raise CombRefereeScopeError(
                f"source PDF has {len(matching)} authoritative matches for "
                f"{slug}; exactly one is required")
        sources.append({
            "slug": slug,
            "declared_file": declared,
            "declared_sha256": expected_sha,
            "declared_bytes": expected_bytes,
            "layout_pin": dict(source),
            "candidate_count": len(candidate_records),
            "matching_count": len(matching),
            "selected": matching[0]["path"],
            "candidates": candidate_records,
        })
    provenance_manifest = _file_manifest(provenance)
    sources_manifest = {
        "relation_count": len(sources),
        "candidate_file_count": sum(item["candidate_count"] for item in sources),
        "sha256": canonical_digest(sources),
        "relations": sources,
    }
    return provenance_manifest, sources_manifest


AUDIT_ASSERTION_SUMMARY_KEYS = (
    "combs_expected", "combs_checked", "expected_comb_ids",
    "checked_comb_ids", "emitted_comb_ids",
    "unexpected_emitted_comb_ids", "duplicate_layout_comb_ids",
    "duplicate_emitted_cell_ids", "raw_live_comb_issues",
    "emitted_cell_binding_issues", "inventory_complete",
    "layout_mismatches", "layout_unevaluable",
    "owner_certificates_valid", "owner_certificates_invalid",
    "source_u_frame_evaluable", "source_certified_unframed_evaluable",
    "emission_behind_layout", "emission_invalid",
)
AUDIT_POSITION_FAILURE_KINDS = {
    "emission-layout-position-mismatch",
    "emission-layout-outer-position-mismatch",
    "emission-source-position-mismatch",
    "emission-source-outer-position-mismatch",
    "layout-source-outer-position-mismatch",
}
AUDIT_INVENTORY_FAILURE_KINDS = {
    "duplicate-layout-subject", "unexpected-emitted-comb",
    "emitted-cell-binding-invalid", "duplicate-emitted-cell-id",
    "missing-layout-cell-owner", "duplicate-layout-cell-owner",
    "emitted-cell-page-mismatch", "emitted-cell-geometry-mismatch",
    "unowned-live-comb-markup", "comb-inventory-mismatch",
    "emission-container-page-mismatch",
    "emission-container-geometry-mismatch",
    "comb-owner-registry-invalid",
}
AUDIT_LAYOUT_RELATIONS = {
    "match", "mismatch", "unevaluable", "duplicate-subject",
    "not-owned", "cell-binding-invalid", "inventory-invalid",
    "registry-invalid",
}
AUDIT_FAILURE_KINDS = {
    "source-topology-unevaluable", "layout-printed-mismatch",
    "duplicate-layout-subject", "emission-container-page-mismatch",
    "emission-container-geometry-mismatch",
    "emission-layout-position-mismatch",
    "emission-layout-outer-position-mismatch",
    "emission-source-position-mismatch",
    "emission-source-outer-position-mismatch",
    "layout-source-outer-position-mismatch", "invalid-emission",
    "emission-layout-mismatch", "emission-printed-mismatch",
    "unexpected-emitted-comb", "emitted-cell-binding-invalid",
    "duplicate-emitted-cell-id", "missing-layout-cell-owner",
    "duplicate-layout-cell-owner", "emitted-cell-page-mismatch",
    "emitted-cell-geometry-mismatch", "unowned-live-comb-markup",
    "comb-inventory-mismatch",
    "comb-owner-registry-invalid",
}


def _canonical_decimal_identity(value: Any) -> str:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CombRefereeScopeError("owner-certificate bbox is not numeric")
    try:
        number = Decimal(str(value))
    except InvalidOperation as error:
        raise CombRefereeScopeError(
            "owner-certificate bbox is not decimal") from error
    if not number.is_finite():
        raise CombRefereeScopeError("owner-certificate bbox is not finite")
    rendered = format(number, "f")
    if "." in rendered:
        rendered = rendered.rstrip("0").rstrip(".")
    return "0" if rendered in {"", "-0"} else rendered


def _normalise_owner_certificate(
        value: Any, expected: dict[str, Any] | None,
        ) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CombRefereeScopeError("audit offender owner certificate is missing")
    if value.get("criterion") != "exact-reviewed-layout-comb-subject-owner-v1":
        raise CombRefereeScopeError("audit offender owner criterion is invalid")
    if value.get("valid") is True:
        keys = {
            "criterion", "valid", "layout_sha256", "page", "cell_id",
            "legacy_cell_id", "subject_key", "legacy_bbox",
            "bbox_number_format", "state", "supplies_topology",
        }
        if set(value) != keys or value.get("supplies_topology") is not False:
            raise CombRefereeScopeError(
                "audit offender valid owner certificate schema is false")
        if expected is not None:
            expected_value = {
                "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
                "valid": True,
                "layout_sha256": expected["layout_sha256"],
                "page": expected["page"],
                "cell_id": expected["cell"],
                "legacy_cell_id": expected["legacy_cell_id"],
                "subject_key": expected["subject_key"],
                "legacy_bbox": [
                    _canonical_decimal_identity(item)
                    for item in expected["bbox"]
                ],
                "bbox_number_format": "canonical-decimal-string-v1",
                "state": expected["ledger_state"],
                "supplies_topology": False,
            }
            if value != expected_value:
                raise CombRefereeScopeError(
                    "audit offender owner certificate is not layout-bound")
        return value
    if (set(value) != {"criterion", "valid", "reason", "supplies_topology"}
            or value.get("valid") is not False
            or value.get("supplies_topology") is not False
            or not isinstance(value.get("reason"), str)
            or not value["reason"]):
        raise CombRefereeScopeError(
            "audit offender invalid owner certificate schema is false")
    return value


def _normalise_outer_offender(
        item: Any, expected_owner: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
    """Project trusted raw audit evidence into the child's public relation."""
    if not isinstance(item, dict):
        raise CombRefereeScopeError("audit comb offender is not an object")
    required = {
        "cell", "page", "slots", "latticed", "printed",
        "printed_divider_x", "emission_state", "physical_slots",
        "declared_slots", "emitted_occurrences", "layout_relation",
        "emission_relation", "failure_kinds", "why",
    }
    allowed = required | {
        "slot_indexes", "input_slot_indexes", "slot_geometry",
        "emission_container_binding", "emission_layout_position",
        "emission_layout_outer_position", "emission_source_position",
        "source_frame_geometry", "emission_source_outer_position",
        "layout_source_outer_position", "source_topology_evidence",
        "effective_emission_state", "source_owner_certificate",
        "emitted_cell_binding_evidence", "raw_dom_evidence",
    }
    if not required <= set(item) or set(item) - allowed:
        raise CombRefereeScopeError("audit comb offender schema is incomplete")
    cell = item.get("cell")
    page = item.get("page")
    slots = item.get("slots")
    latticed = item.get("latticed")
    printed = item.get("printed")
    physical = item.get("physical_slots")
    declared_slots = item.get("declared_slots")
    occurrences = item.get("emitted_occurrences")
    layout_relation = item.get("layout_relation")
    emission_state = item.get("emission_state")
    failure_kinds = item.get("failure_kinds")
    if not isinstance(cell, str) or not cell:
        raise CombRefereeScopeError("audit comb offender has no cell identity")
    if page is not None and (not _is_count(page) or page < 1):
        raise CombRefereeScopeError(f"audit comb offender page is invalid: {cell}")
    for name, value in (("slots", slots), ("latticed", latticed),
                        ("printed", printed), ("physical_slots", physical),
                        ("declared_slots", declared_slots)):
        if value is not None and not _is_count(value):
            raise CombRefereeScopeError(
                f"audit comb offender {name} is invalid: {cell}")
    if slots is not None and physical is not None and slots != physical:
        raise CombRefereeScopeError(
            f"audit comb offender physical slot count is false: {cell}")
    divider_x = item.get("printed_divider_x")
    if (not _finite_number_list(divider_x)
            or (printed is None and divider_x)
            or (printed is not None and len(divider_x) != max(0, printed - 1))):
        raise CombRefereeScopeError(
            f"audit comb offender printed topology is invalid: {cell}")
    if not _is_count(occurrences):
        raise CombRefereeScopeError(
            f"audit comb offender occurrences are invalid: {cell}")
    if layout_relation not in AUDIT_LAYOUT_RELATIONS:
        raise CombRefereeScopeError(
            f"audit comb offender layout relation is invalid: {cell}")
    if not isinstance(emission_state, str) or not emission_state:
        raise CombRefereeScopeError(
            f"audit comb offender emission state is invalid: {cell}")
    if (not _string_list(failure_kinds, nonempty=True)
            or set(failure_kinds) - AUDIT_FAILURE_KINDS):
        raise CombRefereeScopeError(
            f"audit comb offender failures are invalid: {cell}")
    if (not isinstance(item.get("emission_relation"), str)
            or not item["emission_relation"]
            or not isinstance(item.get("why"), str) or not item["why"]):
        raise CombRefereeScopeError(
            f"audit comb offender explanation/relation is invalid: {cell}")

    kinds = set(failure_kinds)
    normal_subject = layout_relation in {"match", "mismatch", "unevaluable"}
    if normal_subject and (
            "emission_container_binding" not in item
            or any(field not in item for field in (
                "emission_layout_position", "emission_layout_outer_position",
                "emission_source_position", "emission_source_outer_position",
                "layout_source_outer_position"))):
        raise CombRefereeScopeError(
            f"audit comb offender omits normal-subject geometry: {cell}")
    owner_certificate = item.get("source_owner_certificate")
    if normal_subject or layout_relation == "duplicate-subject":
        owner_certificate = _normalise_owner_certificate(
            owner_certificate, expected_owner)
    elif layout_relation == "registry-invalid":
        owner_certificate = _normalise_owner_certificate(
            owner_certificate, None)
        if (owner_certificate.get("valid") is not False
                or cell != "<comb-owner-registry>"
                or page is not None
                or any(value is not None for value in (
                    slots, latticed, printed, physical, declared_slots))
                or divider_x != [] or occurrences != 0
                or emission_state != "not-evaluated"
                or item.get("effective_emission_state") != "not-evaluated"
                or item.get("emission_relation") != "not-evaluated"
                or failure_kinds != ["comb-owner-registry-invalid"]):
            raise CombRefereeScopeError(
                "audit comb owner-registry offender is malformed")
    elif owner_certificate is not None:
        raise CombRefereeScopeError(
            f"non-owned audit offender invents owner certificate: {cell}")
    position_mismatch = bool(kinds & AUDIT_POSITION_FAILURE_KINDS)
    container_mismatch = bool(kinds & {
        "emission-container-page-mismatch",
        "emission-container-geometry-mismatch",
    })
    binding_invalid = position_mismatch or container_mismatch
    physical_emission_valid = emission_state == "physical-slots"
    emission_invalid = not physical_emission_valid or binding_invalid
    emission_behind = bool(
        layout_relation == "duplicate-subject"
        or not physical_emission_valid
        or binding_invalid
        or (slots is not None and latticed is not None and slots != latticed)
        or kinds & {"unexpected-emitted-comb", "unowned-live-comb-markup"}
    )
    if not normal_subject:
        emission_invalid = bool(
            "unowned-live-comb-markup" in kinds
            or ("unexpected-emitted-comb" in kinds
                and not physical_emission_valid)
            or (layout_relation == "duplicate-subject"
                and not physical_emission_valid)
        )
        emission_behind = bool(
            kinds & {"unexpected-emitted-comb", "unowned-live-comb-markup"}
            or layout_relation == "duplicate-subject"
        )
    dimensions = {
        "layout_mismatch": layout_relation == "mismatch",
        "source_unevaluable": layout_relation in {
            "unevaluable", "duplicate-subject", "inventory-invalid"},
        "emission_invalid": emission_invalid,
        "emission_behind": emission_behind,
        "position_mismatch": position_mismatch,
        "inventory_binding": bool(kinds & AUDIT_INVENTORY_FAILURE_KINDS),
    }
    if not any(dimensions.values()):
        raise CombRefereeScopeError(
            f"audit comb offender has no failure dimension: {cell}")
    return {
        "cell": cell,
        "page": page,
        "slots": slots,
        "latticed": latticed,
        "printed": printed,
        "emitted_occurrences": occurrences,
        "layout_relation": layout_relation,
        "emission_state": emission_state,
        "failure_kinds": failure_kinds,
        "source_owner_certificate": owner_certificate,
        "dimensions": dimensions,
    }


def _layout_audit_owner_ids(layout_binding: Any) -> list[str]:
    """Return the exact ordered non-relocated comb-owner registry.

    ``audit_expected_ids`` is derived from the layout cell stream while
    ``cells`` is derived independently from the reviewed subject ledger.  A
    summary count is trustworthy only when those two byte-bound projections
    name the same active emission owners in the same order.
    """
    if not isinstance(layout_binding, dict):
        raise CombRefereeScopeError("parsed layout-owner registry is missing")
    audit_ids = layout_binding.get("audit_expected_ids")
    cells = layout_binding.get("cells")
    if (not isinstance(audit_ids, list)
            or not all(isinstance(item, str) and item for item in audit_ids)
            or len(audit_ids) != len(set(audit_ids))
            or not isinstance(cells, dict)):
        raise CombRefereeScopeError(
            "parsed layout-owner registry is malformed")
    owner_ids: list[str] = []
    for cell_id, cell in cells.items():
        if (not isinstance(cell_id, str) or not cell_id
                or not isinstance(cell, dict)
                or cell.get("cell") != cell_id):
            raise CombRefereeScopeError(
                "parsed layout-owner registry has a malformed subject")
        if (cell.get("ledger_state") in {
                "active_resolved", "active_unresolved"}
                and cell.get("expected_emission_geometry") is not None):
            owner_ids.append(cell_id)
    if audit_ids != owner_ids:
        raise CombRefereeScopeError(
            "layout cell and reviewed-subject owner registries differ")
    return owner_ids


def _normalise_outer_comb_assertion(
        assertion: Any, layout_binding: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
    """Require exhaustive offender publication and expose an exact cell map."""
    if not isinstance(assertion, dict):
        raise CombRefereeScopeError("comb audit assertion is missing")
    missing = [key for key in AUDIT_ASSERTION_SUMMARY_KEYS
               if key not in assertion]
    if missing:
        raise CombRefereeScopeError(
            "comb audit assertion omits: " + ", ".join(missing))
    holds = assertion.get("holds")
    offenders = assertion.get("offenders")
    if not isinstance(holds, bool) or not isinstance(offenders, list):
        raise CombRefereeScopeError(
            "comb audit verdict/offender inventory is malformed")
    expected_ids = assertion.get("expected_comb_ids")
    checked_ids = assertion.get("checked_comb_ids")
    if (not isinstance(expected_ids, list)
            or not all(isinstance(item, str) and item for item in expected_ids)
            or len(expected_ids) != len(set(expected_ids))
            or checked_ids != expected_ids
            or assertion.get("combs_expected") != len(expected_ids)
            or assertion.get("combs_checked") != len(expected_ids)):
        raise CombRefereeScopeError(
            "comb audit checked inventory is incomplete or duplicated")
    for key in (
            "emitted_comb_ids", "unexpected_emitted_comb_ids",
            "duplicate_layout_comb_ids", "duplicate_emitted_cell_ids"):
        values = assertion.get(key)
        if (not isinstance(values, list)
                or not all(isinstance(item, str) and item for item in values)
                or len(values) != len(set(values))):
            raise CombRefereeScopeError(
                f"comb audit inventory is malformed: {key}")
    for key in (
            "raw_live_comb_issues", "emitted_cell_binding_issues",
            "layout_mismatches", "layout_unevaluable",
            "owner_certificates_valid", "owner_certificates_invalid",
            "source_u_frame_evaluable",
            "source_certified_unframed_evaluable",
            "emission_behind_layout", "emission_invalid"):
        if not _is_count(assertion.get(key)):
            raise CombRefereeScopeError(
                f"comb audit count is malformed: {key}")
    if not isinstance(assertion.get("inventory_complete"), bool):
        raise CombRefereeScopeError(
            "comb audit inventory-complete flag is malformed")
    if (assertion["owner_certificates_valid"]
            + assertion["owner_certificates_invalid"]
            != assertion["combs_checked"]):
        raise CombRefereeScopeError(
            "comb audit owner-certificate partition is false")

    count = assertion.get("offender_count", 0 if holds else None)
    published = assertion.get("offenders_published", 0 if holds else None)
    omitted = assertion.get("offenders_omitted", 0 if holds else None)
    complete = assertion.get("offenders_complete", True if holds else None)
    if (not _is_count(count) or not _is_count(published)
            or not _is_count(omitted) or not isinstance(complete, bool)
            or count != len(offenders) or published != len(offenders)
            or count != published + omitted or omitted != 0
            or complete is not True or (holds and offenders)):
        raise CombRefereeScopeError(
            "comb audit offender publication is truncated or inconsistent")

    dimensions: dict[str, Any] = {}
    raw_offenders_by_cell: dict[str, dict[str, Any]] = {}
    for raw in offenders:
        raw_cell = raw.get("cell") if isinstance(raw, dict) else None
        expected_owner = None
        if isinstance(layout_binding, dict) and isinstance(raw_cell, str):
            projected = layout_binding.get("cells", {}).get(raw_cell)
            if isinstance(projected, dict):
                expected_owner = {
                    **projected,
                    "layout_sha256": layout_binding.get("layout_sha256"),
                }
        relation = _normalise_outer_offender(raw, expected_owner)
        cell = relation["cell"]
        if cell in dimensions:
            raise CombRefereeScopeError(
                f"comb audit publishes duplicate offender: {cell}")
        dimensions[cell] = relation
        raw_offenders_by_cell[cell] = raw
    expected_set = set(expected_ids)
    emitted_ids = assertion["emitted_comb_ids"]
    emitted_set = set(emitted_ids)
    unexpected_ids = assertion["unexpected_emitted_comb_ids"]
    duplicate_layout = assertion["duplicate_layout_comb_ids"]
    duplicate_emitted = assertion["duplicate_emitted_cell_ids"]
    if (emitted_ids != sorted(emitted_ids)
            or unexpected_ids != sorted(emitted_set - expected_set)):
        raise CombRefereeScopeError(
            "comb audit emitted/unexpected inventories are not derived")
    if any(cell_id not in expected_set for cell_id in duplicate_layout):
        raise CombRefereeScopeError(
            "comb audit duplicate-layout inventory has no expected owner")
    if duplicate_layout != sorted(duplicate_layout):
        raise CombRefereeScopeError(
            "comb audit duplicate-layout inventory is not canonical")
    if duplicate_emitted != sorted(duplicate_emitted):
        raise CombRefereeScopeError(
            "comb audit duplicate-emitted inventory is not canonical")

    unexpected_offenders: set[str] = set()
    duplicate_layout_offenders: set[str] = set()
    raw_live_issues = 0
    binding_issue_cells: set[str] = set()
    inventory_failure = False
    for cell_id, relation in dimensions.items():
        kinds = set(relation["failure_kinds"])
        layout_relation = relation["layout_relation"]
        if layout_relation in {"match", "mismatch", "unevaluable"}:
            if cell_id not in expected_set:
                raise CombRefereeScopeError(
                    f"comb audit normal offender is orphaned: {cell_id}")
        elif layout_relation == "duplicate-subject":
            if cell_id not in duplicate_layout:
                raise CombRefereeScopeError(
                    f"comb audit duplicate offender is unlisted: {cell_id}")
            duplicate_layout_offenders.add(cell_id)
        elif "unexpected-emitted-comb" in kinds:
            if cell_id not in unexpected_ids or cell_id not in emitted_set:
                raise CombRefereeScopeError(
                    f"comb audit unexpected offender is orphaned: {cell_id}")
            unexpected_offenders.add(cell_id)
            binding_issue_cells.add(cell_id)
        elif "unowned-live-comb-markup" in kinds:
            raw_live_issues += 1
        elif "emitted-cell-binding-invalid" in kinds:
            binding_issue_cells.add(cell_id)
        elif layout_relation == "registry-invalid":
            if (cell_id != "<comb-owner-registry>"
                    or kinds != {"comb-owner-registry-invalid"}):
                raise CombRefereeScopeError(
                    "comb audit owner-registry offender identity is invalid")
            inventory_failure = True
        elif "comb-inventory-mismatch" in kinds:
            if cell_id != "<comb-inventory>":
                raise CombRefereeScopeError(
                    "comb audit inventory offender identity is invalid")
            inventory_failure = True
        else:
            raise CombRefereeScopeError(
                f"comb audit offender has no declared inventory owner: {cell_id}")
        if kinds & {
                "emission-container-page-mismatch",
                "emission-container-geometry-mismatch"}:
            binding_issue_cells.add(cell_id)

    if unexpected_offenders != set(unexpected_ids):
        raise CombRefereeScopeError(
            "comb audit unexpected inventory/offenders disagree")
    if duplicate_layout_offenders != set(duplicate_layout):
        raise CombRefereeScopeError(
            "comb audit duplicate-layout inventory/offenders disagree")
    published_certificates = {
        cell_id: relation["source_owner_certificate"]
        for cell_id, relation in dimensions.items()
        if isinstance(relation.get("source_owner_certificate"), dict)
        and relation.get("layout_relation") in {
            "match", "mismatch", "unevaluable", "duplicate-subject"}
    }
    published_valid = sum(
        certificate.get("valid") is True
        for certificate in published_certificates.values())
    published_invalid = len(published_certificates) - published_valid
    if (published_valid > assertion["owner_certificates_valid"]
            or published_invalid > assertion["owner_certificates_invalid"]):
        raise CombRefereeScopeError(
            "comb audit published owner certificates exceed summary counts")
    if set(published_certificates) == set(checked_ids) and (
            assertion["owner_certificates_valid"] != published_valid
            or assertion["owner_certificates_invalid"] != published_invalid):
        raise CombRefereeScopeError(
            "comb audit complete owner-certificate publication disagrees "
            "with summary counts")
    if isinstance(layout_binding, dict):
        projected_ids = _layout_audit_owner_ids(layout_binding)
        if (expected_ids != projected_ids
                or assertion["owner_certificates_valid"] != len(expected_ids)
                or assertion["owner_certificates_invalid"] != 0):
            raise CombRefereeScopeError(
                "comb audit owner-certificate summary disagrees with the "
                "exact parsed layout-owner registry")
    checked_source_unevaluable = {
        cell_id for cell_id, relation in dimensions.items()
        if cell_id in expected_set
        and relation["dimensions"]["source_unevaluable"]
    }
    source_evaluable = (
        assertion["combs_checked"] - len(checked_source_unevaluable))
    if (assertion["source_u_frame_evaluable"]
            + assertion["source_certified_unframed_evaluable"]
            != source_evaluable):
        raise CombRefereeScopeError(
            "comb audit source frame/unframed counts do not partition "
            "evaluable checked cells")
    published_u_frame = 0
    published_certified_unframed = 0
    for cell_id, relation in dimensions.items():
        if cell_id not in expected_set or relation["printed"] is None:
            continue
        certificate = relation.get("source_owner_certificate")
        if (not isinstance(certificate, dict)
                or certificate.get("valid") is not True):
            raise CombRefereeScopeError(
                f"comb audit measured source lacks a valid owner "
                f"certificate: {cell_id}")
        if raw_offenders_by_cell[cell_id].get("source_frame_geometry") is None:
            published_certified_unframed += 1
        else:
            published_u_frame += 1
    if published_u_frame > assertion["source_u_frame_evaluable"]:
        raise CombRefereeScopeError(
            "comb audit published U-frame source results exceed their count")
    if (published_certified_unframed
            > assertion["source_certified_unframed_evaluable"]):
        raise CombRefereeScopeError(
            "comb audit published certified-unframed source results exceed "
            "their count")
    for cell_id in expected_set - emitted_set:
        relation = dimensions.get(cell_id)
        if (not isinstance(relation, dict)
                or relation.get("emission_state") != "missing-emitted-cell"
                or not set(relation.get("failure_kinds", [])) & {
                    "invalid-emission", "duplicate-layout-subject"}):
            raise CombRefereeScopeError(
                f"comb audit omits missing-emission offender: {cell_id}")
    if any(cell_id not in dimensions for cell_id in duplicate_emitted):
        raise CombRefereeScopeError(
            "comb audit duplicate-emitted inventory has no offender")
    binding_issue_cells.update(duplicate_emitted)
    binding_issue_cells.update(
        cell_id for cell_id in duplicate_layout if cell_id in emitted_set)
    derived_counts = {
        "layout_mismatches": sum(
            relation["dimensions"]["layout_mismatch"]
            for relation in dimensions.values()),
        "layout_unevaluable": sum(
            relation["dimensions"]["source_unevaluable"]
            for relation in dimensions.values()),
        "emission_behind_layout": sum(
            relation["dimensions"]["emission_behind"]
            for relation in dimensions.values()),
        "emission_invalid": sum(
            relation["dimensions"]["emission_invalid"]
            for relation in dimensions.values()),
        "raw_live_comb_issues": raw_live_issues,
        "emitted_cell_binding_issues": len(binding_issue_cells),
    }
    for key, derived in derived_counts.items():
        if assertion.get(key) != derived:
            raise CombRefereeScopeError(
                f"comb audit summary counter is false: {key}")
    derived_inventory_complete = not (
        unexpected_ids
        or duplicate_layout
        or any(cell_id in expected_set or cell_id in emitted_set
               for cell_id in duplicate_emitted)
        or inventory_failure
        or raw_live_issues
        or binding_issue_cells
    )
    if assertion["inventory_complete"] is not derived_inventory_complete:
        raise CombRefereeScopeError(
            "comb audit inventory-complete relation is false")
    if holds is not (not dimensions):
        raise CombRefereeScopeError(
            "comb audit holds verdict disagrees with exhaustive offenders")
    return {
        **{key: assertion[key] for key in AUDIT_ASSERTION_SUMMARY_KEYS},
        "offender_count": count,
        "offenders_published": published,
        "offenders_omitted": omitted,
        "offender_dimensions": dimensions,
        "holds": holds,
    }


AUDIT_APPLICATION_ENVELOPE_KEYS = {
    "schema_version", "application_scope_name", "application_snapshot",
    "invocation", "raw_report", "relations", "host_tcb_required",
    "host_scope_complete", "host_closure_claimed", "operating_system_bound",
    "python_stdlib_bound", "dynamic_libraries_bound",
    "application_scope_complete", "enforceable", "enforcement_scope",
    "self_digest", "payload_sha256",
}
AUDIT_APPLICATION_INVOCATION_KEYS = {
    "executable", "resolved_executable", "python_flags",
    "pythonpath_removed", "pythonhome_removed", "timeout_seconds", "output",
    "child_exit",
}
AUDIT_APPLICATION_RAW_KEYS = {
    "file", "bytes", "sha256", "form_count",
}
AUDIT_APPLICATION_RELATIONS = {
    "clean_revision_before_after",
    "tracked_producers_equal_head_before_after",
    "declared_inputs_hashed_before_after",
    "python_executable_hashed_before_after",
    "sanitized_python_environment",
    "isolated_python_mode",
    "fresh_isolated_pycache_prefix",
    "hard_timeout_enforced",
    "audit_report_schema_valid",
    "validated_output_only",
    "atomic_report_publish",
    "atomic_envelope_publish",
}


def validate_audit_application_envelope(
        envelope: Any, audit_payload: bytes,
        current_scope: dict[str, Any] | None = None,
        ) -> list[str]:
    errors: list[str] = []
    if not isinstance(envelope, dict):
        return ["audit application envelope is not an object"]
    if set(envelope) != AUDIT_APPLICATION_ENVELOPE_KEYS:
        errors.append("audit application envelope schema is unsupported")
    if envelope.get("schema_version") != AUDIT_APPLICATION_ATTESTATION_VERSION:
        errors.append("audit application envelope version is unsupported")
    if envelope.get("application_scope_name") != AUDIT_APPLICATION_SCOPE:
        errors.append("audit application envelope scope is wrong")
    if not self_digest_valid(envelope):
        errors.append("audit application envelope self-digest is stale")
    relations = envelope.get("relations")
    if (not isinstance(relations, dict)
            or set(relations) != AUDIT_APPLICATION_RELATIONS
            or any(value is not True for value in relations.values())):
        errors.append("audit application relations are incomplete")
    boundary = {
        "host_tcb_required": True,
        "host_scope_complete": False,
        "host_closure_claimed": False,
        "operating_system_bound": False,
        "python_stdlib_bound": False,
        "dynamic_libraries_bound": False,
        "application_scope_complete": True,
        "enforceable": True,
        "enforcement_scope": "application-only",
    }
    for key, expected in boundary.items():
        if envelope.get(key) != expected:
            errors.append(f"audit application boundary is invalid: {key}")
    snapshot = envelope.get("application_snapshot")
    if not isinstance(snapshot, dict):
        errors.append("audit application snapshot is missing")
        snapshot = {}
    if current_scope is not None and snapshot != current_scope:
        errors.append("audit application envelope is stale")
    invocation = envelope.get("invocation")
    if not isinstance(invocation, dict):
        errors.append("audit application invocation is missing")
        invocation = {}
    elif set(invocation) != AUDIT_APPLICATION_INVOCATION_KEYS:
        errors.append("audit application invocation schema is unsupported")
    snapshot_python = snapshot.get("runtime", {}).get("python", {})
    if (invocation.get("executable") != sys.executable
            or invocation.get("resolved_executable")
            != snapshot_python.get("path")
            or invocation.get("python_flags")
            != ISOLATED_PYTHON_ATTESTED_FLAGS
            or invocation.get("pythonpath_removed") is not True
            or invocation.get("pythonhome_removed") is not True
            or invocation.get("timeout_seconds") != 5400
            or invocation.get("output") != "private-temporary-output"
            or invocation.get("child_exit") != 0):
        errors.append("audit application invocation contract is incomplete")
    raw = envelope.get("raw_report")
    if not isinstance(raw, dict):
        errors.append("audit application raw-report identity is missing")
        raw = {}
    elif set(raw) != AUDIT_APPLICATION_RAW_KEYS:
        errors.append("audit application raw-report schema is unsupported")
    try:
        form_count = len(json.loads(audit_payload))
    except (UnicodeError, json.JSONDecodeError, TypeError):
        form_count = -1
    if (raw.get("file") != "build/audit.json"
            or raw.get("bytes") != len(audit_payload)
            or raw.get("sha256") != sha256_bytes(audit_payload)
            or raw.get("form_count") != form_count):
        errors.append("audit application raw report is stale or unbound")
    return errors


def _audit_snapshot(application_scope: dict[str, Any]) -> dict[str, Any]:
    record = _stable_file_record(AUDIT_JSON, "build/audit.json")
    try:
        payload = AUDIT_JSON.read_bytes()
        data = json.loads(payload)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CombRefereeScopeError(f"audit JSON is malformed: {error}") from error
    if (sha256_bytes(payload) != record["sha256"]
            or not isinstance(data, list) or len(data) != EXPECTED_FORMS):
        raise CombRefereeScopeError(
            "audit JSON changed while hashing or has incomplete corpus coverage")
    try:
        envelope_payload = AUDIT_APPLICATION_ATTESTATION.read_bytes()
        envelope = json.loads(envelope_payload)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CombRefereeScopeError(
            f"audit application attestation is malformed: {error}") from error
    envelope_errors = validate_audit_application_envelope(
        envelope, payload, application_scope)
    if envelope_errors:
        raise CombRefereeScopeError("; ".join(envelope_errors[:5]))
    envelope_record = _stable_file_record(
        AUDIT_APPLICATION_ATTESTATION, "build/audit-attested.json")
    if envelope_record["sha256"] != sha256_bytes(envelope_payload):
        raise CombRefereeScopeError(
            "audit application attestation changed while hashing")
    payload_errors = full_audit_payload_errors(data, canonical_form_slugs())
    if payload_errors:
        raise CombRefereeScopeError(
            "audit JSON is not a complete producer-shaped report: "
            + "; ".join(payload_errors[:5]))
    forms: dict[str, Any] = {}
    for audit_form in data:
        if not isinstance(audit_form, dict):
            raise CombRefereeScopeError("audit JSON contains a non-object form")
        slug = audit_form.get("slug")
        manifest = audit_form.get("input_manifest")
        assertions = audit_form.get("assertions")
        assertion = (assertions.get("comb_slots_match_printed")
                     if isinstance(assertions, dict) else None)
        if (not isinstance(slug, str) or not slug or slug in forms
                or not isinstance(manifest, dict)
                or not isinstance(manifest.get("inputs"), dict)
                or not isinstance(manifest.get("render"), dict)
                or not isinstance(assertion, dict)):
            raise CombRefereeScopeError(
                f"audit per-form input relation is incomplete: {slug}")
        layout_binding = application_scope.get("layout_bindings", {}).get(slug)
        if not isinstance(layout_binding, dict):
            raise CombRefereeScopeError(
                f"audit form has no parsed layout binding: {slug}")
        assertion_relation = _normalise_outer_comb_assertion(
            assertion, layout_binding)
        forms[slug] = {
            "record_sha256": canonical_digest(audit_form),
            "input_manifest_sha256": canonical_digest(manifest),
            "inputs": manifest["inputs"],
            "render": manifest["render"],
            "assertion_sha256": canonical_digest(assertion),
            "assertion_relation": assertion_relation,
            "top_level_holds": audit_form.get("comb_slots_match_printed"),
        }
    if set(forms) != set(canonical_form_slugs()):
        raise CombRefereeScopeError(
            "audit JSON does not match the exact tracked form corpus")
    record["form_count"] = len(forms)
    record["forms_sha256"] = canonical_digest(forms)
    record["forms"] = forms
    record["application_attestation"] = envelope_record
    record["application_scope_attested"] = True
    return record


def capture_audit_application_snapshot() -> dict[str, Any]:
    """Capture every application byte consumed by the isolated audit run."""
    git = _git_state()
    if not git["worktree_clean"]:
        raise CombRefereeScopeError(
            "worktree is not clean (staged, unstaged, or untracked change)")
    producers = {
        relative: _tracked_record(relative, git["commit"])
        for relative in COMB_REFEREE_PRODUCERS
    }
    if not all(record["equals_head"] for record in producers.values()):
        changed = [name for name, record in producers.items()
                   if not record["equals_head"]]
        raise CombRefereeScopeError(
            "tracked producer bytes differ from HEAD: " + ", ".join(changed))

    python_path = pathlib.Path(sys.executable).resolve()
    poppler_name = shutil.which("pdftocairo")
    if poppler_name is None:
        raise CombRefereeScopeError("pdftocairo is not installed")
    poppler_path = pathlib.Path(poppler_name).resolve()
    artifact_trees = {
        name: _tree_manifest(path, f"build/{name}")
        for name, path in COMB_REFEREE_ARTIFACT_TREES.items()
    }
    layout_bindings = _layout_binding_snapshots(
        artifact_trees["layout"], artifact_trees["guides"],
        producers["tools/formgen/lattice.py"])
    provenance, sources = _layout_declared_inputs(
        artifact_trees["layout"], git["commit"])
    return {
        "git": git,
        "producers": producers,
        "runtime": {
            "python": _stable_file_record(python_path, str(python_path)),
            "pdftocairo": _stable_file_record(poppler_path, str(poppler_path)),
        },
        "artifact_trees": artifact_trees,
        "layout_bindings": layout_bindings,
        "provenance": provenance,
        "source_pdfs": sources,
    }


def capture_comb_referee_snapshot() -> dict[str, Any]:
    """Capture the complete application scope; host closure stays explicit."""
    application = capture_audit_application_snapshot()
    return {
        **application,
        "audit": _audit_snapshot(application),
    }


def snapshot_pair_errors(before: dict[str, Any],
                         after: dict[str, Any]) -> list[str]:
    """Pure validation used by the runner and its adversarial self-tests."""
    errors: list[str] = []
    for label, snapshot in (("before", before), ("after", after)):
        git = snapshot.get("git")
        if not isinstance(git, dict) or git.get("worktree_clean") is not True:
            errors.append(f"{label} worktree is dirty")
        producers = snapshot.get("producers")
        if (not isinstance(producers, dict)
                or set(producers) != set(COMB_REFEREE_PRODUCERS)
                or not all(isinstance(record, dict)
                           and record.get("equals_head") is True
                           for record in producers.values())):
            errors.append(f"{label} producer/HEAD binding is incomplete")
    before_git = before.get("git") if isinstance(before.get("git"), dict) else {}
    after_git = after.get("git") if isinstance(after.get("git"), dict) else {}
    if (before_git.get("commit") != after_git.get("commit")
            or before_git.get("tree") != after_git.get("tree")):
        errors.append("HEAD commit or tree changed during referee run")
    if before != after:
        changed = sorted(
            key for key in set(before) | set(after)
            if before.get(key) != after.get(key))
        errors.append("application snapshot changed: " + ", ".join(changed))
    return errors


SELF_DIGEST_CONTRACT = {
    "algorithm": "sha256",
    "canonicalization": "json-sort-keys-compact-utf8",
    "excluded_field": "payload_sha256",
}


def attach_self_digest(value: dict[str, Any]) -> None:
    if "self_digest" in value or "payload_sha256" in value:
        raise CombRefereeScopeError("self-digest is already attached")
    value["self_digest"] = dict(SELF_DIGEST_CONTRACT)
    value["payload_sha256"] = canonical_digest(value)


def self_digest_valid(value: dict[str, Any]) -> bool:
    claimed = value.get("payload_sha256")
    if (value.get("self_digest") != SELF_DIGEST_CONTRACT
            or not _is_sha256(claimed)):
        return False
    unsigned = {key: item for key, item in value.items()
                if key != "payload_sha256"}
    return claimed == canonical_digest(unsigned)


REPORT_KEYS = {
    "schema_version", "producer", "producer_sha256", "python_version",
    "provenance", "status", "status_reasons", "attestation", "poppler",
    "inputs", "totals", "errors", "forms", "self_digest",
    "payload_sha256",
}
REPORT_PROVENANCE_KEYS = {"producer", "dependencies", "runtime"}
REPORT_PRODUCER_KEYS = {"file", "bytes", "sha256"}
REPORT_DEPENDENCY_ROLE_KEYS = {"audit", "lattice"}
REPORT_AUDIT_DEPENDENCY_KEYS = {
    "file", "bytes", "sha256", "expected_sha256", "dependencies",
}
REPORT_PINNED_DEPENDENCY_KEYS = {
    "file", "bytes", "sha256", "expected_sha256",
}
REPORT_RUNTIME_KEYS = {
    "python_implementation", "python_version", "python_executable",
    "python_executable_sha256", "poppler",
}
REPORT_INPUT_KEYS = {"audit_sha256", "audit_bytes", "layout_count"}
REPORT_AUDIT_CHILD_DEPENDENCIES = (
    "tools/formgen/extract.py", "tools/formgen/verify.py",
)
TOTAL_KEYS = {
    "forms_expected", "forms_measured", "forms_error", "combs_expected",
    "combs_found", "combs_measured", "combs_unevaluable",
    "combs_source_unevaluable", "subjects_active",
    "subjects_active_resolved", "subjects_active_unresolved",
    "subjects_retained_unresolved", "inferences_suppressed",
    "ledger_blocking", "referee_layout_mismatches",
    "referee_layout_position_mismatches", "comparisons", "forms_ok",
    "forms_disagreement", "forms_unevaluable",
    "audit_evidence_complete_forms", "referee_attestation_complete",
    "referee_enforceable",
}
FORM_KEYS = {
    "slug", "status", "reason", "source", "artifacts", "lattice_evidence",
    "poppler", "pages", "audit_evidence", "emission_inventory",
    "emission_binding_errors", "counts", "inferences", "cells",
}
FORM_COUNT_KEYS = {
    "combs", "subjects", "subjects_active", "subjects_active_resolved",
    "subjects_active_unresolved", "subjects_retained_unresolved",
    "inferences_suppressed", "ledger_blocking", "measured",
    "source_unevaluable", "unevaluable", "referee_layout_mismatches",
    "referee_layout_position_mismatches", "emission_layout_mismatches",
    "comparisons",
}
CELL_KEYS = {
    "cell", "subject_key", "legacy_cell_id", "cell_id", "ledger_state",
    "ledger_blocks_gate", "ledger_reason_codes", "ledger_topology_sha256",
    "ledger_evidence", "page", "bbox", "latticed", "lattice_divider_x",
    "emitted", "emitted_indexes_valid", "emitted_evidence", "audit_printed",
    "audit_relation", "referee", "comparison_status", "comparison_reason",
    "transition_status", "transition_reason", "four_way",
}
INFERENCE_KEYS = {
    "page", "subject_key", "cell_id", "state", "blocks_gate",
    "reason_codes", "bbox", "topology_sha256", "ledger_evidence",
    "emitted_evidence",
}
AUDIT_EVIDENCE_KEYS = {
    "assertion_valid", "complete", "reason", "errors", "offender_count",
    "offenders_published", "offenders_omitted", "combs_expected",
    "combs_checked", "expected_comb_ids", "checked_comb_ids",
    "emitted_comb_ids", "unexpected_emitted_comb_ids",
    "duplicate_layout_comb_ids", "duplicate_emitted_cell_ids",
    "raw_live_comb_issues", "emitted_cell_binding_issues",
    "inventory_complete", "layout_mismatches", "layout_unevaluable",
    "owner_certificates_valid", "owner_certificates_invalid",
    "source_u_frame_evaluable", "source_certified_unframed_evaluable",
    "emission_behind_layout", "emission_invalid", "offender_dimensions",
    "holds", "input_manifest_verified", "input_manifest_reason",
    "manifest_binding", "ledger_binding", "evidence_published",
    "byte_and_relation_binding_valid",
    "runtime_closure_independently_attested", "integrity_valid",
}
MANIFEST_BINDING_KEYS = {
    "binding_valid", "manifest_inputs_complete", "attestation_complete",
    "enforceable", "complete", "reason", "errors", "blockers",
    "producer_sha256", "runtime_tree_sha256",
    "runtime_manifest_self_consistent",
    "base_runtime_closure_independently_attested",
    "roundtrip_runtime_closure_independently_attested",
    "render_dependency_count", "render_dependencies", "roundtrip_present",
}
LEDGER_BINDING_KEYS = {
    "binding_valid", "reason", "errors", "active_subject_ids",
    "emitted_ids", "legacy_alias_count",
}
EMISSION_INVENTORY_KEYS = {
    "complete", "reason", "expected_active_cell_ids", "emitted_cell_ids",
    "missing_active_cell_ids", "unexpected_emitted_cell_ids",
    "retained_emitted_cell_ids", "inference_emitted_cell_ids",
    "invalid_active_cell_ids",
}
FORM_POPPLER_KEYS = {
    "version", "binary_path", "binary_sha256", "identity_timeout_seconds",
    "page_timeout_seconds", "subprocess_cleanup_policy",
}
FORM_PAGE_KEYS = {
    "page", "svg_sha256", "vector_paints", "unsupported_regions",
}
MEASURED_REFEREE_KEYS = {
    "status", "reason", "y0", "y1", "source_divider_x",
    "extra_divider_x", "compartments", "anchor_matches",
    "positions_match", "anchors_complete", "subject_gap_proofs",
    "unproven_subject_gaps", "components", "contract_y0", "contract_y1",
    "open_y0", "open_y1", "contract_span_pt", "seed_span_pt",
    "measured_span_pt", "unmeasured_span_pt", "topology_coverage_pt",
    "ignored_slabs", "chosen_topology", "topology_superset_relations",
}
LEDGER_STATES = {
    "active_resolved", "active_unresolved", "retained_unresolved",
}
INFERENCE_STATE = "suppressed_unreviewed_inference"
RAW_REFEREE_ATTESTATION_KEYS = {
    "schema", "producer_and_declared_dependency_bytes_bound",
    "published_form_input_bytes_bound_before_after",
    "python_executable_fingerprinted",
    "python_executable_validated_before_after",
    "poppler_executable_bound_before_after",
    "poppler_invocations_have_hard_deadlines",
    "poppler_timeout_cleanup_policy", "clean_source_revision_bound",
    "python_stdlib_closure_bound", "python_dynamic_libraries_bound",
    "poppler_dynamic_libraries_bound",
    "operating_system_and_host_services_bound", "scope_complete",
    "complete", "enforceable", "incomplete_reasons",
    "future_gate_required",
}
RAW_REFEREE_INCOMPLETE_REASONS = [
    (
        "the standalone referee hashes its source and declared local "
        "dependencies but is not bound to a reviewed clean source revision"
    ),
    (
        "the Python standard library, Python dynamic libraries, Poppler "
        "dynamic libraries, and operating-system services are outside the "
        "independently rehashed application closure"
    ),
    (
        "the Python executable is fingerprinted for reporting but is not "
        "independently snapshotted and revalidated before and after the run"
    ),
]
RAW_REFEREE_FUTURE_GATE = (
    "trusted clean-source and host/runtime closure binding")
RAW_AUDIT_SCOPE_BLOCKERS = [
    "audit producer/runtime attestation is incomplete",
    "audit evidence is not yet enforceable",
    "audit input manifest is intentionally non-gating",
    "audit base runtime scope is incomplete",
    (
        "audit PyMuPDF/application runtime closure is manifest-self-consistent "
        "only; the referee independently rehashes the Python executable but "
        "not every named module or native dependency"
    ),
    "audit roundtrip native runtime scope is incomplete",
    (
        "audit Playwright/Chromium closure is manifest-schema checked but not "
        "independently rehashed by the standalone referee"
    ),
]


def _raw_referee_attestation_errors(value: Any) -> list[str]:
    if not isinstance(value, dict) or set(value) != RAW_REFEREE_ATTESTATION_KEYS:
        return ["report raw attestation schema is unsupported"]
    expected = {
        "schema": "comb-referee-runtime-attestation-v1",
        "producer_and_declared_dependency_bytes_bound": True,
        "published_form_input_bytes_bound_before_after": True,
        "python_executable_fingerprinted": True,
        "python_executable_validated_before_after": False,
        "poppler_executable_bound_before_after": True,
        "poppler_invocations_have_hard_deadlines": True,
        "poppler_timeout_cleanup_policy": "kill-isolated-process-group",
        "clean_source_revision_bound": False,
        "python_stdlib_closure_bound": False,
        "python_dynamic_libraries_bound": False,
        "poppler_dynamic_libraries_bound": False,
        "operating_system_and_host_services_bound": False,
        "scope_complete": False,
        "complete": False,
        "enforceable": False,
    }
    errors = [
        f"report raw attestation relation is false: {key}"
        for key, expected_value in expected.items()
        if value.get(key) != expected_value
    ]
    if value.get("incomplete_reasons") != RAW_REFEREE_INCOMPLETE_REASONS:
        errors.append("report raw attestation reasons are not the exact contract")
    if value.get("future_gate_required") != RAW_REFEREE_FUTURE_GATE:
        errors.append("report raw attestation future gate is not the exact contract")
    return errors


def _transition_for_cell(ledger_state: str, comparison_status: str
                         ) -> tuple[str, str]:
    if ledger_state == "active_resolved":
        return "none", "active ledger subject is already resolved"
    if ledger_state == "active_unresolved":
        if comparison_status == "agree":
            return (
                "eligible-for-reviewed-resolution",
                "four-way evidence agrees; explicit review is still required",
            )
        return (
            "blocked",
            "active unresolved ledger subject remains blocking while "
            f"comparison status is {comparison_status}",
        )
    if ledger_state == "retained_unresolved":
        return (
            "explicit-transition-required",
            "retained unresolved subject has no active topology; an explicit "
            "ledger transition is required",
        )
    return "invalid", "invalid ledger state"


def _finite_number(value: Any) -> bool:
    return (isinstance(value, (int, float))
            and not isinstance(value, bool)
            and math.isfinite(float(value)))


def _finite_number_list(value: Any, *, length: int | None = None) -> bool:
    return (isinstance(value, list)
            and (length is None or len(value) == length)
            and all(_finite_number(item) for item in value))


def _comparison_for_cell(
        cell: dict[str, Any], audit_complete: bool,
        ) -> tuple[str, str]:
    """Mirror comb_referee.comparison; published labels are never trusted."""
    ledger_state = cell.get("ledger_state")
    if ledger_state not in {"active_resolved", "active_unresolved"}:
        return (
            "unevaluable",
            "ledger subject has no active topology for adjudication",
        )
    lattice = cell.get("latticed")
    emitted = cell.get("emitted")
    referee = cell.get("referee")
    if emitted != lattice or cell.get("emitted_indexes_valid") is not True:
        return "stale-generation", "emitted physical slots disagree with lattice"
    if not audit_complete or cell.get("audit_printed") is None:
        return "unevaluable", "audit evidence is incomplete"
    if not isinstance(referee, dict) or referee.get("status") != "measured":
        reason = referee.get("reason", "no reason") if isinstance(
            referee, dict) else "no reason"
        return "unevaluable", f"referee: {reason}"
    if referee.get("positions_match") is not True:
        return "stop", "referee positions disagree with lattice anchors"
    source = referee.get("compartments")
    audit = cell.get("audit_printed")
    if not _is_count(source) or not _is_count(lattice) or not _is_count(audit):
        raise ValueError("comparison operands are not exact nonnegative integers")
    if source == lattice == audit:
        return "agree", "referee, lattice, audit, and emitted agree"
    if source == audit and source != lattice:
        return "repair-lattice", "referee and audit agree against lattice"
    if source == lattice and source != audit:
        return "repair-audit", "referee and lattice agree against audit"
    if lattice == audit and source != lattice:
        return "stop", "lattice and audit agree against the independent referee"
    return "stop", "referee, lattice, and audit all differ"


def _form_status_relation(
        *, ledger_blocking: int, emission_inventory: dict[str, Any],
        audit_evidence: dict[str, Any], comparisons: dict[str, int],
        ) -> tuple[str, str]:
    """Mirror the producer's ordered form status/reason relation exactly."""
    status = "ok"
    reasons: list[str] = []
    if ledger_blocking:
        status = "unevaluable"
        reasons.append(f"{ledger_blocking} lattice-ledger blockers")
    if emission_inventory.get("complete") is not True:
        status = "unevaluable"
        reasons.append(
            "emission inventory incomplete: "
            f"{emission_inventory.get('reason')}")
    if audit_evidence.get("complete") is not True:
        status = "unevaluable"
        reasons.append(
            f"audit evidence incomplete: {audit_evidence.get('reason')}")
    if comparisons["unevaluable"]:
        status = "unevaluable"
        reasons.append(f"{comparisons['unevaluable']} combs unevaluable")
    if status != "unevaluable" and any(
            comparisons[name] for name in (
                "repair-lattice", "repair-audit", "stale-generation", "stop")):
        status = "disagreement"
        reasons.append("one or more four-way comparisons disagree")
    return status, ", ".join(reasons) if reasons else "all combs measured"


def _string_list(value: Any, *, nonempty: bool = False) -> bool:
    return (isinstance(value, list)
            and (not nonempty or bool(value))
            and all(isinstance(item, str) and item for item in value))


def _report_poppler_identity_valid(value: Any) -> bool:
    return bool(
        isinstance(value, dict)
        and set(value) == FORM_POPPLER_KEYS
        and isinstance(value.get("version"), str) and value["version"]
        and isinstance(value.get("binary_path"), str)
        and value["binary_path"]
        and _is_sha256(value.get("binary_sha256"))
        and value.get("identity_timeout_seconds") == 10.0
        and value.get("page_timeout_seconds") == 60.0
        and value.get("subprocess_cleanup_policy")
        == "kill-isolated-process-group"
    )


def _report_provenance_schema_errors(report: dict[str, Any]) -> list[str]:
    """Validate the child's exact declared producer/runtime closure."""
    errors: list[str] = []
    provenance = report.get("provenance")
    if not isinstance(provenance, dict) or set(provenance) != REPORT_PROVENANCE_KEYS:
        return ["report provenance schema is unsupported"]
    producer = provenance.get("producer")
    if (not isinstance(producer, dict)
            or set(producer) != REPORT_PRODUCER_KEYS
            or producer.get("file") != "tools/formgen/comb_referee.py"
            or not _is_count(producer.get("bytes"))
            or not _is_sha256(producer.get("sha256"))
            or producer.get("sha256") != report.get("producer_sha256")):
        errors.append("report producer provenance is malformed")

    dependencies = provenance.get("dependencies")
    if (not isinstance(dependencies, dict)
            or set(dependencies) != REPORT_DEPENDENCY_ROLE_KEYS):
        errors.append("report dependency-role inventory is not exact")
        dependencies = {}
    audit = dependencies.get("audit")
    if (not isinstance(audit, dict)
            or set(audit) != REPORT_AUDIT_DEPENDENCY_KEYS
            or audit.get("file") != "tools/formgen/audit.py"
            or not _is_count(audit.get("bytes"))
            or not _is_sha256(audit.get("sha256"))
            or audit.get("expected_sha256") != audit.get("sha256")):
        errors.append("report audit dependency provenance is malformed")
        audit = {}
    audit_children = audit.get("dependencies")
    if not isinstance(audit_children, list):
        errors.append("report audit child-dependency inventory is malformed")
        audit_children = []
    child_files = [
        item.get("file") if isinstance(item, dict) else None
        for item in audit_children
    ]
    if child_files != list(REPORT_AUDIT_CHILD_DEPENDENCIES):
        errors.append("report audit child-dependency inventory is not exact")
    for expected_file, child in zip(
            REPORT_AUDIT_CHILD_DEPENDENCIES, audit_children):
        if (not isinstance(child, dict)
                or set(child) != REPORT_PINNED_DEPENDENCY_KEYS
                or child.get("file") != expected_file
                or not _is_count(child.get("bytes"))
                or not _is_sha256(child.get("sha256"))
                or child.get("expected_sha256") != child.get("sha256")):
            errors.append(
                f"report audit child dependency is malformed: {expected_file}")

    lattice = dependencies.get("lattice")
    if (not isinstance(lattice, dict)
            or set(lattice) != REPORT_PINNED_DEPENDENCY_KEYS
            or lattice.get("file") != "tools/formgen/lattice.py"
            or not _is_count(lattice.get("bytes"))
            or not _is_sha256(lattice.get("sha256"))
            or lattice.get("expected_sha256") != lattice.get("sha256")):
        errors.append("report lattice dependency provenance is malformed")

    runtime = provenance.get("runtime")
    if (not isinstance(runtime, dict) or set(runtime) != REPORT_RUNTIME_KEYS):
        errors.append("report runtime provenance schema is unsupported")
        runtime = {}
    if (not isinstance(runtime.get("python_implementation"), str)
            or not runtime.get("python_implementation")
            or not isinstance(runtime.get("python_version"), str)
            or not runtime.get("python_version")
            or runtime.get("python_version") != report.get("python_version")
            or not isinstance(runtime.get("python_executable"), str)
            or not runtime.get("python_executable")
            or not _is_sha256(runtime.get("python_executable_sha256"))):
        errors.append("report Python runtime provenance is malformed")
    poppler = report.get("poppler")
    if (not _report_poppler_identity_valid(poppler)
            or runtime.get("poppler") != poppler):
        errors.append("report Poppler runtime provenance is malformed")

    inputs = report.get("inputs")
    if (not isinstance(inputs, dict) or set(inputs) != REPORT_INPUT_KEYS
            or not _is_sha256(inputs.get("audit_sha256"))
            or not _is_count(inputs.get("audit_bytes"))
            or not _is_count(inputs.get("layout_count"))):
        errors.append("report input identity schema is unsupported")
    return errors


def validate_comb_referee_report(
        report: Any, *, child_exit: int | None = None,
        expected_forms: int = EXPECTED_FORMS,
        expected_subjects: int = EXPECTED_COMB_SUBJECTS,
        ) -> tuple[list[str], dict[str, Any]]:
    """Validate v2 shape, digest, and internally recomputed corpus totals."""
    errors: list[str] = []
    stats: dict[str, Any] = {
        "pending_transitions": 0,
        "referee_layout_mismatches": 0,
        "referee_layout_position_mismatches": 0,
        "emission_layout_mismatches": 0,
        "application_status": "unevaluable",
    }
    if not isinstance(report, dict):
        return ["report is not an object"], stats
    if set(report) != REPORT_KEYS:
        errors.append("report top-level schema is incomplete or unsupported")
    if report.get("schema_version") != COMB_REFEREE_REPORT_VERSION:
        errors.append("report schema version is not supported")
    if report.get("producer") != "tools/formgen/comb_referee.py":
        errors.append("report producer identity is invalid")
    if not _is_sha256(report.get("producer_sha256")):
        errors.append("report producer digest is invalid")
    if not self_digest_valid(report):
        errors.append("report self-digest is missing or stale")
    status = report.get("status")
    if status not in {"ok", "disagreement", "unevaluable"}:
        errors.append("report status is invalid")
    reasons = report.get("status_reasons")
    if (not isinstance(reasons, list)
            or not all(isinstance(reason, str) and reason for reason in reasons)):
        errors.append("report status reasons are malformed")
    attestation = report.get("attestation")
    errors.extend(_raw_referee_attestation_errors(attestation))
    if not isinstance(attestation, dict):
        attestation = {}
    if not isinstance(report.get("python_version"), str) or not report.get(
            "python_version"):
        errors.append("report Python version is malformed")
    errors.extend(_report_provenance_schema_errors(report))

    expected_exit = {"ok": 0, "disagreement": 1, "unevaluable": 2}.get(status)
    if child_exit is not None and child_exit != expected_exit:
        errors.append("child exit code disagrees with report status")

    totals = report.get("totals")
    if not isinstance(totals, dict) or set(totals) != TOTAL_KEYS:
        errors.append("report totals schema is incomplete or unsupported")
        totals = {}
    for key in TOTAL_KEYS - {
            "comparisons", "referee_attestation_complete",
            "referee_enforceable"}:
        if not _is_count(totals.get(key)):
            errors.append(f"report total is invalid: {key}")
    for key in ("referee_attestation_complete", "referee_enforceable"):
        if not isinstance(totals.get(key), bool):
            errors.append(f"report total is not boolean: {key}")
    comparisons = totals.get("comparisons")
    if (not isinstance(comparisons, dict)
            or set(comparisons) != set(COMPARISON_NAMES)
            or not all(_is_count(comparisons.get(name))
                       for name in COMPARISON_NAMES)):
        errors.append("report comparison totals are malformed")
        comparisons = {name: 0 for name in COMPARISON_NAMES}

    raw_errors = report.get("errors")
    if (not isinstance(raw_errors, list)
            or not all(isinstance(item, dict)
                       and isinstance(item.get("slug"), str)
                       and isinstance(item.get("error"), str)
                       for item in raw_errors)):
        errors.append("report error inventory is malformed")
        raw_errors = []
    forms = report.get("forms")
    if not isinstance(forms, list):
        errors.append("report forms inventory is missing")
        forms = []
    if len(forms) != expected_forms:
        errors.append(
            f"report is partial: {len(forms)}/{expected_forms} forms")

    slugs: set[str] = set()
    corpus_cell_ids: set[str] = set()
    corpus_subject_keys: set[str] = set()
    recomputed = {
        "combs": 0, "measured": 0, "source_unevaluable": 0,
        "unevaluable": 0, "ledger_blocking": 0,
        "subjects_active": 0, "subjects_active_resolved": 0,
        "subjects_active_unresolved": 0,
        "subjects_retained_unresolved": 0, "inferences_suppressed": 0,
        "referee_layout_mismatches": 0,
        "referee_layout_position_mismatches": 0,
        "emission_layout_mismatches": 0,
        "audit_evidence_complete_forms": 0,
        **{f"comparison:{name}": 0 for name in COMPARISON_NAMES},
        "forms_ok": 0, "forms_disagreement": 0, "forms_unevaluable": 0,
    }
    for form in forms:
        if not isinstance(form, dict) or set(form) != FORM_KEYS:
            errors.append("form report schema is incomplete or unsupported")
            continue
        slug = form.get("slug")
        if not isinstance(slug, str) or not slug or slug in slugs:
            errors.append("form report has a missing or duplicate slug")
        else:
            slugs.add(slug)
        form_status = form.get("status")
        if form_status not in {"ok", "disagreement", "unevaluable"}:
            errors.append(f"form status is invalid: {slug}")
        if not isinstance(form.get("reason"), str) or not form["reason"]:
            errors.append(f"form reason is malformed: {slug}")
        counts = form.get("counts")
        cells = form.get("cells")
        inferences = form.get("inferences")
        if (not isinstance(counts, dict) or not isinstance(cells, list)
                or not isinstance(inferences, list)):
            errors.append(f"form counts/cells/inferences are malformed: {slug}")
            continue
        if set(counts) != FORM_COUNT_KEYS:
            errors.append(f"form totals schema is incomplete or unsupported: {slug}")
        for key in FORM_COUNT_KEYS - {"comparisons"}:
            if not _is_count(counts.get(key)):
                errors.append(f"form total is invalid: {slug}/{key}")
        form_comparisons = counts.get("comparisons")
        if (not isinstance(form_comparisons, dict)
                or set(form_comparisons) != set(COMPARISON_NAMES)
                or not all(_is_count(form_comparisons.get(name))
                           for name in COMPARISON_NAMES)):
            errors.append(f"form comparison totals are invalid: {slug}")
            form_comparisons = {name: 0 for name in COMPARISON_NAMES}
        if counts.get("combs") != len(cells):
            errors.append(f"form cell inventory is partial: {slug}")
        audit_evidence = form.get("audit_evidence")
        emission_inventory = form.get("emission_inventory")
        if (not isinstance(audit_evidence, dict)
                or set(audit_evidence) != AUDIT_EVIDENCE_KEYS
                or not isinstance(audit_evidence.get("complete"), bool)
                or not isinstance(audit_evidence.get("reason"), str)
                or not audit_evidence.get("reason")):
            errors.append(f"form audit evidence schema is malformed: {slug}")
            audit_evidence = {"complete": False, "reason": "invalid"}
        else:
            if (audit_evidence["complete"] is True
                    and audit_evidence["reason"] != "complete"):
                errors.append(f"form audit-complete reason is false: {slug}")
            source_u_frame = audit_evidence.get("source_u_frame_evaluable")
            source_unframed = audit_evidence.get(
                "source_certified_unframed_evaluable")
            checked_count = audit_evidence.get("combs_checked")
            expected_ids = audit_evidence.get("expected_comb_ids")
            checked_ids = audit_evidence.get("checked_comb_ids")
            offender_dimensions = audit_evidence.get("offender_dimensions")
            source_accounting_malformed = bool(
                not _is_count(source_u_frame)
                or not _is_count(source_unframed)
                or not _is_count(checked_count)
                or not isinstance(expected_ids, list)
                or not all(isinstance(item, str) and item
                           for item in (expected_ids or []))
                or len(expected_ids or []) != len(set(expected_ids or []))
                or checked_ids != expected_ids
                or checked_count != len(expected_ids or [])
                or not isinstance(offender_dimensions, dict)
            )
            checked_source_unevaluable: set[str] = set()
            if not source_accounting_malformed:
                for cell_id in expected_ids:
                    offender = offender_dimensions.get(cell_id)
                    if offender is None:
                        continue
                    dimensions = (offender.get("dimensions")
                                  if isinstance(offender, dict) else None)
                    source_unevaluable = (
                        dimensions.get("source_unevaluable")
                        if isinstance(dimensions, dict) else None)
                    if not isinstance(source_unevaluable, bool):
                        source_accounting_malformed = True
                        break
                    if source_unevaluable:
                        checked_source_unevaluable.add(cell_id)
            if source_accounting_malformed:
                errors.append(
                    f"form audit source accounting is malformed: {slug}")
            elif (source_u_frame + source_unframed
                  != checked_count - len(checked_source_unevaluable)):
                errors.append(
                    f"form audit source frame/unframed partition is false: "
                    f"{slug}")
        if (not isinstance(emission_inventory, dict)
                or set(emission_inventory) != EMISSION_INVENTORY_KEYS
                or not isinstance(emission_inventory.get("complete"), bool)
                or not isinstance(emission_inventory.get("reason"), str)
                or not emission_inventory.get("reason")):
            errors.append(f"form emission inventory is malformed: {slug}")
            emission_inventory = {"complete": False, "reason": "invalid"}
        elif (emission_inventory["complete"] is True
              and emission_inventory["reason"] != "complete"):
            errors.append(f"form emission-complete reason is false: {slug}")
        if (not isinstance(form.get("emission_binding_errors"), list)
                or form.get("emission_binding_errors")):
            errors.append(f"form emission binding has errors: {slug}")
        manifest_binding = audit_evidence.get("manifest_binding")
        if (not isinstance(manifest_binding, dict)
                or set(manifest_binding) != MANIFEST_BINDING_KEYS
                or manifest_binding.get("binding_valid") is not True
                or manifest_binding.get("manifest_inputs_complete") is not True
                or manifest_binding.get("runtime_manifest_self_consistent")
                is not True
                or manifest_binding.get("errors") != []
                or not isinstance(manifest_binding.get("blockers"), list)
                or not all(isinstance(item, str) and item
                           for item in manifest_binding.get("blockers", []))):
            errors.append(f"form audit manifest binding is not clean: {slug}")
        ledger_binding = audit_evidence.get("ledger_binding")
        if (not isinstance(ledger_binding, dict)
                or set(ledger_binding) != LEDGER_BINDING_KEYS
                or ledger_binding.get("binding_valid") is not True
                or ledger_binding.get("reason") != "complete"
                or ledger_binding.get("errors") != []):
            errors.append(f"form audit ledger binding is not clean: {slug}")
        if (audit_evidence.get("assertion_valid") is not True
                or audit_evidence.get("errors") != []
                or audit_evidence.get("input_manifest_verified") is not True
                or audit_evidence.get("evidence_published") is not True
                or audit_evidence.get("byte_and_relation_binding_valid")
                is not True):
            errors.append(f"form audit relation contains errors: {slug}")
        form_poppler = form.get("poppler")
        if (not isinstance(form_poppler, dict)
                or set(form_poppler) != FORM_POPPLER_KEYS
                or not isinstance(form_poppler.get("version"), str)
                or not form_poppler.get("version")
                or not _is_sha256(form_poppler.get("binary_sha256"))
                or form_poppler.get("identity_timeout_seconds") != 10.0
                or form_poppler.get("page_timeout_seconds") != 60.0
                or form_poppler.get("subprocess_cleanup_policy")
                != "kill-isolated-process-group"):
            errors.append(f"form Poppler evidence is malformed: {slug}")
        pages = form.get("pages")
        source = form.get("source")
        expected_page_count = source.get("page_count") if isinstance(
            source, dict) else None
        if (not isinstance(pages, list)
                or len(pages) != expected_page_count):
            errors.append(f"form page evidence is incomplete: {slug}")
        else:
            for expected_page, page_record in enumerate(pages, 1):
                if (not isinstance(page_record, dict)
                        or set(page_record) != FORM_PAGE_KEYS
                        or page_record.get("page") != expected_page
                        or not _is_sha256(page_record.get("svg_sha256"))
                        or not _is_count(page_record.get("vector_paints"))
                        or not _is_count(page_record.get(
                            "unsupported_regions"))):
                    errors.append(
                        f"form page evidence is malformed: "
                        f"{slug}/p{expected_page}")
        if audit_evidence["complete"]:
            recomputed["audit_evidence_complete_forms"] += 1
        cell_comparisons = {name: 0 for name in COMPARISON_NAMES}
        state_counts = {state: 0 for state in LEDGER_STATES}
        measured_cells = source_unevaluable_cells = pending = 0
        blocking_cells = layout_mismatches = position_mismatches = 0
        emission_mismatches = 0
        form_cell_ids: set[str] = set()
        form_legacy_ids: set[str] = set()
        form_subject_keys: set[str] = set()
        for cell in cells:
            if not isinstance(cell, dict) or set(cell) != CELL_KEYS:
                errors.append(f"form has malformed cell evidence: {slug}")
                continue
            published_id = cell.get("cell")
            legacy_id = cell.get("legacy_cell_id")
            active_id = cell.get("cell_id")
            subject_key = cell.get("subject_key")
            if (not isinstance(published_id, str) or not published_id
                    or published_id in form_cell_ids):
                errors.append(f"form has a missing or duplicate cell ID: {slug}")
            else:
                form_cell_ids.add(published_id)
                qualified = f"{slug}:{published_id}"
                if qualified in corpus_cell_ids:
                    errors.append(f"corpus has a duplicate cell identity: {qualified}")
                corpus_cell_ids.add(qualified)
            if (not isinstance(legacy_id, str) or not legacy_id
                    or legacy_id in form_legacy_ids):
                errors.append(f"form has a missing or duplicate legacy cell ID: {slug}")
            else:
                form_legacy_ids.add(legacy_id)
            if (not isinstance(subject_key, str) or not subject_key
                    or subject_key in form_subject_keys):
                errors.append(f"form has a missing or duplicate subject key: {slug}")
            else:
                form_subject_keys.add(subject_key)
                qualified = f"{slug}:{subject_key}"
                if qualified in corpus_subject_keys:
                    errors.append(
                        f"corpus has a duplicate subject identity: {qualified}")
                corpus_subject_keys.add(qualified)

            ledger_state = cell.get("ledger_state")
            blocks_gate = cell.get("ledger_blocks_gate")
            if ledger_state not in LEDGER_STATES:
                errors.append(f"cell ledger state is invalid: {slug}/{published_id}")
            else:
                state_counts[ledger_state] += 1
                expected_block = ledger_state != "active_resolved"
                if blocks_gate is not expected_block:
                    errors.append(
                        f"cell ledger blocking relation is false: {slug}/{published_id}")
                if blocks_gate is True:
                    blocking_cells += 1
                if (ledger_state == "retained_unresolved"
                        and active_id is not None):
                    errors.append(
                        f"retained cell publishes an active ID: {slug}/{published_id}")
                if (ledger_state != "retained_unresolved"
                        and (not isinstance(active_id, str)
                             or active_id != published_id)):
                    errors.append(
                        f"active cell identity is invalid: {slug}/{published_id}")
                if (ledger_state == "retained_unresolved"
                        and published_id != legacy_id):
                    errors.append(
                        f"retained cell identity is invalid: {slug}/{published_id}")
                if not _string_list(
                        cell.get("ledger_reason_codes"),
                        nonempty=ledger_state != "active_resolved"):
                    errors.append(
                        f"cell ledger reasons are invalid: {slug}/{published_id}")
            if not isinstance(blocks_gate, bool):
                errors.append(f"cell ledger blocking flag is not boolean: {slug}")
            if not _is_sha256(cell.get("ledger_topology_sha256")):
                errors.append(f"cell topology digest is invalid: {slug}")
            page = cell.get("page")
            bbox = cell.get("bbox")
            if not _is_count(page) or page < 1:
                errors.append(f"cell page is invalid: {slug}/{published_id}")
            if (not _finite_number_list(bbox, length=4)
                    or not (bbox[0] < bbox[2] and bbox[1] < bbox[3])):
                errors.append(f"cell bbox is invalid: {slug}/{published_id}")
            latticed = cell.get("latticed")
            if not _is_count(latticed) or latticed < 1:
                errors.append(f"cell lattice count is invalid: {slug}/{published_id}")
            divider_x = cell.get("lattice_divider_x")
            expected_dividers = max(0, latticed - 1) if _is_count(latticed) else 0
            if (not _finite_number_list(divider_x, length=expected_dividers)
                    or any(left >= right for left, right in zip(
                        divider_x or [], (divider_x or [])[1:]))
                    or (_finite_number_list(bbox, length=4)
                        and any(not (bbox[0] < value < bbox[2])
                                for value in (divider_x or [])))):
                errors.append(
                    f"cell lattice divider geometry is invalid: {slug}/{published_id}")
            emitted = cell.get("emitted")
            if emitted is not None and not _is_count(emitted):
                errors.append(f"cell emitted count is invalid: {slug}/{published_id}")
            indexes_valid = cell.get("emitted_indexes_valid")
            if not isinstance(indexes_valid, bool):
                errors.append(
                    f"cell emitted-index flag is invalid: {slug}/{published_id}")
            if emitted != cell.get("latticed") or indexes_valid is not True:
                emission_mismatches += 1

            referee = cell.get("referee")
            try:
                expected_comparison = _comparison_for_cell(
                    cell, audit_evidence["complete"])
            except (TypeError, ValueError) as error:
                errors.append(
                    f"cell comparison is not derivable: {slug}/{published_id}: "
                    f"{error}")
                expected_comparison = (
                    "unevaluable", "comparison evidence is malformed")
            actual_comparison = (
                cell.get("comparison_status"), cell.get("comparison_reason"))
            if actual_comparison != expected_comparison:
                errors.append(
                    f"cell comparison relation is false: {slug}/{published_id}")
            comparison_status = expected_comparison[0]
            cell_comparisons[comparison_status] += 1
            if ledger_state in LEDGER_STATES:
                expected_transition = _transition_for_cell(
                    ledger_state, comparison_status)
                actual_transition = (
                    cell.get("transition_status"), cell.get("transition_reason"))
                if actual_transition != expected_transition:
                    errors.append(
                        f"cell transition relation is false: {slug}/{published_id}")
                if expected_transition[0] != "none":
                    pending += 1
            else:
                errors.append(f"cell transition is not evaluable: {slug}")

            if not isinstance(referee, dict) or referee.get("status") not in {
                    "measured", "unevaluable"}:
                errors.append(f"cell source result is malformed: {slug}")
            elif referee["status"] == "measured":
                measured_cells += 1
                source_page = next((
                    item for item in pages
                    if isinstance(item, dict)
                    and item.get("page") == cell.get("page")
                ), None)
                if (not isinstance(source_page, dict)
                        or not _is_count(source_page.get("vector_paints"))
                        or source_page["vector_paints"] < 1):
                    errors.append(
                        f"measured source page has no vector paint: "
                        f"{slug}/{published_id}")
                certificate_errors = _measured_referee_certificate_errors(
                    str(slug), cell, referee)
                errors.extend(certificate_errors)
                if not certificate_errors:
                    if referee["compartments"] != cell.get("latticed"):
                        layout_mismatches += 1
                    if referee["positions_match"] is not True:
                        position_mismatches += 1
            else:
                source_unevaluable_cells += 1
                if (not isinstance(referee.get("reason"), str)
                        or not referee["reason"]
                        or any(key in referee for key in (
                            "error", "errors", "blockers"))):
                    errors.append(
                        f"unevaluable source result hides errors: {slug}")

            four_way = cell.get("four_way")
            expected_four_way = {
                "referee": (
                    referee.get("compartments")
                    if isinstance(referee, dict)
                    and referee.get("status") == "measured" else None),
                "lattice": cell.get("latticed"),
                "audit": cell.get("audit_printed"),
                "emitted": emitted,
            }
            if four_way != expected_four_way:
                errors.append(f"cell four-way publication is false: {slug}")

        inference_blockers = 0
        form_inference_ids: set[str] = set()
        form_inference_keys: set[str] = set()
        for inference in inferences:
            if not isinstance(inference, dict) or set(inference) != INFERENCE_KEYS:
                errors.append(f"form has malformed inference evidence: {slug}")
                continue
            if (not _is_count(inference.get("page"))
                    or inference.get("page", 0) < 1
                    or not _finite_number_list(inference.get("bbox"), length=4)
                    or not (inference["bbox"][0] < inference["bbox"][2]
                            and inference["bbox"][1] < inference["bbox"][3])):
                errors.append(f"inference geometry is invalid: {slug}")
            inference_id = inference.get("cell_id")
            subject_key = inference.get("subject_key")
            if (not isinstance(inference_id, str) or not inference_id
                    or inference_id in form_inference_ids
                    or inference_id in form_cell_ids):
                errors.append(f"form has a duplicate inference cell ID: {slug}")
            else:
                form_inference_ids.add(inference_id)
                qualified = f"{slug}:{inference_id}"
                if qualified in corpus_cell_ids:
                    errors.append(
                        f"corpus has a duplicate inferred cell identity: {qualified}")
                corpus_cell_ids.add(qualified)
            if (not isinstance(subject_key, str) or not subject_key
                    or subject_key in form_inference_keys
                    or subject_key in form_subject_keys):
                errors.append(f"form has a duplicate inference subject key: {slug}")
            else:
                form_inference_keys.add(subject_key)
                qualified = f"{slug}:{subject_key}"
                if qualified in corpus_subject_keys:
                    errors.append(
                        f"corpus has a duplicate inferred subject identity: {qualified}")
                corpus_subject_keys.add(qualified)
            if (inference.get("state") != INFERENCE_STATE
                    or inference.get("blocks_gate") is not True):
                errors.append(f"inference state/blocking relation is false: {slug}")
            else:
                inference_blockers += 1
            if (not _string_list(inference.get("reason_codes"), nonempty=True)
                    or not _is_sha256(inference.get("topology_sha256"))):
                errors.append(f"inference provenance is malformed: {slug}")

        active_resolved = state_counts["active_resolved"]
        active_unresolved = state_counts["active_unresolved"]
        retained = state_counts["retained_unresolved"]
        derived_counts = {
            "combs": len(cells),
            "subjects": len(cells),
            "subjects_active": active_resolved + active_unresolved,
            "subjects_active_resolved": active_resolved,
            "subjects_active_unresolved": active_unresolved,
            "subjects_retained_unresolved": retained,
            "inferences_suppressed": len(inferences),
            "ledger_blocking": blocking_cells + inference_blockers,
            "measured": measured_cells,
            "source_unevaluable": source_unevaluable_cells,
            "unevaluable": cell_comparisons["unevaluable"],
            "referee_layout_mismatches": layout_mismatches,
            "referee_layout_position_mismatches": position_mismatches,
            "emission_layout_mismatches": emission_mismatches,
        }
        for key, actual in derived_counts.items():
            if counts.get(key) != actual:
                errors.append(f"form total disagrees with evidence: {slug}/{key}")
        if (len(cells) != active_resolved + active_unresolved + retained
                or measured_cells + source_unevaluable_cells != len(cells)
                or sum(cell_comparisons.values()) != len(cells)):
            errors.append(f"form subject partitions are inconsistent: {slug}")
        stats["pending_transitions"] += pending
        if cell_comparisons != form_comparisons:
            errors.append(f"cell comparison totals disagree: {slug}")
        derived_status, derived_reason = _form_status_relation(
            ledger_blocking=derived_counts["ledger_blocking"],
            emission_inventory=emission_inventory,
            audit_evidence=audit_evidence,
            comparisons=cell_comparisons,
        )
        if form_status != derived_status or form.get("reason") != derived_reason:
            errors.append(f"form status/reason relation is false: {slug}")
        recomputed[f"forms_{derived_status}"] += 1
        for key in (
                "combs", "measured", "source_unevaluable", "unevaluable",
                "ledger_blocking", "subjects_active", "subjects_active_resolved",
                "subjects_active_unresolved", "subjects_retained_unresolved",
                "inferences_suppressed", "referee_layout_mismatches",
                "referee_layout_position_mismatches",
                "emission_layout_mismatches"):
            recomputed[key] += derived_counts[key]
        for name in COMPARISON_NAMES:
            recomputed[f"comparison:{name}"] += cell_comparisons[name]

    total_pairs = {
        "forms_measured": len(forms),
        "forms_error": len(raw_errors),
        "combs_found": recomputed["combs"],
        "combs_measured": recomputed["measured"],
        "combs_source_unevaluable": recomputed["source_unevaluable"],
        "combs_unevaluable": recomputed["unevaluable"],
        "ledger_blocking": recomputed["ledger_blocking"],
        "subjects_active": recomputed["subjects_active"],
        "subjects_active_resolved": recomputed["subjects_active_resolved"],
        "subjects_active_unresolved": recomputed["subjects_active_unresolved"],
        "subjects_retained_unresolved": (
            recomputed["subjects_retained_unresolved"]),
        "inferences_suppressed": recomputed["inferences_suppressed"],
        "referee_layout_mismatches": (
            recomputed["referee_layout_mismatches"]),
        "referee_layout_position_mismatches": (
            recomputed["referee_layout_position_mismatches"]),
        "forms_ok": recomputed["forms_ok"],
        "forms_disagreement": recomputed["forms_disagreement"],
        "forms_unevaluable": recomputed["forms_unevaluable"],
        "audit_evidence_complete_forms": (
            recomputed["audit_evidence_complete_forms"]),
    }
    for key, actual in total_pairs.items():
        if totals.get(key) != actual:
            errors.append(f"report total disagrees with forms: {key}")
    if comparisons != {
            name: recomputed[f"comparison:{name}"]
            for name in COMPARISON_NAMES}:
        errors.append("report comparison totals disagree with forms")
    if (totals.get("forms_expected") != expected_forms
            or totals.get("combs_expected") != expected_subjects
            or totals.get("combs_found") != expected_subjects):
        errors.append("report corpus identity is incomplete")
    if (sum(comparisons.values()) != totals.get("combs_found")
            or totals.get("combs_measured", 0)
            + totals.get("combs_source_unevaluable", 0)
            != totals.get("combs_found")):
        errors.append("report subject partition is inconsistent")
    if totals.get("referee_attestation_complete") is not attestation.get(
            "complete"):
        errors.append("report attestation-complete total is false")
    if totals.get("referee_enforceable") is not attestation.get("enforceable"):
        errors.append("report enforceable total is false")

    coverage_ok = bool(
        len(forms) == expected_forms
        and len(slugs) == expected_forms
        and not raw_errors
        and recomputed["combs"] == expected_subjects
    )
    derived_status_reasons: list[str] = []
    if not coverage_ok or recomputed["forms_unevaluable"]:
        application_status = "unevaluable"
        derived_status_reasons.append(
            "corpus coverage or one or more forms are unevaluable")
    elif recomputed["forms_disagreement"]:
        application_status = "disagreement"
        derived_status_reasons.append(
            "one or more four-way form comparisons disagree")
    else:
        application_status = "ok"
    derived_report_status = application_status
    if attestation.get("complete") is not True:
        derived_report_status = "unevaluable"
        derived_status_reasons.append(
            "standalone referee runtime/application attestation is incomplete "
            "and non-enforceable")
    if status != derived_report_status or reasons != derived_status_reasons:
        errors.append("report status/reasons relation is false")
    stats.update({
        "referee_layout_mismatches": (
            recomputed["referee_layout_mismatches"]),
        "referee_layout_position_mismatches": (
            recomputed["referee_layout_position_mismatches"]),
        "emission_layout_mismatches": (
            recomputed["emission_layout_mismatches"]),
        "application_status": application_status,
    })
    return errors, stats


FORM_ARTIFACT_KEYS = {
    "ir_sha256", "layout_sha256", "html_sha256",
    "html_structure_sha256", "guide_sha256", "guide_html_sha256",
    "tracked_provenance_file", "tracked_provenance_sha256",
}
FORM_SOURCE_KEYS = {"file", "sha256", "bytes", "page_count", "layout_pin"}
HTML_GEOMETRY_EPSILON_PT = 0.0002
REFEREE_POSITION_TOLERANCE_PT = 0.25
REFEREE_ROUNDING_EPSILON_PT = 0.00001
REFEREE_MEASURED_REASONS = {
    "one source topology contains every recognised anchor",
    (
        "one richer source topology contains every other slab and "
        "occupies a strict majority of the comb band"
    ),
}


def _same_finite_numbers(left: Any, right: Any) -> bool:
    return (
        _finite_number_list(left)
        and _finite_number_list(right)
        and len(left) == len(right)
        and all(abs(float(a) - float(b)) <= 1e-9
                for a, b in zip(left, right))
    )


def _rounded_six(value: Any) -> float:
    return round(float(value), 6)


def _referee_topology_contains(
        superset: tuple[float, ...], subset: tuple[float, ...],
        ) -> bool:
    available = list(superset)
    for value in subset:
        choices = sorted(
            (abs(candidate - value), index)
            for index, candidate in enumerate(available)
            if abs(candidate - value) <= REFEREE_POSITION_TOLERANCE_PT
        )
        if not choices:
            return False
        _distance, index = choices[0]
        available.pop(index)
    return True


def _referee_topology_key(values: Sequence[float]) -> str:
    return ",".join(str(_rounded_six(value)) for value in values)


def _measured_referee_certificate_errors(
        slug: str, cell: dict[str, Any], referee: dict[str, Any],
        ) -> list[str]:
    """Independently derive the producer's measured-source acceptance proof."""
    cell_id = cell.get("cell")
    label = f"{slug}/{cell_id}"
    errors: list[str] = []
    if set(referee) != MEASURED_REFEREE_KEYS:
        return [f"measured source certificate schema is unsupported: {label}"]
    reason = referee.get("reason")
    if reason not in REFEREE_MEASURED_REASONS:
        errors.append(f"measured source reason is not derived: {label}")

    lattice = cell.get("lattice_divider_x")
    source = referee.get("source_divider_x")
    extras = referee.get("extra_divider_x")
    chosen = referee.get("chosen_topology")
    compartments = referee.get("compartments")
    if (not _finite_number_list(lattice)
            or not _finite_number_list(source)
            or not _finite_number_list(extras)
            or not _finite_number_list(chosen)
            or not _is_count(compartments)
            or compartments < 2):
        return [*errors, f"measured source topology is malformed: {label}"]
    lattice_values = [_rounded_six(value) for value in lattice]
    source_values = [_rounded_six(value) for value in source]
    extra_values = [_rounded_six(value) for value in extras]
    chosen_values = [_rounded_six(value) for value in chosen]
    if (any(float(value) != rounded for value, rounded in zip(
                source, source_values))
            or any(float(value) != rounded for value, rounded in zip(
                extras, extra_values))
            or any(float(value) != rounded for value, rounded in zip(
                chosen, chosen_values))):
        errors.append(f"measured source coordinates exceed fixed precision: {label}")
    if (source_values != sorted(set(source_values))
            or extra_values != sorted(set(extra_values))
            or chosen_values != source_values
            or compartments != len(source_values) + 1):
        errors.append(f"measured source topology relation is false: {label}")
    bbox = cell.get("bbox")
    if (_finite_number_list(bbox, length=4)
            and any(not (float(bbox[0]) < value < float(bbox[2]))
                    for value in source_values)):
        errors.append(f"measured source divider lies outside its owner: {label}")

    anchor_matches = referee.get("anchor_matches")
    anchor_sources: list[float] = []
    anchor_pairs: list[tuple[float, float]] = []
    anchor_relation_valid = True
    if (not isinstance(anchor_matches, list)
            or len(anchor_matches) != len(lattice_values)):
        errors.append(f"measured anchor inventory is incomplete: {label}")
        anchor_relation_valid = False
    else:
        for expected_layout, match in zip(lattice_values, anchor_matches):
            if (not isinstance(match, dict)
                    or set(match) != {"layout_x", "source_x", "delta_pt"}
                    or not all(_finite_number(match.get(key)) for key in (
                        "layout_x", "source_x", "delta_pt"))):
                errors.append(f"measured anchor evidence is malformed: {label}")
                anchor_relation_valid = False
                continue
            layout_x = _rounded_six(match["layout_x"])
            source_x = _rounded_six(match["source_x"])
            delta = _rounded_six(match["delta_pt"])
            expected_delta = _rounded_six(source_x - layout_x)
            if (float(match["layout_x"]) != layout_x
                    or float(match["source_x"]) != source_x
                    or float(match["delta_pt"]) != delta
                    or layout_x != expected_layout
                    or delta != expected_delta):
                errors.append(f"measured anchor relation is false: {label}")
                anchor_relation_valid = False
            anchor_sources.append(source_x)
            anchor_pairs.append((layout_x, source_x))
    derived_positions_match = bool(
        anchor_relation_valid
        and len(anchor_sources) == len(lattice_values)
        and all(abs(source_x - layout_x)
                <= REFEREE_POSITION_TOLERANCE_PT
                for layout_x, source_x in anchor_pairs)
    )
    if (referee.get("anchors_complete") is not True
            or referee.get("positions_match") is not derived_positions_match):
        errors.append(f"measured anchor verdict is false: {label}")
    if any(
            abs(extra - anchor) <= REFEREE_POSITION_TOLERANCE_PT
            for extra in extra_values for anchor in anchor_sources):
        errors.append(f"measured extra divider duplicates an anchor: {label}")
    derived_source = sorted(set(anchor_sources) | set(extra_values))
    if source_values != derived_source:
        errors.append(f"measured source divider inventory is false: {label}")

    components = referee.get("components")
    component_x: list[float] = []
    if not isinstance(components, list) or not components:
        errors.append(f"measured source components are missing: {label}")
    else:
        for component in components:
            if (not isinstance(component, dict)
                    or set(component) != {
                        "x", "x0", "x1", "tone", "elements", "clipped"}
                    or not all(_finite_number(component.get(key)) for key in (
                        "x", "x0", "x1", "tone"))
                    or not isinstance(component.get("elements"), list)
                    or not component["elements"]
                    or not all(isinstance(item, str) and item
                               for item in component["elements"])
                    or len(component["elements"])
                    != len(set(component["elements"]))
                    or component.get("clipped") is not False):
                errors.append(f"measured source component is malformed: {label}")
                continue
            x = _rounded_six(component["x"])
            x0 = _rounded_six(component["x0"])
            x1 = _rounded_six(component["x1"])
            tone = float(component["tone"])
            if (x0 >= x1 or x != _rounded_six((x0 + x1) / 2)
                    or not 0.0 <= tone <= 1.0):
                errors.append(f"measured source component relation is false: {label}")
            component_x.append(x)
        if component_x != sorted(component_x):
            errors.append(f"measured source components are not ordered: {label}")
        if (any(not any(abs(component - divider)
                        <= REFEREE_POSITION_TOLERANCE_PT
                        for divider in source_values)
                for component in component_x)
                or any(not any(abs(component - divider)
                               <= REFEREE_POSITION_TOLERANCE_PT
                               for component in component_x)
                       for divider in source_values)):
            errors.append(f"measured components do not bind the topology: {label}")

    proofs = referee.get("subject_gap_proofs")
    unproven = referee.get("unproven_subject_gaps")
    adjacent_anchors = set(zip(anchor_sources, anchor_sources[1:]))
    seen_proofs: set[tuple[float, float]] = set()
    if not isinstance(proofs, list):
        errors.append(f"measured subject-gap proofs are malformed: {label}")
    else:
        for proof in proofs:
            keys = {
                "left", "right", "gap_pt", "pitch_pt",
                "integral_residual_pt", "single_frame_elements",
                "unsupported_regions",
            }
            if (not isinstance(proof, dict) or set(proof) != keys
                    or not all(_finite_number(proof.get(key)) for key in (
                        "left", "right", "gap_pt", "pitch_pt",
                        "integral_residual_pt"))
                    or not isinstance(proof.get("single_frame_elements"), list)
                    or not proof["single_frame_elements"]
                    or not all(isinstance(item, str) and item
                               for item in proof["single_frame_elements"])
                    or len(proof["single_frame_elements"])
                    != len(set(proof["single_frame_elements"]))
                    or proof.get("unsupported_regions") != []):
                errors.append(f"measured subject-gap proof is malformed: {label}")
                continue
            left = _rounded_six(proof["left"])
            right = _rounded_six(proof["right"])
            pair = (left, right)
            if (pair not in adjacent_anchors or pair in seen_proofs
                    or _rounded_six(proof["gap_pt"])
                    != _rounded_six(right - left)
                    or float(proof["pitch_pt"]) <= 0
                    or float(proof["integral_residual_pt"]) < 0):
                errors.append(f"measured subject-gap proof is false: {label}")
            seen_proofs.add(pair)
    if unproven != []:
        errors.append(f"measured source has unproven subject gaps: {label}")

    vertical_names = (
        "y0", "y1", "contract_y0", "contract_y1", "open_y0", "open_y1",
        "contract_span_pt", "seed_span_pt", "measured_span_pt",
        "unmeasured_span_pt",
    )
    if not all(_finite_number(referee.get(name)) for name in vertical_names):
        return [*errors, f"measured source span evidence is malformed: {label}"]
    y0, y1, contract_y0, contract_y1, open_y0, open_y1 = (
        float(referee[name]) for name in vertical_names[:6])
    contract_span = float(referee["contract_span_pt"])
    seed_span = float(referee["seed_span_pt"])
    measured_span = float(referee["measured_span_pt"])
    unmeasured_span = float(referee["unmeasured_span_pt"])
    if (not contract_y0 <= open_y0 < open_y1 <= contract_y1
            or not open_y0 <= y0 < y1 <= open_y1
            or y1 - y0 <= REFEREE_POSITION_TOLERANCE_PT
            or _rounded_six(contract_span)
            != _rounded_six(contract_y1 - contract_y0)
            or _rounded_six(seed_span) != _rounded_six(open_y1 - open_y0)
            or measured_span <= seed_span / 2
            or measured_span > seed_span + REFEREE_ROUNDING_EPSILON_PT
            or _rounded_six(unmeasured_span)
            != _rounded_six(max(0.0, seed_span - measured_span))):
        errors.append(f"measured source span relation is false: {label}")

    coverage = referee.get("topology_coverage_pt")
    topology_by_key: dict[str, tuple[float, ...]] = {}
    if not isinstance(coverage, dict) or not coverage:
        errors.append(f"measured topology coverage is missing: {label}")
        coverage = {}
    else:
        for key, amount in coverage.items():
            try:
                values = tuple(float(item) for item in key.split(","))
            except (AttributeError, TypeError, ValueError):
                values = ()
            if (not isinstance(key, str) or not values
                    or not all(math.isfinite(value) for value in values)
                    or list(values) != sorted(set(values))
                    or key != _referee_topology_key(values)
                    or not _finite_number(amount) or float(amount) <= 0):
                errors.append(f"measured topology coverage is malformed: {label}")
                continue
            topology_by_key[key] = values
    chosen_key = _referee_topology_key(source_values)
    if chosen_key not in topology_by_key:
        errors.append(f"chosen source topology has no coverage: {label}")
    coverage_total = sum(
        float(amount) for amount in coverage.values()
        if _finite_number(amount))
    if abs(coverage_total - measured_span) > (
            REFEREE_ROUNDING_EPSILON_PT * max(1, len(coverage))):
        errors.append(f"measured topology coverage total is false: {label}")
    chosen_coverage = coverage.get(chosen_key)
    if (_finite_number(chosen_coverage)
            and float(chosen_coverage) + REFEREE_ROUNDING_EPSILON_PT
            < y1 - y0):
        errors.append(f"chosen band exceeds its topology coverage: {label}")

    topologies = sorted(topology_by_key.values())
    expected_relations = [
        {
            "candidate": list(candidate),
            "other": list(other),
            "contains": _referee_topology_contains(candidate, other),
            "proper": (
                len(candidate) > len(other)
                and _referee_topology_contains(candidate, other)
            ),
        }
        for candidate in topologies for other in topologies
        if candidate != other
    ]
    relations = referee.get("topology_superset_relations")
    if len(topologies) == 1:
        if (reason != "one source topology contains every recognised anchor"
                or relations != []
                or topologies[0] != tuple(source_values)
                or not _finite_number(chosen_coverage)
                or abs(float(chosen_coverage) - measured_span)
                > REFEREE_ROUNDING_EPSILON_PT):
            errors.append(f"single-topology acceptance relation is false: {label}")
    elif len(topologies) > 1:
        dominant = [
            candidate for candidate in topologies
            if all(
                other == candidate
                or (len(candidate) > len(other)
                    and _referee_topology_contains(candidate, other))
                for other in topologies
            )
            and _finite_number(coverage.get(_referee_topology_key(candidate)))
            and float(coverage[_referee_topology_key(candidate)]) > seed_span / 2
        ]
        if (reason != (
                    "one richer source topology contains every other slab and "
                    "occupies a strict majority of the comb band")
                or relations != expected_relations
                or dominant != [tuple(source_values)]):
            errors.append(f"multi-topology acceptance relation is false: {label}")

    ignored = referee.get("ignored_slabs")
    ignored_reasons = {
        "slab is no wider than the fixed position bound",
        "no candidate divider ink",
        "only cell-edge frames remain when an anchor is absent",
    }
    if not isinstance(ignored, list):
        errors.append(f"ignored source slabs are malformed: {label}")
    else:
        for slab in ignored:
            slab_keys = set(slab) if isinstance(slab, dict) else set()
            if (not isinstance(slab, dict)
                    or slab_keys not in (
                        {"y0", "y1", "reason"},
                        {"y0", "y1", "reason", "source_divider_x"})
                    or slab.get("reason") not in ignored_reasons
                    or not _finite_number(slab.get("y0"))
                    or not _finite_number(slab.get("y1"))
                    or float(slab["y0"]) >= float(slab["y1"])
                    or not (open_y0 <= float(slab["y0"])
                            < float(slab["y1"]) <= open_y1)):
                errors.append(f"ignored source slab is malformed: {label}")
                continue
            if "source_divider_x" in slab and not _finite_number_list(
                    slab["source_divider_x"]):
                errors.append(f"ignored source topology is malformed: {label}")
    return errors


def _project_layout_topology(
        comb: Any, bbox: list[float], label: str,
        ) -> dict[str, Any]:
    """Project exactly the topology digest published by comb_referee.py."""
    if not isinstance(comb, dict):
        raise CombRefereeScopeError(f"{label} has no comb topology")
    cells = comb.get("cells")
    divider_count = comb.get("divider_count")
    raw_dividers = comb.get("divider_x")
    raw_slots = comb.get("slot_x")
    if (not _is_count(cells) or cells < 1
            or divider_count != cells - 1
            or not _finite_number_list(raw_dividers, length=cells - 1)
            or not _finite_number_list(raw_slots, length=cells + 1)):
        raise CombRefereeScopeError(f"{label} has invalid comb counts/edges")
    dividers = [float(value) for value in raw_dividers]
    slots = [float(value) for value in raw_slots]
    if (any(right <= left for left, right in zip(slots, slots[1:]))
            or not _same_finite_numbers(slots[1:-1], dividers)
            or not _same_finite_numbers(
                [slots[0], slots[-1]], [bbox[0], bbox[2]])):
        raise CombRefereeScopeError(f"{label} comb edges are inconsistent")
    y0 = comb.get("y0")
    y1 = comb.get("y1")
    pitch = comb.get("pitch_pt")
    resolution = comb.get("resolution")
    if (not _finite_number(y0) or not _finite_number(y1)
            or float(y1) <= float(y0)
            or not _finite_number(pitch) or float(pitch) <= 0
            or not isinstance(resolution, dict)):
        raise CombRefereeScopeError(f"{label} comb band is invalid")
    resolution_status = resolution.get("status")
    reason_codes = resolution.get("reason_codes")
    if (resolution_status not in {"resolved", "unresolved"}
            or not _string_list(reason_codes)
            or bool(reason_codes) != (resolution_status == "unresolved")):
        raise CombRefereeScopeError(f"{label} comb resolution is invalid")
    topology = {
        "cells": cells,
        "divider_x": dividers,
        "slot_x": slots,
        "y0": float(y0),
        "y1": float(y1),
        "resolution_status": resolution_status,
        "reason_codes": reason_codes,
    }
    topology["sha256"] = canonical_digest(topology)
    return topology


def _emission_geometry_from_layout(
        page_index: int, cell: dict[str, Any], box: dict[str, float],
        ) -> dict[str, Any]:
    comb = cell["comb"]
    slot_x = [float(value) for value in comb["slot_x"]]
    left = float(box["x0"])
    top = float(box["y0"])
    right = float(box["x1"])
    bottom = float(box["y1"])
    band_top = float(comb["y0"])
    band_bottom = float(comb["y1"])
    return {
        "page_index": page_index,
        "left": left,
        "top": top,
        "width": right - left,
        "height": bottom - top,
        "slots": [
            {
                "index": index,
                "left": slot_left - left,
                "top": band_top - top,
                "width": slot_right - slot_left,
                "height": band_bottom - band_top,
            }
            for index, (slot_left, slot_right) in enumerate(
                zip(slot_x, slot_x[1:]))
        ],
    }


def _layout_binding_projection(
        slug: str, layout: Any, guide: Any,
        lattice_record: dict[str, Any], layout_sha256: str,
        guide_sha256: str,
        ) -> dict[str, Any]:
    """Bind every deterministic child ledger claim to parsed layout bytes."""
    if not isinstance(layout, dict) or not isinstance(layout.get("pages"), list):
        raise CombRefereeScopeError(f"layout projection is malformed: {slug}")
    if not isinstance(guide, dict):
        raise CombRefereeScopeError(f"guide projection is malformed: {slug}")
    relocated: set[str] = set()
    clipped: dict[str, dict[str, float]] = {}
    for region in guide.get("inline") or []:
        if not isinstance(region, dict):
            raise CombRefereeScopeError(f"guide inline region is malformed: {slug}")
        cell_ids = region.get("cell_ids") or []
        if (not isinstance(cell_ids, list)
                or not all(isinstance(item, str) for item in cell_ids)):
            raise CombRefereeScopeError(f"guide relocation list is malformed: {slug}")
        relocated.update(cell_ids)
        for straddler in region.get("straddlers") or []:
            if (not isinstance(straddler, dict)
                    or straddler.get("kind") != "cell"
                    or straddler.get("disposition") != "clipped"):
                continue
            cell_id = straddler.get("ref")
            form_box = straddler.get("form")
            if (not isinstance(cell_id, str) or cell_id in clipped
                    or not isinstance(form_box, dict)
                    or any(not _finite_number(form_box.get(name))
                           for name in ("x0", "y0", "x1", "y1"))):
                raise CombRefereeScopeError(
                    f"guide clipped-cell evidence is malformed: {slug}")
            clipped[cell_id] = {
                name: float(form_box[name])
                for name in ("x0", "y0", "x1", "y1")
            }

    projected_cells: dict[str, Any] = {}
    projected_inferences: dict[str, Any] = {}
    audit_expected_ids: list[str] = []
    for expected_page, page in enumerate(layout["pages"], 1):
        if (not isinstance(page, dict)
                or page.get("index") != expected_page
                or not isinstance(page.get("cells"), list)
                or not isinstance(page.get("comb_subjects"), list)
                or not isinstance(page.get("comb_inferences"), list)):
            raise CombRefereeScopeError(
                f"layout ledger page is incomplete: {slug}/p{expected_page}")
        cells_by_id: dict[str, dict[str, Any]] = {}
        for raw_cell in page["cells"]:
            if not isinstance(raw_cell, dict) or not isinstance(
                    raw_cell.get("id"), str):
                raise CombRefereeScopeError(
                    f"layout cell is malformed: {slug}/p{expected_page}")
            cell_id = raw_cell["id"]
            if cell_id in cells_by_id:
                raise CombRefereeScopeError(
                    f"layout cell is duplicated: {slug}/{cell_id}")
            cells_by_id[cell_id] = raw_cell
            if isinstance(raw_cell.get("comb"), dict) and cell_id not in relocated:
                audit_expected_ids.append(cell_id)

        for subject in page["comb_subjects"]:
            if not isinstance(subject, dict):
                raise CombRefereeScopeError(
                    f"layout subject is malformed: {slug}/p{expected_page}")
            state = subject.get("state")
            subject_key = subject.get("subject_key")
            legacy_id = subject.get("legacy_cell_id")
            active_id = subject.get("cell_id")
            bbox_raw = subject.get("legacy_bbox")
            reason_codes = subject.get("reason_codes")
            blocks_gate = subject.get("blocks_gate")
            if (state not in LEDGER_STATES
                    or not isinstance(subject_key, str) or not subject_key
                    or not isinstance(legacy_id, str) or not legacy_id
                    or not _finite_number_list(bbox_raw, length=4)
                    or not _string_list(
                        reason_codes, nonempty=state != "active_resolved")
                    or blocks_gate is not (state != "active_resolved")):
                raise CombRefereeScopeError(
                    f"layout subject relation is malformed: {slug}/{legacy_id}")
            bbox = [float(value) for value in bbox_raw]
            if bbox[2] <= bbox[0] or bbox[3] <= bbox[1]:
                raise CombRefereeScopeError(
                    f"layout subject bbox is invalid: {slug}/{legacy_id}")
            if state == "retained_unresolved":
                if active_id is not None:
                    raise CombRefereeScopeError(
                        f"retained subject has active id: {slug}/{legacy_id}")
                report_id = legacy_id
                comb = subject.get("legacy_comb")
                emission_geometry = None
            else:
                if not isinstance(active_id, str) or active_id not in cells_by_id:
                    raise CombRefereeScopeError(
                        f"active subject has no layout owner: {slug}/{legacy_id}")
                owner = cells_by_id[active_id]
                owner_bbox = [owner.get(name) for name in ("x0", "y0", "x1", "y1")]
                if (owner.get("subject_key") != subject_key
                        or not _same_finite_numbers(owner_bbox, bbox)):
                    raise CombRefereeScopeError(
                        f"active subject owner relation is false: {slug}/{active_id}")
                report_id = active_id
                comb = owner.get("comb")
                form_box = clipped.get(active_id, {
                    name: float(owner[name])
                    for name in ("x0", "y0", "x1", "y1")
                })
                emission_geometry = (
                    None if active_id in relocated else
                    _emission_geometry_from_layout(expected_page, owner, form_box)
                )
            topology = _project_layout_topology(
                comb, bbox, f"{slug}/{report_id}")
            if report_id in projected_cells:
                raise CombRefereeScopeError(
                    f"layout report subject is duplicated: {slug}/{report_id}")
            projected_cells[report_id] = {
                "cell": report_id,
                "subject_key": subject_key,
                "legacy_cell_id": legacy_id,
                "cell_id": active_id,
                "ledger_state": state,
                "ledger_blocks_gate": blocks_gate,
                "ledger_reason_codes": reason_codes,
                "ledger_topology_sha256": topology["sha256"],
                "ledger_evidence": subject,
                "page": expected_page,
                "bbox": bbox,
                "latticed": topology["cells"],
                "lattice_divider_x": topology["divider_x"],
                "expected_emission_geometry": emission_geometry,
            }

        for inference in page["comb_inferences"]:
            if not isinstance(inference, dict):
                raise CombRefereeScopeError(
                    f"layout inference is malformed: {slug}/p{expected_page}")
            inference_id = inference.get("cell_id")
            subject_key = inference.get("subject_key")
            bbox_raw = inference.get("bbox")
            if (not isinstance(inference_id, str) or not inference_id
                    or inference_id in projected_inferences
                    or not isinstance(subject_key, str) or not subject_key
                    or inference.get("state") != INFERENCE_STATE
                    or inference.get("blocks_gate") is not True
                    or not _string_list(
                        inference.get("reason_codes"), nonempty=True)
                    or not _finite_number_list(bbox_raw, length=4)):
                raise CombRefereeScopeError(
                    f"layout inference relation is malformed: {slug}/{inference_id}")
            bbox = [float(value) for value in bbox_raw]
            topology = _project_layout_topology(
                inference.get("inferred_comb"), bbox,
                f"{slug}/{inference_id} inference")
            projected_inferences[inference_id] = {
                "page": expected_page,
                "subject_key": subject_key,
                "cell_id": inference_id,
                "state": INFERENCE_STATE,
                "blocks_gate": True,
                "reason_codes": inference["reason_codes"],
                "bbox": bbox,
                "topology_sha256": topology["sha256"],
                "ledger_evidence": inference,
            }

    generator = layout.get("generator")
    if not isinstance(generator, dict):
        raise CombRefereeScopeError(f"layout generator is missing: {slug}")
    lattice_evidence = {
        "file": "tools/formgen/lattice.py",
        "bytes": lattice_record.get("bytes"),
        "sha256": lattice_record.get("sha256"),
        "expected_sha256": lattice_record.get("sha256"),
        "layout_generator": generator,
    }
    result = {
        "layout_sha256": layout_sha256,
        "guide_sha256": guide_sha256,
        "lattice_evidence": lattice_evidence,
        "audit_expected_ids": audit_expected_ids,
        "cells": projected_cells,
        "inferences": projected_inferences,
    }
    _layout_audit_owner_ids(result)
    return result


def _layout_binding_snapshots(
        layout_tree: dict[str, Any], guide_tree: dict[str, Any],
        lattice_record: dict[str, Any],
        ) -> dict[str, Any]:
    layout_files = _manifest_files(layout_tree)
    guide_files = _manifest_files(guide_tree)
    result: dict[str, Any] = {}
    for logical, layout_record in sorted(layout_files.items()):
        if not logical.endswith(".layout.json"):
            continue
        slug = pathlib.PurePosixPath(logical).name.removesuffix(".layout.json")
        guide_logical = f"build/guides/{slug}.guide.json"
        guide_record = guide_files.get(guide_logical)
        if guide_record is None:
            raise CombRefereeScopeError(f"guide is missing for layout: {slug}")
        layout_path = BUILD / "layout" / f"{slug}.layout.json"
        guide_path = BUILD / "guides" / f"{slug}.guide.json"
        if (_stable_file_record(layout_path, logical) != layout_record
                or _stable_file_record(guide_path, guide_logical) != guide_record):
            raise CombRefereeScopeError(
                f"layout/guide changed while projecting ledger: {slug}")
        try:
            layout = json.loads(layout_path.read_text(encoding="utf-8"))
            guide = json.loads(guide_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            raise CombRefereeScopeError(
                f"cannot parse layout/guide projection for {slug}: {error}") from error
        result[slug] = _layout_binding_projection(
            slug, layout, guide, lattice_record,
            layout_record["sha256"], guide_record["sha256"])
    if len(result) != EXPECTED_FORMS:
        raise CombRefereeScopeError(
            f"layout binding corpus has {len(result)}/{EXPECTED_FORMS} forms")
    return result


def _manifest_files(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    files = manifest.get("files")
    if not isinstance(files, list):
        return {}
    return {
        item["path"]: item for item in files
        if isinstance(item, dict) and isinstance(item.get("path"), str)
    }


EMITTED_EVIDENCE_KEYS = {
    "count", "indexes", "editable_indexes", "declared_capacity",
    "declared_count", "page_index", "container_position",
    "container_geometry", "layout_binding_valid", "expected_geometry",
    "slot_geometry", "valid",
}


def _emitted_evidence_binding_errors(
        slug: str, cell: dict[str, Any], expected: dict[str, Any],
        ) -> list[str]:
    errors: list[str] = []
    cell_id = cell.get("cell")
    evidence = cell.get("emitted_evidence")
    expected_geometry = expected.get("expected_emission_geometry")
    if expected_geometry is None:
        if (evidence is not None or cell.get("emitted") is not None
                or cell.get("emitted_indexes_valid") is not False):
            errors.append(
                f"suppressed cell has fabricated emission: {slug}/{cell_id}")
        return errors
    if not isinstance(evidence, dict) or set(evidence) != EMITTED_EVIDENCE_KEYS:
        return [f"cell emitted evidence schema is unsupported: {slug}/{cell_id}"]
    count = evidence.get("count")
    indexes = evidence.get("indexes")
    editable = evidence.get("editable_indexes")
    if (not _is_count(count)
            or not isinstance(indexes, list)
            or not all(_is_count(index) for index in indexes)
            or not isinstance(editable, list)
            or not all(_is_count(index) for index in editable)
            or len(indexes) != len(set(indexes))
            or len(editable) != len(set(editable))):
        return [f"cell emitted index evidence is malformed: {slug}/{cell_id}"]
    if (cell.get("emitted") != count
            or cell.get("emitted_indexes_valid") is not evidence.get("valid")):
        errors.append(f"cell emitted summary is false: {slug}/{cell_id}")
    if (evidence.get("expected_geometry") != expected_geometry
            or evidence.get("page_index") != expected_geometry["page_index"]):
        errors.append(f"cell expected emission geometry is unbound: {slug}/{cell_id}")
    position = evidence.get("container_position")
    geometry = evidence.get("container_geometry")
    slot_geometry = evidence.get("slot_geometry")
    expected_slots = expected_geometry["slots"]
    actual_container = (
        [*position, *geometry]
        if _finite_number_list(position, length=2)
        and _finite_number_list(geometry, length=2) else None)
    expected_container = [
        expected_geometry["left"], expected_geometry["top"],
        expected_geometry["width"], expected_geometry["height"],
    ]
    container_matches = bool(
        actual_container is not None
        and all(abs(float(actual) - float(target))
                <= HTML_GEOMETRY_EPSILON_PT
                for actual, target in zip(actual_container, expected_container)))
    slots_match = bool(
        isinstance(slot_geometry, list)
        and len(slot_geometry) == len(expected_slots)
        and all(
            isinstance(actual, dict)
            and set(actual) == {"index", "left", "top", "width", "height"}
            and actual.get("index") == target["index"]
            and all(_finite_number(actual.get(name))
                    and abs(float(actual[name]) - float(target[name]))
                    <= HTML_GEOMETRY_EPSILON_PT
                    for name in ("left", "top", "width", "height"))
            for actual, target in zip(slot_geometry, expected_slots)
        ))
    expected_layout_binding = container_matches and slots_match
    if evidence.get("layout_binding_valid") is not expected_layout_binding:
        errors.append(f"cell layout-binding verdict is false: {slug}/{cell_id}")
    if evidence.get("valid") is True:
        expected_count = expected.get("latticed")
        if (count != expected_count
                or indexes != list(range(count))
                or evidence.get("declared_capacity") != count
                or evidence.get("declared_count") != count
                or not all(index in set(indexes) for index in editable)
                or not expected_layout_binding):
            errors.append(f"cell valid-emission claim is false: {slug}/{cell_id}")
    elif evidence.get("valid") is not False:
        errors.append(f"cell emitted validity is not boolean: {slug}/{cell_id}")
    return errors


def form_binding_errors(form: dict[str, Any],
                        snapshot: dict[str, Any]) -> list[str]:
    """Bind every per-form claim to bytes in the outer immutable snapshot."""
    errors: list[str] = []
    slug = form.get("slug")
    if not isinstance(slug, str):
        return ["form binding has no slug"]
    artifacts = form.get("artifacts")
    source = form.get("source")
    if not isinstance(artifacts, dict) or set(artifacts) != FORM_ARTIFACT_KEYS:
        return [f"form artifact schema is incomplete: {slug}"]
    if not isinstance(source, dict) or set(source) != FORM_SOURCE_KEYS:
        return [f"form source schema is incomplete: {slug}"]

    trees = snapshot.get("artifact_trees", {})
    expected_artifacts = {
        "ir_sha256": ("ir", f"build/ir/{slug}.ir.json"),
        "layout_sha256": ("layout", f"build/layout/{slug}.layout.json"),
        "html_sha256": ("html", f"build/html/{slug}.html"),
        "guide_sha256": ("guides", f"build/guides/{slug}.guide.json"),
    }
    for field, (tree_name, logical) in expected_artifacts.items():
        tree = trees.get(tree_name, {}) if isinstance(trees, dict) else {}
        record = _manifest_files(tree).get(logical)
        if record is None or artifacts.get(field) != record.get("sha256"):
            errors.append(f"form artifact is not bound: {slug}/{field}")
    layout_bindings = snapshot.get("layout_bindings")
    layout_binding = layout_bindings.get(slug) if isinstance(
        layout_bindings, dict) else None
    layout_owner_ids: list[str] | None = None
    if (not isinstance(layout_binding, dict)
            or layout_binding.get("layout_sha256")
            != artifacts.get("layout_sha256")
            or layout_binding.get("guide_sha256")
            != artifacts.get("guide_sha256")):
        errors.append(f"form parsed layout binding is missing: {slug}")
        layout_binding = None
    elif form.get("lattice_evidence") != layout_binding.get(
            "lattice_evidence"):
        errors.append(f"form lattice producer/layout binding is false: {slug}")
    else:
        try:
            layout_owner_ids = _layout_audit_owner_ids(layout_binding)
        except CombRefereeScopeError as error:
            errors.append(f"form parsed owner registry is invalid: {slug}: {error}")
    if artifacts.get("html_structure_sha256") != artifacts.get("html_sha256"):
        errors.append(f"form HTML structure digest is not byte-exact: {slug}")
    guide_logical = f"build/html/{slug}.guide.html"
    guide_record = _manifest_files(
        trees.get("html", {}) if isinstance(trees, dict) else {}
    ).get(guide_logical)
    expected_guide_sha = guide_record.get("sha256") if guide_record else None
    if artifacts.get("guide_html_sha256") != expected_guide_sha:
        errors.append(f"form optional guide HTML is not bound: {slug}")

    if layout_binding is not None:
        expected_cells = layout_binding.get("cells")
        expected_inferences = layout_binding.get("inferences")
        report_cells = form.get("cells")
        report_inferences = form.get("inferences")
        if (not isinstance(expected_cells, dict)
                or not isinstance(report_cells, list)):
            errors.append(f"form layout/report cell inventory is malformed: {slug}")
        else:
            report_ids = [
                cell.get("cell") if isinstance(cell, dict) else None
                for cell in report_cells
            ]
            report_by_id = {
                cell.get("cell"): cell for cell in report_cells
                if isinstance(cell, dict) and isinstance(cell.get("cell"), str)
            }
            if (len(report_by_id) != len(report_cells)
                    or set(report_by_id) != set(expected_cells)
                    or report_ids != list(expected_cells)):
                errors.append(
                    f"form report/layout subject inventory differs: {slug}")
            if layout_owner_ids is not None:
                report_owner_ids = [
                    cell_id for cell_id in report_ids
                    if isinstance(cell_id, str)
                    and isinstance(expected_cells.get(cell_id), dict)
                    and expected_cells[cell_id].get(
                        "expected_emission_geometry") is not None
                ]
                if report_owner_ids != layout_owner_ids:
                    errors.append(
                        f"form emitted owner order differs from layout: {slug}")
            deterministic_fields = (
                "cell", "subject_key", "legacy_cell_id", "cell_id",
                "ledger_state", "ledger_blocks_gate", "ledger_reason_codes",
                "ledger_topology_sha256", "ledger_evidence", "page", "bbox",
                "latticed", "lattice_divider_x",
            )
            for cell_id, expected_cell in expected_cells.items():
                actual = report_by_id.get(cell_id)
                if not isinstance(actual, dict):
                    continue
                for field in deterministic_fields:
                    if actual.get(field) != expected_cell.get(field):
                        errors.append(
                            f"cell layout/ledger binding is false: "
                            f"{slug}/{cell_id}/{field}")
                errors.extend(_emitted_evidence_binding_errors(
                    slug, actual, expected_cell))
        if (not isinstance(expected_inferences, dict)
                or not isinstance(report_inferences, list)):
            errors.append(f"form inference inventory is malformed: {slug}")
        else:
            inference_by_id = {
                inference.get("cell_id"): inference
                for inference in report_inferences
                if isinstance(inference, dict)
                and isinstance(inference.get("cell_id"), str)
            }
            if (len(inference_by_id) != len(report_inferences)
                    or set(inference_by_id) != set(expected_inferences)):
                errors.append(
                    f"form report/layout inference inventory differs: {slug}")
            for cell_id, expected_inference in expected_inferences.items():
                actual = inference_by_id.get(cell_id)
                if not isinstance(actual, dict):
                    continue
                for field, expected_value in expected_inference.items():
                    if actual.get(field) != expected_value:
                        errors.append(
                            f"inference layout/ledger binding is false: "
                            f"{slug}/{cell_id}/{field}")
                if actual.get("emitted_evidence") is not None:
                    errors.append(
                        f"suppressed inference has emitted evidence: "
                        f"{slug}/{cell_id}")
        inventory = form.get("emission_inventory")
        if isinstance(inventory, dict) and inventory.get("complete") is True:
            active_ids = sorted(
                cell_id for cell_id, expected_cell in expected_cells.items()
                if expected_cell.get("ledger_state") != "retained_unresolved")
            exact_inventory = {
                "complete": True,
                "reason": "complete",
                "expected_active_cell_ids": active_ids,
                "emitted_cell_ids": active_ids,
                "missing_active_cell_ids": [],
                "unexpected_emitted_cell_ids": [],
                "retained_emitted_cell_ids": [],
                "inference_emitted_cell_ids": [],
                "invalid_active_cell_ids": [],
            }
            if inventory != exact_inventory:
                errors.append(
                    f"form complete emission inventory is not derived: {slug}")

    provenance_records = _manifest_files(snapshot.get("provenance", {}))
    provenance_file = artifacts.get("tracked_provenance_file")
    provenance_record = provenance_records.get(provenance_file)
    if (not isinstance(provenance_file, str)
            or provenance_record is None
            or artifacts.get("tracked_provenance_sha256")
            != provenance_record.get("sha256")
            or provenance_record.get("equals_head") is not True):
        errors.append(f"form tracked provenance is not bound: {slug}")

    source_manifest = snapshot.get("source_pdfs", {})
    source_relations = source_manifest.get("relations", []) if isinstance(
        source_manifest, dict) else []
    if (not isinstance(source_relations, list)
            or source_manifest.get("relation_count") != len(source_relations)
            or source_manifest.get("candidate_file_count")
            != sum(
                relation.get("candidate_count", -1)
                for relation in source_relations
                if isinstance(relation, dict))
            or source_manifest.get("sha256")
            != canonical_digest(source_relations)):
        errors.append("source PDF manifest relation is invalid")
    source_by_slug: dict[str, Any] = {}
    if isinstance(source_relations, list):
        for relation in source_relations:
            relation_slug = relation.get("slug") if isinstance(
                relation, dict) else None
            if (not isinstance(relation_slug, str)
                    or relation_slug in source_by_slug):
                errors.append("source PDF relation inventory is duplicated")
                continue
            source_by_slug[relation_slug] = relation
    source_relation = source_by_slug.get(slug)
    if (not isinstance(source_relation, dict)
            or source.get("file") != source_relation.get("selected")
            or source.get("sha256") != source_relation.get("declared_sha256")
            or source.get("bytes") != source_relation.get("declared_bytes")
            or source.get("layout_pin") != source_relation.get("layout_pin")
            or source.get("page_count")
            != (source_relation.get("layout_pin") or {}).get("page_count")):
        errors.append(f"form source PDF pin/bytes are not bound: {slug}")
    else:
        candidates = source_relation.get("candidates")
        authoritative = [
            candidate for candidate in candidates
            if isinstance(candidate, dict)
            and candidate.get("sha256") == source_relation.get("declared_sha256")
            and candidate.get("bytes") == source_relation.get("declared_bytes")
        ] if isinstance(candidates, list) else []
        if (not isinstance(candidates, list)
                or source_relation.get("candidate_count") != len(candidates)
                or source_relation.get("matching_count") != 1
                or len(authoritative) != 1
                or authoritative[0].get("path")
                != source_relation.get("selected")
                or authoritative[0].get("sha256") != source.get("sha256")
                or authoritative[0].get("bytes") != source.get("bytes")):
            errors.append(f"form selected source PDF bytes are not bound: {slug}")

    audit_forms = snapshot.get("audit", {}).get("forms", {})
    audit_relation = audit_forms.get(slug) if isinstance(audit_forms, dict) else None
    if not isinstance(audit_relation, dict):
        errors.append(f"form has no outer audit relation: {slug}")
        return errors
    inputs = audit_relation.get("inputs")
    if not isinstance(inputs, dict):
        errors.append(f"form audit input manifest is malformed: {slug}")
        return errors
    input_expectations = {
        "ir": (f"{slug}.ir.json", True, artifacts.get("ir_sha256"),
               _manifest_files(trees.get("ir", {})).get(
                   f"build/ir/{slug}.ir.json")),
        "layout": (f"{slug}.layout.json", True, artifacts.get("layout_sha256"),
                   _manifest_files(trees.get("layout", {})).get(
                       f"build/layout/{slug}.layout.json")),
        "html": (f"{slug}.html", True, artifacts.get("html_sha256"),
                 _manifest_files(trees.get("html", {})).get(
                     f"build/html/{slug}.html")),
        "guide": (f"{slug}.guide.json", True, artifacts.get("guide_sha256"),
                  _manifest_files(trees.get("guides", {})).get(
                      f"build/guides/{slug}.guide.json")),
        "guide_html": (f"{slug}.guide.html", False, expected_guide_sha,
                       guide_record),
    }
    for role, (filename, required, digest, record) in input_expectations.items():
        present = record is not None
        expected_entry = {
            "file": filename,
            "required": required,
            "present": present,
            "bytes": record.get("bytes") if present else None,
            "sha256": digest if present else None,
        }
        if inputs.get(role) != expected_entry:
            errors.append(f"form audit input is not byte-bound: {slug}/{role}")
    if isinstance(source_relation, dict):
        expected_source_input = {
            "file": source_relation.get("declared_file"),
            "logical_identity": (
                source_relation.get("layout_pin") or {}).get("file"),
            "path": source_relation.get("selected"),
            "required": True,
            "present": True,
            "bytes": source.get("bytes"),
            "sha256": source.get("sha256"),
            "expected_sha256": source.get("sha256"),
        }
        if inputs.get("source_pdf") != expected_source_input:
            errors.append(f"form audit source input is not byte-bound: {slug}")

    audit_evidence = form.get("audit_evidence")
    if not isinstance(audit_evidence, dict):
        errors.append(f"form audit evidence is missing: {slug}")
        return errors
    assertion_relation = audit_relation.get("assertion_relation")
    if not isinstance(assertion_relation, dict):
        errors.append(f"outer audit assertion relation is missing: {slug}")
    else:
        if (layout_binding is not None
                and assertion_relation.get("expected_comb_ids")
                != layout_binding.get("audit_expected_ids")):
            errors.append(
                f"outer audit/layout comb inventory is not bound: {slug}")
        for key, expected in assertion_relation.items():
            if audit_evidence.get(key) != expected:
                errors.append(f"form audit assertion is not bound: {slug}/{key}")
        offender_dimensions = assertion_relation.get("offender_dimensions")
        cells = form.get("cells")
        if isinstance(offender_dimensions, dict) and isinstance(cells, list):
            expected_ids = set(assertion_relation.get("expected_comb_ids", []))
            unexpected_ids = set(assertion_relation.get(
                "unexpected_emitted_comb_ids", []))
            for offender_id, offender in offender_dimensions.items():
                kinds = set(offender.get("failure_kinds", [])) if isinstance(
                    offender, dict) else set()
                relation_name = offender.get("layout_relation") if isinstance(
                    offender, dict) else None
                owned = (
                    offender_id in expected_ids
                    or (offender_id in unexpected_ids
                        and "unexpected-emitted-comb" in kinds)
                    or ("emitted-cell-binding-invalid" in kinds
                        and relation_name == "cell-binding-invalid")
                    or ("unowned-live-comb-markup" in kinds
                        and relation_name == "not-owned")
                    or (offender_id == "<comb-inventory>"
                        and "comb-inventory-mismatch" in kinds)
                    or (offender_id == "<comb-owner-registry>"
                        and relation_name == "registry-invalid"
                        and "comb-owner-registry-invalid" in kinds)
                )
                if not owned:
                    errors.append(
                        f"outer audit offender is orphaned: "
                        f"{slug}/{offender_id}")
            for cell in cells:
                if not isinstance(cell, dict):
                    continue
                cell_id = cell.get("cell")
                offender = offender_dimensions.get(cell_id)
                ledger_state = cell.get("ledger_state")
                if offender is not None:
                    expected_audit = (
                        offender.get("printed"), "published-offender")
                elif (audit_evidence.get("complete") is True
                      and ledger_state in {
                          "active_resolved", "active_unresolved"}):
                    expected_audit = (
                        cell.get("latticed"), "complete-non-offender")
                elif audit_evidence.get("complete") is True:
                    expected_audit = (None, "complete-blocked-subject")
                else:
                    expected_audit = (None, "unknown-truncated")
                actual_audit = (
                    cell.get("audit_printed"), cell.get("audit_relation"))
                if actual_audit != expected_audit:
                    errors.append(
                        f"cell audit relation is not bound: {slug}/{cell_id}")
        if audit_evidence.get("complete") is True and (
                assertion_relation.get("holds") is not True
                or assertion_relation.get("inventory_complete") is not True
                or assertion_relation.get("offender_count") != 0
                or assertion_relation.get("offender_dimensions") != {}
                or assertion_relation.get("expected_comb_ids")
                != assertion_relation.get("emitted_comb_ids")
                or assertion_relation.get("owner_certificates_invalid") != 0
                or assertion_relation.get("owner_certificates_valid")
                != assertion_relation.get("combs_checked")
                or any(assertion_relation.get(key) != 0 for key in (
                    "raw_live_comb_issues", "emitted_cell_binding_issues",
                    "layout_mismatches", "layout_unevaluable",
                    "emission_behind_layout", "emission_invalid"))):
            errors.append(
                f"form audit-complete claim hides audit failures: {slug}")
    if audit_relation.get("top_level_holds") is not audit_evidence.get("holds"):
        errors.append(f"form audit top-level verdict is not bound: {slug}")
    manifest_binding = audit_evidence.get("manifest_binding")
    ledger_binding = audit_evidence.get("ledger_binding")
    expected_truths = {
        "assertion_valid": True,
        "input_manifest_verified": True,
        "evidence_published": True,
        "byte_and_relation_binding_valid": True,
    }
    for key, expected in expected_truths.items():
        if audit_evidence.get(key) is not expected:
            errors.append(f"form audit relation is false: {slug}/{key}")
    if (not isinstance(manifest_binding, dict)
            or manifest_binding.get("binding_valid") is not True
            or manifest_binding.get("manifest_inputs_complete") is not True
            or manifest_binding.get("attestation_complete") is not False
            or manifest_binding.get("enforceable") is not False
            or manifest_binding.get("complete") is not False
            or manifest_binding.get(
                "base_runtime_closure_independently_attested") is not False
            or manifest_binding.get(
                "roundtrip_runtime_closure_independently_attested") is not False
            or manifest_binding.get("producer_sha256")
            != snapshot["producers"]["tools/formgen/audit.py"]["sha256"]):
        errors.append(f"form audit manifest binding is invalid: {slug}")
    if (not isinstance(ledger_binding, dict)
            or ledger_binding.get("binding_valid") is not True
            or layout_binding is None
            or ledger_binding.get("active_subject_ids") != [
                cell_id for cell_id, expected in layout_binding["cells"].items()
                if expected.get("ledger_state") != "retained_unresolved"]
            or ledger_binding.get("emitted_ids") != sorted(
                cell_id for cell_id, expected in layout_binding["cells"].items()
                if expected.get("expected_emission_geometry") is not None)
            or ledger_binding.get("legacy_alias_count")
            != len(layout_binding["cells"])):
        errors.append(f"form audit ledger binding is invalid: {slug}")
    if (audit_evidence.get("input_manifest_reason")
            != (manifest_binding or {}).get("reason")):
        errors.append(f"form audit manifest reason is not bound: {slug}")
    if (audit_evidence.get("runtime_closure_independently_attested") is not False
            or audit_evidence.get("integrity_valid") is not False
            or audit_evidence.get("complete") is not False):
        errors.append(
            f"raw form audit overclaims standalone runtime closure: {slug}")
    form_poppler = form.get("poppler")
    snapshot_poppler = snapshot.get("runtime", {}).get("pdftocairo", {})
    if (not isinstance(form_poppler, dict)
            or form_poppler.get("binary_path") != snapshot_poppler.get("path")
            or form_poppler.get("binary_sha256")
            != snapshot_poppler.get("sha256")):
        errors.append(f"form Poppler executable is not bound: {slug}")
    render = audit_relation.get("render")
    render_dependencies = (
        render.get("dependencies") if isinstance(render, dict) else None)
    if (not isinstance(manifest_binding, dict)
            or manifest_binding.get("render_dependencies") != render_dependencies
            or manifest_binding.get("render_dependency_count")
            != (len(render_dependencies)
                if isinstance(render_dependencies, list) else -1)):
        errors.append(f"form audit render closure is not bound: {slug}")
    if isinstance(render_dependencies, list):
        html_files = _manifest_files(trees.get("html", {}))
        for dependency in render_dependencies:
            logical = dependency.get("path") if isinstance(dependency, dict) else None
            record = html_files.get(
                f"build/html/{logical}" if isinstance(logical, str) else "")
            if (record is None
                    or dependency.get("sha256") != record.get("sha256")
                    or dependency.get("bytes") != record.get("bytes")):
                errors.append(
                    f"form audit render dependency is not bound: {slug}/{logical}")
    return errors


def derive_application_scope_elevation(
        report: dict[str, Any], snapshot: dict[str, Any],
        ) -> tuple[list[str], dict[str, Any] | None]:
    """Derive the sole allowed raw-exit-2 elevation from outer evidence.

    The child intentionally refuses to attest its own host/runtime closure and
    therefore treats otherwise exhaustive audit evidence as truncated.  The
    gate may replace only that narrow uncertainty: its separately persisted
    audit application envelope must be current, and every deterministic,
    audit, ledger, emission, and source relation must independently be green.
    """
    errors: list[str] = []
    audit_snapshot = snapshot.get("audit")
    if (not isinstance(audit_snapshot, dict)
            or audit_snapshot.get("application_scope_attested") is not True
            or not isinstance(
                audit_snapshot.get("application_attestation"), dict)):
        return ["outer audit application execution is not attested"], None
    if report.get("status") != "unevaluable" or report.get(
            "status_reasons") != [
                "corpus coverage or one or more forms are unevaluable",
                (
                    "standalone referee runtime/application attestation is "
                    "incomplete and non-enforceable"
                ),
            ]:
        errors.append("raw report has non-exclusive unevaluable reasons")
    if report.get("errors") != []:
        errors.append("raw report contains form execution errors")
    forms = report.get("forms")
    if not isinstance(forms, list):
        return [*errors, "raw report forms are missing"], None

    effective_subjects = 0
    for form in forms:
        if not isinstance(form, dict):
            errors.append("raw report contains a malformed form")
            continue
        slug = form.get("slug")
        audit_evidence = form.get("audit_evidence")
        manifest = audit_evidence.get("manifest_binding") if isinstance(
            audit_evidence, dict) else None
        ledger = audit_evidence.get("ledger_binding") if isinstance(
            audit_evidence, dict) else None
        outer_form = audit_snapshot.get("forms", {}).get(slug)
        relation = outer_form.get("assertion_relation") if isinstance(
            outer_form, dict) else None
        layout_binding = snapshot.get("layout_bindings", {}).get(slug)
        try:
            layout_owner_ids = _layout_audit_owner_ids(layout_binding)
        except CombRefereeScopeError as error:
            errors.append(f"layout owner registry is invalid: {slug}: {error}")
            layout_owner_ids = None
        if (not isinstance(audit_evidence, dict)
                or audit_evidence.get("complete") is not False
                or audit_evidence.get("integrity_valid") is not False
                or audit_evidence.get(
                    "runtime_closure_independently_attested") is not False
                or audit_evidence.get("assertion_valid") is not True
                or audit_evidence.get("errors") != []
                or audit_evidence.get("input_manifest_verified") is not True
                or audit_evidence.get("evidence_published") is not True
                or audit_evidence.get("byte_and_relation_binding_valid")
                is not True):
            errors.append(f"raw audit evidence has a non-scope failure: {slug}")
        if (not isinstance(manifest, dict)
                or manifest.get("binding_valid") is not True
                or manifest.get("manifest_inputs_complete") is not True
                or manifest.get("errors") != []
                or manifest.get("blockers") != RAW_AUDIT_SCOPE_BLOCKERS
                or manifest.get("reason") != "; ".join(
                    f"blocked: {item}" for item in RAW_AUDIT_SCOPE_BLOCKERS)
                or manifest.get("attestation_complete") is not False
                or manifest.get("enforceable") is not False
                or manifest.get("complete") is not False
                or manifest.get("runtime_manifest_self_consistent") is not True
                or manifest.get("base_runtime_closure_independently_attested")
                is not False
                or manifest.get(
                    "roundtrip_runtime_closure_independently_attested")
                is not False
                or manifest.get("roundtrip_present") is not True):
            errors.append(f"raw audit manifest has a non-scope failure: {slug}")
        if (not isinstance(ledger, dict)
                or ledger.get("binding_valid") is not True
                or ledger.get("reason") != "complete"
                or ledger.get("errors") != []):
            errors.append(f"raw audit ledger has a failure: {slug}")
        if (not isinstance(relation, dict)
                or relation.get("holds") is not True
                or relation.get("inventory_complete") is not True
                or relation.get("offender_count") != 0
                or relation.get("offender_dimensions") != {}
                or relation.get("expected_comb_ids")
                != relation.get("checked_comb_ids")
                or relation.get("expected_comb_ids")
                != relation.get("emitted_comb_ids")
                or relation.get("owner_certificates_invalid") != 0
                or relation.get("owner_certificates_valid")
                != relation.get("combs_checked")
                or any(relation.get(key) != 0 for key in (
                    "raw_live_comb_issues", "emitted_cell_binding_issues",
                    "layout_mismatches", "layout_unevaluable",
                    "emission_behind_layout", "emission_invalid"))):
            errors.append(f"outer audit assertion is not green: {slug}")
        if (not isinstance(form.get("emission_inventory"), dict)
                or form["emission_inventory"].get("complete") is not True
                or form.get("emission_binding_errors") != []):
            errors.append(f"emission evidence is not complete: {slug}")
        counts = form.get("counts")
        cells = form.get("cells")
        if (not isinstance(counts, dict) or not isinstance(cells, list)
                or counts.get("ledger_blocking") != 0
                or counts.get("subjects_active_resolved") != len(cells)
                or counts.get("subjects_active_unresolved") != 0
                or counts.get("subjects_retained_unresolved") != 0
                or counts.get("inferences_suppressed") != 0
                or form.get("inferences") != []):
            errors.append(f"ledger evidence is not fully resolved: {slug}")
            continue
        report_ids = [
            cell.get("cell") if isinstance(cell, dict) else None
            for cell in cells
        ]
        emission_inventory = form.get("emission_inventory")
        if layout_owner_ids is not None:
            exact_inventories = [
                list(layout_binding.get("cells", {})),
                report_ids,
                relation.get("expected_comb_ids") if isinstance(
                    relation, dict) else None,
                relation.get("checked_comb_ids") if isinstance(
                    relation, dict) else None,
                relation.get("emitted_comb_ids") if isinstance(
                    relation, dict) else None,
                audit_evidence.get("expected_comb_ids") if isinstance(
                    audit_evidence, dict) else None,
                audit_evidence.get("checked_comb_ids") if isinstance(
                    audit_evidence, dict) else None,
                audit_evidence.get("emitted_comb_ids") if isinstance(
                    audit_evidence, dict) else None,
                ledger.get("active_subject_ids") if isinstance(
                    ledger, dict) else None,
                ledger.get("emitted_ids") if isinstance(ledger, dict) else None,
                emission_inventory.get("expected_active_cell_ids")
                if isinstance(emission_inventory, dict) else None,
                emission_inventory.get("emitted_cell_ids")
                if isinstance(emission_inventory, dict) else None,
            ]
            if any(inventory != layout_owner_ids
                   for inventory in exact_inventories):
                errors.append(
                    f"elevatable owner/audit/report/ledger inventories differ: "
                    f"{slug}")
        for cell in cells:
            if not isinstance(cell, dict):
                errors.append(f"cell evidence is malformed: {slug}")
                continue
            referee = cell.get("referee")
            expected_raw_four_way = {
                "referee": (
                    referee.get("compartments")
                    if isinstance(referee, dict) else None),
                "lattice": cell.get("latticed"),
                "audit": None,
                "emitted": cell.get("emitted"),
            }
            if (cell.get("ledger_state") != "active_resolved"
                    or cell.get("ledger_blocks_gate") is not False
                    or not isinstance(referee, dict)
                    or referee.get("status") != "measured"
                    or referee.get("positions_match") is not True
                    or referee.get("compartments") != cell.get("latticed")
                    or cell.get("emitted") != cell.get("latticed")
                    or cell.get("emitted_indexes_valid") is not True
                    or cell.get("audit_printed") is not None
                    or cell.get("audit_relation") != "unknown-truncated"
                    or cell.get("comparison_status") != "unevaluable"
                    or cell.get("comparison_reason")
                    != "audit evidence is incomplete"
                    or cell.get("transition_status") != "none"
                    or cell.get("four_way") != expected_raw_four_way):
                errors.append(
                    f"cell has a non-audit-scope blocker: "
                    f"{slug}/{cell.get('cell')}")
        effective_subjects += len(cells)

    if errors:
        return errors, None
    raw_totals = report.get("totals")
    if not isinstance(raw_totals, dict):
        return ["raw totals are missing"], None
    effective_totals = dict(raw_totals)
    effective_totals.update({
        "combs_unevaluable": 0,
        "forms_ok": len(forms),
        "forms_disagreement": 0,
        "forms_unevaluable": 0,
        "audit_evidence_complete_forms": len(forms),
        "comparisons": {
            name: effective_subjects if name == "agree" else 0
            for name in COMPARISON_NAMES
        },
    })
    return [], effective_totals


def report_binding_errors(report: dict[str, Any],
                          snapshot: dict[str, Any],
                          stats: dict[str, Any] | None = None) -> list[str]:
    """Bind the child's own provenance claims to the outer application scope."""
    errors: list[str] = []
    producers = snapshot["producers"]
    referee = producers["tools/formgen/comb_referee.py"]
    if report.get("producer_sha256") != referee["sha256"]:
        errors.append("report producer digest disagrees with snapshot")
    provenance = report.get("provenance", {})
    producer = provenance.get("producer", {}) if isinstance(provenance, dict) else {}
    expected_producer = {
        "file": "tools/formgen/comb_referee.py",
        "bytes": referee["bytes"],
        "sha256": referee["sha256"],
    }
    if producer != expected_producer:
        errors.append("report producer provenance is not bound")
    dependencies = provenance.get("dependencies", {}) if isinstance(
        provenance, dict) else {}
    expected_children = [
        {
            "file": relative,
            "bytes": producers[relative]["bytes"],
            "sha256": producers[relative]["sha256"],
            "expected_sha256": producers[relative]["sha256"],
        }
        for relative in REPORT_AUDIT_CHILD_DEPENDENCIES
    ]
    audit_record = producers["tools/formgen/audit.py"]
    lattice_record = producers["tools/formgen/lattice.py"]
    expected_dependencies = {
        "audit": {
            "file": "tools/formgen/audit.py",
            "bytes": audit_record["bytes"],
            "sha256": audit_record["sha256"],
            "expected_sha256": audit_record["sha256"],
            "dependencies": expected_children,
        },
        "lattice": {
            "file": "tools/formgen/lattice.py",
            "bytes": lattice_record["bytes"],
            "sha256": lattice_record["sha256"],
            "expected_sha256": lattice_record["sha256"],
        },
    }
    if dependencies != expected_dependencies:
        errors.append("report dependency provenance closure is not bound")
    runtime = provenance.get("runtime", {}) if isinstance(provenance, dict) else {}
    python = snapshot["runtime"]["python"]
    poppler = snapshot["runtime"]["pdftocairo"]
    if (not isinstance(runtime, dict) or set(runtime) != REPORT_RUNTIME_KEYS
            or runtime.get("python_executable") != python["path"]
            or runtime.get("python_executable_sha256") != python["sha256"]):
        errors.append("report Python executable is not bound")
    report_poppler = report.get("poppler", {})
    if (report_poppler.get("binary_path") != poppler["path"]
            or report_poppler.get("binary_sha256") != poppler["sha256"]
            or runtime.get("poppler") != report_poppler):
        errors.append("report pdftocairo executable is not bound")
    inputs = report.get("inputs", {})
    expected_inputs = {
        "audit_sha256": snapshot["audit"]["sha256"],
        "audit_bytes": snapshot["audit"]["bytes"],
        "layout_count": EXPECTED_FORMS,
    }
    if inputs != expected_inputs:
        errors.append("report audit/layout inputs are not bound")
    audit_forms = snapshot["audit"].get("forms")
    report_forms = report.get("forms")
    if (not isinstance(audit_forms, dict)
            or len(audit_forms) != EXPECTED_FORMS
            or not isinstance(report_forms, list)):
        errors.append("outer per-form audit scope is incomplete")
    else:
        for form in report_forms:
            if isinstance(form, dict):
                errors.extend(form_binding_errors(form, snapshot))
    elevation_errors, effective_totals = derive_application_scope_elevation(
        report, snapshot)
    if stats is not None:
        stats["application_scope_elevated"] = not elevation_errors
        stats["application_scope_elevation_errors"] = elevation_errors
        stats["effective_totals"] = effective_totals
        if effective_totals is not None:
            stats["application_status"] = "ok"
    return errors


ENVELOPE_KEYS = {
    "schema_version", "application_scope_name", "application_snapshot",
    "invocation", "raw_report", "relations", "host_tcb_required",
    "host_scope_complete", "host_closure_claimed", "operating_system_bound",
    "python_stdlib_bound", "dynamic_libraries_bound",
    "application_scope_complete", "enforceable", "enforcement_scope",
    "self_digest", "payload_sha256",
}
ENVELOPE_RELATIONS = {
    "clean_revision_before_after",
    "tracked_producers_equal_head_before_after",
    "declared_inputs_hashed_before_after",
    "python_executable_hashed_before_after",
    "pdftocairo_executable_hashed_before_after",
    "sanitized_python_environment",
    "isolated_python_mode",
    "fresh_isolated_pycache_prefix",
    "hard_timeout_enforced",
    "child_report_schema_valid",
    "child_report_self_digest_valid",
    "child_exit_matches_report_status",
    "repeat_run_byte_identical",
    "validated_output_only",
    "atomic_report_publish",
    "atomic_envelope_publish",
}
INVOCATION_KEYS = {
    "executable", "resolved_executable", "python_flags",
    "pythonpath_removed", "pythonhome_removed", "timeout_seconds",
    "total_timeout_seconds", "run_count", "child_exits", "output",
    "child_exit",
}
RAW_REPORT_KEYS = {
    "file", "bytes", "sha256", "payload_sha256", "schema_version",
    "status", "repeat_sha256",
}


def validate_comb_referee_envelope(
        envelope: Any, raw_payload: bytes, report: dict[str, Any],
        current_snapshot: dict[str, Any] | None = None,
        ) -> list[str]:
    """Validate the deterministic application-only envelope and currentness."""
    errors: list[str] = []
    if not isinstance(envelope, dict):
        return ["attestation envelope is not an object"]
    if set(envelope) != ENVELOPE_KEYS:
        errors.append("attestation envelope schema is incomplete or unsupported")
    if envelope.get("schema_version") != COMB_REFEREE_ATTESTATION_VERSION:
        errors.append("attestation envelope version is unsupported")
    if envelope.get("application_scope_name") != COMB_REFEREE_SCOPE:
        errors.append("attestation application scope is wrong")
    if not self_digest_valid(envelope):
        errors.append("attestation envelope self-digest is missing or stale")
    relations = envelope.get("relations")
    if (not isinstance(relations, dict)
            or set(relations) != ENVELOPE_RELATIONS
            or any(value is not True for value in relations.values())):
        errors.append("one or more application-scope relations are not enforced")
    boundary = {
        "host_tcb_required": True,
        "host_scope_complete": False,
        "host_closure_claimed": False,
        "operating_system_bound": False,
        "python_stdlib_bound": False,
        "dynamic_libraries_bound": False,
        "application_scope_complete": True,
        "enforceable": True,
        "enforcement_scope": "application-only",
    }
    for key, expected in boundary.items():
        if envelope.get(key) != expected:
            errors.append(f"attestation boundary is invalid: {key}")
    snapshot = envelope.get("application_snapshot")
    if not isinstance(snapshot, dict):
        errors.append("attestation application snapshot is missing")
        snapshot = {}
    if current_snapshot is not None and snapshot != current_snapshot:
        errors.append("attestation is stale for the current application snapshot")
    invocation = envelope.get("invocation")
    if not isinstance(invocation, dict):
        errors.append("attested invocation is missing")
        invocation = {}
    elif set(invocation) != INVOCATION_KEYS:
        errors.append("attested invocation schema is unsupported")
    child_exit = invocation.get("child_exit")
    expected_exit = {"ok": 0, "disagreement": 1, "unevaluable": 2}.get(
        report.get("status"))
    if child_exit != expected_exit:
        errors.append("attested child exit disagrees with report status")
    snapshot_python = (
        snapshot.get("runtime", {}).get("python", {})
        if isinstance(snapshot, dict) else {})
    if (invocation.get("executable") != sys.executable
            or invocation.get("resolved_executable")
            != snapshot_python.get("path")
            or invocation.get("python_flags")
            != ISOLATED_PYTHON_ATTESTED_FLAGS
            or invocation.get("pythonpath_removed") is not True
            or invocation.get("pythonhome_removed") is not True
            or invocation.get("timeout_seconds") != COMB_REFEREE_TIMEOUT_SECONDS
            or invocation.get("total_timeout_seconds")
            != COMB_REFEREE_TOTAL_TIMEOUT_SECONDS
            or invocation.get("run_count") != COMB_REFEREE_RUN_COUNT
            or invocation.get("child_exits")
            != [expected_exit] * COMB_REFEREE_RUN_COUNT
            or invocation.get("output") != "private-temporary-output"):
        errors.append("attested invocation contract is incomplete")
    raw = envelope.get("raw_report")
    if not isinstance(raw, dict):
        errors.append("attested raw report identity is missing")
        raw = {}
    elif set(raw) != RAW_REPORT_KEYS:
        errors.append("attested raw report schema is unsupported")
    if (raw.get("file") != "build/comb-referee.json"
            or raw.get("bytes") != len(raw_payload)
            or raw.get("sha256") != sha256_bytes(raw_payload)
            or raw.get("payload_sha256") != report.get("payload_sha256")
            or raw.get("schema_version") != report.get("schema_version")
            or raw.get("status") != report.get("status")
            or raw.get("repeat_sha256")
            != [sha256_bytes(raw_payload)] * COMB_REFEREE_RUN_COUNT):
        errors.append("raw report is missing, stale, or not bound to the envelope")
    if snapshot:
        errors.extend(report_binding_errors(report, snapshot))
    return errors


def _atomic_write(path: pathlib.Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary_path = pathlib.Path(temporary)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary_path, path)
        try:
            directory_fd = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
        except OSError:
            # Directory durability is host TCB; atomic replace still holds.
            pass
    finally:
        try:
            temporary_path.unlink()
        except FileNotFoundError:
            pass


def _sanitized_referee_environment(
        snapshot: dict[str, Any],
        base_environment: dict[str, str] | None = None,
        ) -> dict[str, str]:
    environment = dict(os.environ if base_environment is None
                       else base_environment)
    environment.pop("PYTHONPATH", None)
    environment.pop("PYTHONHOME", None)
    poppler = pathlib.Path(snapshot["runtime"]["pdftocairo"]["path"])
    # Make the child's shutil.which resolve the exact executable we hashed.
    environment["PATH"] = str(poppler.parent)
    return environment


def _comb_referee_command(
        output: pathlib.Path, pycache_prefix: pathlib.Path,
        ) -> list[str]:
    return [
        sys.executable, "-I", "-B", "-X",
        f"pycache_prefix={pycache_prefix}", str(HERE / "comb_referee.py"),
        "--source-root", str(COMB_REFEREE_SOURCE_ROOT),
        "--layout-dir", str(BUILD / "layout"),
        "--ir-dir", str(BUILD / "ir"),
        "--html-dir", str(BUILD / "html"),
        "--guide-dir", str(BUILD / "guides"),
        "--audit", str(AUDIT_JSON),
        "--out", str(output),
    ]


def _run_comb_referee_bounded(
        command: Sequence[str], environment: dict[str, str],
        timeout: int = COMB_REFEREE_TIMEOUT_SECONDS,
        ) -> tuple[int, str]:
    process = subprocess.Popen(
        list(command), cwd=REPO, env=environment,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        start_new_session=(os.name == "posix"),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        if os.name == "posix":
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        else:
            process.kill()
        try:
            process.communicate(timeout=COMB_REFEREE_CLEANUP_TIMEOUT_SECONDS)
        except subprocess.TimeoutExpired:
            process.kill()
            try:
                process.communicate(
                    timeout=COMB_REFEREE_CLEANUP_TIMEOUT_SECONDS)
            except subprocess.TimeoutExpired as cleanup_error:
                raise CombRefereeScopeError(
                    "comb referee process group could not be reaped within "
                    "the bounded cleanup budget") from cleanup_error
        raise CombRefereeScopeError(
            f"comb referee exceeded hard {timeout}-second timeout") from error
    return process.returncode, stdout + stderr


def repeat_run_errors(exits: Sequence[int],
                      payloads: Sequence[bytes]) -> list[str]:
    """Pure repeated-run relation: both isolated children must be identical."""
    errors: list[str] = []
    if len(exits) != COMB_REFEREE_RUN_COUNT:
        errors.append("referee repeated-run exit inventory is incomplete")
    if len(payloads) != COMB_REFEREE_RUN_COUNT:
        errors.append("referee repeated-run payload inventory is incomplete")
    if exits and any(exit_code != exits[0] for exit_code in exits[1:]):
        errors.append("referee repeated-run exit codes differ")
    if payloads and any(payload != payloads[0] for payload in payloads[1:]):
        errors.append("referee repeated-run output bytes differ")
    return errors


def repeat_run_failure(exits: Sequence[int],
                       payloads: Sequence[bytes]) -> Result | None:
    errors = repeat_run_errors(exits, payloads)
    if errors:
        return Result("comb-referee", Verdict.UNEVALUABLE, "; ".join(errors))
    return None


def _comb_referee_outcome(report: dict[str, Any],
                          stats: dict[str, Any], *,
                          expected_forms: int = EXPECTED_FORMS,
                          expected_subjects: int = EXPECTED_COMB_SUBJECTS,
                          ) -> Result:
    elevated = stats.get("application_scope_elevated") is True
    totals = (
        stats.get("effective_totals")
        if elevated else report["totals"])
    if not isinstance(totals, dict):
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            "application-scope effective totals are missing")
    comparisons = totals["comparisons"]
    disagreements = sum(comparisons[name] for name in (
        "repair-lattice", "repair-audit", "stale-generation", "stop"))
    independently_derived_mismatches = sum(
        int(stats.get(key, 0)) for key in (
            "referee_layout_mismatches",
            "referee_layout_position_mismatches",
            "emission_layout_mismatches",
        ))
    if independently_derived_mismatches:
        return Result(
            "comb-referee", Verdict.FAIL,
            f"{independently_derived_mismatches} independently derived "
            "source/layout/emission mismatch(es)")
    if disagreements or (not elevated and report["status"] == "disagreement"):
        detail = ", ".join(
            f"{name}={comparisons[name]}" for name in (
                "repair-lattice", "repair-audit", "stale-generation", "stop")
            if comparisons[name])
        return Result("comb-referee", Verdict.FAIL,
                      f"{disagreements} actual disagreement(s): {detail}")
    required = {
        "forms_expected": expected_forms,
        "forms_measured": expected_forms,
        "forms_error": 0,
        "combs_expected": expected_subjects,
        "combs_found": expected_subjects,
        "combs_measured": expected_subjects,
        "combs_unevaluable": 0,
        "combs_source_unevaluable": 0,
        "subjects_active": expected_subjects,
        "subjects_active_resolved": expected_subjects,
        "subjects_active_unresolved": 0,
        "subjects_retained_unresolved": 0,
        "inferences_suppressed": 0,
        "ledger_blocking": 0,
        "referee_layout_mismatches": 0,
        "referee_layout_position_mismatches": 0,
        "forms_ok": expected_forms,
        "forms_disagreement": 0,
        "forms_unevaluable": 0,
        "audit_evidence_complete_forms": expected_forms,
    }
    incomplete = [
        f"{key}={totals.get(key)} (expected {expected})"
        for key, expected in required.items() if totals.get(key) != expected
    ]
    if comparisons["agree"] != expected_subjects:
        incomplete.append(
            f"comparisons.agree={comparisons['agree']} "
            f"(expected {expected_subjects})")
    if comparisons["unevaluable"]:
        incomplete.append(f"comparisons.unevaluable={comparisons['unevaluable']}")
    for key in (
            "referee_layout_mismatches",
            "referee_layout_position_mismatches",
            "emission_layout_mismatches"):
        if stats.get(key):
            incomplete.append(f"derived.{key}={stats[key]} (expected 0)")
    if stats["pending_transitions"]:
        incomplete.append(
            f"pending_transitions={stats['pending_transitions']} (expected 0)")
    if report["errors"]:
        incomplete.append(f"errors={len(report['errors'])}")
    if stats.get("application_status") != "ok":
        incomplete.append(
            f"application_status={stats.get('application_status')} "
            "(expected ok)")
    if incomplete:
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      "; ".join(incomplete[:8]))
    if not elevated:
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            "outer audit/referee application scope did not exclusively "
            "close the raw host-attestation gap: "
            + "; ".join(
                stats.get("application_scope_elevation_errors", [])[:3]),
        )
    return Result(
        "comb-referee", Verdict.PASS,
        f"{expected_forms} forms / {expected_subjects} subjects agree; "
        "application scope attested (host TCB explicitly required)",
    )


def check_comb_referee() -> Result:
    try:
        raw_payload = COMB_REFEREE_REPORT.read_bytes()
        envelope_payload = COMB_REFEREE_ATTESTATION.read_bytes()
        report = json.loads(raw_payload)
        envelope = json.loads(envelope_payload)
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      f"missing or malformed report/envelope: {error}")
    child_exit = None
    if isinstance(envelope, dict) and isinstance(envelope.get("invocation"), dict):
        child_exit = envelope["invocation"].get("child_exit")
    try:
        report_errors, stats = validate_comb_referee_report(
            report, child_exit=child_exit)
    except Exception as error:  # noqa: BLE001 - malformed is UNEVALUABLE
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      f"report validation failed closed: {error}")
    if report_errors:
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      "; ".join(report_errors[:5]))
    try:
        current = capture_comb_referee_snapshot()
    except Exception as error:  # noqa: BLE001 - currentness must fail closed
        return Result("comb-referee", Verdict.UNEVALUABLE, str(error))
    binding_errors = report_binding_errors(report, current, stats)
    if binding_errors:
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      "; ".join(binding_errors[:5]))
    try:
        envelope_errors = validate_comb_referee_envelope(
            envelope, raw_payload, report, current)
    except Exception as error:  # noqa: BLE001 - malformed is UNEVALUABLE
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      f"envelope validation failed closed: {error}")
    if envelope_errors:
        return Result("comb-referee", Verdict.UNEVALUABLE,
                      "; ".join(envelope_errors[:5]))
    return _comb_referee_outcome(report, stats)


def refresh_comb_referee_report() -> Result:
    """Run the referee inside a clean, immutable application-scoped envelope."""
    try:
        before = capture_comb_referee_snapshot()
        environment = _sanitized_referee_environment(before)
        with tempfile.TemporaryDirectory(
                prefix=".comb-referee-", dir=BUILD) as temporary:
            exits: list[int] = []
            payloads: list[bytes] = []
            reports: list[dict[str, Any]] = []
            for run_index in range(COMB_REFEREE_RUN_COUNT):
                fresh_path = (
                    pathlib.Path(temporary)
                    / f"comb-referee-{run_index + 1}.json")
                pycache_prefix = (
                    pathlib.Path(temporary)
                    / f"python-pycache-{run_index + 1}")
                pycache_prefix.mkdir()
                child_exit, _diagnostic = _run_comb_referee_bounded(
                    _comb_referee_command(
                        fresh_path, pycache_prefix), environment)
                exits.append(child_exit)
                current = capture_comb_referee_snapshot()
                changed = snapshot_pair_errors(before, current)
                if changed:
                    raise CombRefereeScopeError("; ".join(changed))
                try:
                    payload = fresh_path.read_bytes()
                    child_report = json.loads(payload)
                except (OSError, UnicodeError, json.JSONDecodeError) as error:
                    raise CombRefereeScopeError(
                        f"referee run {run_index + 1} produced no usable "
                        f"report: {error}") from error
                report_errors, run_stats = validate_comb_referee_report(
                    child_report, child_exit=child_exit)
                report_errors.extend(
                    report_binding_errors(child_report, before, run_stats))
                if report_errors:
                    raise CombRefereeScopeError(
                        "; ".join(report_errors[:8]))
                payloads.append(payload)
                reports.append(child_report)
            repeated_failure = repeat_run_failure(exits, payloads)
            if repeated_failure is not None:
                raise CombRefereeScopeError(repeated_failure.detail)
            raw_payload = payloads[0]
            report = reports[0]
            child_exit = exits[0]

            relations = {name: True for name in ENVELOPE_RELATIONS}
            envelope: dict[str, Any] = {
                "schema_version": COMB_REFEREE_ATTESTATION_VERSION,
                "application_scope_name": COMB_REFEREE_SCOPE,
                "application_snapshot": before,
                "invocation": {
                    "executable": sys.executable,
                    "resolved_executable": before["runtime"]["python"]["path"],
                    "python_flags": list(ISOLATED_PYTHON_ATTESTED_FLAGS),
                    "pythonpath_removed": True,
                    "pythonhome_removed": True,
                    "timeout_seconds": COMB_REFEREE_TIMEOUT_SECONDS,
                    "total_timeout_seconds": (
                        COMB_REFEREE_TOTAL_TIMEOUT_SECONDS),
                    "run_count": COMB_REFEREE_RUN_COUNT,
                    "child_exits": exits,
                    "output": "private-temporary-output",
                    "child_exit": child_exit,
                },
                "raw_report": {
                    "file": "build/comb-referee.json",
                    "bytes": len(raw_payload),
                    "sha256": sha256_bytes(raw_payload),
                    "payload_sha256": report["payload_sha256"],
                    "schema_version": report["schema_version"],
                    "status": report["status"],
                    "repeat_sha256": [
                        sha256_bytes(payload) for payload in payloads
                    ],
                },
                "relations": relations,
                "host_tcb_required": True,
                "host_scope_complete": False,
                "host_closure_claimed": False,
                "operating_system_bound": False,
                "python_stdlib_bound": False,
                "dynamic_libraries_bound": False,
                "application_scope_complete": all(relations.values()),
                "enforceable": all(relations.values()),
                "enforcement_scope": "application-only",
            }
            attach_self_digest(envelope)
            envelope_payload = (
                json.dumps(envelope, indent=2, sort_keys=True,
                           ensure_ascii=False) + "\n").encode("utf-8")
            # Each publication is atomic. Publishing the report first leaves
            # any old envelope stale (UNEVALUABLE), never falsely green.
            _atomic_write(COMB_REFEREE_REPORT, raw_payload)
            _atomic_write(COMB_REFEREE_ATTESTATION, envelope_payload)
    except Exception as error:  # noqa: BLE001 - any incomplete run is not evidence
        return Result("comb-referee", Verdict.UNEVALUABLE, str(error))
    return check_comb_referee()


@dataclasses.dataclass
class FullRefresh:
    determinism: Result
    audit_refresh: Result
    comb_referee: Result
    diagnostics: list[str]
    generated_scope: dict[str, Any] | None = None


BATCH_RECORD_KEYS = {
    "slug", "code", "revision", "variant", "in_corpus", "source_file",
    "sha256", "stage_failed", "error", "images_extracted", "pages",
    "paper", "uniform_paper", "fonts", "rules", "text_runs", "images",
    "cells", "comb_cells", "growables", "sources", "guide",
    "guide_detected", "html_bytes", "html", "guide_build",
    "guide_source_irs", "font_plans",
}
AUDIT_ATTESTATION_KEYS = {
    "inputs_complete", "producer_execution_bound",
    "base_runtime_scope_complete", "roundtrip_runtime_scope_complete",
    "validated_before_after", "complete", "enforceable",
    "incomplete_reasons", "future_gate_required",
}
AUDIT_INPUT_MANIFEST_KEYS = {
    "schema", "algorithm", "producer", "runtime", "inputs_complete",
    "attestation_complete", "enforceable", "complete", "missing_required",
    "inputs", "render",
}


def canonical_form_slugs(head: str | None = None) -> frozenset[str]:
    """The exact tracked corpus, independent of regenerated working bytes."""
    revision = head or _git_text(("rev-parse", "--verify", "HEAD"))
    paths = _git((
        "ls-tree", "-r", "--name-only", revision, "--", "forms",
    )).decode("utf-8", errors="strict").splitlines()
    slugs = {
        parts[1]
        for name in paths
        for parts in (pathlib.PurePosixPath(name).parts,)
        if len(parts) == 3
        and parts[0] == "forms"
        and parts[2] == "provenance.json"
    }
    if len(slugs) != EXPECTED_FORMS:
        raise CombRefereeScopeError(
            f"tracked form corpus has {len(slugs)}/{EXPECTED_FORMS} slugs")
    return frozenset(slugs)


def batch_report_errors(
        data: Any, expected_slugs: frozenset[str],
        ) -> list[str]:
    errors: list[str] = []
    if not isinstance(data, list):
        return ["batch report is not a list"]
    slugs: list[str] = []
    for index, record in enumerate(data):
        if not isinstance(record, dict) or set(record) != BATCH_RECORD_KEYS:
            errors.append(f"batch record schema is unsupported: {index}")
            continue
        slug = record.get("slug")
        code = record.get("code")
        revision = record.get("revision")
        variant = record.get("variant")
        if not all(isinstance(value, str) for value in (
                slug, code, revision, variant)):
            errors.append(f"batch record identity is malformed: {index}")
            continue
        expected_slug = f"{code}-{revision}{f'-{variant}' if variant else ''}".lower()
        if not slug or slug != expected_slug:
            errors.append(f"batch record identity relation is false: {slug}")
        slugs.append(slug)
        if (not isinstance(record.get("in_corpus"), bool)
                or not isinstance(record.get("source_file"), str)
                or not record["source_file"]
                or not _is_sha256(record.get("sha256"))
                or record.get("stage_failed") is not None
                or record.get("error") is not None):
            errors.append(f"batch record did not complete: {slug}")
        for key in (
                "images_extracted", "pages", "rules", "text_runs", "images",
                "cells", "comb_cells", "html_bytes"):
            if not _is_count(record.get(key)):
                errors.append(f"batch record count is malformed: {slug}/{key}")
        if (not isinstance(record.get("paper"), str)
                or not isinstance(record.get("uniform_paper"), bool)
                or not isinstance(record.get("fonts"), list)
                or not isinstance(record.get("growables"), list)
                or not isinstance(record.get("sources"), list)
                or not isinstance(record.get("guide_detected"), dict)
                or not isinstance(record.get("guide_build"), dict)
                or not isinstance(record.get("guide_source_irs"), list)
                or not isinstance(record.get("font_plans"), list)):
            errors.append(f"batch record evidence is malformed: {slug}")
        form_sources = [
            source for source in record.get("sources", [])
            if isinstance(source, dict) and source.get("role") == "form"
        ]
        if (len(form_sources) != 1
                or form_sources[0].get("file") != record.get("source_file")
                or form_sources[0].get("sha256") != record.get("sha256")):
            errors.append(f"batch source relation is false: {slug}")
    if len(slugs) != len(set(slugs)):
        errors.append("batch report contains duplicate slugs")
    if set(slugs) != set(expected_slugs):
        errors.append("batch report does not match the exact tracked slug corpus")
    return errors


def _fresh_batch_report(
        path: pathlib.Path, expected_slugs: frozenset[str],
        ) -> tuple[dict[str, Any], bytes]:
    record = _stable_file_record(path, "build/batch-report.json")
    payload = path.read_bytes()
    if sha256_bytes(payload) != record["sha256"]:
        raise CombRefereeScopeError("batch report changed while validating")
    try:
        data = json.loads(payload)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise CombRefereeScopeError(f"batch report is malformed: {error}") from error
    errors = batch_report_errors(data, expected_slugs)
    if errors:
        raise CombRefereeScopeError("; ".join(errors[:5]))
    record["form_count"] = len(data)
    record["slug_sha256"] = canonical_digest(sorted(expected_slugs))
    return record, payload


def full_audit_payload_errors(
        data: Any, expected_slugs: frozenset[str],
        ) -> list[str]:
    """Reject a slug-only JSON fixture; require producer-shaped relations."""
    errors: list[str] = []
    if not isinstance(data, list):
        return ["audit report is not a list"]
    slugs: list[str] = []
    for index, record in enumerate(data):
        if not isinstance(record, dict):
            errors.append(f"audit record is not an object: {index}")
            continue
        slug = record.get("slug")
        if not isinstance(slug, str) or not slug:
            errors.append(f"audit record has no slug: {index}")
            continue
        slugs.append(slug)
        manifest = record.get("input_manifest")
        assertions = record.get("assertions")
        provenance = record.get("provenance_validation")
        attestation = record.get("attestation")
        if (not isinstance(manifest, dict)
                or set(manifest) != AUDIT_INPUT_MANIFEST_KEYS
                or not isinstance(manifest.get("inputs"), dict)
                or not isinstance(manifest.get("render"), dict)):
            errors.append(f"audit input manifest is malformed: {slug}")
        if (not isinstance(provenance, dict)
                or set(provenance) != {
                    "validated_before", "validated_after", "error"}
                or not isinstance(provenance.get("validated_before"), bool)
                or not isinstance(provenance.get("validated_after"), bool)):
            errors.append(f"audit provenance relation is malformed: {slug}")
        if (not isinstance(attestation, dict)
                or set(attestation) != AUDIT_ATTESTATION_KEYS
                or not isinstance(attestation.get("complete"), bool)
                or not isinstance(attestation.get("enforceable"), bool)):
            errors.append(f"audit attestation is malformed: {slug}")
        if (not isinstance(assertions, dict)
                or set(assertions) != set(REQUIRED_ASSERTIONS)):
            errors.append(f"audit assertion inventory is malformed: {slug}")
            continue
        held_count = 0
        for key in REQUIRED_ASSERTIONS:
            detail = assertions.get(key)
            if (not isinstance(detail, dict)
                    or not isinstance(detail.get("holds"), bool)
                    or record.get(key) is not detail.get("holds")):
                errors.append(f"audit assertion relation is false: {slug}/{key}")
            elif detail["holds"]:
                held_count += 1
        if record.get("assertions_held") != held_count:
            errors.append(f"audit assertion total is false: {slug}")
        try:
            _normalise_outer_comb_assertion(
                assertions.get("comb_slots_match_printed"))
        except CombRefereeScopeError as error:
            errors.append(f"audit comb publication is invalid: {slug}: {error}")
        if record.get("comb_slots_match_printed") is not assertions[
                "comb_slots_match_printed"].get("holds"):
            errors.append(f"audit top-level comb verdict is false: {slug}")
        if record.get("status") not in {"ok", "error"}:
            errors.append(f"audit status is invalid: {slug}")
        if record.get("status") == "ok" and (
                not isinstance(record.get("measured"), bool)
                or not isinstance(record.get("paper_ok"), bool)):
            errors.append(f"audit round-trip relation is incomplete: {slug}")
    if len(slugs) != len(set(slugs)):
        errors.append("audit report contains duplicate slugs")
    if set(slugs) != set(expected_slugs):
        errors.append("audit report does not match the exact tracked slug corpus")
    return errors


def compose_generated_scope(
        trees: dict[str, Any], batch_report: dict[str, Any]) -> dict[str, Any]:
    expected_trees = {"forms", *COMB_REFEREE_ARTIFACT_TREES}
    if set(trees) != expected_trees:
        raise CombRefereeScopeError(
            "generated determinism scope omits or invents an artifact tree")
    unsigned = {"trees": trees, "batch_report": batch_report}
    return {**unsigned, "sha256": canonical_digest(unsigned)}


def compose_final_referee_scope(
        generation: dict[str, Any], audit_record: dict[str, Any],
        ) -> dict[str, Any]:
    unsigned = {"generation": generation, "audit": audit_record}
    return {**unsigned, "sha256": canonical_digest(unsigned)}


def current_audit_identity() -> dict[str, Any]:
    records = [
        _stable_file_record(AUDIT_JSON, "build/audit.json"),
        _stable_file_record(
            AUDIT_APPLICATION_ATTESTATION, "build/audit-attested.json"),
    ]
    return _file_manifest(records)


def generated_scope_manifest(
        batch_report: dict[str, Any] | None = None,
        ) -> dict[str, Any]:
    """Canonical bytes consumed downstream after each batch generation."""
    trees = {
        "forms": _tree_manifest(FORMS, "forms"),
        **{
            name: _tree_manifest(path, f"build/{name}")
            for name, path in COMB_REFEREE_ARTIFACT_TREES.items()
        },
    }
    report_record = batch_report or _stable_file_record(
        BATCH_REPORT, "build/batch-report.json")
    return compose_generated_scope(trees, report_record)


def refresh_full_audit_report(
        target: pathlib.Path = AUDIT_JSON,
        attestation_target: pathlib.Path = AUDIT_APPLICATION_ATTESTATION,
        scratch_root: pathlib.Path = BUILD,
        runner: Callable[[list[str], int], tuple[int, str]] = (
            run_isolated_python),
        expected_slugs: frozenset[str] | None = None,
        scope_reader: Callable[[], dict[str, Any]] = (
            capture_audit_application_snapshot),
        ) -> Result:
    """Publish a full audit only after a successful, complete temp refresh."""
    try:
        before = scope_reader()
        scratch_root.mkdir(parents=True, exist_ok=True)
    except Exception as error:  # noqa: BLE001 - scope must fail closed
        return Result(
            "audit-refresh", Verdict.UNEVALUABLE,
            f"cannot bind audit application scope: {error}")
    with tempfile.TemporaryDirectory(
            prefix=".full-audit-", dir=scratch_root) as temporary:
        fresh = pathlib.Path(temporary) / "audit.json"
        try:
            code, out = runner(
                [str(HERE / "audit.py"), "--out", str(fresh)], 5400)
        except Exception as error:  # noqa: BLE001 - child failure is evidence
            return Result(
                "audit-refresh", Verdict.UNEVALUABLE,
                f"full audit refresh raised: {type(error).__name__}: {error}")
        if code != 0:
            tail = out.strip().splitlines()[-1:] or ["no diagnostic"]
            return Result("audit-refresh", Verdict.UNEVALUABLE,
                          f"full audit refresh failed: {tail[0]}")
        try:
            payload = fresh.read_bytes()
            data = json.loads(payload)
        except (OSError, UnicodeError, json.JSONDecodeError) as error:
            return Result("audit-refresh", Verdict.UNEVALUABLE,
                          f"full audit produced no usable report: {error}")
        try:
            slugs = expected_slugs or canonical_form_slugs()
        except Exception as error:  # noqa: BLE001 - corpus identity is required
            return Result("audit-refresh", Verdict.UNEVALUABLE,
                          f"cannot resolve tracked audit corpus: {error}")
        report_errors = full_audit_payload_errors(data, slugs)
        if report_errors:
            return Result("audit-refresh", Verdict.UNEVALUABLE,
                          "; ".join(report_errors[:5]))
        try:
            after = scope_reader()
            scope_errors = snapshot_pair_errors(before, after)
            if scope_errors:
                return Result(
                    "audit-refresh", Verdict.UNEVALUABLE,
                    "; ".join(scope_errors[:5]))
            relations = {
                key: True for key in AUDIT_APPLICATION_RELATIONS}
            envelope: dict[str, Any] = {
                "schema_version": AUDIT_APPLICATION_ATTESTATION_VERSION,
                "application_scope_name": AUDIT_APPLICATION_SCOPE,
                "application_snapshot": before,
                "invocation": {
                    "executable": sys.executable,
                    "resolved_executable": before["runtime"]["python"]["path"],
                    "python_flags": list(ISOLATED_PYTHON_ATTESTED_FLAGS),
                    "pythonpath_removed": True,
                    "pythonhome_removed": True,
                    "timeout_seconds": 5400,
                    "output": "private-temporary-output",
                    "child_exit": code,
                },
                "raw_report": {
                    "file": "build/audit.json",
                    "bytes": len(payload),
                    "sha256": sha256_bytes(payload),
                    "form_count": len(data),
                },
                "relations": relations,
                "host_tcb_required": True,
                "host_scope_complete": False,
                "host_closure_claimed": False,
                "operating_system_bound": False,
                "python_stdlib_bound": False,
                "dynamic_libraries_bound": False,
                "application_scope_complete": True,
                "enforceable": True,
                "enforcement_scope": "application-only",
            }
            attach_self_digest(envelope)
            envelope_payload = (
                json.dumps(envelope, indent=2, sort_keys=True,
                           ensure_ascii=False) + "\n").encode("utf-8")
            envelope_errors = validate_audit_application_envelope(
                envelope, payload, after)
            if envelope_errors:
                return Result(
                    "audit-refresh", Verdict.UNEVALUABLE,
                    "; ".join(envelope_errors[:5]))
            _atomic_write(target, payload)
            _atomic_write(attestation_target, envelope_payload)
        except Exception as error:  # noqa: BLE001 - publication is fail closed
            return Result("audit-refresh", Verdict.UNEVALUABLE,
                          f"could not publish full audit/envelope: {error}")
    return Result("audit-refresh", Verdict.PASS,
                  f"fresh audit atomically published for {EXPECTED_FORMS} forms")


def refresh_full_pipeline(
        runner: Callable[[list[str], int], tuple[int, str]] = run,
        generation_reader: Callable[[dict[str, Any]], dict[str, Any]] = (
            generated_scope_manifest),
        audit_refresher: Callable[[], Result] = refresh_full_audit_report,
        referee_refresher: Callable[[], Result] | None = (
            refresh_comb_referee_report),
        scratch_root: pathlib.Path = BUILD,
        batch_target: pathlib.Path = BATCH_REPORT,
        expected_slugs: frozenset[str] | None = None,
        audit_identity_reader: Callable[[], dict[str, Any]] = (
            current_audit_identity),
        ) -> FullRefresh:
    """Two generations first, audit the final bytes, then referee exactly last."""
    diagnostics: list[str] = []
    try:
        slugs = expected_slugs or canonical_form_slugs()
        scratch_root.mkdir(parents=True, exist_ok=True)
    except Exception as error:  # noqa: BLE001 - corpus identity is required
        return FullRefresh(
            Result("determinism", Verdict.UNEVALUABLE,
                   f"cannot resolve exact generation corpus: {error}"),
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "generation corpus is unknown; audit not run"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "generation corpus is unknown; referee not run"),
            diagnostics, None,
        )
    generations: list[dict[str, Any]] = []
    batch_payloads: list[bytes] = []
    with tempfile.TemporaryDirectory(
            prefix=".gate-batches-", dir=scratch_root) as temporary:
        for run_index in range(2):
            fresh_report = pathlib.Path(temporary) / (
                f"batch-{run_index + 1}.json")
            batch_args = [
                str(HERE / "batch.py"), "--report", str(fresh_report)]
            try:
                code, out = runner(batch_args, 5400)
            except Exception as error:  # noqa: BLE001
                code, out = 1, f"{type(error).__name__}: {error}"
            if code != 0:
                diagnostics.append(
                    f"batch #{run_index + 1} failed:\n{out[-2000:]}")
                return FullRefresh(
                    Result("determinism", Verdict.FAIL,
                           f"regenerate #{run_index + 1} failed"),
                    Result("audit-refresh", Verdict.UNEVALUABLE,
                           "final generation failed; audit not run"),
                    Result("comb-referee", Verdict.UNEVALUABLE,
                           "final corpus was not generated; referee not run"),
                    diagnostics, None,
                )
            try:
                report_record, payload = _fresh_batch_report(
                    fresh_report, slugs)
                generation = generation_reader(report_record)
            except Exception as error:  # noqa: BLE001
                return FullRefresh(
                    Result("determinism", Verdict.UNEVALUABLE,
                           f"cannot attest generation #{run_index + 1}: {error}"),
                    Result("audit-refresh", Verdict.UNEVALUABLE,
                           "generation attestation failed; audit not run"),
                    Result("comb-referee", Verdict.UNEVALUABLE,
                           "generation attestation failed; referee not run"),
                    diagnostics, None,
                )
            generations.append(generation)
            batch_payloads.append(payload)

    first_generation, second_generation = generations
    first_digest = first_generation.get("sha256")
    second_digest = second_generation.get("sha256")
    if (not _is_sha256(first_digest)
            or first_generation != second_generation):
        determinism = Result(
            "determinism", Verdict.FAIL,
            "generated forms/build/batch evidence differs between runs "
            f"({str(first_digest)[:12]} vs {str(second_digest)[:12]})")
        return FullRefresh(
            determinism,
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "nondeterministic generation; audit not run"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "nondeterministic generation; referee not run"),
            diagnostics, None,
        )
    determinism = Result(
        "determinism", Verdict.PASS,
        f"byte-identical ({first_digest[:12]})")
    try:
        _atomic_write(batch_target, batch_payloads[1])
        canonical_batch, _payload = _fresh_batch_report(batch_target, slugs)
        published_generation = generation_reader(canonical_batch)
    except Exception as error:  # noqa: BLE001
        return FullRefresh(
            Result("determinism", Verdict.UNEVALUABLE,
                   f"could not publish/bind final batch report: {error}"),
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "final batch report is not bound; audit not run"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "final batch report is not bound; referee not run"),
            diagnostics, None,
        )
    if published_generation != second_generation:
        return FullRefresh(
            Result("determinism", Verdict.FAIL,
                   "published batch report changed deterministic scope"),
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "published scope mismatch; audit not run"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "published scope mismatch; referee not run"),
            diagnostics, None,
        )

    try:
        audit_refresh = audit_refresher()
    except Exception as error:  # noqa: BLE001 - failed child is not evidence
        audit_refresh = Result(
            "audit-refresh", Verdict.UNEVALUABLE,
            f"full audit refresh raised: {type(error).__name__}: {error}")
    if not audit_refresh.verdict.ok:
        return FullRefresh(
            determinism,
            audit_refresh,
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "fresh final-corpus audit failed; referee not run"),
            diagnostics, None,
        )
    try:
        post_audit_generation = generation_reader(canonical_batch)
    except Exception as error:  # noqa: BLE001
        return FullRefresh(
            Result("determinism", Verdict.UNEVALUABLE,
                   f"cannot revalidate generated scope after audit: {error}"),
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "post-audit generated scope could not be attested"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "post-audit scope is unknown; referee not run"),
            diagnostics, None,
        )
    if post_audit_generation != second_generation:
        return FullRefresh(
            Result("determinism", Verdict.FAIL,
                   "generated scope changed during final audit"),
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   "audit mutated deterministic generated bytes"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "audit mutated generated bytes; referee not run"),
            diagnostics, None,
        )
    try:
        final_scope = compose_final_referee_scope(
            second_generation, audit_identity_reader())
    except Exception as error:  # noqa: BLE001
        return FullRefresh(
            determinism,
            Result("audit-refresh", Verdict.UNEVALUABLE,
                   f"cannot bind final audit bytes: {error}"),
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "final audit bytes are unbound; referee not run"),
            diagnostics, None,
        )
    if referee_refresher is None:
        return FullRefresh(
            determinism, audit_refresh,
            Result("comb-referee", Verdict.UNEVALUABLE,
                   "referee deferred until all other gate checks finish"),
            diagnostics, final_scope,
        )
    try:
        comb_referee = referee_refresher()
    except Exception as error:  # noqa: BLE001 - failed child is not evidence
        comb_referee = Result(
            "comb-referee", Verdict.UNEVALUABLE,
            f"referee refresh raised: {type(error).__name__}: {error}")
    return FullRefresh(
        determinism, audit_refresh, comb_referee, diagnostics,
        final_scope)


def refresh_final_comb_referee(
        expected_scope: dict[str, Any] | None,
        *, referee_refresher: Callable[[], Result] = (
            refresh_comb_referee_report),
        generation_reader: Callable[[dict[str, Any]], dict[str, Any]] = (
            generated_scope_manifest),
        batch_target: pathlib.Path = BATCH_REPORT,
        expected_slugs: frozenset[str] | None = None,
        audit_identity_reader: Callable[[], dict[str, Any]] = (
            current_audit_identity),
        ) -> Result:
    """Last executable gate step: rebind current bytes, then run the referee."""
    if expected_scope is None:
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            "no deterministic post-audit scope exists; referee not run",
        )
    try:
        slugs = expected_slugs or canonical_form_slugs()
        batch_record, _payload = _fresh_batch_report(batch_target, slugs)
        current_generation = generation_reader(batch_record)
        current_scope = compose_final_referee_scope(
            current_generation, audit_identity_reader())
    except Exception as error:  # noqa: BLE001 - currentness is mandatory
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            f"cannot revalidate final generated scope: {error}",
        )
    if current_scope != expected_scope:
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            "generated scope changed after audit/other checks; referee not run",
        )
    try:
        return referee_refresher()
    except Exception as error:  # noqa: BLE001
        return Result(
            "comb-referee", Verdict.UNEVALUABLE,
            f"referee refresh raised: {type(error).__name__}: {error}",
        )


# ---------------------------------------------------------------------------
# checks
# ---------------------------------------------------------------------------


def check_self_tests() -> Result:
    failures, missing = [], []
    for module in SELF_TEST_MODULES:
        path = HERE / f"{module}.py"
        if not path.is_file():
            missing.append(module)
            continue
        args = [str(path), "--self-test"]
        # Two modules key their self-test off a form's IR rather than shipping a
        # fixture, so the gate supplies one instead of skipping them.
        if module in ("lattice", "fonts"):
            ir = BUILD / "ir" / "2551q-2018.ir.json"
            if not ir.is_file():
                missing.append(f"{module} (no IR to test against)")
                continue
            args += ["--ir", str(ir)]
        code, _ = run(args, timeout=900)
        if code != 0:
            failures.append(module)
    if missing:
        return Result("self-tests", Verdict.UNEVALUABLE,
                      f"cannot run: {', '.join(missing)}")
    if failures:
        return Result("self-tests", Verdict.FAIL, f"failing: {', '.join(failures)}")
    return Result("self-tests", Verdict.PASS,
                  f"{len(SELF_TEST_MODULES)} modules pass")


def check_conversion() -> Result:
    report = load(BATCH_REPORT)
    try:
        expected_slugs = canonical_form_slugs()
    except Exception as error:  # noqa: BLE001 - exact corpus is mandatory
        return Result("conversion", Verdict.UNEVALUABLE,
                      f"cannot resolve tracked corpus: {error}")
    errors = batch_report_errors(report, expected_slugs)
    if errors:
        return Result("conversion", Verdict.FAIL, "; ".join(errors[:5]))
    return Result(
        "conversion", Verdict.PASS,
        f"{len(expected_slugs)}/{EXPECTED_FORMS} unique tracked forms converted",
    )


def audit_records() -> list[dict] | None:
    data = load(AUDIT_JSON)
    if not isinstance(data, list):
        return None
    return [r for r in data if r.get("status") == "ok"]


def refresh_assertions_report(
    target: pathlib.Path = AUDIT_JSON,
    scratch_root: pathlib.Path = BUILD,
    runner: Callable[[list[str], int], tuple[int, str]] = run,
) -> Result | None:
    """Atomically refresh the assertion audit, or return a fail-closed result."""
    scratch_root.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".assertions-", dir=scratch_root) as tmp:
        fresh = pathlib.Path(tmp) / "audit.json"
        code, out = runner(
            [str(HERE / "audit.py"), "--assertions-only", "--out", str(fresh)],
            5400,
        )
        if code != 0:
            tail = out.strip().splitlines()[-1:] or ["no diagnostic"]
            return Result("assertions", Verdict.UNEVALUABLE,
                          f"assertion audit refresh failed: {tail[0]}")
        if not isinstance(load(fresh), list):
            return Result("assertions", Verdict.UNEVALUABLE,
                          "assertion audit refresh produced no usable report")
        try:
            fresh.replace(target)
        except OSError as error:
            return Result("assertions", Verdict.UNEVALUABLE,
                          f"could not publish refreshed assertion audit: {error}")
    return None


def _tally(name: str, keys: Iterable[str], pct_key: str | None = None) -> Result:
    records = audit_records()
    if records is None:
        return Result(name, Verdict.UNEVALUABLE, "no audit report")
    if len(records) != EXPECTED_FORMS:
        return Result(name, Verdict.FAIL,
                      f"audit covers {len(records)}/{EXPECTED_FORMS} forms")

    # A form whose round trip hard-failed reports every total as 0, because the
    # differ never walked its pages. Counting those zeros as "clean" is how this
    # gate came to report `rules clean on 51/51` while five forms had not been
    # measured at all -- the same disease the gate exists to cure, in the gate.
    unmeasured = [r["slug"] for r in records if r.get("measured") is False]
    if unmeasured:
        return Result(name, Verdict.UNEVALUABLE,
                      f"{len(unmeasured)} form(s) not measured "
                      f"({', '.join(unmeasured[:5])}); their totals are zeros "
                      f"from a hard failure, not results")

    bad: list[str] = []
    for key in keys:
        offenders = [r for r in records if r.get(key)]
        if offenders:
            total = sum(r.get(key, 0) for r in offenders)
            bad.append(f"{key}={total} on {len(offenders)} form(s) "
                       f"(e.g. {offenders[0]['slug']})")
    if pct_key:
        short = [r for r in records if r.get(pct_key) != 100.0]
        if short:
            worst = min(short, key=lambda r: r.get(pct_key) or 0)
            bad.append(f"{pct_key} below 100 on {len(short)} form(s), "
                       f"worst {worst['slug']} {worst.get(pct_key)}%")
    if bad:
        return Result(name, Verdict.FAIL, "; ".join(bad))
    return Result(name, Verdict.PASS, f"clean on {len(records)}/{EXPECTED_FORMS}")


def check_rules() -> Result:
    return _tally("rules", ("rules_missing", "rules_extra",
                            "rules_thickness_violations"), "rules_pct")


def check_paper() -> Result:
    records = audit_records()
    if records is None:
        return Result("paper", Verdict.UNEVALUABLE, "no audit report")
    bad = [r["slug"] for r in records if not r.get("paper_ok")]
    if bad:
        return Result("paper", Verdict.FAIL, f"{len(bad)} form(s): {', '.join(bad[:5])}")
    return Result("paper", Verdict.PASS, f"exact on {len(records)}/{EXPECTED_FORMS}")


def check_artwork() -> Result:
    return _tally("artwork", ("images_missing", "images_placement_violations"))


def check_text() -> Result:
    return _tally("text", ("text_missing", "text_extra"), "text_pct")


def check_assertions() -> Result:
    """The eight assertions that would have caught the 137 audit-blind defects.

    audit.py owns them. Until each publishes a boolean per form, this reports
    UNEVALUABLE, which the gate counts as a failure -- the whole point of the
    exercise is that an unchecked claim must not read as a satisfied one.
    """
    records = audit_records()
    if records is None:
        return Result("assertions", Verdict.UNEVALUABLE, "no audit report")
    if len(records) != EXPECTED_FORMS:
        return Result("assertions", Verdict.FAIL,
                      f"audit covers {len(records)}/{EXPECTED_FORMS} forms")
    absent = [k for k in REQUIRED_ASSERTIONS if not any(k in r for r in records)]
    if absent:
        return Result("assertions", Verdict.UNEVALUABLE,
                      f"{len(absent)}/{len(REQUIRED_ASSERTIONS)} not implemented in "
                      f"audit.py: {', '.join(sorted(absent))}")
    violations: list[str] = []
    for key, description in REQUIRED_ASSERTIONS.items():
        offenders = [r["slug"] for r in records if r.get(key) is not True]
        if offenders:
            violations.append(f"{key} fails on {len(offenders)} form(s) "
                              f"({description})")
    if violations:
        return Result("assertions", Verdict.FAIL, "; ".join(violations))
    return Result("assertions", Verdict.PASS,
                  f"all {len(REQUIRED_ASSERTIONS)} hold on {len(records)} forms")


def check_findings() -> Result:
    data = load(FINDINGS)
    if not isinstance(data, dict) or "findings" not in data:
        return Result("findings", Verdict.UNEVALUABLE, "no findings ledger")
    gating = [f for f in data["findings"] if f.get("severity") in ("blocker", "major")]
    unresolved = [f for f in gating
                  if f.get("status") not in ("fixed", "not-a-defect")
                  or not (f.get("resolution") or "").strip()]
    if unresolved:
        by_form: dict[str, int] = {}
        for f in unresolved:
            by_form[f["form"]] = by_form.get(f["form"], 0) + 1
        worst = sorted(by_form.items(), key=lambda kv: -kv[1])[:4]
        return Result("findings", Verdict.FAIL,
                      f"{len(unresolved)}/{len(gating)} blocker+major unresolved "
                      f"(worst: {', '.join(f'{k} {v}' for k, v in worst)})")
    return Result("findings", Verdict.PASS,
                  f"all {len(gating)} blocker+major resolved")


def check_determinism(regenerate: bool) -> Result:
    del regenerate
    return Result(
        "determinism", Verdict.UNEVALUABLE,
        "needs the full two-generation pipeline; --only/--skip cannot evaluate it",
    )


def check_no_tracked_deletions() -> Result:
    proc = subprocess.run(["git", "status", "--porcelain", "--", "forms/"],
                          cwd=REPO, capture_output=True, text=True)
    if proc.returncode != 0:
        return Result("tracked-files", Verdict.UNEVALUABLE, "git status failed")
    deleted = [line[3:] for line in proc.stdout.splitlines()
               if line[:2].strip() in ("D", "AD") or line.startswith(" D")]
    if deleted:
        return Result("tracked-files", Verdict.FAIL,
                      f"{len(deleted)} tracked file(s) deleted: {', '.join(deleted[:3])}")
    return Result("tracked-files", Verdict.PASS, "no tracked deletion")


CHECKS: dict[str, Callable[[], Result]] = {
    "self-tests": check_self_tests,
    "conversion": check_conversion,
    "rules": check_rules,
    "paper": check_paper,
    "artwork": check_artwork,
    "text": check_text,
    "assertions": check_assertions,
    "comb-referee": check_comb_referee,
    "findings": check_findings,
    "tracked-files": check_no_tracked_deletions,
}


def _resign_for_self_test(value: dict[str, Any]) -> None:
    value.pop("payload_sha256", None)
    value.pop("self_digest", None)
    attach_self_digest(value)


def _producer_raw_referee_attestation_fixture() -> dict[str, Any]:
    return {
        "schema": "comb-referee-runtime-attestation-v1",
        "producer_and_declared_dependency_bytes_bound": True,
        "published_form_input_bytes_bound_before_after": True,
        "python_executable_fingerprinted": True,
        "python_executable_validated_before_after": False,
        "poppler_executable_bound_before_after": True,
        "poppler_invocations_have_hard_deadlines": True,
        "poppler_timeout_cleanup_policy": "kill-isolated-process-group",
        "clean_source_revision_bound": False,
        "python_stdlib_closure_bound": False,
        "python_dynamic_libraries_bound": False,
        "poppler_dynamic_libraries_bound": False,
        "operating_system_and_host_services_bound": False,
        "scope_complete": False,
        "complete": False,
        "enforceable": False,
        "incomplete_reasons": list(RAW_REFEREE_INCOMPLETE_REASONS),
        "future_gate_required": RAW_REFEREE_FUTURE_GATE,
    }


def _synthetic_comb_fixture(
        ) -> tuple[dict[str, Any], dict[str, Any], bytes, dict[str, Any]]:
    """Small, entirely in-memory v2 fixture for gate adversarial tests."""
    producer_records: dict[str, Any] = {}
    for relative in COMB_REFEREE_PRODUCERS:
        payload = relative.encode("utf-8")
        digest = sha256_bytes(payload)
        producer_records[relative] = {
            "path": relative, "bytes": len(payload), "sha256": digest,
            "head_sha256": digest, "equals_head": True,
        }
    python_path = str(pathlib.Path(sys.executable).resolve())
    python_digest = sha256_bytes(b"python")
    poppler_digest = sha256_bytes(b"pdftocairo")
    artifact_payloads = {
        "ir": b"ir", "layout": b"layout", "html": b"html",
        "guides": b"guide", "guide_html": b"guide-html",
    }
    artifact_records = {
        "ir": {"path": "build/ir/fixture-1.ir.json", "bytes": 2,
               "sha256": sha256_bytes(artifact_payloads["ir"])},
        "layout": {"path": "build/layout/fixture-1.layout.json", "bytes": 6,
                   "sha256": sha256_bytes(artifact_payloads["layout"])},
        "html": {"path": "build/html/fixture-1.html", "bytes": 4,
                 "sha256": sha256_bytes(artifact_payloads["html"])},
        "guides": {"path": "build/guides/fixture-1.guide.json", "bytes": 5,
                   "sha256": sha256_bytes(artifact_payloads["guides"])},
        "guide_html": {
            "path": "build/html/fixture-1.guide.html", "bytes": 10,
            "sha256": sha256_bytes(artifact_payloads["guide_html"]),
        },
    }
    provenance_payload = b"provenance"
    provenance_digest = sha256_bytes(provenance_payload)
    provenance_record = {
        "path": "forms/fixture-1/provenance.json",
        "bytes": len(provenance_payload),
        "sha256": provenance_digest,
        "head_sha256": provenance_digest,
        "equals_head": True,
    }
    source_payload = b"%PDF-fixture"
    source_digest = sha256_bytes(source_payload)
    source_candidate = {
        "path": "fixture.pdf", "bytes": len(source_payload),
        "sha256": source_digest,
    }
    layout_pin = {
        "file": "external:fixture.pdf", "sha256": source_digest,
        "bytes": len(source_payload), "page_count": 1,
    }
    source_relation = {
        "slug": "fixture-1",
        "declared_file": "fixture.pdf",
        "declared_sha256": source_digest,
        "declared_bytes": len(source_payload),
        "layout_pin": layout_pin,
        "candidate_count": 1,
        "matching_count": 1,
        "selected": "fixture.pdf",
        "candidates": [source_candidate],
    }
    fixture_subject = {
        "subject_key": "p1@0,0,10,10",
        "legacy_cell_id": "p1c1",
        "legacy_bbox": [0.0, 0.0, 10.0, 10.0],
        "cell_id": "p1c1",
        "mapped_partition_cell_ids": ["p1c1"],
        "state": "active_resolved",
        "blocks_gate": False,
        "reason_codes": [],
        "cells": 2,
    }
    fixture_comb = {
        "cells": 2,
        "divider_count": 1,
        "divider_x": [5.0],
        "slot_x": [0.0, 5.0, 10.0],
        "y0": 0.0,
        "y1": 10.0,
        "pitch_pt": 5.0,
        "resolution": {"status": "resolved", "reason_codes": []},
    }
    fixture_topology = _project_layout_topology(
        fixture_comb, [0.0, 0.0, 10.0, 10.0], "fixture topology")
    fixture_emission_geometry = {
        "page_index": 1,
        "left": 0.0,
        "top": 0.0,
        "width": 10.0,
        "height": 10.0,
        "slots": [
            {
                "index": 0, "left": 0.0, "top": 0.0,
                "width": 5.0, "height": 10.0,
            },
            {
                "index": 1, "left": 5.0, "top": 0.0,
                "width": 5.0, "height": 10.0,
            },
        ],
    }
    fixture_emitted_evidence = {
        "count": 2,
        "indexes": [0, 1],
        "editable_indexes": [0, 1],
        "declared_capacity": 2,
        "declared_count": 2,
        "page_index": 1,
        "container_position": [0.0, 0.0],
        "container_geometry": [10.0, 10.0],
        "layout_binding_valid": True,
        "expected_geometry": fixture_emission_geometry,
        "slot_geometry": fixture_emission_geometry["slots"],
        "valid": True,
    }
    fixture_lattice_evidence = {
        "file": "tools/formgen/lattice.py",
        "bytes": producer_records["tools/formgen/lattice.py"]["bytes"],
        "sha256": producer_records["tools/formgen/lattice.py"]["sha256"],
        "expected_sha256": producer_records[
            "tools/formgen/lattice.py"]["sha256"],
        "layout_generator": {"fixture": True},
    }
    assertion_relation = {
        "combs_expected": 1,
        "combs_checked": 1,
        "expected_comb_ids": ["p1c1"],
        "checked_comb_ids": ["p1c1"],
        "emitted_comb_ids": ["p1c1"],
        "unexpected_emitted_comb_ids": [],
        "duplicate_layout_comb_ids": [],
        "duplicate_emitted_cell_ids": [],
        "raw_live_comb_issues": 0,
        "emitted_cell_binding_issues": 0,
        "inventory_complete": True,
        "layout_mismatches": 0,
        "layout_unevaluable": 0,
        "owner_certificates_valid": 1,
        "owner_certificates_invalid": 0,
        "source_u_frame_evaluable": 0,
        "source_certified_unframed_evaluable": 1,
        "emission_behind_layout": 0,
        "emission_invalid": 0,
        "offender_count": 0,
        "offenders_published": 0,
        "offenders_omitted": 0,
        "offender_dimensions": {},
        "holds": True,
    }
    audit_inputs = {
        "ir": {
            "file": "fixture-1.ir.json", "required": True, "present": True,
            "bytes": artifact_records["ir"]["bytes"],
            "sha256": artifact_records["ir"]["sha256"],
        },
        "layout": {
            "file": "fixture-1.layout.json", "required": True, "present": True,
            "bytes": artifact_records["layout"]["bytes"],
            "sha256": artifact_records["layout"]["sha256"],
        },
        "html": {
            "file": "fixture-1.html", "required": True, "present": True,
            "bytes": artifact_records["html"]["bytes"],
            "sha256": artifact_records["html"]["sha256"],
        },
        "guide": {
            "file": "fixture-1.guide.json", "required": True, "present": True,
            "bytes": artifact_records["guides"]["bytes"],
            "sha256": artifact_records["guides"]["sha256"],
        },
        "guide_html": {
            "file": "fixture-1.guide.html", "required": False, "present": True,
            "bytes": artifact_records["guide_html"]["bytes"],
            "sha256": artifact_records["guide_html"]["sha256"],
        },
        "source_pdf": {
            "file": "fixture.pdf",
            "logical_identity": "external:fixture.pdf",
            "path": "fixture.pdf",
            "required": True,
            "present": True,
            "bytes": len(source_payload),
            "sha256": source_digest,
            "expected_sha256": source_digest,
        },
    }
    audit_render = {
        "entrypoint": "fixture-1.html", "dependencies": [],
        "errors": [], "complete": True,
        "network_policy": "deny-except-retained-relative-resources-and-inline-data",
    }
    audit_form_relation = {
        "record_sha256": sha256_bytes(b"audit-record"),
        "input_manifest_sha256": sha256_bytes(b"audit-manifest"),
        "inputs": audit_inputs,
        "render": audit_render,
        "assertion_sha256": sha256_bytes(b"audit-assertion"),
        "assertion_relation": assertion_relation,
        "top_level_holds": True,
    }
    synthetic_audit_forms = {"fixture-1": audit_form_relation}
    synthetic_audit_forms.update({
        f"unused-{index}": {"record_sha256": sha256_bytes(
            f"unused-{index}".encode("utf-8"))}
        for index in range(2, EXPECTED_FORMS + 1)
    })
    audit_digest = sha256_bytes(b"audit")
    html_files = [artifact_records["html"], artifact_records["guide_html"]]
    snapshot: dict[str, Any] = {
        "git": {
            "commit": "1" * 40,
            "tree": "2" * 40,
            "worktree_clean": True,
        },
        "producers": producer_records,
        "runtime": {
            "python": {
                "path": python_path, "bytes": 6,
                "sha256": python_digest,
            },
            "pdftocairo": {
                "path": "/trusted/pdftocairo", "bytes": 10,
                "sha256": poppler_digest,
            },
        },
        "audit": {
            "path": "build/audit.json", "bytes": 5, "sha256": audit_digest,
            "form_count": EXPECTED_FORMS,
            "forms_sha256": canonical_digest(synthetic_audit_forms),
            "forms": synthetic_audit_forms,
            "application_attestation": {
                "path": "build/audit-attested.json",
                "bytes": 10,
                "sha256": sha256_bytes(b"audit-app"),
            },
            "application_scope_attested": True,
        },
        "artifact_trees": {
            "ir": {**_file_manifest([artifact_records["ir"]]), "root": "build/ir"},
            "layout": {
                **_file_manifest([artifact_records["layout"]]),
                "root": "build/layout",
            },
            "html": {**_file_manifest(html_files), "root": "build/html"},
            "guides": {
                **_file_manifest([artifact_records["guides"]]),
                "root": "build/guides",
            },
        },
        "layout_bindings": {
            "fixture-1": {
                "layout_sha256": artifact_records["layout"]["sha256"],
                "guide_sha256": artifact_records["guides"]["sha256"],
                "lattice_evidence": fixture_lattice_evidence,
                "audit_expected_ids": ["p1c1"],
                "cells": {
                    "p1c1": {
                        "cell": "p1c1",
                        "subject_key": fixture_subject["subject_key"],
                        "legacy_cell_id": "p1c1",
                        "cell_id": "p1c1",
                        "ledger_state": "active_resolved",
                        "ledger_blocks_gate": False,
                        "ledger_reason_codes": [],
                        "ledger_topology_sha256": fixture_topology["sha256"],
                        "ledger_evidence": fixture_subject,
                        "page": 1,
                        "bbox": [0.0, 0.0, 10.0, 10.0],
                        "latticed": 2,
                        "lattice_divider_x": [5.0],
                        "expected_emission_geometry": (
                            fixture_emission_geometry),
                    },
                },
                "inferences": {},
            },
        },
        "provenance": _file_manifest([provenance_record]),
        "source_pdfs": {
            "relation_count": 1, "candidate_file_count": 1,
            "sha256": canonical_digest([source_relation]),
            "relations": [source_relation],
        },
    }
    fixture_manifest_reason = "; ".join(
        f"blocked: {blocker}" for blocker in RAW_AUDIT_SCOPE_BLOCKERS)
    fixture_manifest_binding = {
        "binding_valid": True,
        "manifest_inputs_complete": True,
        "attestation_complete": False,
        "enforceable": False,
        "complete": False,
        "reason": fixture_manifest_reason,
        "errors": [],
        "blockers": list(RAW_AUDIT_SCOPE_BLOCKERS),
        "producer_sha256": producer_records[
            "tools/formgen/audit.py"]["sha256"],
        "runtime_tree_sha256": sha256_bytes(b"audit-runtime-tree"),
        "runtime_manifest_self_consistent": True,
        "base_runtime_closure_independently_attested": False,
        "roundtrip_runtime_closure_independently_attested": False,
        "render_dependency_count": 0,
        "render_dependencies": [],
        "roundtrip_present": True,
    }
    fixture_ledger_binding = {
        "binding_valid": True,
        "reason": "complete",
        "errors": [],
        "active_subject_ids": ["p1c1"],
        "emitted_ids": ["p1c1"],
        "legacy_alias_count": 1,
    }
    comparisons = {name: 0 for name in COMPARISON_NAMES}
    comparisons["unevaluable"] = 1
    cell = {
        "cell": "p1c1",
        "subject_key": "p1@0,0,10,10",
        "legacy_cell_id": "p1c1",
        "cell_id": "p1c1",
        "ledger_state": "active_resolved",
        "ledger_blocks_gate": False,
        "ledger_reason_codes": [],
        "ledger_topology_sha256": fixture_topology["sha256"],
        "ledger_evidence": fixture_subject,
        "page": 1,
        "bbox": [0.0, 0.0, 10.0, 10.0],
        "latticed": 2,
        "lattice_divider_x": [5.0],
        "emitted": 2,
        "emitted_indexes_valid": True,
        "emitted_evidence": fixture_emitted_evidence,
        "audit_printed": None,
        "audit_relation": "unknown-truncated",
        "comparison_reason": "audit evidence is incomplete",
        "comparison_status": "unevaluable",
        "transition_status": "none",
        "transition_reason": "active ledger subject is already resolved",
        "referee": {
            "status": "measured",
            "reason": (
                "one source topology contains every recognised anchor"),
            "y0": 0.0,
            "y1": 10.0,
            "source_divider_x": [5.0],
            "extra_divider_x": [],
            "compartments": 2,
            "anchor_matches": [{
                "layout_x": 5.0,
                "source_x": 5.0,
                "delta_pt": 0.0,
            }],
            "positions_match": True,
            "anchors_complete": True,
            "subject_gap_proofs": [],
            "unproven_subject_gaps": [],
            "components": [{
                "x": 5.0,
                "x0": 4.9,
                "x1": 5.1,
                "tone": 0.0,
                "elements": ["fixture-divider"],
                "clipped": False,
            }],
            "contract_y0": 0.0,
            "contract_y1": 10.0,
            "open_y0": 0.0,
            "open_y1": 10.0,
            "contract_span_pt": 10.0,
            "seed_span_pt": 10.0,
            "measured_span_pt": 10.0,
            "unmeasured_span_pt": 0.0,
            "topology_coverage_pt": {"5.0": 10.0},
            "ignored_slabs": [],
            "chosen_topology": [5.0],
            "topology_superset_relations": [],
        },
        "four_way": {
            "referee": 2, "lattice": 2, "audit": None, "emitted": 2,
        },
    }
    form_counts = {
        "combs": 1,
        "subjects": 1,
        "subjects_active": 1,
        "subjects_active_resolved": 1,
        "subjects_active_unresolved": 0,
        "subjects_retained_unresolved": 0,
        "inferences_suppressed": 0,
        "ledger_blocking": 0,
        "measured": 1,
        "source_unevaluable": 0,
        "unevaluable": 1,
        "referee_layout_mismatches": 0,
        "referee_layout_position_mismatches": 0,
        "emission_layout_mismatches": 0,
        "comparisons": comparisons,
    }
    fixture_poppler = {
        "version": "fixture-poppler",
        "binary_path": "/trusted/pdftocairo",
        "binary_sha256": poppler_digest,
        "identity_timeout_seconds": 10.0,
        "page_timeout_seconds": 60.0,
        "subprocess_cleanup_policy": "kill-isolated-process-group",
    }
    form = {
        "slug": "fixture-1",
        "status": "unevaluable",
        "reason": (
            f"audit evidence incomplete: {fixture_manifest_reason}, "
            "1 combs unevaluable"),
        "source": {
            "file": "fixture.pdf",
            "sha256": source_digest,
            "bytes": len(source_payload),
            "page_count": 1,
            "layout_pin": layout_pin,
        },
        "artifacts": {
            "ir_sha256": artifact_records["ir"]["sha256"],
            "layout_sha256": artifact_records["layout"]["sha256"],
            "html_sha256": artifact_records["html"]["sha256"],
            "html_structure_sha256": artifact_records["html"]["sha256"],
            "guide_sha256": artifact_records["guides"]["sha256"],
            "guide_html_sha256": artifact_records["guide_html"]["sha256"],
            "tracked_provenance_file": provenance_record["path"],
            "tracked_provenance_sha256": provenance_digest,
        },
        "lattice_evidence": fixture_lattice_evidence,
        "poppler": fixture_poppler,
        "pages": [{
            "page": 1,
            "svg_sha256": sha256_bytes(b"fixture-svg"),
            "vector_paints": 1,
            "unsupported_regions": 0,
        }],
        "audit_evidence": {
            **assertion_relation,
            "complete": False,
            "reason": fixture_manifest_reason,
            "errors": [],
            "assertion_valid": True,
            "input_manifest_verified": True,
            "input_manifest_reason": fixture_manifest_reason,
            "evidence_published": True,
            "byte_and_relation_binding_valid": True,
            "manifest_binding": fixture_manifest_binding,
            "ledger_binding": fixture_ledger_binding,
            "runtime_closure_independently_attested": False,
            "integrity_valid": False,
        },
        "emission_inventory": {
            "complete": True,
            "reason": "complete",
            "expected_active_cell_ids": ["p1c1"],
            "emitted_cell_ids": ["p1c1"],
            "missing_active_cell_ids": [],
            "unexpected_emitted_cell_ids": [],
            "retained_emitted_cell_ids": [],
            "inference_emitted_cell_ids": [],
            "invalid_active_cell_ids": [],
        },
        "emission_binding_errors": [],
        "counts": form_counts,
        "inferences": [],
        "cells": [cell],
    }
    audit_record = producer_records["tools/formgen/audit.py"]
    lattice_record = producer_records["tools/formgen/lattice.py"]
    referee_record = producer_records["tools/formgen/comb_referee.py"]
    report: dict[str, Any] = {
        "schema_version": COMB_REFEREE_REPORT_VERSION,
        "producer": "tools/formgen/comb_referee.py",
        "producer_sha256": referee_record["sha256"],
        "python_version": "fixture",
        "provenance": {
            "producer": {
                "file": "tools/formgen/comb_referee.py",
                "bytes": referee_record["bytes"],
                "sha256": referee_record["sha256"],
            },
            "dependencies": {
                "audit": {
                    "file": "tools/formgen/audit.py",
                    "bytes": audit_record["bytes"],
                    "sha256": audit_record["sha256"],
                    "expected_sha256": audit_record["sha256"],
                    "dependencies": [
                        {
                            "file": relative,
                            "bytes": producer_records[relative]["bytes"],
                            "sha256": producer_records[relative]["sha256"],
                            "expected_sha256": (
                                producer_records[relative]["sha256"]),
                        }
                        for relative in (
                            "tools/formgen/extract.py",
                            "tools/formgen/verify.py",
                        )
                    ],
                },
                "lattice": {
                    "file": "tools/formgen/lattice.py",
                    "bytes": lattice_record["bytes"],
                    "sha256": lattice_record["sha256"],
                    "expected_sha256": lattice_record["sha256"],
                },
            },
            "runtime": {
                "python_implementation": "cpython",
                "python_version": "fixture",
                "python_executable": python_path,
                "python_executable_sha256": python_digest,
                "poppler": fixture_poppler,
            },
        },
        "status": "unevaluable",
        "status_reasons": [
            "corpus coverage or one or more forms are unevaluable",
            "standalone referee runtime/application attestation is incomplete "
            "and non-enforceable",
        ],
        "attestation": _producer_raw_referee_attestation_fixture(),
        "poppler": fixture_poppler,
        "inputs": {
            "audit_sha256": audit_digest,
            "audit_bytes": 5,
            # report_binding_errors checks the production full-corpus binding.
            "layout_count": EXPECTED_FORMS,
        },
        "totals": {
            "forms_expected": 1,
            "forms_measured": 1,
            "forms_error": 0,
            "combs_expected": 1,
            "combs_found": 1,
            "combs_measured": 1,
            "combs_unevaluable": 1,
            "combs_source_unevaluable": 0,
            "subjects_active": 1,
            "subjects_active_resolved": 1,
            "subjects_active_unresolved": 0,
            "subjects_retained_unresolved": 0,
            "inferences_suppressed": 0,
            "ledger_blocking": 0,
            "referee_layout_mismatches": 0,
            "referee_layout_position_mismatches": 0,
            "comparisons": comparisons,
            "forms_ok": 0,
            "forms_disagreement": 0,
            "forms_unevaluable": 1,
            "audit_evidence_complete_forms": 0,
            "referee_attestation_complete": False,
            "referee_enforceable": False,
        },
        "errors": [],
        "forms": [form],
    }
    attach_self_digest(report)
    raw = (json.dumps(report, indent=2, sort_keys=True, ensure_ascii=False)
           + "\n").encode("utf-8")
    relations = {name: True for name in ENVELOPE_RELATIONS}
    envelope: dict[str, Any] = {
        "schema_version": COMB_REFEREE_ATTESTATION_VERSION,
        "application_scope_name": COMB_REFEREE_SCOPE,
        "application_snapshot": snapshot,
        "invocation": {
            "executable": sys.executable,
            "resolved_executable": python_path,
            "python_flags": list(ISOLATED_PYTHON_ATTESTED_FLAGS),
            "pythonpath_removed": True,
            "pythonhome_removed": True,
            "timeout_seconds": COMB_REFEREE_TIMEOUT_SECONDS,
            "total_timeout_seconds": COMB_REFEREE_TOTAL_TIMEOUT_SECONDS,
            "run_count": COMB_REFEREE_RUN_COUNT,
            "child_exits": [2, 2],
            "output": "private-temporary-output",
            "child_exit": 2,
        },
        "raw_report": {
            "file": "build/comb-referee.json",
            "bytes": len(raw),
            "sha256": sha256_bytes(raw),
            "payload_sha256": report["payload_sha256"],
            "schema_version": report["schema_version"],
            "status": report["status"],
            "repeat_sha256": [sha256_bytes(raw)] * COMB_REFEREE_RUN_COUNT,
        },
        "relations": relations,
        "host_tcb_required": True,
        "host_scope_complete": False,
        "host_closure_claimed": False,
        "operating_system_bound": False,
        "python_stdlib_bound": False,
        "dynamic_libraries_bound": False,
        "application_scope_complete": True,
        "enforceable": True,
        "enforcement_scope": "application-only",
    }
    attach_self_digest(envelope)
    return report, snapshot, raw, envelope


def _synthetic_batch_record(slug: str) -> dict[str, Any]:
    code, revision = slug.rsplit("-", 1)
    source_file = f"{slug}.pdf"
    source_digest = sha256_bytes(slug.encode("utf-8"))
    return {
        "slug": slug,
        "code": code.upper(),
        "revision": revision,
        "variant": "",
        "in_corpus": True,
        "source_file": source_file,
        "sha256": source_digest,
        "stage_failed": None,
        "error": None,
        "images_extracted": 0,
        "pages": 1,
        "paper": "612.0x792.0",
        "uniform_paper": True,
        "fonts": [],
        "rules": 0,
        "text_runs": 0,
        "images": 0,
        "cells": 0,
        "comb_cells": 0,
        "growables": [],
        "sources": [{
            "role": "form", "file": source_file, "sha256": source_digest,
        }],
        "guide": None,
        "guide_detected": {"inline_pages": [], "standalone_pdfs": []},
        "html_bytes": 1,
        "html": f"build/html/{slug}.html",
        "guide_build": {"plan": f"build/guides/{slug}.guide.json",
                        "html": None, "pdfs": []},
        "guide_source_irs": [],
        "font_plans": [f"build/fonts/{slug}.fontplan.json"],
    }


def _synthetic_audit_record(slug: str) -> dict[str, Any]:
    comb_assertion = {
        "holds": True,
        "reason": "",
        "offenders": [],
        "combs_expected": 0,
        "combs_checked": 0,
        "expected_comb_ids": [],
        "checked_comb_ids": [],
        "emitted_comb_ids": [],
        "unexpected_emitted_comb_ids": [],
        "duplicate_layout_comb_ids": [],
        "duplicate_emitted_cell_ids": [],
        "raw_live_comb_issues": 0,
        "emitted_cell_binding_issues": 0,
        "inventory_complete": True,
        "layout_mismatches": 0,
        "layout_unevaluable": 0,
        "owner_certificates_valid": 0,
        "owner_certificates_invalid": 0,
        "source_u_frame_evaluable": 0,
        "source_certified_unframed_evaluable": 0,
        "emission_behind_layout": 0,
        "emission_invalid": 0,
    }
    assertions = {
        key: (comb_assertion if key == "comb_slots_match_printed" else {
            "holds": True, "reason": "", "offenders": [],
        })
        for key in REQUIRED_ASSERTIONS
    }
    record: dict[str, Any] = {
        "slug": slug,
        "status": "ok",
        "error": None,
        "input_manifest": {
            "schema": 1,
            "algorithm": "sha256",
            "producer": {},
            "runtime": {},
            "inputs_complete": True,
            "attestation_complete": False,
            "enforceable": False,
            "complete": False,
            "missing_required": [],
            "inputs": {},
            "render": {
                "entrypoint": f"{slug}.html", "dependencies": [],
                "errors": [], "complete": True,
                "network_policy": (
                    "deny-except-retained-relative-resources-and-inline-data"),
            },
        },
        "provenance_validation": {
            "validated_before": True,
            "validated_after": True,
            "error": None,
        },
        "assertions": assertions,
        "assertions_held": len(REQUIRED_ASSERTIONS),
        "attestation": {
            "inputs_complete": True,
            "producer_execution_bound": False,
            "base_runtime_scope_complete": False,
            "roundtrip_runtime_scope_complete": False,
            "validated_before_after": True,
            "complete": False,
            "enforceable": False,
            "incomplete_reasons": ["synthetic host scope is incomplete"],
            "future_gate_required": "outer application wrapper",
        },
        "measured": True,
        "paper_ok": True,
    }
    record.update({key: True for key in REQUIRED_ASSERTIONS})
    return record


def self_test() -> int:
    """Prove the gate can fail, and that it treats absence as failure.

    A gate that cannot fail is worthless, and one that passes on a missing check
    is worse than none at all -- so both properties are asserted rather than
    assumed.
    """
    failures = []
    if Verdict.UNEVALUABLE.ok:
        failures.append("UNEVALUABLE must not count as ok")
    if not Verdict.PASS.ok:
        failures.append("PASS must count as ok")

    missing = check_assertions()
    if missing.verdict is Verdict.PASS:
        failures.append("assertions reported PASS while audit.py does not "
                        "implement them")

    probe = Result("probe", Verdict.UNEVALUABLE, "x")
    if summarise([probe]) == 0:
        failures.append("an UNEVALUABLE check must make the gate exit non-zero")
    if summarise([Result("probe", Verdict.PASS, "x")]) != 0:
        failures.append("an all-PASS run must exit 0")

    if len(REQUIRED_ASSERTIONS) != 8:
        failures.append(f"GOAL.md names 8 assertions, gate has "
                        f"{len(REQUIRED_ASSERTIONS)}")
    if "comb_referee" not in SELF_TEST_MODULES:
        failures.append("comb_referee.py must be included in module self-tests")

    try:
        import py_compile
        with tempfile.TemporaryDirectory(
                prefix=".gate-isolation-self-test-") as temporary:
            root = pathlib.Path(temporary)
            module_root = root / "module"
            shadow_root = root / "shadow"
            module_root.mkdir()
            shadow_root.mkdir()
            marker = root / "sitecustomize-ran"
            (shadow_root / "sitecustomize.py").write_text(
                "from pathlib import Path\n"
                f"Path({str(marker)!r}).write_text('forged')\n",
                encoding="utf-8")
            module = module_root / "cache_probe.py"
            module.write_text("VALUE = 'forged'\n", encoding="utf-8")
            py_compile.compile(str(module), doraise=True)
            compiled_stat = module.stat()
            # Same size and timestamp make the repository-style pyc look
            # current.  The isolated prefix must nevertheless load source.
            module.write_text("VALUE = 'source'\n", encoding="utf-8")
            os.utime(module, ns=(
                compiled_stat.st_atime_ns, compiled_stat.st_mtime_ns))
            probe = (
                "import json,sys;"
                f"sys.path.insert(0,{str(module_root)!r});"
                "import cache_probe;"
                "print(json.dumps({"
                "'value':cache_probe.VALUE,"
                "'isolated':sys.flags.isolated,"
                "'dont_write':sys.dont_write_bytecode,"
                "'pycache_prefix':bool(sys.pycache_prefix)}))"
            )
            hostile_environment = dict(os.environ)
            hostile_environment["PYTHONPATH"] = str(shadow_root)
            hostile_environment["PYTHONHOME"] = str(shadow_root)
            code, output = run_isolated_python(
                ["-c", probe], 30, hostile_environment)
            lines = [line for line in output.splitlines() if line.strip()]
            isolation = json.loads(lines[-1]) if code == 0 and lines else {}
            if (isolation != {
                    "value": "source",
                    "isolated": 1,
                    "dont_write": True,
                    "pycache_prefix": True,
                    } or marker.exists()):
                failures.append(
                    "isolated Python child must ignore inherited sitecustomize "
                    "and repository pyc")
    except Exception as error:  # noqa: BLE001 - self-test must report failure
        failures.append(
            "isolated Python child probe failed: "
            f"{type(error).__name__}: {error}")

    clone = lambda value: json.loads(json.dumps(value))  # noqa: E731
    report, snapshot, raw_payload, envelope = _synthetic_comb_fixture()
    report_errors, report_stats = validate_comb_referee_report(
        report, child_exit=2, expected_forms=1, expected_subjects=1)
    report_errors.extend(report_binding_errors(report, snapshot, report_stats))
    if report_errors or report_stats["pending_transitions"] != 0:
        failures.append(
            "a complete synthetic comb-referee report must validate: "
            + "; ".join(report_errors[:3]))

    # Real source bands can contain a shorter minority topology below a richer
    # strict-majority topology.  Every layout anchor belongs to the chosen
    # topology; a dominated alternative is intentionally only a proper subset.
    dominated_subset_cell = clone(report["forms"][0]["cells"][0])
    dominated_subset_cell.update({
        "latticed": 3,
        "lattice_divider_x": [3.0, 7.0],
    })
    dominated_subset_referee = clone(dominated_subset_cell["referee"])
    dominated_subset_referee.update({
        "reason": (
            "one richer source topology contains every other slab and "
            "occupies a strict majority of the comb band"),
        "y1": 6.0,
        "source_divider_x": [3.0, 7.0],
        "extra_divider_x": [],
        "compartments": 3,
        "anchor_matches": [
            {"layout_x": 3.0, "source_x": 3.0, "delta_pt": 0.0},
            {"layout_x": 7.0, "source_x": 7.0, "delta_pt": 0.0},
        ],
        "components": [
            {
                "x": 3.0, "x0": 2.9, "x1": 3.1, "tone": 0.0,
                "elements": ["fixture-left-divider"], "clipped": False,
            },
            {
                "x": 7.0, "x0": 6.9, "x1": 7.1, "tone": 0.0,
                "elements": ["fixture-right-divider"], "clipped": False,
            },
        ],
        "topology_coverage_pt": {
            "3.0": 4.0,
            "3.0,7.0": 6.0,
        },
        "chosen_topology": [3.0, 7.0],
        "topology_superset_relations": [
            {
                "candidate": [3.0],
                "other": [3.0, 7.0],
                "contains": False,
                "proper": False,
            },
            {
                "candidate": [3.0, 7.0],
                "other": [3.0],
                "contains": True,
                "proper": True,
            },
        ],
    })
    dominated_subset_cell["referee"] = dominated_subset_referee
    subset_errors = _measured_referee_certificate_errors(
        "real-subset-shape", dominated_subset_cell,
        dominated_subset_referee)
    if subset_errors:
        failures.append(
            "a dominated minority source topology must remain evaluable: "
            + "; ".join(subset_errors[:3]))
    forged_subset_relations = clone(dominated_subset_referee)
    forged_subset_relations["topology_superset_relations"][1][
        "proper"] = False
    if not _measured_referee_certificate_errors(
            "forged-subset-shape", dominated_subset_cell,
            forged_subset_relations):
        failures.append(
            "forged topology-superset evidence must still fail closed")
    envelope_errors = validate_comb_referee_envelope(
        envelope, raw_payload, report, snapshot)
    if envelope_errors:
        failures.append(
            "OS=false with explicit host TCB and enforceable application scope "
            "must validate: " + "; ".join(envelope_errors[:3]))
    if _comb_referee_outcome(
            report, report_stats, expected_forms=1,
            expected_subjects=1).verdict is not Verdict.PASS:
        failures.append(
            "outer application attestation must elevate a producer-shaped "
            "raw standalone-attestation UNEVALUABLE report")
    if snapshot_pair_errors(snapshot, snapshot):
        failures.append("an identical clean application snapshot must validate")

    dirty = clone(snapshot)
    dirty["git"]["worktree_clean"] = False
    if not snapshot_pair_errors(snapshot, dirty):
        failures.append("a dirty before/after snapshot must fail closed")
    wrong_revision = clone(snapshot)
    wrong_revision["git"]["commit"] = "3" * 40
    if not snapshot_pair_errors(snapshot, wrong_revision):
        failures.append("a wrong HEAD revision must fail closed")
    mutated_snapshot = clone(snapshot)
    mutated_snapshot["audit"]["sha256"] = "4" * 64
    if not snapshot_pair_errors(snapshot, mutated_snapshot):
        failures.append("an input mutation during the referee run must fail closed")

    if not validate_comb_referee_report(
            None, expected_forms=1, expected_subjects=1)[0]:
        failures.append("a missing comb-referee report must be UNEVALUABLE")
    digest_bad = clone(report)
    digest_bad["status_reasons"] = ["mutated after signing"]
    if not validate_comb_referee_report(
            digest_bad, child_exit=2, expected_forms=1,
            expected_subjects=1)[0]:
        failures.append("a digest-bad comb-referee report must be UNEVALUABLE")
    partial = clone(report)
    partial["forms"] = []
    _resign_for_self_test(partial)
    if not validate_comb_referee_report(
            partial, child_exit=2, expected_forms=1,
            expected_subjects=1)[0]:
        failures.append("a partial comb-referee report must be UNEVALUABLE")
    stale_current = clone(snapshot)
    stale_current["runtime"]["python"]["sha256"] = "5" * 64
    if not validate_comb_referee_envelope(
            envelope, raw_payload, report, stale_current):
        failures.append("a stale comb-referee envelope must be UNEVALUABLE")
    non_enforceable = clone(envelope)
    non_enforceable["enforceable"] = False
    _resign_for_self_test(non_enforceable)
    if not validate_comb_referee_envelope(
            non_enforceable, raw_payload, report, snapshot):
        failures.append("a non-enforceable application scope must be UNEVALUABLE")
    no_host_tcb = clone(envelope)
    no_host_tcb["host_tcb_required"] = False
    _resign_for_self_test(no_host_tcb)
    if not validate_comb_referee_envelope(
            no_host_tcb, raw_payload, report, snapshot):
        failures.append(
            "OS=false is valid only with an explicit required host TCB")
    incomplete_scope = clone(envelope)
    incomplete_scope["application_scope_complete"] = False
    _resign_for_self_test(incomplete_scope)
    if not validate_comb_referee_envelope(
            incomplete_scope, raw_payload, report, snapshot):
        failures.append("an incomplete application scope must be UNEVALUABLE")
    wrong_executable = clone(envelope)
    wrong_executable["invocation"]["executable"] = "/tmp/shadow-python"
    _resign_for_self_test(wrong_executable)
    if not validate_comb_referee_envelope(
            wrong_executable, raw_payload, report, snapshot):
        failures.append("a substituted invocation executable must fail closed")
    public_output = clone(envelope)
    public_output["invocation"]["output"] = "build/comb-referee.json"
    _resign_for_self_test(public_output)
    if not validate_comb_referee_envelope(
            public_output, raw_payload, report, snapshot):
        failures.append("a non-private child output contract must fail closed")
    invented_invocation = clone(envelope)
    invented_invocation["invocation"]["invented"] = True
    _resign_for_self_test(invented_invocation)
    if not validate_comb_referee_envelope(
            invented_invocation, raw_payload, report, snapshot):
        failures.append("an inexact invocation schema must fail closed")

    def mutation_errors(mutator: Callable[[dict[str, Any]], None]) -> list[str]:
        mutated = clone(report)
        mutator(mutated)
        _resign_for_self_test(mutated)
        return validate_comb_referee_report(
            mutated, child_exit=2, expected_forms=1,
            expected_subjects=1)[0]

    def bound_application_verdict(
            candidate: dict[str, Any], scope: dict[str, Any],
            ) -> Verdict:
        errors, stats = validate_comb_referee_report(
            candidate, child_exit=2, expected_forms=1,
            expected_subjects=1)
        errors.extend(report_binding_errors(candidate, scope, stats))
        if errors:
            return Verdict.UNEVALUABLE
        return _comb_referee_outcome(
            candidate, stats, expected_forms=1, expected_subjects=1).verdict

    def application_verdict(
            mutator: Callable[[dict[str, Any]], None]) -> Verdict:
        mutated = clone(report)
        mutator(mutated)
        _resign_for_self_test(mutated)
        return bound_application_verdict(mutated, snapshot)

    if not mutation_errors(
            lambda value: value["forms"][0]["counts"].update({"invented": 0})):
        failures.append("an inexact per-form count schema must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["counts"].update(
                {"subjects_active_resolved": 0})):
        failures.append("a false subject-state total must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["cells"][0].update(
                {"ledger_blocks_gate": True})):
        failures.append("a false ledger blocking relation must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["cells"][0].update(
                {"transition_reason": "invented"})):
        failures.append("a false transition status/reason must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["cells"][0].update({"emitted": 0})):
        failures.append("a false emission-mismatch total must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["cells"][0]["referee"].update(
                {"compartments": 3})):
        failures.append("a false referee-layout total must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0]["cells"][0]["bbox"].__setitem__(
                0, float("nan"))):
        failures.append("non-finite cell geometry must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["forms"][0].update({
                "status": "unevaluable", "reason": "fabricated"})):
        failures.append("a fabricated per-form status must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["totals"].update(
                {"audit_evidence_complete_forms": 1})):
        failures.append("a false audit-completeness aggregate must be UNEVALUABLE")
    if not mutation_errors(
            lambda value: value["totals"].update(
                {"referee_attestation_complete": True})):
        failures.append("a false raw-attestation aggregate must be UNEVALUABLE")
    raw_reason_substitution = lambda value: value["attestation"].update({
        "incomplete_reasons": ["fatal source ambiguity"]})
    raw_reason_addition = lambda value: value["attestation"].update({
        "incomplete_reasons": [
            *RAW_REFEREE_INCOMPLETE_REASONS, "unrelated fatal blocker"]})
    raw_future_substitution = lambda value: value["attestation"].update({
        "future_gate_required": "arbitrary nonempty text"})
    for label, mutator in (
            ("substituted raw attestation reason", raw_reason_substitution),
            ("extra raw attestation reason", raw_reason_addition),
            ("substituted future-gate reason", raw_future_substitution)):
        if application_verdict(mutator) is Verdict.PASS:
            failures.append(f"{label} must prevent exit-2 elevation")

    def rewrite_outer_audit_inventory(
            candidate: dict[str, Any], scope: dict[str, Any],
            ids: list[str],
            ) -> None:
        scope["layout_bindings"]["fixture-1"]["audit_expected_ids"] = ids
        outer = scope["audit"]["forms"]["fixture-1"]["assertion_relation"]
        outer.update({
            "combs_expected": len(ids),
            "combs_checked": len(ids),
            "expected_comb_ids": ids,
            "checked_comb_ids": ids,
            "emitted_comb_ids": ids,
            "owner_certificates_valid": len(ids),
            "owner_certificates_invalid": 0,
        })
        evidence = candidate["forms"][0]["audit_evidence"]
        for key, value in outer.items():
            evidence[key] = clone(value)
        scope["audit"]["forms_sha256"] = canonical_digest(
            scope["audit"]["forms"])
        _resign_for_self_test(candidate)

    audit_only_report = clone(report)
    audit_only_scope = clone(snapshot)
    rewrite_outer_audit_inventory(
        audit_only_report, audit_only_scope, ["p1c1", "p1c2"])
    if bound_application_verdict(
            audit_only_report, audit_only_scope) is Verdict.PASS:
        failures.append(
            "an audit-only extra owner must prevent exit-2 elevation")

    report_only_report = clone(report)
    report_only_scope = clone(snapshot)
    rewrite_outer_audit_inventory(
        report_only_report, report_only_scope, [])
    if bound_application_verdict(
            report_only_report, report_only_scope) is Verdict.PASS:
        failures.append(
            "an audit-missing/report-only owner must prevent exit-2 elevation")

    opaque_mutations: list[tuple[str, Callable[[dict[str, Any]], None]]] = [
        (
            "emission binding error",
            lambda value: value["forms"][0]["emission_binding_errors"].append(
                "fatal emission binding"),
        ),
        (
            "audit error",
            lambda value: value["forms"][0]["audit_evidence"]["errors"].append(
                "fatal audit"),
        ),
        (
            "manifest error",
            lambda value: value["forms"][0]["audit_evidence"][
                "manifest_binding"]["errors"].append("fatal manifest"),
        ),
        (
            "ledger error",
            lambda value: value["forms"][0]["audit_evidence"][
                "ledger_binding"]["errors"].append("fatal ledger"),
        ),
        (
            "lattice error",
            lambda value: value["forms"][0].update({
                "lattice_evidence": {
                    "complete": False, "errors": ["fatal lattice"]}}),
        ),
        (
            "Poppler error",
            lambda value: value["forms"][0]["poppler"].update({
                "error": "fatal Poppler"}),
        ),
        (
            "page error",
            lambda value: value["forms"][0]["pages"][0].update({
                "status": "error", "reason": "fatal page"}),
        ),
        (
            "measured-referee error",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "error": "fatal source"}),
        ),
    ]
    for label, mutator in opaque_mutations:
        if application_verdict(mutator) is Verdict.PASS:
            failures.append(f"opaque {label} must prevent exit-2 elevation")

    provenance_mutations: list[
        tuple[str, Callable[[dict[str, Any]], None]]
    ] = [
        (
            "extra dependency role",
            lambda value: value["provenance"]["dependencies"].update({
                "evil": {"file": "/tmp/evil.py"}}),
        ),
        (
            "extra audit child dependency",
            lambda value: value["provenance"]["dependencies"]["audit"][
                "dependencies"].append({
                    "file": "/tmp/evil.py", "bytes": 1,
                    "sha256": "e" * 64, "expected_sha256": "e" * 64,
                }),
        ),
        (
            "duplicate audit child dependency",
            lambda value: value["provenance"]["dependencies"]["audit"][
                "dependencies"].append(clone(
                    value["provenance"]["dependencies"]["audit"][
                        "dependencies"][0])),
        ),
        (
            "extra report input",
            lambda value: value["inputs"].update({"evil": True}),
        ),
        (
            "extra provenance field",
            lambda value: value["provenance"].update({"evil": True}),
        ),
        (
            "extra runtime field",
            lambda value: value["provenance"]["runtime"].update({
                "evil": True}),
        ),
    ]
    for label, mutator in provenance_mutations:
        if not mutation_errors(mutator):
            failures.append(f"{label} must fail the exact provenance closure")

    false_report_source_partition = mutation_errors(
        lambda value: value["forms"][0]["audit_evidence"].update({
            "source_u_frame_evaluable": 1,
            "source_certified_unframed_evaluable": 1,
        }))
    if not any("source frame/unframed partition" in error
               for error in false_report_source_partition):
        failures.append(
            "a false published source frame/unframed partition must fail")

    measured_certificate_mutations: list[
        tuple[str, Callable[[dict[str, Any]], None]]
    ] = [
        (
            "false source-position boolean",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "positions_match": False}),
        ),
        (
            "source coordinates detached from the lattice",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "source_divider_x": [4.0],
                "chosen_topology": [4.0],
                "anchor_matches": [{
                    "layout_x": 5.0, "source_x": 4.0, "delta_pt": -1.0,
                }],
                "components": [{
                    "x": 4.0, "x0": 3.9, "x1": 4.1, "tone": 0.0,
                    "elements": ["forged-divider"], "clipped": False,
                }],
                "topology_coverage_pt": {"4.0": 10.0},
                "positions_match": True,
            }),
        ),
        (
            "unproven subject gap",
            lambda value: value["forms"][0]["cells"][0]["referee"][
                "unproven_subject_gaps"].append({"reason": "forged"}),
        ),
        (
            "zero measured span",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "measured_span_pt": 0.0}),
        ),
        (
            "negative contract span",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "contract_span_pt": -1.0}),
        ),
        (
            "non-finite measured reason",
            lambda value: value["forms"][0]["cells"][0]["referee"].update({
                "reason": float("nan")}),
        ),
        (
            "zero vector-paint source page",
            lambda value: value["forms"][0]["pages"][0].update({
                "vector_paints": 0}),
        ),
    ]
    for label, mutator in measured_certificate_mutations:
        if not mutation_errors(mutator):
            failures.append(f"{label} must invalidate measured source evidence")

    def layout_disagreement(value: dict[str, Any], *, global_total: int) -> None:
        cell = value["forms"][0]["cells"][0]
        cell["referee"]["compartments"] = 3
        cell["referee"]["source_divider_x"] = [5.0, 7.0]
        cell["referee"]["extra_divider_x"] = [7.0]
        cell["referee"]["chosen_topology"] = [5.0, 7.0]
        cell["referee"]["topology_coverage_pt"] = {"5.0,7.0": 10.0}
        cell["referee"]["components"].append({
            "x": 7.0,
            "x0": 6.9,
            "x1": 7.1,
            "tone": 0.0,
            "elements": ["fixture-extra-divider"],
            "clipped": False,
        })
        cell["four_way"]["referee"] = 3
        form = value["forms"][0]
        form["counts"]["referee_layout_mismatches"] = 1
        value["totals"]["referee_layout_mismatches"] = global_total

    missing_global = clone(report)
    layout_disagreement(missing_global, global_total=0)
    _resign_for_self_test(missing_global)
    missing_global_errors, _missing_stats = validate_comb_referee_report(
        missing_global, child_exit=2, expected_forms=1, expected_subjects=1)
    if not any("referee_layout_mismatches" in error
               for error in missing_global_errors):
        failures.append(
            "cell/form referee mismatch must derive into the global total")

    complete_disagreement = clone(report)
    layout_disagreement(complete_disagreement, global_total=1)
    _resign_for_self_test(complete_disagreement)
    complete_errors, complete_stats = validate_comb_referee_report(
        complete_disagreement, child_exit=2,
        expected_forms=1, expected_subjects=1)
    if (complete_errors or _comb_referee_outcome(
            complete_disagreement, complete_stats,
            expected_forms=1, expected_subjects=1).verdict is not Verdict.FAIL):
        failures.append(
            "a fully derived independent-referee disagreement must be FAIL")

    position_report = clone(report)
    position_cell = position_report["forms"][0]["cells"][0]
    position_cell["referee"]["positions_match"] = False
    position_cell["comparison_status"] = "stop"
    position_cell["comparison_reason"] = (
        "referee positions disagree with lattice anchors")
    position_form = position_report["forms"][0]
    position_form["counts"]["referee_layout_position_mismatches"] = 1
    position_form["counts"]["comparisons"]["agree"] = 0
    position_form["counts"]["comparisons"]["stop"] = 1
    position_form["status"] = "disagreement"
    position_form["reason"] = "one or more four-way comparisons disagree"
    position_report["totals"]["comparisons"]["agree"] = 0
    position_report["totals"]["comparisons"]["stop"] = 1
    position_report["totals"]["forms_ok"] = 0
    position_report["totals"]["forms_disagreement"] = 1
    position_report["status_reasons"] = [
        "one or more four-way form comparisons disagree",
        "standalone referee runtime/application attestation is incomplete "
        "and non-enforceable",
    ]
    _resign_for_self_test(position_report)
    position_errors, _position_stats = validate_comb_referee_report(
        position_report, child_exit=2,
        expected_forms=1, expected_subjects=1)
    if not any("referee_layout_position_mismatches" in error
               for error in position_errors):
        failures.append(
            "cell/form position mismatch must derive into the global total")

    def emission_disagreement(value: dict[str, Any]) -> None:
        cell = value["forms"][0]["cells"][0]
        cell["emitted"] = 0
        cell["four_way"]["emitted"] = 0
        cell["comparison_status"] = "stale-generation"
        cell["comparison_reason"] = (
            "emitted physical slots disagree with lattice")
        form = value["forms"][0]
        form["counts"]["emission_layout_mismatches"] = 1
        form["counts"]["comparisons"]["agree"] = 0
        form["counts"]["comparisons"]["stale-generation"] = 1
        form["counts"]["comparisons"]["unevaluable"] = 0
        form["counts"]["unevaluable"] = 0
        form["status"] = "unevaluable"
        form["reason"] = (
            "audit evidence incomplete: "
            + form["audit_evidence"]["reason"])
        value["totals"]["comparisons"]["agree"] = 0
        value["totals"]["comparisons"]["stale-generation"] = 1
        value["totals"]["comparisons"]["unevaluable"] = 0
        value["totals"]["combs_unevaluable"] = 0
        value["totals"]["forms_ok"] = 0
        value["totals"]["forms_disagreement"] = 0
        value["totals"]["forms_unevaluable"] = 1
        value["status_reasons"] = [
            "corpus coverage or one or more forms are unevaluable",
            "standalone referee runtime/application attestation is incomplete "
            "and non-enforceable",
        ]

    emission_report = clone(report)
    emission_disagreement(emission_report)
    _resign_for_self_test(emission_report)
    emission_errors, emission_stats = validate_comb_referee_report(
        emission_report, child_exit=2, expected_forms=1, expected_subjects=1)
    if (emission_errors
            or emission_stats["emission_layout_mismatches"] != 1
            or _comb_referee_outcome(
                emission_report, emission_stats,
                expected_forms=1,
                expected_subjects=1).verdict is not Verdict.FAIL):
        failures.append(
            "emission mismatch must derive globally and prevent PASS: "
            + "; ".join(emission_errors[:3]))

    def add_inference(value: dict[str, Any]) -> None:
        value["forms"][0]["inferences"].append({
            "page": 1,
            "subject_key": "p1@20,0,30,10",
            "cell_id": "p1c2",
            "state": INFERENCE_STATE,
            "blocks_gate": True,
            "reason_codes": ["unreviewed"],
            "bbox": [20.0, 0.0, 30.0, 10.0],
            "topology_sha256": sha256_bytes(b"inference"),
            "ledger_evidence": {},
            "emitted_evidence": None,
        })

    if not mutation_errors(add_inference):
        failures.append("a false inference/blocker total must be UNEVALUABLE")
    duplicate_cell_errors = mutation_errors(
        lambda value: value["forms"][0]["cells"].append(
            clone(value["forms"][0]["cells"][0])))
    if not any("duplicate cell" in error or "duplicate subject" in error
               for error in duplicate_cell_errors):
        failures.append("duplicate cell/subject identities must be UNEVALUABLE")
    duplicate_slug_errors = mutation_errors(
        lambda value: value["forms"].append(clone(value["forms"][0])))
    if not any("duplicate slug" in error for error in duplicate_slug_errors):
        failures.append("duplicate form slugs must be UNEVALUABLE")

    bound_form = report["forms"][0]
    coherent_identity = clone(bound_form)
    coherent_identity["cells"][0].update({
        "cell": "p9c9", "legacy_cell_id": "p9c9", "cell_id": "p9c9",
        "subject_key": "p9@100,0,110,10",
    })
    if not form_binding_errors(coherent_identity, snapshot):
        failures.append("coherent fabricated cell identity must be unbound")
    coherent_topology = clone(bound_form)
    coherent_topology["cells"][0].update({
        "subject_key": "p1@100,0,110,10",
        "bbox": [100.0, 0.0, 110.0, 10.0],
        "latticed": 3,
        "lattice_divider_x": [103.0, 107.0],
        "emitted": 3,
    })
    if not form_binding_errors(coherent_topology, snapshot):
        failures.append("coherent fabricated cell topology must be unbound")
    invented_ledger = clone(bound_form)
    invented_ledger["cells"][0].update({
        "ledger_topology_sha256": "f" * 64,
        "ledger_evidence": {"invented": ["anything"]},
        "emitted_evidence": {"invented": True},
    })
    if not form_binding_errors(invented_ledger, snapshot):
        failures.append("invented ledger/emission evidence must be unbound")
    stale_artifact = clone(bound_form)
    stale_artifact["artifacts"]["ir_sha256"] = "6" * 64
    if not form_binding_errors(stale_artifact, snapshot):
        failures.append("a stale per-form IR hash must be UNEVALUABLE")
    stale_optional_guide = clone(bound_form)
    stale_optional_guide["artifacts"]["guide_html_sha256"] = "7" * 64
    if not form_binding_errors(stale_optional_guide, snapshot):
        failures.append("a stale optional guide HTML hash must be UNEVALUABLE")
    stale_provenance = clone(bound_form)
    stale_provenance["artifacts"]["tracked_provenance_sha256"] = "8" * 64
    if not form_binding_errors(stale_provenance, snapshot):
        failures.append("a stale tracked provenance hash must be UNEVALUABLE")
    stale_source = clone(bound_form)
    stale_source["source"]["sha256"] = "9" * 64
    if not form_binding_errors(stale_source, snapshot):
        failures.append("a stale source PDF pin must be UNEVALUABLE")
    stale_audit_relation = clone(bound_form)
    stale_audit_relation["audit_evidence"]["holds"] = False
    if not form_binding_errors(stale_audit_relation, snapshot):
        failures.append("a false per-form audit relation must be UNEVALUABLE")
    mutated_outer_audit = clone(snapshot)
    mutated_outer_audit["audit"]["forms"]["fixture-1"]["inputs"]["ir"][
        "sha256"] = "a" * 64
    if not form_binding_errors(bound_form, mutated_outer_audit):
        failures.append("a mutated outer audit input must be UNEVALUABLE")
    fabricated_cell_audit = clone(bound_form)
    fabricated_cell_audit["cells"][0]["audit_printed"] = 2
    fabricated_cell_audit["cells"][0]["audit_relation"] = (
        "complete-non-offender")
    fabricated_cell_audit["cells"][0]["four_way"]["audit"] = 2
    if not form_binding_errors(fabricated_cell_audit, snapshot):
        failures.append(
            "cell audit topology must bind to the outer offender ledger")
    orphan_snapshot = clone(snapshot)
    orphan_relation = orphan_snapshot["audit"]["forms"]["fixture-1"][
        "assertion_relation"]
    orphan_relation.update({
        "holds": False,
        "offender_count": 1,
        "offenders_published": 1,
        "offender_dimensions": {
            "orphan-cell": {
                "cell": "orphan-cell", "page": 1, "slots": 2,
                "latticed": None, "printed": None,
                "emitted_occurrences": 1,
                "layout_relation": "not-owned",
                "emission_state": "physical-slots",
                "failure_kinds": ["unexpected-emitted-comb"],
                "source_owner_certificate": None,
                "dimensions": {
                    "layout_mismatch": False,
                    "source_unevaluable": False,
                    "emission_invalid": False,
                    "emission_behind": True,
                    "position_mismatch": False,
                    "inventory_binding": True,
                },
            },
        },
    })
    orphan_form = clone(bound_form)
    for key, value in orphan_relation.items():
        orphan_form["audit_evidence"][key] = clone(value)
    if not any("orphaned" in error for error in form_binding_errors(
            orphan_form, orphan_snapshot)):
        failures.append("orphan outer audit offender must fail closed")
    duplicate_source = clone(snapshot)
    duplicate_relation = duplicate_source["source_pdfs"]["relations"][0]
    duplicate_candidate = clone(duplicate_relation["candidates"][0])
    duplicate_candidate["path"] = "duplicate/fixture.pdf"
    duplicate_relation["candidates"].append(duplicate_candidate)
    duplicate_relation["candidate_count"] = 2
    duplicate_relation["matching_count"] = 2
    duplicate_source["source_pdfs"]["candidate_file_count"] = 2
    duplicate_source["source_pdfs"]["sha256"] = canonical_digest(
        duplicate_source["source_pdfs"]["relations"])
    if not form_binding_errors(bound_form, duplicate_source):
        failures.append(
            "two byte-identical authoritative source PDFs must fail closed")

    raw_offender = {
        "cell": "p1c2", "page": 1, "slots": 1, "latticed": None,
        "printed": None, "printed_divider_x": [],
        "physical_slots": 1, "declared_slots": 1,
        "emitted_occurrences": 1,
        "layout_relation": "not-owned", "emission_relation": "unexpected",
        "emission_state": "physical-slots",
        "failure_kinds": ["unexpected-emitted-comb"],
        "why": "synthetic unexpected emission",
    }
    outer_assertion_relation = snapshot[
        "audit"]["forms"]["fixture-1"]["assertion_relation"]
    registry_offender = {
        "cell": "<comb-owner-registry>",
        "page": None,
        "slots": None,
        "latticed": None,
        "printed": None,
        "printed_divider_x": [],
        "physical_slots": None,
        "declared_slots": None,
        "emitted_occurrences": 0,
        "emission_state": "not-evaluated",
        "effective_emission_state": "not-evaluated",
        "source_owner_certificate": {
            "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
            "valid": False,
            "reason": "synthetic global registry failure",
            "supplies_topology": False,
        },
        "layout_relation": "registry-invalid",
        "emission_relation": "not-evaluated",
        "failure_kinds": ["comb-owner-registry-invalid"],
        "why": "synthetic global registry failure",
    }
    registry_assertion = {
        **{
            key: outer_assertion_relation[key]
            for key in AUDIT_ASSERTION_SUMMARY_KEYS
        },
        "combs_expected": 0,
        "combs_checked": 0,
        "expected_comb_ids": [],
        "checked_comb_ids": [],
        "emitted_comb_ids": [],
        "owner_certificates_valid": 0,
        "owner_certificates_invalid": 0,
        "source_u_frame_evaluable": 0,
        "source_certified_unframed_evaluable": 0,
        "inventory_complete": False,
        "holds": False,
        "reason": "global owner registry is invalid",
        "offender_count": 1,
        "offenders_published": 1,
        "offenders_omitted": 0,
        "offenders_complete": True,
        "offenders": [registry_offender],
    }
    try:
        registry_relation = _normalise_outer_comb_assertion(
            registry_assertion)
    except CombRefereeScopeError as error:
        failures.append(
            f"complete red owner-registry evidence must validate: {error}")
    else:
        registry_dimensions = registry_relation.get(
            "offender_dimensions", {}).get("<comb-owner-registry>", {})
        if (registry_relation.get("holds") is not False
                or registry_relation.get("inventory_complete") is not False
                or registry_dimensions.get("dimensions", {}).get(
                    "inventory_binding") is not True):
            failures.append(
                "owner-registry pseudo offender must remain fail-closed")

    raw_assertion = {
        **{
            key: outer_assertion_relation[key]
            for key in AUDIT_ASSERTION_SUMMARY_KEYS
        },
        "holds": False,
        "reason": "one mismatch",
        "offender_count": 1,
        "offenders_published": 1,
        "offenders_omitted": 0,
        "offenders_complete": True,
        "offenders": [raw_offender],
    }
    raw_assertion.update({
        "emitted_comb_ids": ["p1c1", "p1c2"],
        "unexpected_emitted_comb_ids": ["p1c2"],
        "emitted_cell_binding_issues": 1,
        "inventory_complete": False,
        "emission_behind_layout": 1,
    })
    try:
        normalised_offenders = _normalise_outer_comb_assertion(raw_assertion)
    except CombRefereeScopeError as error:
        failures.append(f"complete outer offender ledger must validate: {error}")
        normalised_offenders = {}
    if set(normalised_offenders.get("offender_dimensions", {})) != {"p1c2"}:
        failures.append("outer offender ledger must publish an exact cell map")
    duplicated_offenders = clone(raw_assertion)
    duplicated_offenders["offenders"].append(clone(raw_offender))
    duplicated_offenders["offender_count"] = 2
    duplicated_offenders["offenders_published"] = 2
    try:
        _normalise_outer_comb_assertion(duplicated_offenders)
    except CombRefereeScopeError:
        pass
    else:
        failures.append("duplicate outer offender IDs must fail closed")
    truncated_offenders = clone(raw_assertion)
    truncated_offenders["offender_count"] = 2
    truncated_offenders["offenders_omitted"] = 1
    truncated_offenders["offenders_complete"] = False
    try:
        _normalise_outer_comb_assertion(truncated_offenders)
    except CombRefereeScopeError:
        pass
    else:
        failures.append("truncated outer offender publication must fail closed")
    false_offender_summary = clone(raw_assertion)
    false_offender_summary.update({
        "inventory_complete": True,
        "emitted_cell_binding_issues": 0,
        "emission_behind_layout": 0,
    })
    try:
        _normalise_outer_comb_assertion(false_offender_summary)
    except CombRefereeScopeError:
        pass
    else:
        failures.append(
            "offender-derived audit counters/inventory must fail closed")

    owner_certificate = {
        "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
        "valid": True,
        "layout_sha256": snapshot["layout_bindings"]["fixture-1"][
            "layout_sha256"],
        "page": 1,
        "cell_id": "p1c1",
        "legacy_cell_id": "p1c1",
        "subject_key": "p1@0,0,10,10",
        "legacy_bbox": ["0", "0", "10", "10"],
        "bbox_number_format": "canonical-decimal-string-v1",
        "state": "active_resolved",
        "supplies_topology": False,
    }
    normal_owner_offender = {
        "cell": "p1c1", "page": 1, "slots": 2, "latticed": 2,
        "printed": 3, "printed_divider_x": [3.0, 7.0],
        "physical_slots": 2, "declared_slots": 2,
        "emitted_occurrences": 1,
        "slot_indexes": [0, 1], "input_slot_indexes": [0, 1],
        "slot_geometry": [],
        "emission_container_binding": {},
        "emission_layout_position": {},
        "emission_layout_outer_position": {},
        "emission_source_position": {},
        "emission_source_outer_position": {},
        "layout_source_outer_position": {},
        "source_frame_geometry": None,
        "source_owner_certificate": owner_certificate,
        "emission_state": "physical-slots",
        "layout_relation": "mismatch",
        "emission_relation": "mismatch-printed",
        "failure_kinds": ["layout-printed-mismatch"],
        "why": "synthetic source/layout mismatch",
    }
    owner_assertion = {
        **{
            key: outer_assertion_relation[key]
            for key in AUDIT_ASSERTION_SUMMARY_KEYS
        },
        "holds": False,
        "reason": "one mismatch",
        "offender_count": 1,
        "offenders_published": 1,
        "offenders_omitted": 0,
        "offenders_complete": True,
        "offenders": [normal_owner_offender],
        "layout_mismatches": 1,
    }
    try:
        _normalise_outer_comb_assertion(
            owner_assertion, snapshot["layout_bindings"]["fixture-1"])
    except CombRefereeScopeError as error:
        failures.append(
            f"exact layout-bound owner certificate must validate: {error}")
    invalid_physical_offender = clone(normal_owner_offender)
    invalid_physical_offender.update({
        "emission_state": "invalid-slot-geometry",
        "emission_relation": "invalid",
        # The source/layout mismatch remains real, but differing emitted/source
        # counts are not comparable while physical slot geometry is invalid.
        "failure_kinds": ["layout-printed-mismatch", "invalid-emission"],
        "why": (
            "synthetic source/layout mismatch and invalid physical slots"),
    })
    invalid_physical_assertion = clone(owner_assertion)
    invalid_physical_assertion.update({
        "offenders": [invalid_physical_offender],
        "emission_behind_layout": 1,
        "emission_invalid": 1,
    })
    try:
        _normalise_outer_comb_assertion(
            invalid_physical_assertion,
            snapshot["layout_bindings"]["fixture-1"])
    except CombRefereeScopeError as error:
        failures.append(
            "invalid physical geometry must not require emitted/source "
            f"mismatch kinds: {error}")
    owner_mutations: list[tuple[str, Callable[[dict[str, Any]], None]]] = [
        ("valid", lambda value: value.update({"valid": False})),
        ("supplies_topology", lambda value: value.update({
            "supplies_topology": True})),
        ("layout_sha256", lambda value: value.update({
            "layout_sha256": "f" * 64})),
        ("page", lambda value: value.update({"page": 2})),
        ("cell_id", lambda value: value.update({"cell_id": "p1c9"})),
        ("legacy_cell_id", lambda value: value.update({
            "legacy_cell_id": "p1c9"})),
        ("subject_key", lambda value: value.update({
            "subject_key": "p1@1,0,11,10"})),
        ("legacy_bbox", lambda value: value.update({
            "legacy_bbox": ["1", "0", "11", "10"]})),
        ("state", lambda value: value.update({
            "state": "active_unresolved"})),
    ]
    for label, mutator in owner_mutations:
        mutated = clone(owner_assertion)
        mutator(mutated["offenders"][0]["source_owner_certificate"])
        try:
            _normalise_outer_comb_assertion(
                mutated, snapshot["layout_bindings"]["fixture-1"])
        except CombRefereeScopeError:
            pass
        else:
            failures.append(
                f"mutated owner-certificate {label} must fail closed")
    owner_count_mutation = clone(owner_assertion)
    owner_count_mutation.update({
        "owner_certificates_valid": 0,
        "owner_certificates_invalid": 1,
    })
    try:
        _normalise_outer_comb_assertion(
            owner_count_mutation,
            snapshot["layout_bindings"]["fixture-1"])
    except CombRefereeScopeError:
        pass
    else:
        failures.append("false owner-certificate summary must fail closed")
    false_source_partition = clone(owner_assertion)
    false_source_partition.update({
        "source_u_frame_evaluable": 1,
        "source_certified_unframed_evaluable": 1,
    })
    try:
        _normalise_outer_comb_assertion(
            false_source_partition,
            snapshot["layout_bindings"]["fixture-1"])
    except CombRefereeScopeError:
        pass
    else:
        failures.append(
            "source frame/unframed counts must partition evaluable subjects")
    false_source_classification = clone(owner_assertion)
    false_source_classification.update({
        "source_u_frame_evaluable": 1,
        "source_certified_unframed_evaluable": 0,
    })
    try:
        _normalise_outer_comb_assertion(
            false_source_classification,
            snapshot["layout_bindings"]["fixture-1"])
    except CombRefereeScopeError:
        pass
    else:
        failures.append(
            "published certified-unframed evidence must bind its counter")
    two_cell_binding = clone(snapshot["layout_bindings"]["fixture-1"])
    second_projection = clone(two_cell_binding["cells"]["p1c1"])
    second_projection.update({
        "cell": "p1c2", "legacy_cell_id": "p1c2", "cell_id": "p1c2",
        "subject_key": "p1@10,0,20,10",
        "bbox": [10.0, 0.0, 20.0, 10.0],
    })
    two_cell_binding["cells"]["p1c2"] = second_projection
    two_cell_binding["audit_expected_ids"] = ["p1c1", "p1c2"]
    second_owner_offender = clone(normal_owner_offender)
    second_owner_offender.update({"cell": "p1c2"})
    second_owner_offender["source_owner_certificate"].update({
        "cell_id": "p1c2", "legacy_cell_id": "p1c2",
        "subject_key": "p1@10,0,20,10",
        "legacy_bbox": ["10", "0", "20", "10"],
    })
    two_valid_assertion = clone(owner_assertion)
    two_valid_assertion.update({
        "combs_expected": 2,
        "combs_checked": 2,
        "expected_comb_ids": ["p1c1", "p1c2"],
        "checked_comb_ids": ["p1c1", "p1c2"],
        "emitted_comb_ids": ["p1c1", "p1c2"],
        "owner_certificates_valid": 2,
        "owner_certificates_invalid": 0,
        "source_u_frame_evaluable": 0,
        "source_certified_unframed_evaluable": 2,
        "layout_mismatches": 2,
        "offender_count": 2,
        "offenders_published": 2,
        "offenders": [normal_owner_offender, second_owner_offender],
    })
    try:
        _normalise_outer_comb_assertion(two_valid_assertion, two_cell_binding)
    except CombRefereeScopeError as error:
        failures.append(f"two exact valid owner certificates must bind: {error}")
    false_two_valid = clone(two_valid_assertion)
    false_two_valid.update({
        "owner_certificates_valid": 1,
        "owner_certificates_invalid": 1,
    })
    try:
        _normalise_outer_comb_assertion(false_two_valid, two_cell_binding)
    except CombRefereeScopeError:
        pass
    else:
        failures.append("two valid certificates cannot be summarized as 1/1")
    two_invalid = clone(two_valid_assertion)
    invalid_certificate = {
        "criterion": "exact-reviewed-layout-comb-subject-owner-v1",
        "valid": False,
        "reason": "synthetic invalid owner",
        "supplies_topology": False,
    }
    for offender in two_invalid["offenders"]:
        offender["source_owner_certificate"] = clone(invalid_certificate)
        offender.update({
            "printed": None,
            "printed_divider_x": [],
            "layout_relation": "unevaluable",
            "emission_relation": "source-unevaluable",
            "failure_kinds": ["source-topology-unevaluable"],
            "why": "synthetic source topology is unevaluable",
        })
    two_invalid.update({
        "owner_certificates_valid": 0,
        "owner_certificates_invalid": 2,
        "source_u_frame_evaluable": 0,
        "source_certified_unframed_evaluable": 0,
        "layout_mismatches": 0,
        "layout_unevaluable": 2,
    })
    try:
        _normalise_outer_comb_assertion(two_invalid)
    except CombRefereeScopeError as error:
        failures.append(f"two explicit invalid owner certificates must bind: {error}")
    false_two_invalid = clone(two_invalid)
    false_two_invalid.update({
        "owner_certificates_valid": 1,
        "owner_certificates_invalid": 1,
    })
    try:
        _normalise_outer_comb_assertion(false_two_invalid)
    except CombRefereeScopeError:
        pass
    else:
        failures.append("two invalid certificates cannot be summarized as 1/1")

    repeated_failure = repeat_run_failure(
        [0, 0], [raw_payload, raw_payload + b"x"])
    if (repeated_failure is None
            or repeated_failure.verdict is not Verdict.UNEVALUABLE):
        failures.append(
            "byte-different repeated referee output must be UNEVALUABLE")

    scope_trees = {
        name: {"sha256": sha256_bytes(name.encode("utf-8"))}
        for name in {"forms", *COMB_REFEREE_ARTIFACT_TREES}
    }
    scope_batch = {"sha256": sha256_bytes(b"batch")}
    base_scope = compose_generated_scope(scope_trees, scope_batch)
    for tree_name in scope_trees:
        mutated_trees = clone(scope_trees)
        mutated_trees[tree_name]["sha256"] = "f" * 64
        if (compose_generated_scope(mutated_trees, scope_batch)["sha256"]
                == base_scope["sha256"]):
            failures.append(
                f"determinism digest ignores generated tree: {tree_name}")
    mutated_batch = clone(scope_batch)
    mutated_batch["sha256"] = "0" * 64
    if (compose_generated_scope(scope_trees, mutated_batch)["sha256"]
            == base_scope["sha256"]):
        failures.append("determinism digest ignores the batch report")

    with tempfile.TemporaryDirectory(
            prefix="formgen-gate-pipeline-self-test-") as pipeline_tmp:
        pipeline_root = pathlib.Path(pipeline_tmp)
        test_slugs = frozenset({"fixture-1"})
        batch_payload = (json.dumps([
            _synthetic_batch_record("fixture-1")], indent=2) + "\n")

        def write_batch(args: list[str]) -> None:
            output = pathlib.Path(args[args.index("--report") + 1])
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_text(batch_payload, encoding="utf-8")

        ordering: list[str] = []

        def ordered_runner(args: list[str], _timeout: int) -> tuple[int, str]:
            ordering.append(pathlib.Path(args[0]).name)
            write_batch(args)
            return 0, ""

        def ordered_referee() -> Result:
            ordering.append("referee")
            return Result("comb-referee", Verdict.UNEVALUABLE, "synthetic")

        def ordered_audit() -> Result:
            ordering.append("audit.py")
            return Result("audit-refresh", Verdict.PASS, "synthetic")

        same_generation = {"sha256": "b" * 64, "scope": "same"}
        same_audit = {"path": "build/audit.json", "sha256": "a" * 64}
        ordered_refresh = refresh_full_pipeline(
            runner=ordered_runner,
            generation_reader=lambda _batch: clone(same_generation),
            audit_refresher=ordered_audit,
            referee_refresher=ordered_referee,
            scratch_root=pipeline_root,
            batch_target=pipeline_root / "published-batch.json",
            expected_slugs=test_slugs,
            audit_identity_reader=lambda: clone(same_audit),
        )
        if ordering != ["batch.py", "batch.py", "audit.py", "referee"]:
            failures.append(
                "full refresh must order batch, batch, audit, referee exactly")
        if ordered_refresh.determinism.verdict is not Verdict.PASS:
            failures.append("identical pre-audit generations must pass determinism")

        changed_order: list[str] = []
        changed_generations = iter([
            {"sha256": "c" * 64, "scope": "first"},
            {"sha256": "d" * 64, "scope": "second"},
        ])

        def changed_runner(args: list[str], _timeout: int) -> tuple[int, str]:
            changed_order.append(pathlib.Path(args[0]).name)
            write_batch(args)
            return 0, ""

        changed_refresh = refresh_full_pipeline(
            runner=changed_runner,
            generation_reader=lambda _batch: next(changed_generations),
            audit_refresher=lambda: (
                changed_order.append("audit.py")
                or Result("audit-refresh", Verdict.PASS, "must not run")),
            referee_refresher=lambda: (
                changed_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            scratch_root=pipeline_root,
            batch_target=pipeline_root / "changed-batch.json",
            expected_slugs=test_slugs,
        )
        if (changed_refresh.determinism.verdict is not Verdict.FAIL
                or changed_order != ["batch.py", "batch.py"]):
            failures.append(
                "nondeterminism must suppress audit/referee after batch #2")

        stale_order: list[str] = []
        stale_refresh = refresh_full_pipeline(
            runner=lambda args, _timeout: (
                stale_order.append(pathlib.Path(args[0]).name) or 0, ""),
            generation_reader=lambda _batch: clone(same_generation),
            audit_refresher=lambda: (
                stale_order.append("audit.py")
                or Result("audit-refresh", Verdict.PASS, "must not run")),
            referee_refresher=lambda: (
                stale_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            scratch_root=pipeline_root,
            batch_target=pipeline_root / "stale-batch.json",
            expected_slugs=test_slugs,
        )
        if (stale_refresh.determinism.verdict is Verdict.PASS
                or stale_order != ["batch.py"]):
            failures.append(
                "exit-0 batch without a fresh private report must fail closed")

        audit_failure_order: list[str] = []

        def audit_failure_runner(
                args: list[str], _timeout: int) -> tuple[int, str]:
            audit_failure_order.append("batch.py")
            write_batch(args)
            return 0, ""

        def failed_audit() -> Result:
            audit_failure_order.append("audit.py")
            return Result(
                "audit-refresh", Verdict.UNEVALUABLE, "synthetic failure")

        audit_failure_refresh = refresh_full_pipeline(
            runner=audit_failure_runner,
            generation_reader=lambda _batch: clone(same_generation),
            audit_refresher=failed_audit,
            referee_refresher=lambda: (
                audit_failure_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            scratch_root=pipeline_root,
            batch_target=pipeline_root / "audit-failure-batch.json",
            expected_slugs=test_slugs,
        )
        if (audit_failure_refresh.audit_refresh.verdict
                is not Verdict.UNEVALUABLE
                or audit_failure_refresh.comb_referee.verdict
                is not Verdict.UNEVALUABLE
                or audit_failure_order
                != ["batch.py", "batch.py", "audit.py"]):
            failures.append(
                "a failed final audit must fail the gate and suppress referee")

        post_audit_order: list[str] = []
        post_audit_generations = iter([
            clone(same_generation), clone(same_generation),
            clone(same_generation),
            {"sha256": "d" * 64, "scope": "audit-mutated"},
        ])

        def post_audit_runner(
                args: list[str], _timeout: int) -> tuple[int, str]:
            post_audit_order.append("batch.py")
            write_batch(args)
            return 0, ""

        post_audit_refresh = refresh_full_pipeline(
            runner=post_audit_runner,
            generation_reader=lambda _batch: next(post_audit_generations),
            audit_refresher=lambda: (
                post_audit_order.append("audit.py")
                or Result("audit-refresh", Verdict.PASS, "synthetic")),
            referee_refresher=lambda: (
                post_audit_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            scratch_root=pipeline_root,
            batch_target=pipeline_root / "post-audit-batch.json",
            expected_slugs=test_slugs,
        )
        if (post_audit_refresh.determinism.verdict is not Verdict.FAIL
                or post_audit_order != ["batch.py", "batch.py", "audit.py"]):
            failures.append(
                "an audit mutation must invalidate scope and suppress referee")

        final_order: list[str] = []
        final_result = refresh_final_comb_referee(
            compose_final_referee_scope(same_generation, same_audit),
            referee_refresher=lambda: (
                final_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            generation_reader=lambda _batch: {
                "sha256": "f" * 64, "scope": "changed-after-checks"},
            batch_target=pipeline_root / "published-batch.json",
            expected_slugs=test_slugs,
            audit_identity_reader=lambda: clone(same_audit),
        )
        if (final_result.verdict is not Verdict.UNEVALUABLE or final_order):
            failures.append(
                "final scope mutation must suppress the last referee execution")
        final_audit_order: list[str] = []
        final_audit_result = refresh_final_comb_referee(
            compose_final_referee_scope(same_generation, same_audit),
            referee_refresher=lambda: (
                final_audit_order.append("referee")
                or Result("comb-referee", Verdict.PASS, "must not run")),
            generation_reader=lambda _batch: clone(same_generation),
            batch_target=pipeline_root / "published-batch.json",
            expected_slugs=test_slugs,
            audit_identity_reader=lambda: {
                "path": "build/audit.json", "sha256": "0" * 64},
        )
        if (final_audit_result.verdict is not Verdict.UNEVALUABLE
                or final_audit_order):
            failures.append(
                "final audit mutation must suppress the last referee execution")

        duplicate_batch = [
            _synthetic_batch_record("fixture-1"),
            _synthetic_batch_record("fixture-1"),
        ]
        if not batch_report_errors(duplicate_batch, test_slugs):
            failures.append("duplicate batch-report slugs must fail closed")

    disagreement = clone(report)
    disagreement["totals"]["comparisons"]["agree"] = 0
    disagreement["totals"]["comparisons"]["repair-lattice"] = 1
    if _comb_referee_outcome(
            disagreement, {"pending_transitions": 0}).verdict is not Verdict.FAIL:
        failures.append("an actual referee disagreement must be FAIL, not UNEVALUABLE")

    command = _comb_referee_command(
        pathlib.Path("/tmp/private-report.json"),
        pathlib.Path("/tmp/private-empty-pycache"))
    if command[:3] != [sys.executable, "-I", "-B"]:
        failures.append("comb referee must use exact sys.executable with -I -B")
    environment_probe = {
        "PATH": "/poison", "PYTHONPATH": "poison", "PYTHONHOME": "poison",
    }
    sanitized = _sanitized_referee_environment(snapshot, environment_probe)
    if "PYTHONPATH" in sanitized or "PYTHONHOME" in sanitized:
        failures.append("comb referee environment must remove Python path/home")

    with tempfile.TemporaryDirectory(prefix="formgen-gate-self-test-") as tmp:
        root = pathlib.Path(tmp)
        audit_scope_fixture = {
            key: clone(value) for key, value in snapshot.items()
            if key != "audit"
        }
        audit_scope_reader = lambda: clone(audit_scope_fixture)
        target = root / "audit.json"
        target.write_text('[{"stale": true}]\n', encoding="utf-8")

        refresh_failure = refresh_assertions_report(
            target=target,
            scratch_root=root,
            runner=lambda _args, _timeout: (1, "synthetic refresh failure"),
        )
        if (refresh_failure is None
                or refresh_failure.verdict is not Verdict.UNEVALUABLE):
            failures.append("a failed assertion refresh must be UNEVALUABLE")
        if load(target) != [{"stale": True}]:
            failures.append("a failed assertion refresh must not publish partial data")

        def fake_refresh(args: list[str], _timeout: int) -> tuple[int, str]:
            out = pathlib.Path(args[args.index("--out") + 1])
            out.write_text('[{"fresh": true}]\n', encoding="utf-8")
            return 0, ""

        refresh_success = refresh_assertions_report(
            target=target,
            scratch_root=root,
            runner=fake_refresh,
        )
        if refresh_success is not None or load(target) != [{"fresh": True}]:
            failures.append("a successful assertion refresh must publish fresh data")

        full_target = root / "full-audit.json"
        full_target.write_text('[{"stale": true}]\n', encoding="utf-8")
        audit_slugs = frozenset(
            f"fixture-{index}" for index in range(EXPECTED_FORMS))
        failed_full_audit = refresh_full_audit_report(
            target=full_target,
            attestation_target=root / "failed-audit-attested.json",
            scratch_root=root,
            runner=lambda _args, _timeout: (1, "synthetic full-audit failure"),
            expected_slugs=audit_slugs,
            scope_reader=audit_scope_reader,
        )
        if (failed_full_audit.verdict is not Verdict.UNEVALUABLE
                or load(full_target) != [{"stale": True}]):
            failures.append(
                "a failed full audit refresh must not publish stale/partial data")

        def fake_slug_only_audit(
                args: list[str], _timeout: int) -> tuple[int, str]:
            out = pathlib.Path(args[args.index("--out") + 1])
            out.write_text(json.dumps([
                {"slug": f"fixture-{index}"}
                for index in range(EXPECTED_FORMS)
            ]) + "\n", encoding="utf-8")
            return 0, ""

        slug_only_audit = refresh_full_audit_report(
            target=full_target,
            attestation_target=root / "slug-audit-attested.json",
            scratch_root=root,
            runner=fake_slug_only_audit,
            expected_slugs=audit_slugs,
            scope_reader=audit_scope_reader,
        )
        if (slug_only_audit.verdict is not Verdict.UNEVALUABLE
                or load(full_target) != [{"stale": True}]):
            failures.append(
                "a slug-only audit fixture must not replace prior evidence")

        def fake_full_audit(
                args: list[str], _timeout: int) -> tuple[int, str]:
            out = pathlib.Path(args[args.index("--out") + 1])
            out.write_text(json.dumps([
                _synthetic_audit_record(f"fixture-{index}")
                for index in range(EXPECTED_FORMS)
            ]) + "\n", encoding="utf-8")
            return 0, ""

        successful_full_audit = refresh_full_audit_report(
            target=full_target,
            attestation_target=root / "full-audit-attested.json",
            scratch_root=root,
            runner=fake_full_audit,
            expected_slugs=audit_slugs,
            scope_reader=audit_scope_reader,
        )
        if (successful_full_audit.verdict is not Verdict.PASS
                or not isinstance(load(full_target), list)
                or len(load(full_target)) != EXPECTED_FORMS):
            failures.append(
                "a complete full audit refresh must publish atomically")
        else:
            attested_payload = full_target.read_bytes()
            attested_envelope = load(root / "full-audit-attested.json")
            if validate_audit_application_envelope(
                    attested_envelope, attested_payload,
                    audit_scope_fixture):
                failures.append(
                    "fresh audit application envelope must validate")
            for label, mutator in (
                    (
                        "audit isolated-Python flags",
                        lambda value: value["invocation"].update({
                            "python_flags": []}),
                    ),
                    (
                        "audit private output",
                        lambda value: value["invocation"].update({
                            "output": "build/audit.json"}),
                    ),
                    (
                        "audit raw digest",
                        lambda value: value["raw_report"].update({
                            "sha256": "f" * 64}),
                    )):
                mutated_envelope = clone(attested_envelope)
                mutator(mutated_envelope)
                _resign_for_self_test(mutated_envelope)
                if not validate_audit_application_envelope(
                        mutated_envelope, attested_payload,
                        audit_scope_fixture):
                    failures.append(f"mutated {label} must fail closed")
            changed_audit_scope = clone(audit_scope_fixture)
            changed_audit_scope["artifact_trees"]["layout"]["sha256"] = (
                "e" * 64)
            if not validate_audit_application_envelope(
                    attested_envelope, attested_payload,
                    changed_audit_scope):
                failures.append(
                    "post-audit application input mutation must fail closed")

    for name in failures:
        print(f"FAIL {name}", file=sys.stderr)
    print(f"gate self-test: {len(failures)} failure(s)", file=sys.stderr)
    return 1 if failures else 0


def summarise(results: list[Result], echo: bool = False) -> int:
    width = max((len(r.name) for r in results), default=0)
    for r in results:
        if echo:
            print(f"  {r.verdict.value:<11} {r.name:<{width}}  {r.detail}")
    failed = [r for r in results if not r.verdict.ok]
    if echo:
        print()
        if failed:
            print(f"GATE FAILS -- {len(failed)} of {len(results)} checks not satisfied")
            for r in failed:
                print(f"  - {r.name}: {r.detail}")
        else:
            print(f"GATE PASSES -- all {len(results)} checks satisfied")
    return 1 if failed else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--only", action="append", choices=sorted(CHECKS) + ["determinism"],
                        help="Run one check while iterating. Not the done-condition.")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--skip-regenerate", action="store_true",
                        help="Score the forms/ tree as it stands. Not the done-condition.")
    parser.add_argument("--json", type=pathlib.Path, default=None)
    args = parser.parse_args(argv)

    if args.list:
        for name in list(CHECKS) + ["determinism"]:
            print(name)
        return 0
    if args.self_test:
        return self_test()

    full = not args.only
    full_refresh = None
    if full and args.json is not None:
        json_output = args.json.resolve()
        for protected_root in (REPO, COMB_REFEREE_SOURCE_ROOT):
            try:
                json_output.relative_to(protected_root.resolve())
            except ValueError:
                continue
            parser.error(
                "the full gate's --json output must be outside the repository "
                "and official-source tree; the write would stale the final "
                "referee snapshot")
    if full and not args.skip_regenerate:
        print("regenerating twice and auditing final bytes; referee runs last...",
              file=sys.stderr)
        full_refresh = refresh_full_pipeline(referee_refresher=None)
        for diagnostic in full_refresh.diagnostics:
            print(f"  {diagnostic}", file=sys.stderr)

    wanted = args.only or list(CHECKS)
    refresh_failure = None
    if args.only and "assertions" in args.only:
        print("refreshing assertion audit...", file=sys.stderr)
        refresh_failure = refresh_assertions_report()
    def evaluate_check(name: str) -> Result:
        if name == "assertions" and refresh_failure is not None:
            return refresh_failure
        if (full_refresh is not None
                and not full_refresh.determinism.verdict.ok
                and name == "conversion"):
            return Result(
                name, Verdict.UNEVALUABLE,
                "fresh deterministic generation failed; stale batch report "
                "was not scored",
            )
        if (full_refresh is not None
                and not full_refresh.audit_refresh.verdict.ok
                and name in AUDIT_DEPENDENT_CHECKS):
            return Result(
                name, Verdict.UNEVALUABLE,
                "fresh final-corpus audit failed; stale audit was not scored",
            )
        return CHECKS[name]()

    results = [
        evaluate_check(name) for name in wanted
        if name in CHECKS and name != "comb-referee"
    ]
    if full_refresh is not None:
        results.append(full_refresh.audit_refresh)
    if "determinism" in wanted or full:
        results.append(
            full_refresh.determinism if full_refresh is not None
            else check_determinism(regenerate=False))

    # No executable/mutating gate check follows this point. In a full run the
    # current generated scope is re-read immediately before the two isolated
    # referee children, so module self-tests cannot silently stale its evidence.
    if "comb-referee" in wanted:
        print("running final application-scoped comb referee...", file=sys.stderr)
        if full_refresh is not None:
            if (not full_refresh.audit_refresh.verdict.ok
                    or full_refresh.generated_scope is None):
                comb_result = full_refresh.comb_referee
            else:
                comb_result = refresh_final_comb_referee(
                    full_refresh.generated_scope)
        elif args.only and "comb-referee" in args.only:
            comb_result = refresh_comb_referee_report()
        else:
            comb_result = check_comb_referee()
        results.append(comb_result)

    print(f"\nformgen gate -- {len(results)} checks\n")
    exit_code = summarise(results, echo=True)

    if args.json:
        args.json.write_text(json.dumps(
            [{"name": r.name, "verdict": r.verdict.value, "detail": r.detail}
             for r in results], indent=2) + "\n", encoding="utf-8")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
