from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.audit_installer_sources import (
    InstallerSourceError,
    validate_installer_sources,
)


class InstallerSourcePolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary_directory = tempfile.TemporaryDirectory()
        self.addCleanup(temporary_directory.cleanup)
        self.root = Path(temporary_directory.name) / "stage"
        self.root.mkdir()
        (self.root / "assets").mkdir()
        (self.root / "bir.exe").write_bytes(b"exe")
        (self.root / "assets" / "icon.ico").write_bytes(b"icon")

    def add_script(self, files: str) -> Path:
        script = self.root / "installer.iss"
        script.write_text(f"[Files]\n{files}\n", encoding="utf-8")
        return script

    def check_valid(self, files: str) -> None:
        self.assertEqual(
            validate_installer_sources(self.add_script(files), self.root),
            2,
        )

    def test_legitimate_entries(self) -> None:
        self.check_valid(
            'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}"; Flags: ignoreversion\n'
            'Source: "{#StageDir}\\assets\\*"; DestDir: "{app}\\assets"; '
            "Flags: ignoreversion recursesubdirs createallsubdirs"
        )

    def test_reordered_source_is_valid(self) -> None:
        self.check_valid(
            'DestDir: "{app}"; Source: "{#StageDir}\\bir.exe"; Flags: ignoreversion\n'
            'Flags: ignoreversion; DestDir: "{app}\\assets"; '
            'Source: "{#StageDir}\\assets\\*"'
        )

    def test_reordered_source_traversal_is_rejected(self) -> None:
        script = self.add_script(
            'DestDir: "{app}"; Source: "{#StageDir}\\..\\outside"; Flags: ignoreversion'
        )
        with self.assertRaisesRegex(InstallerSourceError, "traversal is not allowed"):
            validate_installer_sources(script, self.root)

    def test_unquoted_source_is_rejected(self) -> None:
        script = self.add_script("Source: {#StageDir}\\bir.exe; DestDir: {app}")
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_traversal_is_rejected(self) -> None:
        for relative in (r"assets\..\bir.exe", "assets/../bir.exe"):
            with self.subTest(relative=relative):
                script = self.add_script(
                    f'Source: "{{#StageDir}}\\{relative}"; DestDir: "{{app}}"'
                )
                with self.assertRaisesRegex(
                    InstallerSourceError, "traversal is not allowed"
                ):
                    validate_installer_sources(script, self.root)

    def test_include_is_rejected(self) -> None:
        script = self.root / "installer.iss"
        script.write_text('#include "other.iss"\n[Files]\n', encoding="utf-8")
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_malformed_entry_is_rejected(self) -> None:
        script = self.add_script('Source "{#StageDir}\\bir.exe"')
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_symlink_escape_is_rejected_when_supported(self) -> None:
        outside = self.root.parent / "outside-installer-source"
        outside.write_bytes(b"outside")
        link = self.root / "escaped.exe"
        try:
            link.symlink_to(outside)
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation is not available")
        script = self.add_script(
            'Source: "{#StageDir}\\escaped.exe"; DestDir: "{app}"'
        )
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_nested_symlink_is_rejected_for_recursive_source(self) -> None:
        outside = self.root.parent / "outside-installer-assets"
        outside.mkdir()
        nested = self.root / "assets" / "nested"
        nested.mkdir()
        link = nested / "external"
        try:
            link.symlink_to(outside, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("directory symlink creation is not available")
        script = self.add_script(
            'Source: "{#StageDir}\\assets\\*"; DestDir: "{app}\\assets"; '
            "Flags: ignoreversion recursesubdirs createallsubdirs"
        )
        with self.assertRaisesRegex(
            InstallerSourceError, "stage contains a symlink or reparse point"
        ):
            validate_installer_sources(script, self.root)

    def test_unsafe_destdir_flags_and_duplicates_are_rejected(self) -> None:
        cases = (
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}\\..\\outside"',
                "DestDir traversal is not allowed",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "C:\\outside"',
                "DestDir must be anchored at",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}\\{#OtherDir}"',
                "unsupported DestDir path",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}"; '
                "Flags: ignoreversion external",
                "unsupported Flags value external",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}"; '
                'DestDir: "{app}\\assets"',
                "exactly one Source and DestDir",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; '
                'Source: "{#StageDir}\\assets\\icon.ico"; DestDir: "{app}"',
                "exactly one Source and DestDir",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: {app}',
                "DestDir must be quoted",
            ),
            (
                'Source: "{#StageDir}\\bir.exe"; DestDir: "{app}"; '
                "Flags: ignoreversion; Flags: recursesubdirs",
                "duplicate Flags directive",
            ),
        )
        for files, message in cases:
            with self.subTest(files=files):
                script = self.add_script(files)
                with self.assertRaisesRegex(InstallerSourceError, message):
                    validate_installer_sources(script, self.root)


if __name__ == "__main__":
    unittest.main()
