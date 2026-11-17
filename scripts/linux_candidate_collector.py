#!/usr/bin/env python3
"""Assemble and strictly replay a non-promotional Linux candidate attestation.

The collector is deliberately operator-only.  It does not click application
controls, submit a print job, manufacture rollback drills, or promote a form.
Instead it fail-closed validates separately retained X11 and Wayland run
bundles, a candidate-bound rollback bundle, and packaged-offline evidence.  It
then assembles the existing closed Linux attestation and invokes the strict
candidate verifier while the attested displays and completed CUPS jobs are
still available.

Run bundles must bind preview, PDF export, and system print to one immutable
2551Q document while retaining three distinct one-use nonces.  The current
attestation schema exposes the PDF nonce in its preview/export fields; the
additional preview-readiness and system-print nonces remain in the retained
run bundle and are validated here.  This producer and its report remain
untrusted and non-promotional regardless of success.
"""

from __future__ import annotations

import argparse
import getpass
import hashlib
import json
import os
import platform
import stat
import subprocess
import sys
import tarfile
import uuid
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Iterable


SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import linux_candidate_certification as certification  # noqa: E402


SCHEMA_VERSION = 1
COLLECTOR_NAME = "ebirforms external Linux candidate collector"
COLLECTOR_VERSION = "1"
RUN_SCOPE = "external_linux_candidate_backend_run_bundle"
ROLLBACK_SCOPE = "external_linux_candidate_rollback_bundle"
OFFLINE_SCOPE = "external_linux_candidate_packaged_offline_bundle"
FORM = certification.FORM


EvidenceError = certification.EvidenceError


def utc_now() -> str:
    return datetime.now(UTC).isoformat(timespec="microseconds").replace("+00:00", "Z")


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def require_exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    return certification.require_exact_keys(value, keys, label)


def require_nonempty_string(value: Any, label: str) -> str:
    return certification.require_nonempty_string(value, label)


def require_sha256(value: Any, label: str) -> str:
    return certification.require_sha256(value, label)


def file_record(path: Path) -> dict[str, Any]:
    return certification.common.file_record(path)


def write_json(path: Path, value: dict[str, Any]) -> None:
    certification.common.write_json_atomic(path, value)
    path.chmod(0o600)


def private_output_directory(path: Path) -> Path:
    for ancestor in (path, *path.parents):
        if ancestor.exists() and ancestor.is_symlink():
            raise EvidenceError("collector output directory may not traverse symlinks")
    if path.exists():
        if not path.is_dir() or any(path.iterdir()):
            raise EvidenceError("collector output directory must be absent or empty")
        path.chmod(0o700)
    else:
        path.mkdir(parents=True, mode=0o700)
    resolved = path.resolve(strict=True)
    metadata = resolved.stat()
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) != 0o700:
        raise EvidenceError("collector output directory must be current-user-owned mode 0700")
    return resolved


def candidate_binding(
    manifest_path: Path,
    archive_path: Path,
    identity_path: Path,
    extraction_root: Path,
) -> dict[str, Any]:
    candidate = certification.validate_candidate_inputs(
        manifest_path, archive_path, identity_path
    )
    installed_root = certification.extract_candidate_archive(archive_path, extraction_root)
    installed = certification.bind_installed_candidate(
        candidate, installed_root, identity_path
    )
    return {**candidate, "installed_candidate": installed}


def expected_candidate(binding: dict[str, Any]) -> dict[str, Any]:
    installed = binding["installed_candidate"]
    return {
        "candidate_manifest_sha256": binding["candidate_manifest"]["sha256"],
        "candidate_archive_sha256": binding["candidate_archive"]["sha256"],
        "source_revision": binding["source_revision"],
        "installed_root_sha256": installed["installed_root_sha256"],
        "installed_binary_sha256": installed["binary"]["sha256"],
        "assets_tree_sha256": installed["assets_tree_sha256"],
        "renderer_bundle_sha256": installed["renderer_bundle_sha256"],
        "renderer_identity_sha256": installed["bundled_renderer_identity"]["sha256"],
        "installation_method": "secure_portable_tar_extraction",
    }


