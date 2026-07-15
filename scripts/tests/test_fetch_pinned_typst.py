from __future__ import annotations

import hashlib
import importlib.util
import io
import stat
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "fetch_pinned_typst.py"
SPEC = importlib.util.spec_from_file_location("fetch_pinned_typst", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
fetcher = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = fetcher
SPEC.loader.exec_module(fetcher)


class FetchPinnedTypstTests(unittest.TestCase):
    def test_release_targets_are_versioned_and_checksum_pinned(self) -> None:
        self.assertEqual(fetcher.TYPST_VERSION, "0.13.1")
        self.assertEqual(
            set(fetcher.PINNED_ARCHIVES),
            {
                "aarch64-apple-darwin",
                "x86_64-apple-darwin",
                "x86_64-pc-windows-msvc",
                "x86_64-unknown-linux-musl",
            },
        )
        for target, archive in fetcher.PINNED_ARCHIVES.items():
            with self.subTest(target=target):
                self.assertIn("/releases/download/v0.13.1/", archive.url)
                self.assertNotIn("/latest/", archive.url)
                self.assertRegex(archive.sha256, r"^[0-9a-f]{64}$")

    def test_checksum_mismatch_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            archive = Path(temporary_directory) / "typst.zip"
            archive.write_bytes(b"unexpected")
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                fetcher.verify_archive(archive, "0" * 64)

    def test_zip_install_extracts_only_expected_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            archive_path = root / "typst-test.zip"
            member = "typst-test/typst.exe"
            with zipfile.ZipFile(archive_path, mode="w") as archive:
                archive.writestr(member, b"windows-binary")
                archive.writestr("../../outside", b"must-not-be-extracted")
            spec = self.spec_for(archive_path, member)
            output = root / "installed" / "typst.exe"

            fetcher.install_binary(archive_path, spec, output)

            self.assertEqual(output.read_bytes(), b"windows-binary")
            self.assertFalse((root / "outside").exists())

    def test_tar_install_preserves_executable_intent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            archive_path = root / "typst-test.tar.xz"
            member = "typst-test/typst"
            payload = b"mac-binary"
            with tarfile.open(archive_path, mode="w:xz") as archive:
                info = tarfile.TarInfo(member)
                info.size = len(payload)
                info.mode = 0o755
                archive.addfile(info, io.BytesIO(payload))
            spec = self.spec_for(archive_path, member)
            output = root / "installed" / "typst"

            fetcher.install_binary(archive_path, spec, output)

            self.assertEqual(output.read_bytes(), payload)
            self.assertTrue(output.stat().st_mode & stat.S_IXUSR)

    @staticmethod
    def spec_for(archive_path: Path, member: str):
        return fetcher.PinnedArchive(
            filename=archive_path.name,
            sha256=hashlib.sha256(archive_path.read_bytes()).hexdigest(),
            member=member,
        )


if __name__ == "__main__":
    unittest.main()
