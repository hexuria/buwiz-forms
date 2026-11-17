#!/usr/bin/env python3
"""External, non-promotional macOS HTML-output evidence driver.

The driver never builds or modifies the application package. It independently
hashes a prepared ``.app`` and its bundled renderer twice, launches the
development-only deterministic 2551Q preview, queues reviewed destinations
through the development-only harness into the same immutable-envelope output
state machine used by the Export PDF toolbar button, and writes a reviewable
transcript outside the package. A second invocation with ``verify`` recomputes every available
artifact hash and rejects promotion/trust claims.

This remains a diagnostic foundation. Accessibility permission, a prepared
dev-tools package, and (for system print) an operator/printer are external
requirements. The transcript is intentionally ineligible for release
promotion and is never installed into ``form-release-evidence.json``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import plistlib
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Iterable


SCHEMA_VERSION = 1
SCOPE = "external_macos_diagnostic_non_promotional"
RENDERER_RELATIVE_PATH = Path("Contents/Resources/assets/form-renderer")
IDENTITY_RELATIVE_PATH = Path(
    "Contents/Resources/assets/form-renderer-build-identity.json"
)
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_REVISION = re.compile(r"[0-9a-f]{40}\Z")
PARTIAL_PDF = re.compile(r"^\..+\.[A-Za-z0-9]+\.partial\.pdf$")


class EvidenceError(RuntimeError):
    """Fail-closed evidence or driver error."""


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def read_stable_file(path: Path, *, limit: int | None = None) -> bytes:
    first_stat = path.lstat()
    if stat.S_ISLNK(first_stat.st_mode) or not stat.S_ISREG(first_stat.st_mode):
        raise EvidenceError(f"artifact is not a regular non-symlink file: {path}")
    if limit is not None and first_stat.st_size > limit:
        raise EvidenceError(f"artifact exceeds its {limit}-byte limit: {path}")
    first = path.read_bytes()
    second = path.read_bytes()
    second_stat = path.lstat()
    first_identity = (
        first_stat.st_dev,
        first_stat.st_ino,
        first_stat.st_size,
        first_stat.st_mtime_ns,
    )
    second_identity = (
        second_stat.st_dev,
        second_stat.st_ino,
        second_stat.st_size,
        second_stat.st_mtime_ns,
    )
    if first != second or first_identity != second_identity:
        raise EvidenceError(f"artifact changed while it was read: {path}")
    return first


def _capture_tree(root: Path) -> list[tuple[str, str]]:
    root = root.resolve(strict=True)
    if not root.is_dir():
        raise EvidenceError(f"artifact tree is not a directory: {root}")
    files: list[tuple[str, str]] = []
    for current, directory_names, file_names in os.walk(root, followlinks=False):
        current_path = Path(current)
        directory_names.sort()
        file_names.sort()
        for name in directory_names:
            candidate = current_path / name
            if candidate.is_symlink():
                raise EvidenceError(f"artifact tree contains a symlink: {candidate}")
        for name in file_names:
            candidate = current_path / name
            relative = candidate.relative_to(root).as_posix()
            files.append((relative, sha256_bytes(read_stable_file(candidate))))
    files.sort()
    if not files:
        raise EvidenceError(f"artifact tree contains no files: {root}")
    return files


def tree_hash(root: Path) -> str:
    """Match the Rust/offline sorted path/type/content manifest algorithm."""

    first = _capture_tree(root)
    second = _capture_tree(root)
    if first != second:
        raise EvidenceError(f"artifact tree changed while it was hashed: {root}")
    digest = hashlib.sha256()
    for relative, file_digest in second:
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0file\0")
        digest.update(file_digest.encode("ascii"))
        digest.update(b"\n")
    return digest.hexdigest()


def file_record(path: Path) -> dict[str, Any]:
    value = read_stable_file(path)
    return {
        "path": str(path.resolve()),
        "byte_count": len(value),
        "sha256": sha256_bytes(value),
    }


def validate_envelope(path: Path) -> dict[str, Any]:
    try:
        envelope = json.loads(read_stable_file(path, limit=4 * 1024 * 1024))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid envelope JSON: {error}") from error
    if envelope.get("schema_version") != "1.0":
        raise EvidenceError("deterministic envelope must use schema_version 1.0")
    form = envelope.get("form")
    if not isinstance(form, dict) or form.get("code") != "2551Q" or form.get(
        "version"
    ) != "2018":
        raise EvidenceError("deterministic envelope must target exactly 2551Q:2018")
    record = file_record(path)
    record.update({"form_code": "2551Q", "form_revision": "2018"})
    return record


def validate_build_identity(path: Path, renderer_sha256: str) -> dict[str, Any]:
    try:
        identity = json.loads(read_stable_file(path, limit=64 * 1024))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid renderer build identity: {error}") from error
    if identity.get("scope") != "build_time_non_promotional_identity":
        raise EvidenceError("renderer build identity has an unexpected scope")
    if identity.get("promotion_eligible") is not False:
        raise EvidenceError("renderer build identity must remain non-promotional")
    if identity.get("offline_verification_passed") is not True:
        raise EvidenceError("renderer build identity did not pass offline verification")
    expected = identity.get("renderer_bundle_sha256")
    if not isinstance(expected, str) or not SHA256.fullmatch(expected):
        raise EvidenceError("renderer build identity has no canonical renderer hash")
    if expected != renderer_sha256:
        raise EvidenceError(
            "independently hashed renderer differs from its build-time identity"
        )
    source = identity.get("source_revision")
    if not isinstance(source, dict) or source.get("status") not in {
        "observed",
        "unavailable",
    }:
        raise EvidenceError("renderer build identity has invalid source provenance")
    if source.get("status") == "observed" and not GIT_REVISION.fullmatch(
        str(source.get("value", ""))
    ):
        raise EvidenceError("renderer source revision is not a canonical Git commit")
    return identity


def codesign_record(app: Path) -> dict[str, Any]:
    verify = subprocess.run(
        ["/usr/bin/codesign", "--verify", "--strict", "--verbose=2", str(app)],
        text=True,
        capture_output=True,
        check=False,
    )
    if verify.returncode != 0:
        raise EvidenceError(
            f"codesign verification failed: {(verify.stderr or verify.stdout).strip()}"
        )
    detail = subprocess.run(
        ["/usr/bin/codesign", "-d", "--verbose=4", str(app)],
        text=True,
        capture_output=True,
        check=False,
    )
    output = f"{detail.stdout}\n{detail.stderr}"
    authority = re.findall(r"^Authority=(.+)$", output, flags=re.MULTILINE)
    team = re.search(r"^TeamIdentifier=(.+)$", output, flags=re.MULTILINE)
    return {
        "verified": True,
        "authority": authority,
        "team_identifier": team.group(1) if team else None,
        "developer_id_signed": any(value.startswith("Developer ID") for value in authority),
        "notarization_verified": False,
    }


def app_binary(app: Path) -> Path:
    info_path = app / "Contents/Info.plist"
    try:
        info = plistlib.loads(read_stable_file(info_path, limit=1024 * 1024))
    except (plistlib.InvalidFileException, ValueError) as error:
        raise EvidenceError(f"invalid app Info.plist: {error}") from error
    executable = info.get("CFBundleExecutable")
    if not isinstance(executable, str) or not executable:
        raise EvidenceError("app Info.plist has no CFBundleExecutable")
    path = app / "Contents/MacOS" / executable
    read_stable_file(path)
    if not os.access(path, os.X_OK):
        raise EvidenceError(f"app executable is not executable: {path}")
    return path


NATIVE_EXPORT_SWIFT = r'''
import AppKit
import CoreGraphics
import Foundation

struct WindowGeometry: Codable {
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

struct ExportResult: Codable {
    let initial: WindowGeometry
    let active: WindowGeometry
    let firstClickX: Double
    let firstClickY: Double
    let secondClickX: Double
    let secondClickY: Double
}

enum DriverError: Error, CustomStringConvertible {
    case invalidEnvironment(String)
    case previewWindowUnavailable(Int32)
    case eventCreationFailed(String)

    var description: String {
        switch self {
        case .invalidEnvironment(let name):
            return "missing or invalid environment value: \(name)"
        case .previewWindowUnavailable(let pid):
            return "2551Q preview window is unavailable for exact owner PID \(pid)"
        case .eventCreationFailed(let action):
            return "failed to create CoreGraphics event for \(action)"
        }
    }
}

let environment = ProcessInfo.processInfo.environment
guard let pidText = environment["EBIR_NATIVE_EVIDENCE_PID"],
      let pid = Int32(pidText) else {
    throw DriverError.invalidEnvironment("EBIR_NATIVE_EVIDENCE_PID")
}
guard let destination = environment["EBIR_NATIVE_EVIDENCE_DESTINATION"],
      !destination.isEmpty else {
    throw DriverError.invalidEnvironment("EBIR_NATIVE_EVIDENCE_DESTINATION")
}

func exactPreviewGeometry(pid: Int32) -> WindowGeometry? {
    guard let records = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        return nil
    }
    let candidates = records.compactMap { record -> WindowGeometry? in
        guard let owner = record[kCGWindowOwnerPID as String] as? NSNumber,
              owner.int32Value == pid,
              let name = record[kCGWindowName as String] as? String,
              name.contains("2551Q HTML Form Preview"),
              let bounds = record[kCGWindowBounds as String] as? [String: Any],
              let x = bounds["X"] as? NSNumber,
              let y = bounds["Y"] as? NSNumber,
              let width = bounds["Width"] as? NSNumber,
              let height = bounds["Height"] as? NSNumber else {
            return nil
        }
        return WindowGeometry(
            x: x.doubleValue,
            y: y.doubleValue,
            width: width.doubleValue,
            height: height.doubleValue
        )
    }
    return candidates.max { left, right in
        left.width * left.height < right.width * right.height
    }
}

func waitForPreview(pid: Int32, attempts: Int = 100) throws -> WindowGeometry {
    for _ in 0..<attempts {
        if let geometry = exactPreviewGeometry(pid: pid) {
            return geometry
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    throw DriverError.previewWindowUnavailable(pid)
}

let eventSource = CGEventSource(stateID: .hidSystemState)

func postMouse(_ type: CGEventType, at point: CGPoint) throws {
    guard let event = CGEvent(
        mouseEventSource: eventSource,
        mouseType: type,
        mouseCursorPosition: point,
        mouseButton: .left
    ) else {
        throw DriverError.eventCreationFailed("mouse")
    }
    event.post(tap: .cghidEventTap)
}

func click(_ point: CGPoint) throws {
    try postMouse(.mouseMoved, at: point)
    try postMouse(.leftMouseDown, at: point)
    try postMouse(.leftMouseUp, at: point)
}

func postKey(_ code: CGKeyCode, flags: CGEventFlags = []) throws {
    guard let down = CGEvent(
        keyboardEventSource: eventSource,
        virtualKey: code,
        keyDown: true
    ), let up = CGEvent(
        keyboardEventSource: eventSource,
        virtualKey: code,
        keyDown: false
    ) else {
        throw DriverError.eventCreationFailed("keyboard")
    }
    down.flags = flags
    up.flags = flags
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}

func postText(_ value: String) throws {
    let characters = Array(value.utf16)
    guard let down = CGEvent(
        keyboardEventSource: eventSource,
        virtualKey: 0,
        keyDown: true
    ), let up = CGEvent(
        keyboardEventSource: eventSource,
        virtualKey: 0,
        keyDown: false
    ) else {
        throw DriverError.eventCreationFailed("text")
    }
    characters.withUnsafeBufferPointer { buffer in
        down.keyboardSetUnicodeString(
            stringLength: buffer.count,
            unicodeString: buffer.baseAddress!
        )
        up.keyboardSetUnicodeString(
            stringLength: buffer.count,
            unicodeString: buffer.baseAddress!
        )
    }
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}

func exportPoint(_ geometry: WindowGeometry) -> CGPoint {
    // The GPUI toolbar is custom-painted. Its reviewed export-button center is
    // stable relative to the right/top edge at both supported evidence sizes.
    CGPoint(x: geometry.x + geometry.width - 183.0, y: geometry.y + 58.0)
}

let initial = try waitForPreview(pid: pid)
let firstPoint = exportPoint(initial)

// A first click activates the exact process window. The pinned GPUI backend
// may then reflow it, so this click is deliberately not treated as the action.
try click(firstPoint)
Thread.sleep(forTimeInterval: 0.6)

// Re-query by exact owner PID after activation/reflow and click the recalculated
// Export PDF center. This avoids colliding with another same-name app process.
let active = try waitForPreview(pid: pid, attempts: 20)
let secondPoint = exportPoint(active)
try click(secondPoint)
Thread.sleep(forTimeInterval: 1.0)

// Drive the native AppKit save panel without querying the GPUI window through
// the accessibility hierarchy, which destabilizes this pinned backend.
try postKey(5, flags: [.maskCommand, .maskShift]) // Command-Shift-G
Thread.sleep(forTimeInterval: 0.4)
try postText(destination)
try postKey(36) // Go
Thread.sleep(forTimeInterval: 0.5)
try postKey(36) // Save
Thread.sleep(forTimeInterval: 0.7)
try postKey(36) // Replace, when the pre-existing destination is confirmed

let result = ExportResult(
    initial: initial,
    active: active,
    firstClickX: firstPoint.x,
    firstClickY: firstPoint.y,
    secondClickX: secondPoint.x,
    secondClickY: secondPoint.y
)
let encoded = try JSONEncoder().encode(result)
FileHandle.standardOutput.write(encoded)
FileHandle.standardOutput.write(Data("\n".utf8))
'''


def run_native_export(pid: int, destination: Path, *, timeout: float) -> dict[str, Any]:
    environment = os.environ.copy()
    environment["EBIR_NATIVE_EVIDENCE_PID"] = str(pid)
    environment["EBIR_NATIVE_EVIDENCE_DESTINATION"] = str(destination)
    result = subprocess.run(
        ["/usr/bin/xcrun", "swift", "-e", NATIVE_EXPORT_SWIFT],
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
        env=environment,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise EvidenceError(f"exact-PID macOS export automation failed: {detail}")
    try:
        record = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise EvidenceError("exact-PID export automation returned invalid JSON") from error
    if not isinstance(record, dict) or not isinstance(record.get("active"), dict):
        raise EvidenceError("exact-PID export automation omitted active window geometry")
    return record


NATIVE_PRINT_CANCEL_SWIFT = r'''
import AppKit
import CoreGraphics
import Foundation

struct WindowGeometry: Codable {
    let id: UInt32
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}

struct PrintResult: Codable {
    let initial: WindowGeometry
    let active: WindowGeometry
    let clickX: Double
    let clickY: Double
    let dialog: WindowGeometry
    let dialogObserved: Bool
    let dialogCancelled: Bool
}

enum DriverError: Error, CustomStringConvertible {
    case invalidEnvironment(String)
    case previewWindowUnavailable(Int32)
    case previewWindowOffscreen(Int32)
    case printDialogUnavailable(Int32)
    case printDialogDidNotCancel(UInt32)
    case eventCreationFailed(String)

    var description: String {
        switch self {
        case .invalidEnvironment(let name):
            return "missing or invalid environment value: \(name)"
        case .previewWindowUnavailable(let pid):
            return "2551Q preview window is unavailable for exact owner PID \(pid)"
        case .previewWindowOffscreen(let pid):
            return "2551Q preview window is not materially visible on an active display for exact owner PID \(pid)"
        case .printDialogUnavailable(let pid):
            return "native print dialog did not appear for exact owner PID \(pid)"
        case .printDialogDidNotCancel(let id):
            return "native print dialog window \(id) did not close after Escape"
        case .eventCreationFailed(let action):
            return "failed to create CoreGraphics event for \(action)"
        }
    }
}

let environment = ProcessInfo.processInfo.environment
guard let pidText = environment["EBIR_NATIVE_EVIDENCE_PID"],
      let pid = Int32(pidText) else {
    throw DriverError.invalidEnvironment("EBIR_NATIVE_EVIDENCE_PID")
}

func processWindows(pid: Int32) -> [WindowGeometry] {
    guard let records = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        return []
    }
    return records.compactMap { record in
        guard let owner = record[kCGWindowOwnerPID as String] as? NSNumber,
              owner.int32Value == pid,
              let number = record[kCGWindowNumber as String] as? NSNumber,
              let bounds = record[kCGWindowBounds as String] as? [String: Any],
              let x = bounds["X"] as? NSNumber,
              let y = bounds["Y"] as? NSNumber,
              let width = bounds["Width"] as? NSNumber,
              let height = bounds["Height"] as? NSNumber else {
            return nil
        }
        return WindowGeometry(
            id: number.uint32Value,
            x: x.doubleValue,
            y: y.doubleValue,
            width: width.doubleValue,
            height: height.doubleValue
        )
    }
}

func exactPreview(pid: Int32) -> WindowGeometry? {
    guard let records = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        return nil
    }
    return records.compactMap { record -> WindowGeometry? in
        guard let owner = record[kCGWindowOwnerPID as String] as? NSNumber,
              owner.int32Value == pid,
              let name = record[kCGWindowName as String] as? String,
              name.contains("2551Q HTML Form Preview"),
              let number = record[kCGWindowNumber as String] as? NSNumber,
              let bounds = record[kCGWindowBounds as String] as? [String: Any],
              let x = bounds["X"] as? NSNumber,
              let y = bounds["Y"] as? NSNumber,
              let width = bounds["Width"] as? NSNumber,
              let height = bounds["Height"] as? NSNumber else {
            return nil
        }
        return WindowGeometry(
            id: number.uint32Value,
            x: x.doubleValue,
            y: y.doubleValue,
            width: width.doubleValue,
            height: height.doubleValue
        )
    }.max { left, right in
        left.width * left.height < right.width * right.height
    }
}

func waitForPreview(pid: Int32, attempts: Int = 100) throws -> WindowGeometry {
    for _ in 0..<attempts {
        if let geometry = exactPreview(pid: pid) {
            return geometry
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    throw DriverError.previewWindowUnavailable(pid)
}

func activeDisplayBounds() -> [CGRect] {
    var count: UInt32 = 0
    guard CGGetActiveDisplayList(0, nil, &count) == .success else { return [] }
    var displays = Array(repeating: CGDirectDisplayID(), count: Int(count))
    guard CGGetActiveDisplayList(count, &displays, &count) == .success else { return [] }
    return displays.prefix(Int(count)).map(CGDisplayBounds)
}

func materiallyOnscreen(_ geometry: WindowGeometry) -> Bool {
    let window = CGRect(x: geometry.x, y: geometry.y, width: geometry.width, height: geometry.height)
    let visibleArea = activeDisplayBounds().reduce(0.0) { total, display in
        let intersection = window.intersection(display)
        return total + (intersection.isNull ? 0.0 : intersection.width * intersection.height)
    }
    return visibleArea >= geometry.width * geometry.height * 0.9
}

let eventSource = CGEventSource(stateID: .hidSystemState)

func click(_ point: CGPoint) throws {
    for type in [CGEventType.mouseMoved, .leftMouseDown, .leftMouseUp] {
        guard let event = CGEvent(
            mouseEventSource: eventSource,
            mouseType: type,
            mouseCursorPosition: point,
            mouseButton: .left
        ) else {
            throw DriverError.eventCreationFailed("mouse")
        }
        event.post(tap: .cghidEventTap)
    }
}

func postEscape() throws {
    guard let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 53, keyDown: true),
          let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 53, keyDown: false) else {
        throw DriverError.eventCreationFailed("Escape")
    }
    down.post(tap: .cghidEventTap)
    up.post(tap: .cghidEventTap)
}

func waitForNewDialog(pid: Int32, baseline: Set<UInt32>) throws -> WindowGeometry {
    for _ in 0..<100 {
        if let dialog = processWindows(pid: pid)
            .filter({ !baseline.contains($0.id) && $0.width >= 300 && $0.height >= 200 })
            .max(by: { $0.width * $0.height < $1.width * $1.height }) {
            return dialog
        }
        Thread.sleep(forTimeInterval: 0.1)
    }
    throw DriverError.printDialogUnavailable(pid)
}

let initial = try waitForPreview(pid: pid)
guard materiallyOnscreen(initial) else { throw DriverError.previewWindowOffscreen(pid) }
let baseline = Set(processWindows(pid: pid).map(\.id))

guard let app = NSRunningApplication(processIdentifier: pid_t(pid)) else {
    throw DriverError.previewWindowUnavailable(pid)
}
app.activate(options: [.activateAllWindows, .activateIgnoringOtherApps])
Thread.sleep(forTimeInterval: 0.6)

let active = try waitForPreview(pid: pid, attempts: 20)
guard materiallyOnscreen(active) else { throw DriverError.previewWindowOffscreen(pid) }
// Reviewed GPUI toolbar order is Export PDF, Print, Refresh. The Print center
// is stable at 84 points from the right edge and 58 points from the top edge.
let printPoint = CGPoint(x: active.x + active.width - 84.0, y: active.y + 58.0)
try click(printPoint)

let dialog = try waitForNewDialog(pid: pid, baseline: baseline)
// The newly opened native sheet is already the exact process's key dialog.
// Never click inside it: a coordinate mistake could activate Print. Escape is
// the only event sent after the dialog is observed.
try postEscape()

for _ in 0..<50 {
    if !processWindows(pid: pid).contains(where: { $0.id == dialog.id }) {
        let result = PrintResult(
            initial: initial,
            active: active,
            clickX: printPoint.x,
            clickY: printPoint.y,
            dialog: dialog,
            dialogObserved: true,
            dialogCancelled: true
        )
        let encoded = try JSONEncoder().encode(result)
        FileHandle.standardOutput.write(encoded)
        FileHandle.standardOutput.write(Data("\n".utf8))
        exit(0)
    }
    Thread.sleep(forTimeInterval: 0.1)
}
throw DriverError.printDialogDidNotCancel(dialog.id)
'''


def run_native_print_cancel(pid: int, *, timeout: float) -> dict[str, Any]:
    environment = os.environ.copy()
    environment["EBIR_NATIVE_EVIDENCE_PID"] = str(pid)
    result = subprocess.run(
        ["/usr/bin/xcrun", "swift", "-e", NATIVE_PRINT_CANCEL_SWIFT],
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
        env=environment,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise EvidenceError(f"exact-PID macOS system-print automation failed: {detail}")
    try:
        record = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise EvidenceError("exact-PID system-print automation returned invalid JSON") from error
    if (
        not isinstance(record, dict)
        or record.get("dialogObserved") is not True
        or record.get("dialogCancelled") is not True
    ):
        raise EvidenceError("exact-PID system-print automation omitted dialog evidence")
    return record


def find_child_pid(binary: Path, launcher_pid: int, timeout: float) -> int:
    deadline = time.monotonic() + timeout
    needle = str(binary.resolve())
    while time.monotonic() < deadline:
        result = subprocess.run(
            ["/bin/ps", "-axo", "pid=,command="],
            text=True,
            capture_output=True,
            check=True,
        )
        for line in result.stdout.splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            pid_text, _, command = stripped.partition(" ")
            if needle in command and DEV_FLAG in command:
                return int(pid_text)
        try:
            os.kill(launcher_pid, 0)
        except OSError as error:
            raise EvidenceError("packaged app exited before its evidence window opened") from error
        time.sleep(0.1)
    raise EvidenceError("timed out locating the packaged app process")


DEV_FLAG = "--dev-native-evidence-envelope"


def wait_for_observation(directory: Path, previous: set[Path], timeout: float) -> Path:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        current = set(directory.glob("*.observation.json"))
        created = sorted(current - previous)
        if len(created) == 1:
            return created[0]
        if len(created) > 1:
            raise EvidenceError("one PDF export produced multiple observations")
        time.sleep(0.1)
    raise EvidenceError("timed out waiting for the app-written PDF observation")


def wait_for_failure_observation(
    directory: Path, previous: set[Path], timeout: float
) -> Path:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        current = set(directory.glob("*.failure.json"))
        created = sorted(current - previous)
        if len(created) == 1:
            return created[0]
        if len(created) > 1:
            raise EvidenceError("one failed PDF export produced multiple failure observations")
        time.sleep(0.1)
    raise EvidenceError("timed out waiting for the app-written PDF failure observation")


def validate_failure_observation(
    path: Path, *, destination: Path, destination_before: dict[str, Any]
) -> dict[str, Any]:
    try:
        observation = json.loads(read_stable_file(path, limit=1024 * 1024))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid app-written failure observation: {error}") from error
    if observation.get("schema_version") != 1:
        raise EvidenceError("app-written failure observation has an unsupported schema")
    if observation.get("scope") != "development_diagnostic":
        raise EvidenceError("app-written failure observation has an unexpected scope")
    if observation.get("promotion_eligible") is not False:
        raise EvidenceError("app-written failure observation must remain non-promotional")
    if observation.get("outcome") != "export_failed":
        raise EvidenceError("app-written failure observation did not record export_failed")
    if (observation.get("form_code"), observation.get("form_revision")) != (
        "2551Q",
        "2018",
    ):
        raise EvidenceError("app-written failure observation does not describe 2551Q:2018")
    if Path(observation.get("destination", "")).resolve() != destination.resolve():
        raise EvidenceError("app-written failure observation names the wrong destination")
    expected_snapshot = {"state": "file", "sha256": destination_before["sha256"]}
    if observation.get("destination_before") != expected_snapshot:
        raise EvidenceError("failed export did not bind the pre-existing destination")
    if observation.get("destination_after") != expected_snapshot:
        raise EvidenceError("failed export did not preserve the destination snapshot")
    if observation.get("temporary_file_remaining") is not False:
        raise EvidenceError("failed export retained a temporary file at observation time")
    if not str(observation.get("error", "")).strip():
        raise EvidenceError("failed export observation omitted its failure reason")
    return observation


def temporary_pdf_paths(directory: Path) -> list[Path]:
    return sorted(
        path for path in directory.iterdir() if path.is_file() and PARTIAL_PDF.match(path.name)
    )


def validate_observation(
    path: Path,
    *,
    source_envelope: Path,
    package_sha256: str,
    renderer_sha256: str,
    final_pdf: Path,
    artifact_dir: Path,
) -> tuple[dict[str, Any], list[dict[str, Any]], dict[str, Any]]:
    try:
        observation = json.loads(read_stable_file(path, limit=4 * 1024 * 1024))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid app-written observation: {error}") from error
    if observation.get("scope") != "development_diagnostic":
        raise EvidenceError("app-written observation has an unexpected scope")
    if observation.get("promotion_eligible") is not False:
        raise EvidenceError("app-written observation must remain non-promotional")
    if (observation.get("form_code"), observation.get("form_revision")) != (
        "2551Q",
        "2018",
    ):
        raise EvidenceError("app-written observation does not describe 2551Q:2018")
    stem = path.name.removesuffix(".observation.json")
    runtime_envelope = artifact_dir / f"{stem}.envelope.json"
    source_value = json.loads(read_stable_file(source_envelope, limit=4 * 1024 * 1024))
    runtime_value = json.loads(read_stable_file(runtime_envelope, limit=4 * 1024 * 1024))
    if source_value != runtime_value:
        raise EvidenceError("runtime envelope differs semantically from the supplied fixture")
    runtime_envelope_record = file_record(runtime_envelope)
    if observation.get("envelope_sha256") != runtime_envelope_record["sha256"]:
        raise EvidenceError("app-written observation is not bound to its immutable envelope bytes")
    observed_package = observation.get("package_sha256", {})
    if observed_package.get("status") != "observed" or observed_package.get(
        "value"
    ) != package_sha256:
        raise EvidenceError("app-written observation is not bound to the unchanged app package")
    observed_renderer = observation.get("renderer_bundle_sha256", {})
    if observed_renderer.get("status") != "observed" or observed_renderer.get(
        "value"
    ) != renderer_sha256:
        raise EvidenceError("app-written observation is not bound to the bundled renderer")
    final = file_record(final_pdf)
    output_hash = observation.get("output_pdf_sha256", {})
    if output_hash.get("status") != "observed" or output_hash.get("value") != final[
        "sha256"
    ]:
        raise EvidenceError("final PDF differs from the app-written output hash")
    if observation.get("pdf_validation", {}).get("status") != "observed":
        raise EvidenceError("app-written observation has no successful PDF validation")
    if not observation.get("strict_verifier_gaps"):
        raise EvidenceError("app-written observation erased its strict verifier gaps")

    final_snapshot = artifact_dir / f"{stem}.final.pdf"
    if file_record(final_snapshot)["sha256"] != final["sha256"]:
        raise EvidenceError("retained final PDF snapshot differs from the export destination")
    page_evidence = observation.get("native_page_payloads", {})
    if page_evidence.get("status") != "observed":
        raise EvidenceError("WKPDF callback payload evidence is unavailable on macOS")
    pages: list[dict[str, Any]] = []
    for expected in page_evidence.get("value", []):
        page_number = expected.get("page_number")
        page_path = artifact_dir / f"{stem}.wkpdf-page-{page_number}.pdf"
        record = file_record(page_path)
        if not expected.get("succeeded"):
            raise EvidenceError(f"WKPDF page {page_number} did not succeed")
        if expected.get("sha256") != record["sha256"] or expected.get(
            "byte_count"
        ) != record["byte_count"]:
            raise EvidenceError(f"retained WKPDF page {page_number} differs from callback evidence")
        pages.append(record | {"page_number": page_number})
    if not pages:
        raise EvidenceError("no separate WKPDF page artifacts were retained")
    return observation, pages, runtime_envelope_record


def terminate_process(process: subprocess.Popen[str], app_pid: int | None) -> None:
    if app_pid is not None:
        try:
            os.kill(app_pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        process.terminate()
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def write_json_atomic(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.partial")
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    with temporary.open("xb") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def run_driver(arguments: argparse.Namespace) -> int:
    if platform.system() != "Darwin":
        raise EvidenceError("the macOS native evidence driver must run on macOS")
    app = arguments.app.resolve(strict=True)
    if app.suffix != ".app" or not app.is_dir():
        raise EvidenceError("--app must identify a prepared macOS .app directory")
    envelope = arguments.envelope.resolve(strict=True)
    output = arguments.output_dir.resolve()
    if output.exists() and any(output.iterdir()):
        raise EvidenceError("evidence output directory must be absent or empty")
    output.mkdir(parents=True, exist_ok=True)
    observation_dir = output / "app-observations"
    observation_dir.mkdir()

    renderer = app / RENDERER_RELATIVE_PATH
    identity_path = app / IDENTITY_RELATIVE_PATH
    package_before = tree_hash(app)
    renderer_before = tree_hash(renderer)
    identity = validate_build_identity(identity_path, renderer_before)
    envelope_record = validate_envelope(envelope)
    signature = codesign_record(app)
    binary = app_binary(app)

    success_destination = output / "successful-export.pdf"
    previous_destination = b"pre-existing successful destination\n"
    success_destination.write_bytes(previous_destination)
    success_before = file_record(success_destination)

    failure_dir = output / "induced-failure-read-only"
    failure_dir.mkdir()
    failure_destination = failure_dir / "preserved.pdf"
    failure_destination.write_bytes(b"pre-existing destination that must survive failure\n")
    failure_before = file_record(failure_destination)
    failure_dir.chmod(0o500)

    environment = os.environ.copy()
    environment["EBIR_NATIVE_OUTPUT_EVIDENCE_DIR"] = str(observation_dir)
    environment["EBIR_NATIVE_EVIDENCE_AUTO_EXPORT_DESTINATION"] = str(
        success_destination
    )
    environment["EBIR_NATIVE_EVIDENCE_AUTO_FAILURE_DESTINATION"] = str(
        failure_destination
    )
    environment["DEVELOPER_MODE"] = "true"
    command = [str(binary), DEV_FLAG, str(envelope)]
    network = {
        "requested": bool(arguments.network_denied),
        "mechanism": None,
        "enforced_for_launch": False,
    }
    if arguments.network_denied:
        sandbox = Path("/usr/bin/sandbox-exec")
        if not sandbox.is_file():
            raise EvidenceError("network-denied run requested but sandbox-exec is unavailable")
        command = [
            str(sandbox),
            "-p",
            "(version 1) (allow default) (deny network*)",
            *command,
        ]
        network.update(
            {
                "mechanism": "sandbox-exec deny network*",
                "enforced_for_launch": True,
            }
        )

    process: subprocess.Popen[str] | None = None
    app_pid: int | None = None
    observation_path: Path | None = None
    failure_observation_path: Path | None = None
    page_artifacts: list[dict[str, Any]] = []
    runtime_envelope: dict[str, Any] | None = None
    observation: dict[str, Any] | None = None
    system_print = {
        "requested": bool(arguments.exercise_system_print),
        "automation": "not_requested",
        "passed": False,
    }
    try:
        process = subprocess.Popen(
            command,
            env=environment,
            text=True,
            stdout=(output / "app.stdout.log").open("w", encoding="utf-8"),
            stderr=(output / "app.stderr.log").open("w", encoding="utf-8"),
        )
        app_pid = find_child_pid(binary, process.pid, arguments.timeout)
        existing_observations = set(observation_dir.glob("*.observation.json"))
        existing_failure_observations = set(observation_dir.glob("*.failure.json"))
        observation_path = wait_for_observation(
            observation_dir, existing_observations, arguments.timeout
        )
        observation, page_artifacts, runtime_envelope = validate_observation(
            observation_path,
            source_envelope=envelope,
            package_sha256=package_before,
            renderer_sha256=renderer_before,
            final_pdf=success_destination,
            artifact_dir=observation_dir,
        )
        failure_observation_path = wait_for_failure_observation(
            observation_dir, existing_failure_observations, arguments.timeout
        )
        validate_failure_observation(
            failure_observation_path,
            destination=failure_destination,
            destination_before=failure_before,
        )
        if len(list(observation_dir.glob("*.observation.json"))) != 1:
            raise EvidenceError("induced failed export unexpectedly emitted a success observation")

        if arguments.exercise_system_print:
            print_record = run_native_print_cancel(app_pid, timeout=arguments.timeout)
            system_print = {
                "requested": True,
                "automation": "exact-PID system-print path requested; native dialog observed and cancelled",
                "dialog_observed": print_record["dialogObserved"],
                "dialog_cancelled": print_record["dialogCancelled"],
                "driver_record": print_record,
                "passed": False,
            }
    finally:
        failure_dir.chmod(0o700)
        if process is not None:
            terminate_process(process, app_pid)

    package_after = tree_hash(app)
    renderer_after = tree_hash(renderer)
    if package_after != package_before or renderer_after != renderer_before:
        raise EvidenceError("application package or renderer changed during the exercise")
    success_after = file_record(success_destination)
    failure_after = file_record(failure_destination)
    failure_temps = temporary_pdf_paths(failure_dir)
    if failure_before["sha256"] != failure_after["sha256"] or failure_temps:
        raise EvidenceError("induced failure changed its destination or leaked a temp file")

    strict_gaps = [
        "driver and app package are diagnostic and not independently attested",
        "ad-hoc or Developer ID signature detail is recorded but notarization is not independently verified",
        "the development-only destination queue exercises the toolbar output state machine but not toolbar activation or the native save chooser",
        "system-print completion requires an operator and configured printer",
        "rollback evidence and Windows/Linux platform evidence are outside this transcript",
    ]
    if not signature["developer_id_signed"]:
        strict_gaps.append("app package is not Developer ID signed")
    if not arguments.network_denied:
        strict_gaps.append("packaged runtime was not launched under a network-denial profile")
    if not arguments.exercise_system_print:
        strict_gaps.append("system-print path was not requested")

    transcript = {
        "schema_version": SCHEMA_VERSION,
        "scope": SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "form": {"code": "2551Q", "revision": "2018"},
        "package": {
            "path": str(app),
            "sha256_before": package_before,
            "sha256_after": package_after,
            "unchanged": True,
            "signature": signature,
        },
        "renderer": {
            "path": str(renderer),
            "sha256_before": renderer_before,
            "sha256_after": renderer_after,
            "unchanged": True,
            "expected_sha256": identity["renderer_bundle_sha256"],
        },
        "envelope": envelope_record | {"runtime_canonical": runtime_envelope},
        "launch": {
            "argv": command,
            "network_denial": network,
            "app_pid": app_pid,
        },
        "pdf_export": {
            "destination_before": success_before,
            "destination_after": success_after,
            "app_observation": file_record(observation_path),
            "native_page_artifacts": page_artifacts,
            "final_pdf_separate_from_native_pages": True,
            "app_observation_promotion_eligible": observation["promotion_eligible"],
        },
        "induced_failure": {
            "mechanism": "read-only sibling directory prevents temp creation",
            "app_observation": file_record(failure_observation_path),
            "destination_before": failure_before,
            "destination_after": failure_after,
            "destination_preserved": True,
            "temporary_files_remaining": len(failure_temps),
        },
        "system_print": system_print,
        "strict_verifier_gaps": strict_gaps,
    }
    transcript_path = output / "macos-native-evidence-driver.transcript.json"
    write_json_atomic(transcript_path, transcript)
    verify_transcript(transcript_path)
    print(transcript_path)
    return 0


def _require_digest(field: str, value: Any) -> str:
    if not isinstance(value, str) or not SHA256.fullmatch(value):
        raise EvidenceError(f"{field} is not a canonical SHA-256 digest")
    return value


def verify_file_record(record: dict[str, Any], field: str) -> None:
    path = Path(record.get("path", ""))
    actual = file_record(path)
    expected = _require_digest(f"{field}.sha256", record.get("sha256"))
    if actual["sha256"] != expected or actual["byte_count"] != record.get("byte_count"):
        raise EvidenceError(f"{field} changed after transcript creation")


def verify_transcript(path: Path) -> None:
    try:
        transcript = json.loads(read_stable_file(path, limit=16 * 1024 * 1024))
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise EvidenceError(f"invalid driver transcript JSON: {error}") from error
    if transcript.get("schema_version") != SCHEMA_VERSION or transcript.get("scope") != SCOPE:
        raise EvidenceError("driver transcript has an unsupported schema or scope")
    if transcript.get("promotion_eligible") is not False:
        raise EvidenceError("driver transcript must never be promotion eligible")
    if transcript.get("trusted_producer") is not False:
        raise EvidenceError("driver transcript must never register itself as trusted")
    if transcript.get("form") != {"code": "2551Q", "revision": "2018"}:
        raise EvidenceError("driver transcript must target exactly 2551Q:2018")
    if not transcript.get("strict_verifier_gaps"):
        raise EvidenceError("driver transcript must preserve explicit promotion blockers")

    package = transcript.get("package", {})
    renderer = transcript.get("renderer", {})
    package_hash = tree_hash(Path(package.get("path", "")))
    renderer_hash = tree_hash(Path(renderer.get("path", "")))
    for key in ("sha256_before", "sha256_after"):
        if _require_digest(f"package.{key}", package.get(key)) != package_hash:
            raise EvidenceError("driver transcript package hash no longer matches")
        if _require_digest(f"renderer.{key}", renderer.get(key)) != renderer_hash:
            raise EvidenceError("driver transcript renderer hash no longer matches")
    if package.get("unchanged") is not True or renderer.get("unchanged") is not True:
        raise EvidenceError("driver transcript does not prove an unchanged package")
    if renderer.get("expected_sha256") != renderer_hash:
        raise EvidenceError("driver transcript renderer differs from build identity")

    envelope = transcript.get("envelope", {})
    source_envelope = Path(envelope.get("path", ""))
    validate_envelope(source_envelope)
    runtime_envelope = envelope.get("runtime_canonical", {})
    verify_file_record(runtime_envelope, "runtime envelope")
    if json.loads(read_stable_file(source_envelope)) != json.loads(
        read_stable_file(Path(runtime_envelope.get("path", "")))
    ):
        raise EvidenceError("supplied and runtime envelopes are no longer semantically equal")
    verify_file_record(transcript["pdf_export"]["destination_after"], "final PDF")
    verify_file_record(transcript["pdf_export"]["app_observation"], "app observation")
    for index, record in enumerate(transcript["pdf_export"]["native_page_artifacts"], 1):
        verify_file_record(record, f"WKPDF page {index}")
    failure = transcript.get("induced_failure", {})
    verify_file_record(failure["app_observation"], "failed export observation")
    if failure.get("destination_preserved") is not True or failure.get(
        "temporary_files_remaining"
    ) != 0:
        raise EvidenceError("driver transcript did not prove failure preservation")
    before = failure.get("destination_before", {})
    after = failure.get("destination_after", {})
    if before.get("sha256") != after.get("sha256"):
        raise EvidenceError("failed export destination hashes differ")
    verify_file_record(after, "failed export destination")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    run = subcommands.add_parser("run", help="run the external macOS diagnostic exercise")
    run.add_argument("--app", required=True, type=Path)
    run.add_argument("--envelope", required=True, type=Path)
    run.add_argument("--output-dir", required=True, type=Path)
    run.add_argument("--network-denied", action="store_true")
    run.add_argument("--exercise-system-print", action="store_true")
    run.add_argument("--timeout", type=float, default=60.0)
    verify = subcommands.add_parser("verify", help="re-verify an existing transcript")
    verify.add_argument("transcript", type=Path)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    arguments = build_parser().parse_args(argv)
    try:
        if arguments.command == "run":
            return run_driver(arguments)
        verify_transcript(arguments.transcript.resolve(strict=True))
        print(arguments.transcript)
        return 0
    except (EvidenceError, OSError, subprocess.SubprocessError) as error:
        print(f"macOS native evidence driver failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
