from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "audit_html_form_migration.py"
REPOSITORY_ROOT = SCRIPT_PATH.parent.parent
SPEC = importlib.util.spec_from_file_location("audit_html_form_migration", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
audit = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = audit
SPEC.loader.exec_module(audit)


def write_rgba_png(path: Path, width: int, height: int, pixels: bytes) -> None:
    if len(pixels) != width * height * 4:
        raise ValueError("invalid RGBA pixel buffer")

    def chunk(kind: bytes, payload: bytes) -> bytes:
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )

    rows = b"".join(
        b"\x00" + pixels[row * width * 4 : (row + 1) * width * 4]
        for row in range(height)
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(rows))
        + chunk(b"IEND", b"")
    )


class AuditHtmlFormMigrationTests(unittest.TestCase):
    HEAD_REVISION = "a" * 40
    STALE_REVISION = "b" * 40

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary_directory.cleanup)
        self.root = Path(self.temporary_directory.name) / "repo"
        self.root.mkdir()
        self._copy_current_audit_inputs()

    def _copy(self, relative: str) -> None:
        source = REPOSITORY_ROOT / relative
        destination = self.root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)

    def _copy_current_audit_inputs(self) -> None:
        for relative in (
            "packages/form-specs/form-migration-status.json",
            "packages/form-specs/form-release-evidence.json",
            "packages/form-renderer/references/manifest.json",
            "packages/form-renderer/visual/form-parity.spec.ts",
            "crates/bir-core/src/forms/support_level.rs",
        ):
            self._copy(relative)
        references = self.read_json("packages/form-renderer/references/manifest.json")
        for form in references["forms"]:
            for key in ("fixture", "formtype", "metadata", "template"):
                self._copy(form[key])
            for page in form["pages"]:
                self._copy(page["source_svg"])
                self._copy(page["reference_png"])

    def read_json(self, relative: str) -> dict:
        return json.loads((self.root / relative).read_text(encoding="utf-8"))

    def write_json(self, relative: str, value: dict) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def migration(self) -> tuple[dict, dict]:
        manifest = self.read_json("packages/form-specs/form-migration-status.json")
        return manifest, manifest["forms"][0]

    def references(self) -> tuple[dict, dict]:
        manifest = self.read_json("packages/form-renderer/references/manifest.json")
        return manifest, manifest["forms"][0]

    def install_evidence(self, relative: str, report: dict) -> dict:
        self.write_json(relative, report)
        payload = (self.root / relative).read_bytes()
        return {
            "passed": True,
            "path": relative,
            "sha256": hashlib.sha256(payload).hexdigest(),
        }

    def run_audit(self, *dirty_paths: str) -> audit.AuditResult:
        return audit.audit_repository(
            self.root,
            revision_context=audit.RevisionContext(
                source_revision=self.HEAD_REVISION,
                dirty_paths=tuple(dirty_paths),
            ),
        )

    def valid_visual_report(self) -> dict:
        reference_manifest, reference = self.references()
        manifest_path = self.root / "packages/form-renderer/references/manifest.json"
        page_reports = []
        for page in reference["pages"]:
            reference_path = self.root / page["reference_png"]
            actual_relative = f"evidence/visual/page-{page['page']}-actual.png"
            diff_relative = f"evidence/visual/page-{page['page']}-diff.png"
            actual_path = self.root / actual_relative
            actual_path.parent.mkdir(parents=True, exist_ok=True)
            width = page["reference_width_px"]
            height = page["reference_height_px"]
            reference_image = audit.read_png_rgba(reference_path)
            write_rgba_png(
                actual_path,
                width,
                height,
                reference_image.pixels,
            )
            self.assertNotEqual(actual_path.read_bytes(), reference_path.read_bytes())
            write_rgba_png(
                self.root / diff_relative,
                width,
                height,
                bytes(width * height * 4),
            )
            page_reports.append(
                {
                    "form_code": reference["code"],
                    "form_revision": reference["revision"],
                    "fixture": reference["fixture"],
                    "fixture_sha256": reference["fixture_sha256"],
                    "reference": page["reference_png"],
                    "reference_sha256": page["reference_png_sha256"],
                    "actual": actual_relative,
                    "actual_sha256": hashlib.sha256(actual_path.read_bytes()).hexdigest(),
                    "diff": diff_relative,
                    "diff_sha256": hashlib.sha256(
                        (self.root / diff_relative).read_bytes()
                    ).hexdigest(),
                    "page": page["page"],
                    "expected_width": width,
                    "expected_height": height,
                    "actual_width": width,
                    "actual_height": height,
                    "changed_pixels": 0,
                    "changed_percent": 0.0,
                    "max_changed_percent": 1.0,
                    "pixelmatch_threshold": 0.1,
                    "passed": True,
                }
            )
        return {
            "schema_version": 1,
            "gate": "visual_parity",
            "producer": audit.VISUAL_EVIDENCE_PRODUCER,
            "producer_path": audit.VISUAL_EVIDENCE_PRODUCER_PATH.as_posix(),
            "producer_sha256": hashlib.sha256(
                (
                    self.root
                    / audit.VISUAL_EVIDENCE_PRODUCER_PATH.as_posix()
                ).read_bytes()
            ).hexdigest(),
            "promotion_eligible": True,
            "source_worktree_clean": True,
            "generated_at": "2026-07-15T12:00:00.000Z",
            "passed": True,
            "source_revision": self.HEAD_REVISION,
            "ci_run_id": "12345",
            "platform": "darwin",
            "architecture": "arm64",
            "browser": "chromium",
            "device_scale_factor": 1.5,
            "references_manifest": "packages/form-renderer/references/manifest.json",
            "references_manifest_sha256": hashlib.sha256(
                manifest_path.read_bytes()
            ).hexdigest(),
            "expected_page_count": reference["page_count"],
            "measured_page_count": reference["page_count"],
            "pages": page_reports,
        }

    def valid_platform_report(self, gate: str, platform: str) -> dict:
        _, reference = self.references()
        artifact_relative = f"evidence/platform/{gate}-{platform}.package"
        renderer_relative = f"evidence/platform/{gate}-{platform}-index.html"
        artifact_path = self.root / artifact_relative
        renderer_path = self.root / renderer_relative
        artifact_path.parent.mkdir(parents=True, exist_ok=True)
        artifact_path.write_bytes(f"{gate}:{platform}:package\n".encode())
        renderer_path.write_text(f"{gate}:{platform}:renderer\n", encoding="utf-8")
        return {
            "schema_version": 1,
            "gate": gate,
            "producer": "self-attested-test-platform-report",
            "passed": True,
            "source_revision": self.HEAD_REVISION,
            "form_code": reference["code"],
            "form_revision": reference["revision"],
            "platform": platform,
            "architecture": "aarch64" if platform == "macos" else "x86_64",
            "artifact_kind": "macos_app" if platform == "macos" else "windows_msix",
            "artifact_path": artifact_relative,
            "artifact_sha256": hashlib.sha256(artifact_path.read_bytes()).hexdigest(),
            "renderer_assets": [
                {
                    "path": renderer_relative,
                    "sha256": hashlib.sha256(renderer_path.read_bytes()).hexdigest(),
                }
            ],
            "network_disabled_runtime": {"passed": True},
            "readiness": {"passed": True},
            "renderer_pages": [
                {
                    "page": page,
                    "expected_width_pt": reference["page_width_pt"],
                    "expected_height_pt": reference["page_height_pt"],
                    "actual_width_pt": reference["page_width_pt"],
                    "actual_height_pt": reference["page_height_pt"],
                    "passed": True,
                }
                for page in range(1, reference["page_count"] + 1)
            ],
            "native_print": {"exercised": True, "passed": True},
            "native_pdf_export": {"exercised": True, "passed": True},
            "pdf_validation": {"passed": True},
            "network_runtime_exercised": True,
            "packaged_runtime_promotion_satisfied": True,
        }

    def valid_rollback_report(self) -> dict:
        _, reference = self.references()
        rollback_root = self.root / "evidence/rollback"
        rollback_root.mkdir(parents=True, exist_ok=True)
        snapshots = {}
        for scope in ("destination", "draft"):
            payload = f"stable-{scope}\n".encode()
            for phase in ("before", "after"):
                relative = f"evidence/rollback/{scope}-{phase}.snapshot"
                path = self.root / relative
                path.write_bytes(payload)
                snapshots[f"{scope}_{phase}"] = {
                    "path": relative,
                    "sha256": hashlib.sha256(payload).hexdigest(),
                }
        cases = []
        for name in sorted(audit.ROLLBACK_CASES):
            relative = f"evidence/rollback/case-{name}.log"
            path = self.root / relative
            path.write_text(f"exercised {name}: pass\n", encoding="utf-8")
            cases.append(
                {
                    "name": name,
                    "passed": True,
                    "artifact_path": relative,
                    "artifact_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                }
            )
        temporary_relative = "evidence/rollback/temporary-files.json"
        self.write_json(temporary_relative, {"remaining": []})
        temporary_path = self.root / temporary_relative
        return {
            "schema_version": 1,
            "gate": "rollback_drill",
            "producer": "self-attested-test-rollback-report",
            "passed": True,
            "source_revision": self.HEAD_REVISION,
            "form_code": reference["code"],
            "form_revision": reference["revision"],
            "fixture_sha256": reference["fixture_sha256"],
            "cases": cases,
            **snapshots,
            "temporary_files_manifest_path": temporary_relative,
            "temporary_files_manifest_sha256": hashlib.sha256(
                temporary_path.read_bytes()
            ).hexdigest(),
            "temporary_files_remaining": 0,
        }

    def test_current_conservative_experimental_state_passes(self) -> None:
        result = self.run_audit()

        self.assertTrue(result.passed, result.errors)
        self.assertEqual(result.statuses, ("2551Q:2018 experimental",))

    def test_release_ready_requires_every_flag_and_rollback_evidence(self) -> None:
        manifest, form = self.migration()
        form["release_ready"] = True
        self.write_json("packages/form-specs/form-migration-status.json", manifest)

        result = self.run_audit()

        self.assertTrue(
            any("release_ready is missing promotion flags" in error for error in result.errors)
        )
        self.assertTrue(
            any("release_ready lacks passed rollback-drill evidence" in error for error in result.errors)
        )
        self.assertIn("contract_complete", " ".join(result.errors))

    def test_html_enabled_scaffold_is_rejected(self) -> None:
        manifest, form = self.migration()
        form["support_level"] = "ScaffoldOnly"
        self.write_json("packages/form-specs/form-migration-status.json", manifest)

        result = self.run_audit()

        self.assertTrue(any("scaffold form cannot enable HTML" in error for error in result.errors))

    def test_every_referenced_asset_hash_is_enforced(self) -> None:
        _, reference = self.references()
        fixture = self.root / reference["fixture"]
        fixture.write_bytes(fixture.read_bytes() + b"\n")

        result = self.run_audit()

        self.assertTrue(any("fixture sha256 mismatch" in error for error in result.errors))

    def test_positive_visual_flag_requires_curated_evidence(self) -> None:
        manifest, form = self.migration()
        form["visual_parity_complete"] = True
        self.write_json("packages/form-specs/form-migration-status.json", manifest)

        result = self.run_audit()

        self.assertTrue(
            any("visual_parity_complete lacks passed evidence" in error for error in result.errors)
        )

    def test_visual_evidence_requires_fixture_hash_and_actual_dimensions(self) -> None:
        report = self.valid_visual_report()
        report["pages"][0].pop("fixture_sha256")
        report["pages"][1]["actual_width"] -= 1
        pointer = self.install_evidence("evidence/visual.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("fixture_sha256 is inconsistent" in error for error in result.errors))
        self.assertTrue(any("expected/actual dimensions do not match" in error for error in result.errors))

    def test_reporter_shaped_visual_evidence_cannot_self_promote(self) -> None:
        pointer = self.install_evidence(
            "evidence/visual.json", self.valid_visual_report()
        )
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertFalse(result.passed)
        self.assertTrue(
            any(
                "no trusted attested visual evidence producer is registered" in error
                for error in result.errors
            )
        )

    def test_visual_evidence_rejects_untrusted_producer_and_hash(self) -> None:
        report = self.valid_visual_report()
        report["producer"] = "hand-authored-report"
        report["producer_sha256"] = "0" * 64
        pointer = self.install_evidence("evidence/untrusted-visual.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any(
                "no trusted attested visual evidence producer is registered" in error
                for error in result.errors
            )
        )
        self.assertTrue(any("producer sha256 mismatch" in error for error in result.errors))

    def test_visual_reference_cannot_double_as_rendered_output(self) -> None:
        report = self.valid_visual_report()
        page = report["pages"][0]
        reference_path = self.root / page["reference"]
        actual_path = self.root / page["actual"]
        shutil.copy2(reference_path, actual_path)
        page["actual_sha256"] = page["reference_sha256"]
        pointer = self.install_evidence("evidence/copied-reference.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any("must be derived independently" in error for error in result.errors)
        )

    def test_visual_percent_and_pass_are_recomputed_from_changed_pixels(self) -> None:
        report = self.valid_visual_report()
        report["pages"][0]["changed_pixels"] = 100_000
        report["pages"][0]["changed_percent"] = 0.0
        report["pages"][0]["passed"] = True
        pointer = self.install_evidence("evidence/forged-visual.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any("changed_percent does not match changed_pixels" in error for error in result.errors)
        )
        self.assertTrue(
            any("changed_pixels does not match rendered screenshot" in error for error in result.errors)
        )

    def test_visual_artifacts_are_hashed_and_recomputed_independently(self) -> None:
        report = self.valid_visual_report()
        page = report["pages"][0]
        actual_path = self.root / page["actual"]
        decoded = audit.read_png_rgba(actual_path)
        pixels = bytearray(decoded.pixels)
        pixels[0:4] = b"\x00\x00\x00\xff"
        write_rgba_png(actual_path, decoded.width, decoded.height, bytes(pixels))
        page["actual_sha256"] = hashlib.sha256(actual_path.read_bytes()).hexdigest()
        pointer = self.install_evidence("evidence/forged-artifacts.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any("changed_pixels does not match rendered screenshot" in error for error in result.errors),
            result.errors,
        )
        self.assertTrue(
            any("diff mask does not match independently recomputed" in error for error in result.errors),
            result.errors,
        )

    def test_visual_artifact_paths_must_exist_and_match_hashes(self) -> None:
        report = self.valid_visual_report()
        (self.root / report["pages"][0]["actual"]).unlink()
        diff_path = self.root / report["pages"][1]["diff"]
        diff_path.write_bytes(diff_path.read_bytes() + b"tampered")
        pointer = self.install_evidence("evidence/missing-visual-assets.json", report)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("missing" in error and "rendered screenshot" in error for error in result.errors))
        self.assertTrue(any("diff mask sha256 mismatch" in error for error in result.errors))

    def test_stale_and_mixed_revisions_are_rejected_for_every_evidence_kind(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        form_evidence = evidence["forms"]["2551Q:2018"]

        visual = self.valid_visual_report()
        visual["source_revision"] = "b" * 40
        form_evidence["visual_parity"] = self.install_evidence(
            "evidence/stale-visual.json", visual
        )

        native = self.valid_platform_report("native_print_export", "macos")
        native["source_revision"] = "c" * 40
        form_evidence["native_print_export"]["macos"] = self.install_evidence(
            "evidence/stale-native.json", native
        )

        packaged = self.valid_platform_report("packaged_offline", "windows")
        packaged["source_revision"] = "d" * 40
        form_evidence["packaged_offline"]["windows"] = self.install_evidence(
            "evidence/stale-packaged.json", packaged
        )

        rollback = self.valid_rollback_report()
        rollback["source_revision"] = "f" * 40
        form_evidence["rollback_drill"] = self.install_evidence(
            "evidence/stale-rollback.json", rollback
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        for label in (
            "visual parity",
            "native_print_export macos",
            "packaged_offline windows",
            "rollback drill",
        ):
            self.assertTrue(
                any(label in error and "stale source_revision" in error for error in result.errors),
                (label, result.errors),
            )
        self.assertTrue(
            any("mixed source revisions" in error for error in result.errors)
        )

    def test_curated_evidence_requires_clean_source_worktree(self) -> None:
        pointer = self.install_evidence(
            "evidence/visual.json", self.valid_visual_report()
        )
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["visual_parity"] = pointer
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit(" M packages/form-renderer/src/FormDocument.tsx")

        self.assertTrue(
            any("require a clean curated source worktree" in error for error in result.errors)
        )

    def test_curated_git_revision_ignores_evidence_only_changes_but_tracks_source(self) -> None:
        repository = Path(self.temporary_directory.name) / "revision-repo"
        source = repository / "apps/form-preview/index.html"
        evidence = repository / "packages/form-specs/form-release-evidence.json"
        source.parent.mkdir(parents=True)
        evidence.parent.mkdir(parents=True)
        source.write_text("source-v1\n", encoding="utf-8")
        evidence.write_text("{}\n", encoding="utf-8")

        def git(*arguments: str) -> str:
            return subprocess.run(
                ["git", *arguments],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()

        git("init", "-q")
        git("config", "user.name", "Renderer Audit Test")
        git("config", "user.email", "renderer-audit@example.invalid")
        git("add", ".")
        git("commit", "-qm", "source")
        source_revision = git("rev-parse", "HEAD")

        evidence.write_text('{"schema_version":1}\n', encoding="utf-8")
        uncommitted_evidence = audit.git_revision_context(repository)
        self.assertEqual(uncommitted_evidence.source_revision, source_revision)
        self.assertEqual(uncommitted_evidence.dirty_paths, ())

        git("add", "packages/form-specs/form-release-evidence.json")
        git("commit", "-qm", "evidence only")
        committed_evidence = audit.git_revision_context(repository)
        self.assertEqual(committed_evidence.source_revision, source_revision)
        self.assertEqual(committed_evidence.dirty_paths, ())

        source.write_text("source-v2\n", encoding="utf-8")
        dirty_source = audit.git_revision_context(repository)
        self.assertEqual(dirty_source.source_revision, source_revision)
        self.assertTrue(
            any("apps/form-preview/index.html" in path for path in dirty_source.dirty_paths)
        )

        git("add", "apps/form-preview/index.html")
        git("commit", "-qm", "source change")
        committed_source = audit.git_revision_context(repository)
        self.assertNotEqual(committed_source.source_revision, source_revision)
        self.assertEqual(committed_source.dirty_paths, ())

        clean_producer = subprocess.run(
            [
                sys.executable,
                str(SCRIPT_PATH),
                "--root",
                str(repository),
                "--print-source-revision",
                "--require-clean-source",
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(clean_producer.returncode, 0, clean_producer.stderr)
        self.assertEqual(
            clean_producer.stdout.strip(), committed_source.source_revision
        )

        source.write_text("dirty-before-render\n", encoding="utf-8")
        dirty_producer = subprocess.run(
            [
                sys.executable,
                str(SCRIPT_PATH),
                "--root",
                str(repository),
                "--print-source-revision",
                "--require-clean-source",
            ],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(dirty_producer.returncode, 0)
        self.assertIn(
            "visual evidence requires a clean curated source worktree",
            dirty_producer.stderr,
        )

    def test_complete_native_gate_requires_both_platforms_and_pdf_exercise(self) -> None:
        manifest, form = self.migration()
        form["native_print_export_verified"] = True
        self.write_json("packages/form-specs/form-migration-status.json", manifest)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        native = evidence["forms"]["2551Q:2018"]["native_print_export"]
        macos = self.valid_platform_report("native_print_export", "macos")
        macos["native_pdf_export"]["exercised"] = False
        native["macos"] = self.install_evidence("evidence/native-macos.json", macos)
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("must be exercised and passed" in error for error in result.errors))
        self.assertTrue(
            any("lacks passed windows evidence" in error for error in result.errors)
        )

    def test_platform_artifact_and_renderer_hashes_are_verified(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        report = self.valid_platform_report("native_print_export", "macos")
        (self.root / report["artifact_path"]).unlink()
        renderer_path = self.root / report["renderer_assets"][0]["path"]
        renderer_path.write_text("tampered renderer\n", encoding="utf-8")
        evidence["forms"]["2551Q:2018"]["native_print_export"]["macos"] = (
            self.install_evidence("evidence/forged-platform.json", report)
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("missing" in error and "packaged artifact" in error for error in result.errors))
        self.assertTrue(any("renderer_assets[0] sha256 mismatch" in error for error in result.errors))

    def test_native_print_must_be_explicitly_exercised(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        report = self.valid_platform_report("native_print_export", "macos")
        report["native_print"] = {"exercised": False, "passed": True}
        evidence["forms"]["2551Q:2018"]["native_print_export"]["macos"] = (
            self.install_evidence("evidence/unexercised-native-print.json", report)
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("native_print must be exercised and passed" in error for error in result.errors))

    def test_self_attested_platform_reports_cannot_promote_native_gate(self) -> None:
        manifest, form = self.migration()
        form["native_print_export_verified"] = True
        self.write_json("packages/form-specs/form-migration-status.json", manifest)
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        native = evidence["forms"]["2551Q:2018"]["native_print_export"]
        for platform in ("macos", "windows"):
            native[platform] = self.install_evidence(
                f"evidence/native-{platform}.json",
                self.valid_platform_report("native_print_export", platform),
            )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertFalse(result.passed)
        self.assertEqual(
            sum(
                "no trusted packaged platform evidence producer is registered" in error
                for error in result.errors
            ),
            2,
        )

    def test_static_offline_report_cannot_satisfy_packaged_gate(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        packaged = self.valid_platform_report("packaged_offline", "macos")
        packaged["network_runtime_exercised"] = False
        packaged["packaged_runtime_promotion_satisfied"] = False
        evidence["forms"]["2551Q:2018"]["packaged_offline"]["macos"] = (
            self.install_evidence("evidence/offline-static.json", packaged)
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any("must exercise the network runtime" in error for error in result.errors)
        )
        self.assertTrue(
            any("static or development evidence cannot satisfy" in error for error in result.errors)
        )

    def test_rollback_evidence_binds_case_logs_and_state_snapshots(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        report = self.valid_rollback_report()
        missing_case = report["cases"][0]
        (self.root / missing_case["artifact_path"]).unlink()
        after = report["draft_after"]
        after_path = self.root / after["path"]
        after_path.write_text("changed draft\n", encoding="utf-8")
        after["sha256"] = hashlib.sha256(after_path.read_bytes()).hexdigest()
        evidence["forms"]["2551Q:2018"]["rollback_drill"] = self.install_evidence(
            "evidence/forged-rollback.json", report
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(any("case" in error and "artifact" in error and "missing" in error for error in result.errors))
        self.assertTrue(any("draft before/after snapshot contents changed" in error for error in result.errors))

    def test_self_attested_rollback_report_cannot_promote_release(self) -> None:
        evidence = self.read_json("packages/form-specs/form-release-evidence.json")
        evidence["forms"]["2551Q:2018"]["rollback_drill"] = self.install_evidence(
            "evidence/self-attested-rollback.json",
            self.valid_rollback_report(),
        )
        self.write_json("packages/form-specs/form-release-evidence.json", evidence)

        result = self.run_audit()

        self.assertTrue(
            any(
                "no trusted rollback evidence producer is registered" in error
                for error in result.errors
            )
        )

    def test_curated_sources_cover_ci_evidence_tooling_and_core_mapping(self) -> None:
        for path in (
            ".github/workflows",
            "assets/macos",
            "assets/windows",
            "entitlements.dev.plist",
            "entitlements.plist",
            "installer.wxs",
            "scripts",
            "crates/bir-core/src",
        ):
            self.assertIn(path, audit.CURATED_SOURCE_PATHS)

    def test_unbound_status_boole_cannot_self_promote(self) -> None:
        manifest, form = self.migration()
        form["contract_complete"] = True
        form["validation_complete"] = True
        self.write_json("packages/form-specs/form-migration-status.json", manifest)

        result = self.run_audit()

        self.assertTrue(
            any(
                "promotion flags lack a trusted derived evidence producer" in error
                and "contract_complete" in error
                and "validation_complete" in error
                for error in result.errors
            )
        )

    def test_png_dimension_reader_rejects_non_png(self) -> None:
        path = self.root / "not-a-png.png"
        path.write_bytes(b"not a png")

        with self.assertRaisesRegex(ValueError, "not a PNG"):
            audit.png_dimensions(path)

    def test_png_dimension_reader_uses_ihdr_dimensions(self) -> None:
        path = self.root / "header.png"
        path.write_bytes(
            b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\rIHDR" + struct.pack(">II", 1224, 1872)
        )

        self.assertEqual(audit.png_dimensions(path), (1224, 1872))


if __name__ == "__main__":
    unittest.main()
