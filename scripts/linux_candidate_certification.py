#!/usr/bin/env python3
"""Inspect and verify an exact Linux HTML certification candidate.

This foundation is intentionally non-promotional. It binds the portable Linux
candidate created by ``html-candidate-certification.yml`` to its clean source
revision and packaged offline renderer. A strict external attestation must
cover both supported application-owned Linux hosts: the X11 GPUI/Wry child
WebView under Xvfb and the Wayland GTK/WebKitGTK top-level window under Weston.

Even a successful report remains operator-only and untrusted. The workflow
candidate is not the final ``.deb`` or release tarball, so this tool never
registers release evidence and never changes a readiness capability.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid
from datetime import datetime
from pathlib import Path, PurePosixPath
from typing import Any, Iterable


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import macos_native_evidence_driver as common  # noqa: E402


SCHEMA_VERSION = 1
CANDIDATE_SCOPE = "html_certification_candidate_input"
BINDING_SCOPE = "external_linux_candidate_binding"
PROBE_SCOPE = "external_linux_candidate_nondev_probe"
ATTESTATION_SCOPE = "external_linux_candidate_certification_attestation"
REPORT_SCOPE = "external_linux_candidate_certification_verification"
FORM = {"code": "2551Q", "revision": "2018"}
EXPECTED_PAGE_COUNT = 2
EXPECTED_WIDTH_POINTS = 612.0
EXPECTED_HEIGHT_POINTS = 936.0
EXPECTED_ARCHITECTURE = "x86_64"
RENDERER_RELATIVE_PATH = Path("assets/form-renderer")
IDENTITY_RELATIVE_PATH = Path("assets/form-renderer-build-identity.json")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_REVISION = re.compile(r"[0-9a-f]{40}\Z")
RFC3339_UTC = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z\Z")
NETWORK_NAMESPACE = re.compile(r"net:\[[0-9]+\]\Z")
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
BACKENDS = {
    "x11": {
        "host_strategy": "GpuiWryChild",
        "display_variable": "DISPLAY",
        "compositor": "Xvfb",
        "window_title": "2551Q HTML Form Preview",
    },
    "wayland": {
        "host_strategy": "GtkTopLevel",
        "display_variable": "WAYLAND_DISPLAY",
        "compositor": "Weston",
        "window_title": "HTML Form Preview",
    },
}
MAX_ARCHIVE_FILES = 20_000
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
NON_PROMOTIONAL_GAP = "collector producer is not registered as trusted"
RELEASE_PACKAGE_GAP = (
    "portable candidate bytes are not the final deb and release tarball bytes"
)


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


def require_false(value: Any, label: str) -> None:
    if value is not False:
        raise EvidenceError(f"{label} must remain false")


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
    require_exact_keys(
        manifest,
        {
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
        },
        "candidate manifest",
    )
    if manifest["schema_version"] != 1 or manifest["scope"] != CANDIDATE_SCOPE:
        raise EvidenceError("candidate manifest has an unsupported schema or scope")
    require_false(manifest["promotion_eligible"], "candidate promotion eligibility")
    require_false(manifest["trusted_producer"], "candidate trusted producer")
    if manifest["form"] != FORM:
        raise EvidenceError("candidate manifest must target exactly 2551Q:2018")
    source_revision = manifest["source_revision"]
    if not isinstance(source_revision, str) or GIT_REVISION.fullmatch(source_revision) is None:
        raise EvidenceError("candidate source revision is not canonical")
    if manifest["platform"] != "linux" or manifest["architecture"] != EXPECTED_ARCHITECTURE:
        raise EvidenceError("candidate must be the Linux x86-64 workflow artifact")

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
    if identity.get("source_revision") != {"status": "observed", "value": source_revision}:
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
    with tarfile.open(archive, mode="r:gz") as bundle:
        members = bundle.getmembers()
        if not members or len(members) > MAX_ARCHIVE_FILES:
            raise EvidenceError("candidate archive has an invalid member count")
        for info in members:
            member = _safe_member_path(info.name)
            normalized = member.as_posix().rstrip("/")
            if not normalized:
                continue
            if normalized in seen or normalized.casefold() in seen_casefold:
                raise EvidenceError(f"candidate archive contains duplicate paths: {normalized}")
            seen.add(normalized)
            seen_casefold.add(normalized.casefold())
            if info.issym() or info.islnk() or info.isdev() or info.isfifo():
                raise EvidenceError(f"candidate archive contains an unsafe member: {normalized}")
            if not (info.isfile() or info.isdir()):
                raise EvidenceError(f"candidate archive contains an unsupported member: {normalized}")
            total_bytes += info.size
            if total_bytes > MAX_ARCHIVE_BYTES:
                raise EvidenceError("candidate archive exceeds its extraction size limit")
            target = destination.joinpath(*member.parts)
            if info.isdir():
                target.mkdir(parents=True, exist_ok=True)
                target.chmod(info.mode & 0o777 or 0o755)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            source = bundle.extractfile(info)
            if source is None:
                raise EvidenceError(f"candidate archive member cannot be read: {normalized}")
            with source, target.open("xb") as output:
                remaining = info.size
                while remaining:
                    chunk = source.read(min(1024 * 1024, remaining))
                    if not chunk:
                        raise EvidenceError(f"candidate archive member is truncated: {normalized}")
                    output.write(chunk)
                    remaining -= len(chunk)
                if source.read(1):
                    raise EvidenceError(f"candidate archive member exceeded its declared size: {normalized}")
                output.flush()
                os.fsync(output.fileno())
            target.chmod(info.mode & 0o777 or 0o644)

    top_level = list(destination.iterdir())
    roots = [path for path in top_level if path.is_dir()]
    if (
        len(top_level) != 1
        or len(roots) != 1
        or roots[0].name != "eBIRForms-Linux-x64"
    ):
        raise EvidenceError("candidate archive must contain exactly eBIRForms-Linux-x64")
    return roots[0]


def bind_installed_candidate(
    candidate: dict[str, Any], installed_root: Path, external_identity_path: Path
) -> dict[str, Any]:
    installed_root = installed_root.resolve(strict=True)
    binary = regular_file(installed_root / "bir", "installed Linux binary")
    if not os.access(binary, os.X_OK):
        raise EvidenceError("installed Linux candidate binary is not executable")
    renderer = installed_root / RENDERER_RELATIVE_PATH
    bundled_identity_path = installed_root / IDENTITY_RELATIVE_PATH
    renderer_hash = common.tree_hash(renderer)
    if renderer_hash != candidate["renderer_bundle_sha256"]:
        raise EvidenceError("packaged renderer differs from the candidate manifest")
    bundled_identity_record = common.file_record(bundled_identity_path)
    external_identity_record = common.file_record(external_identity_path)
    if bundled_identity_record["sha256"] != external_identity_record["sha256"]:
        raise EvidenceError("packaged renderer identity differs from the uploaded identity")
    common.validate_build_identity(bundled_identity_path, renderer_hash)
    return {
        "installation_method": "secure_portable_tar_extraction",
        "installed_root": str(installed_root),
        "installed_root_sha256": common.tree_hash(installed_root),
        "binary": common.file_record(binary),
        "assets_tree_sha256": common.tree_hash(installed_root / "assets"),
        "renderer_path": str(renderer.resolve()),
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
    root = extract_candidate_archive(archive_path, output_dir / "installed-candidate")
    installed = bind_installed_candidate(candidate, root, identity_path)
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
        "installed_candidate": installed,
        "package_boundary": {
            "portable_candidate_verified": True,
            "final_release_deb_verified": False,
            "final_release_tarball_verified": False,
            "release_package_signature_verified": False,
        },
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            RELEASE_PACKAGE_GAP,
            "X11/Xvfb and Wayland/Weston preview, export, print, and rollback were not exercised",
        ],
    }
    common.write_json_atomic(output_dir / "linux-candidate-binding.json", binding)
    return binding


def _terminate(process: subprocess.Popen[str]) -> None:
    try:
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def probe_nondev_candidate(
    binding: dict[str, Any], output_dir: Path, timeout: float, backend: str
) -> dict[str, Any]:
    if platform.system() != "Linux":
        raise EvidenceError("the non-development candidate probe must run on Linux")
    if backend not in BACKENDS:
        raise EvidenceError("Linux candidate probe backend must be x11 or wayland")
    bubblewrap = shutil.which("bwrap")
    if bubblewrap is None:
        raise EvidenceError("bubblewrap is required for a network-denied Linux probe")
    display_variable = BACKENDS[backend]["display_variable"]
    if not os.environ.get(display_variable):
        raise EvidenceError(f"{display_variable} is required for the {backend} probe")
    binary = Path(binding["installed_candidate"]["binary"]["path"])
    root = Path(binding["installed_candidate"]["installed_root"])
    package_before = common.tree_hash(root)
    stdout_path = output_dir / f"{backend}-nondev-probe.stdout.log"
    stderr_path = output_dir / f"{backend}-nondev-probe.stderr.log"
    environment = os.environ.copy()
    for key in list(environment):
        if key == "DEVELOPER_MODE" or key.startswith("EBIR_NATIVE_EVIDENCE"):
            environment.pop(key, None)
    environment["EBIRFORMS_HTML_LINUX_HOST"] = "child" if backend == "x11" else "gtk"
    command = [
        bubblewrap,
        "--die-with-parent",
        "--unshare-net",
        "--bind",
        "/",
        "/",
        "--dev-bind",
        "/dev",
        "/dev",
        "--proc",
        "/proc",
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
    package_after = common.tree_hash(root)
    if package_after != package_before:
        raise EvidenceError("installed candidate changed during the non-development probe")
    probe = {
        "schema_version": SCHEMA_VERSION,
        "scope": PROBE_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "certification_complete": False,
        "form": FORM,
        "source_revision": binding["source_revision"],
        "backend": backend,
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "installed_root_sha256_before": package_before,
        "installed_root_sha256_after": package_after,
        "launch_argv": command,
        "dev_tools_enabled": False,
        "network_denial": {
            "mechanism": "bubblewrap --unshare-net",
            "enforced_for_launch": True,
            "passed": True,
        },
        "stdout": common.file_record(stdout_path),
        "stderr": common.file_record(stderr_path),
        "package_boundary": binding["package_boundary"],
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            RELEASE_PACKAGE_GAP,
            "startup does not prove preview, export, print, PDF, or rollback behavior",
        ],
    }
    common.write_json_atomic(output_dir / f"linux-{backend}-candidate-nondev-probe.json", probe)
    return probe


def _validate_geometry(measurements: Any, label: str) -> None:
    if not isinstance(measurements, list) or len(measurements) != 2:
        raise EvidenceError(f"{label} must retain exactly two stable geometry measurements")
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
            f"{label} geometry measurement {index}",
        )
        if measurement["measurement_index"] != index:
            raise EvidenceError(f"{label} geometry measurement indices are not canonical")
        if (
            measurement["page_width_pt"] != EXPECTED_WIDTH_POINTS
            or measurement["page_height_pt"] != EXPECTED_HEIGHT_POINTS
            or measurement["clipping_count"] != 0
            or measurement["overflow_count"] != 0
        ):
            raise EvidenceError(f"{label} preview geometry/clipping is invalid")
        pages = measurement["pages"]
        if not isinstance(pages, list) or len(pages) != EXPECTED_PAGE_COUNT:
            raise EvidenceError(f"{label} geometry must cover exactly two pages")
        for page_number, page in enumerate(pages, 1):
            page = require_exact_keys(
                page, {"page", "x", "y", "width_pt", "height_pt"}, f"{label} page rectangle"
            )
            if (
                page["page"] != page_number
                or page["width_pt"] != EXPECTED_WIDTH_POINTS
                or page["height_pt"] != EXPECTED_HEIGHT_POINTS
            ):
                raise EvidenceError(f"{label} page rectangle is invalid")
        normalized.append(measurement)
    if normalized[0] != ({**normalized[1], "measurement_index": 1}):
        raise EvidenceError(f"{label} geometry measurements are not identical")


def _validate_pdf_pages(pages: Any, label: str) -> None:
    if not isinstance(pages, list) or len(pages) != EXPECTED_PAGE_COUNT:
        raise EvidenceError(f"{label} PDF geometry must cover both pages")
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
            f"{label} PDF page {page_number}",
        )
        if (
            page["page"] != page_number
            or page["media_width_pt"] != EXPECTED_WIDTH_POINTS
            or page["media_height_pt"] != EXPECTED_HEIGHT_POINTS
            or page["crop_width_pt"] != EXPECTED_WIDTH_POINTS
            or page["crop_height_pt"] != EXPECTED_HEIGHT_POINTS
            or page["rotation"] != 0
            or not isinstance(page["content_byte_count"], int)
            or page["content_byte_count"] <= 0
        ):
            raise EvidenceError(f"{label} PDF page {page_number} geometry/content is invalid")


def _validate_rollback(
    rollback: Any, label: str, *, base: Path, verified: list[dict[str, Any]]
) -> None:
    rollback = require_exact_keys(
        rollback,
        {"cases", "destination_preserved", "temporary_files_remaining", "draft_unchanged"},
        label,
    )
    require_true(rollback["destination_preserved"], f"{label}.destination_preserved")
    require_true(rollback["draft_unchanged"], f"{label}.draft_unchanged")
    if rollback["temporary_files_remaining"] != 0:
        raise EvidenceError(f"{label} left temporary files")
    cases = rollback["cases"]
    if not isinstance(cases, list):
        raise EvidenceError(f"{label}.cases must be an array")
    seen: set[str] = set()
    for index, raw in enumerate(cases):
        case = require_exact_keys(raw, {"name", "passed", "artifact"}, f"{label} case {index}")
        name = case["name"]
        if name not in ROLLBACK_CASES or name in seen:
            raise EvidenceError(f"{label} case is unknown or duplicated: {name!r}")
        seen.add(name)
        require_true(case["passed"], f"{label} case {name}")
        verify_file_record(case["artifact"], f"{label} artifact {name}", base=base, verified=verified)
    missing = sorted(ROLLBACK_CASES - seen)
    if missing:
        raise EvidenceError(f"{label} is incomplete: {', '.join(missing)}")


def _validate_backend_run(
    backend: str,
    raw: Any,
    *,
    base: Path,
    verified: list[dict[str, Any]],
) -> dict[str, Any]:
    expected = BACKENDS[backend]
    run = require_exact_keys(
        raw,
        {
            "exercised",
            "passed",
            "display_server",
            "host_strategy",
            "app_owned_window",
            "external_browser",
            "window_title",
            "launch_argv",
            "pid",
            "display_environment",
            "network_denial",
            "lifecycle",
            "preview",
            "toolbar_export",
            "native_print",
            "pdf_validation",
            "integrity",
            "rollback",
            "artifact",
        },
        f"{backend} run",
    )
    require_true(run["exercised"], f"{backend}.exercised")
    require_true(run["passed"], f"{backend}.passed")
    if run["display_server"] != backend or run["host_strategy"] != expected["host_strategy"]:
        raise EvidenceError(f"{backend} used the wrong application-owned host strategy")
    require_true(run["app_owned_window"], f"{backend}.app_owned_window")
    require_false(run["external_browser"], f"{backend}.external_browser")
    if run["window_title"] != expected["window_title"]:
        raise EvidenceError(f"{backend} window title does not match the owned host")
    if not isinstance(run["launch_argv"], list) or not run["launch_argv"]:
        raise EvidenceError(f"{backend} launch argv is required")
    if not any(Path(argument).name == "bir" for argument in run["launch_argv"]):
        raise EvidenceError(f"{backend} launch argv does not contain the packaged binary")
    if any("dev-tools" in argument for argument in run["launch_argv"]):
        raise EvidenceError(f"{backend} launch argv enabled development tooling")
    if not isinstance(run["pid"], int) or run["pid"] < 1:
        raise EvidenceError(f"{backend} PID is invalid")

    environment = require_exact_keys(
        run["display_environment"],
        {
            "display_variable",
            "display_value",
            "runtime_directory",
            "compositor",
            "compositor_version",
            "gtk_version",
            "webkitgtk_version",
            "artifact",
        },
        f"{backend} display environment",
    )
    if (
        environment["display_variable"] != expected["display_variable"]
        or environment["compositor"] != expected["compositor"]
    ):
        raise EvidenceError(f"{backend} display environment does not match Xvfb/Weston policy")
    for key in ("display_value", "compositor_version", "gtk_version", "webkitgtk_version"):
        require_nonempty_string(environment[key], f"{backend}.{key}")
    if backend == "wayland":
        require_nonempty_string(environment["runtime_directory"], "wayland runtime directory")
    elif environment["runtime_directory"] is not None:
        raise EvidenceError("X11 runtime_directory must be null")
    verify_file_record(environment["artifact"], f"{backend} display artifact", base=base, verified=verified)

    denial = require_exact_keys(
        run["network_denial"],
        {
            "mechanism",
            "exercised",
            "enforced_for_launch",
            "passed",
            "host_namespace_inode",
            "candidate_namespace_inode",
            "artifact",
        },
        f"{backend} network denial",
    )
    if "unshare-net" not in require_nonempty_string(denial["mechanism"], f"{backend} denial mechanism"):
        raise EvidenceError(f"{backend} networking was not denied with a separate namespace")
    for key in ("exercised", "enforced_for_launch", "passed"):
        require_true(denial[key], f"{backend}.network_denial.{key}")
    host_inode = require_nonempty_string(denial["host_namespace_inode"], f"{backend} host namespace")
    candidate_inode = require_nonempty_string(
        denial["candidate_namespace_inode"], f"{backend} candidate namespace"
    )
    if NETWORK_NAMESPACE.fullmatch(host_inode) is None or NETWORK_NAMESPACE.fullmatch(candidate_inode) is None:
        raise EvidenceError(f"{backend} network namespace inode records are invalid")
    if host_inode == candidate_inode:
        raise EvidenceError(f"{backend} candidate did not run in a separate network namespace")
    verify_file_record(denial["artifact"], f"{backend} network artifact", base=base, verified=verified)

    lifecycle = require_exact_keys(
        run["lifecycle"],
        {"opened", "preview_ready", "close_reopen", "clean_shutdown", "artifact"},
        f"{backend} lifecycle",
    )
    for key in ("opened", "preview_ready", "close_reopen", "clean_shutdown"):
        require_true(lifecycle[key], f"{backend}.lifecycle.{key}")
    verify_file_record(lifecycle["artifact"], f"{backend} lifecycle artifact", base=base, verified=verified)

    preview = require_exact_keys(
        run["preview"],
        {
            "exercised",
            "passed",
            "document_run_id",
            "envelope_sha256",
            "nonce",
            "page_count",
            "geometry_measurements",
            "artifact",
        },
        f"{backend} preview",
    )
    require_true(preview["exercised"], f"{backend}.preview.exercised")
    require_true(preview["passed"], f"{backend}.preview.passed")
    require_nonempty_string(preview["document_run_id"], f"{backend} document run id")
    require_sha256(preview["envelope_sha256"], f"{backend} envelope hash")
    if not isinstance(preview["nonce"], int) or preview["nonce"] < 1:
        raise EvidenceError(f"{backend} preview nonce is invalid")
    if preview["page_count"] != EXPECTED_PAGE_COUNT:
        raise EvidenceError(f"{backend} preview page count is not exactly two")
    _validate_geometry(preview["geometry_measurements"], f"{backend} preview")
    verify_file_record(preview["artifact"], f"{backend} preview artifact", base=base, verified=verified)

    toolbar = require_exact_keys(
        run["toolbar_export"],
        {
            "exercised",
            "passed",
            "control",
            "save_chooser_exercised",
            "destination_path",
            "nonce",
            "artifact",
        },
        f"{backend} toolbar export",
    )
    require_true(toolbar["exercised"], f"{backend}.toolbar.exercised")
    require_true(toolbar["passed"], f"{backend}.toolbar.passed")
    require_true(toolbar["save_chooser_exercised"], f"{backend}.toolbar.save_chooser")
    if toolbar["control"] != "Export PDF" or toolbar["nonce"] != preview["nonce"]:
        raise EvidenceError(f"{backend} toolbar export is not bound to the preview nonce")
    require_nonempty_string(toolbar["destination_path"], f"{backend} export destination")
    verify_file_record(toolbar["artifact"], f"{backend} toolbar artifact", base=base, verified=verified)

    native_print = require_exact_keys(
        run["native_print"],
        {"exercised", "passed", "completed", "printer_name", "job_id", "artifact"},
        f"{backend} native print",
    )
    for key in ("exercised", "passed", "completed"):
        require_true(native_print[key], f"{backend}.native_print.{key}")
    require_nonempty_string(native_print["printer_name"], f"{backend} printer name")
    require_nonempty_string(native_print["job_id"], f"{backend} print job id")
    verify_file_record(native_print["artifact"], f"{backend} print artifact", base=base, verified=verified)

    pdf = require_exact_keys(
        run["pdf_validation"],
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
        f"{backend} PDF validation",
    )
    require_true(pdf["exercised"], f"{backend}.pdf.exercised")
    require_true(pdf["passed"], f"{backend}.pdf.passed")
    require_true(pdf["content_nonempty"], f"{backend}.pdf.content_nonempty")
    if pdf["expected_page_count"] != 2 or pdf["actual_page_count"] != 2:
        raise EvidenceError(f"{backend} PDF page count is not exactly two")
    if pdf["validated_by"] != "bir-print::html_output::validate_pdf_file":
        raise EvidenceError(f"{backend} PDF was not validated by the owned Rust verifier")
    require_sha256(pdf["verifier_executable_sha256"], f"{backend} verifier hash")
    output_pdf = verify_file_record(pdf["output"], f"{backend} exported PDF", base=base, verified=verified)
    if str(Path(toolbar["destination_path"]).resolve()) != output_pdf["path"]:
        raise EvidenceError(f"{backend} toolbar destination differs from the validated PDF")
    _validate_pdf_pages(pdf["pages"], backend)
    verify_file_record(pdf["artifact"], f"{backend} PDF verifier artifact", base=base, verified=verified)

    integrity = require_exact_keys(
        run["integrity"],
        {
            "installed_root_sha256_before",
            "installed_root_sha256_after",
            "destination_before",
            "destination_after",
            "draft_before",
            "draft_after",
            "temporary_files_manifest",
        },
        f"{backend} integrity",
    )
    for key in ("installed_root_sha256_before", "installed_root_sha256_after"):
        require_sha256(integrity[key], f"{backend}.{key}")
    before_destination = verify_file_record(
        integrity["destination_before"], f"{backend} destination-before", base=base, verified=verified
    )
    after_destination = verify_file_record(
        integrity["destination_after"], f"{backend} destination-after", base=base, verified=verified
    )
    before_draft = verify_file_record(
        integrity["draft_before"], f"{backend} draft-before", base=base, verified=verified
    )
    after_draft = verify_file_record(
        integrity["draft_after"], f"{backend} draft-after", base=base, verified=verified
    )
    for before, after, label in (
        (before_destination, after_destination, "destination"),
        (before_draft, after_draft, "draft"),
    ):
        if before["path"] == after["path"] or before["sha256"] != after["sha256"]:
            raise EvidenceError(f"{backend} {label} preservation evidence is invalid")
    temp_record = verify_file_record(
        integrity["temporary_files_manifest"],
        f"{backend} temporary-files manifest",
        base=base,
        verified=verified,
    )
    if load_json(Path(temp_record["path"]), limit=1024 * 1024) != {"remaining": []}:
        raise EvidenceError(f"{backend} temporary-files manifest reports leaked files")
    _validate_rollback(run["rollback"], f"{backend} rollback", base=base, verified=verified)
    verify_file_record(run["artifact"], f"{backend} run artifact", base=base, verified=verified)
    return run


def validate_attestation(
    attestation_path: Path, binding: dict[str, Any]
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    attestation_path = regular_file(attestation_path, "Linux attestation")
    attestation = load_json(attestation_path)
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
            "runtime",
            "display_runs",
            "package_boundary",
            "strict_verifier_gaps",
        },
        "Linux candidate attestation",
    )
    if attestation["schema_version"] != 1 or attestation["scope"] != ATTESTATION_SCOPE:
        raise EvidenceError("Linux attestation has an unsupported schema or scope")
    require_false(attestation["promotion_eligible"], "attestation promotion eligibility")
    require_false(attestation["trusted_producer"], "attestation trusted producer")
    require_true(attestation["operator_only"], "attestation.operator_only")
    try:
        uuid.UUID(str(attestation["attestation_id"]))
    except (ValueError, AttributeError) as error:
        raise EvidenceError("attestation_id must be a UUID") from error
    if attestation["form"] != FORM:
        raise EvidenceError("Linux attestation must target exactly 2551Q:2018")
    candidate = require_exact_keys(
        attestation["candidate"],
        {
            "candidate_manifest_sha256",
            "candidate_archive_sha256",
            "source_revision",
            "installed_root_sha256",
            "installed_binary_sha256",
            "renderer_bundle_sha256",
            "renderer_identity_sha256",
            "installation_method",
        },
        "attested candidate",
    )
    expected_candidate = {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "installed_root_sha256": binding["installed_candidate"]["installed_root_sha256"],
        "installed_binary_sha256": binding["installed_candidate"]["binary"]["sha256"],
        "renderer_bundle_sha256": binding["installed_candidate"]["renderer_bundle_sha256"],
        "renderer_identity_sha256": binding["installed_candidate"]["bundled_renderer_identity"]["sha256"],
        "installation_method": "secure_portable_tar_extraction",
    }
    if candidate != expected_candidate:
        raise EvidenceError("attestation is not bound to the exact installed candidate")
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
        "collector",
    )
    for key in ("name", "version", "invocation_id"):
        require_nonempty_string(collector[key], f"collector.{key}")
    for key in ("started_at_utc", "completed_at_utc"):
        if not isinstance(collector[key], str) or RFC3339_UTC.fullmatch(collector[key]) is None:
            raise EvidenceError(f"collector.{key} must be an RFC3339 UTC timestamp")
    started_at = datetime.fromisoformat(collector["started_at_utc"].replace("Z", "+00:00"))
    completed_at = datetime.fromisoformat(collector["completed_at_utc"].replace("Z", "+00:00"))
    if completed_at <= started_at:
        raise EvidenceError("collector completion must follow its start")
    require_sha256(collector["executable_sha256"], "collector executable hash")
    require_sha256(collector["host_identifier_sha256"], "collector host hash")

    runtime = require_exact_keys(
        attestation["runtime"],
        {
            "non_dev_build",
            "dev_tools_enabled",
            "installed_root_sha256",
            "installed_binary_sha256",
            "assets_tree_sha256",
            "renderer_bundle_sha256",
            "renderer_identity_sha256",
            "artifact",
        },
        "installed runtime",
    )
    require_true(runtime["non_dev_build"], "runtime.non_dev_build")
    require_false(runtime["dev_tools_enabled"], "runtime.dev_tools_enabled")
    expected_runtime = {
        "installed_root_sha256": binding["installed_candidate"]["installed_root_sha256"],
        "installed_binary_sha256": binding["installed_candidate"]["binary"]["sha256"],
        "assets_tree_sha256": binding["installed_candidate"]["assets_tree_sha256"],
        "renderer_bundle_sha256": binding["installed_candidate"]["renderer_bundle_sha256"],
        "renderer_identity_sha256": binding["installed_candidate"]["bundled_renderer_identity"]["sha256"],
    }
    for key, expected in expected_runtime.items():
        if runtime[key] != expected:
            raise EvidenceError(f"installed runtime {key} differs from the candidate binding")
    base = attestation_path.parent
    verified: list[dict[str, Any]] = []
    verify_file_record(runtime["artifact"], "installed runtime artifact", base=base, verified=verified)
    runs = require_exact_keys(attestation["display_runs"], set(BACKENDS), "display runs")
    for backend in BACKENDS:
        run = _validate_backend_run(backend, runs[backend], base=base, verified=verified)
        before = run["integrity"]["installed_root_sha256_before"]
        after = run["integrity"]["installed_root_sha256_after"]
        if before != expected_runtime["installed_root_sha256"] or after != before:
            raise EvidenceError(f"installed candidate changed during the {backend} run")
    boundary = require_exact_keys(
        attestation["package_boundary"],
        {
            "portable_candidate_verified",
            "final_release_deb_verified",
            "final_release_tarball_verified",
            "release_package_signature_verified",
            "artifact",
        },
        "package boundary",
    )
    require_true(boundary["portable_candidate_verified"], "portable candidate verification")
    for key in (
        "final_release_deb_verified",
        "final_release_tarball_verified",
        "release_package_signature_verified",
    ):
        require_false(boundary[key], f"package_boundary.{key}")
    verify_file_record(boundary["artifact"], "package-boundary artifact", base=base, verified=verified)
    gaps = attestation["strict_verifier_gaps"]
    if not isinstance(gaps, list) or NON_PROMOTIONAL_GAP not in gaps or RELEASE_PACKAGE_GAP not in gaps:
        raise EvidenceError("attestation erased an untrusted or final-package promotion blocker")
    return attestation, verified


def verify_owned_pdf_artifacts(
    attestation_path: Path,
    attestation: dict[str, Any],
    verifier_path: Path,
    verified: list[dict[str, Any]],
) -> dict[str, Any]:
    verifier_path = regular_file(verifier_path, "owned PDF verifier")
    verifier_record = common.file_record(verifier_path)
    if not os.access(verifier_path, os.X_OK):
        raise EvidenceError("owned PDF verifier is not executable")
    base = attestation_path.resolve(strict=True).parent
    results: dict[str, Any] = {}
    for backend in BACKENDS:
        pdf = attestation["display_runs"][backend]["pdf_validation"]
        if verifier_record["sha256"] != pdf["verifier_executable_sha256"]:
            raise EvidenceError(f"owned PDF verifier differs from the {backend} attested hash")
        output_pdf = resolve_record_path(pdf["output"], base).resolve(strict=True)
        artifact_path = resolve_record_path(pdf["artifact"], base).resolve(strict=True)
        try:
            result = subprocess.run(
                [
                    str(verifier_path),
                    str(output_pdf),
                    attestation["display_runs"][backend]["preview"]["envelope_sha256"],
                    "linux",
                ],
                capture_output=True,
                check=False,
                timeout=30,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise EvidenceError(f"owned PDF verifier is unavailable: {error}") from error
        if result.returncode != 0:
            message = result.stderr.decode("utf-8", errors="replace").strip()
            raise EvidenceError(f"owned PDF verifier rejected the {backend} export: {message}")
        if result.stderr:
            raise EvidenceError("owned PDF verifier emitted unexpected stderr output")
        if common.read_stable_file(artifact_path, limit=4 * 1024 * 1024) != result.stdout:
            raise EvidenceError(f"retained {backend} PDF-verifier artifact differs from a fresh run")
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
            f"owned {backend} PDF validation report",
        )
        expected = {
            "schema_version": 1,
            "scope": "owned_linux_candidate_pdf_validation",
            "promotion_eligible": False,
            "form": FORM,
            "envelope_sha256": attestation["display_runs"][backend]["preview"]["envelope_sha256"],
            "output_sha256": pdf["output"]["sha256"],
            "expected_page_count": 2,
            "actual_page_count": 2,
            "width_points": 612.0,
            "height_points": 936.0,
            "content_nonempty": True,
            "validated_by": "bir-print::html_output::validate_pdf_file",
            "pages": pdf["pages"],
        }
        if report != expected:
            raise EvidenceError(f"owned {backend} PDF report differs from the immutable attestation")
        results[backend] = {
            "output_sha256": report["output_sha256"],
            "actual_page_count": report["actual_page_count"],
            "width_points": report["width_points"],
            "height_points": report["height_points"],
            "content_nonempty": report["content_nonempty"],
        }
    verified.append(verifier_record | {"label": "owned PDF verifier executable"})
    return {"verifier_executable_sha256": verifier_record["sha256"], "backends": results}


def _run_required(
    command: list[str], label: str, *, environment: dict[str, str] | None = None
) -> str:
    try:
        result = subprocess.run(
            command,
            text=True,
            capture_output=True,
            check=False,
            timeout=30,
            env=environment,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"{label} is unavailable: {error}") from error
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"{label} failed closed: {output}")
    return output


def verify_live_linux_state(
    installed_root: Path, attestation: dict[str, Any]
) -> dict[str, Any]:
    if platform.system() != "Linux":
        raise EvidenceError("strict Linux attestation verification must run on Linux")
    installed_root = installed_root.resolve(strict=True)
    if common.tree_hash(installed_root) != attestation["runtime"]["installed_root_sha256"]:
        raise EvidenceError("installed candidate tree differs from the attestation")
    binary = installed_root / "bir"
    if common.file_record(binary)["sha256"] != attestation["runtime"]["installed_binary_sha256"]:
        raise EvidenceError("installed candidate binary differs from the attestation")
    if not os.access(binary, os.X_OK):
        raise EvidenceError("installed candidate binary is not executable")

    gtk_version = _run_required(["pkg-config", "--modversion", "gtk+-3.0"], "GTK3 version")
    webkitgtk_version = _run_required(
        ["pkg-config", "--modversion", "webkit2gtk-4.1"], "WebKitGTK version"
    )
    display_hashes: dict[str, str] = {}
    completed_job_hashes: dict[str, str] = {}
    for backend, expected in BACKENDS.items():
        run = attestation["display_runs"][backend]
        environment_record = run["display_environment"]
        if environment_record["gtk_version"] != gtk_version.strip():
            raise EvidenceError(f"live GTK3 version differs from the {backend} attestation")
        if environment_record["webkitgtk_version"] != webkitgtk_version.strip():
            raise EvidenceError(f"live WebKitGTK version differs from the {backend} attestation")
        environment = os.environ.copy()
        environment[expected["display_variable"]] = environment_record["display_value"]
        if backend == "x11":
            display_output = _run_required(
                ["xdpyinfo", "-display", environment_record["display_value"]],
                "X11/Xvfb display",
                environment=environment,
            )
        else:
            environment["XDG_RUNTIME_DIR"] = environment_record["runtime_directory"]
            wayland_info = shutil.which("wayland-info")
            if wayland_info is None:
                raise EvidenceError("wayland-info is required for live Weston verification")
            display_output = _run_required(
                [wayland_info], "Wayland/Weston display", environment=environment
            )
        display_hashes[backend] = common.sha256_bytes(display_output.encode("utf-8"))
        printer_name = run["native_print"]["printer_name"]
        printer_state = _run_required(["lpstat", "-p", printer_name], f"{backend} printer state")
        if "disabled" in printer_state.lower():
            raise EvidenceError(f"{backend} attested printer is disabled")
        completed_jobs = _run_required(
            ["lpstat", "-W", "completed", "-o", printer_name], f"{backend} completed print job"
        )
        completed_job_ids = {
            line.split(maxsplit=1)[0] for line in completed_jobs.splitlines() if line.strip()
        }
        if run["native_print"]["job_id"] not in completed_job_ids:
            raise EvidenceError(f"{backend} print job is not present in completed CUPS jobs")
        completed_job_hashes[backend] = common.sha256_bytes(completed_jobs.encode("utf-8"))
    return {
        "installed_root_hash_verified": True,
        "installed_binary_hash_verified": True,
        "x11_xvfb_available": True,
        "wayland_weston_available": True,
        "gtk_version": gtk_version.strip(),
        "webkitgtk_version": webkitgtk_version.strip(),
        "display_output_sha256": display_hashes,
        "completed_print_jobs_verified": True,
        "completed_print_job_output_sha256": completed_job_hashes,
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
    with tempfile.TemporaryDirectory(prefix="ebirforms-linux-certification-") as directory:
        root = extract_candidate_archive(archive_path, Path(directory))
        installed = bind_installed_candidate(candidate, root, identity_path)
        binding = {**candidate, "installed_candidate": installed}
        attestation, verified = validate_attestation(attestation_path, binding)
        owned_pdf_validation = verify_owned_pdf_artifacts(
            attestation_path, attestation, pdf_verifier_path, verified
        )
        live = verify_live_linux_state(root, attestation)
        if common.tree_hash(root) != installed["installed_root_sha256"]:
            raise EvidenceError("candidate changed during live Linux verification")
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
        "live_linux_verification": live,
        "package_boundary": {
            "portable_candidate_verified": True,
            "final_release_deb_verified": False,
            "final_release_tarball_verified": False,
            "release_package_signature_verified": False,
        },
        "strict_verifier_gaps": [
            NON_PROMOTIONAL_GAP,
            RELEASE_PACKAGE_GAP,
            "collector executable and operator identity are not externally attested",
            "final release package installation, signature, and publisher lineage remain unverified",
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
    probe = subcommands.add_parser(
        "probe", help="launch the actual non-dev app with networking denied"
    )
    probe.add_argument("--candidate-manifest", required=True, type=Path)
    probe.add_argument("--candidate-archive", required=True, type=Path)
    probe.add_argument("--renderer-identity", required=True, type=Path)
    probe.add_argument("--output-dir", required=True, type=Path)
    probe.add_argument("--backend", choices=sorted(BACKENDS), required=True)
    probe.add_argument("--timeout", type=float, default=5.0)
    verify = subcommands.add_parser(
        "verify-attestation", help="strictly verify complete X11 and Wayland attestations"
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
                probe_nondev_candidate(
                    binding,
                    arguments.output_dir.resolve(),
                    arguments.timeout,
                    arguments.backend,
                )
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
    except (EvidenceError, OSError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f"Linux candidate certification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
