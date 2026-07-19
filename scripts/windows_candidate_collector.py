#!/usr/bin/env python3
"""Collect a closed, non-promotional Windows candidate attestation.

This operator-only collector securely extracts the exact workflow candidate,
requires a valid timestamped Authenticode signature, launches that exact
``bir.exe`` with outbound Firewall blocks for both the app and WebView2,
drives the real 2551Q preview through exact-process Windows UI Automation,
exercises Export PDF and its native Save chooser, and—only after explicit
operator consent—submits a real job to the named default printer.

Facts that UI Automation cannot prove (renderer envelope/nonce/geometry and
WebView2 completion HRESULTs/interfaces) must arrive during the run from a
separate, fresh challenge-bound runtime witness. Rollback evidence is likewise
supplied as a distinct exact-candidate bundle. Missing or mismatched evidence
fails closed. The resulting attestation is permanently untrusted,
operator-only, and non-promotional.
"""

from __future__ import annotations

import argparse
import base64
import getpass
import hashlib
import json
import math
import os
import platform
import re
import secrets
import shutil
import signal
import subprocess
import sys
import time
import uuid
import zipfile
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import windows_candidate_certification as certification  # noqa: E402


SCHEMA_VERSION = 1
COLLECTOR_NAME = "ebirforms external Windows candidate collector"
COLLECTOR_VERSION = "1"
ROLLBACK_SCOPE = "external_windows_candidate_rollback_bundle"
RUNTIME_SCOPE = "external_windows_candidate_runtime_observation"
FORM = certification.FORM
RUNTIME_GAPS = [
    "runtime witness producer is not registered as trusted",
    "external UI Automation and printer evidence are required",
    "external exact-candidate binding is required",
]
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
RFC3339_UTC = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\Z")


EvidenceError = certification.EvidenceError
artifact_common = certification.artifact_common


def utc_now() -> str:
    return datetime.now(UTC).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def require_exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    return certification.require_exact_keys(value, keys, label)


def require_sha256(value: Any, label: str) -> str:
    return certification.require_sha256(value, label)


def require_nonempty(value: Any, label: str) -> str:
    return certification.require_nonempty_string(value, label)


def validate_timestamp_range(started: Any, completed: Any, label: str) -> None:
    for value, field in ((started, "started"), (completed, "completed")):
        if not isinstance(value, str) or RFC3339_UTC.fullmatch(value) is None:
            raise EvidenceError(f"{label} {field} timestamp must be RFC3339 UTC")
    try:
        start = datetime.fromisoformat(started.replace("Z", "+00:00"))
        finish = datetime.fromisoformat(completed.replace("Z", "+00:00"))
    except ValueError as error:
        raise EvidenceError(f"{label} timestamps are invalid") from error
    if finish <= start:
        raise EvidenceError(f"{label} completion must follow its start")


def reject_symlink_components(path: Path) -> None:
    absolute = path.absolute()
    for ancestor in (absolute, *absolute.parents):
        if ancestor.exists() and ancestor.is_symlink():
            raise EvidenceError("collector paths may not traverse symlinks")


def private_output_directory(path: Path) -> Path:
    reject_symlink_components(path)
    if path.exists():
        if not path.is_dir() or any(path.iterdir()):
            raise EvidenceError("collector output directory must be absent or empty")
    else:
        path.mkdir(parents=True, mode=0o700)
    try:
        path.chmod(0o700)
    except OSError:
        # Windows ACLs are locked down separately before any taxpayer image is
        # retained. chmod remains useful for non-Windows unit tests.
        pass
    return path.resolve(strict=True)


def write_bytes(path: Path, payload: bytes) -> None:
    temporary = path.with_name(f".{path.name}.{uuid.uuid4()}.partial")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        try:
            path.chmod(0o600)
        except OSError:
            pass
    finally:
        if temporary.exists():
            temporary.unlink()


def write_text(path: Path, value: str) -> None:
    write_bytes(path, value.encode("utf-8"))


def write_json(path: Path, value: dict[str, Any]) -> None:
    write_text(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def file_record(path: Path) -> dict[str, Any]:
    return artifact_common.file_record(path)


def candidate_binding(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    extraction_root: Path,
) -> dict[str, Any]:
    candidate = certification.validate_candidate_inputs(
        manifest_path, archive_path, identity_path
    )
    package = certification.certification_common.extract_portable_zip(
        archive_path, extraction_root
    )
    packaged = certification.bind_extracted_package(candidate, package, identity_path)
    return {**candidate, "packaged_app": packaged}


def expected_candidate(binding: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "package_tree_sha256": binding["packaged_app"]["package_tree_sha256"],
        "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
        "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
    }


def load_rollback_bundle(path: Path, binding: dict[str, Any]) -> dict[str, Any]:
    path = certification.regular_file(path, "Windows rollback evidence bundle")
    bundle = certification.load_json(path)
    require_exact_keys(
        bundle,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "candidate",
            "integrity",
            "cases",
            "strict_verifier_gaps",
        },
        "Windows rollback evidence bundle",
    )
    if bundle["schema_version"] != 1 or bundle["scope"] != ROLLBACK_SCOPE:
        raise EvidenceError("rollback bundle has an unsupported schema or scope")
    if bundle["promotion_eligible"] is not False or bundle["trusted_producer"] is not False:
        raise EvidenceError("rollback bundle must remain non-promotional and untrusted")
    if bundle["candidate"] != expected_candidate(binding):
        raise EvidenceError("rollback bundle does not bind the exact Windows candidate")

    base = path.parent
    integrity = require_exact_keys(
        bundle["integrity"],
        {
            "destination_before",
            "destination_after",
            "draft_before",
            "draft_after",
            "temporary_files_manifest",
        },
        "Windows rollback integrity",
    )
    normalized: dict[str, dict[str, Any]] = {}
    for field, record in integrity.items():
        normalized[field] = certification.verify_file_record(
            record, f"rollback {field}", base=base
        )
    for before_name, after_name, label in (
        ("destination_before", "destination_after", "destination"),
        ("draft_before", "draft_after", "draft"),
    ):
        before = normalized[before_name]
        after = normalized[after_name]
        if before["path"] == after["path"] or before["sha256"] != after["sha256"]:
            raise EvidenceError(f"rollback {label} snapshots are not distinct and preserved")
    temporary = certification.load_json(
        Path(normalized["temporary_files_manifest"]["path"]), limit=1024 * 1024
    )
    if temporary != {"remaining": []}:
        raise EvidenceError("rollback temporary-files manifest reports leaked files")

    cases = bundle["cases"]
    if not isinstance(cases, list):
        raise EvidenceError("rollback cases must be an array")
    seen: set[str] = set()
    normalized_cases: list[dict[str, Any]] = []
    for index, raw in enumerate(cases):
        case = require_exact_keys(raw, {"name", "passed", "artifact"}, f"rollback case {index}")
        name = case["name"]
        if name not in certification.ROLLBACK_CASES or name in seen:
            raise EvidenceError(f"rollback case is unknown or duplicated: {name!r}")
        if case["passed"] is not True:
            raise EvidenceError(f"rollback case did not pass: {name}")
        seen.add(name)
        normalized_cases.append(
            {
                "name": name,
                "passed": True,
                "artifact": certification.verify_file_record(
                    case["artifact"], f"rollback case {name}", base=base
                ),
            }
        )
    missing = sorted(certification.ROLLBACK_CASES - seen)
    if missing:
        raise EvidenceError(f"rollback evidence is incomplete: {', '.join(missing)}")
    gaps = bundle["strict_verifier_gaps"]
    if (
        not isinstance(gaps, list)
        or not gaps
        or not all(isinstance(item, str) and item for item in gaps)
    ):
        raise EvidenceError("rollback evidence must retain explicit verifier gaps")
    return {
        "integrity": normalized,
        "cases": sorted(normalized_cases, key=lambda item: item["name"]),
        "strict_verifier_gaps": gaps,
        "bundle": file_record(path),
    }


