#!/usr/bin/env python3
"""Inspect and verify an exact macOS HTML certification candidate.

This tool is intentionally non-promotional.  It binds the candidate archive
created by ``html-candidate-certification.yml`` to its manifest, clean source
revision, packaged renderer identity, and extracted application.  It can also
launch the real non-development executable under a network-denial sandbox.

The strict ``verify-attestation`` command accepts only a complete external
macOS attestation covering the user-visible preview/export/print operations,
PDF validation, package security, state preservation, and every rollback case.
It fails closed when Accessibility, a completed printer job, Developer ID
signing, notarization, or stapling cannot be independently confirmed.  Even a
successful foundation report remains untrusted and cannot be copied into form
release evidence.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import re
import signal
import stat
import subprocess
import sys
import tempfile
import time
import uuid
import zipfile
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import macos_native_evidence_driver as common  # noqa: E402


SCHEMA_VERSION = 1
CANDIDATE_SCOPE = "html_certification_candidate_input"
BINDING_SCOPE = "external_macos_candidate_binding"
PROBE_SCOPE = "external_macos_candidate_nondev_probe"
ATTESTATION_SCOPE = "external_macos_candidate_certification_attestation"
REPORT_SCOPE = "external_macos_candidate_certification_verification"
FORM = {"code": "2551Q", "revision": "2018"}
EXPECTED_PAGE_COUNT = 2
EXPECTED_WIDTH_POINTS = 612.0
EXPECTED_HEIGHT_POINTS = 936.0
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_REVISION = re.compile(r"[0-9a-f]{40}\Z")
RFC3339_UTC = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\Z")
ROLLBACK_CASES = frozenset(
    {
        "release_ready_false",
        "kill_switch",
        "missing_assets",
        "renderer_error",
        "late_renderer_error",
        "readiness_timeout",
        "invalid_geometry",
        "rejected_pdf",
        "destination_preserved",
        "no_temp_leaks",
        "draft_unchanged",
    }
)
MAX_ARCHIVE_FILES = 20_000
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
NON_PROMOTIONAL_GAP = "collector producer is not registered as trusted"


EvidenceError = common.EvidenceError


def regular_file(path: Path, label: str) -> Path:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise EvidenceError(f"{label} must be a regular non-symlink file")
    return path.resolve(strict=True)


def load_json(path: Path, *, limit: int = 16 * 1024 * 1024) -> dict[str, Any]:
    try:
        value = json.loads(common.read_stable_file(path, limit=limit))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid JSON artifact {path}: {error}") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"JSON artifact must contain an object: {path}")
    return value


def require_exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be an object")
    actual = set(value)
    if actual != keys:
        missing = sorted(keys - actual)
        unknown = sorted(actual - keys)
        raise EvidenceError(
            f"{label} schema mismatch; missing={missing!r}, unknown={unknown!r}"
        )
    return value


def require_sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or SHA256.fullmatch(value) is None:
        raise EvidenceError(f"{label} must be a canonical SHA-256 digest")
    return value


def require_true(value: Any, label: str) -> None:
    if value is not True:
        raise EvidenceError(f"{label} must be observed and true")


def require_nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise EvidenceError(f"{label} is required")
    return value


def resolve_record_path(record: dict[str, Any], base: Path) -> Path:
    raw = require_nonempty_string(record.get("path"), "artifact path")
    candidate = Path(raw)
    if candidate.is_absolute():
        return candidate
    if ".." in candidate.parts:
        raise EvidenceError(f"artifact path escapes its attestation directory: {raw}")
    return base / candidate


def verify_file_record(
    record: Any,
    label: str,
    *,
    base: Path,
    verified: list[dict[str, Any]] | None = None,
) -> dict[str, Any]:
    value = require_exact_keys(record, {"path", "byte_count", "sha256"}, label)
    expected_hash = require_sha256(value["sha256"], f"{label}.sha256")
    if not isinstance(value["byte_count"], int) or value["byte_count"] < 0:
        raise EvidenceError(f"{label}.byte_count must be a non-negative integer")
    path = resolve_record_path(value, base)
    actual = common.file_record(path)
    if actual["sha256"] != expected_hash or actual["byte_count"] != value["byte_count"]:
        raise EvidenceError(f"{label} changed or does not match its immutable record")
    normalized = {
        "path": str(path.resolve()),
        "byte_count": actual["byte_count"],
        "sha256": actual["sha256"],
    }
    if verified is not None:
        verified.append(normalized | {"label": label})
    return normalized


def validate_candidate_inputs(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
) -> dict[str, Any]:
    manifest_path = regular_file(manifest_path, "candidate manifest")
    archive_path = regular_file(archive_path, "candidate archive")
    identity_path = regular_file(identity_path, "renderer identity")
    manifest = load_json(manifest_path, limit=256 * 1024)

    required = {
        "schema_version",
        "scope",
        "promotion_eligible",
        "trusted_producer",
        "form",
        "source_revision",
        "platform",
        "architecture",
        "artifact",
        "renderer_identity",
        "release_policy",
    }
    require_exact_keys(manifest, required, "candidate manifest")
    if manifest["schema_version"] != 1 or manifest["scope"] != CANDIDATE_SCOPE:
        raise EvidenceError("candidate manifest has an unsupported schema or scope")
    if manifest["promotion_eligible"] is not False:
        raise EvidenceError("candidate manifest must remain non-promotional")
    if manifest["trusted_producer"] is not False:
        raise EvidenceError("candidate manifest must not claim a trusted producer")
    if manifest["form"] != FORM:
        raise EvidenceError("candidate manifest must target exactly 2551Q:2018")
    source_revision = manifest["source_revision"]
    if not isinstance(source_revision, str) or GIT_REVISION.fullmatch(source_revision) is None:
        raise EvidenceError("candidate source revision is not canonical")
    if manifest["platform"] != "macos" or manifest["architecture"] != "universal":
        raise EvidenceError("candidate must be the macOS universal workflow artifact")

    archive = require_exact_keys(
        manifest["artifact"], {"name", "byte_count", "sha256"}, "candidate archive"
    )
    archive_record = common.file_record(archive_path)
    if archive.get("name") != archive_path.name:
        raise EvidenceError("candidate archive name differs from its manifest")
    if archive.get("byte_count") != archive_record["byte_count"]:
        raise EvidenceError("candidate archive size differs from its manifest")
    if require_sha256(archive.get("sha256"), "candidate archive hash") != archive_record[
        "sha256"
    ]:
        raise EvidenceError("candidate archive hash differs from its manifest")

    identity_record = require_exact_keys(
        manifest["renderer_identity"],
        {"name", "sha256", "renderer_bundle_sha256"},
        "candidate renderer identity",
    )
    actual_identity = common.file_record(identity_path)
    if identity_record.get("name") != identity_path.name:
        raise EvidenceError("renderer identity name differs from the manifest")
    if require_sha256(identity_record.get("sha256"), "renderer identity hash") != actual_identity[
        "sha256"
    ]:
        raise EvidenceError("renderer identity bytes differ from the manifest")
    expected_renderer = require_sha256(
        identity_record.get("renderer_bundle_sha256"), "renderer bundle hash"
    )
    identity = common.validate_build_identity(identity_path, expected_renderer)
    identity_source = identity.get("source_revision", {})
    if identity_source != {"status": "observed", "value": source_revision}:
        raise EvidenceError("renderer identity is not bound to the candidate source revision")

    release_policy = require_exact_keys(
        manifest["release_policy"],
        {
            "candidate_build_requires_release_ready",
            "tagged_release_still_requires_release_ready",
        },
        "candidate release policy",
    )
    if release_policy != {
        "candidate_build_requires_release_ready": False,
        "tagged_release_still_requires_release_ready": True,
    }:
        raise EvidenceError("candidate manifest weakened the tagged release policy")

    return {
        "source_revision": source_revision,
        "candidate_manifest": common.file_record(manifest_path),
        "candidate_archive": archive_record,
        "renderer_identity": actual_identity,
        "renderer_bundle_sha256": expected_renderer,
    }


def _safe_member_path(name: str) -> PurePosixPath:
    if not name or "\\" in name:
        raise EvidenceError(f"candidate archive contains an invalid member name: {name!r}")
    member = PurePosixPath(name)
    if member.is_absolute() or ".." in member.parts or not member.parts:
        raise EvidenceError(f"candidate archive member escapes extraction root: {name!r}")
    return member


def extract_candidate_archive(archive: Path, destination: Path) -> Path:
    archive = regular_file(archive, "candidate archive")
    if destination.exists() and any(destination.iterdir()):
        raise EvidenceError("candidate extraction directory must be absent or empty")
    destination.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    seen_casefold: set[str] = set()
    total_bytes = 0
    with zipfile.ZipFile(archive) as bundle:
        members = bundle.infolist()
        if not members or len(members) > MAX_ARCHIVE_FILES:
            raise EvidenceError("candidate archive has an invalid member count")
        for info in members:
            member = _safe_member_path(info.filename)
            normalized = member.as_posix().rstrip("/")
            if not normalized:
                continue
            if normalized in seen or normalized.casefold() in seen_casefold:
                raise EvidenceError(f"candidate archive contains duplicate paths: {normalized}")
            seen.add(normalized)
            seen_casefold.add(normalized.casefold())
            total_bytes += info.file_size
            if total_bytes > MAX_ARCHIVE_BYTES:
                raise EvidenceError("candidate archive exceeds its extraction size limit")
            mode = (info.external_attr >> 16) & 0xFFFF
            if stat.S_ISLNK(mode):
                raise EvidenceError(f"candidate archive contains a symlink: {normalized}")
            target = destination.joinpath(*member.parts)
            if info.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                raise EvidenceError(f"candidate extraction target already exists: {target}")
            with bundle.open(info) as source, target.open("xb") as output:
                while chunk := source.read(1024 * 1024):
                    output.write(chunk)
                output.flush()
                os.fsync(output.fileno())
            permissions = mode & 0o777
            target.chmod(permissions or 0o644)

    apps = [
        path
        for path in destination.rglob("*.app")
        if path.is_dir()
        and "__MACOSX" not in path.parts
        and not any(parent.suffix == ".app" for parent in path.parents)
    ]
    if len(apps) != 1:
        raise EvidenceError("candidate archive must contain exactly one application bundle")
    return apps[0]


def bind_extracted_app(
    candidate: dict[str, Any], app: Path, external_identity_path: Path
) -> dict[str, Any]:
    app = app.resolve(strict=True)
    if app.suffix != ".app" or not app.is_dir():
        raise EvidenceError("extracted candidate is not a macOS application bundle")
    binary = common.app_binary(app)
    bundled_identity_path = app / common.IDENTITY_RELATIVE_PATH
    bundled_identity_record = common.file_record(bundled_identity_path)
    external_identity_record = common.file_record(external_identity_path)
    if bundled_identity_record["sha256"] != external_identity_record["sha256"]:
        raise EvidenceError("packaged renderer identity differs from the uploaded identity")
    identity = common.validate_build_identity(
        bundled_identity_path, candidate["renderer_bundle_sha256"]
    )
    renderer = app / common.RENDERER_RELATIVE_PATH
    renderer_hash = identity["renderer_bundle_sha256"]
    if renderer.exists():
        renderer_hash = common.tree_hash(renderer)
        if renderer_hash != candidate["renderer_bundle_sha256"]:
            raise EvidenceError("packaged renderer differs from the candidate manifest")
        common.validate_build_identity(bundled_identity_path, renderer_hash)
    return {
        "app_path": str(app),
        "app_tree_sha256": common.tree_hash(app),
        "binary": common.file_record(binary),
        "renderer_path": str(renderer) if renderer.exists() else None,
        "renderer_bundle_sha256": renderer_hash,
        "bundled_renderer_identity": bundled_identity_record,
    }


def inspect_candidate(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    output_dir: Path,
) -> dict[str, Any]:
    candidate = validate_candidate_inputs(manifest_path, archive_path, identity_path)
    output_dir = output_dir.resolve()
    if output_dir.exists() and any(output_dir.iterdir()):
        raise EvidenceError("inspection output directory must be absent or empty")
    output_dir.mkdir(parents=True, exist_ok=True)
    app = extract_candidate_archive(archive_path, output_dir / "extracted")
    packaged = bind_extracted_app(candidate, app, identity_path)
    binding = {
        "schema_version": SCHEMA_VERSION,
        "scope": BINDING_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "certification_complete": False,
        "form": FORM,
        "source_revision": candidate["source_revision"],
        "candidate_manifest": candidate["candidate_manifest"],
        "candidate_archive": candidate["candidate_archive"],
        "renderer_identity": candidate["renderer_identity"],
        "packaged_app": packaged,
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            "preview, Export PDF/save chooser, native print, and rollback were not exercised",
            "Developer ID, notarization, stapling, Accessibility, and printer state were not verified",
        ],
    }
    common.write_json_atomic(output_dir / "macos-candidate-binding.json", binding)
    return binding


def _terminate(process: subprocess.Popen[str]) -> None:
    try:
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def probe_nondev_candidate(binding: dict[str, Any], output_dir: Path, timeout: float) -> dict[str, Any]:
    if platform.system() != "Darwin":
        raise EvidenceError("the non-development candidate probe must run on macOS")
    sandbox = Path("/usr/bin/sandbox-exec")
    if not sandbox.is_file():
        raise EvidenceError("network-denial sandbox-exec is unavailable")
    binary = Path(binding["packaged_app"]["binary"]["path"])
    app = Path(binding["packaged_app"]["app_path"])
    package_before = common.tree_hash(app)
    stdout_path = output_dir / "nondev-probe.stdout.log"
    stderr_path = output_dir / "nondev-probe.stderr.log"
    environment = os.environ.copy()
    for key in list(environment):
        if key == "DEVELOPER_MODE" or key.startswith("EBIR_NATIVE_EVIDENCE"):
            environment.pop(key, None)
    command = [
        str(sandbox),
        "-p",
        "(version 1) (allow default) (deny network*)",
        str(binary),
    ]
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
        "w", encoding="utf-8"
    ) as stderr:
        process = subprocess.Popen(command, env=environment, text=True, stdout=stdout, stderr=stderr)
        try:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise EvidenceError("non-development candidate exited during the launch probe")
                time.sleep(0.1)
        finally:
            _terminate(process)
    package_after = common.tree_hash(app)
    if package_after != package_before:
        raise EvidenceError("candidate application changed during the non-development probe")
    probe = {
        "schema_version": SCHEMA_VERSION,
        "scope": PROBE_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "certification_complete": False,
        "form": FORM,
        "source_revision": binding["source_revision"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "app_tree_sha256_before": package_before,
        "app_tree_sha256_after": package_after,
        "launch_argv": command,
        "dev_tools_enabled": False,
        "network_denial": {
            "mechanism": "sandbox-exec deny network*",
            "enforced_for_launch": True,
            "passed": True,
        },
        "stdout": common.file_record(stdout_path),
        "stderr": common.file_record(stderr_path),
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            "startup does not prove preview, export, print, PDF, or rollback behavior",
            "Accessibility, printer completion, notarization, and stapling remain unverified",
        ],
    }
    common.write_json_atomic(output_dir / "macos-candidate-nondev-probe.json", probe)
    return probe


def _validate_gate(value: Any, label: str, *, base: Path, verified: list[dict[str, Any]]) -> dict[str, Any]:
    gate = require_exact_keys(value, {"exercised", "passed", "artifact"}, label)
    require_true(gate["exercised"], f"{label}.exercised")
    require_true(gate["passed"], f"{label}.passed")
    verify_file_record(gate["artifact"], f"{label}.artifact", base=base, verified=verified)
    return gate


def _validate_geometry(measurements: Any) -> None:
    if not isinstance(measurements, list) or len(measurements) != 2:
        raise EvidenceError("preview must retain exactly two stable geometry measurements")
    normalized: list[dict[str, Any]] = []
    for index, raw in enumerate(measurements, 1):
        measurement = require_exact_keys(
            raw,
            {"measurement_index", "page_width_pt", "page_height_pt", "pages", "clipping_count", "overflow_count"},
            f"geometry measurement {index}",
        )
        if measurement["measurement_index"] != index:
            raise EvidenceError("geometry measurement indices are not canonical")
        if measurement["page_width_pt"] != EXPECTED_WIDTH_POINTS or measurement[
            "page_height_pt"
        ] != EXPECTED_HEIGHT_POINTS:
            raise EvidenceError("preview paper geometry is not 612 x 936 points")
        if measurement["clipping_count"] != 0 or measurement["overflow_count"] != 0:
            raise EvidenceError("preview geometry contains clipping or overflow")
        pages = measurement["pages"]
        if not isinstance(pages, list) or len(pages) != EXPECTED_PAGE_COUNT:
            raise EvidenceError("preview geometry must cover both 2551Q pages")
        for page_number, page in enumerate(pages, 1):
            page = require_exact_keys(
                page, {"page", "x", "y", "width_pt", "height_pt"}, f"preview page {page_number}"
            )
            if page["page"] != page_number:
                raise EvidenceError("preview page numbers are not canonical")
            numbers = [page[key] for key in ("x", "y", "width_pt", "height_pt")]
            if not all(isinstance(number, (int, float)) and math.isfinite(number) for number in numbers):
                raise EvidenceError("preview page geometry contains non-finite values")
            if page["width_pt"] != EXPECTED_WIDTH_POINTS or page["height_pt"] != EXPECTED_HEIGHT_POINTS:
                raise EvidenceError("preview page rectangle has incorrect paper geometry")
        normalized.append({key: value for key, value in measurement.items() if key != "measurement_index"})
    if normalized[0] != normalized[1]:
        raise EvidenceError("preview geometry measurements are not identical")


def validate_attestation(
    attestation_path: Path,
    binding: dict[str, Any],
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    attestation_path = regular_file(attestation_path, "macOS certification attestation")
    base = attestation_path.parent
    attestation = load_json(attestation_path)
    verified: list[dict[str, Any]] = []
    top_keys = {
        "schema_version",
        "scope",
        "promotion_eligible",
        "trusted_producer",
        "operator_only",
        "attestation_id",
        "form",
        "candidate",
        "collector",
        "accessibility",
        "runtime",
        "preview",
        "toolbar_export",
        "native_print",
        "pdf_validation",
        "package_security",
        "integrity",
        "rollback",
        "strict_verifier_gaps",
    }
    require_exact_keys(attestation, top_keys, "macOS certification attestation")
    if attestation["schema_version"] != 1 or attestation["scope"] != ATTESTATION_SCOPE:
        raise EvidenceError("macOS attestation has an unsupported schema or scope")
    if attestation["promotion_eligible"] is not False or attestation["trusted_producer"] is not False:
        raise EvidenceError("macOS attestation must remain non-promotional and untrusted")
    require_true(attestation["operator_only"], "attestation.operator_only")
    try:
        attestation_id = str(uuid.UUID(str(attestation["attestation_id"])))
    except (ValueError, AttributeError) as error:
        raise EvidenceError("attestation_id must be a canonical UUID") from error
    if attestation_id != attestation["attestation_id"]:
        raise EvidenceError("attestation_id must be a canonical lowercase UUID")
    if attestation["form"] != FORM:
        raise EvidenceError("macOS attestation must target exactly 2551Q:2018")

    candidate = require_exact_keys(
        attestation["candidate"],
        {"candidate_manifest_sha256", "candidate_archive_sha256", "source_revision", "app_tree_sha256", "renderer_bundle_sha256"},
        "attestation candidate binding",
    )
    expected_candidate = {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "app_tree_sha256": binding["packaged_app"]["app_tree_sha256"],
        "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
    }
    if candidate != expected_candidate:
        raise EvidenceError("attestation candidate binding differs from the exact workflow artifact")

    collector = require_exact_keys(
        attestation["collector"],
        {"name", "version", "invocation_id", "started_at_utc", "completed_at_utc", "executable_sha256", "host_identifier_sha256"},
        "external collector",
    )
    for field in ("name", "version", "invocation_id"):
        require_nonempty_string(collector[field], f"collector.{field}")
    for field in ("started_at_utc", "completed_at_utc"):
        if not isinstance(collector[field], str) or RFC3339_UTC.fullmatch(collector[field]) is None:
            raise EvidenceError(f"collector.{field} must be an RFC3339 UTC timestamp")
    try:
        started_at = datetime.fromisoformat(
            collector["started_at_utc"].replace("Z", "+00:00")
        )
        completed_at = datetime.fromisoformat(
            collector["completed_at_utc"].replace("Z", "+00:00")
        )
    except ValueError as error:
        raise EvidenceError("collector timestamps are not valid calendar times") from error
    if completed_at <= started_at:
        raise EvidenceError("collector completion time must be after its start time")
    require_sha256(collector["executable_sha256"], "collector executable hash")
    require_sha256(collector["host_identifier_sha256"], "collector host identifier hash")

    accessibility = require_exact_keys(
        attestation["accessibility"], {"permission_granted", "automation_identity", "artifact"}, "Accessibility evidence"
    )
    require_true(accessibility["permission_granted"], "Accessibility permission")
    require_nonempty_string(accessibility["automation_identity"], "Accessibility automation identity")
    verify_file_record(accessibility["artifact"], "Accessibility artifact", base=base, verified=verified)

    runtime = require_exact_keys(
        attestation["runtime"],
        {"non_dev_build", "dev_tools_enabled", "launch_argv", "pid", "network_denial", "artifact"},
        "non-development runtime",
    )
    require_true(runtime["non_dev_build"], "runtime.non_dev_build")
    if runtime["dev_tools_enabled"] is not False:
        raise EvidenceError("certification runtime must not enable dev-tools")
    argv = runtime["launch_argv"]
    if not isinstance(argv, list) or not argv or not all(isinstance(item, str) and item for item in argv):
        raise EvidenceError("runtime.launch_argv must contain the exact launched command")
    if common.DEV_FLAG in argv or any("dev-tools" in item for item in argv):
        raise EvidenceError("certification runtime used a development-only launch path")
    if not isinstance(runtime["pid"], int) or runtime["pid"] <= 0:
        raise EvidenceError("runtime PID must be a positive integer")
    network = require_exact_keys(
        runtime["network_denial"], {"mechanism", "exercised", "enforced_for_launch", "passed", "artifact"}, "network denial"
    )
    if "deny network" not in require_nonempty_string(network["mechanism"], "network denial mechanism"):
        raise EvidenceError("network denial mechanism does not deny networking")
    for field in ("exercised", "enforced_for_launch", "passed"):
        require_true(network[field], f"network_denial.{field}")
    verify_file_record(network["artifact"], "network-denial artifact", base=base, verified=verified)
    verify_file_record(runtime["artifact"], "runtime artifact", base=base, verified=verified)

    preview = require_exact_keys(
        attestation["preview"],
        {"exercised", "passed", "window_title", "document_run_id", "envelope_sha256", "nonce", "page_count", "geometry_measurements", "artifact"},
        "native preview",
    )
    require_true(preview["exercised"], "preview.exercised")
    require_true(preview["passed"], "preview.passed")
    if "2551Q HTML Form Preview" not in require_nonempty_string(preview["window_title"], "preview.window_title"):
        raise EvidenceError("preview attestation did not identify the exact 2551Q window")
    require_nonempty_string(preview["document_run_id"], "preview.document_run_id")
    require_sha256(preview["envelope_sha256"], "preview envelope hash")
    if not isinstance(preview["nonce"], int) or preview["nonce"] <= 0:
        raise EvidenceError("preview nonce must be positive")
    if preview["page_count"] != EXPECTED_PAGE_COUNT:
        raise EvidenceError("preview must contain exactly two pages")
    _validate_geometry(preview["geometry_measurements"])
    verify_file_record(preview["artifact"], "preview artifact", base=base, verified=verified)

    toolbar = require_exact_keys(
        attestation["toolbar_export"],
        {"exercised", "passed", "control", "save_chooser_exercised", "destination_path", "nonce", "artifact"},
        "toolbar Export PDF",
    )
    require_true(toolbar["exercised"], "toolbar_export.exercised")
    require_true(toolbar["passed"], "toolbar_export.passed")
    if toolbar["control"] != "Export PDF":
        raise EvidenceError("toolbar attestation did not exercise Export PDF")
    require_true(toolbar["save_chooser_exercised"], "toolbar save chooser")
    require_nonempty_string(toolbar["destination_path"], "toolbar export destination")
    if toolbar["nonce"] != preview["nonce"]:
        raise EvidenceError("toolbar export nonce differs from the preview run")
    verify_file_record(toolbar["artifact"], "toolbar/export artifact", base=base, verified=verified)

    native_print = require_exact_keys(
        attestation["native_print"],
        {"exercised", "passed", "completed", "printer_name", "job_id", "artifact"},
        "native system print",
    )
    for field in ("exercised", "passed", "completed"):
        require_true(native_print[field], f"native_print.{field}")
    require_nonempty_string(native_print["printer_name"], "native printer name")
    require_nonempty_string(native_print["job_id"], "native print job id")
    verify_file_record(native_print["artifact"], "native-print artifact", base=base, verified=verified)

    pdf = require_exact_keys(
        attestation["pdf_validation"],
        {
            "exercised",
            "passed",
            "output",
            "expected_page_count",
            "actual_page_count",
            "pages",
            "content_nonempty",
            "validated_by",
            "verifier_executable_sha256",
            "artifact",
        },
        "PDF validation",
    )
    require_true(pdf["exercised"], "pdf_validation.exercised")
    require_true(pdf["passed"], "pdf_validation.passed")
    if pdf["expected_page_count"] != EXPECTED_PAGE_COUNT or pdf["actual_page_count"] != EXPECTED_PAGE_COUNT:
        raise EvidenceError("exported PDF page count is not exactly two")
    require_true(pdf["content_nonempty"], "pdf_validation.content_nonempty")
    if pdf["validated_by"] != "bir-print::html_output::validate_pdf_file":
        raise EvidenceError("PDF was not validated by the owned Rust PDF verifier")
    require_sha256(
        pdf["verifier_executable_sha256"], "PDF verifier executable hash"
    )
    output_pdf = verify_file_record(pdf["output"], "exported PDF", base=base, verified=verified)
    if str(Path(toolbar["destination_path"]).resolve()) != output_pdf["path"]:
        raise EvidenceError("toolbar destination differs from the validated PDF")
    pages = pdf["pages"]
    if not isinstance(pages, list) or len(pages) != EXPECTED_PAGE_COUNT:
        raise EvidenceError("PDF geometry must cover both pages")
    for page_number, raw in enumerate(pages, 1):
        page = require_exact_keys(
            raw,
            {"page", "media_width_pt", "media_height_pt", "crop_width_pt", "crop_height_pt", "rotation", "content_byte_count"},
            f"PDF page {page_number}",
        )
        if page["page"] != page_number:
            raise EvidenceError("PDF page numbers are not canonical")
        if (
            page["media_width_pt"] != EXPECTED_WIDTH_POINTS
            or page["media_height_pt"] != EXPECTED_HEIGHT_POINTS
            or page["crop_width_pt"] != EXPECTED_WIDTH_POINTS
            or page["crop_height_pt"] != EXPECTED_HEIGHT_POINTS
            or page["rotation"] != 0
            or not isinstance(page["content_byte_count"], int)
            or page["content_byte_count"] <= 0
        ):
            raise EvidenceError(f"PDF page {page_number} geometry/content is invalid")
    verify_file_record(pdf["artifact"], "PDF-verifier artifact", base=base, verified=verified)

    security = require_exact_keys(
        attestation["package_security"], {"codesign", "notarization", "stapling"}, "package security"
    )
    codesign = require_exact_keys(
        security["codesign"], {"passed", "developer_id_signed", "authority", "team_identifier", "artifact"}, "codesign evidence"
    )
    require_true(codesign["passed"], "codesign.passed")
    require_true(codesign["developer_id_signed"], "codesign.developer_id_signed")
    require_nonempty_string(codesign["authority"], "codesign authority")
    require_nonempty_string(codesign["team_identifier"], "codesign team identifier")
    verify_file_record(codesign["artifact"], "codesign artifact", base=base, verified=verified)
    notarization = require_exact_keys(
        security["notarization"], {"passed", "gatekeeper_accepted", "artifact"}, "notarization evidence"
    )
    require_true(notarization["passed"], "notarization.passed")
    require_true(notarization["gatekeeper_accepted"], "notarization.gatekeeper_accepted")
    verify_file_record(notarization["artifact"], "notarization artifact", base=base, verified=verified)
    stapling = require_exact_keys(security["stapling"], {"passed", "artifact"}, "stapling evidence")
    require_true(stapling["passed"], "stapling.passed")
    verify_file_record(stapling["artifact"], "stapling artifact", base=base, verified=verified)

    integrity = require_exact_keys(
        attestation["integrity"],
        {"app_tree_sha256_before", "app_tree_sha256_after", "destination_before", "destination_after", "draft_before", "draft_after", "temporary_files_manifest"},
        "state integrity",
    )
    expected_app_hash = binding["packaged_app"]["app_tree_sha256"]
    if integrity["app_tree_sha256_before"] != expected_app_hash or integrity[
        "app_tree_sha256_after"
    ] != expected_app_hash:
        raise EvidenceError("application package changed during certification")
    before_destination = verify_file_record(
        integrity["destination_before"], "destination-before snapshot", base=base, verified=verified
    )
    after_destination = verify_file_record(
        integrity["destination_after"], "destination-after snapshot", base=base, verified=verified
    )
    before_draft = verify_file_record(integrity["draft_before"], "draft-before snapshot", base=base, verified=verified)
    after_draft = verify_file_record(integrity["draft_after"], "draft-after snapshot", base=base, verified=verified)
    for before, after, label in (
        (before_destination, after_destination, "destination"),
        (before_draft, after_draft, "draft"),
    ):
        if before["path"] == after["path"]:
            raise EvidenceError(f"{label} snapshots must be retained as distinct files")
        if before["sha256"] != after["sha256"]:
            raise EvidenceError(f"{label} changed during the failed-output drill")
    temp_record = verify_file_record(
        integrity["temporary_files_manifest"], "temporary-files manifest", base=base, verified=verified
    )
    if load_json(Path(temp_record["path"]), limit=1024 * 1024) != {"remaining": []}:
        raise EvidenceError("temporary-files manifest reports leaked output files")

    rollback = require_exact_keys(
        attestation["rollback"], {"cases", "destination_preserved", "temporary_files_remaining", "draft_unchanged"}, "rollback drill"
    )
    require_true(rollback["destination_preserved"], "rollback.destination_preserved")
    require_true(rollback["draft_unchanged"], "rollback.draft_unchanged")
    if rollback["temporary_files_remaining"] != 0:
        raise EvidenceError("rollback drill left temporary files")
    cases = rollback["cases"]
    if not isinstance(cases, list):
        raise EvidenceError("rollback.cases must be an array")
    seen: set[str] = set()
    for index, raw in enumerate(cases):
        case = require_exact_keys(raw, {"name", "passed", "artifact"}, f"rollback case {index}")
        name = case["name"]
        if name not in ROLLBACK_CASES or name in seen:
            raise EvidenceError(f"rollback case is unknown or duplicated: {name!r}")
        seen.add(name)
        require_true(case["passed"], f"rollback case {name}")
        verify_file_record(case["artifact"], f"rollback artifact {name}", base=base, verified=verified)
    missing = sorted(ROLLBACK_CASES - seen)
    if missing:
        raise EvidenceError(f"rollback attestation is incomplete: {', '.join(missing)}")

    gaps = attestation["strict_verifier_gaps"]
    if not isinstance(gaps, list) or NON_PROMOTIONAL_GAP not in gaps:
        raise EvidenceError("attestation erased the untrusted-producer promotion blocker")
    return attestation, verified


def verify_owned_pdf_artifact(
    attestation_path: Path,
    attestation: dict[str, Any],
    verifier_path: Path,
    verified: list[dict[str, Any]],
) -> dict[str, Any]:
    verifier_path = regular_file(verifier_path, "owned PDF verifier")
    verifier_record = common.file_record(verifier_path)
    if verifier_record["sha256"] != attestation["pdf_validation"][
        "verifier_executable_sha256"
    ]:
        raise EvidenceError("owned PDF verifier differs from its attested executable hash")
    if not os.access(verifier_path, os.X_OK):
        raise EvidenceError("owned PDF verifier is not executable")

    base = attestation_path.resolve(strict=True).parent
    output_pdf = resolve_record_path(attestation["pdf_validation"]["output"], base).resolve(
        strict=True
    )
    artifact_path = resolve_record_path(
        attestation["pdf_validation"]["artifact"], base
    ).resolve(strict=True)
    try:
        result = subprocess.run(
            [
                str(verifier_path),
                str(output_pdf),
                attestation["preview"]["envelope_sha256"],
            ],
            capture_output=True,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"owned PDF verifier is unavailable: {error}") from error
    if result.returncode != 0:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise EvidenceError(f"owned PDF verifier rejected the export: {message}")
    if result.stderr:
        raise EvidenceError("owned PDF verifier emitted unexpected stderr output")
    recorded_output = common.read_stable_file(artifact_path, limit=4 * 1024 * 1024)
    if recorded_output != result.stdout:
        raise EvidenceError(
            "retained PDF-verifier artifact differs from a fresh owned validation run"
        )
    try:
        report = json.loads(result.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"owned PDF verifier returned invalid JSON: {error}") from error
    require_exact_keys(
        report,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "form",
            "envelope_sha256",
            "output_sha256",
            "expected_page_count",
            "actual_page_count",
            "width_points",
            "height_points",
            "content_nonempty",
            "validated_by",
            "pages",
        },
        "owned PDF validation report",
    )
    expected = {
        "schema_version": 1,
        "scope": "owned_macos_candidate_pdf_validation",
        "promotion_eligible": False,
        "form": FORM,
        "envelope_sha256": attestation["preview"]["envelope_sha256"],
        "output_sha256": attestation["pdf_validation"]["output"]["sha256"],
        "expected_page_count": EXPECTED_PAGE_COUNT,
        "actual_page_count": EXPECTED_PAGE_COUNT,
        "width_points": EXPECTED_WIDTH_POINTS,
        "height_points": EXPECTED_HEIGHT_POINTS,
        "content_nonempty": True,
        "validated_by": "bir-print::html_output::validate_pdf_file",
        "pages": attestation["pdf_validation"]["pages"],
    }
    if report != expected:
        raise EvidenceError(
            "owned PDF validation report differs from the immutable attestation"
        )
    verified.append(verifier_record | {"label": "owned PDF verifier executable"})
    return {
        "verifier_executable_sha256": verifier_record["sha256"],
        "output_sha256": report["output_sha256"],
        "actual_page_count": report["actual_page_count"],
        "width_points": report["width_points"],
        "height_points": report["height_points"],
        "content_nonempty": report["content_nonempty"],
    }


def _run_required(command: list[str], label: str) -> str:
    try:
        result = subprocess.run(command, text=True, capture_output=True, check=False, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"{label} failed closed: {output}")
    return output


def verify_live_macos_state(app: Path, attestation: dict[str, Any]) -> dict[str, Any]:
    if platform.system() != "Darwin":
        raise EvidenceError("strict macOS attestation verification must run on macOS")
    accessibility = _run_required(
        ["/usr/bin/osascript", "-e", 'tell application "System Events" to get UI elements enabled'],
        "macOS Accessibility state",
    )
    if accessibility.strip().lower() != "true":
        raise EvidenceError("macOS Accessibility permission is unavailable")
    printer_name = attestation["native_print"]["printer_name"]
    printer_state = _run_required(["/usr/bin/lpstat", "-p", printer_name], "configured printer state")
    if "disabled" in printer_state.lower():
        raise EvidenceError("attested printer is disabled")
    print_job_id = attestation["native_print"]["job_id"]
    completed_jobs = _run_required(
        ["/usr/bin/lpstat", "-W", "completed", "-o", printer_name],
        "completed native print job",
    )
    completed_job_ids = {
        line.split(maxsplit=1)[0]
        for line in completed_jobs.splitlines()
        if line.strip()
    }
    if print_job_id not in completed_job_ids:
        raise EvidenceError("attested native print job is not present in completed CUPS jobs")
    signature = common.codesign_record(app)
    if not signature["developer_id_signed"] or not signature.get("team_identifier"):
        raise EvidenceError("candidate is not Developer ID signed")
    if signature["team_identifier"] != attestation["package_security"]["codesign"]["team_identifier"]:
        raise EvidenceError("live Developer ID team differs from the attestation")
    authority = attestation["package_security"]["codesign"]["authority"]
    if authority not in signature["authority"]:
        raise EvidenceError("live Developer ID authority differs from the attestation")
    gatekeeper = _run_required(
        ["/usr/sbin/spctl", "--assess", "--type", "execute", "--verbose=4", str(app)],
        "Gatekeeper notarization assessment",
    )
    if "notarized developer id" not in gatekeeper.lower():
        raise EvidenceError("Gatekeeper did not identify a notarized Developer ID package")
    stapling = _run_required(
        ["/usr/bin/xcrun", "stapler", "validate", str(app)], "stapled notarization ticket"
    )
    return {
        "accessibility_permission_granted": True,
        "printer_available": True,
        "printer_name": printer_name,
        "completed_print_job_verified": True,
        "print_job_output_sha256": common.sha256_bytes(completed_jobs.encode("utf-8")),
        "developer_id_signed": True,
        "team_identifier": signature["team_identifier"],
        "gatekeeper_notarization_verified": True,
        "stapling_verified": True,
        "gatekeeper_output_sha256": common.sha256_bytes(gatekeeper.encode("utf-8")),
        "stapler_output_sha256": common.sha256_bytes(stapling.encode("utf-8")),
    }


def verify_attestation_command(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    attestation_path: Path,
    pdf_verifier_path: Path,
    report_path: Path,
) -> dict[str, Any]:
    candidate = validate_candidate_inputs(manifest_path, archive_path, identity_path)

    with tempfile.TemporaryDirectory(prefix="ebirforms-macos-certification-") as directory:
        app = extract_candidate_archive(archive_path, Path(directory))
        packaged = bind_extracted_app(candidate, app, identity_path)
        binding = {
            **candidate,
            "packaged_app": packaged,
        }
        attestation, verified = validate_attestation(attestation_path, binding)
        owned_pdf_validation = verify_owned_pdf_artifact(
            attestation_path,
            attestation,
            pdf_verifier_path,
            verified,
        )
        live = verify_live_macos_state(app, attestation)
        if common.tree_hash(app) != packaged["app_tree_sha256"]:
            raise EvidenceError("candidate changed during live macOS verification")

    report = {
        "schema_version": SCHEMA_VERSION,
        "scope": REPORT_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "operator_only": True,
        "foundation_verification_passed": True,
        "promotion_satisfied": False,
        "form": FORM,
        "source_revision": candidate["source_revision"],
        "candidate_manifest": candidate["candidate_manifest"],
        "candidate_archive": candidate["candidate_archive"],
        "renderer_identity": candidate["renderer_identity"],
        "attestation": common.file_record(attestation_path),
        "verified_artifact_count": len(verified),
        "owned_pdf_validation": owned_pdf_validation,
        "live_macos_verification": live,
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            "collector executable and operator identity are not externally attested",
            "Windows and Linux native certification remain incomplete",
        ],
    }
    common.write_json_atomic(report_path, report)
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)

    inspect = subcommands.add_parser("inspect", help="verify and extract the exact candidate")
    inspect.add_argument("--candidate-manifest", required=True, type=Path)
    inspect.add_argument("--candidate-archive", required=True, type=Path)
    inspect.add_argument("--renderer-identity", required=True, type=Path)
    inspect.add_argument("--output-dir", required=True, type=Path)

    probe = subcommands.add_parser("probe", help="launch the actual non-dev app with networking denied")
    probe.add_argument("--candidate-manifest", required=True, type=Path)
    probe.add_argument("--candidate-archive", required=True, type=Path)
    probe.add_argument("--renderer-identity", required=True, type=Path)
    probe.add_argument("--output-dir", required=True, type=Path)
    probe.add_argument("--timeout", type=float, default=5.0)

    verify = subcommands.add_parser(
        "verify-attestation", help="strictly verify a complete external macOS attestation"
    )
    verify.add_argument("--candidate-manifest", required=True, type=Path)
    verify.add_argument("--candidate-archive", required=True, type=Path)
    verify.add_argument("--renderer-identity", required=True, type=Path)
    verify.add_argument("--attestation", required=True, type=Path)
    verify.add_argument("--pdf-verifier", required=True, type=Path)
    verify.add_argument("--report", required=True, type=Path)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        if arguments.command in {"inspect", "probe"}:
            binding = inspect_candidate(
                arguments.candidate_manifest,
                arguments.candidate_archive,
                arguments.renderer_identity,
                arguments.output_dir,
            )
            if arguments.command == "probe":
                probe_nondev_candidate(binding, arguments.output_dir.resolve(), arguments.timeout)
            print(arguments.output_dir.resolve())
            return 0
        verify_attestation_command(
            arguments.candidate_manifest,
            arguments.candidate_archive,
            arguments.renderer_identity,
            arguments.attestation,
            arguments.pdf_verifier,
            arguments.report,
        )
        print(arguments.report.resolve())
        return 0
    except (EvidenceError, OSError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        print(f"macOS candidate certification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
