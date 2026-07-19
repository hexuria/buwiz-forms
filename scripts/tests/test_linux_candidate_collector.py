from __future__ import annotations

import argparse
import importlib.util
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/linux_candidate_collector.py"
SPEC = importlib.util.spec_from_file_location("linux_candidate_collector", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
collector = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(collector)


class LinuxCandidateCollectorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name).resolve()
        self.source_revision = "a" * 40
        self.application = self.root / "source/eBIRForms-Linux-x64"
        renderer = self.application / collector.certification.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.application / "bir"
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        self.renderer_hash = collector.certification.common.tree_hash(renderer)
        self.identity = self.root / "form-renderer-build-identity.json"
        identity = {
            "schema_version": 1,
            "scope": "build_time_non_promotional_identity",
            "promotion_eligible": False,
            "offline_verification_passed": True,
            "renderer_bundle_relative_path": "assets/form-renderer",
            "renderer_bundle_sha256": self.renderer_hash,
            "source_revision": {"status": "observed", "value": self.source_revision},
        }
        encoded_identity = (json.dumps(identity, sort_keys=True) + "\n").encode()
        self.identity.write_bytes(encoded_identity)
        bundled_identity = self.application / collector.certification.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(encoded_identity)
        self.archive = self.root / "eBIRForms-Linux-x64-candidate.tar.gz"
        with tarfile.open(self.archive, "w:gz") as bundle:
            bundle.add(
                self.application,
                arcname=self.application.name,
                recursive=True,
            )
        self.manifest = self.root / "candidate-manifest.json"
        archive_record = self._record(self.archive)
        identity_record = self._record(self.identity)
        self.manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": collector.certification.CANDIDATE_SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": collector.FORM,
                    "source_revision": self.source_revision,
                    "platform": "linux",
                    "architecture": "x86_64",
                    "artifact": {
                        "name": self.archive.name,
                        "byte_count": archive_record["byte_count"],
                        "sha256": archive_record["sha256"],
                    },
                    "renderer_identity": {
                        "name": self.identity.name,
                        "sha256": identity_record["sha256"],
                        "renderer_bundle_sha256": self.renderer_hash,
                    },
                    "release_policy": {
                        "candidate_build_requires_release_ready": False,
                        "tagged_release_still_requires_release_ready": True,
                    },
                },
                sort_keys=True,
            )
            + "\n"
        )
        self.binding = collector.candidate_binding(
            self.manifest,
            self.archive,
            self.identity,
            self.root / "extracted",
        )
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def _record(self, path: Path) -> dict:
        return collector.certification.common.file_record(path)

    def _artifact(self, name: str, content: str = "observed\n") -> dict:
        path = self.evidence / name
        path.write_text(content)
        return self._record(path)

    def _run(self, backend: str) -> tuple[dict, dict]:
        expected = collector.certification.BACKENDS[backend]
        document = f"{backend}-2551q-document"
        envelope = ("d" if backend == "x11" else "e") * 64
        base_nonce = 10 if backend == "x11" else 20
        generic = self._artifact(f"{backend}-generic.log")
        output_pdf = self.evidence / f"{backend}-export.pdf"
        output_pdf.write_bytes(f"{backend} PDF placeholder\n".encode())
        binary = self.binding["installed_candidate"]["binary"]["path"]
        network = {
            "mechanism": "bubblewrap --unshare-net",
            "exercised": True,
            "enforced_for_launch": True,
            "passed": True,
            "host_namespace_inode": "net:[100]",
            "candidate_namespace_inode": f"net:[{200 + base_nonce}]",
            "artifact": generic,
        }
        integrity = {
            "installed_root_sha256_before": self.binding["installed_candidate"][
                "installed_root_sha256"
            ],
            "installed_root_sha256_after": self.binding["installed_candidate"][
                "installed_root_sha256"
            ],
            "destination_before": generic,
            "destination_after": generic,
            "draft_before": generic,
            "draft_after": generic,
            "temporary_files_manifest": generic,
        }
        rollback = {
            "cases": [],
            "destination_preserved": True,
            "temporary_files_remaining": 0,
            "draft_unchanged": True,
        }
        run = {
            "exercised": True,
            "passed": True,
            "display_server": backend,
            "host_strategy": expected["host_strategy"],
            "app_owned_window": True,
            "external_browser": False,
            "window_title": expected["window_title"],
            "launch_argv": ["bwrap", "--unshare-net", binary],
            "pid": base_nonce,
            "display_environment": {},
            "network_denial": network,
            "lifecycle": {},
            "preview": {
                "document_run_id": document,
                "envelope_sha256": envelope,
                "nonce": base_nonce + 1,
            },
            "toolbar_export": {
                "nonce": base_nonce + 1,
                "destination_path": str(output_pdf.resolve()),
            },
            "native_print": {
                "completed": True,
                "printer_name": f"Certification_{backend}",
                "job_id": f"Certification_{backend}-42",
            },
            "pdf_validation": {},
            "integrity": integrity,
            "rollback": rollback,
            "artifact": generic,
        }
        operator_artifact = self._artifact(f"{backend}-operator.json")
        bundle = {
            "schema_version": 1,
            "scope": collector.RUN_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "operator_only": True,
            "backend": backend,
            "candidate": collector.expected_candidate(self.binding),
            "operations": {
                "preview": {
                    "document_run_id": document,
                    "envelope_sha256": envelope,
                    "nonce": base_nonce,
                    "preflight_consumptions": [base_nonce],
                    "completion_nonce": base_nonce,
                },
                "pdf_export": {
                    "document_run_id": document,
                    "envelope_sha256": envelope,
                    "nonce": base_nonce + 1,
                    "preflight_consumptions": [base_nonce + 1],
                    "completion_nonce": base_nonce + 1,
                },
                "system_print": {
                    "document_run_id": document,
                    "envelope_sha256": envelope,
                    "nonce": base_nonce + 2,
                    "preflight_consumptions": [base_nonce + 2],
                    "completion_nonce": base_nonce + 2,
                },
            },
            "operator": {
                "identity": "external Linux operator",
                "live_physical_print_consent": True,
                "print_submitted_by_operator": True,
                "collector_submitted_print": False,
                "artifact": operator_artifact,
            },
            "run": run,
            "strict_verifier_gaps": ["external run driver is not registered as trusted"],
        }
        path = self.evidence / f"{backend}-run-bundle.json"
        path.write_text(json.dumps(bundle, indent=2, sort_keys=True) + "\n")
        return path, bundle

    def _run_bundles(self) -> tuple[dict[str, dict], dict[str, Path]]:
        values = {}
        paths = {}
        for backend in collector.certification.BACKENDS:
            path, value = self._run(backend)
            values[backend] = value
            paths[backend] = path
        return values, paths

    def _rollback_bundle(self, runs: dict[str, dict]) -> tuple[Path, dict]:
        value = {
            "schema_version": 1,
            "scope": collector.ROLLBACK_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "candidate": collector.expected_candidate(self.binding),
            "backends": {
                backend: {
                    "integrity": runs[backend]["run"]["integrity"],
                    "rollback": runs[backend]["run"]["rollback"],
                }
                for backend in collector.certification.BACKENDS
            },
            "strict_verifier_gaps": ["rollback producer is not registered as trusted"],
        }
        path = self.evidence / "rollback-bundle.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def _offline_bundle(self, runs: dict[str, dict]) -> tuple[Path, dict]:
        value = {
            "schema_version": 1,
            "scope": collector.OFFLINE_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "candidate": collector.expected_candidate(self.binding),
            "offline_package": {
                "offline_renderer_verified": True,
                "no_legacy_audit_passed": True,
                "external_network_requests": 0,
                "node_runtime_present": False,
                "node_modules_present": False,
                "typst_present": False,
                "runtime_formtypes_present": False,
            },
            "network_denial": {
                backend: runs[backend]["run"]["network_denial"]
                for backend in collector.certification.BACKENDS
            },
            "artifacts": {
                "offline_renderer": self._artifact("offline-renderer.json"),
                "no_legacy": self._artifact("no-legacy.json"),
                "x11_network": self._artifact("x11-network.json"),
                "wayland_network": self._artifact("wayland-network.json"),
            },
            "strict_verifier_gaps": ["offline producer is not registered as trusted"],
        }
        path = self.evidence / "offline-bundle.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def test_operation_binding_requires_one_document_and_three_distinct_nonces(self) -> None:
        _, bundle = self._run("x11")
        operations = collector.validate_operation_binding(bundle["operations"], "x11")
        self.assertEqual(operations["pdf_export"]["nonce"], 11)
        bundle["operations"]["system_print"]["nonce"] = 11
        bundle["operations"]["system_print"]["preflight_consumptions"] = [11]
        bundle["operations"]["system_print"]["completion_nonce"] = 11
        with self.assertRaisesRegex(collector.EvidenceError, "distinct one-use nonces"):
            collector.validate_operation_binding(bundle["operations"], "x11")
        bundle["operations"]["system_print"]["nonce"] = 12
        bundle["operations"]["system_print"]["preflight_consumptions"] = [12]
        bundle["operations"]["system_print"]["completion_nonce"] = 12
        bundle["operations"]["system_print"]["envelope_sha256"] = "f" * 64
        with self.assertRaisesRegex(collector.EvidenceError, "immutable document"):
            collector.validate_operation_binding(bundle["operations"], "x11")
        bundle["operations"]["system_print"]["envelope_sha256"] = "d" * 64
        bundle["operations"]["pdf_export"]["preflight_consumptions"] = [11, 11]
        with self.assertRaisesRegex(collector.EvidenceError, "exactly once"):
            collector.validate_operation_binding(bundle["operations"], "x11")

    def test_backend_bundle_binds_exact_binary_host_and_operator_print(self) -> None:
        path, value = self._run("wayland")
        accepted = collector.load_backend_bundle(path, "wayland", self.binding)
        self.assertEqual(accepted["run"]["host_strategy"], "GtkTopLevel")
        value["operator"]["collector_submitted_print"] = True
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "submitted only by the operator"):
            collector.load_backend_bundle(path, "wayland", self.binding)

    def test_backend_bundle_rejects_candidate_or_binary_substitution(self) -> None:
        path, value = self._run("x11")
        value["candidate"]["renderer_bundle_sha256"] = "f" * 64
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "exact candidate"):
            collector.load_backend_bundle(path, "x11", self.binding)
        path, value = self._run("x11")
        value["run"]["launch_argv"][-1] = "/tmp/other/bir"
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "exact binary"):
            collector.load_backend_bundle(path, "x11", self.binding)

    def test_rollback_bundle_must_preexist_and_match_both_runs(self) -> None:
        raw, paths = self._run_bundles()
        loaded = {
            backend: collector.load_backend_bundle(paths[backend], backend, self.binding)
            for backend in collector.certification.BACKENDS
        }
        path, value = self._rollback_bundle(raw)
        collector.load_rollback_bundle(path, self.binding, loaded)
        value["backends"]["x11"]["rollback"]["temporary_files_remaining"] = 1
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "differ"):
            collector.load_rollback_bundle(path, self.binding, loaded)

    def test_offline_bundle_rejects_node_typst_or_network_drift(self) -> None:
        raw, paths = self._run_bundles()
        loaded = {
            backend: collector.load_backend_bundle(paths[backend], backend, self.binding)
            for backend in collector.certification.BACKENDS
        }
        path, value = self._offline_bundle(raw)
        collector.load_offline_bundle(path, self.binding, loaded)
        value["offline_package"]["typst_present"] = True
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "incomplete"):
            collector.load_offline_bundle(path, self.binding, loaded)
        path, value = self._offline_bundle(raw)
        value["network_denial"]["wayland"]["candidate_namespace_inode"] = "net:[999]"
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "differs"):
            collector.load_offline_bundle(path, self.binding, loaded)

    @mock.patch.object(collector.platform, "system", return_value="Darwin")
    def test_collection_is_linux_only(self, _system: mock.Mock) -> None:
        with self.assertRaisesRegex(collector.EvidenceError, "must run on Linux"):
            collector.collect(argparse.Namespace())

    def test_completed_job_verifier_never_submits_a_print(self) -> None:
        completed = mock.Mock(
            returncode=0,
            stdout="Certification_x11-42 operator 1024 2026-07-19\n",
            stderr="",
        )
        with mock.patch.object(collector.subprocess, "run", return_value=completed) as run:
            observed = collector.completed_job_exists(
                "Certification_x11", "Certification_x11-42"
            )
        self.assertEqual(observed["job_id"], "Certification_x11-42")
        self.assertEqual(
            run.call_args.args[0],
            ["lpstat", "-W", "completed", "-o", "Certification_x11"],
        )

    @mock.patch.object(collector.platform, "system", return_value="Linux")
    def test_collection_invokes_strict_verifier_and_stays_non_promotional(
        self, _system: mock.Mock
    ) -> None:
        raw, paths = self._run_bundles()
        rollback_path, _ = self._rollback_bundle(raw)
        offline_path, _ = self._offline_bundle(raw)
        output = self.root / "collector-output"
        verifier = self.root / "verify_certification_pdf"
        verifier.write_text("#!/bin/sh\nexit 0\n")
        verifier.chmod(0o755)
        arguments = argparse.Namespace(
            candidate_manifest=self.manifest,
            candidate_archive=self.archive,
            renderer_identity=self.identity,
            pdf_verifier=verifier,
            x11_run_bundle=paths["x11"],
            wayland_run_bundle=paths["wayland"],
            rollback_bundle=rollback_path,
            offline_bundle=offline_path,
            output_dir=output,
            operator_identity="external Linux operator",
            allow_live_print_evidence=True,
        )

        def strict_verifier(
            manifest: Path,
            archive: Path,
            identity: Path,
            attestation: Path,
            pdf_verifier: Path,
            report: Path,
        ) -> None:
            self.assertEqual(manifest, self.manifest)
            self.assertEqual(archive, self.archive)
            self.assertEqual(identity, self.identity)
            self.assertEqual(pdf_verifier, verifier)
            retained = json.loads(attestation.read_text())
            self.assertFalse(retained["promotion_eligible"])
            self.assertFalse(retained["trusted_producer"])
            collector.write_json(
                report,
                {
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "promotion_satisfied": False,
                },
            )

        with (
            mock.patch.object(
                collector,
                "completed_job_exists",
                side_effect=lambda printer, job: {
                    "printer": printer,
                    "job_id": job,
                    "output_sha256": "f" * 64,
                },
            ),
            mock.patch.object(collector.certification, "validate_attestation"),
            mock.patch.object(
                collector.certification,
                "verify_attestation_command",
                side_effect=strict_verifier,
            ) as verify,
        ):
            report = collector.collect(arguments)
        self.assertEqual(report, output / "linux-candidate-certification-report.json")
        verify.assert_called_once()
        attestation = json.loads((output / "linux-candidate-attestation.json").read_text())
        self.assertEqual(attestation["display_runs"]["x11"]["host_strategy"], "GpuiWryChild")
        self.assertEqual(attestation["display_runs"]["wayland"]["host_strategy"], "GtkTopLevel")
        self.assertNotIn("release_ready", json.dumps(attestation))


if __name__ == "__main__":
    unittest.main()
