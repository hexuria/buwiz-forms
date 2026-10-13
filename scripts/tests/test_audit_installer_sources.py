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

    def test_reordered_source_outside_stage_is_rejected(self) -> None:
        script = self.add_script(
            'DestDir: "{app}"; Source: "{#StageDir}\\..\\outside"; Flags: ignoreversion'
        )
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_unquoted_source_is_rejected(self) -> None:
        script = self.add_script("Source: {#StageDir}\\bir.exe; DestDir: {app}")
        with self.assertRaises(InstallerSourceError):
            validate_installer_sources(script, self.root)

    def test_traversal_is_rejected(self) -> None:
        script = self.add_script(
            'Source: "{#StageDir}/assets/../bir.exe"; DestDir: "{app}"'
        )
        with self.assertRaises(InstallerSourceError):
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


if __name__ == "__main__":
    unittest.main()
