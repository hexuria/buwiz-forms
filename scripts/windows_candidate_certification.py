#!/usr/bin/env python3
"""Inspect and verify an exact Windows HTML certification candidate.

This tool is intentionally non-promotional. It binds the portable archive
created by ``html-candidate-certification.yml`` to its manifest, clean source
revision, packaged renderer identity, and actual non-development executable.

The strict verifier accepts only a complete external Windows attestation for
the user-visible preview, Export PDF/save chooser, system print, owned PDF
validation, rollback drills, and package security. A successful report remains
operator-only and untrusted; it cannot promote a form or become release
evidence. The current workflow archive is unsigned, so its strict
Authenticode gate is expected to fail until candidate construction changes.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid
import zipfile
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import candidate_certification_common as certification_common  # noqa: E402


artifact_common = certification_common.artifact_common
EvidenceError = certification_common.EvidenceError
SCHEMA_VERSION = 1
BINDING_SCOPE = "external_windows_candidate_binding"
PROBE_SCOPE = "external_windows_candidate_nondev_probe"
ATTESTATION_SCOPE = "external_windows_candidate_certification_attestation"
REPORT_SCOPE = "external_windows_candidate_certification_verification"
FORM = certification_common.FORM
EXPECTED_PAGE_COUNT = 2
EXPECTED_WIDTH_POINTS = 612.0
EXPECTED_HEIGHT_POINTS = 936.0
RENDERER_RELATIVE_PATH = Path("assets/form-renderer")
IDENTITY_RELATIVE_PATH = Path("assets/form-renderer-build-identity.json")
BINARY_RELATIVE_PATH = Path("bir.exe")
RFC3339_UTC = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\Z")
CERTIFICATE_THUMBPRINT = re.compile(r"[0-9A-F]{40}\Z")
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
        "missing_webview2_runtime",
        "old_webview2_runtime",
        "missing_core_webview2_7",
        "missing_core_webview2_16",
        "print_hresult_failure",
        "print_status_failure",
        "print_to_pdf_hresult_failure",
        "print_to_pdf_result_failure",
        "printer_unavailable",
        "invalid_authenticode",
        "missing_authenticode_timestamp",
        "package_tree_mismatch",
        "firewall_cleanup",
    }
)
NON_PROMOTIONAL_GAP = "collector producer is not registered as trusted"
PUBLIC_INSTALLER_GAP = (
    "public EXE/MSI and installed-tree certification remain separate and incomplete"
)
PUBLIC_RELEASE_FORMATS = ["signed_inno_setup_exe", "signed_msi"]
STORE_MSIX_POLICY = "separate_store_only_certification_blocked"


regular_file = certification_common.regular_file
load_json = certification_common.load_json
require_exact_keys = certification_common.require_exact_keys
require_sha256 = certification_common.require_sha256
require_true = certification_common.require_true
require_nonempty_string = certification_common.require_nonempty_string
resolve_record_path = certification_common.resolve_record_path
verify_file_record = certification_common.verify_file_record


def validate_candidate_inputs(
    manifest_path: Path, archive_path: Path, identity_path: Path
) -> dict[str, Any]:
    return certification_common.validate_candidate_inputs(
        manifest_path,
        archive_path,
        identity_path,
        expected_platform="windows",
        expected_architecture="x86_64",
    )


def bind_extracted_package(
    candidate: dict[str, Any], package: Path, external_identity_path: Path
) -> dict[str, Any]:
    package = package.resolve(strict=True)
    if not package.is_dir():
        raise EvidenceError("extracted candidate is not a portable Windows package")
    binary = regular_file(package / BINARY_RELATIVE_PATH, "packaged Windows executable")
    renderer = package / RENDERER_RELATIVE_PATH
    bundled_identity_path = package / IDENTITY_RELATIVE_PATH
    renderer_hash = artifact_common.tree_hash(renderer)
    if renderer_hash != candidate["renderer_bundle_sha256"]:
        raise EvidenceError("packaged renderer differs from the candidate manifest")
    bundled_identity = artifact_common.file_record(bundled_identity_path)
    external_identity = artifact_common.file_record(external_identity_path)
    if bundled_identity["sha256"] != external_identity["sha256"]:
        raise EvidenceError("packaged renderer identity differs from the uploaded identity")
    artifact_common.validate_build_identity(bundled_identity_path, renderer_hash)

    forbidden = [
        path
        for path in package.rglob("*")
        if path.is_file()
        and (
            path.suffix.casefold() in {".msi", ".msix"}
            or path.name.casefold().endswith("-setup.exe")
        )
    ]
    if forbidden:
        raise EvidenceError(
            "portable candidate unexpectedly contains installer artifacts: "
            + ", ".join(str(path.relative_to(package)) for path in forbidden)
        )
    return {
        "package_path": str(package),
        "package_tree_sha256": artifact_common.tree_hash(package),
        "binary": artifact_common.file_record(binary),
        "renderer_path": str(renderer.resolve()),
        "renderer_bundle_sha256": renderer_hash,
        "bundled_renderer_identity": bundled_identity,
        "distribution_kind": "portable_zip",
        "distribution_track": "portable_candidate",
        "contains_installer": False,
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
    package = certification_common.extract_portable_zip(
        archive_path, output_dir / "extracted"
    )
    packaged = bind_extracted_package(candidate, package, identity_path)
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
            PUBLIC_INSTALLER_GAP,
            "preview, Export PDF/save chooser, native print, and rollback were not exercised",
            "the portable workflow candidate is not Authenticode signed",
            "public EXE/MSI installers and Store-only MSIX have separate policy gates",
        ],
    }
    artifact_common.write_json_atomic(
        output_dir / "windows-candidate-binding.json", binding
    )
    return binding


def _powershell_executable() -> str:
    executable = shutil.which("pwsh") or shutil.which("powershell.exe")
    if executable is None:
        raise EvidenceError("PowerShell is unavailable")
    return executable


def _powershell_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def _run_powershell(script: str, label: str, *, timeout: float = 30.0) -> str:
    try:
        result = subprocess.run(
            [
                _powershell_executable(),
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ],
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
    return result.stdout.strip()


def _terminate(process: subprocess.Popen[str]) -> None:
    try:
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        process.kill()
        process.wait(timeout=5)


def probe_nondev_candidate(
    binding: dict[str, Any],
    output_dir: Path,
    timeout: float,
    webview2_executable: Path,
) -> dict[str, Any]:
    if platform.system() != "Windows":
        raise EvidenceError("the non-development candidate probe must run on Windows")
    administrator = _run_powershell(
        "([Security.Principal.WindowsPrincipal] "
        "[Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole("
        "[Security.Principal.WindowsBuiltInRole]::Administrator)",
        "Windows administrator state",
    )
    if administrator.casefold() != "true":
        raise EvidenceError("Windows Firewall probe requires an elevated operator")
    profiles = _run_powershell(
        "Get-NetFirewallProfile | Select-Object Name,Enabled | ConvertTo-Json -Compress",
        "Windows Firewall profile state",
    )
    parsed_profiles = json.loads(profiles)
    if isinstance(parsed_profiles, dict):
        parsed_profiles = [parsed_profiles]
    if not parsed_profiles or any(
        item.get("Enabled") is not True for item in parsed_profiles
    ):
        raise EvidenceError("all Windows Firewall profiles must be enabled")

    package = Path(binding["packaged_app"]["package_path"])
    binary = Path(binding["packaged_app"]["binary"]["path"])
    webview2_executable = regular_file(
        webview2_executable, "WebView2 runtime executable"
    )
    if webview2_executable.name.casefold() != "msedgewebview2.exe":
        raise EvidenceError("WebView2 runtime executable must be msedgewebview2.exe")
    package_before = artifact_common.tree_hash(package)
    rule_prefix = f"eBIRForms candidate deny {uuid.uuid4()}"
    blocked_programs = [binary, webview2_executable]
    rule_names = [f"{rule_prefix} app", f"{rule_prefix} webview2"]
    stdout_path = output_dir / "nondev-probe.stdout.log"
    stderr_path = output_dir / "nondev-probe.stderr.log"
    environment = os.environ.copy()
    for key in list(environment):
        if key == "DEVELOPER_MODE" or key.startswith("EBIR_NATIVE_EVIDENCE"):
            environment.pop(key, None)
    added_rule_names: list[str] = []
    try:
        for rule_name, program in zip(rule_names, blocked_programs, strict=True):
            _run_powershell(
                f"New-NetFirewallRule -DisplayName {_powershell_quote(rule_name)} "
                f"-Direction Outbound -Program {_powershell_quote(str(program))} "
                "-Action Block -Profile Any -Enabled True "
                "| Select-Object DisplayName,Enabled,Direction,Action "
                "| ConvertTo-Json -Compress",
                "Windows Firewall network-denial rule creation",
            )
            added_rule_names.append(rule_name)
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr:
            process = subprocess.Popen(
                [str(binary)],
                cwd=package,
                env=environment,
                text=True,
                stdout=stdout,
                stderr=stderr,
            )
            try:
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise EvidenceError(
                            "non-development candidate exited during the launch probe"
                        )
                    time.sleep(0.1)
            finally:
                _terminate(process)
    finally:
        cleanup_errors: list[str] = []
        for rule_name in reversed(added_rule_names):
            try:
                _run_powershell(
                    "Remove-NetFirewallRule -DisplayName "
                    + _powershell_quote(rule_name),
                    "Windows Firewall network-denial rule cleanup",
                )
            except EvidenceError as error:
                cleanup_errors.append(str(error))
        if cleanup_errors:
            raise EvidenceError("; ".join(cleanup_errors))
    for rule_name in rule_names:
        remaining = _run_powershell(
            "@(Get-NetFirewallRule -DisplayName "
            + _powershell_quote(rule_name)
            + " -ErrorAction SilentlyContinue).Count",
            "Windows Firewall cleanup verification",
        )
        if remaining != "0":
            raise EvidenceError(
                "temporary Windows Firewall rule remains after the probe"
            )
    package_after = artifact_common.tree_hash(package)
    if package_after != package_before:
        raise EvidenceError("candidate package changed during the non-development probe")
    probe = {
        "schema_version": SCHEMA_VERSION,
        "scope": PROBE_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "certification_complete": False,
        "form": FORM,
        "source_revision": binding["source_revision"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "package_tree_sha256_before": package_before,
        "package_tree_sha256_after": package_after,
        "launch_argv": [str(binary)],
        "dev_tools_enabled": False,
        "network_denial": {
            "mechanism": (
                "Windows Defender Firewall outbound block rules for exact bir.exe "
                "and msedgewebview2.exe"
            ),
            "rule_names": rule_names,
            "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
            "webview2_executable_sha256": artifact_common.file_record(
                webview2_executable
            )["sha256"],
            "enforced_for_launch": True,
            "passed": True,
            "cleanup_verified": True,
        },
        "stdout": artifact_common.file_record(stdout_path),
        "stderr": artifact_common.file_record(stderr_path),
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            PUBLIC_INSTALLER_GAP,
            "startup does not prove preview, export, print, PDF, or rollback behavior",
            "the workflow candidate is not Authenticode signed and contains no public installer",
        ],
    }
    artifact_common.write_json_atomic(
        output_dir / "windows-candidate-nondev-probe.json", probe
    )
    return probe


def _validate_geometry(measurements: Any) -> None:
    if not isinstance(measurements, list) or len(measurements) != 2:
        raise EvidenceError("preview must retain exactly two stable geometry measurements")
    normalized: list[dict[str, Any]] = []
    for index, raw in enumerate(measurements, 1):
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
            f"geometry measurement {index}",
        )
        if measurement["measurement_index"] != index:
            raise EvidenceError("geometry measurement indices are not canonical")
        if (
            measurement["page_width_pt"] != EXPECTED_WIDTH_POINTS
            or measurement["page_height_pt"] != EXPECTED_HEIGHT_POINTS
        ):
            raise EvidenceError("preview paper geometry is not 612 x 936 points")
        if measurement["clipping_count"] != 0 or measurement["overflow_count"] != 0:
            raise EvidenceError("preview reports clipping or overflow")
        pages = measurement["pages"]
        if not isinstance(pages, list) or len(pages) != EXPECTED_PAGE_COUNT:
            raise EvidenceError("preview geometry does not contain exactly two pages")
        for page_number, page in enumerate(pages, 1):
            value = require_exact_keys(
                page, {"page", "x", "y", "width_pt", "height_pt"}, "preview page rectangle"
            )
            if value["page"] != page_number:
                raise EvidenceError("preview page rectangle order is not canonical")
            if value["width_pt"] != EXPECTED_WIDTH_POINTS or value[
                "height_pt"
            ] != EXPECTED_HEIGHT_POINTS:
                raise EvidenceError("preview page rectangle has incorrect paper geometry")
        normalized.append(
            {key: value for key, value in measurement.items() if key != "measurement_index"}
        )
    if normalized[0] != normalized[1]:
        raise EvidenceError("preview geometry measurements are not identical")


def _validate_timestamp_range(collector: dict[str, Any]) -> None:
    for field in ("started_at_utc", "completed_at_utc"):
        value = collector[field]
        if not isinstance(value, str) or RFC3339_UTC.fullmatch(value) is None:
            raise EvidenceError(f"collector.{field} must be an RFC3339 UTC timestamp")
    try:
        started = datetime.fromisoformat(collector["started_at_utc"].replace("Z", "+00:00"))
        completed = datetime.fromisoformat(collector["completed_at_utc"].replace("Z", "+00:00"))
    except ValueError as error:
        raise EvidenceError("collector timestamps are not valid calendar times") from error
    if completed <= started:
        raise EvidenceError("collector completion time must be after its start time")


def _require_thumbprint(value: Any, label: str) -> str:
    if not isinstance(value, str) or CERTIFICATE_THUMBPRINT.fullmatch(value) is None:
        raise EvidenceError(f"{label} must be a canonical uppercase SHA-1 certificate thumbprint")
    return value


def validate_attestation(
    attestation_path: Path, binding: dict[str, Any]
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    attestation_path = regular_file(attestation_path, "Windows certification attestation")
    base = attestation_path.parent
    attestation = load_json(attestation_path)
    verified: list[dict[str, Any]] = []
    require_exact_keys(
        attestation,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "operator_only",
            "attestation_id",
            "form",
            "candidate",
            "collector",
            "host",
            "ui_automation",
            "webview2",
            "dependencies",
            "runtime",
            "preview",
            "toolbar_export",
            "native_print",
            "pdf_validation",
            "package_security",
            "integrity",
            "rollback",
            "strict_verifier_gaps",
        },
        "Windows certification attestation",
    )
    if attestation["schema_version"] != 1 or attestation["scope"] != ATTESTATION_SCOPE:
        raise EvidenceError("Windows attestation has an unsupported schema or scope")
    if attestation["promotion_eligible"] is not False or attestation[
        "trusted_producer"
    ] is not False:
        raise EvidenceError("Windows attestation must remain non-promotional and untrusted")
    require_true(attestation["operator_only"], "attestation.operator_only")
    try:
        attestation_id = str(uuid.UUID(str(attestation["attestation_id"])))
    except (ValueError, AttributeError) as error:
        raise EvidenceError("attestation_id must be a canonical UUID") from error
    if attestation_id != attestation["attestation_id"]:
        raise EvidenceError("attestation_id must be a canonical lowercase UUID")
    if attestation["form"] != FORM:
        raise EvidenceError("Windows attestation must target exactly 2551Q:2018")

    candidate = require_exact_keys(
        attestation["candidate"],
        {
            "candidate_manifest_sha256",
            "candidate_archive_sha256",
            "source_revision",
            "package_tree_sha256",
            "binary_sha256",
            "renderer_bundle_sha256",
        },
        "attestation candidate binding",
    )
    expected_candidate = {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "package_tree_sha256": binding["packaged_app"]["package_tree_sha256"],
        "binary_sha256": binding["packaged_app"]["binary"]["sha256"],
        "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
    }
    if candidate != expected_candidate:
        raise EvidenceError("attestation candidate binding differs from the exact workflow artifact")

    collector = require_exact_keys(
        attestation["collector"],
        {
            "name",
            "version",
            "invocation_id",
            "started_at_utc",
            "completed_at_utc",
            "executable_sha256",
            "host_identifier_sha256",
        },
        "external collector",
    )
    for field in ("name", "version", "invocation_id"):
        require_nonempty_string(collector[field], f"collector.{field}")
    _validate_timestamp_range(collector)
    require_sha256(collector["executable_sha256"], "collector executable hash")
    require_sha256(collector["host_identifier_sha256"], "collector host identifier hash")

    host = require_exact_keys(
        attestation["host"],
        {
            "windows_edition",
            "windows_build",
            "os_architecture",
            "process_architecture",
            "session_id",
            "elevated",
            "process_integrity_level",
            "artifact",
        },
        "Windows host",
    )
    require_nonempty_string(host["windows_edition"], "Windows edition")
    require_nonempty_string(host["windows_build"], "Windows build")
    if host["os_architecture"] != "x86_64" or host["process_architecture"] != "x86_64":
        raise EvidenceError("Windows host and candidate process must both be x86-64")
    if not isinstance(host["session_id"], int) or host["session_id"] < 0:
        raise EvidenceError("Windows session ID must be a non-negative integer")
    require_true(host["elevated"], "Windows elevated operator state")
    if host["process_integrity_level"] != "High":
        raise EvidenceError("Windows certification requires High integrity")
    verify_file_record(host["artifact"], "Windows host artifact", base=base, verified=verified)

    automation = require_exact_keys(
        attestation["ui_automation"],
        {"available", "automation_identity", "process_architecture", "artifact"},
        "Windows UI Automation evidence",
    )
    require_true(automation["available"], "Windows UI Automation availability")
    require_nonempty_string(automation["automation_identity"], "UI Automation identity")
    if automation["process_architecture"] != "x86_64":
        raise EvidenceError("UI Automation must exercise the x86-64 candidate")
    verify_file_record(
        automation["artifact"], "UI Automation artifact", base=base, verified=verified
    )

    webview2 = require_exact_keys(
        attestation["webview2"],
        {
            "runtime_version",
            "channel",
            "architecture",
            "install_scope",
            "core_webview2_7_available",
            "core_webview2_16_available",
            "executable",
            "artifact",
        },
        "WebView2 runtime",
    )
    require_nonempty_string(webview2["runtime_version"], "WebView2 runtime version")
    if webview2["channel"] not in {"stable", "beta", "dev", "canary", "fixed"}:
        raise EvidenceError("WebView2 channel is unsupported")
    if webview2["architecture"] != "x86_64":
        raise EvidenceError("WebView2 runtime must match the x86-64 candidate")
    if webview2["install_scope"] not in {"per_machine", "per_user", "fixed"}:
        raise EvidenceError("WebView2 install scope is unsupported")
    require_true(webview2["core_webview2_7_available"], "ICoreWebView2_7 availability")
    require_true(webview2["core_webview2_16_available"], "ICoreWebView2_16 availability")
    webview2_executable = verify_file_record(
        webview2["executable"],
        "WebView2 runtime executable",
        base=base,
        verified=verified,
    )
    if not Path(webview2["executable"]["path"]).is_absolute():
        raise EvidenceError("WebView2 runtime executable path must be absolute")
    if Path(webview2_executable["path"]).name.casefold() != "msedgewebview2.exe":
        raise EvidenceError("WebView2 runtime executable must be msedgewebview2.exe")
    verify_file_record(webview2["artifact"], "WebView2 artifact", base=base, verified=verified)

    dependencies = require_exact_keys(
        attestation["dependencies"],
        {"msvc_runtime_loaded", "webview2_loader_bound", "artifact"},
        "Windows runtime dependencies",
    )
    require_true(dependencies["msvc_runtime_loaded"], "MSVC runtime loading")
    require_true(dependencies["webview2_loader_bound"], "WebView2 loader binding")
    verify_file_record(
        dependencies["artifact"], "runtime-dependency artifact", base=base, verified=verified
    )

    runtime = require_exact_keys(
        attestation["runtime"],
        {"non_dev_build", "dev_tools_enabled", "launch_argv", "pid", "network_denial", "artifact"},
        "non-development runtime",
    )
    require_true(runtime["non_dev_build"], "runtime.non_dev_build")
    if runtime["dev_tools_enabled"] is not False:
        raise EvidenceError("certification runtime must not enable dev-tools")
    argv = runtime["launch_argv"]
    if not isinstance(argv, list) or not argv or not all(
        isinstance(item, str) and item for item in argv
    ):
        raise EvidenceError("runtime.launch_argv must contain the exact launched command")
    if artifact_common.DEV_FLAG in argv or any("dev-tools" in item for item in argv):
        raise EvidenceError("certification runtime used a development-only launch path")
    if not isinstance(runtime["pid"], int) or runtime["pid"] <= 0:
        raise EvidenceError("runtime PID must be a positive integer")
    network = require_exact_keys(
        runtime["network_denial"],
        {
            "mechanism",
            "exercised",
            "enforced_for_launch",
            "passed",
            "binary_sha256",
            "webview2_executable_sha256",
            "rule_names",
            "cleanup_verified",
            "artifact",
        },
        "network denial",
    )
    mechanism = require_nonempty_string(network["mechanism"], "network denial mechanism")
    if (
        "windows defender firewall" not in mechanism.casefold()
        or "outbound block" not in mechanism.casefold()
        or "bir.exe" not in mechanism.casefold()
        or "msedgewebview2.exe" not in mechanism.casefold()
    ):
        raise EvidenceError(
            "network denial must block the exact app and WebView2 runtime executables"
        )
    for field in ("exercised", "enforced_for_launch", "passed"):
        require_true(network[field], f"network_denial.{field}")
    if network["binary_sha256"] != expected_candidate["binary_sha256"]:
        raise EvidenceError("network denial was not bound to the candidate executable")
    if network["webview2_executable_sha256"] != webview2_executable["sha256"]:
        raise EvidenceError("network denial was not bound to the WebView2 runtime")
    rule_names = network["rule_names"]
    if (
        not isinstance(rule_names, list)
        or len(rule_names) != 2
        or len(set(rule_names)) != 2
        or not all(isinstance(name, str) and name.strip() for name in rule_names)
    ):
        raise EvidenceError("network denial requires two distinct Firewall rule names")
    require_true(network["cleanup_verified"], "network_denial.cleanup_verified")
    verify_file_record(network["artifact"], "network-denial artifact", base=base, verified=verified)
    verify_file_record(runtime["artifact"], "runtime artifact", base=base, verified=verified)

    preview = require_exact_keys(
        attestation["preview"],
        {
            "exercised",
            "passed",
            "window_title",
            "document_run_id",
            "envelope_sha256",
            "nonce",
            "page_count",
            "geometry_measurements",
            "artifact",
        },
        "native preview",
    )
    require_true(preview["exercised"], "preview.exercised")
    require_true(preview["passed"], "preview.passed")
    if "2551Q HTML Form Preview" not in require_nonempty_string(
        preview["window_title"], "preview.window_title"
    ):
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
        {
            "exercised",
            "passed",
            "control",
            "save_chooser_exercised",
            "destination_path",
            "nonce",
            "print_to_pdf_hresult",
            "print_to_pdf_result",
            "artifact",
        },
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
    if (
        toolbar["print_to_pdf_hresult"] != "S_OK"
        or toolbar["print_to_pdf_result"] is not True
    ):
        raise EvidenceError("WebView2 PrintToPdf did not return HRESULT S_OK and success=true")
    verify_file_record(toolbar["artifact"], "toolbar/export artifact", base=base, verified=verified)

    native_print = require_exact_keys(
        attestation["native_print"],
        {
            "exercised",
            "passed",
            "completed",
            "printer_name",
            "job_id",
            "event_record_id",
            "document_name",
            "submitted_at_utc",
            "completed_at_utc",
            "total_pages",
            "completion_status",
            "output_sha256",
            "webview2_print_hresult",
            "webview2_print_status",
            "artifact",
        },
        "native system print",
    )
    for field in ("exercised", "passed", "completed"):
        require_true(native_print[field], f"native_print.{field}")
    require_nonempty_string(native_print["printer_name"], "native printer name")
    require_nonempty_string(native_print["job_id"], "native print job id")
    if not isinstance(native_print["event_record_id"], int) or native_print[
        "event_record_id"
    ] <= 0:
        raise EvidenceError("native print completion requires a positive EventRecordID")
    require_nonempty_string(native_print["document_name"], "native print document name")
    _validate_timestamp_range(
        {
            "started_at_utc": native_print["submitted_at_utc"],
            "completed_at_utc": native_print["completed_at_utc"],
        }
    )
    if native_print["total_pages"] != 2 or native_print["completion_status"] != "Completed":
        raise EvidenceError("native print evidence must prove two completed pages")
    if native_print["output_sha256"] is not None:
        require_sha256(native_print["output_sha256"], "native print output hash")
    if native_print["webview2_print_hresult"] != "S_OK" or native_print[
        "webview2_print_status"
    ] != "Succeeded":
        raise EvidenceError("WebView2 Print did not return HRESULT S_OK and Succeeded status")
    verify_file_record(
        native_print["artifact"], "native-print artifact", base=base, verified=verified
    )

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
    if pdf["expected_page_count"] != 2 or pdf["actual_page_count"] != 2:
        raise EvidenceError("exported PDF page count is not exactly two")
    require_true(pdf["content_nonempty"], "pdf_validation.content_nonempty")
    if pdf["validated_by"] != "bir-print::html_output::validate_pdf_file":
        raise EvidenceError("PDF was not validated by the owned Rust PDF verifier")
    require_sha256(pdf["verifier_executable_sha256"], "PDF verifier executable hash")
    output_pdf = verify_file_record(pdf["output"], "exported PDF", base=base, verified=verified)
    if str(Path(toolbar["destination_path"]).resolve()) != output_pdf["path"]:
        raise EvidenceError("toolbar destination differs from the validated PDF")
    pages = pdf["pages"]
    if not isinstance(pages, list) or len(pages) != 2:
        raise EvidenceError("PDF geometry must cover both pages")
    for page_number, raw in enumerate(pages, 1):
        page = require_exact_keys(
            raw,
            {
                "page",
                "media_width_pt",
                "media_height_pt",
                "crop_width_pt",
                "crop_height_pt",
                "rotation",
                "content_byte_count",
            },
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
        attestation["package_security"],
        {"authenticode", "distribution_policy"},
        "Windows package security",
    )
    authenticode = require_exact_keys(
        security["authenticode"],
        {
            "passed",
            "status",
            "binary_sha256",
            "signer_subject",
            "signer_issuer",
            "signer_serial_number",
            "signer_thumbprint",
            "code_signing_eku",
            "file_digest_algorithm",
            "timestamp_signature_present",
            "timestamp_subject",
            "timestamp_thumbprint",
            "timestamp_time_utc",
            "timestamp_digest_algorithm",
            "chain_trusted",
            "signtool_policy",
            "artifact",
        },
        "Authenticode evidence",
    )
    require_true(authenticode["passed"], "authenticode.passed")
    if authenticode["status"] != "Valid":
        raise EvidenceError("candidate executable does not have a valid Authenticode status")
    if authenticode["binary_sha256"] != expected_candidate["binary_sha256"]:
        raise EvidenceError("Authenticode evidence is not bound to the candidate executable")
    require_nonempty_string(authenticode["signer_subject"], "Authenticode signer subject")
    require_nonempty_string(authenticode["signer_issuer"], "Authenticode signer issuer")
    require_nonempty_string(authenticode["signer_serial_number"], "Authenticode serial number")
    _require_thumbprint(authenticode["signer_thumbprint"], "Authenticode signer thumbprint")
    require_true(authenticode["code_signing_eku"], "Authenticode code-signing EKU")
    if authenticode["file_digest_algorithm"] != "SHA256":
        raise EvidenceError("Authenticode file digest must be SHA256")
    require_true(
        authenticode["timestamp_signature_present"],
        "Authenticode timestamp signature",
    )
    require_nonempty_string(authenticode["timestamp_subject"], "timestamp signer subject")
    _require_thumbprint(authenticode["timestamp_thumbprint"], "timestamp signer thumbprint")
    timestamp = {
        "started_at_utc": authenticode["timestamp_time_utc"],
        "completed_at_utc": collector["completed_at_utc"],
    }
    _validate_timestamp_range(timestamp)
    if authenticode["timestamp_digest_algorithm"] != "SHA256":
        raise EvidenceError("RFC3161 timestamp digest must be SHA256")
    require_true(authenticode["chain_trusted"], "Authenticode chain trust")
    if authenticode["signtool_policy"] != "/pa /all /v":
        raise EvidenceError("Authenticode verification must use the release /pa /all policy")
    verify_file_record(
        authenticode["artifact"], "Authenticode artifact", base=base, verified=verified
    )

    policy = require_exact_keys(
        security["distribution_policy"],
        {
            "candidate_format",
            "distribution_track",
            "candidate_contains_installer",
            "public_release_formats",
            "public_release_allows_msix",
            "store_msix_policy",
            "msix_certification_claimed",
            "public_installer_certification_claimed",
            "installed_payload_certification_claimed",
            "artifact",
        },
        "Windows distribution policy",
    )
    if policy["candidate_format"] != "portable_zip" or policy[
        "candidate_contains_installer"
    ] is not False:
        raise EvidenceError("workflow candidate must remain the portable non-installer ZIP")
    if policy["distribution_track"] != "portable_candidate":
        raise EvidenceError("attestation mixed the portable and public-installer tracks")
    if policy["public_release_formats"] != PUBLIC_RELEASE_FORMATS:
        raise EvidenceError("public Windows release formats differ from the reviewed EXE/MSI policy")
    if policy["public_release_allows_msix"] is not False:
        raise EvidenceError("public GitHub releases must reject MSIX artifacts")
    if policy["store_msix_policy"] != STORE_MSIX_POLICY:
        raise EvidenceError("Store-only MSIX must retain its separate blocked certification policy")
    if policy["msix_certification_claimed"] is not False:
        raise EvidenceError("this portable candidate must not claim Store MSIX certification")
    if policy["public_installer_certification_claimed"] is not False or policy[
        "installed_payload_certification_claimed"
    ] is not False:
        raise EvidenceError(
            "portable evidence must not claim public installer or installed-payload certification"
        )
    verify_file_record(
        policy["artifact"], "distribution-policy artifact", base=base, verified=verified
    )

    integrity = require_exact_keys(
        attestation["integrity"],
        {
            "package_tree_sha256_before",
            "package_tree_sha256_after",
            "destination_before",
            "destination_after",
            "draft_before",
            "draft_after",
            "temporary_files_manifest",
        },
        "state integrity",
    )
    expected_tree = binding["packaged_app"]["package_tree_sha256"]
    if integrity["package_tree_sha256_before"] != expected_tree or integrity[
        "package_tree_sha256_after"
    ] != expected_tree:
        raise EvidenceError("candidate package changed during certification")
    before_destination = verify_file_record(
        integrity["destination_before"],
        "destination-before snapshot",
        base=base,
        verified=verified,
    )
    after_destination = verify_file_record(
        integrity["destination_after"],
        "destination-after snapshot",
        base=base,
        verified=verified,
    )
    before_draft = verify_file_record(
        integrity["draft_before"],
        "draft-before snapshot",
        base=base,
        verified=verified,
    )
    after_draft = verify_file_record(
        integrity["draft_after"],
        "draft-after snapshot",
        base=base,
        verified=verified,
    )
    for before, after, label in (
        (before_destination, after_destination, "destination"),
        (before_draft, after_draft, "draft"),
    ):
        if before["path"] == after["path"]:
            raise EvidenceError(f"{label} snapshots must be retained as distinct files")
        if before["sha256"] != after["sha256"]:
            raise EvidenceError(f"{label} changed during the failed-output drill")
    temp_record = verify_file_record(
        integrity["temporary_files_manifest"],
        "temporary-files manifest",
        base=base,
        verified=verified,
    )
    if load_json(Path(temp_record["path"]), limit=1024 * 1024) != {"remaining": []}:
        raise EvidenceError("temporary-files manifest reports leaked output files")

    rollback = require_exact_keys(
        attestation["rollback"],
        {"cases", "destination_preserved", "temporary_files_remaining", "draft_unchanged"},
        "rollback drill",
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
        verify_file_record(
            case["artifact"], f"rollback artifact {name}", base=base, verified=verified
        )
    missing = sorted(ROLLBACK_CASES - seen)
    if missing:
        raise EvidenceError(f"rollback attestation is incomplete: {', '.join(missing)}")

    gaps = attestation["strict_verifier_gaps"]
    if not isinstance(gaps, list) or NON_PROMOTIONAL_GAP not in gaps:
        raise EvidenceError("attestation erased the untrusted-producer promotion blocker")
    if PUBLIC_INSTALLER_GAP not in gaps:
        raise EvidenceError("attestation erased the public installer/installed-tree blocker")
    return attestation, verified


def verify_owned_pdf_artifact(
    attestation_path: Path,
    attestation: dict[str, Any],
    verifier_path: Path,
    verified: list[dict[str, Any]],
) -> dict[str, Any]:
    verifier_path = regular_file(verifier_path, "owned PDF verifier")
    verifier_record = artifact_common.file_record(verifier_path)
    if verifier_record["sha256"] != attestation["pdf_validation"][
        "verifier_executable_sha256"
    ]:
        raise EvidenceError("owned PDF verifier differs from its attested executable hash")
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
                "windows",
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
    recorded_output = artifact_common.read_stable_file(artifact_path, limit=4 * 1024 * 1024)
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
        "scope": "owned_windows_candidate_pdf_validation",
        "promotion_eligible": False,
        "form": FORM,
        "envelope_sha256": attestation["preview"]["envelope_sha256"],
        "output_sha256": attestation["pdf_validation"]["output"]["sha256"],
        "expected_page_count": 2,
        "actual_page_count": 2,
        "width_points": 612.0,
        "height_points": 936.0,
        "content_nonempty": True,
        "validated_by": "bir-print::html_output::validate_pdf_file",
        "pages": attestation["pdf_validation"]["pages"],
    }
    if report != expected:
        raise EvidenceError("owned PDF validation report differs from the immutable attestation")
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
        result = subprocess.run(
            command, text=True, capture_output=True, check=False, timeout=30
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"{label} failed closed: {output}")
    return output


def verify_live_windows_state(
    package: Path,
    attestation: dict[str, Any],
    signtool_path: Path,
) -> dict[str, Any]:
    if platform.system() != "Windows":
        raise EvidenceError("strict Windows attestation verification must run on Windows")
    binary = regular_file(package / BINARY_RELATIVE_PATH, "packaged Windows executable")
    signtool = regular_file(signtool_path, "Windows signtool")
    webview2_record = attestation["webview2"]["executable"]
    webview2_executable = regular_file(
        Path(webview2_record["path"]), "live WebView2 runtime executable"
    )
    if webview2_executable.name.casefold() != "msedgewebview2.exe":
        raise EvidenceError("live WebView2 runtime executable has an unexpected name")
    if artifact_common.file_record(webview2_executable) != webview2_record:
        raise EvidenceError("live WebView2 runtime executable differs from the attestation")
    quoted_binary = _powershell_quote(str(binary))
    signature_json = _run_powershell(
        "$s = Get-AuthenticodeSignature -LiteralPath "
        + quoted_binary
        + "; [ordered]@{Status=$s.Status.ToString();"
        "SignerSubject=$s.SignerCertificate.Subject;"
        "SignerIssuer=$s.SignerCertificate.Issuer;"
        "SignerSerialNumber=$s.SignerCertificate.SerialNumber;"
        "SignerThumbprint=$s.SignerCertificate.Thumbprint;"
        "CodeSigningEku=(@($s.SignerCertificate.EnhancedKeyUsageList | "
        "Where-Object {$_.ObjectId.Value -eq '1.3.6.1.5.5.7.3.3'}).Count -gt 0);"
        "TimeStamperSubject=$s.TimeStamperCertificate.Subject;"
        "TimeStamperThumbprint=$s.TimeStamperCertificate.Thumbprint} "
        "| ConvertTo-Json -Compress",
        "live Authenticode signature",
    )
    try:
        signature = json.loads(signature_json)
    except json.JSONDecodeError as error:
        raise EvidenceError(
            f"PowerShell returned invalid Authenticode JSON: {error}"
        ) from error
    expected = attestation["package_security"]["authenticode"]
    live_fields = {
        "Status": expected["status"],
        "SignerSubject": expected["signer_subject"],
        "SignerIssuer": expected["signer_issuer"],
        "SignerSerialNumber": expected["signer_serial_number"],
        "SignerThumbprint": expected["signer_thumbprint"],
        "CodeSigningEku": True,
        "TimeStamperSubject": expected["timestamp_subject"],
        "TimeStamperThumbprint": expected["timestamp_thumbprint"],
    }
    if signature != live_fields:
        raise EvidenceError("live Authenticode identity differs from the attestation")

    host_json = _run_powershell(
        "$os = Get-CimInstance Win32_OperatingSystem; "
        "$product = Get-ItemProperty "
        "'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion'; "
        "$identity = [Security.Principal.WindowsIdentity]::GetCurrent(); "
        "$principal = [Security.Principal.WindowsPrincipal]$identity; "
        "$elevated = $principal.IsInRole("
        "[Security.Principal.WindowsBuiltInRole]::Administrator); "
        "[ordered]@{"
        "WindowsEdition=$product.ProductName;"
        "WindowsBuild=($os.BuildNumber + '.' + $product.UBR);"
        "OsArchitecture=$(if ([Environment]::Is64BitOperatingSystem) {'x86_64'} else {'x86'});"
        "ProcessArchitecture=$(if ([Environment]::Is64BitProcess) {'x86_64'} else {'x86'});"
        "SessionId=[Diagnostics.Process]::GetCurrentProcess().SessionId;"
        "Elevated=$elevated;"
        "ProcessIntegrityLevel=$(if ($elevated) {'High'} else {'Medium'})} "
        "| ConvertTo-Json -Compress",
        "live Windows host state",
    )
    try:
        host = json.loads(host_json)
    except json.JSONDecodeError as error:
        raise EvidenceError(
            f"PowerShell returned invalid Windows host JSON: {error}"
        ) from error
    attested_host = attestation["host"]
    expected_host = {
        "WindowsEdition": attested_host["windows_edition"],
        "WindowsBuild": attested_host["windows_build"],
        "OsArchitecture": attested_host["os_architecture"],
        "ProcessArchitecture": attested_host["process_architecture"],
        "SessionId": attested_host["session_id"],
        "Elevated": attested_host["elevated"],
        "ProcessIntegrityLevel": attested_host["process_integrity_level"],
    }
    if host != expected_host:
        raise EvidenceError("live Windows host/session/integrity differs from the attestation")
    automation_available = _run_powershell(
        "Add-Type -AssemblyName UIAutomationClient; "
        "[bool][System.Windows.Automation.AutomationElement]::RootElement",
        "live Windows UI Automation state",
    )
    if automation_available.casefold() != "true":
        raise EvidenceError("Windows UI Automation is unavailable")
    signtool_output = _run_required(
        [str(signtool), "verify", "/pa", "/all", "/v", str(binary)],
        "signtool Authenticode verification",
    )

    printer_name = attestation["native_print"]["printer_name"]
    printer_json = _run_powershell(
        "Get-Printer -Name "
        + _powershell_quote(printer_name)
        + " | Select-Object Name,PrinterStatus,WorkOffline | ConvertTo-Json -Compress",
        "configured Windows printer state",
    )
    try:
        printer = json.loads(printer_json)
    except json.JSONDecodeError as error:
        raise EvidenceError(f"PowerShell returned invalid printer JSON: {error}") from error
    if printer.get("Name") != printer_name or printer.get("WorkOffline") is True:
        raise EvidenceError("attested Windows printer is unavailable or offline")
    if str(printer.get("PrinterStatus", "")).casefold() in {
        "error",
        "offline",
        "paperout",
    }:
        raise EvidenceError("attested Windows printer is in an error state")

    record_id = attestation["native_print"]["event_record_id"]
    event_json = _run_powershell(
        "$e = Get-WinEvent -LogName "
        "'Microsoft-Windows-PrintService/Operational' "
        f"-FilterXPath \"*[System[(EventRecordID={record_id})]]\" -MaxEvents 1; "
        "[ordered]@{Id=$e.Id;RecordId=$e.RecordId;Message=$e.Message} "
        "| ConvertTo-Json -Compress",
        "completed Windows print event",
    )
    try:
        event = json.loads(event_json)
    except json.JSONDecodeError as error:
        raise EvidenceError(f"PowerShell returned invalid print-event JSON: {error}") from error
    if event.get("Id") != 307 or event.get("RecordId") != record_id:
        raise EvidenceError("attested Windows event is not a completed print event 307")
    message = str(event.get("Message", ""))
    if (
        printer_name not in message
        or attestation["native_print"]["job_id"] not in message
        or attestation["native_print"]["document_name"] not in message
    ):
        raise EvidenceError("completed print event is not bound to the attested job and printer")
    return {
        "windows_edition": host["WindowsEdition"],
        "windows_build": host["WindowsBuild"],
        "os_architecture": host["OsArchitecture"],
        "process_architecture": host["ProcessArchitecture"],
        "session_id": host["SessionId"],
        "elevated": host["Elevated"],
        "process_integrity_level": host["ProcessIntegrityLevel"],
        "webview2_executable_sha256": webview2_record["sha256"],
        "ui_automation_available": True,
        "printer_available": True,
        "printer_name": printer_name,
        "completed_print_job_verified": True,
        "print_event_record_id": record_id,
        "print_event_output_sha256": artifact_common.sha256_bytes(
            event_json.encode("utf-8")
        ),
        "authenticode_valid": True,
        "signer_subject": signature["SignerSubject"],
        "signer_issuer": signature["SignerIssuer"],
        "signer_thumbprint": signature["SignerThumbprint"],
        "timestamp_signature_verified": True,
        "timestamp_thumbprint": signature["TimeStamperThumbprint"],
        "signtool_output_sha256": artifact_common.sha256_bytes(
            signtool_output.encode("utf-8")
        ),
    }


def verify_attestation_command(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    attestation_path: Path,
    pdf_verifier_path: Path,
    signtool_path: Path,
    report_path: Path,
) -> dict[str, Any]:
    candidate = validate_candidate_inputs(manifest_path, archive_path, identity_path)
    with tempfile.TemporaryDirectory(prefix="ebirforms-windows-certification-") as directory:
        package = certification_common.extract_portable_zip(archive_path, Path(directory))
        packaged = bind_extracted_package(candidate, package, identity_path)
        binding = {**candidate, "packaged_app": packaged}
        attestation, verified = validate_attestation(attestation_path, binding)
        owned_pdf_validation = verify_owned_pdf_artifact(
            attestation_path, attestation, pdf_verifier_path, verified
        )
        live = verify_live_windows_state(package, attestation, signtool_path)
        if artifact_common.tree_hash(package) != packaged["package_tree_sha256"]:
            raise EvidenceError("candidate changed during live Windows verification")
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
        "attestation": artifact_common.file_record(attestation_path),
        "verified_artifact_count": len(verified),
        "owned_pdf_validation": owned_pdf_validation,
        "live_windows_verification": live,
        "distribution_policy": {
            "candidate_format": "portable_zip",
            "distribution_track": "portable_candidate",
            "public_release_formats": PUBLIC_RELEASE_FORMATS,
            "public_release_allows_msix": False,
            "store_msix_policy": STORE_MSIX_POLICY,
        },
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            PUBLIC_INSTALLER_GAP,
            "collector executable and operator identity are not externally attested",
            "macOS and Linux native certification remain incomplete",
            "public EXE/MSI installer evidence is separate from this portable candidate",
        ],
    }
    artifact_common.write_json_atomic(report_path, report)
    return report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    inspect = subcommands.add_parser("inspect", help="verify and extract the exact candidate")
    inspect.add_argument("--candidate-manifest", required=True, type=Path)
    inspect.add_argument("--candidate-archive", required=True, type=Path)
    inspect.add_argument("--renderer-identity", required=True, type=Path)
    inspect.add_argument("--output-dir", required=True, type=Path)
    probe = subcommands.add_parser(
        "probe",
        help="launch the non-dev app with app and WebView2 networking blocked",
    )
    probe.add_argument("--candidate-manifest", required=True, type=Path)
    probe.add_argument("--candidate-archive", required=True, type=Path)
    probe.add_argument("--renderer-identity", required=True, type=Path)
    probe.add_argument("--output-dir", required=True, type=Path)
    probe.add_argument("--timeout", type=float, default=5.0)
    probe.add_argument("--webview2-executable", required=True, type=Path)
    verify = subcommands.add_parser(
        "verify-attestation", help="strictly verify a complete external Windows attestation"
    )
    verify.add_argument("--candidate-manifest", required=True, type=Path)
    verify.add_argument("--candidate-archive", required=True, type=Path)
    verify.add_argument("--renderer-identity", required=True, type=Path)
    verify.add_argument("--attestation", required=True, type=Path)
    verify.add_argument("--pdf-verifier", required=True, type=Path)
    verify.add_argument("--signtool", required=True, type=Path)
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
                probe_nondev_candidate(
                    binding,
                    arguments.output_dir.resolve(),
                    arguments.timeout,
                    arguments.webview2_executable,
                )
            print(arguments.output_dir.resolve())
            return 0
        verify_attestation_command(
            arguments.candidate_manifest,
            arguments.candidate_archive,
            arguments.renderer_identity,
            arguments.attestation,
            arguments.pdf_verifier,
            arguments.signtool,
            arguments.report,
        )
        print(arguments.report.resolve())
        return 0
    except (
        EvidenceError,
        OSError,
        json.JSONDecodeError,
        zipfile.BadZipFile,
        subprocess.SubprocessError,
    ) as error:
        print(f"Windows candidate certification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
