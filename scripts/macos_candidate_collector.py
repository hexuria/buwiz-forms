#!/usr/bin/env python3
"""Collect a closed, non-promotional macOS candidate attestation.

This operator-only tool launches the exact signed/notarized candidate under a
network-denial sandbox, challenge-binds the candidate's path-free runtime
observations, observes the real 2551Q preview and native save panel, requires a
completed CUPS print job, reruns the owned Rust PDF verifier, and assembles the
existing closed macOS attestation format.

The collector cannot inject failures into a production candidate.  It therefore
requires a separately retained rollback bundle covering every mandatory case;
missing or candidate-mismatched rollback evidence fails before the app starts.
The resulting attestation is permanently untrusted and non-promotional.
"""

from __future__ import annotations

import argparse
import getpass
import hashlib
import json
import math
import os
import platform
import secrets
import signal
import stat
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

import macos_candidate_certification as certification  # noqa: E402
import macos_native_evidence_driver as native_driver  # noqa: E402


SCHEMA_VERSION = 1
COLLECTOR_NAME = "ebirforms external macOS candidate collector"
COLLECTOR_VERSION = "1"
ROLLBACK_SCOPE = "external_macos_candidate_rollback_bundle"
FORM = certification.FORM
RUNTIME_SCOPE = "macos_candidate_runtime_observation"
RUNTIME_GAPS = [
    "runtime_self_authored",
    "external_ui_and_print_required",
    "external_candidate_binding_required",
]


EvidenceError = certification.EvidenceError


DIALOG_WATCHER_SWIFT = r'''
import AppKit
import CoreGraphics
import Foundation

struct WindowRecord: Codable {
    let id: UInt32
    let title: String
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

struct WatchResult: Codable {
    let preview: WindowRecord
    let dialog: WindowRecord
    let previewScreenshot: String
    let dialogScreenshot: String
}

enum WatchError: Error, CustomStringConvertible {
    case invalidEnvironment(String)
    case previewUnavailable(Int32)
    case dialogUnavailable(Int32)
    case screenshotFailed(String)

    var description: String {
        switch self {
        case .invalidEnvironment(let field): return "invalid environment: \(field)"
        case .previewUnavailable(let pid): return "2551Q preview unavailable for PID \(pid)"
        case .dialogUnavailable(let pid): return "native save dialog unavailable for PID \(pid)"
        case .screenshotFailed(let path): return "window screenshot failed: \(path)"
        }
    }
}

let environment = ProcessInfo.processInfo.environment
guard let pidText = environment["EBIR_COLLECTOR_PID"], let pid = Int32(pidText) else {
    throw WatchError.invalidEnvironment("EBIR_COLLECTOR_PID")
}
guard let previewScreenshot = environment["EBIR_COLLECTOR_PREVIEW_SCREENSHOT"],
      !previewScreenshot.isEmpty else {
    throw WatchError.invalidEnvironment("EBIR_COLLECTOR_PREVIEW_SCREENSHOT")
}
guard let dialogScreenshot = environment["EBIR_COLLECTOR_DIALOG_SCREENSHOT"],
      !dialogScreenshot.isEmpty else {
    throw WatchError.invalidEnvironment("EBIR_COLLECTOR_DIALOG_SCREENSHOT")
}
guard let readyPath = environment["EBIR_COLLECTOR_WATCHER_READY"],
      !readyPath.isEmpty else {
    throw WatchError.invalidEnvironment("EBIR_COLLECTOR_WATCHER_READY")
}

func processWindows(pid: Int32) -> [WindowRecord] {
    guard let records = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID
    ) as? [[String: Any]] else { return [] }
    return records.compactMap { record in
        guard let owner = record[kCGWindowOwnerPID as String] as? NSNumber,
              owner.int32Value == pid,
              let number = record[kCGWindowNumber as String] as? NSNumber,
              let bounds = record[kCGWindowBounds as String] as? [String: Any],
              let x = bounds["X"] as? NSNumber,
              let y = bounds["Y"] as? NSNumber,
              let width = bounds["Width"] as? NSNumber,
              let height = bounds["Height"] as? NSNumber else { return nil }
        return WindowRecord(
            id: number.uint32Value,
            title: (record[kCGWindowName as String] as? String) ?? "",
            x: x.doubleValue, y: y.doubleValue,
            width: width.doubleValue, height: height.doubleValue
        )
    }
}

func waitForPreview() throws -> WindowRecord {
    for _ in 0..<600 {
        if let preview = processWindows(pid: pid)
            .filter({ $0.title.contains("2551Q HTML Form Preview") })
            .max(by: { $0.width * $0.height < $1.width * $1.height }) {
            return preview
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    throw WatchError.previewUnavailable(pid)
}

func capture(_ window: WindowRecord, at path: String) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    process.arguments = ["-x", "-l", String(window.id), path]
    try process.run()
    process.waitUntilExit()
    if process.terminationStatus != 0 || !FileManager.default.fileExists(atPath: path) {
        throw WatchError.screenshotFailed(path)
    }
}

let preview = try waitForPreview()
try capture(preview, at: previewScreenshot)
let baseline = Set(processWindows(pid: pid).map(\.id))
guard FileManager.default.createFile(atPath: readyPath, contents: Data("ready\n".utf8)) else {
    throw WatchError.screenshotFailed(readyPath)
}
var dialog: WindowRecord? = nil
for _ in 0..<600 {
    dialog = processWindows(pid: pid)
        .filter({ !baseline.contains($0.id) && $0.width >= 300 && $0.height >= 180 })
        .max(by: { $0.width * $0.height < $1.width * $1.height })
    if dialog != nil { break }
    Thread.sleep(forTimeInterval: 0.1)
}
guard let dialog else { throw WatchError.dialogUnavailable(pid) }
try capture(dialog, at: dialogScreenshot)
let result = WatchResult(
    preview: preview,
    dialog: dialog,
    previewScreenshot: previewScreenshot,
    dialogScreenshot: dialogScreenshot
)
FileHandle.standardOutput.write(try JSONEncoder().encode(result))
FileHandle.standardOutput.write(Data("\n".utf8))
'''