def verify_record(record: Any, label: str, base: Path) -> dict[str, Any]:
    return certification.verify_file_record(record, label, base=base)


def validate_operation_binding(value: Any, backend: str) -> dict[str, Any]:
    operations = require_exact_keys(
        value, {"preview", "pdf_export", "system_print"}, f"{backend} operations"
    )
    normalized: dict[str, dict[str, Any]] = {}
    for operation in ("preview", "pdf_export", "system_print"):
        item = require_exact_keys(
            operations[operation],
            {
                "document_run_id",
                "envelope_sha256",
                "nonce",
                "preflight_consumptions",
                "completion_nonce",
            },
            f"{backend} {operation} binding",
        )
        require_nonempty_string(
            item["document_run_id"], f"{backend} {operation} document_run_id"
        )
        require_sha256(
            item["envelope_sha256"], f"{backend} {operation} envelope_sha256"
        )
        if type(item["nonce"]) is not int or item["nonce"] < 1:
            raise EvidenceError(f"{backend} {operation} nonce must be a positive integer")
        if (
            item["preflight_consumptions"] != [item["nonce"]]
            or item["completion_nonce"] != item["nonce"]
        ):
            raise EvidenceError(
                f"{backend} {operation} nonce was not consumed and completed exactly once"
            )
        normalized[operation] = item
    document_ids = {item["document_run_id"] for item in normalized.values()}
    envelopes = {item["envelope_sha256"] for item in normalized.values()}
    nonces = {item["nonce"] for item in normalized.values()}
    if len(document_ids) != 1 or len(envelopes) != 1:
        raise EvidenceError(
            f"{backend} preview, export, and print do not share one immutable document"
        )
    if len(nonces) != 3:
        raise EvidenceError(
            f"{backend} preview, export, and print must use distinct one-use nonces"
        )
    return normalized


def _validate_launch_argv(run: dict[str, Any], binding: dict[str, Any], backend: str) -> None:
    argv = run.get("launch_argv")
    if not isinstance(argv, list) or not argv or not all(
        isinstance(item, str) and item for item in argv
    ):
        raise EvidenceError(f"{backend} launch argv is unavailable")
    expected_hash = binding["installed_candidate"]["binary"]["sha256"]
    matching = []
    for argument in argv:
        try:
            candidate = Path(argument)
            if candidate.is_absolute() and candidate.name == "bir":
                candidate = certification.regular_file(
                    candidate, f"{backend} launched candidate binary"
                )
                if file_record(candidate)["sha256"] == expected_hash:
                    matching.append(argument)
        except (OSError, EvidenceError):
            continue
    if len(matching) != 1:
        raise EvidenceError(f"{backend} launch argv is not bound once to the exact binary")
    if not any("unshare-net" in argument for argument in argv):
        raise EvidenceError(f"{backend} launch argv did not retain network denial")
    if any("dev-tools" in argument for argument in argv):
        raise EvidenceError(f"{backend} launch argv enabled development tooling")


