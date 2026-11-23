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
        self.assertIn('artifacts/**/*.msix', release)
        self.assertIn('artifacts/**/*-windows-x86_64.zip', release)

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