def validate_finite_number(value: Any, label: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or (positive and number <= 0):
        raise EvidenceError(f"{label} must be finite" + (" and positive" if positive else ""))
    return number


def validate_geometry_measurements(value: Any) -> list[dict[str, Any]]:
    if not isinstance(value, list) or len(value) != 2:
        raise EvidenceError("runtime witness requires exactly two geometry measurements")
    normalized: list[dict[str, Any]] = []
    for index, raw in enumerate(value, 1):
        measurement = require_exact_keys(
            raw,
            {
                "measurement_index",
                "page_width_pt",
                "page_height_pt",
                "pages",
                "clipping_count",
                "overflow_count",
            },
            f"runtime geometry measurement {index}",
        )
        if measurement["measurement_index"] != index:
            raise EvidenceError("runtime geometry indices are not canonical")
        if measurement["page_width_pt"] != 612.0 or measurement["page_height_pt"] != 936.0:
            raise EvidenceError("runtime witness paper geometry is not 612 x 936 points")
        if measurement["clipping_count"] != 0 or measurement["overflow_count"] != 0:
            raise EvidenceError("runtime witness reports clipping or overflow")
        pages = measurement["pages"]
        if not isinstance(pages, list) or len(pages) != 2:
            raise EvidenceError("runtime witness must retain exactly two page rectangles")
        for page_number, page in enumerate(pages, 1):
            page = require_exact_keys(
                page, {"page", "x", "y", "width_pt", "height_pt"}, "runtime page rectangle"
            )
            if page["page"] != page_number:
                raise EvidenceError("runtime page rectangles are not canonical")
            validate_finite_number(page["x"], "runtime page x")
            validate_finite_number(page["y"], "runtime page y")
            if (
                validate_finite_number(page["width_pt"], "runtime page width", positive=True)
                != 612.0
                or validate_finite_number(page["height_pt"], "runtime page height", positive=True)
                != 936.0
            ):
                raise EvidenceError("runtime page rectangle has incorrect paper geometry")
        normalized.append(
            {key: item for key, item in measurement.items() if key != "measurement_index"}
        )
    if normalized[0] != normalized[1]:
        raise EvidenceError("runtime geometry measurements are not identical")
    return value


def validate_runtime_observation(
    path: Path,
    *,
    binding: dict[str, Any],
    challenge_sha256: str,
    pid: int,
    output_pdf: Path,
    destination_before_sha256: str,
    webview2_executable: Path,
    printer_name: str,
    witness_executable: Path,
) -> dict[str, Any]:
    path = certification.regular_file(path, "Windows candidate runtime observation")
    observation = certification.load_json(path, limit=4 * 1024 * 1024)
    require_exact_keys(
        observation,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "collector_challenge_sha256",
            "witness_name",
            "witness_version",
            "witness_executable_sha256",
            "candidate",
            "pid",
            "form",
            "non_dev_build",
            "dev_tools_enabled",
            "started_at_utc",
            "completed_at_utc",
            "document_run_id",
            "envelope_sha256",
            "preview_nonce",
            "print_nonce",
            "geometry_measurements",
            "export",
            "print",
            "webview2",
            "dependencies",
            "strict_verifier_gaps",
        },
        "Windows candidate runtime observation",
    )
    constants = (
        observation["schema_version"] == 1,
        observation["scope"] == RUNTIME_SCOPE,
        observation["promotion_eligible"] is False,
        observation["trusted_producer"] is False,
        observation["collector_challenge_sha256"] == challenge_sha256,
        observation["candidate"] == expected_candidate(binding),
        observation["pid"] == pid,
        observation["form"] == FORM,
        observation["non_dev_build"] is True,
        observation["dev_tools_enabled"] is False,
        observation["strict_verifier_gaps"] == RUNTIME_GAPS,
    )
    if not all(constants):
        raise EvidenceError("runtime observation constants, challenge, PID, or candidate differ")
    require_sha256(observation["collector_challenge_sha256"], "runtime challenge hash")
    require_nonempty(observation["witness_name"], "runtime witness name")
    require_nonempty(observation["witness_version"], "runtime witness version")
    witness_hash = require_sha256(
        observation["witness_executable_sha256"], "runtime witness executable hash"
    )
    if witness_hash != file_record(witness_executable)["sha256"]:
        raise EvidenceError("runtime observation names another witness executable")
    require_nonempty(observation["document_run_id"], "runtime document run ID")
    require_sha256(observation["envelope_sha256"], "runtime envelope hash")
    validate_timestamp_range(
        observation["started_at_utc"], observation["completed_at_utc"], "runtime observation"
    )
    preview_nonce = observation["preview_nonce"]
    print_nonce = observation["print_nonce"]
    if (
        type(preview_nonce) is not int
        or preview_nonce <= 0
        or type(print_nonce) is not int
        or print_nonce <= 0
        or preview_nonce == print_nonce
    ):
        raise EvidenceError("runtime witness requires distinct positive export and print nonces")
    validate_geometry_measurements(observation["geometry_measurements"])

    output_record = file_record(output_pdf)
    export = require_exact_keys(
        observation["export"],
        {
            "nonce",
            "print_to_pdf_hresult",
            "print_to_pdf_result",
            "destination_before_sha256",
            "output_pdf_sha256",
            "output_pdf_byte_count",
            "temporary_file_remaining",
        },
        "runtime export observation",
    )
    if (
        export["nonce"] != preview_nonce
        or export["print_to_pdf_hresult"] != "S_OK"
        or export["print_to_pdf_result"] is not True
        or export["destination_before_sha256"] != destination_before_sha256
        or export["output_pdf_sha256"] != output_record["sha256"]
        or export["output_pdf_byte_count"] != output_record["byte_count"]
        or export["temporary_file_remaining"] is not False
    ):
        raise EvidenceError("runtime export observation is incomplete or differs from output bytes")
    require_sha256(export["destination_before_sha256"], "destination-before challenge hash")
    require_sha256(export["output_pdf_sha256"], "runtime PDF hash")

    printed = require_exact_keys(
        observation["print"],
        {"nonce", "webview2_print_hresult", "webview2_print_status", "printer_name"},
        "runtime print observation",
    )
    if printed != {
        "nonce": print_nonce,
        "webview2_print_hresult": "S_OK",
        "webview2_print_status": "Succeeded",
        "printer_name": printer_name,
    }:
        raise EvidenceError("runtime print observation is incomplete or names another printer")

    webview2 = require_exact_keys(
        observation["webview2"],
        {
            "runtime_version",
            "channel",
            "architecture",
            "install_scope",
            "executable_sha256",
            "core_webview2_7_available",
            "core_webview2_16_available",
        },
        "runtime WebView2 observation",
    )
    executable_record = file_record(webview2_executable)
    require_nonempty(webview2["runtime_version"], "WebView2 runtime version")
    if (
        webview2["channel"] not in {"stable", "beta", "dev", "canary", "fixed"}
        or webview2["architecture"] != "x86_64"
        or webview2["install_scope"] not in {"per_machine", "per_user", "fixed"}
        or webview2["executable_sha256"] != executable_record["sha256"]
        or webview2["core_webview2_7_available"] is not True
        or webview2["core_webview2_16_available"] is not True
    ):
        raise EvidenceError("runtime WebView2 evidence is incomplete or executable-mismatched")
    dependencies = require_exact_keys(
        observation["dependencies"],
        {"msvc_runtime_loaded", "webview2_loader_bound"},
        "runtime dependency observation",
    )
    if dependencies != {"msvc_runtime_loaded": True, "webview2_loader_bound": True}:
        raise EvidenceError("runtime dependency evidence is incomplete")
    return observation


