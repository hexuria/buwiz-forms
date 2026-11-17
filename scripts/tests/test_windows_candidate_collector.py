from __future__ import annotations

import argparse
from contextlib import ExitStack
import importlib.util
import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts/windows_candidate_collector.py"
SPEC = importlib.util.spec_from_file_location("windows_candidate_collector", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
collector = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(collector)


class WindowsCandidateCollectorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name).resolve()
        self.source_revision = "a" * 40
        self.package = self.root / "source"
        renderer = self.package / collector.certification.RENDERER_RELATIVE_PATH
        renderer.mkdir(parents=True)
        (renderer / "index.html").write_text("<!doctype html><title>forms</title>\n")
        binary = self.package / collector.certification.BINARY_RELATIVE_PATH
        self._pe(binary, 0x8664)

        self.renderer_hash = collector.artifact_common.tree_hash(renderer)
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
        bundled_identity = self.package / collector.certification.IDENTITY_RELATIVE_PATH
        bundled_identity.parent.mkdir(parents=True, exist_ok=True)
        bundled_identity.write_bytes(encoded_identity)

        self.archive = self.root / "eBIRForms-Windows-x64-candidate.zip"
        with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_DEFLATED) as bundle:
            for path in sorted(item for item in self.package.rglob("*") if item.is_file()):
                bundle.write(path, path.relative_to(self.package).as_posix())
        self.manifest = self.root / "candidate-manifest.json"
        archive_record = self._record(self.archive)
        identity_record = self._record(self.identity)
        self.manifest.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "scope": collector.certification.certification_common.CANDIDATE_SCOPE,
                    "promotion_eligible": False,
                    "trusted_producer": False,
                    "form": collector.FORM,
                    "source_revision": self.source_revision,
                    "platform": "windows",
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
        return collector.file_record(path)

    def _rollback_bundle(self) -> tuple[Path, dict]:
        destination_before = self.evidence / "destination-before.bin"
        destination_after = self.evidence / "destination-after.bin"
        draft_before = self.evidence / "draft-before.json"
        draft_after = self.evidence / "draft-after.json"
        destination_before.write_bytes(b"preserved destination\n")
        destination_after.write_bytes(b"preserved destination\n")
        draft_before.write_text('{"unchanged":true}\n')
        draft_after.write_text('{"unchanged":true}\n')
        temporary = self.evidence / "temporary-files.json"
        temporary.write_text('{"remaining":[]}\n')
        cases = []
        for name in sorted(collector.certification.ROLLBACK_CASES):
            artifact = self.evidence / f"rollback-{name}.json"
            artifact.write_text(json.dumps({"name": name, "passed": True}) + "\n")
            cases.append({"name": name, "passed": True, "artifact": self._record(artifact)})
        value = {
            "schema_version": 1,
            "scope": collector.ROLLBACK_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "candidate": collector.expected_candidate(self.binding),
            "integrity": {
                "destination_before": self._record(destination_before),
                "destination_after": self._record(destination_after),
                "draft_before": self._record(draft_before),
                "draft_after": self._record(draft_after),
                "temporary_files_manifest": self._record(temporary),
            },
            "cases": cases,
            "strict_verifier_gaps": [
                "rollback producer is external and is not registered as trusted"
            ],
        }
        path = self.evidence / "rollback-bundle.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    def _geometry(self) -> list[dict]:
        pages = [
            {
                "page": page,
                "x": 0.0,
                "y": float((page - 1) * 936),
                "width_pt": 612.0,
                "height_pt": 936.0,
            }
            for page in (1, 2)
        ]
        return [
            {
                "measurement_index": index,
                "page_width_pt": 612.0,
                "page_height_pt": 936.0,
                "pages": pages,
                "clipping_count": 0,
                "overflow_count": 0,
            }
            for index in (1, 2)
        ]

    def _runtime_observation(
        self,
        *,
        output_pdf: Path,
        witness: Path,
        webview2: Path,
        challenge_sha256: str = "b" * 64,
        destination_before_sha256: str = "c" * 64,
        pid: int = 731,
        printer: str = "eBIRForms Certification Printer",
    ) -> tuple[Path, dict]:
        output = self._record(output_pdf)
        value = {
            "schema_version": 1,
            "scope": collector.RUNTIME_SCOPE,
            "promotion_eligible": False,
            "trusted_producer": False,
            "collector_challenge_sha256": challenge_sha256,
            "witness_name": "reviewed external Windows runtime witness",
            "witness_version": "1",
            "witness_executable_sha256": self._record(witness)["sha256"],
            "candidate": collector.expected_candidate(self.binding),
            "pid": pid,
            "form": collector.FORM,
            "non_dev_build": True,
            "dev_tools_enabled": False,
            "started_at_utc": "2026-07-19T00:00:00.000Z",
            "completed_at_utc": "2026-07-19T00:01:00.000Z",
            "document_run_id": "2551q-run-1",
            "envelope_sha256": "d" * 64,
            "preview_nonce": 17,
            "print_nonce": 18,
            "geometry_measurements": self._geometry(),
            "export": {
                "nonce": 17,
                "print_to_pdf_hresult": "S_OK",
                "print_to_pdf_result": True,
                "destination_before_sha256": destination_before_sha256,
                "output_pdf_sha256": output["sha256"],
                "output_pdf_byte_count": output["byte_count"],
                "temporary_file_remaining": False,
            },
            "print": {
                "nonce": 18,
                "webview2_print_hresult": "S_OK",
                "webview2_print_status": "Succeeded",
                "printer_name": printer,
            },
            "webview2": {
                "runtime_version": "138.0.3351.83",
                "channel": "stable",
                "architecture": "x86_64",
                "install_scope": "per_machine",
                "executable_sha256": self._record(webview2)["sha256"],
                "core_webview2_7_available": True,
                "core_webview2_16_available": True,
            },
            "dependencies": {
                "msvc_runtime_loaded": True,
                "webview2_loader_bound": True,
            },
            "strict_verifier_gaps": collector.RUNTIME_GAPS,
        }
        path = self.evidence / "runtime-observation.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
        return path, value

    @staticmethod
    def _element(pid: int, name: str, control_type: str, runtime_id: int) -> dict:
        return {
            "processId": pid,
            "name": name,
            "automationId": f"automation-{runtime_id}",
            "className": "WindowClass",
            "controlType": control_type,
            "runtimeId": [42, runtime_id],
            "bounds": {"x": 10.0, "y": 20.0, "width": 200.0, "height": 40.0},
        }

    def _export_automation(
        self, pid: int, destination: Path, preview: Path, chooser: Path
    ) -> dict:
        return {
            "preview": self._element(pid, "2551Q HTML Form Preview", "ControlType.Window", 1),
            "exportControl": self._element(pid, "Export PDF", "ControlType.Button", 2),
            "saveChooser": self._element(pid, "Save As", "ControlType.Window", 3),
            "fileNameControl": self._element(pid, "File name", "ControlType.Edit", 4),
            "saveControl": self._element(pid, "Save", "ControlType.Button", 5),
            "replaceControl": self._element(pid, "Yes", "ControlType.Button", 7),
            "replaceConfirmation": True,
            "invoked": True,
            "destination": str(destination.resolve()),
            "previewScreenshot": str(preview.resolve()),
            "chooserScreenshot": str(chooser.resolve()),
        }

    @staticmethod
    def _pe(path: Path, machine: int) -> None:
        payload = bytearray(512)
        payload[0:2] = b"MZ"
        payload[0x3C:0x40] = (0x80).to_bytes(4, "little")
        payload[0x80:0x84] = b"PE\0\0"
        payload[0x84:0x86] = machine.to_bytes(2, "little")
        path.write_bytes(payload)

    def test_complete_rollback_bundle_is_exact_candidate_bound(self) -> None:
        path, _ = self._rollback_bundle()
        loaded = collector.load_rollback_bundle(path, self.binding)
        self.assertEqual(len(loaded["cases"]), 24)
        self.assertEqual(
            loaded["integrity"]["destination_before"]["sha256"],
            loaded["integrity"]["destination_after"]["sha256"],
        )

    def test_rollback_missing_case_candidate_drift_and_temp_leak_fail_closed(self) -> None:
        path, value = self._rollback_bundle()
        value["cases"].pop()
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "incomplete"):
            collector.load_rollback_bundle(path, self.binding)

        path, value = self._rollback_bundle()
        value["candidate"]["binary_sha256"] = "f" * 64
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "exact Windows candidate"):
            collector.load_rollback_bundle(path, self.binding)

        path, value = self._rollback_bundle()
        temporary = Path(value["integrity"]["temporary_files_manifest"]["path"])
        temporary.write_text('{"remaining":["leaked.partial"]}\n')
        value["integrity"]["temporary_files_manifest"] = self._record(temporary)
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "leaked files"):
            collector.load_rollback_bundle(path, self.binding)

    def test_runtime_observation_binds_candidate_process_witness_pdf_and_interfaces(self) -> None:
        output = self.evidence / "2551q.pdf"
        output.write_bytes(b"two-page PDF placeholder")
        witness = self.evidence / "witness.exe"
        witness.write_bytes(b"reviewed witness")
        webview2 = self.evidence / "msedgewebview2.exe"
        webview2.write_bytes(b"reviewed runtime")
        path, _ = self._runtime_observation(
            output_pdf=output, witness=witness, webview2=webview2
        )
        accepted = collector.validate_runtime_observation(
            path,
            binding=self.binding,
            challenge_sha256="b" * 64,
            pid=731,
            output_pdf=output,
            destination_before_sha256="c" * 64,
            webview2_executable=webview2,
            printer_name="eBIRForms Certification Printer",
            witness_executable=witness,
        )
        self.assertEqual(accepted["print_nonce"], 18)

    def test_runtime_observation_rejects_drift_and_nonfinite_geometry(self) -> None:
        output = self.evidence / "2551q.pdf"
        output.write_bytes(b"two-page PDF placeholder")
        witness = self.evidence / "witness.exe"
        witness.write_bytes(b"reviewed witness")
        webview2 = self.evidence / "msedgewebview2.exe"
        webview2.write_bytes(b"reviewed runtime")
        path, value = self._runtime_observation(
            output_pdf=output, witness=witness, webview2=webview2
        )

        value["candidate"]["renderer_bundle_sha256"] = "f" * 64
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "challenge, PID, or candidate"):
            collector.validate_runtime_observation(
                path,
                binding=self.binding,
                challenge_sha256="b" * 64,
                pid=731,
                output_pdf=output,
                destination_before_sha256="c" * 64,
                webview2_executable=webview2,
                printer_name="eBIRForms Certification Printer",
                witness_executable=witness,
            )

        path, value = self._runtime_observation(
            output_pdf=output, witness=witness, webview2=webview2
        )
        value["geometry_measurements"][0]["pages"][0]["width_pt"] = float("inf")
        value["geometry_measurements"][1]["pages"][0]["width_pt"] = float("inf")
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "finite"):
            collector.validate_runtime_observation(
                path,
                binding=self.binding,
                challenge_sha256="b" * 64,
                pid=731,
                output_pdf=output,
                destination_before_sha256="c" * 64,
                webview2_executable=webview2,
                printer_name="eBIRForms Certification Printer",
                witness_executable=witness,
            )

        path, value = self._runtime_observation(
            output_pdf=output, witness=witness, webview2=webview2
        )
        value["webview2"]["core_webview2_16_available"] = False
        path.write_text(json.dumps(value))
        with self.assertRaisesRegex(collector.EvidenceError, "WebView2 evidence"):
            collector.validate_runtime_observation(
                path,
                binding=self.binding,
                challenge_sha256="b" * 64,
                pid=731,
                output_pdf=output,
                destination_before_sha256="c" * 64,
                webview2_executable=webview2,
                printer_name="eBIRForms Certification Printer",
                witness_executable=witness,
            )

    def test_export_and_print_automation_are_exact_process_and_native_dialog_bound(self) -> None:
        pid = 731
        destination = self.evidence / "2551q.pdf"
        destination.write_bytes(b"pdf")
        preview = self.evidence / "preview.png"
        chooser = self.evidence / "chooser.png"
        preview.write_bytes(b"PNG preview")
        chooser.write_bytes(b"PNG chooser")
        value = self._export_automation(pid, destination, preview, chooser)
        collector.validate_export_automation(
            value,
            pid=pid,
            destination=destination,
            preview_screenshot=preview,
            chooser_screenshot=chooser,
        )
        print_value = {
            "preview": value["preview"],
            "printControl": self._element(pid, "Print", "ControlType.Button", 6),
            "invoked": True,
        }
        collector.validate_print_automation(print_value, pid=pid)

        value["saveChooser"]["processId"] = pid + 1
        with self.assertRaisesRegex(collector.EvidenceError, "exact candidate process"):
            collector.validate_export_automation(
                value,
                pid=pid,
                destination=destination,
                preview_screenshot=preview,
                chooser_screenshot=chooser,
            )

    def test_export_requires_overwrite_confirmation_and_exact_destination(self) -> None:
        pid = 731
        destination = self.evidence / "2551q.pdf"
        destination.write_bytes(b"pdf")
        preview = self.evidence / "preview.png"
        chooser = self.evidence / "chooser.png"
        preview.write_bytes(b"PNG preview")
        chooser.write_bytes(b"PNG chooser")
        value = self._export_automation(pid, destination, preview, chooser)
        value["replaceConfirmation"] = False
        with self.assertRaisesRegex(collector.EvidenceError, "did not replace"):
            collector.validate_export_automation(
                value,
                pid=pid,
                destination=destination,
                preview_screenshot=preview,
                chooser_screenshot=chooser,
            )
        value = self._export_automation(pid, destination, preview, chooser)
        value["destination"] = str(self.evidence / "substituted.pdf")
        with self.assertRaisesRegex(collector.EvidenceError, "path differs"):
            collector.validate_export_automation(
                value,
                pid=pid,
                destination=destination,
                preview_screenshot=preview,
                chooser_screenshot=chooser,
            )

    def test_completed_print_event_is_unique_named_two_page_job(self) -> None:
        printer = "eBIRForms Certification Printer"
        event = {
            "eventId": 307,
            "eventRecordId": 101,
            "completedAtUtc": "2026-07-19T00:01:00.000Z",
            "jobId": "42",
            "documentName": "2551Q HTML Form Preview",
            "printerName": printer,
            "totalPages": 2,
            "message": f"Printed 2551Q HTML Form Preview job 42 on {printer}",
        }
        accepted = collector.validate_completed_print_event(
            {"events": [event]},
            printer_name=printer,
            baseline_record_id=100,
            submitted_at_utc="2026-07-19T00:00:00.000Z",
        )
        self.assertEqual(accepted["eventRecordId"], 101)
        event["totalPages"] = 1
        with self.assertRaisesRegex(collector.EvidenceError, "exactly two pages"):
            collector.validate_completed_print_event(
                {"events": [event]},
                printer_name=printer,
                baseline_record_id=100,
                submitted_at_utc="2026-07-19T00:00:00.000Z",
            )

    def test_pe_architecture_check_rejects_non_x64(self) -> None:
        x64 = self.evidence / "x64.exe"
        self._pe(x64, 0x8664)
        collector.require_pe_x86_64(x64, "x64 test executable")
        x86 = self.evidence / "x86.exe"
        self._pe(x86, 0x014C)
        with self.assertRaisesRegex(collector.EvidenceError, "not x86-64"):
            collector.require_pe_x86_64(x86, "x86 test executable")

    def test_live_print_requires_explicit_consent_and_identity(self) -> None:
        arguments = argparse.Namespace(
            allow_live_print=False,
            printer="eBIRForms Certification Printer",
            automation_identity="DOMAIN\\operator",
            timeout=180.0,
        )
        with self.assertRaisesRegex(collector.EvidenceError, "--allow-live-print"):
            collector.validate_live_arguments(arguments)
        arguments.allow_live_print = True
        collector.validate_live_arguments(arguments)
        arguments.automation_identity = ""
        with self.assertRaisesRegex(collector.EvidenceError, "automation-identity"):
            collector.validate_live_arguments(arguments)

    @mock.patch.object(collector, "remove_firewall_rules")
    @mock.patch.object(
        collector,
        "run_powershell_json",
        side_effect=collector.EvidenceError("simulated partial Firewall failure"),
    )
    def test_firewall_creation_failure_attempts_exact_rule_cleanup(
        self, _run: mock.Mock, cleanup: mock.Mock
    ) -> None:
        binary = self.evidence / "bir.exe"
        webview2 = self.evidence / "msedgewebview2.exe"
        binary.write_bytes(b"binary")
        webview2.write_bytes(b"runtime")
        with self.assertRaisesRegex(collector.EvidenceError, "partial Firewall failure"):
            collector.add_firewall_rules(binary, webview2, self.evidence / "network.json")
        cleanup.assert_called_once()
        names = cleanup.call_args.args[0]
        self.assertEqual(len(names), 2)
        self.assertEqual(len(set(names)), 2)

    @mock.patch.object(collector.subprocess, "Popen")
    @mock.patch.object(collector.platform, "system", return_value="Darwin")
    def test_collection_is_windows_only_and_never_launches_elsewhere(
        self, _system: mock.Mock, popen: mock.Mock
    ) -> None:
        with self.assertRaisesRegex(collector.EvidenceError, "must run on Windows"):
            collector.collect(argparse.Namespace())
        popen.assert_not_called()

    def test_embedded_uia_scripts_are_exact_and_never_cancel_the_print_job(self) -> None:
        export = collector.EXPORT_UIA_POWERSHELL
        printing = collector.PRINT_UIA_POWERSHELL
        self.assertIn("AutomationElement]::ProcessIdProperty", export)
        self.assertIn("FindExactButton $preview 'Export PDF'", export)
        self.assertIn("$value.SetValue($destination)", export)
        self.assertIn("FindExactButton $window 'Yes'", export)
        self.assertIn("Export PDF did not replace the challenged destination", export)
        self.assertIn("FindExactButton $preview 'Print'", printing)
        self.assertIn("$invoke.Invoke()", printing)
        self.assertNotIn("FindExactButton $preview 'Cancel'", printing)
        self.assertNotIn("SendKeys", printing)

    def test_collection_assembles_closed_attestation_and_invokes_strict_verifier(self) -> None:
        printer_name = "eBIRForms Certification Printer"
        webview2 = self.root / "msedgewebview2.exe"
        self._pe(webview2, 0x8664)
        witness = self.root / "reviewed-runtime-witness.exe"
        witness.write_bytes(b"reviewed runtime witness")
        verifier = self.root / "verify_certification_pdf.exe"
        verifier.write_bytes(b"owned PDF verifier")
        signtool = self.root / "signtool.exe"
        signtool.write_bytes(b"Windows SDK signtool")
        rollback_path, _ = self._rollback_bundle()
        output = self.root / "collected"
        arguments = argparse.Namespace(
            candidate_manifest=self.manifest,
            candidate_archive=self.archive,
            renderer_identity=self.identity,
            pdf_verifier=verifier,
            signtool=signtool,
            webview2_executable=webview2,
            runtime_witness=witness,
            rollback_bundle=rollback_path,
            output_dir=output,
            printer=printer_name,
            printer_output=None,
            automation_identity="DOMAIN\\operator",
            timeout=30.0,
            allow_live_print=True,
        )

        def retain(path: Path, value: dict) -> dict:
            collector.write_json(path, value)
            return value

        def lock_down(_path: Path, artifact: Path) -> dict:
            return retain(
                artifact,
                {
                    "path": str(output),
                    "ownerSid": "S-1-5-21-1",
                    "currentSid": "S-1-5-21-1",
                    "inheritanceProtected": True,
                    "accessRuleCount": 1,
                    "currentIdentity": "DOMAIN\\operator",
                },
            )

        def host(path: Path) -> dict:
            return retain(
                path,
                {
                    "windowsEdition": "Windows 11 Pro",
                    "windowsBuild": "26100.4652",
                    "osArchitecture": "x86_64",
                    "processArchitecture": "x86_64",
                    "sessionId": 2,
                    "elevated": True,
                    "processIntegrityLevel": "High",
                    "uiAutomationAvailable": True,
                },
            )

        def signature(binary: Path, _signtool: Path, artifact: Path) -> dict:
            retain(artifact, {"powershell": "reviewed", "signtool": "verified"})
            return {
                "passed": True,
                "status": "Valid",
                "binary_sha256": self._record(binary)["sha256"],
                "signer_subject": "CN=Goldcoders",
                "signer_issuer": "CN=Reviewed CA",
                "signer_serial_number": "01",
                "signer_thumbprint": "A" * 40,
                "code_signing_eku": True,
                "file_digest_algorithm": "SHA256",
                "timestamp_signature_present": True,
                "timestamp_subject": "CN=Reviewed TSA",
                "timestamp_thumbprint": "B" * 40,
                "timestamp_time_utc": "2026-07-18T00:00:00.000Z",
                "timestamp_digest_algorithm": "SHA256",
                "chain_trusted": True,
                "signtool_policy": "/pa /all /v",
                "artifact": self._record(artifact),
            }

        def printer_state(_name: str, artifact: Path) -> dict:
            return retain(
                artifact,
                {
                    "name": printer_name,
                    "printerStatus": "Normal",
                    "workOffline": False,
                    "defaultPrinter": printer_name,
                    "operationalLogEnabled": True,
                    "baselineEventRecordId": 100,
                },
            )

        def firewall(_binary: Path, _webview2: Path, artifact: Path) -> tuple[list, dict]:
            names = ["eBIRForms test app deny", "eBIRForms test WebView2 deny"]
            creation = {
                "profiles": [{"name": "Domain", "enabled": True}],
                "rules": [{"name": name} for name in names],
            }
            retain(artifact, {"creation": creation, "cleanup": None})
            return names, creation

        def process_record(pid: int, binary: Path, artifact: Path) -> dict:
            return retain(
                artifact,
                {"pid": pid, "executable": str(binary.resolve()), "modules": []},
            )

        def webview_processes(pid: int, executable: Path, artifact: Path) -> dict:
            return retain(
                artifact,
                {
                    "rootPid": pid,
                    "processes": [
                        {
                            "pid": pid + 1,
                            "parentPid": pid,
                            "executable": str(executable.resolve()),
                            "commandLineSha256": "e" * 64,
                        }
                    ],
                },
            )

        def export_ui(**kwargs: object) -> dict:
            destination = Path(kwargs["destination"])
            preview = Path(kwargs["preview_screenshot"])
            chooser = Path(kwargs["chooser_screenshot"])
            destination.write_bytes(b"two-page PDF placeholder")
            preview.write_bytes(b"PNG preview")
            chooser.write_bytes(b"PNG chooser")
            value = self._export_automation(731, destination, preview, chooser)
            return collector.validate_export_automation(
                value,
                pid=731,
                destination=destination,
                preview_screenshot=preview,
                chooser_screenshot=chooser,
            )

        def runtime_observation(path: Path, **kwargs: object) -> dict:
            validation = kwargs["validation"]
            observation_path, value = self._runtime_observation(
                output_pdf=Path(validation["output_pdf"]),
                witness=Path(validation["witness_executable"]),
                webview2=Path(validation["webview2_executable"]),
                challenge_sha256=str(validation["challenge_sha256"]),
                destination_before_sha256=str(validation["destination_before_sha256"]),
                pid=int(validation["pid"]),
                printer=str(validation["printer_name"]),
            )
            if observation_path != path:
                path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
            return collector.validate_runtime_observation(path, **validation)

        def pdf_report(
            _verifier: Path, pdf: Path, envelope_sha256: str, artifact: Path
        ) -> dict:
            report = {
                "schema_version": 1,
                "scope": "owned_windows_candidate_pdf_validation",
                "promotion_eligible": False,
                "form": collector.FORM,
                "envelope_sha256": envelope_sha256,
                "output_sha256": self._record(pdf)["sha256"],
                "expected_page_count": 2,
                "actual_page_count": 2,
                "width_points": 612.0,
                "height_points": 936.0,
                "content_nonempty": True,
                "validated_by": "bir-print::html_output::validate_pdf_file",
                "pages": [
                    {
                        "page": page,
                        "media_width_pt": 612.0,
                        "media_height_pt": 936.0,
                        "crop_width_pt": 612.0,
                        "crop_height_pt": 936.0,
                        "rotation": 0,
                        "content_byte_count": page * 10,
                    }
                    for page in (1, 2)
                ],
            }
            retain(artifact, report)
            return report

        process = mock.Mock()
        process.pid = 731
        process.poll.return_value = None
        timestamps = iter(
            [
                "2026-07-19T00:00:01.000Z",
                "2026-07-19T00:00:02.000Z",
                "2026-07-19T00:00:03.000Z",
            ]
        )
        def write_strict_report(*_args: object) -> int:
            report = output / "windows-candidate-certification-report.json"
            return report.write_text('{"promotion_satisfied":false}\n')

        strict = mock.Mock(side_effect=write_strict_report)
        event = {
            "eventId": 307,
            "eventRecordId": 101,
            "completedAtUtc": "2026-07-19T00:00:02.500Z",
            "jobId": "42",
            "documentName": "2551Q HTML Form Preview",
            "printerName": printer_name,
            "totalPages": 2,
            "message": f"Printed 2551Q HTML Form Preview job 42 on {printer_name}",
        }

        patches = (
            mock.patch.object(collector.platform, "system", return_value="Windows"),
            mock.patch.object(collector, "lock_down_output_directory", side_effect=lock_down),
            mock.patch.object(collector, "collect_host_state", side_effect=host),
            mock.patch.object(collector, "collect_authenticode", side_effect=signature),
            mock.patch.object(
                collector,
                "webview2_file_version",
                return_value={
                    "path": str(webview2.resolve()),
                    "fileVersion": "138.0.3351.83",
                    "productVersion": "138.0.3351.83",
                },
            ),
            mock.patch.object(collector, "collect_printer_state", side_effect=printer_state),
            mock.patch.object(collector, "add_firewall_rules", side_effect=firewall),
            mock.patch.object(collector, "remove_firewall_rules", return_value={"remaining": []}),
            mock.patch.object(collector.subprocess, "Popen", return_value=process),
            mock.patch.object(collector, "terminate_process"),
            mock.patch.object(collector, "process_runtime_record", side_effect=process_record),
            mock.patch.object(
                collector, "webview2_descendant_record", side_effect=webview_processes
            ),
            mock.patch.object(collector, "run_export_automation", side_effect=export_ui),
            mock.patch.object(
                collector,
                "run_print_automation",
                return_value={
                    "preview": self._element(
                        731, "2551Q HTML Form Preview", "ControlType.Window", 1
                    ),
                    "printControl": self._element(731, "Print", "ControlType.Button", 6),
                    "invoked": True,
                },
            ),
            mock.patch.object(
                collector,
                "wait_for_completed_print_event",
                return_value=(event, {"events": [event]}),
            ),
            mock.patch.object(
                collector, "wait_for_runtime_observation", side_effect=runtime_observation
            ),
            mock.patch.object(collector, "run_pdf_verifier", side_effect=pdf_report),
            mock.patch.object(collector, "host_identifier_sha256", return_value="f" * 64),
            mock.patch.object(collector, "utc_now", side_effect=lambda: next(timestamps)),
            mock.patch.object(collector.time, "sleep"),
            mock.patch("builtins.print"),
            mock.patch("builtins.input", side_effect=["", printer_name]),
            mock.patch.object(
                collector.certification, "verify_attestation_command", strict
            ),
        )
        with ExitStack() as stack:
            for patcher in patches:
                stack.enter_context(patcher)
            report = collector.collect(arguments)

        self.assertTrue(report.is_file())
        strict.assert_called_once()
        attestation_path = output / "windows-candidate-attestation.json"
        attestation, _ = collector.certification.validate_attestation(
            attestation_path,
            collector.candidate_binding(
                self.manifest, self.archive, self.identity, self.root / "second-extraction"
            ),
        )
        self.assertFalse(attestation["promotion_eligible"])
        self.assertFalse(attestation["trusted_producer"])
        self.assertTrue(attestation["operator_only"])
        self.assertEqual(attestation["native_print"]["printer_name"], printer_name)


if __name__ == "__main__":
    unittest.main()
