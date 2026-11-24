"""Release-version manifest rewriting."""

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

_PATH = Path(__file__).resolve().parents[1] / "set_release_version.py"
_SPEC = importlib.util.spec_from_file_location("set_release_version", _PATH)
assert _SPEC is not None and _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)

refresh_lockfiles = _MODULE.refresh_lockfiles
set_version = _MODULE.set_version
update_version = _MODULE.update_version


class SetReleaseVersionTests(unittest.TestCase):
    def write_tree(self, root: Path, nsh_json: str | None = None) -> None:
        (root / "crates/bir-print").mkdir(parents=True)
        (root / "crates/bir-core").mkdir(parents=True)
        (root / "nsh").mkdir()
        (root / "Cargo.toml").write_text(
            '[workspace.package]\nversion = "0.2.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        (root / "crates/bir-core/Cargo.toml").write_text(
            '[dependencies]\nbir-rules = { path = "../bir-rules", version = "0.2.0" }\n',
            encoding="utf-8",
        )
        (root / "crates/bir-print/Cargo.toml").write_text(
            '[package]\nname = "bir-print"\nversion = "0.2.0"\n'
            '[dependencies]\nbir-core = { path = "../bir-core", version = "0.2.0" }\n',
            encoding="utf-8",
        )
        (root / "nsh/Cargo.toml").write_text(
            '[package]\nname = "nsh-app-ebirforms"\nversion = "0.2.0"\n',
            encoding="utf-8",
        )
        (root / "nsh/nsh.json").write_text(
            nsh_json
            if nsh_json is not None
            else '{\n  "name": "eBIRForms",\n  "version": "0.2.0",\n  "build": 27\n}\n',
            encoding="utf-8",
        )

    def test_updates_every_manifest_without_reformatting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            set_version(root, "1.2.3")
            self.assertEqual(
                (root / "Cargo.toml").read_text(encoding="utf-8"),
                '[workspace.package]\nversion = "1.2.3"\nedition = "2024"\n',
            )
            self.assertEqual(
                (root / "crates/bir-core/Cargo.toml").read_text(encoding="utf-8"),
                '[dependencies]\nbir-rules = { path = "../bir-rules", version = "1.2.3" }\n',
            )
            self.assertEqual(
                (root / "crates/bir-print/Cargo.toml").read_text(encoding="utf-8"),
                '[package]\nname = "bir-print"\nversion = "1.2.3"\n'
                '[dependencies]\nbir-core = { path = "../bir-core", version = "1.2.3" }\n',
            )
            self.assertEqual(
                (root / "nsh/Cargo.toml").read_text(encoding="utf-8"),
                '[package]\nname = "nsh-app-ebirforms"\nversion = "1.2.3"\n',
            )
            self.assertEqual(
                (root / "nsh/nsh.json").read_text(encoding="utf-8"),
                '{\n  "name": "eBIRForms",\n  "version": "1.2.3",\n  "build": 27\n}\n',
            )

    def test_refuses_a_non_semver_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            with self.assertRaises(ValueError):
                set_version(root, "1.2")

    def test_a_missing_nsh_version_writes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root, '{\n  "name": "eBIRForms"\n}\n')
            cargo_before = (root / "Cargo.toml").read_bytes()
            with self.assertRaises(ValueError):
                set_version(root, "1.2.3")
            self.assertEqual((root / "Cargo.toml").read_bytes(), cargo_before)

    def test_a_missing_bir_print_manifest_writes_nothing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            (root / "crates/bir-print/Cargo.toml").unlink()
            cargo_before = (root / "Cargo.toml").read_bytes()
            with self.assertRaises(FileNotFoundError):
                set_version(root, "1.2.3")
            self.assertEqual((root / "Cargo.toml").read_bytes(), cargo_before)

    def test_lockfiles_refresh_offline_for_each_workspace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.return_value = subprocess.CompletedProcess([], 0)
                refresh_lockfiles(root)
            self.assertEqual(run.call_count, 2)
            commands = [call.args[0] for call in run.call_args_list]
            self.assertTrue(all("--offline" in command for command in commands))
            manifests = [
                Path(command[command.index("--manifest-path") + 1]).relative_to(root).as_posix()
                for command in commands
            ]
            self.assertEqual(
                manifests,
                ["Cargo.toml", "nsh/Cargo.toml"],
            )

    def test_lockfile_refresh_falls_back_online(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.side_effect = [
                    subprocess.CompletedProcess([], 1, stderr="missing offline cache"),
                    subprocess.CompletedProcess([], 0),
                    subprocess.CompletedProcess([], 0),
                ]
                refresh_lockfiles(root)
            commands = [call.args[0] for call in run.call_args_list]
            self.assertIn("--offline", commands[0])
            self.assertNotIn("--offline", commands[1])
            self.assertIn("--offline", commands[2])

    def test_lockfile_refresh_reports_cargo_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.return_value = subprocess.CompletedProcess([], 1, stderr="cargo broke")
                with self.assertRaisesRegex(RuntimeError, "cargo broke"):
                    refresh_lockfiles(root)
            self.assertEqual(run.call_count, 2)

    def test_a_lockfile_failure_rolls_everything_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            (root / "Cargo.lock").write_bytes(b"root lock")
            (root / "nsh/Cargo.lock").write_bytes(b"nsh lock")
            tracked = [
                root / "Cargo.toml",
                root / "Cargo.lock",
                root / "crates/bir-core/Cargo.toml",
                root / "crates/bir-print/Cargo.toml",
                root / "nsh/Cargo.toml",
                root / "nsh/Cargo.lock",
                root / "nsh/nsh.json",
            ]
            before = {path: path.read_bytes() for path in tracked}

            def fail_on_nsh(command, **_kwargs):
                manifest = Path(command[command.index("--manifest-path") + 1]).as_posix()
                if manifest.endswith("/nsh/Cargo.toml"):
                    (root / "Cargo.lock").write_bytes(b"corrupt root lock")
                    (root / "nsh/Cargo.lock").write_bytes(b"corrupt nsh lock")
                    return subprocess.CompletedProcess([], 1, stderr="missing ../nsh")
                return subprocess.CompletedProcess([], 0)

            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.side_effect = fail_on_nsh
                with self.assertRaises(RuntimeError) as failure:
                    update_version(root, "1.2.3")
            self.assertIn("../nsh checked out", str(failure.exception))
            for path, content in before.items():
                self.assertEqual(path.read_bytes(), content, str(path))

    def test_the_rules_manifest_is_regenerated_and_checked_after_the_lockfiles(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            (root / "crates/bir-rules-codegen").mkdir(parents=True)
            (root / "crates/bir-rules-codegen/Cargo.toml").write_text("[package]\n", encoding="utf-8")
            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.return_value = subprocess.CompletedProcess([], 0)
                update_version(root, "1.2.3")
            commands = [call.args[0] for call in run.call_args_list]
            self.assertEqual(
                commands[-2:],
                [
                    ["cargo", "run", "--locked", "-p", "bir-rules-codegen", "--", "generate"],
                    ["cargo", "run", "--locked", "-p", "bir-rules-codegen", "--", "check"],
                ],
            )
            self.assertTrue(all(command[1] == "metadata" for command in commands[:-2]))

    def test_a_tree_without_the_rules_generator_runs_no_generator(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.return_value = subprocess.CompletedProcess([], 0)
                update_version(root, "1.2.3")
            self.assertTrue(all(call.args[0][1] == "metadata" for call in run.call_args_list))

    def test_a_failed_rules_check_rolls_everything_back(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            (root / "crates/bir-rules-codegen").mkdir(parents=True)
            (root / "crates/bir-rules-codegen/Cargo.toml").write_text("[package]\n", encoding="utf-8")
            manifest = root / "crates/bir-rules/src/generated/manifest.json"
            manifest.parent.mkdir(parents=True)
            manifest.write_bytes(b'{"generator":{"version":"0.2.0"}}')
            tracked = [root / "Cargo.toml", root / "nsh/nsh.json", manifest]
            before = {path: path.read_bytes() for path in tracked}

            def fail_on_check(command, **_kwargs):
                if command[:2] == ["cargo", "run"] and command[-1] == "generate":
                    manifest.write_bytes(b'{"generator":{"version":"1.2.3"}}')
                if command[:2] == ["cargo", "run"] and command[-1] == "check":
                    return subprocess.CompletedProcess([], 1, stderr="noise\nrules check failed: digest")
                return subprocess.CompletedProcess([], 0)

            with mock.patch.object(_MODULE.subprocess, "run") as run:
                run.side_effect = fail_on_check
                with self.assertRaises(RuntimeError) as failure:
                    update_version(root, "1.2.3")
            self.assertIn("bir-rules-codegen check failed: rules check failed: digest", str(failure.exception))
            for path, content in before.items():
                self.assertEqual(path.read_bytes(), content, str(path))

    def test_tests_that_name_the_old_version_are_listed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.write_tree(root)
            (root / "nsh/shell").mkdir()
            (root / "nsh/shell/about.test.tsx").write_text(
                'expect(texts).toContain("Version 0.2.0");\nexpect(n).toBe(10.2.0.1);\n', encoding="utf-8"
            )
            (root / "nsh/shell/other.test.ts").write_text('expect(v).toBe("1.0.0");\n', encoding="utf-8")
            self.assertEqual(_MODULE.current_version(root), "0.2.0")
            self.assertEqual(_MODULE.tests_naming(root, "0.2.0"), ["nsh/shell/about.test.tsx:1"])


if __name__ == "__main__":
    unittest.main()
