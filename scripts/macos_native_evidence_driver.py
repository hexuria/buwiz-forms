#!/usr/bin/env python3
"""External, non-promotional macOS HTML-output evidence driver.

The driver never builds or modifies the application package. It independently
hashes a prepared ``.app`` and its bundled renderer twice, launches the
development-only deterministic 2551Q preview, drives the existing Export PDF
button through macOS Accessibility, and writes a reviewable transcript outside
the package. A second invocation with ``verify`` recomputes every available
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


def run_osascript(source: str, *arguments: str, timeout: float = 20.0) -> str:
    result = subprocess.run(
        ["/usr/bin/osascript", "-e", source, *arguments],
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        raise EvidenceError(
            "macOS Accessibility automation failed. Grant Accessibility permission "
            f"to the invoking terminal/Codex app. Detail: {detail}"
        )
    return result.stdout.strip()


WAIT_READY_SCRIPT = r'''
on run argv
    set targetPid to (item 1 of argv) as integer
    tell application "System Events"
        tell first process whose unix id is targetPid
            set frontmost to true
            repeat 100 times
                try
                    set targetWindow to first window whose name contains "2551Q HTML Form Preview"
                    set exportButton to first button of targetWindow whose title is "Export PDF"
                    if enabled of exportButton then return "ready"
                end try
                delay 0.1
            end repeat
        end tell
    end tell
    error "2551Q preview did not become ready"
end run
'''


EXPORT_SCRIPT = r'''
on run argv
    set targetPid to (item 1 of argv) as integer
    set targetPath to item 2 of argv
    tell application "System Events"
        tell first process whose unix id is targetPid
            set frontmost to true
            set targetWindow to first window whose name contains "2551Q HTML Form Preview"
            click first button of targetWindow whose title is "Export PDF"
            delay 0.5
            keystroke "g" using {command down, shift down}
            delay 0.3
            keystroke targetPath
            key code 36
            delay 0.5
            key code 36
            delay 0.5
            try
                click first button whose title is "Replace" of sheet 1 of targetWindow
            end try
            try
                click first button whose title is "Replace" of window 1
            end try
        end tell
    end tell
    return "requested"
end run
'''


PRINT_CANCEL_SCRIPT = r'''
on run argv
    set targetPid to (item 1 of argv) as integer
    tell application "System Events"
        tell first process whose unix id is targetPid
            set frontmost to true
            set targetWindow to first window whose name contains "2551Q HTML Form Preview"
            click first button of targetWindow whose title is "Print"
            delay 1.0
            key code 53
        end tell
    end tell
    return "requested_and_cancelled"
end run
'''


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
        run_osascript(WAIT_READY_SCRIPT, str(app_pid), timeout=arguments.timeout)

        existing_observations = set(observation_dir.glob("*.observation.json"))
        run_osascript(EXPORT_SCRIPT, str(app_pid), str(success_destination))
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
        run_osascript(WAIT_READY_SCRIPT, str(app_pid), timeout=arguments.timeout)

        failure_observation_count = len(list(observation_dir.glob("*.observation.json")))
        run_osascript(EXPORT_SCRIPT, str(app_pid), str(failure_destination))
        time.sleep(2.0)
        run_osascript(WAIT_READY_SCRIPT, str(app_pid), timeout=arguments.timeout)
        if len(list(observation_dir.glob("*.observation.json"))) != failure_observation_count:
            raise EvidenceError("induced failed export unexpectedly emitted a success observation")

        if arguments.exercise_system_print:
            run_osascript(PRINT_CANCEL_SCRIPT, str(app_pid), timeout=arguments.timeout)
            system_print = {
                "requested": True,
                "automation": "existing system-print path requested; native dialog cancelled",
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