def load_backend_bundle(
    path: Path, backend: str, binding: dict[str, Any]
) -> dict[str, Any]:
    path = certification.regular_file(path, f"{backend} run bundle")
    value = certification.load_json(path)
    require_exact_keys(
        value,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "operator_only",
            "backend",
            "candidate",
            "operations",
            "operator",
            "run",
            "strict_verifier_gaps",
        },
        f"{backend} run bundle",
    )
    if value["schema_version"] != SCHEMA_VERSION or value["scope"] != RUN_SCOPE:
        raise EvidenceError(f"{backend} run bundle has an unsupported schema or scope")
    if (
        value["promotion_eligible"] is not False
        or value["trusted_producer"] is not False
        or value["operator_only"] is not True
    ):
        raise EvidenceError(f"{backend} run bundle must remain operator-only and untrusted")
    if value["backend"] != backend:
        raise EvidenceError(f"{backend} run bundle names the wrong display backend")
    if value["candidate"] != expected_candidate(binding):
        raise EvidenceError(f"{backend} run bundle does not bind the exact candidate")

    operations = validate_operation_binding(value["operations"], backend)
    operator = require_exact_keys(
        value["operator"],
        {
            "identity",
            "live_physical_print_consent",
            "print_submitted_by_operator",
            "collector_submitted_print",
            "artifact",
        },
        f"{backend} operator record",
    )
    require_nonempty_string(operator["identity"], f"{backend} operator identity")
    if (
        operator["live_physical_print_consent"] is not True
        or operator["print_submitted_by_operator"] is not True
        or operator["collector_submitted_print"] is not False
    ):
        raise EvidenceError(
            f"{backend} print must be explicitly consented to and submitted only by the operator"
        )
    verify_record(operator["artifact"], f"{backend} operator artifact", path.parent)

    run = value["run"]
    if not isinstance(run, dict):
        raise EvidenceError(f"{backend} run must be an object")
    expected = certification.BACKENDS[backend]
    constants = (
        run.get("display_server") == backend,
        run.get("host_strategy") == expected["host_strategy"],
        run.get("window_title") == expected["window_title"],
        run.get("app_owned_window") is True,
        run.get("external_browser") is False,
    )
    if not all(constants):
        raise EvidenceError(f"{backend} run did not use the required app-owned host")
    _validate_launch_argv(run, binding, backend)
    preview = run.get("preview")
    toolbar = run.get("toolbar_export")
    native_print = run.get("native_print")
    if not all(isinstance(item, dict) for item in (preview, toolbar, native_print)):
        raise EvidenceError(f"{backend} run is missing preview/export/print records")
    pdf_operation = operations["pdf_export"]
    if (
        preview.get("document_run_id") != pdf_operation["document_run_id"]
        or preview.get("envelope_sha256") != pdf_operation["envelope_sha256"]
        or preview.get("nonce") != pdf_operation["nonce"]
        or toolbar.get("nonce") != pdf_operation["nonce"]
    ):
        raise EvidenceError(f"{backend} run does not bind export to its immutable document")
    if native_print.get("completed") is not True:
        raise EvidenceError(f"{backend} physical print job was not completed")
    integrity = run.get("integrity")
    if not isinstance(integrity, dict):
        raise EvidenceError(f"{backend} run omitted installed-root integrity")
    installed_hash = binding["installed_candidate"]["installed_root_sha256"]
    if (
        integrity.get("installed_root_sha256_before") != installed_hash
        or integrity.get("installed_root_sha256_after") != installed_hash
    ):
        raise EvidenceError(f"{backend} run changed or substituted the candidate root")
    gaps = value["strict_verifier_gaps"]
    if not isinstance(gaps, list) or not gaps or not all(
        isinstance(item, str) and item for item in gaps
    ):
        raise EvidenceError(f"{backend} run bundle must retain verifier gaps")
    return {
        "path": path,
        "record": file_record(path),
        "operations": operations,
        "operator_identity": operator["identity"],
        "run": run,
        "strict_verifier_gaps": gaps,
    }


