import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
AUDIT = ROOT / 'scripts/audit_google_client.py'
# Shaped like Google's, so the detector's own patterns match them; neither is a real client.
PLANTED_ID = b'123456789012-plantedplantedplanted.apps.googleusercontent.com'
PLANTED_SECRET = b'GOCSPX-plantedplantedplanted'
NEIGHBOUR = b'NEIGHBOUR-BYTES-NEVER-PRINTED'


def run(*args):
    return subprocess.run([sys.executable, '-I', str(AUDIT), *args], capture_output=True, text=True, timeout=30)


def msix(folder: Path, name: str, exe: bytes) -> Path:
    path = folder / name
    with zipfile.ZipFile(path, 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('AppxManifest.xml', '<Package/>')
        archive.writestr('eBIRForms.exe', exe)
    return path


class AuditGoogleClientTests(unittest.TestCase):
    def test_the_self_test_finds_a_planted_id_and_secret_and_nothing_in_a_clean_file(self):
        result = run('--self-test')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('self-test passed', result.stdout)

    def test_absent_passes_a_clean_package_and_fails_one_with_either_kind(self):
        with tempfile.TemporaryDirectory() as scratch:
            scratch = Path(scratch)
            clean = msix(scratch, 'clean.msix', b'MZ' + b'\0' * 4096 + b'.apps.googleusercontent.com GOCSPX-')
            self.assertEqual(run('--absent', str(clean)).returncode, 0)
            for name, exe in (('id', PLANTED_ID), ('secret', PLANTED_SECRET), ('both', PLANTED_ID + PLANTED_SECRET)):
                with self.subTest(kind=name):
                    result = run('--absent', str(msix(scratch, f'{name}.msix', b'MZ' + exe)))
                    self.assertEqual(result.returncode, 1, result.stdout)

    def test_present_is_the_positive_control_and_needs_both(self):
        with tempfile.TemporaryDirectory() as scratch:
            exe = Path(scratch) / 'eBIRForms.exe'
            for data, want in ((PLANTED_ID + PLANTED_SECRET, 0), (PLANTED_ID, 1), (PLANTED_SECRET, 1), (b'MZ', 1)):
                with self.subTest(data=len(data)):
                    exe.write_bytes(b'MZ' + data)
                    self.assertEqual(run('--present', str(exe)).returncode, want)

    def test_nothing_found_is_printed_nor_the_bytes_around_it(self):
        with tempfile.TemporaryDirectory() as scratch:
            dirty = msix(Path(scratch), 'dirty.msix', NEIGHBOUR + PLANTED_ID + NEIGHBOUR + PLANTED_SECRET + NEIGHBOUR)
            result = run('--absent', str(dirty))
            self.assertIn('holds a client id and a client secret', result.stdout)
            for leaked in (NEIGHBOUR, b'123456789012', b'planted', b'GOCSPX-p'):
                self.assertNotIn(leaked.decode(), result.stdout + result.stderr)

    def test_a_package_without_the_executable_is_refused(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / 'empty.msix'
            with zipfile.ZipFile(path, 'w') as archive:
                archive.writestr('AppxManifest.xml', '<Package/>')
            result = run('--absent', str(path))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('expected one eBIRForms.exe', result.stderr)


if __name__ == '__main__':
    unittest.main()