# Reuse the reviewed exact-PID print-button targeting and dialog discovery from
# the development driver, but stop immediately after the real dialog appears.
# No event is sent inside the dialog; the operator must explicitly finish it.
OPEN_PRINT_DIALOG_SWIFT = native_driver.NATIVE_PRINT_CANCEL_SWIFT.split(
    "// The newly opened native sheet is already the exact process's key dialog.", 1
)[0] + r'''
let payload: [String: Any] = [
    "previewWindowId": initial.id,
    "previewWindowWidth": active.width,
    "previewWindowHeight": active.height,
    "clickX": printPoint.x,
    "clickY": printPoint.y,
    "dialogWindowId": dialog.id,
    "dialogWidth": dialog.width,
    "dialogHeight": dialog.height,
    "dialogObserved": true
]
FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: payload))
FileHandle.standardOutput.write(Data("\n".utf8))
'''


def utc_now() -> str:
    return datetime.now(UTC).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def require_exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    return certification.require_exact_keys(value, keys, label)


def require_sha256(value: Any, label: str) -> str:
    return certification.require_sha256(value, label)


def private_output_directory(path: Path) -> Path:
    for ancestor in (path, *path.parents):
        if ancestor.exists() and ancestor.is_symlink():
            raise EvidenceError("collector output directory may not traverse symlinks")
    if path.exists():
        if path.is_symlink() or not path.is_dir() or any(path.iterdir()):
            raise EvidenceError("collector output directory must be absent or an empty directory")
        path.chmod(0o700)
    else:
        path.mkdir(parents=True, mode=0o700)
    path = path.resolve(strict=True)
    metadata = path.stat()
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
        raise EvidenceError("collector output directory must be current-user-owned mode 0700")
    return path


def write_json(path: Path, value: dict[str, Any]) -> None:
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    temporary = path.with_name(f".{path.name}.{uuid.uuid4()}.partial")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        if temporary.exists():
            temporary.unlink()


def write_text(path: Path, value: str) -> None:
    temporary = path.with_name(f".{path.name}.{uuid.uuid4()}.partial")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
            stream.write(value)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
        os.chmod(path, 0o600)
    finally:
        if temporary.exists():
            temporary.unlink()


def candidate_binding(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    extraction_root: Path,
) -> dict[str, Any]:
    candidate = certification.validate_candidate_inputs(
        manifest_path, archive_path, identity_path
    )
    app = certification.extract_candidate_archive(archive_path, extraction_root)
    packaged = certification.bind_extracted_app(candidate, app, identity_path)
    return {**candidate, "packaged_app": packaged}


def expected_candidate(binding: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "app_tree_sha256": binding["packaged_app"]["app_tree_sha256"],
        "renderer_bundle_sha256": binding["packaged_app"]["renderer_bundle_sha256"],
    }


def load_rollback_bundle(path: Path, binding: dict[str, Any]) -> dict[str, Any]:
    path = certification.regular_file(path, "rollback evidence bundle")
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
        "rollback evidence bundle",
    )
    if bundle["schema_version"] != SCHEMA_VERSION or bundle["scope"] != ROLLBACK_SCOPE:
        raise EvidenceError("rollback bundle has an unsupported schema or scope")
    if bundle["promotion_eligible"] is not False or bundle["trusted_producer"] is not False:
        raise EvidenceError("rollback bundle must remain non-promotional and untrusted")
    if bundle["candidate"] != expected_candidate(binding):
        raise EvidenceError("rollback bundle does not bind the exact candidate")

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
        "rollback integrity",
    )
    normalized: dict[str, dict[str, Any]] = {}
    for field in integrity:
        normalized[field] = certification.verify_file_record(
            integrity[field], f"rollback {field}", base=base
        )
    for before_name, after_name, label in (
        ("destination_before", "destination_after", "destination"),
        ("draft_before", "draft_after", "draft"),
    ):
        before = normalized[before_name]
        after = normalized[after_name]
        if before["path"] == after["path"] or before["sha256"] != after["sha256"]:
            raise EvidenceError(f"rollback {label} snapshots are not distinct and preserved")
    temporary_manifest = certification.load_json(
        Path(normalized["temporary_files_manifest"]["path"]), limit=1024 * 1024
    )
    if temporary_manifest != {"remaining": []}:
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
        "bundle": native_driver.file_record(path),
    }


def validate_geometry_report(value: Any, label: str) -> dict[str, Any]:
    report = require_exact_keys(
        value, {"page_count", "page_width_pt", "page_height_pt", "pages"}, label
    )
    if report["page_count"] != 2 or report["page_width_pt"] != 612.0 or report[
        "page_height_pt"
    ] != 936.0:
        raise EvidenceError(f"{label} has incorrect paper geometry")
    pages = report["pages"]
    if not isinstance(pages, list) or len(pages) != 2:
        raise EvidenceError(f"{label} must contain exactly two pages")
    for page in pages:
        page = require_exact_keys(
            page,
            {
                "x",
                "y",
                "width",
                "height",
                "client_width",
                "client_height",
                "scroll_width",
                "scroll_height",
                "descendant_overflow_x",
                "descendant_overflow_y",
                "descendant_clipped_x",
                "descendant_clipped_y",
            },
            f"{label} page",
        )
        for field in (
            "descendant_overflow_x",
            "descendant_overflow_y",
            "descendant_clipped_x",
            "descendant_clipped_y",
        ):
            if type(page[field]) is not int or page[field] != 0:
                raise EvidenceError(f"{label} contains clipping or overflow")
        for field in (
            "x",
            "y",
            "width",
            "height",
            "client_width",
            "client_height",
            "scroll_width",
            "scroll_height",
        ):
            if isinstance(page[field], bool) or not isinstance(page[field], (int, float)):
                raise EvidenceError(f"{label} contains invalid geometry")
            if not math.isfinite(float(page[field])):
                raise EvidenceError(f"{label} contains invalid geometry")
        for field in (
            "width",
            "height",
            "client_width",
            "client_height",
            "scroll_width",
            "scroll_height",
        ):
            if page[field] <= 0:
                raise EvidenceError(f"{label} contains non-positive geometry")
    return report


