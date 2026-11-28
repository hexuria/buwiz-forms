import gzip
import io
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
CLEAN_EXE = b'MZ' + b'\0' * 4096 + b'.apps.googleusercontent.com GOCSPX-'


def run(*args):
    return subprocess.run([sys.executable, '-I', str(AUDIT), *args], capture_output=True, text=True, timeout=30)


def zip_bytes(members):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
        for name, data in members.items():
            archive.writestr(name, data)
    return buffer.getvalue()


def package(folder: Path, name: str, members) -> Path:
    path = folder / name
    path.write_bytes(zip_bytes({'AppxManifest.xml': b'<Package/>', **members}))
    return path


class AuditGoogleClientTests(unittest.TestCase):
    def test_the_self_test_passes(self):
        result = run('--self-test')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('self-test passed', result.stdout)

    def test_absent_reads_every_member_not_only_the_exe(self):
        with tempfile.TemporaryDirectory() as scratch:
            scratch = Path(scratch)
            clean = package(scratch, 'clean.msix', {'eBIRForms.exe': CLEAN_EXE, 'nsh/bundle.nshc': b'\0' * 64})
            self.assertEqual(run('--absent', str(clean)).returncode, 0)
            cases = {
                'id in the exe': {'eBIRForms.exe': b'MZ' + PLANTED_ID},
                'secret in a dll': {'eBIRForms.exe': CLEAN_EXE, 'lib/other.dll': PLANTED_SECRET},
                'id in the payload': {'eBIRForms.exe': CLEAN_EXE, 'nsh/bundle.nshc': PLANTED_ID},
                'id in a nested zip': {'eBIRForms.exe': CLEAN_EXE, 'inner.zip': zip_bytes({'a.bin': PLANTED_ID})},
                'secret in a gzip member': {'eBIRForms.exe': CLEAN_EXE, 'x.gz': gzip.compress(PLANTED_SECRET)},
            }
            for name, members in cases.items():
                with self.subTest(case=name):
                    result = run('--absent', str(package(scratch, 'dirty.msix', members)))
                    self.assertEqual(result.returncode, 1, result.stdout + result.stderr)

    def test_an_archive_it_cannot_read_is_refused_both_ways(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = package(Path(scratch), 'zstd.msix', {'eBIRForms.exe': CLEAN_EXE, 'x.zst': b'\x28\xb5\x2f\xfd' + b'\0' * 32})
            for mode in ('--absent', '--present'):
                with self.subTest(mode=mode):
                    result = run(mode, str(path))
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn('zstd archive', result.stderr)

    def test_present_is_the_positive_control_and_needs_both(self):
        with tempfile.TemporaryDirectory() as scratch:
            scratch = Path(scratch)
            for members, want in (
                ({'eBIRForms/eBIRForms.exe': b'MZ' + PLANTED_ID + PLANTED_SECRET}, 0),
                ({'eBIRForms/eBIRForms.exe': b'MZ' + PLANTED_ID}, 1),
                ({'eBIRForms/eBIRForms.exe': b'MZ' + PLANTED_SECRET}, 1),
                ({'eBIRForms/eBIRForms.exe': CLEAN_EXE}, 1),
            ):
                with self.subTest(want=want, members=sorted(members)):
                    self.assertEqual(run('--present', str(package(scratch, 'direct.zip', members))).returncode, want)

    def test_only_the_names_of_files_are_printed_never_what_was_found(self):
        with tempfile.TemporaryDirectory() as scratch:
            dirty = package(Path(scratch), 'dirty.msix', {
                'eBIRForms.exe': NEIGHBOUR + PLANTED_ID + NEIGHBOUR + PLANTED_SECRET + NEIGHBOUR,
            })
            result = run('--absent', str(dirty))
            self.assertIn('a client id in dirty.msix/eBIRForms.exe', result.stdout)
            self.assertIn('a client secret in dirty.msix/eBIRForms.exe', result.stdout)
            for leaked in (NEIGHBOUR, b'123456789012', b'planted', b'GOCSPX-p'):
                self.assertNotIn(leaked.decode(), result.stdout + result.stderr)

    def test_members_zipfile_cannot_read_are_refused_not_tracebacks(self):
        with tempfile.TemporaryDirectory() as scratch:
            stored = io.BytesIO()
            with zipfile.ZipFile(stored, 'w', zipfile.ZIP_STORED) as archive:
                archive.writestr('eBIRForms.exe', CLEAN_EXE)
            data = stored.getvalue()
            central = data.index(b'PK\x01\x02')
            # Deflate64 (method 9), which zipfile cannot decompress; and the "encrypted" flag (bit 0).
            deflate64 = bytearray(data)
            deflate64[8:10] = deflate64[central + 10:central + 12] = (9).to_bytes(2, 'little')
            encrypted = bytearray(data)
            encrypted[6] |= 1
            encrypted[central + 8] |= 1
            for name, patched, said in (
                ('deflate64', deflate64, 'a compression this audit cannot read'),
                ('encrypted', encrypted, 'could not be read'),
            ):
                with self.subTest(case=name):
                    path = Path(scratch) / f'{name}.msix'
                    path.write_bytes(bytes(patched))
                    result = run('--absent', str(path))
                    self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                    self.assertIn(said, result.stderr)
                    self.assertNotIn('Traceback', result.stderr)

    def test_a_missing_package_is_refused(self):
        result = run('--absent', '/nonexistent/eBIRForms.msix')
        self.assertEqual(result.returncode, 2)


if __name__ == '__main__':
    unittest.main()
