#!/usr/bin/env python3
"""Shared fail-closed helpers for platform candidate certification tools.

The helpers in this module only bind explicitly non-promotional workflow
artifacts to their clean source revision and offline renderer identity.  They
do not write release evidence, register a producer, or change form readiness.
"""

from __future__ import annotations

import json
import os
import re
import stat
import unicodedata
import zipfile
from pathlib import Path, PurePosixPath
from typing import Any

import macos_native_evidence_driver as artifact_common


CANDIDATE_SCOPE = "html_certification_candidate_input"
FORM = {"code": "2551Q", "revision": "2018"}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
GIT_REVISION = re.compile(r"[0-9a-f]{40}\Z")
MAX_ARCHIVE_FILES = 20_000
MAX_ARCHIVE_BYTES = 2 * 1024 * 1024 * 1024
WINDOWS_DEVICE_NAMES = frozenset(
    {"con", "prn", "aux", "nul"}
    | {f"com{index}" for index in range(1, 10)}
    | {f"lpt{index}" for index in range(1, 10)}
)

EvidenceError = artifact_common.EvidenceError


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
        value = json.loads(artifact_common.read_stable_file(path, limit=limit))
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
    actual = artifact_common.file_record(path)
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
    *,
    expected_platform: str,
    expected_architecture: str,
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
    if manifest["promotion_eligible"] is not False:
        raise EvidenceError("candidate manifest must remain non-promotional")
    if manifest["trusted_producer"] is not False:
        raise EvidenceError("candidate manifest must not claim a trusted producer")
    if manifest["form"] != FORM:
        raise EvidenceError("candidate manifest must target exactly 2551Q:2018")
    source_revision = manifest["source_revision"]
    if (
        not isinstance(source_revision, str)
        or GIT_REVISION.fullmatch(source_revision) is None
    ):
        raise EvidenceError("candidate source revision is not canonical")
    if (
        manifest["platform"] != expected_platform
        or manifest["architecture"] != expected_architecture
    ):
        raise EvidenceError(
            f"candidate must be the {expected_platform} {expected_architecture} workflow artifact"
        )

    archive = require_exact_keys(
        manifest["artifact"], {"name", "byte_count", "sha256"}, "candidate archive"
    )
    archive_record = artifact_common.file_record(archive_path)
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
    actual_identity = artifact_common.file_record(identity_path)
    if identity_record.get("name") != identity_path.name:
        raise EvidenceError("renderer identity name differs from the manifest")
    if (
        require_sha256(identity_record.get("sha256"), "renderer identity hash")
        != actual_identity["sha256"]
    ):
        raise EvidenceError("renderer identity bytes differ from the manifest")
    expected_renderer = require_sha256(
        identity_record.get("renderer_bundle_sha256"), "renderer bundle hash"
    )
    identity = artifact_common.validate_build_identity(identity_path, expected_renderer)
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
        "candidate_manifest": artifact_common.file_record(manifest_path),
        "candidate_archive": archive_record,
        "renderer_identity": actual_identity,
        "renderer_bundle_sha256": expected_renderer,
    }


def _safe_member_path(name: str) -> PurePosixPath:
    if not name or "\\" in name or "\0" in name:
        raise EvidenceError(f"candidate archive contains an invalid member name: {name!r}")
    member = PurePosixPath(name)
    if member.is_absolute() or ".." in member.parts or not member.parts:
        raise EvidenceError(f"candidate archive member escapes extraction root: {name!r}")
    return member


def extract_portable_zip(archive: Path, destination: Path) -> Path:
    """Safely extract one case-insensitive portable application ZIP."""

    archive = regular_file(archive, "candidate archive")
    if destination.exists() and any(destination.iterdir()):
        raise EvidenceError("candidate extraction directory must be absent or empty")
    destination.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    seen_casefold: set[str] = set()
    seen_normalized: set[str] = set()
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
            for component in member.parts:
                if component.endswith((".", " ")) or ":" in component:
                    raise EvidenceError(
                        f"candidate archive contains a Windows-ambiguous path: {normalized}"
                    )
                device_name = component.split(".", maxsplit=1)[0].casefold()
                if device_name in WINDOWS_DEVICE_NAMES:
                    raise EvidenceError(
                        f"candidate archive contains a reserved Windows device path: {normalized}"
                    )
            normalized_casefold = unicodedata.normalize("NFC", normalized).casefold()
            if normalized in seen or normalized.casefold() in seen_casefold:
                raise EvidenceError(
                    f"candidate archive contains duplicate paths: {normalized}"
                )
            if normalized_casefold in seen_normalized:
                raise EvidenceError(
                    f"candidate archive contains Unicode-normalization-colliding paths: {normalized}"
                )
            seen.add(normalized)
            seen_casefold.add(normalized.casefold())
            seen_normalized.add(normalized_casefold)
            if info.flag_bits & 0x1:
                raise EvidenceError(f"candidate archive contains an encrypted member: {normalized}")
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
            target.chmod((mode & 0o777) or 0o644)
    return destination