def validate_destination_snapshot(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be an object")
    status = value.get("status")
    if status == "absent":
        return require_exact_keys(value, {"status"}, label)
    if status == "file":
        snapshot = require_exact_keys(value, {"status", "sha256"}, label)
        require_sha256(snapshot["sha256"], f"{label} hash")
        return snapshot
    if status == "unavailable":
        snapshot = require_exact_keys(value, {"status", "reason_code"}, label)
        if snapshot["reason_code"] not in {
            "metadata_read_failed",
            "not_regular_file",
            "file_read_failed",
        }:
            raise EvidenceError(f"{label} has an unknown unavailable reason")
        return snapshot
    raise EvidenceError(f"{label} has an unknown status")


def validate_runtime_observation(
    path: Path,
    *,
    challenge_sha256: str,
    expected_kind: str,
    destination: Path | None = None,
    destination_before_sha256: str | None = None,
) -> dict[str, Any]:
    observation = certification.load_json(path, limit=4 * 1024 * 1024)
    require_exact_keys(
        observation,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "collector_challenge_sha256",
            "form_code",
            "form_revision",
            "document_run_id_sha256",
            "envelope_sha256",
            "render_epoch",
            "readiness_revision",
            "issued_nonce",
            "preflight_consumptions",
            "backend_completion_nonce",
            "started_at_unix_ms",
            "completed_at_unix_ms",
            "geometry_reports",
            "output",
            "strict_verifier_gaps",
        },
        "candidate runtime observation",
    )
    constants = (
        observation["schema_version"] == 1,
        observation["scope"] == RUNTIME_SCOPE,
        observation["promotion_eligible"] is False,
        observation["trusted_producer"] is False,
        observation["form_code"] == "2551Q",
        observation["form_revision"] == "2018",
        observation["collector_challenge_sha256"] == challenge_sha256,
        observation["strict_verifier_gaps"] == RUNTIME_GAPS,
    )
    if not all(constants):
        raise EvidenceError("candidate runtime observation constants or challenge differ")
    for field in ("collector_challenge_sha256", "document_run_id_sha256", "envelope_sha256"):
        require_sha256(observation[field], f"runtime observation {field}")
    for field in ("render_epoch", "readiness_revision"):
        if type(observation[field]) is not int or observation[field] <= 0:
            raise EvidenceError(f"runtime observation {field} must be positive")
    nonce = observation["issued_nonce"]
    if type(nonce) is not int or nonce <= 0:
        raise EvidenceError("runtime output nonce must be positive")
    consumptions = observation["preflight_consumptions"]
    completion_nonce = observation["backend_completion_nonce"]
    if (
        not isinstance(consumptions, list)
        or len(consumptions) != 1
        or type(consumptions[0]) is not int
        or consumptions[0] != nonce
        or type(completion_nonce) is not int
        or completion_nonce != nonce
    ):
        raise EvidenceError("runtime observation reused or changed its one-use nonce")
    started_at = observation["started_at_unix_ms"]
    completed_at = observation["completed_at_unix_ms"]
    if (
        type(started_at) is not int
        or type(completed_at) is not int
        or started_at <= 0
        or completed_at < started_at
    ):
        raise EvidenceError("runtime observation timestamps are invalid")
    reports = observation["geometry_reports"]
    if not isinstance(reports, list) or len(reports) != 2:
        raise EvidenceError("runtime observation requires two geometry reports")
    first = validate_geometry_report(reports[0], "first geometry report")
    second = validate_geometry_report(reports[1], "second geometry report")
    if first != second:
        raise EvidenceError("runtime geometry reports are not byte-equivalent")

    output = observation["output"]
    if expected_kind == "pdf_export_succeeded":
        require_exact_keys(
            output,
            {
                "kind",
                "wkpdf_pages",
                "output_pdf_sha256",
                "output_pdf_byte_count",
                "pdf_validation",
                "destination_before",
                "destination_after",
                "temporary_file_remaining",
            },
            "PDF runtime output",
        )
        if output["kind"] != expected_kind or output["temporary_file_remaining"] is not False:
            raise EvidenceError("runtime PDF output did not complete safely")
        before = validate_destination_snapshot(
            output["destination_before"], "runtime destination-before snapshot"
        )
        after = validate_destination_snapshot(
            output["destination_after"], "runtime destination-after snapshot"
        )
        if destination_before_sha256 is not None:
            require_sha256(destination_before_sha256, "expected destination-before hash")
            if before != {"status": "file", "sha256": destination_before_sha256}:
                raise EvidenceError(
                    "runtime destination-before snapshot differs from the collector challenge"
                )
        if destination is None:
            raise EvidenceError("runtime PDF validation requires the exported destination")
        record = native_driver.file_record(destination)
        if (
            type(output["output_pdf_byte_count"]) is not int
            or output["output_pdf_byte_count"] <= 0
            or output["output_pdf_sha256"] != record["sha256"]
            or output["output_pdf_byte_count"] != record["byte_count"]
        ):
            raise EvidenceError("runtime PDF observation differs from exported bytes")
        if after != {"status": "file", "sha256": record["sha256"]}:
            raise EvidenceError("runtime destination snapshot differs from exported bytes")
        validation = require_exact_keys(
            output["pdf_validation"],
            {"page_count", "width_points", "height_points", "content_nonempty", "validated_by"},
            "runtime PDF validation",
        )
        if validation != {
            "page_count": 2,
            "width_points": 612.0,
            "height_points": 936.0,
            "content_nonempty": True,
            "validated_by": "bir-print::html_output::validate_pdf_file",
        }:
            raise EvidenceError("runtime PDF validation result is incomplete")
        pages = output["wkpdf_pages"]
        if not isinstance(pages, list) or len(pages) != 2:
            raise EvidenceError("runtime PDF observation omitted WKPDF page callbacks")
        for index, page in enumerate(pages, 1):
            require_exact_keys(page, {"page", "byte_count", "sha256"}, f"WKPDF page {index}")
            if (
                type(page["page"]) is not int
                or page["page"] != index
                or type(page["byte_count"]) is not int
                or page["byte_count"] <= 0
            ):
                raise EvidenceError("runtime WKPDF callback is invalid")
            require_sha256(page["sha256"], f"WKPDF page {index} hash")
    elif expected_kind == "system_print_completed":
        require_exact_keys(output, {"kind", "appkit_completion_succeeded"}, "print runtime output")
        if output != {"kind": expected_kind, "appkit_completion_succeeded": True}:
            raise EvidenceError("runtime system print did not complete successfully")
    else:
        raise EvidenceError(f"unsupported runtime observation kind: {expected_kind}")
    return observation