def wait_for_runtime_observation(
    path: Path, *, timeout: float, validation: dict[str, Any]
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        if path.is_file():
            try:
                return validate_runtime_observation(path, **validation)
            except (EvidenceError, OSError, json.JSONDecodeError) as error:
                last_error = error
        time.sleep(0.2)
    detail = f": {last_error}" if last_error else ""
    raise EvidenceError(f"timed out waiting for the Windows runtime witness{detail}")


def powershell_executable() -> str:
    executable = shutil.which("pwsh") or shutil.which("powershell.exe")
    if executable is None:
        raise EvidenceError("PowerShell is unavailable")
    return executable


def run_powershell(
    source: str,
    label: str,
    *,
    environment: dict[str, str] | None = None,
    timeout: float = 60.0,
) -> str:
    encoded = base64.b64encode(source.encode("utf-16le")).decode("ascii")
    try:
        result = subprocess.run(
            [
                powershell_executable(),
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
                encoded,
            ],
            env=os.environ | (environment or {}),
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    if result.returncode != 0:
        output = (result.stderr or result.stdout).strip()
        raise EvidenceError(f"{label} failed closed: {output}")
    return result.stdout.strip()


def run_powershell_json(
    source: str,
    label: str,
    *,
    environment: dict[str, str] | None = None,
    timeout: float = 60.0,
) -> dict[str, Any]:
    output = run_powershell(source, label, environment=environment, timeout=timeout)
    try:
        value = json.loads(output)
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{label} returned invalid JSON") from error
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} returned a non-object")
    return value


def lock_down_output_directory(path: Path, artifact: Path) -> dict[str, Any]:
    source = r'''
$ErrorActionPreference = 'Stop'
$path = [IO.Path]::GetFullPath($env:EBIR_COLLECTOR_OUTPUT)
$item = Get-Item -LiteralPath $path -Force
if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'collector output directory is a reparse point'
}
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$sid = $identity.User
$acl = New-Object Security.AccessControl.DirectorySecurity
$acl.SetOwner($sid)
$acl.SetAccessRuleProtection($true, $false)
$inheritance = [Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit'
$propagation = [Security.AccessControl.PropagationFlags]::None
$rights = [Security.AccessControl.FileSystemRights]::FullControl
$type = [Security.AccessControl.AccessControlType]::Allow
$rule = New-Object Security.AccessControl.FileSystemAccessRule(
    $sid, $rights, $inheritance, $propagation, $type
)
$acl.AddAccessRule($rule)
Set-Acl -LiteralPath $path -AclObject $acl
$check = Get-Acl -LiteralPath $path
[ordered]@{
    path = $path
    ownerSid = $check.Owner
    currentSid = $sid.Value
    inheritanceProtected = $check.AreAccessRulesProtected
    accessRuleCount = @($check.Access).Count
    currentIdentity = $identity.Name
} | ConvertTo-Json -Compress
'''
    record = run_powershell_json(
        source,
        "Windows collector output ACL",
        environment={"EBIR_COLLECTOR_OUTPUT": str(path)},
    )
    require_exact_keys(
        record,
        {
            "path",
            "ownerSid",
            "currentSid",
            "inheritanceProtected",
            "accessRuleCount",
            "currentIdentity",
        },
        "Windows collector ACL",
    )
    if (
        record["ownerSid"] != record["currentSid"]
        or record["inheritanceProtected"] is not True
        or record["accessRuleCount"] != 1
    ):
        raise EvidenceError("collector output ACL is not current-user-only")
    write_json(artifact, record)
    return record


def require_pe_x86_64(path: Path, label: str) -> None:
    path = certification.regular_file(path, label)
    with path.open("rb") as stream:
        header = stream.read(64)
        if len(header) < 64 or header[:2] != b"MZ":
            raise EvidenceError(f"{label} is not a PE executable")
        pe_offset = int.from_bytes(header[0x3C:0x40], "little")
        stream.seek(pe_offset)
        signature = stream.read(6)
    if len(signature) != 6 or signature[:4] != b"PE\0\0":
        raise EvidenceError(f"{label} has no valid PE header")
    if int.from_bytes(signature[4:6], "little") != 0x8664:
        raise EvidenceError(f"{label} is not x86-64")


def validate_rectangle(value: Any, label: str) -> dict[str, Any]:
    rectangle = require_exact_keys(value, {"x", "y", "width", "height"}, label)
    validate_finite_number(rectangle["x"], f"{label} x")
    validate_finite_number(rectangle["y"], f"{label} y")
    validate_finite_number(rectangle["width"], f"{label} width", positive=True)
    validate_finite_number(rectangle["height"], f"{label} height", positive=True)
    return rectangle


def validate_automation_element(
    value: Any,
    label: str,
    *,
    pid: int,
    control_type: str,
    exact_name: str | None = None,
    title_fragment: str | None = None,
) -> dict[str, Any]:
    element = require_exact_keys(
        value,
        {
            "processId",
            "name",
            "automationId",
            "className",
            "controlType",
            "runtimeId",
            "bounds",
        },
        label,
    )
    if element["processId"] != pid or element["controlType"] != control_type:
        raise EvidenceError(f"{label} is not bound to the exact candidate process/type")
    if exact_name is not None and element["name"] != exact_name:
        raise EvidenceError(f"{label} did not identify {exact_name!r}")
    if title_fragment is not None and title_fragment not in str(element["name"]):
        raise EvidenceError(f"{label} did not identify the 2551Q preview")
    if not isinstance(element["automationId"], str) or not isinstance(element["className"], str):
        raise EvidenceError(f"{label} has invalid Windows Automation metadata")
    runtime_id = element["runtimeId"]
    if (
        not isinstance(runtime_id, list)
        or not runtime_id
        or not all(type(item) is int for item in runtime_id)
    ):
        raise EvidenceError(f"{label} has no exact Windows Automation runtime ID")
    validate_rectangle(element["bounds"], f"{label} bounds")
    return element


def resolve_exact_path(value: Any, expected: Path, label: str) -> None:
    if not isinstance(value, str) or Path(value).resolve() != expected.resolve():
        raise EvidenceError(f"{label} path differs from the collector destination")


def validate_export_automation(
    value: Any,
    *,
    pid: int,
    destination: Path,
    preview_screenshot: Path,
    chooser_screenshot: Path,
) -> dict[str, Any]:
    record = require_exact_keys(
        value,
        {
            "preview",
            "exportControl",
            "saveChooser",
            "fileNameControl",
            "saveControl",
            "replaceControl",
            "replaceConfirmation",
            "invoked",
            "destination",
            "previewScreenshot",
            "chooserScreenshot",
        },
        "Windows Export PDF UI Automation",
    )
    preview = validate_automation_element(
        record["preview"],
        "2551Q preview window",
        pid=pid,
        control_type="ControlType.Window",
        title_fragment="2551Q HTML Form Preview",
    )
    control = validate_automation_element(
        record["exportControl"],
        "Export PDF control",
        pid=pid,
        control_type="ControlType.Button",
        exact_name="Export PDF",
    )
    chooser = validate_automation_element(
        record["saveChooser"],
        "native Save chooser",
        pid=pid,
        control_type="ControlType.Window",
        title_fragment="Save",
    )
    file_name = validate_automation_element(
        record["fileNameControl"],
        "native filename control",
        pid=pid,
        control_type="ControlType.Edit",
    )
    if file_name["automationId"] != "1001" and file_name["name"] not in {
        "File name",
        "File name:",
    }:
        raise EvidenceError("native filename control is not the Save chooser file-name field")
    save = validate_automation_element(
        record["saveControl"],
        "native Save control",
        pid=pid,
        control_type="ControlType.Button",
        exact_name="Save",
    )
    replace = validate_automation_element(
        record["replaceControl"],
        "native overwrite confirmation",
        pid=pid,
        control_type="ControlType.Button",
        exact_name="Yes",
    )
    if record["replaceConfirmation"] is not True or record["invoked"] is not True:
        raise EvidenceError("native Save chooser did not replace the pre-existing destination")
    if (
        preview["runtimeId"] == chooser["runtimeId"]
        or control["runtimeId"] == save["runtimeId"]
    ):
        raise EvidenceError("UI Automation reused unrelated element identities")
    native_runtime_ids = {
        tuple(file_name["runtimeId"]),
        tuple(save["runtimeId"]),
        tuple(replace["runtimeId"]),
    }
    if len(native_runtime_ids) != 3:
        raise EvidenceError("native filename, Save, and overwrite controls reused identities")
    resolve_exact_path(record["destination"], destination, "toolbar destination")
    resolve_exact_path(record["previewScreenshot"], preview_screenshot, "preview screenshot")
    resolve_exact_path(record["chooserScreenshot"], chooser_screenshot, "chooser screenshot")
    for path, label in (
        (preview_screenshot, "preview screenshot"),
        (chooser_screenshot, "save chooser screenshot"),
    ):
        if not path.is_file() or path.stat().st_size <= 0:
            raise EvidenceError(f"{label} was not retained")
    return record


def validate_print_automation(value: Any, *, pid: int) -> dict[str, Any]:
    record = require_exact_keys(
        value, {"preview", "printControl", "invoked"}, "Windows Print UI Automation"
    )
    validate_automation_element(
        record["preview"],
        "2551Q preview window",
        pid=pid,
        control_type="ControlType.Window",
        title_fragment="2551Q HTML Form Preview",
    )
    validate_automation_element(
        record["printControl"],
        "Print control",
        pid=pid,
        control_type="ControlType.Button",
        exact_name="Print",
    )
    if record["invoked"] is not True:
        raise EvidenceError("Windows UI Automation did not invoke the Print control")
    return record


UIA_COMMON_POWERSHELL = r'''
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
try { Add-Type -AssemblyName System.Drawing.Common } catch { Add-Type -AssemblyName System.Drawing }
$pidValue = [int]$env:EBIR_COLLECTOR_PID
$timeoutMs = [int]$env:EBIR_COLLECTOR_TIMEOUT_MS
$deadline = [DateTime]::UtcNow.AddMilliseconds($timeoutMs)
$root = [Windows.Automation.AutomationElement]::RootElement

function ElementRecord([Windows.Automation.AutomationElement]$element) {
    $bounds = $element.Current.BoundingRectangle
    return [ordered]@{
        processId = $element.Current.ProcessId
        name = [string]$element.Current.Name
        automationId = [string]$element.Current.AutomationId
        className = [string]$element.Current.ClassName
        controlType = [string]$element.Current.ControlType.ProgrammaticName
        runtimeId = @($element.GetRuntimeId())
        bounds = [ordered]@{
            x = [double]$bounds.X
            y = [double]$bounds.Y
            width = [double]$bounds.Width
            height = [double]$bounds.Height
        }
    }
}

function RuntimeKey([Windows.Automation.AutomationElement]$element) {
    return (@($element.GetRuntimeId()) -join '.')
}

function ExactProcessWindows() {
    $pidCondition = New-Object Windows.Automation.PropertyCondition(
        [Windows.Automation.AutomationElement]::ProcessIdProperty, $pidValue
    )
    $windowCondition = New-Object Windows.Automation.PropertyCondition(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        [Windows.Automation.ControlType]::Window
    )
    $condition = New-Object Windows.Automation.AndCondition($pidCondition, $windowCondition)
    return @($root.FindAll([Windows.Automation.TreeScope]::Descendants, $condition))
}

function WaitPreview() {
    while ([DateTime]::UtcNow -lt $deadline) {
        $candidate = @(ExactProcessWindows | Where-Object {
            $_.Current.Name -like '*2551Q HTML Form Preview*'
        } | Sort-Object {
            -($_.Current.BoundingRectangle.Width * $_.Current.BoundingRectangle.Height)
        } | Select-Object -First 1)
        if ($candidate.Count -eq 1) { return $candidate[0] }
        Start-Sleep -Milliseconds 100
    }
    throw 'exact-process 2551Q HTML preview was not found'
}

function FindExactButton(
    [Windows.Automation.AutomationElement]$parent,
    [string]$name
) {
    $nameCondition = New-Object Windows.Automation.PropertyCondition(
        [Windows.Automation.AutomationElement]::NameProperty, $name
    )
    $buttonCondition = New-Object Windows.Automation.PropertyCondition(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        [Windows.Automation.ControlType]::Button
    )
    $condition = New-Object Windows.Automation.AndCondition($nameCondition, $buttonCondition)
    $buttons = @($parent.FindAll([Windows.Automation.TreeScope]::Descendants, $condition))
    if ($buttons.Count -ne 1) { throw "expected exactly one $name button" }
    if ($buttons[0].Current.ProcessId -ne $pidValue) {
        throw "$name button is not owned by the exact process"
    }
    return $buttons[0]
}

function FindFileNameControl([Windows.Automation.AutomationElement]$parent) {
    $condition = New-Object Windows.Automation.PropertyCondition(
        [Windows.Automation.AutomationElement]::ControlTypeProperty,
        [Windows.Automation.ControlType]::Edit
    )
    $controls = @($parent.FindAll([Windows.Automation.TreeScope]::Descendants, $condition) |
        Where-Object {
            $_.Current.ProcessId -eq $pidValue -and
            $_.Current.IsValuePatternAvailable -and
            ($_.Current.AutomationId -eq '1001' -or
                $_.Current.Name -eq 'File name' -or
                $_.Current.Name -eq 'File name:')
        })
    if ($controls.Count -ne 1) {
        throw 'expected exactly one native Save chooser file-name control'
    }
    return $controls[0]
}

function CaptureElement(
    [Windows.Automation.AutomationElement]$element,
    [string]$path
) {
    $bounds = $element.Current.BoundingRectangle
    $width = [Math]::Max(1, [int][Math]::Ceiling($bounds.Width))
    $height = [Math]::Max(1, [int][Math]::Ceiling($bounds.Height))
    $bitmap = New-Object Drawing.Bitmap($width, $height)
    $graphics = [Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(
            [int][Math]::Floor($bounds.X),
            [int][Math]::Floor($bounds.Y),
            0,
            0,
            (New-Object Drawing.Size($width, $height))
        )
        $bitmap.Save($path, [Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}
'''


EXPORT_UIA_POWERSHELL = UIA_COMMON_POWERSHELL + r'''
$destination = [IO.Path]::GetFullPath($env:EBIR_COLLECTOR_DESTINATION)
$previewShot = [IO.Path]::GetFullPath($env:EBIR_COLLECTOR_PREVIEW_SCREENSHOT)
$chooserShot = [IO.Path]::GetFullPath($env:EBIR_COLLECTOR_CHOOSER_SCREENSHOT)
$beforeHash = $env:EBIR_COLLECTOR_DESTINATION_BEFORE_SHA256
$preview = WaitPreview
$exportButton = FindExactButton $preview 'Export PDF'
$baseline = @{}
foreach ($window in @(ExactProcessWindows)) { $baseline[(RuntimeKey $window)] = $true }
CaptureElement $preview $previewShot
$invoke = [Windows.Automation.InvokePattern]$exportButton.GetCurrentPattern(
    [Windows.Automation.InvokePattern]::Pattern
)
$invoke.Invoke()

$chooser = $null
while ([DateTime]::UtcNow -lt $deadline -and $null -eq $chooser) {
    foreach ($window in @(ExactProcessWindows)) {
        if (-not $baseline.ContainsKey((RuntimeKey $window))) {
            try {
                $save = FindExactButton $window 'Save'
                $edit = FindFileNameControl $window
                $chooser = $window
                break
            } catch { }
        }
    }
    if ($null -eq $chooser) { Start-Sleep -Milliseconds 100 }
}
if ($null -eq $chooser) { throw 'native Save chooser was not observed' }
$save = FindExactButton $chooser 'Save'
$edit = FindFileNameControl $chooser
CaptureElement $chooser $chooserShot
$value = [Windows.Automation.ValuePattern]$edit.GetCurrentPattern(
    [Windows.Automation.ValuePattern]::Pattern
)
$value.SetValue($destination)
$saveInvoke = [Windows.Automation.InvokePattern]$save.GetCurrentPattern(
    [Windows.Automation.InvokePattern]::Pattern
)
$saveInvoke.Invoke()

$replaceConfirmed = $false
$replaceControl = $null
while ([DateTime]::UtcNow -lt $deadline -and -not $replaceConfirmed) {
    foreach ($window in @(ExactProcessWindows)) {
        try {
            $yes = FindExactButton $window 'Yes'
            $yesInvoke = [Windows.Automation.InvokePattern]$yes.GetCurrentPattern(
                [Windows.Automation.InvokePattern]::Pattern
            )
            $yesInvoke.Invoke()
            $replaceControl = $yes
            $replaceConfirmed = $true
            break
        } catch { }
    }
    if (-not $replaceConfirmed) { Start-Sleep -Milliseconds 100 }
}
if (-not $replaceConfirmed) { throw 'native overwrite confirmation was not exercised' }

$outputChanged = $false
while ([DateTime]::UtcNow -lt $deadline) {
    if (Test-Path -LiteralPath $destination -PathType Leaf) {
        try {
            $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destination).Hash.ToLowerInvariant()
            if ($hash -ne $beforeHash -and (Get-Item -LiteralPath $destination).Length -gt 0) {
                $outputChanged = $true
                break
            }
        } catch { }
    }
    Start-Sleep -Milliseconds 100
}
if (-not $outputChanged) { throw 'Export PDF did not replace the challenged destination' }

[ordered]@{
    preview = ElementRecord $preview
    exportControl = ElementRecord $exportButton
    saveChooser = ElementRecord $chooser
    fileNameControl = ElementRecord $edit
    saveControl = ElementRecord $save
    replaceControl = ElementRecord $replaceControl
    replaceConfirmation = $replaceConfirmed
    invoked = $true
    destination = $destination
    previewScreenshot = $previewShot
    chooserScreenshot = $chooserShot
} | ConvertTo-Json -Depth 8 -Compress
'''


PRINT_UIA_POWERSHELL = UIA_COMMON_POWERSHELL + r'''
$preview = WaitPreview
$printButton = FindExactButton $preview 'Print'
$invoke = [Windows.Automation.InvokePattern]$printButton.GetCurrentPattern(
    [Windows.Automation.InvokePattern]::Pattern
)
$invoke.Invoke()
[ordered]@{
    preview = ElementRecord $preview
    printControl = ElementRecord $printButton
    invoked = $true
} | ConvertTo-Json -Depth 8 -Compress
'''


def run_export_automation(
    *,
    pid: int,
    destination: Path,
    destination_before_sha256: str,
    preview_screenshot: Path,
    chooser_screenshot: Path,
    timeout: float,
) -> dict[str, Any]:
    environment = {
        "EBIR_COLLECTOR_PID": str(pid),
        "EBIR_COLLECTOR_TIMEOUT_MS": str(max(1, round(timeout * 1000))),
        "EBIR_COLLECTOR_DESTINATION": str(destination),
        "EBIR_COLLECTOR_DESTINATION_BEFORE_SHA256": destination_before_sha256,
        "EBIR_COLLECTOR_PREVIEW_SCREENSHOT": str(preview_screenshot),
        "EBIR_COLLECTOR_CHOOSER_SCREENSHOT": str(chooser_screenshot),
    }
    raw = run_powershell_json(
        EXPORT_UIA_POWERSHELL,
        "exact-process Export PDF UI Automation",
        environment=environment,
        timeout=timeout + 15,
    )
    return validate_export_automation(
        raw,
        pid=pid,
        destination=destination,
        preview_screenshot=preview_screenshot,
        chooser_screenshot=chooser_screenshot,
    )


def run_print_automation(*, pid: int, timeout: float) -> dict[str, Any]:
    raw = run_powershell_json(
        PRINT_UIA_POWERSHELL,
        "exact-process Print UI Automation",
        environment={
            "EBIR_COLLECTOR_PID": str(pid),
            "EBIR_COLLECTOR_TIMEOUT_MS": str(max(1, round(timeout * 1000))),
        },
        timeout=timeout + 15,
    )
    return validate_print_automation(raw, pid=pid)


def run_required(command: list[str], label: str, *, timeout: float = 60.0) -> str:
    try:
        result = subprocess.run(
            command,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"{label} failed closed: {output}")
    return output


HOST_STATE_POWERSHELL = r'''
$ErrorActionPreference = 'Stop'
$os = Get-CimInstance Win32_OperatingSystem
$product = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]$identity
$elevated = $principal.IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
[ordered]@{
    windowsEdition = [string]$product.ProductName
    windowsBuild = [string]($os.BuildNumber + '.' + $product.UBR)
    osArchitecture = $(if ([Environment]::Is64BitOperatingSystem) {'x86_64'} else {'x86'})
    processArchitecture = $(if ([Environment]::Is64BitProcess) {'x86_64'} else {'x86'})
    sessionId = [Diagnostics.Process]::GetCurrentProcess().SessionId
    elevated = $elevated
    processIntegrityLevel = $(if ($elevated) {'High'} else {'Medium'})
    uiAutomationAvailable = $(
        Add-Type -AssemblyName UIAutomationClient -PassThru | Out-Null
        [bool][Windows.Automation.AutomationElement]::RootElement
    )
} | ConvertTo-Json -Compress
'''


def collect_host_state(path: Path) -> dict[str, Any]:
    host = run_powershell_json(HOST_STATE_POWERSHELL, "Windows host and UI Automation state")
    require_exact_keys(
        host,
        {
            "windowsEdition",
            "windowsBuild",
            "osArchitecture",
            "processArchitecture",
            "sessionId",
            "elevated",
            "processIntegrityLevel",
            "uiAutomationAvailable",
        },
        "Windows host state",
    )
    require_nonempty(host["windowsEdition"], "Windows edition")
    require_nonempty(host["windowsBuild"], "Windows build")
    if (
        host["osArchitecture"] != "x86_64"
        or host["processArchitecture"] != "x86_64"
        or type(host["sessionId"]) is not int
        or host["sessionId"] < 0
        or host["elevated"] is not True
        or host["processIntegrityLevel"] != "High"
        or host["uiAutomationAvailable"] is not True
    ):
        raise EvidenceError(
            "Windows collection requires elevated x86-64 interactive UI Automation"
        )
    write_json(path, host)
    return host


AUTHENTICODE_POWERSHELL = r'''
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Security
$binary = [IO.Path]::GetFullPath($env:EBIR_COLLECTOR_BINARY)
$signature = Get-AuthenticodeSignature -LiteralPath $binary
if ($signature.Status.ToString() -ne 'Valid') { throw 'Authenticode status is not Valid' }
if ($null -eq $signature.SignerCertificate) { throw 'signer certificate is absent' }
if ($null -eq $signature.TimeStamperCertificate) { throw 'timestamp certificate is absent' }
$codeSigning = @(
    $signature.SignerCertificate.EnhancedKeyUsageList |
    Where-Object { $_.ObjectId.Value -eq '1.3.6.1.5.5.7.3.3' }
).Count -gt 0
if (-not $codeSigning) { throw 'signer certificate lacks the code-signing EKU' }

$bytes = [IO.File]::ReadAllBytes($binary)
if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
    throw 'candidate is not a PE executable'
}
$peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
if ([Text.Encoding]::ASCII.GetString($bytes, $peOffset, 4) -ne "PE`0`0") {
    throw 'candidate PE signature is invalid'
}
$optional = $peOffset + 24
$magic = [BitConverter]::ToUInt16($bytes, $optional)
$dataDirectory = if ($magic -eq 0x20b) { $optional + 112 } elseif ($magic -eq 0x10b) {
    $optional + 96
} else { throw 'candidate optional header is unsupported' }
$certificateOffset = [BitConverter]::ToUInt32($bytes, $dataDirectory + 32)
$certificateSize = [BitConverter]::ToUInt32($bytes, $dataDirectory + 36)
if ($certificateOffset -le 0 -or $certificateSize -le 8 -or
    ($certificateOffset + $certificateSize) -gt $bytes.Length) {
    throw 'PE certificate table is absent or invalid'
}
$cmsBytes = New-Object byte[] ($certificateSize - 8)
[Array]::Copy($bytes, $certificateOffset + 8, $cmsBytes, 0, $cmsBytes.Length)
$cms = New-Object Security.Cryptography.Pkcs.SignedCms
$cms.Decode($cmsBytes)
if ($cms.SignerInfos.Count -ne 1) { throw 'expected exactly one Authenticode signer' }
$signer = $cms.SignerInfos[0]
$sha256Oid = '2.16.840.1.101.3.4.2.1'
$fileDigest = if ($signer.DigestAlgorithm.Value -eq $sha256Oid) { 'SHA256' } else {
    $signer.DigestAlgorithm.Value
}

$timestampSigner = $null
$timestampTime = $null
foreach ($attribute in $signer.UnsignedAttributes) {
    if ($attribute.Oid.Value -eq '1.3.6.1.4.1.311.3.3.1') {
        if ($attribute.Values.Count -ne 1) { throw 'RFC3161 timestamp token is ambiguous' }
        $rawToken = [byte[]]$attribute.Values[0].RawData
        $timestampCms = New-Object Security.Cryptography.Pkcs.SignedCms
        $timestampCms.Decode($rawToken)
        if ($timestampCms.SignerInfos.Count -ne 1) { throw 'RFC3161 timestamp signer is ambiguous' }
        $timestampSigner = $timestampCms.SignerInfos[0]
        $decodedToken = $null
        $bytesConsumed = 0
        $memory = [ReadOnlyMemory[byte]]::new($rawToken)
        if (-not [Security.Cryptography.Pkcs.Rfc3161TimestampToken]::TryDecode(
            $memory, [ref]$decodedToken, [ref]$bytesConsumed
        ) -or $null -eq $decodedToken -or $bytesConsumed -ne $rawToken.Length) {
            throw 'RFC3161 timestamp token cannot be decoded exactly'
        }
        $timestampTime = $decodedToken.TokenInfo.Timestamp.UtcDateTime
        break
    }
}
if ($null -eq $timestampSigner) { throw 'RFC3161 signed timestamp token is absent' }
$timestampDigest = if ($timestampSigner.DigestAlgorithm.Value -eq $sha256Oid) { 'SHA256' } else {
    $timestampSigner.DigestAlgorithm.Value
}
if ($null -eq $timestampTime) { throw 'RFC3161 timestamp generation time is absent' }
if ($timestampSigner.Certificate.Thumbprint -ne $signature.TimeStamperCertificate.Thumbprint) {
    throw 'parsed timestamp certificate differs from Get-AuthenticodeSignature'
}
[ordered]@{
    status = $signature.Status.ToString()
    signerSubject = [string]$signature.SignerCertificate.Subject
    signerIssuer = [string]$signature.SignerCertificate.Issuer
    signerSerialNumber = [string]$signature.SignerCertificate.SerialNumber
    signerThumbprint = [string]$signature.SignerCertificate.Thumbprint
    codeSigningEku = $codeSigning
    fileDigestAlgorithm = $fileDigest
    timestampSignaturePresent = $true
    timestampSubject = [string]$signature.TimeStamperCertificate.Subject
    timestampThumbprint = [string]$signature.TimeStamperCertificate.Thumbprint
    timestampTimeUtc = $timestampTime.ToString(
        'yyyy-MM-ddTHH:mm:ss.fffZ', [Globalization.CultureInfo]::InvariantCulture
    )
    timestampDigestAlgorithm = $timestampDigest
} | ConvertTo-Json -Compress
'''


def collect_authenticode(binary: Path, signtool: Path, artifact: Path) -> dict[str, Any]:
    binary = certification.regular_file(binary, "candidate Windows executable")
    signtool = certification.regular_file(signtool, "Windows signtool")
    signature = run_powershell_json(
        AUTHENTICODE_POWERSHELL,
        "timestamped Authenticode inspection",
        environment={"EBIR_COLLECTOR_BINARY": str(binary)},
    )
    require_exact_keys(
        signature,
        {
            "status",
            "signerSubject",
            "signerIssuer",
            "signerSerialNumber",
            "signerThumbprint",
            "codeSigningEku",
            "fileDigestAlgorithm",
            "timestampSignaturePresent",
            "timestampSubject",
            "timestampThumbprint",
            "timestampTimeUtc",
            "timestampDigestAlgorithm",
        },
        "Authenticode inspection",
    )
    signtool_output = run_required(
        [str(signtool), "verify", "/pa", "/all", "/v", str(binary)],
        "signtool Authenticode verification",
    )
    if "successfully verified" not in signtool_output.casefold():
        raise EvidenceError("signtool did not report a successful /pa /all verification")
    for field in ("signerSubject", "signerIssuer", "signerSerialNumber", "timestampSubject"):
        require_nonempty(signature[field], f"Authenticode {field}")
    for field in ("signerThumbprint", "timestampThumbprint"):
        value = signature[field]
        if (
            not isinstance(value, str)
            or certification.CERTIFICATE_THUMBPRINT.fullmatch(value) is None
        ):
            raise EvidenceError(f"Authenticode {field} is not a canonical thumbprint")
    if (
        signature["status"] != "Valid"
        or signature["codeSigningEku"] is not True
        or signature["fileDigestAlgorithm"] != "SHA256"
        or signature["timestampSignaturePresent"] is not True
        or signature["timestampDigestAlgorithm"] != "SHA256"
    ):
        raise EvidenceError("candidate lacks valid SHA-256 code signing and RFC3161 timestamp")
    validate_timestamp_range(signature["timestampTimeUtc"], utc_now(), "Authenticode timestamp")
    retained = {
        "powershell": signature,
        "signtool_policy": "/pa /all /v",
        "signtool": signtool_output,
    }
    write_json(artifact, retained)
    return {
        "passed": True,
        "status": "Valid",
        "binary_sha256": file_record(binary)["sha256"],
        "signer_subject": signature["signerSubject"],
        "signer_issuer": signature["signerIssuer"],
        "signer_serial_number": signature["signerSerialNumber"],
        "signer_thumbprint": signature["signerThumbprint"],
        "code_signing_eku": True,
        "file_digest_algorithm": "SHA256",
        "timestamp_signature_present": True,
        "timestamp_subject": signature["timestampSubject"],
        "timestamp_thumbprint": signature["timestampThumbprint"],
        "timestamp_time_utc": signature["timestampTimeUtc"],
        "timestamp_digest_algorithm": "SHA256",
        "chain_trusted": True,
        "signtool_policy": "/pa /all /v",
        "artifact": file_record(artifact),
    }


PRINTER_STATE_POWERSHELL = r'''
$ErrorActionPreference = 'Stop'
$name = $env:EBIR_COLLECTOR_PRINTER
$printer = Get-Printer -Name $name
$default = Get-CimInstance Win32_Printer | Where-Object { $_.Default } | Select-Object -First 1
$log = Get-WinEvent -ListLog 'Microsoft-Windows-PrintService/Operational'
$last = Get-WinEvent -LogName 'Microsoft-Windows-PrintService/Operational' `
    -FilterXPath '*[System[(EventID=307)]]' -MaxEvents 1 -ErrorAction SilentlyContinue
[ordered]@{
    name = [string]$printer.Name
    printerStatus = [string]$printer.PrinterStatus
    workOffline = [bool]$printer.WorkOffline
    defaultPrinter = [string]$default.Name
    operationalLogEnabled = [bool]$log.IsEnabled
    baselineEventRecordId = $(if ($null -eq $last) { 0 } else { [long]$last.RecordId })
} | ConvertTo-Json -Compress
'''


def collect_printer_state(printer_name: str, artifact: Path) -> dict[str, Any]:
    state = run_powershell_json(
        PRINTER_STATE_POWERSHELL,
        "configured Windows certification printer",
        environment={"EBIR_COLLECTOR_PRINTER": printer_name},
    )
    require_exact_keys(
        state,
        {
            "name",
            "printerStatus",
            "workOffline",
            "defaultPrinter",
            "operationalLogEnabled",
            "baselineEventRecordId",
        },
        "Windows printer state",
    )
    if (
        state["name"] != printer_name
        or state["defaultPrinter"] != printer_name
        or state["workOffline"] is True
        or state["operationalLogEnabled"] is not True
        or type(state["baselineEventRecordId"]) is not int
        or state["baselineEventRecordId"] < 0
    ):
        raise EvidenceError(
            "named certification printer must be online, default, and Event 307 logging enabled"
        )
    if str(state["printerStatus"]).casefold() in {"error", "offline", "paperout"}:
        raise EvidenceError("named certification printer is in an error state")
    write_json(artifact, state)
    return state


PRINT_EVENTS_POWERSHELL = r'''
$ErrorActionPreference = 'Stop'
$baseline = [long]$env:EBIR_COLLECTOR_EVENT_BASELINE
$events = @(Get-WinEvent -LogName 'Microsoft-Windows-PrintService/Operational' `
    -FilterXPath "*[System[(EventID=307) and (EventRecordID > $baseline)]]" `
    -ErrorAction SilentlyContinue)
$records = @($events | ForEach-Object {
    $properties = @($_.Properties | ForEach-Object { $_.Value })
    [ordered]@{
        eventId = [int]$_.Id
        eventRecordId = [long]$_.RecordId
        completedAtUtc = $_.TimeCreated.ToUniversalTime().ToString(
            'yyyy-MM-ddTHH:mm:ss.fffZ', [Globalization.CultureInfo]::InvariantCulture
        )
        jobId = [string]$properties[0]
        documentName = [string]$properties[1]
        printerName = [string]$properties[4]
        totalPages = [int]$properties[7]
        message = [string]$_.Message
    }
})
[ordered]@{events = $records} | ConvertTo-Json -Depth 5 -Compress
'''


def validate_completed_print_event(
    value: Any,
    *,
    printer_name: str,
    baseline_record_id: int,
    submitted_at_utc: str,
) -> dict[str, Any]:
    wrapper = require_exact_keys(value, {"events"}, "Windows print-event query")
    events = wrapper["events"]
    if events is None:
        events = []
    if isinstance(events, dict):
        events = [events]
    if not isinstance(events, list):
        raise EvidenceError("Windows print-event query did not return an array")
    matching: list[dict[str, Any]] = []
    for raw in events:
        event = require_exact_keys(
            raw,
            {
                "eventId",
                "eventRecordId",
                "completedAtUtc",
                "jobId",
                "documentName",
                "printerName",
                "totalPages",
                "message",
            },
            "Windows completed print event",
        )
        if event["eventId"] != 307 or event["eventRecordId"] <= baseline_record_id:
            continue
        if event["printerName"] != printer_name:
            continue
        if "2551q" not in str(event["documentName"]).casefold():
            continue
        if event["totalPages"] != 2:
            raise EvidenceError("candidate print event did not complete exactly two pages")
        require_nonempty(event["jobId"], "Windows print job ID")
        require_nonempty(event["message"], "Windows print event message")
        validate_timestamp_range(submitted_at_utc, event["completedAtUtc"], "Windows print job")
        if (
            printer_name not in event["message"]
            or event["jobId"] not in event["message"]
            or event["documentName"] not in event["message"]
        ):
            raise EvidenceError("Windows Event 307 message is not bound to the job and printer")
        matching.append(event)
    if len(matching) != 1:
        raise EvidenceError("expected exactly one new completed 2551Q job on the named printer")
    return matching[0]


def wait_for_completed_print_event(
    *,
    printer_name: str,
    baseline_record_id: int,
    submitted_at_utc: str,
    timeout: float,
) -> tuple[dict[str, Any], dict[str, Any]]:
    deadline = time.monotonic() + timeout
    last_query: dict[str, Any] = {"events": []}
    last_error: Exception | None = None
    environment = {"EBIR_COLLECTOR_EVENT_BASELINE": str(baseline_record_id)}
    while time.monotonic() < deadline:
        last_query = run_powershell_json(
            PRINT_EVENTS_POWERSHELL,
            "Windows completed print-event query",
            environment=environment,
        )
        try:
            event = validate_completed_print_event(
                last_query,
                printer_name=printer_name,
                baseline_record_id=baseline_record_id,
                submitted_at_utc=submitted_at_utc,
            )
            return event, last_query
        except EvidenceError as error:
            last_error = error
        time.sleep(0.5)
    detail = f": {last_error}" if last_error else ""
    raise EvidenceError(f"timed out waiting for completed Windows print Event 307{detail}")


def add_firewall_rules(
    binary: Path, webview2: Path, artifact: Path
) -> tuple[list[str], dict[str, Any]]:
    prefix = f"eBIRForms candidate deny {uuid.uuid4()}"
    names = [f"{prefix} app", f"{prefix} webview2"]
    source = r'''
$ErrorActionPreference = 'Stop'
$profiles = @(Get-NetFirewallProfile | Select-Object Name,Enabled)
if ($profiles.Count -eq 0 -or @($profiles | Where-Object {-not $_.Enabled}).Count -ne 0) {
    throw 'all Windows Defender Firewall profiles must be enabled'
}
$created = @()
try {
    $first = New-NetFirewallRule -DisplayName $env:EBIR_RULE_APP `
        -Direction Outbound -Program $env:EBIR_BINARY -Action Block `
        -Profile Any -Enabled True
    $created += $env:EBIR_RULE_APP
    $second = New-NetFirewallRule -DisplayName $env:EBIR_RULE_WEBVIEW2 `
        -Direction Outbound -Program $env:EBIR_WEBVIEW2 -Action Block `
        -Profile Any -Enabled True
    $created += $env:EBIR_RULE_WEBVIEW2
    [ordered]@{
        profiles = @($profiles | ForEach-Object {
            [ordered]@{name=[string]$_.Name;enabled=[bool]$_.Enabled}
        })
        rules = @(
            [ordered]@{name=[string]$first.DisplayName;program=$env:EBIR_BINARY},
            [ordered]@{name=[string]$second.DisplayName;program=$env:EBIR_WEBVIEW2}
        )
    } | ConvertTo-Json -Depth 5 -Compress
} catch {
    foreach ($name in $created) {
        Remove-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue
    }
    throw
}
'''
    try:
        record = run_powershell_json(
            source,
            "Windows Defender Firewall rule creation",
            environment={
                "EBIR_RULE_APP": names[0],
                "EBIR_RULE_WEBVIEW2": names[1],
                "EBIR_BINARY": str(binary),
                "EBIR_WEBVIEW2": str(webview2),
            },
        )
        require_exact_keys(record, {"profiles", "rules"}, "Firewall creation record")
        profiles = record["profiles"]
        if (
            not isinstance(profiles, list)
            or not profiles
            or not all(
                isinstance(profile, dict)
                and set(profile) == {"name", "enabled"}
                and isinstance(profile["name"], str)
                and profile["name"]
                and profile["enabled"] is True
                for profile in profiles
            )
        ):
            raise EvidenceError("all Windows Defender Firewall profiles must be enabled")
        rules = record["rules"]
        if not isinstance(rules, list) or len(rules) != 2:
            raise EvidenceError("Firewall rules were not created with exact unique identities")
        expected_programs = [binary.resolve(), webview2.resolve()]
        for index, rule in enumerate(rules):
            rule = require_exact_keys(rule, {"name", "program"}, "Firewall rule record")
            if (
                rule["name"] != names[index]
                or Path(rule["program"]).resolve() != expected_programs[index]
            ):
                raise EvidenceError("Firewall rule was not bound to its exact executable")
    except Exception as error:
        try:
            remove_firewall_rules(names)
        except Exception as cleanup_error:
            raise EvidenceError(
                "Firewall creation failed and exact-rule cleanup also failed: "
                f"{cleanup_error}"
            ) from error
        raise
    write_json(artifact, {"creation": record, "cleanup": None})
    return names, record


def remove_firewall_rules(names: list[str]) -> dict[str, Any]:
    if len(names) != 2 or len(set(names)) != 2:
        raise EvidenceError("Firewall cleanup requires two exact rule names")
    source = r'''
$ErrorActionPreference = 'Stop'
Remove-NetFirewallRule -DisplayName $env:EBIR_RULE_APP -ErrorAction SilentlyContinue
Remove-NetFirewallRule -DisplayName $env:EBIR_RULE_WEBVIEW2 -ErrorAction SilentlyContinue
$remaining = @(
    Get-NetFirewallRule -DisplayName $env:EBIR_RULE_APP,$env:EBIR_RULE_WEBVIEW2 `
        -ErrorAction SilentlyContinue
)
[ordered]@{remaining=@($remaining | ForEach-Object {[string]$_.DisplayName})} `
    | ConvertTo-Json -Compress
'''
    record = run_powershell_json(
        source,
        "Windows Defender Firewall rule cleanup",
        environment={"EBIR_RULE_APP": names[0], "EBIR_RULE_WEBVIEW2": names[1]},
    )
    require_exact_keys(record, {"remaining"}, "Firewall cleanup record")
    remaining = record["remaining"]
    if remaining not in ([], None):
        raise EvidenceError("Windows Defender Firewall temporary rules remain")
    return {"remaining": []}


def webview2_file_version(path: Path) -> dict[str, Any]:
    path = certification.regular_file(path, "WebView2 runtime executable")
    if path.name.casefold() != "msedgewebview2.exe":
        raise EvidenceError("WebView2 runtime executable must be msedgewebview2.exe")
    require_pe_x86_64(path, "WebView2 runtime executable")
    source = r'''
$ErrorActionPreference = 'Stop'
$item = Get-Item -LiteralPath $env:EBIR_WEBVIEW2
[ordered]@{
    path = [IO.Path]::GetFullPath($item.FullName)
    fileVersion = [string]$item.VersionInfo.FileVersion
    productVersion = [string]$item.VersionInfo.ProductVersion
} | ConvertTo-Json -Compress
'''
    value = run_powershell_json(
        source,
        "WebView2 version inspection",
        environment={"EBIR_WEBVIEW2": str(path)},
    )
    require_exact_keys(value, {"path", "fileVersion", "productVersion"}, "WebView2 version")
    require_nonempty(value["productVersion"], "WebView2 product version")
    if Path(value["path"]).resolve() != path.resolve():
        raise EvidenceError("WebView2 version inspection resolved another executable")
    return value


def process_runtime_record(pid: int, binary: Path, artifact: Path) -> dict[str, Any]:
    source = r'''
$ErrorActionPreference = 'Stop'
$process = Get-Process -Id ([int]$env:EBIR_COLLECTOR_PID) -ErrorAction Stop
$modules = @($process.Modules | ForEach-Object {
    [ordered]@{name=[string]$_.ModuleName;path=[string]$_.FileName}
})
[ordered]@{
    pid = [int]$process.Id
    executable = [IO.Path]::GetFullPath($process.Path)
    modules = $modules
} | ConvertTo-Json -Depth 5 -Compress
'''
    record = run_powershell_json(
        source,
        "exact candidate process inspection",
        environment={"EBIR_COLLECTOR_PID": str(pid)},
    )
    require_exact_keys(record, {"pid", "executable", "modules"}, "candidate process")
    if record["pid"] != pid or Path(record["executable"]).resolve() != binary.resolve():
        raise EvidenceError("launched PID does not execute the manifest-bound bir.exe")
    modules = record["modules"]
    if isinstance(modules, dict):
        modules = [modules]
    if not isinstance(modules, list):
        raise EvidenceError("candidate process module list is unavailable")
    record["modules"] = modules
    write_json(artifact, record)
    return record


def webview2_descendant_record(
    pid: int, executable: Path, artifact: Path
) -> dict[str, Any]:
    source = r'''
$ErrorActionPreference = 'Stop'
$rootPid = [int]$env:EBIR_COLLECTOR_PID
$all = @(Get-CimInstance Win32_Process)
$ids = New-Object 'System.Collections.Generic.HashSet[int]'
[void]$ids.Add($rootPid)
$changed = $true
while ($changed) {
    $changed = $false
    foreach ($process in $all) {
        if ($ids.Contains([int]$process.ParentProcessId) -and
            -not $ids.Contains([int]$process.ProcessId)) {
            [void]$ids.Add([int]$process.ProcessId)
            $changed = $true
        }
    }
}
$records = @($all | Where-Object {
    $ids.Contains([int]$_.ProcessId) -and $_.Name -ieq 'msedgewebview2.exe'
} | ForEach-Object {
    [ordered]@{
        pid = [int]$_.ProcessId
        parentPid = [int]$_.ParentProcessId
        executable = [string]$_.ExecutablePath
        commandLineSha256 = $(
            $bytes = [Text.Encoding]::UTF8.GetBytes([string]$_.CommandLine)
            $sha = [Security.Cryptography.SHA256]::Create()
            ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
        )
    }
})
[ordered]@{rootPid=$rootPid;processes=$records} | ConvertTo-Json -Depth 5 -Compress
'''
    value = run_powershell_json(
        source,
        "WebView2 descendant process inspection",
        environment={"EBIR_COLLECTOR_PID": str(pid)},
    )
    require_exact_keys(value, {"rootPid", "processes"}, "WebView2 process record")
    processes = value["processes"]
    if isinstance(processes, dict):
        processes = [processes]
    if not isinstance(processes, list) or not processes:
        raise EvidenceError("candidate has no live WebView2 descendant process")
    expected = executable.resolve()
    for process in processes:
        process = require_exact_keys(
            process,
            {"pid", "parentPid", "executable", "commandLineSha256"},
            "WebView2 descendant",
        )
        if (
            type(process["pid"]) is not int
            or process["pid"] <= 0
            or type(process["parentPid"]) is not int
            or Path(process["executable"]).resolve() != expected
        ):
            raise EvidenceError("candidate used an unexpected WebView2 executable")
        require_sha256(process["commandLineSha256"], "WebView2 command-line hash")
    value["processes"] = processes
    write_json(artifact, value)
    return value


def terminate_process(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    try:
        if hasattr(signal, "CTRL_BREAK_EVENT"):
            process.send_signal(signal.CTRL_BREAK_EVENT)
        else:
            process.terminate()
        process.wait(timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        process.kill()
        process.wait(timeout=10)


def host_identifier_sha256() -> str:
    source = r'''
$ErrorActionPreference = 'Stop'
$machineGuid = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Cryptography').MachineGuid
$system = Get-CimInstance Win32_ComputerSystemProduct
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
[string]::Join('|', @($machineGuid, $system.UUID, $identity.User.Value))
'''
    try:
        raw = run_powershell(source, "Windows host identity")
    except EvidenceError:
        raw = f"{platform.node()}:{getpass.getuser()}"
    return sha256_text(raw)


def write_runtime_request(
    path: Path,
    *,
    challenge: str,
    binding: dict[str, Any],
    pid: int,
    output_pdf: Path,
    destination_before_sha256: str,
    runtime_observation: Path,
    webview2_executable: Path,
    printer_name: str,
    witness_executable: Path,
) -> None:
    write_json(
        path,
        {
            "schema_version": 1,
            "scope": "external_windows_candidate_runtime_observation_request",
            "promotion_eligible": False,
            "trusted_producer": False,
            "challenge": challenge,
            "collector_challenge_sha256": sha256_text(challenge),
            "candidate": expected_candidate(binding),
            "pid": pid,
            "form": FORM,
            "output_pdf": str(output_pdf.resolve()),
            "destination_before_sha256": destination_before_sha256,
            "webview2_executable": file_record(webview2_executable),
            "runtime_witness_executable": file_record(witness_executable),
            "printer_name": printer_name,
            "write_observation_to": str(runtime_observation.resolve()),
            "required_scope": RUNTIME_SCOPE,
            "required_strict_verifier_gaps": RUNTIME_GAPS,
        },
    )


def run_pdf_verifier(
    verifier: Path,
    pdf: Path,
    envelope_sha256: str,
    artifact: Path,
) -> dict[str, Any]:
    verifier = certification.regular_file(verifier, "owned Windows PDF verifier")
    result = subprocess.run(
        [str(verifier), str(pdf), envelope_sha256, "windows"],
        capture_output=True,
        check=False,
        timeout=60,
    )
    if result.returncode != 0 or result.stderr:
        message = result.stderr.decode("utf-8", errors="replace").strip()
        raise EvidenceError(f"owned Windows PDF verifier rejected the export: {message}")
    try:
        report = json.loads(result.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError("owned Windows PDF verifier returned invalid JSON") from error
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
        "owned Windows PDF validation report",
    )
    if (
        report["schema_version"] != 1
        or report["scope"] != "owned_windows_candidate_pdf_validation"
        or report["promotion_eligible"] is not False
        or report["form"] != FORM
        or report["envelope_sha256"] != envelope_sha256
        or report["output_sha256"] != file_record(pdf)["sha256"]
        or report["expected_page_count"] != 2
        or report["actual_page_count"] != 2
        or report["width_points"] != 612.0
        or report["height_points"] != 936.0
        or report["content_nonempty"] is not True
        or report["validated_by"] != "bir-print::html_output::validate_pdf_file"
    ):
        raise EvidenceError("owned Windows PDF verifier did not prove exact two-page output")
    pages = report["pages"]
    if not isinstance(pages, list) or len(pages) != 2:
        raise EvidenceError("owned Windows PDF verifier omitted page evidence")
    for page_number, page in enumerate(pages, 1):
        page = require_exact_keys(
            page,
            {
                "page",
                "media_width_pt",
                "media_height_pt",
                "crop_width_pt",
                "crop_height_pt",
                "rotation",
                "content_byte_count",
            },
            f"owned PDF page {page_number}",
        )
        if page != {
            "page": page_number,
            "media_width_pt": 612.0,
            "media_height_pt": 936.0,
            "crop_width_pt": 612.0,
            "crop_height_pt": 936.0,
            "rotation": 0,
            "content_byte_count": page["content_byte_count"],
        } or type(page["content_byte_count"]) is not int or page["content_byte_count"] <= 0:
            raise EvidenceError(f"owned PDF page {page_number} geometry/content is invalid")
    write_bytes(artifact, result.stdout)
    return report


def validate_live_arguments(arguments: argparse.Namespace) -> None:
    if not arguments.allow_live_print:
        raise EvidenceError(
            "--allow-live-print is required because this run completes a real printer job"
        )
    if not isinstance(arguments.printer, str) or not arguments.printer.strip():
        raise EvidenceError("--printer must name the configured default certification printer")
    if (
        not isinstance(arguments.automation_identity, str)
        or not arguments.automation_identity.strip()
    ):
        raise EvidenceError("--automation-identity must be non-empty")
    if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        raise EvidenceError("--timeout must be a positive finite number")


def collect(arguments: argparse.Namespace) -> Path:
    if platform.system() != "Windows":
        raise EvidenceError("the external Windows collector must run on Windows")
    validate_live_arguments(arguments)
    started_at = utc_now()
    output = private_output_directory(arguments.output_dir)
    acl_artifact = output / "collector-output-acl.json"
    acl = lock_down_output_directory(output, acl_artifact)
    if acl["currentIdentity"] != arguments.automation_identity:
        raise EvidenceError(
            "--automation-identity must exactly match the current Windows logon identity"
        )

    extraction = output / "candidate"
    binding = candidate_binding(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        extraction,
    )
    package = Path(binding["packaged_app"]["package_path"])
    binary = Path(binding["packaged_app"]["binary"]["path"])
    webview2_executable = certification.regular_file(
        arguments.webview2_executable, "WebView2 runtime executable"
    )
    runtime_witness = certification.regular_file(
        arguments.runtime_witness, "external Windows runtime witness"
    )
    pdf_verifier = certification.regular_file(
        arguments.pdf_verifier, "owned Windows PDF verifier"
    )
    signtool = certification.regular_file(arguments.signtool, "Windows signtool")
    require_pe_x86_64(binary, "candidate Windows executable")
    require_pe_x86_64(webview2_executable, "WebView2 runtime executable")

    rollback = load_rollback_bundle(arguments.rollback_bundle, binding)
    package_tree_before = artifact_common.tree_hash(package)
    host_artifact = output / "windows-host.json"
    host = collect_host_state(host_artifact)
    signature_artifact = output / "authenticode.json"
    authenticode = collect_authenticode(binary, signtool, signature_artifact)
    webview2_version = webview2_file_version(webview2_executable)
    printer_artifact = output / "configured-printer.json"
    printer = collect_printer_state(arguments.printer, printer_artifact)

    destination = output / "2551q-toolbar-export.pdf"
    destination_before = output / "export-destination-before.bin"
    destination_challenge = secrets.token_bytes(32)
    write_bytes(destination_before, destination_challenge)
    write_bytes(destination, destination_challenge)
    destination_before_sha256 = hashlib.sha256(destination_challenge).hexdigest()
    preview_screenshot = output / "2551q-preview-toolbar.png"
    chooser_screenshot = output / "native-save-chooser.png"
    stdout_path = output / "candidate.stdout.log"
    stderr_path = output / "candidate.stderr.log"
    write_text(stdout_path, "")
    write_text(stderr_path, "")
    runtime_observation_path = output / "external-runtime-observation.json"
    runtime_request_path = output / "runtime-observation-request.json"
    challenge = secrets.token_hex(32)
    challenge_sha256 = sha256_text(challenge)

    firewall_artifact = output / "network-denial.json"
    runtime_artifact = output / "runtime.json"
    process_artifact = output / "candidate-process.json"
    webview2_process_artifact = output / "webview2-processes.json"
    rules: list[str] = []
    firewall_creation: dict[str, Any] | None = None
    firewall_cleanup: dict[str, Any] | None = None
    process: subprocess.Popen[str] | None = None
    process_record: dict[str, Any] | None = None
    webview2_processes: dict[str, Any] | None = None
    export_ui: dict[str, Any] | None = None
    print_ui: dict[str, Any] | None = None
    print_event: dict[str, Any] | None = None
    print_query: dict[str, Any] | None = None
    runtime_observation: dict[str, Any] | None = None
    pdf_report: dict[str, Any] | None = None
    submitted_at: str | None = None
    launched_pid: int | None = None
    launch_argv = [str(binary)]
    environment = os.environ.copy()
    environment.pop("DEVELOPER_MODE", None)
    for key in list(environment):
        if key.startswith("EBIR_NATIVE_EVIDENCE") or key.startswith("EBIR_NATIVE_OUTPUT"):
            environment.pop(key, None)
        if key.startswith("EBIR_CERTIFICATION_EVIDENCE"):
            environment.pop(key, None)

    try:
        rules, firewall_creation = add_firewall_rules(
            binary, webview2_executable, firewall_artifact
        )
        creation_flags = getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0)
        with (
            stdout_path.open("w", encoding="utf-8") as stdout_stream,
            stderr_path.open("w", encoding="utf-8") as stderr_stream,
        ):
            process = subprocess.Popen(
                launch_argv,
                cwd=package,
                env=environment,
                text=True,
                stdout=stdout_stream,
                stderr=stderr_stream,
                creationflags=creation_flags,
            )
        launched_pid = process.pid
        time.sleep(1.0)
        if process.poll() is not None:
            raise EvidenceError("non-development Windows candidate exited during startup")
        process_record = process_runtime_record(launched_pid, binary, process_artifact)
        write_runtime_request(
            runtime_request_path,
            challenge=challenge,
            binding=binding,
            pid=launched_pid,
            output_pdf=destination,
            destination_before_sha256=destination_before_sha256,
            runtime_observation=runtime_observation_path,
            webview2_executable=webview2_executable,
            printer_name=arguments.printer,
            witness_executable=runtime_witness,
        )
        print(
            "Start the reviewed external Windows runtime witness with this fresh request: "
            f"{runtime_request_path}"
        )
        print(
            "In the exact launched candidate, open a real 2551Q draft and click Print Preview. "
            "Leave the 2551Q HTML Form Preview visible."
        )
        input("Press Enter only after the runtime witness is attached and preview is visible: ")
        webview2_processes = webview2_descendant_record(
            launched_pid, webview2_executable, webview2_process_artifact
        )
        export_ui = run_export_automation(
            pid=launched_pid,
            destination=destination,
            destination_before_sha256=destination_before_sha256,
            preview_screenshot=preview_screenshot,
            chooser_screenshot=chooser_screenshot,
            timeout=arguments.timeout,
        )

        confirmation = input(
            "A real two-page print job will now be submitted to the default printer "
            f"{arguments.printer!r}. Type that printer name exactly to consent: "
        )
        if confirmation != arguments.printer:
            raise EvidenceError("operator did not explicitly confirm the exact printer name")
        submitted_at = utc_now()
        print_ui = run_print_automation(pid=launched_pid, timeout=arguments.timeout)
        print_event, print_query = wait_for_completed_print_event(
            printer_name=arguments.printer,
            baseline_record_id=printer["baselineEventRecordId"],
            submitted_at_utc=submitted_at,
            timeout=arguments.timeout,
        )
        runtime_observation = wait_for_runtime_observation(
            runtime_observation_path,
            timeout=arguments.timeout,
            validation={
                "binding": binding,
                "challenge_sha256": challenge_sha256,
                "pid": launched_pid,
                "output_pdf": destination,
                "destination_before_sha256": destination_before_sha256,
                "webview2_executable": webview2_executable,
                "printer_name": arguments.printer,
                "witness_executable": runtime_witness,
            },
        )
        runtime_version = runtime_observation["webview2"]["runtime_version"]
        if runtime_version not in {
            webview2_version["fileVersion"],
            webview2_version["productVersion"],
        }:
            raise EvidenceError("runtime witness WebView2 version differs from executable metadata")
        pdf_verifier_artifact = output / "owned-pdf-verifier.json"
        pdf_report = run_pdf_verifier(
            pdf_verifier,
            destination,
            runtime_observation["envelope_sha256"],
            pdf_verifier_artifact,
        )
    finally:
        if process is not None:
            terminate_process(process)
        if rules:
            firewall_cleanup = remove_firewall_rules(rules)
            write_json(
                firewall_artifact,
                {"creation": firewall_creation, "cleanup": firewall_cleanup},
            )

    if any(
        value is None
        for value in (
            launched_pid,
            process_record,
            webview2_processes,
            export_ui,
            print_ui,
            print_event,
            print_query,
            runtime_observation,
            pdf_report,
            submitted_at,
            firewall_creation,
            firewall_cleanup,
        )
    ):
        raise EvidenceError("Windows candidate collection did not complete every evidence stage")
    assert launched_pid is not None
    assert runtime_observation is not None
    assert print_event is not None
    assert pdf_report is not None
    assert submitted_at is not None

    package_tree_after = artifact_common.tree_hash(package)
    if package_tree_before != package_tree_after:
        raise EvidenceError("candidate package changed during Windows collection")

    export_artifact = output / "toolbar-export-observation.json"
    write_json(
        export_artifact,
        {
            "challenge_sha256": challenge_sha256,
            "exact_pid": launched_pid,
            "ui_automation": export_ui,
            "preview_screenshot": file_record(preview_screenshot),
            "save_chooser_screenshot": file_record(chooser_screenshot),
            "runtime_witness": file_record(runtime_observation_path),
            "runtime_witness_executable": file_record(runtime_witness),
            "destination_before": file_record(destination_before),
            "output": file_record(destination),
        },
    )
    print_artifact = output / "native-print-observation.json"
    printer_output_sha256: str | None = None
    if arguments.printer_output is not None:
        printer_output_sha256 = file_record(
            certification.regular_file(arguments.printer_output, "retained printer output")
        )["sha256"]
    write_json(
        print_artifact,
        {
            "challenge_sha256": challenge_sha256,
            "exact_pid": launched_pid,
            "explicit_printer_consent": arguments.printer,
            "ui_automation": print_ui,
            "printer": file_record(printer_artifact),
            "completed_event": print_event,
            "event_query": print_query,
            "runtime_witness": file_record(runtime_observation_path),
            "printer_output_sha256": printer_output_sha256,
        },
    )
    webview2_artifact = output / "webview2-runtime.json"
    write_json(
        webview2_artifact,
        {
            "file_version": webview2_version,
            "executable": file_record(webview2_executable),
            "processes": file_record(webview2_process_artifact),
            "runtime_witness": runtime_observation["webview2"],
        },
    )
    dependencies_artifact = output / "runtime-dependencies.json"
    write_json(
        dependencies_artifact,
        {
            "candidate_process": file_record(process_artifact),
            "runtime_witness": runtime_observation["dependencies"],
        },
    )
    write_json(
        runtime_artifact,
        {
            "challenge_sha256": challenge_sha256,
            "pid": launched_pid,
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "launch_argv": launch_argv,
            "stdout": file_record(stdout_path),
            "stderr": file_record(stderr_path),
            "package_tree_sha256_before": package_tree_before,
            "package_tree_sha256_after": package_tree_after,
            "runtime_witness": file_record(runtime_observation_path),
        },
    )
    distribution_artifact = output / "windows-distribution-policy.json"
    write_json(
        distribution_artifact,
        {
            "candidate_format": "portable_zip",
            "distribution_track": "portable_candidate",
            "candidate_contains_installer": False,
            "public_release_formats": certification.PUBLIC_RELEASE_FORMATS,
            "public_release_allows_msix": False,
            "store_msix_policy": certification.STORE_MSIX_POLICY,
            "msix_certification_claimed": False,
            "public_installer_certification_claimed": False,
            "installed_payload_certification_claimed": False,
        },
    )

    completed_at = utc_now()
    strict_gaps = [
        certification.NON_PROMOTIONAL_GAP,
        certification.PUBLIC_INSTALLER_GAP,
        "runtime witness and local UI Automation are untrusted external observations",
        "rollback artifacts are externally supplied and not generated by this collector",
        "macOS and Linux native certification remain separate and incomplete",
        *rollback["strict_verifier_gaps"],
    ]
    strict_gaps = list(dict.fromkeys(strict_gaps))
    script_record = file_record(Path(__file__).resolve())
    runtime_webview2 = runtime_observation["webview2"]
    attestation = {
        "schema_version": 1,
        "scope": certification.ATTESTATION_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "operator_only": True,
        "attestation_id": str(uuid.uuid4()),
        "form": FORM,
        "candidate": expected_candidate(binding),
        "collector": {
            "name": COLLECTOR_NAME,
            "version": COLLECTOR_VERSION,
            "invocation_id": str(uuid.uuid4()),
            "started_at_utc": started_at,
            "completed_at_utc": completed_at,
            "executable_sha256": script_record["sha256"],
            "host_identifier_sha256": host_identifier_sha256(),
        },
        "host": {
            "windows_edition": host["windowsEdition"],
            "windows_build": host["windowsBuild"],
            "os_architecture": "x86_64",
            "process_architecture": "x86_64",
            "session_id": host["sessionId"],
            "elevated": True,
            "process_integrity_level": "High",
            "artifact": file_record(host_artifact),
        },
        "ui_automation": {
            "available": True,
            "automation_identity": arguments.automation_identity,
            "process_architecture": "x86_64",
            "artifact": file_record(export_artifact),
        },
        "webview2": {
            "runtime_version": runtime_webview2["runtime_version"],
            "channel": runtime_webview2["channel"],
            "architecture": "x86_64",
            "install_scope": runtime_webview2["install_scope"],
            "core_webview2_7_available": True,
            "core_webview2_16_available": True,
            "executable": file_record(webview2_executable),
            "artifact": file_record(webview2_artifact),
        },
        "dependencies": {
            "msvc_runtime_loaded": True,
            "webview2_loader_bound": True,
            "artifact": file_record(dependencies_artifact),
        },
        "runtime": {
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "launch_argv": launch_argv,
            "pid": launched_pid,
            "network_denial": {
                "mechanism": (
                    "Windows Defender Firewall outbound block rules for exact bir.exe "
                    "and msedgewebview2.exe"
                ),
                "exercised": True,
                "enforced_for_launch": True,
                "passed": True,
                "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
                "webview2_executable_sha256": file_record(webview2_executable)["sha256"],
                "rule_names": rules,
                "cleanup_verified": True,
                "artifact": file_record(firewall_artifact),
            },
            "artifact": file_record(runtime_artifact),
        },
        "preview": {
            "exercised": True,
            "passed": True,
            "window_title": export_ui["preview"]["name"],
            "document_run_id": runtime_observation["document_run_id"],
            "envelope_sha256": runtime_observation["envelope_sha256"],
            "nonce": runtime_observation["preview_nonce"],
            "page_count": 2,
            "geometry_measurements": runtime_observation["geometry_measurements"],
            "artifact": file_record(preview_screenshot),
        },
        "toolbar_export": {
            "exercised": True,
            "passed": True,
            "control": "Export PDF",
            "save_chooser_exercised": True,
            "destination_path": str(destination.resolve()),
            "nonce": runtime_observation["preview_nonce"],
            "print_to_pdf_hresult": "S_OK",
            "print_to_pdf_result": True,
            "artifact": file_record(export_artifact),
        },
        "native_print": {
            "exercised": True,
            "passed": True,
            "completed": True,
            "printer_name": arguments.printer,
            "job_id": print_event["jobId"],
            "event_record_id": print_event["eventRecordId"],
            "document_name": print_event["documentName"],
            "submitted_at_utc": submitted_at,
            "completed_at_utc": print_event["completedAtUtc"],
            "total_pages": 2,
            "completion_status": "Completed",
            "output_sha256": printer_output_sha256,
            "webview2_print_hresult": "S_OK",
            "webview2_print_status": "Succeeded",
            "artifact": file_record(print_artifact),
        },
        "pdf_validation": {
            "exercised": True,
            "passed": True,
            "output": file_record(destination),
            "expected_page_count": 2,
            "actual_page_count": pdf_report["actual_page_count"],
            "pages": pdf_report["pages"],
            "content_nonempty": pdf_report["content_nonempty"],
            "validated_by": pdf_report["validated_by"],
            "verifier_executable_sha256": file_record(pdf_verifier)["sha256"],
            "artifact": file_record(output / "owned-pdf-verifier.json"),
        },
        "package_security": {
            "authenticode": authenticode,
            "distribution_policy": {
                "candidate_format": "portable_zip",
                "distribution_track": "portable_candidate",
                "candidate_contains_installer": False,
                "public_release_formats": certification.PUBLIC_RELEASE_FORMATS,
                "public_release_allows_msix": False,
                "store_msix_policy": certification.STORE_MSIX_POLICY,
                "msix_certification_claimed": False,
                "public_installer_certification_claimed": False,
                "installed_payload_certification_claimed": False,
                "artifact": file_record(distribution_artifact),
            },
        },
        "integrity": {
            "package_tree_sha256_before": package_tree_before,
            "package_tree_sha256_after": package_tree_after,
            **rollback["integrity"],
        },
        "rollback": {
            "cases": rollback["cases"],
            "destination_preserved": True,
            "temporary_files_remaining": 0,
            "draft_unchanged": True,
        },
        "strict_verifier_gaps": strict_gaps,
    }
    attestation_path = output / "windows-candidate-attestation.json"
    write_json(attestation_path, attestation)
    certification.validate_attestation(attestation_path, binding)
    report_path = output / "windows-candidate-certification-report.json"
    certification.verify_attestation_command(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        attestation_path,
        pdf_verifier,
        signtool,
        report_path,
    )
    return report_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--candidate-archive", required=True, type=Path)
    parser.add_argument("--renderer-identity", required=True, type=Path)
    parser.add_argument("--pdf-verifier", required=True, type=Path)
    parser.add_argument("--signtool", required=True, type=Path)
    parser.add_argument("--webview2-executable", required=True, type=Path)
    parser.add_argument("--runtime-witness", required=True, type=Path)
    parser.add_argument("--rollback-bundle", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--printer", required=True)
    parser.add_argument("--printer-output", type=Path)
    parser.add_argument(
        "--automation-identity",
        required=True,
        help="exact current Windows logon identity used by UI Automation",
    )
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument(
        "--allow-live-print",
        action="store_true",
        help="required acknowledgement that this run completes a real printer job",
    )
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        report = collect(arguments)
        print(report)
        return 0
    except (
        EvidenceError,
        OSError,
        ValueError,
        json.JSONDecodeError,
        zipfile.BadZipFile,
        subprocess.SubprocessError,
        EOFError,
    ) as error:
        print(f"Windows candidate collection failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