def load_rollback_bundle(
    path: Path, binding: dict[str, Any], runs: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    path = certification.regular_file(path, "rollback bundle")
    value = certification.load_json(path)
    require_exact_keys(
        value,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "candidate",
            "backends",
            "strict_verifier_gaps",
        },
        "rollback bundle",
    )
    if value["schema_version"] != SCHEMA_VERSION or value["scope"] != ROLLBACK_SCOPE:
        raise EvidenceError("rollback bundle has an unsupported schema or scope")
    if value["promotion_eligible"] is not False or value["trusted_producer"] is not False:
        raise EvidenceError("rollback bundle must remain non-promotional and untrusted")
    if value["candidate"] != expected_candidate(binding):
        raise EvidenceError("rollback bundle does not bind the exact candidate")
    backends = require_exact_keys(value["backends"], set(certification.BACKENDS), "rollback backends")
    for backend in certification.BACKENDS:
        item = require_exact_keys(
            backends[backend], {"integrity", "rollback"}, f"{backend} rollback bundle"
        )
        run = runs[backend]["run"]
        if item["integrity"] != run.get("integrity") or item["rollback"] != run.get("rollback"):
            raise EvidenceError(f"{backend} run and rollback bundle differ")
    gaps = value["strict_verifier_gaps"]
    if not isinstance(gaps, list) or not gaps:
        raise EvidenceError("rollback bundle must retain explicit verifier gaps")
    return {"path": path, "record": file_record(path), "strict_verifier_gaps": gaps}