def wait_for_observation(
    directory: Path,
    *,
    challenge_sha256: str,
    kind: str,
    timeout: float,
    destination: Path | None = None,
    destination_before_sha256: str | None = None,
) -> tuple[Path, dict[str, Any]]:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        for path in sorted(directory.glob("runtime-*.json")):
            try:
                value = validate_runtime_observation(
                    path,
                    challenge_sha256=challenge_sha256,
                    expected_kind=kind,
                    destination=destination,
                    destination_before_sha256=destination_before_sha256,
                )
                return path, value
            except (EvidenceError, OSError, json.JSONDecodeError) as error:
                last_error = error
        time.sleep(0.2)
    detail = f": {last_error}" if last_error else ""
    raise EvidenceError(f"timed out waiting for {kind} runtime observation{detail}")


def run_swift(source: str, environment: dict[str, str], *, timeout: float) -> dict[str, Any]:
    try:
        result = subprocess.run(
            ["/usr/bin/xcrun", "swift", "-e", source],
            env=os.environ | environment,
            text=True,
            capture_output=True,
            check=False,
            timeout=timeout,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"macOS UI helper is unavailable: {error}") from error
    if result.returncode != 0:
        raise EvidenceError(
            f"macOS UI helper failed closed: {(result.stderr or result.stdout).strip()}"
        )
    try:
        value = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise EvidenceError("macOS UI helper returned invalid JSON") from error
    if not isinstance(value, dict):
        raise EvidenceError("macOS UI helper returned a non-object")
    return value


def validate_window_record(value: Any, label: str) -> dict[str, Any]:
    window = require_exact_keys(value, {"id", "title", "x", "y", "width", "height"}, label)
    if type(window["id"]) is not int or window["id"] <= 0:
        raise EvidenceError(f"{label} has an invalid window identifier")
    if not isinstance(window["title"], str):
        raise EvidenceError(f"{label} has an invalid title")
    for field in ("x", "y", "width", "height"):
        number = window[field]
        if isinstance(number, bool) or not isinstance(number, (int, float)):
            raise EvidenceError(f"{label} has invalid geometry")
        if not math.isfinite(float(number)):
            raise EvidenceError(f"{label} has invalid geometry")
    if window["width"] <= 0 or window["height"] <= 0:
        raise EvidenceError(f"{label} has non-positive geometry")
    return window


def validate_rectangle(value: Any, label: str) -> dict[str, Any]:
    rectangle = require_exact_keys(value, {"x", "y", "width", "height"}, label)
    for field in ("x", "y", "width", "height"):
        number = rectangle[field]
        if isinstance(number, bool) or not isinstance(number, (int, float)):
            raise EvidenceError(f"{label} has invalid geometry")
        if not math.isfinite(float(number)):
            raise EvidenceError(f"{label} has invalid geometry")
    if rectangle["width"] <= 0 or rectangle["height"] <= 0:
        raise EvidenceError(f"{label} has non-positive geometry")
    return rectangle


def validate_export_automation(value: Any) -> dict[str, Any]:
    record = require_exact_keys(
        value,
        {
            "initial",
            "active",
            "firstClickX",
            "firstClickY",
            "secondClickX",
            "secondClickY",
        },
        "exact-PID export automation",
    )
    initial = validate_rectangle(record["initial"], "initial preview rectangle")
    active = validate_rectangle(record["active"], "active preview rectangle")
    points = (
        ("firstClickX", "firstClickY", initial),
        ("secondClickX", "secondClickY", active),
    )
    for x_field, y_field, rectangle in points:
        x = record[x_field]
        y = record[y_field]
        if (
            isinstance(x, bool)
            or isinstance(y, bool)
            or not isinstance(x, (int, float))
            or not isinstance(y, (int, float))
            or not math.isfinite(float(x))
            or not math.isfinite(float(y))
        ):
            raise EvidenceError("exact-PID export automation has an invalid click point")
        if not math.isclose(x, rectangle["x"] + rectangle["width"] - 183.0, abs_tol=0.01):
            raise EvidenceError("exact-PID export automation did not target Export PDF")
        if not math.isclose(y, rectangle["y"] + 58.0, abs_tol=0.01):
            raise EvidenceError("exact-PID export automation did not target Export PDF")
    return record


def validate_save_chooser_observation(
    value: Any, *, preview_screenshot: Path, chooser_screenshot: Path
) -> dict[str, Any]:
    observation = require_exact_keys(
        value,
        {"preview", "dialog", "previewScreenshot", "dialogScreenshot"},
        "native save chooser observation",
    )
    preview = validate_window_record(observation["preview"], "2551Q preview window")
    dialog = validate_window_record(observation["dialog"], "native save chooser window")
    if preview["title"] != "2551Q HTML Form Preview":
        raise EvidenceError("visible preview observer did not identify 2551Q")
    if dialog["id"] == preview["id"]:
        raise EvidenceError("native save chooser reused the preview window identifier")
    if observation["previewScreenshot"] != str(preview_screenshot):
        raise EvidenceError("preview screenshot path differs from the collector-owned path")
    if observation["dialogScreenshot"] != str(chooser_screenshot):
        raise EvidenceError("save chooser screenshot path differs from the collector-owned path")
    return observation


