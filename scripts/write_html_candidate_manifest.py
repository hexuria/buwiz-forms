#!/usr/bin/env python3
"""Write a hashed, explicitly non-promotional HTML candidate manifest.

This manifest is a certification input, not release evidence. It binds one
platform candidate archive to the clean renderer build identity so an external
collector can select the exact artifact it will exercise without changing
``form-release-evidence.json`` or any readiness capability.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
from pathlib import Path


SCHEMA_VERSION = 1
SCOPE = "html_certification_candidate_input"
SHA256 = re.compile(r"[0-9a-f]{64}")
GIT_REVISION = re.compile(r"[0-9a-f]{40}")
PLATFORMS = {"macos", "windows", "linux"}


class CandidateManifestError(ValueError):
    """Raised when a candidate cannot be bound to a clean renderer identity."""


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_file(path: Path, label: str) -> Path:
    if path.is_symlink() or not path.is_file():
        raise CandidateManifestError(f"{label} must be a regular, non-symlink file")
    return path


def load_renderer_identity(path: Path, source_revision: str) -> dict:
    regular_file(path, "renderer identity")
    try:
        identity = json.loads(path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CandidateManifestError(f"renderer identity is invalid JSON: {error}") from error

    if (
        identity.get("schema_version") != 1
        or identity.get("scope") != "build_time_non_promotional_identity"
        or identity.get("promotion_eligible") is not False
        or identity.get("offline_verification_passed") is not True
    ):
        raise CandidateManifestError("renderer identity has an unsupported or unsafe scope")
    renderer_hash = identity.get("renderer_bundle_sha256")
    if not isinstance(renderer_hash, str) or SHA256.fullmatch(renderer_hash) is None:
        raise CandidateManifestError("renderer identity has no canonical bundle hash")
    revision = identity.get("source_revision")
    if not isinstance(revision, dict) or revision.get("status") != "observed":
        raise CandidateManifestError("renderer identity is not bound to a clean source revision")
    if revision.get("value") != source_revision:
        raise CandidateManifestError("renderer identity source revision differs from the candidate")
    return identity


def build_manifest(
    *,
    platform: str,
    architecture: str,
    source_revision: str,
    artifact_path: Path,
    renderer_identity_path: Path,
) -> dict:
    if platform not in PLATFORMS:
        raise CandidateManifestError(f"unsupported candidate platform: {platform}")
    if not architecture.strip():
        raise CandidateManifestError("candidate architecture is required")
    if GIT_REVISION.fullmatch(source_revision) is None:
        raise CandidateManifestError("source revision must be a canonical Git commit")
    artifact = regular_file(artifact_path, "candidate artifact")
    identity = load_renderer_identity(renderer_identity_path, source_revision)

    return {
        "schema_version": SCHEMA_VERSION,
        "scope": SCOPE,
        "promotion_eligible": False,
        "trusted_producer": False,
        "form": {"code": "2551Q", "revision": "2018"},
        "source_revision": source_revision,
        "platform": platform,
        "architecture": architecture,
        "artifact": {
            "name": artifact.name,
            "byte_count": artifact.stat().st_size,
            "sha256": sha256_file(artifact),
        },
        "renderer_identity": {
            "name": renderer_identity_path.name,
            "sha256": sha256_file(renderer_identity_path),
            "renderer_bundle_sha256": identity["renderer_bundle_sha256"],
        },
        "release_policy": {
            "candidate_build_requires_release_ready": False,
            "tagged_release_still_requires_release_ready": True,
        },
    }


def write_json_atomic(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.partial")
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")
    with temporary.open("xb") as stream:
        stream.write(payload)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=sorted(PLATFORMS), required=True)
    parser.add_argument("--architecture", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--renderer-identity", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()

    try:
        manifest = build_manifest(
            platform=arguments.platform,
            architecture=arguments.architecture,
            source_revision=arguments.source_revision,
            artifact_path=arguments.artifact,
            renderer_identity_path=arguments.renderer_identity,
        )
        write_json_atomic(arguments.output, manifest)
    except (CandidateManifestError, OSError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