def load_offline_bundle(
    path: Path, binding: dict[str, Any], runs: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    path = certification.regular_file(path, "packaged offline bundle")
    value = certification.load_json(path)
    require_exact_keys(
        value,
        {
            "schema_version",
            "scope",
            "promotion_eligible",
            "trusted_producer",
            "candidate",
            "offline_package",
            "network_denial",
            "artifacts",
            "strict_verifier_gaps",
        },
        "packaged offline bundle",
    )
    if value["schema_version"] != SCHEMA_VERSION or value["scope"] != OFFLINE_SCOPE:
        raise EvidenceError("packaged offline bundle has an unsupported schema or scope")
    if value["promotion_eligible"] is not False or value["trusted_producer"] is not False:
        raise EvidenceError("packaged offline bundle must remain non-promotional and untrusted")
    if value["candidate"] != expected_candidate(binding):
        raise EvidenceError("packaged offline bundle does not bind the exact candidate")
    package = require_exact_keys(
        value["offline_package"],
        {
            "offline_renderer_verified",
            "no_legacy_audit_passed",
            "external_network_requests",
            "node_runtime_present",
            "node_modules_present",
            "typst_present",
            "runtime_formtypes_present",
        },
        "offline package result",
    )
    if package != {
        "offline_renderer_verified": True,
        "no_legacy_audit_passed": True,
        "external_network_requests": 0,
        "node_runtime_present": False,
        "node_modules_present": False,
        "typst_present": False,
        "runtime_formtypes_present": False,
    }:
        raise EvidenceError("packaged offline verification is incomplete")
    denial = require_exact_keys(
        value["network_denial"], set(certification.BACKENDS), "offline network denial"
    )
    for backend in certification.BACKENDS:
        if denial[backend] != runs[backend]["run"].get("network_denial"):
            raise EvidenceError(f"{backend} network-denial evidence differs from its run")
    artifacts = require_exact_keys(
        value["artifacts"],
        {"offline_renderer", "no_legacy", "x11_network", "wayland_network"},
        "offline artifacts",
    )
    verified = {
        name: verify_record(record, f"offline {name}", path.parent)
        for name, record in artifacts.items()
    }
    gaps = value["strict_verifier_gaps"]
    if not isinstance(gaps, list) or not gaps:
        raise EvidenceError("offline bundle must retain explicit verifier gaps")
    return {
        "path": path,
        "record": file_record(path),
        "artifacts": verified,
        "strict_verifier_gaps": gaps,
    }


def completed_job_exists(printer: str, job_id: str) -> dict[str, Any]:
    if not printer.strip() or not job_id.strip():
        raise EvidenceError("completed CUPS printer and job identifiers are required")
    try:
        result = subprocess.run(
            ["lpstat", "-W", "completed", "-o", printer],
            text=True,
            capture_output=True,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise EvidenceError(f"completed CUPS jobs are unavailable: {error}") from error
    output = f"{result.stdout}\n{result.stderr}".strip()
    if result.returncode != 0:
        raise EvidenceError(f"completed CUPS jobs could not be read: {output}")
    identifiers = {
        line.split(maxsplit=1)[0] for line in result.stdout.splitlines() if line.strip()
    }
    if job_id not in identifiers:
        raise EvidenceError(f"completed CUPS job is absent: {job_id}")
    return {"printer": printer, "job_id": job_id, "output_sha256": sha256_text(result.stdout)}


def host_identifier_sha256() -> str:
    parts = [platform.node(), getpass.getuser()]
    machine_id = Path("/etc/machine-id")
    try:
        parts.append(machine_id.read_text(encoding="utf-8").strip())
    except OSError:
        pass
    return sha256_text(":".join(parts))


def _attested_candidate(binding: dict[str, Any]) -> dict[str, Any]:
    expected = expected_candidate(binding)
    return {
        key: expected[key]
        for key in (
            "candidate_manifest_sha256",
            "candidate_archive_sha256",
            "source_revision",
            "installed_root_sha256",
            "installed_binary_sha256",
            "renderer_bundle_sha256",
            "renderer_identity_sha256",
            "installation_method",
        )
    }


def collect(arguments: argparse.Namespace) -> Path:
    if platform.system() != "Linux":
        raise EvidenceError("the external Linux collector must run on Linux")
    if not arguments.allow_live_print_evidence:
        raise EvidenceError(
            "--allow-live-print-evidence is required because both bundles retain real print jobs"
        )
    require_nonempty_string(arguments.operator_identity, "operator identity")
    started_at = utc_now()
    output = private_output_directory(arguments.output_dir)
    binding = candidate_binding(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        output / "installed-candidate",
    )
    installed_root = Path(binding["installed_candidate"]["installed_root"])
    installed_hash_before = certification.common.tree_hash(installed_root)
    runs = {
        "x11": load_backend_bundle(arguments.x11_run_bundle, "x11", binding),
        "wayland": load_backend_bundle(arguments.wayland_run_bundle, "wayland", binding),
    }
    for backend in certification.BACKENDS:
        if runs[backend]["operator_identity"] != arguments.operator_identity:
            raise EvidenceError(
                f"{backend} run bundle operator identity differs from --operator-identity"
            )
    rollback = load_rollback_bundle(arguments.rollback_bundle, binding, runs)
    offline = load_offline_bundle(arguments.offline_bundle, binding, runs)

    print_jobs = {
        (
            runs[backend]["run"]["native_print"]["printer_name"],
            runs[backend]["run"]["native_print"]["job_id"],
        )
        for backend in certification.BACKENDS
    }
    if len(print_jobs) != len(certification.BACKENDS):
        raise EvidenceError("X11 and Wayland must retain distinct completed print jobs")
    export_destinations = {
        str(runs[backend]["run"]["toolbar_export"].get("destination_path", ""))
        for backend in certification.BACKENDS
    }
    if "" in export_destinations or len(export_destinations) != len(certification.BACKENDS):
        raise EvidenceError("X11 and Wayland must retain distinct PDF export destinations")
    cups = {}
    for backend in certification.BACKENDS:
        native_print = runs[backend]["run"]["native_print"]
        cups[backend] = completed_job_exists(
            native_print["printer_name"], native_print["job_id"]
        )
    if certification.common.tree_hash(installed_root) != installed_hash_before:
        raise EvidenceError("installed candidate changed while evidence was assembled")

    runtime_artifact = output / "runtime-binding.json"
    write_json(
        runtime_artifact,
        {
            "promotion_eligible": False,
            "trusted_producer": False,
            "candidate": expected_candidate(binding),
            "run_bundles": {backend: runs[backend]["record"] for backend in runs},
            "rollback_bundle": rollback["record"],
            "offline_bundle": offline["record"],
            "operator_identity_sha256": sha256_text(arguments.operator_identity),
            "completed_cups_jobs": cups,
        },
    )
    package_boundary_artifact = output / "package-boundary.json"
    write_json(
        package_boundary_artifact,
        {
            "portable_candidate_verified": True,
            "final_release_deb_verified": False,
            "final_release_tarball_verified": False,
            "release_package_signature_verified": False,
            "reason": "the workflow portable candidate is not a final release package",
        },
    )
    completed_at = utc_now()
    strict_gaps = list(
        dict.fromkeys(
            [
                certification.NON_PROMOTIONAL_GAP,
                certification.RELEASE_PACKAGE_GAP,
                "backend run bundles are supplied by an untrusted external operator",
                "the collector validates but does not itself drive the application UI",
                "physical print jobs are submitted only by the operator",
                *runs["x11"]["strict_verifier_gaps"],
                *runs["wayland"]["strict_verifier_gaps"],
                *rollback["strict_verifier_gaps"],
                *offline["strict_verifier_gaps"],
            ]
        )
    )
    installed = binding["installed_candidate"]
    attestation = {
        "schema_version": SCHEMA_VERSION,
        "scope": certification.ATTESTATION_SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "operator_only": True,
        "attestation_id": str(uuid.uuid4()),
        "form": FORM,
        "candidate": _attested_candidate(binding),
        "collector": {
            "name": COLLECTOR_NAME,
            "version": COLLECTOR_VERSION,
            "invocation_id": str(uuid.uuid4()),
            "started_at_utc": started_at,
            "completed_at_utc": completed_at,
            "executable_sha256": file_record(Path(__file__).resolve())["sha256"],
            "host_identifier_sha256": host_identifier_sha256(),
        },
        "runtime": {
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "installed_root_sha256": installed["installed_root_sha256"],
            "installed_binary_sha256": installed["binary"]["sha256"],
            "assets_tree_sha256": installed["assets_tree_sha256"],
            "renderer_bundle_sha256": installed["renderer_bundle_sha256"],
            "renderer_identity_sha256": installed["bundled_renderer_identity"]["sha256"],
            "artifact": file_record(runtime_artifact),
        },
        "display_runs": {backend: runs[backend]["run"] for backend in runs},
        "package_boundary": {
            "portable_candidate_verified": True,
            "final_release_deb_verified": False,
            "final_release_tarball_verified": False,
            "release_package_signature_verified": False,
            "artifact": file_record(package_boundary_artifact),
        },
        "strict_verifier_gaps": strict_gaps,
    }
    attestation_path = output / "linux-candidate-attestation.json"
    write_json(attestation_path, attestation)
    certification.validate_attestation(attestation_path, binding)
    report_path = output / "linux-candidate-certification-report.json"
    certification.verify_attestation_command(
        arguments.candidate_manifest,
        arguments.candidate_archive,
        arguments.renderer_identity,
        attestation_path,
        arguments.pdf_verifier,
        report_path,
    )
    report = certification.load_json(report_path)
    if (
        report.get("promotion_eligible") is not False
        or report.get("trusted_producer") is not False
        or report.get("promotion_satisfied") is not False
    ):
        raise EvidenceError("strict verifier report escaped the non-promotional boundary")
    return report_path


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-manifest", required=True, type=Path)
    parser.add_argument("--candidate-archive", required=True, type=Path)
    parser.add_argument("--renderer-identity", required=True, type=Path)
    parser.add_argument("--pdf-verifier", required=True, type=Path)
    parser.add_argument("--x11-run-bundle", required=True, type=Path)
    parser.add_argument("--wayland-run-bundle", required=True, type=Path)
    parser.add_argument("--rollback-bundle", required=True, type=Path)
    parser.add_argument("--offline-bundle", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--operator-identity", required=True)
    parser.add_argument(
        "--allow-live-print-evidence",
        action="store_true",
        help="required acknowledgement that both run bundles retain real completed print jobs",
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
        tarfile.TarError,
        subprocess.SubprocessError,
    ) as error:
        print(f"Linux candidate collection failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