def validate_print_dialog_observation(value: Any) -> dict[str, Any]:
    record = require_exact_keys(
        value,
        {
            "previewWindowId",
            "previewWindowWidth",
            "previewWindowHeight",
            "clickX",
            "clickY",
            "dialogWindowId",
            "dialogWidth",
            "dialogHeight",
            "dialogObserved",
        },
        "native print dialog observation",
    )
    for field in ("previewWindowId", "dialogWindowId"):
        if type(record[field]) is not int or record[field] <= 0:
            raise EvidenceError("native print dialog observation has an invalid window identifier")
    if record["previewWindowId"] == record["dialogWindowId"]:
        raise EvidenceError("native print dialog reused the preview window identifier")
    for field in (
        "previewWindowWidth",
        "previewWindowHeight",
        "clickX",
        "clickY",
        "dialogWidth",
        "dialogHeight",
    ):
        number = record[field]
        if (
            isinstance(number, bool)
            or not isinstance(number, (int, float))
            or not math.isfinite(float(number))
        ):
            raise EvidenceError("native print dialog observation has invalid geometry")
    for field in ("previewWindowWidth", "previewWindowHeight", "dialogWidth", "dialogHeight"):
        if record[field] <= 0:
            raise EvidenceError("native print dialog observation has non-positive geometry")
    if record["dialogObserved"] is not True:
        raise EvidenceError("native print dialog was not observed")
    return record


def protect_screenshot(path: Path, label: str) -> None:
    screenshot = certification.regular_file(path, label)
    if screenshot.stat().st_size <= 0:
        raise EvidenceError(f"{label} is empty")
    path.chmod(0o600)


def create_private_file(path: Path) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    os.close(descriptor)


def stop_helper(process: subprocess.Popen[str] | None) -> None:
    if process is None or process.poll() is not None:
        return
    process.terminate()
    try:
        process.communicate(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.communicate()


def accessibility_record(path: Path, automation_identity: str) -> dict[str, Any]:
    result = subprocess.run(
        [
            "/usr/bin/osascript",
            "-e",
            'tell application "System Events" to get UI elements enabled',
        ],
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )
    if result.returncode != 0 or result.stdout.strip().lower() != "true":
        raise EvidenceError("macOS Accessibility permission is unavailable")
    value = {
        "permission_granted": True,
        "automation_identity": automation_identity,
        "observed_at_utc": utc_now(),
        "challenge": "permission queried by an external process",
    }
    write_json(path, value)
    return value


def run_required(command: list[str], label: str) -> str:
    result = subprocess.run(command, text=True, capture_output=True, check=False, timeout=60)
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"{label} failed closed: {output}")
    return output


def package_security(app: Path, output: Path) -> dict[str, Any]:
    signature = native_driver.codesign_record(app)
    if not signature["developer_id_signed"] or not signature.get("team_identifier"):
        raise EvidenceError("candidate is not Developer ID signed")
    authorities = [
        value for value in signature["authority"] if value.startswith("Developer ID Application:")
    ]
    if len(authorities) != 1:
        raise EvidenceError("candidate must expose exactly one Developer ID Application authority")
    codesign_output = run_required(
        ["/usr/bin/codesign", "-d", "--verbose=4", str(app)], "codesign detail"
    )
    gatekeeper_output = run_required(
        ["/usr/sbin/spctl", "--assess", "--type", "execute", "--verbose=4", str(app)],
        "Gatekeeper assessment",
    )
    if "notarized developer id" not in gatekeeper_output.lower():
        raise EvidenceError("Gatekeeper did not identify a notarized Developer ID candidate")
    stapler_output = run_required(
        ["/usr/bin/xcrun", "stapler", "validate", str(app)], "stapled ticket validation"
    )
    codesign_path = output / "codesign.txt"
    gatekeeper_path = output / "gatekeeper.txt"
    stapler_path = output / "stapler.txt"
    write_text(codesign_path, codesign_output + "\n")
    write_text(gatekeeper_path, gatekeeper_output + "\n")
    write_text(stapler_path, stapler_output + "\n")
    return {
        "codesign": {
            "passed": True,
            "developer_id_signed": True,
            "authority": authorities[0],
            "team_identifier": signature["team_identifier"],
            "artifact": native_driver.file_record(codesign_path),
        },
        "notarization": {
            "passed": True,
            "gatekeeper_accepted": True,
            "artifact": native_driver.file_record(gatekeeper_path),
        },
        "stapling": {
            "passed": True,
            "artifact": native_driver.file_record(stapler_path),
        },
    }


def completed_jobs(printer: str) -> tuple[dict[str, str], str]:
    output = run_required(
        ["/usr/bin/lpstat", "-W", "completed", "-o", printer], "completed CUPS jobs"
    )
    jobs = {
        line.split(maxsplit=1)[0]: line
        for line in output.splitlines()
        if line.strip()
    }
    return jobs, output


def wait_for_completed_job(printer: str, before: set[str], timeout: float) -> tuple[str, str]:
    deadline = time.monotonic() + timeout
    latest = ""
    while time.monotonic() < deadline:
        jobs, latest = completed_jobs(printer)
        created = sorted(set(jobs) - before)
        if len(created) == 1:
            return created[0], latest
        if len(created) > 1:
            raise EvidenceError("more than one new completed CUPS job prevents causal binding")
        time.sleep(1.0)
    raise EvidenceError("timed out waiting for the completed native print job")


def command_executes_binary(command: str, binary: Path) -> bool:
    executable = str(binary.resolve())
    return command == executable or command.startswith(executable + " ")


