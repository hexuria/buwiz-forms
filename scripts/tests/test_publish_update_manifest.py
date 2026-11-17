"""Platform keys and checksum parsing for the updater manifest."""

import importlib.util
import unittest
from pathlib import Path

_PATH = Path(__file__).resolve().parents[1] / "publish_update_manifest.py"
_SPEC = importlib.util.spec_from_file_location("publish_update_manifest", _PATH)
assert _SPEC is not None and _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)

build_manifest = _MODULE.build_manifest
classify_asset = _MODULE.classify_asset
parse_checksum_text = _MODULE.parse_checksum_text


class ClassifyTests(unittest.TestCase):
    def test_platform_keys(self):
        self.assertEqual(
            classify_asset("eBIRForms-macOS-universal-0.1.1.dmg"),
            "darwin-universal",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Windows-x64-0.1.1-Setup.exe"),
            "windows-x64-setup",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Windows-x64-0.1.1.msi"),
            "windows-x64-msi",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Linux-x64-0.1.1.deb"),
            "linux-x64-deb",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Linux-arm64-0.1.1.rpm"),
            "linux-arm64-rpm",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Linux-x64-0.1.1.tar.gz"),
            "linux-x64-tarball",
        )
        self.assertEqual(
            classify_asset("eBIRForms-Linux-x64-0.1.1.pkg.tar.zst"),
            "linux-x64-arch",
        )
        self.assertIsNone(classify_asset("eBIRForms-Linux-arm64-0.1.1.pkg.tar.zst"))
        self.assertIsNone(classify_asset("latest.json"))

    def test_checksum_lines_and_a_bare_digest(self):
        digest = "ab" * 32
        parsed = parse_checksum_text(f"{digest}  eBIRForms.dmg\n")
        self.assertEqual(parsed["eBIRForms.dmg"], digest)
        starred = parse_checksum_text(f"{digest} *package.deb\n")
        self.assertEqual(starred["package.deb"], digest)
        bare = parse_checksum_text(f"{digest.upper()}\n")
        self.assertEqual(bare[""], digest)

    def test_manifest_points_at_the_checksum_sidecar(self):
        digest = "cd" * 32
        manifest = build_manifest(
            "0.1.1",
            "hexuria/buwiz-forms",
            "v0.1.1",
            [
                {"name": "eBIRForms-macOS-universal-0.1.1.dmg"},
                {"name": "eBIRForms-macOS-universal-0.1.1.dmg.sha256"},
                {"name": "eBIRForms-Linux-x64-0.1.1.deb"},
                {"name": "SHA256SUMS-linux-x64.txt"},
                {"name": "eBIRForms-Windows-x64-0.1.1-Setup.exe"},
                {"name": "SHA256SUMS-windows-x64.txt"},
            ],
            {
                "eBIRForms-macOS-universal-0.1.1.dmg": digest,
                "eBIRForms-macOS-universal-0.1.1.dmg.sha256": "eBIRForms-macOS-universal-0.1.1.dmg.sha256",
                "eBIRForms-Linux-x64-0.1.1.deb": digest,
                "SHA256SUMS-linux-x64.txt": "SHA256SUMS-linux-x64.txt",
                "eBIRForms-Windows-x64-0.1.1-Setup.exe": digest,
                "SHA256SUMS-windows-x64.txt": "SHA256SUMS-windows-x64.txt",
            },
        )
        darwin = manifest["platforms"]["darwin-universal"]
        self.assertEqual(darwin["sha256"], digest)
        self.assertTrue(darwin["checksum_url"].endswith(".dmg.sha256"))
        self.assertTrue(
            manifest["platforms"]["linux-x64-deb"]["checksum_url"].endswith(
                "SHA256SUMS-linux-x64.txt"
            )
        )
        self.assertTrue(
            manifest["platforms"]["windows-x64-setup"]["checksum_url"].endswith(
                "SHA256SUMS-windows-x64.txt"
            )
        )


if __name__ == "__main__":
    unittest.main()
