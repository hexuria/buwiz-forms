import json
import os
import shutil
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class NshWindowsReleaseTests(unittest.TestCase):
    def test_packaging_is_reusable_and_does_not_publish(self):
        workflow = (ROOT / '.github/workflows/release-windows.yml').read_text()
        self.assertIn('workflow_dispatch:', workflow)
        self.assertIn('workflow_call:', workflow)
        self.assertIn('nsh package --target windows', workflow)
        self.assertIn('actions/upload-artifact@v4', workflow)
        self.assertNotIn('contents: write', workflow)
        self.assertNotIn('gh-release', workflow)
        self.assertNotIn('bir-shell', workflow)

    def test_ci_and_tagged_release_use_the_same_packaging_workflow(self):
        ci = (ROOT / '.github/workflows/ci.yml').read_text()
        release = (ROOT / '.github/workflows/release.yml').read_text()
        for workflow in (ci, release):
            self.assertIn('uses: ./.github/workflows/release-windows.yml', workflow)
        self.assertNotIn('.msix', release.split('files: |', 1)[1].split('\n\n', 1)[0])
        self.assertIn('artifacts/**/*-windows-x86_64.zip', release)

    def test_setup_and_updater_zip_do_not_require_a_vc_redistributable(self):
        workflow = (ROOT / '.github/workflows/release-windows.yml').read_text()
        msix = (ROOT / 'scripts/package_msix.ps1').read_text()
        self.assertIn('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS: "-C target-feature=+crt-static"', workflow)
        self.assertIn('if ($NshPayload -and $runtimeImports.Count -gt 0)', msix)
        self.assertIn('if (-not $NshPayload -and $runtimeImports.Count -eq 0)', msix)

    @unittest.skipUnless(shutil.which('pwsh'), 'requires PowerShell')
    def test_invalid_version_tag_or_payload_fails_before_packaging(self):
        cases = (
            ('0.2.00', 'v0.2.00', '0.2.00', 'demo.id', 'Invalid Windows release version'),
            ('256.0.0', 'v256.0.0', '256.0.0', 'demo.id', 'exceeds MSI limits'),
            ('0.2.0', 'v0.2.1', '0.2.0', 'demo.id', 'does not match Windows app version'),
            ('0.2.0', 'v0.2.0', '0.2.1', 'demo.id', 'payload identity or version does not match'),
            ('0.2.0', 'v0.2.0', '0.2.0', 'other.id', 'payload identity or version does not match'),
        )
        with tempfile.TemporaryDirectory() as scratch:
            scratch = Path(scratch)
            script = scratch / 'scripts/package_nsh_windows.ps1'
            script.parent.mkdir()
            shutil.copyfile(ROOT / 'scripts/package_nsh_windows.ps1', script)
            config = scratch / 'nsh/nsh.json'
            config.parent.mkdir()
            package = scratch / 'package'
            app = package / 'eBIRForms/nsh/app.json'
            app.parent.mkdir(parents=True)
            out = scratch / 'out'
            for version, tag, app_version, app_id, error in cases:
                with self.subTest(version=version, tag=tag, app_version=app_version, app_id=app_id):
                    config.write_text(json.dumps({'version': version, 'id': 'demo.id'}), encoding='utf-8')
                    app.write_text(json.dumps({'version': app_version, 'id': app_id}), encoding='utf-8')
                    result = subprocess.run(
                        ['pwsh', '-NoProfile', '-File', str(script), '-PackageRoot', str(package), '-OutDir', str(out)],
                        env={**os.environ, 'GITHUB_REF_TYPE': 'tag', 'GITHUB_REF_NAME': tag, 'NO_COLOR': '1'},
                        capture_output=True, text=True, timeout=20,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(error, result.stderr)
                    self.assertFalse(out.exists())

    @unittest.skipUnless(shutil.which('pwsh'), 'requires PowerShell')
    def test_store_and_direct_switches_refuse_what_they_cannot_build(self):
        cases = (
            (['-MsixOnly', '-NoMsix'], {}, 'Pass -MsixOnly or -NoMsix, not both'),
            (['-NoMsix', '-SmokeMsix'], {}, '-SmokeMsix needs the .msix'),
            (['-MsixOnly'], {'WINDOWS_CERTIFICATE_PFX': 'AAAA'}, 'The Store package is never signed here'),
        )
        with tempfile.TemporaryDirectory() as scratch:
            out = Path(scratch) / 'out'
            for switches, env, error in cases:
                with self.subTest(switches=switches, env=sorted(env)):
                    result = subprocess.run(
                        ['pwsh', '-NoProfile', '-File', str(ROOT / 'scripts/package_nsh_windows.ps1'),
                         '-PackageRoot', scratch, '-OutDir', str(out), *switches],
                        env={**os.environ, 'WINDOWS_CERTIFICATE_PFX': '', **env, 'NO_COLOR': '1'},
                        capture_output=True, text=True, timeout=20,
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn(error, result.stderr)
                    self.assertFalse(out.exists())

    def test_msi_has_new_per_user_identity_and_distinct_install_directory(self):
        ns = {'w': 'http://schemas.microsoft.com/wix/2006/wi'}
        product = ET.parse(ROOT / 'packaging/windows/nsh.wxs').find('w:Product', ns)
        self.assertNotEqual(product.attrib['UpgradeCode'], 'e8a5f36b-9c17-4e62-9721-a1b2c3d4e5f6')
        self.assertEqual(product.find('w:Package', ns).attrib['InstallScope'], 'perUser')
        self.assertIsNotNone(product.find('.//w:Directory[@Id="LocalAppDataFolder"]', ns))
        self.assertIsNotNone(product.find('.//w:Directory[@Id="INSTALLDIR"][@Name="eBIRForms MSI"]', ns))
        self.assertIsNotNone(product.find('.//w:RegistryValue[@Root="HKCU"][@KeyPath="yes"]', ns))


if __name__ == '__main__':
    unittest.main()