def find_candidate_pid(binary: Path, launcher_pid: int, timeout: float) -> int:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = subprocess.run(
            ["/bin/ps", "-axo", "pid=,command="], text=True, capture_output=True, check=True
        )
        matches: list[int] = []
        for line in result.stdout.splitlines():
            stripped = line.strip()
            pid_text, separator, command = stripped.partition(" ")
            command = command.lstrip()
            if (
                separator
                and command_executes_binary(command, binary)
                and native_driver.DEV_FLAG not in command
            ):
                matches.append(int(pid_text))
        if launcher_pid in matches:
            return launcher_pid
        if len(matches) == 1:
            return matches[0]
        if len(matches) > 1:
            raise EvidenceError("multiple exact candidate processes are running")
        if subprocess.run(
            ["/bin/kill", "-0", str(launcher_pid)], capture_output=True, check=False
        ).returncode != 0:
            raise EvidenceError("candidate exited before its preview opened")
        time.sleep(0.1)
    raise EvidenceError("timed out locating the exact candidate process")


def terminate(process: subprocess.Popen[str], app_pid: int | None) -> None:
    if app_pid is not None:
        try:
            os.kill(app_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        process.terminate()
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def run_pdf_verifier(
    verifier: Path, pdf: Path, envelope_sha256: str, artifact: Path
) -> dict[str, Any]:
    verifier = certification.regular_file(verifier, "owned PDF verifier")
    if not os.access(verifier, os.X_OK):
        raise EvidenceError("owned PDF verifier is not executable")
    result = subprocess.run(
        [str(verifier), str(pdf), envelope_sha256],
        capture_output=True,
        check=False,
        timeout=60,
    )
    if result.returncode != 0 or result.stderr:
        raise EvidenceError(
            "owned PDF verifier rejected the export: "
            + result.stderr.decode("utf-8", errors="replace").strip()
        )
    try:
        report = json.loads(result.stdout)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError("owned PDF verifier returned invalid JSON") from error
    if not isinstance(report, dict):
        raise EvidenceError("owned PDF verifier returned a non-object")
    if report.get("scope") != "owned_macos_candidate_pdf_validation":
        raise EvidenceError("owned PDF verifier returned the wrong platform scope")
    if report.get("actual_page_count") != 2 or report.get("width_points") != 612.0 or report.get(
        "height_points"
    ) != 936.0 or report.get("content_nonempty") is not True:
        raise EvidenceError("owned PDF verifier did not prove the required two-page geometry")
    write_text(artifact, result.stdout.decode("utf-8"))
    return report


def geometry_measurements(observation: dict[str, Any]) -> list[dict[str, Any]]:
    measurements: list[dict[str, Any]] = []
    for index, report in enumerate(observation["geometry_reports"], 1):
        measurements.append(
            {
                "measurement_index": index,
                "page_width_pt": report["page_width_pt"],
                "page_height_pt": report["page_height_pt"],
                "pages": [
                    {
                        "page": page_number,
                        "x": page["x"],
                        "y": page["y"],
                        "width_pt": report["page_width_pt"],
                        "height_pt": report["page_height_pt"],
                    }
                    for page_number, page in enumerate(report["pages"], 1)
                ],
                "clipping_count": 0,
                "overflow_count": 0,
            }
        )
    return measurements


def host_identifier_sha256() -> str:
    try:
        raw = run_required(
            ["/usr/sbin/ioreg", "-rd1", "-c", "IOPlatformExpertDevice"],
            "macOS host identity",
        )
    except EvidenceError:
        raw = f"{platform.node()}:{getpass.getuser()}"
    return sha256_text(raw)


def collect(arguments: argparse.Namespace) -> Path:
    if platform.system() != "Darwin":
        raise EvidenceError("the external macOS collector must run on macOS")
    if not arguments.allow_live_print:
        raise EvidenceError(
            "--allow-live-print is required because this run completes a real print job"
        )
    if not isinstance(arguments.printer, str) or not arguments.printer.strip():
        raise EvidenceError("--printer must name the configured certification printer")
    if not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        raise EvidenceError("--timeout must be a positive finite number")
    if (
        not isinstance(arguments.automation_identity, str)
        or not arguments.automation_identity.strip()
    ):
        raise EvidenceError("--automation-identity must be non-empty")
    started_at = utc_now()
    output = private_output_directory(arguments.output_dir)
    extraction = output / "candidate"
    binding = candidate_binding(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        extraction,
    )
    rollback = load_rollback_bundle(arguments.rollback_bundle, binding)
    app = Path(binding["packaged_app"]["app_path"])
    binary = Path(binding["packaged_app"]["binary"]["path"])
    app_hash_before = native_driver.tree_hash(app)
    security = package_security(app, output)
    accessibility = accessibility_record(
        output / "accessibility.json", arguments.automation_identity
    )
    printer_state = run_required(
        ["/usr/bin/lpstat", "-p", arguments.printer], "configured printer state"
    )
    if "disabled" in printer_state.lower():
        raise EvidenceError("configured certification printer is disabled")
    jobs_before, jobs_before_raw = completed_jobs(arguments.printer)
    printer_state_artifact = output / "configured-printer.txt"
    cups_before_artifact = output / "cups-completed-before.txt"
    write_text(printer_state_artifact, printer_state + "\n")
    write_text(cups_before_artifact, jobs_before_raw + "\n")

    observations = output / "runtime-observations"
    observations.mkdir(mode=0o700)
    challenge = secrets.token_hex(32)
    challenge_sha256 = sha256_text(challenge)
    export_pdf = output / "2551q-toolbar-export.pdf"
    destination_before_artifact = output / "export-destination-before.bin"
    # The reviewed external automation confirms the native Replace sheet.  A
    # pre-existing destination makes its final Return key causal and prevents
    # that key from falling through to the preview window.
    destination_challenge = secrets.token_bytes(32)
    destination_before_artifact.write_bytes(destination_challenge)
    destination_before_artifact.chmod(0o600)
    export_pdf.write_bytes(destination_challenge)
    export_pdf.chmod(0o600)
    destination_before_sha256 = hashlib.sha256(destination_challenge).hexdigest()
    preview_screenshot = output / "2551q-preview-toolbar.png"
    chooser_screenshot = output / "native-save-chooser.png"
    watcher_ready = output / ".save-chooser-watcher-ready"
    print_screenshot = output / "native-print-dialog.png"
    stdout_path = output / "candidate.stdout.log"
    stderr_path = output / "candidate.stderr.log"
    create_private_file(stdout_path)
    create_private_file(stderr_path)
    environment = os.environ.copy()
    environment["EBIR_CERTIFICATION_EVIDENCE_DIR"] = str(observations)
    environment["EBIR_CERTIFICATION_EVIDENCE_CHALLENGE"] = challenge
    environment.pop("DEVELOPER_MODE", None)
    for key in list(environment):
        if key.startswith("EBIR_NATIVE_EVIDENCE") or key.startswith("EBIR_NATIVE_OUTPUT"):
            environment.pop(key, None)
    sandbox = Path("/usr/bin/sandbox-exec")
    if not sandbox.is_file():
        raise EvidenceError("sandbox-exec is required for the network-denied candidate launch")
    launch_argv = [
        str(sandbox),
        "-p",
        "(version 1) (allow default) (deny network*)",
        str(binary),
    ]
    process: subprocess.Popen[str] | None = None
    watcher: subprocess.Popen[str] | None = None
    app_pid: int | None = None
    try:
        with (
            stdout_path.open("w", encoding="utf-8") as stdout_stream,
            stderr_path.open("w", encoding="utf-8") as stderr_stream,
        ):
            process = subprocess.Popen(
                launch_argv,
                env=environment,
                text=True,
                stdout=stdout_stream,
                stderr=stderr_stream,
            )
        app_pid = find_candidate_pid(binary, process.pid, arguments.timeout)
        print(
            "Open a real 2551Q draft in the launched candidate, then click Print Preview. "
            "Leave the 2551Q HTML Form Preview visible."
        )
        input("Press Return only after that preview is visible: ")

        watcher_environment = {
            "EBIR_COLLECTOR_PID": str(app_pid),
            "EBIR_COLLECTOR_PREVIEW_SCREENSHOT": str(preview_screenshot),
            "EBIR_COLLECTOR_DIALOG_SCREENSHOT": str(chooser_screenshot),
            "EBIR_COLLECTOR_WATCHER_READY": str(watcher_ready),
        }
        watcher = subprocess.Popen(
            ["/usr/bin/xcrun", "swift", "-e", DIALOG_WATCHER_SWIFT],
            env=os.environ | watcher_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        watcher_deadline = time.monotonic() + arguments.timeout
        while not watcher_ready.is_file() and time.monotonic() < watcher_deadline:
            if watcher.poll() is not None:
                watcher_stdout, watcher_stderr = watcher.communicate()
                raise EvidenceError(
                    "native save chooser observer exited before it was ready: "
                    + (watcher_stderr or watcher_stdout).strip()
                )
            time.sleep(0.1)
        if not watcher_ready.is_file():
            watcher.kill()
            watcher.communicate()
            raise EvidenceError("timed out preparing the native save chooser observer")
        watcher_ready.unlink()
        export_ui = validate_export_automation(
            native_driver.run_native_export(
                app_pid, export_pdf, timeout=arguments.timeout
            )
        )
        try:
            watcher_stdout, watcher_stderr = watcher.communicate(timeout=arguments.timeout)
        except subprocess.TimeoutExpired:
            watcher.kill()
            watcher.communicate()
            raise EvidenceError("timed out observing the native save chooser")
        if watcher.returncode != 0:
            raise EvidenceError(
                f"native save chooser observer failed: {(watcher_stderr or watcher_stdout).strip()}"
            )
        save_chooser = validate_save_chooser_observation(
            json.loads(watcher_stdout),
            preview_screenshot=preview_screenshot,
            chooser_screenshot=chooser_screenshot,
        )
        if not preview_screenshot.is_file() or not chooser_screenshot.is_file():
            raise EvidenceError("preview or native save chooser screenshot was not retained")
        protect_screenshot(preview_screenshot, "2551Q preview screenshot")
        protect_screenshot(chooser_screenshot, "native save chooser screenshot")
        pdf_observation_path, pdf_observation = wait_for_observation(
            observations,
            challenge_sha256=challenge_sha256,
            kind="pdf_export_succeeded",
            timeout=arguments.timeout,
            destination=export_pdf,
            destination_before_sha256=destination_before_sha256,
        )

        print_dialog = validate_print_dialog_observation(
            run_swift(
                OPEN_PRINT_DIALOG_SWIFT,
                {"EBIR_NATIVE_EVIDENCE_PID": str(app_pid)},
                timeout=arguments.timeout,
            )
        )
        run_required(
            [
                "/usr/sbin/screencapture",
                "-x",
                "-l",
                str(print_dialog["dialogWindowId"]),
                str(print_screenshot),
            ],
            "native print dialog screenshot",
        )
        protect_screenshot(print_screenshot, "native print dialog screenshot")
        print(
            f"The native print dialog is open. Select exactly {arguments.printer!r}, "
            "review the job, and click Print. This will produce a real printer job."
        )
        input("Press Return after you have clicked Print: ")
        print_job_id, jobs_after_raw = wait_for_completed_job(
            arguments.printer, set(jobs_before), arguments.timeout
        )
        cups_after_artifact = output / "cups-completed-after.txt"
        write_text(cups_after_artifact, jobs_after_raw + "\n")
        print_observation_path, print_observation = wait_for_observation(
            observations,
            challenge_sha256=challenge_sha256,
            kind="system_print_completed",
            timeout=arguments.timeout,
        )
        for field in ("document_run_id_sha256", "envelope_sha256"):
            if pdf_observation[field] != print_observation[field]:
                raise EvidenceError("PDF and print observations do not share one preview document")
        if pdf_observation["issued_nonce"] == print_observation["issued_nonce"]:
            raise EvidenceError("PDF and print reused one output nonce")

        pdf_verifier_artifact = output / "owned-pdf-verifier.json"
        pdf_report = run_pdf_verifier(
            arguments.pdf_verifier,
            export_pdf,
            pdf_observation["envelope_sha256"],
            pdf_verifier_artifact,
        )
        export_artifact = output / "toolbar-export-observation.json"
        write_json(
            export_artifact,
            {
                "challenge_sha256": challenge_sha256,
                "exact_pid": app_pid,
                "preview": save_chooser["preview"],
                "save_chooser": save_chooser["dialog"],
                "automation": export_ui,
                "preview_screenshot": native_driver.file_record(preview_screenshot),
                "save_chooser_screenshot": native_driver.file_record(chooser_screenshot),
                "runtime_observation": native_driver.file_record(pdf_observation_path),
                "destination_before": native_driver.file_record(
                    destination_before_artifact
                ),
                "output": native_driver.file_record(export_pdf),
            },
        )
        print_artifact = output / "native-print-observation.json"
        write_json(
            print_artifact,
            {
                "challenge_sha256": challenge_sha256,
                "exact_pid": app_pid,
                "dialog": print_dialog,
                "dialog_screenshot": native_driver.file_record(print_screenshot),
                "printer": arguments.printer,
                "completed_job_id": print_job_id,
                "configured_printer": native_driver.file_record(
                    printer_state_artifact
                ),
                "completed_jobs_before": native_driver.file_record(
                    cups_before_artifact
                ),
                "completed_jobs_after": native_driver.file_record(
                    cups_after_artifact
                ),
                "runtime_observation": native_driver.file_record(print_observation_path),
            },
        )
    finally:
        stop_helper(watcher)
        if process is not None:
            terminate(process, app_pid)

    if app_pid is None:
        raise EvidenceError("candidate process was never established")
    app_hash_after = native_driver.tree_hash(app)
    if app_hash_after != app_hash_before:
        raise EvidenceError("candidate application changed during collection")
    network_artifact = output / "network-denial.json"
    write_json(
        network_artifact,
        {
            "mechanism": "sandbox-exec deny network*",
            "launch_argv": launch_argv,
            "exercised": True,
            "enforced_for_launch": True,
            "passed": True,
        },
    )
    runtime_artifact = output / "runtime.json"
    write_json(
        runtime_artifact,
        {
            "challenge_sha256": challenge_sha256,
            "pid": app_pid,
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "launch_argv": launch_argv,
            "stdout": native_driver.file_record(stdout_path),
            "stderr": native_driver.file_record(stderr_path),
            "app_tree_sha256_before": app_hash_before,
            "app_tree_sha256_after": app_hash_after,
        },
    )

    completed_at = utc_now()
    script_record = native_driver.file_record(Path(__file__).resolve())
    strict_gaps = [
        certification.NON_PROMOTIONAL_GAP,
        "candidate runtime observations are self-authored and only challenge-bound",
        "operator UI and printer actions are an untrusted local attestation",
        "rollback artifacts are externally supplied and not generated by this collector",
        "Windows and Linux native certification remain incomplete",
        *rollback["strict_verifier_gaps"],
    ]
    strict_gaps = list(dict.fromkeys(strict_gaps))
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
        "accessibility": {
            "permission_granted": True,
            "automation_identity": accessibility["automation_identity"],
            "artifact": native_driver.file_record(output / "accessibility.json"),
        },
        "runtime": {
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "launch_argv": launch_argv,
            "pid": app_pid,
            "network_denial": {
                "mechanism": "sandbox-exec deny network*",
                "exercised": True,
                "enforced_for_launch": True,
                "passed": True,
                "artifact": native_driver.file_record(network_artifact),
            },
            "artifact": native_driver.file_record(runtime_artifact),
        },
        "preview": {
            "exercised": True,
            "passed": True,
            "window_title": "2551Q HTML Form Preview",
            "document_run_id": pdf_observation["document_run_id_sha256"],
            "envelope_sha256": pdf_observation["envelope_sha256"],
            "nonce": pdf_observation["issued_nonce"],
            "page_count": 2,
            "geometry_measurements": geometry_measurements(pdf_observation),
            "artifact": native_driver.file_record(preview_screenshot),
        },
        "toolbar_export": {
            "exercised": True,
            "passed": True,
            "control": "Export PDF",
            "save_chooser_exercised": True,
            "destination_path": str(export_pdf.resolve()),
            "nonce": pdf_observation["issued_nonce"],
            "artifact": native_driver.file_record(export_artifact),
        },
        "native_print": {
            "exercised": True,
            "passed": True,
            "completed": True,
            "printer_name": arguments.printer,
            "job_id": print_job_id,
            "artifact": native_driver.file_record(print_artifact),
        },
        "pdf_validation": {
            "exercised": True,
            "passed": True,
            "output": native_driver.file_record(export_pdf),
            "expected_page_count": 2,
            "actual_page_count": pdf_report["actual_page_count"],
            "pages": pdf_report["pages"],
            "content_nonempty": pdf_report["content_nonempty"],
            "validated_by": pdf_report["validated_by"],
            "verifier_executable_sha256": native_driver.file_record(
                arguments.pdf_verifier
            )["sha256"],
            "artifact": native_driver.file_record(pdf_verifier_artifact),
        },
        "package_security": security,
        "integrity": {
            "app_tree_sha256_before": app_hash_before,
            "app_tree_sha256_after": app_hash_after,
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
    attestation_path = output / "macos-candidate-attestation.json"
    write_json(attestation_path, attestation)
    certification.validate_attestation(attestation_path, binding)
    report_path = output / "macos-candidate-certification-report.json"
    certification.verify_attestation_command(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        attestation_path,
        arguments.pdf_verifier,
        report_path,
    )
    return report_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--candidate-archive", required=True, type=Path)
    parser.add_argument("--renderer-identity", required=True, type=Path)
    parser.add_argument("--pdf-verifier", required=True, type=Path)
    parser.add_argument("--rollback-bundle", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--printer", required=True)
    parser.add_argument(
        "--automation-identity", default="macos_candidate_collector.py operator"
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
        print(f"macOS candidate collection failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
