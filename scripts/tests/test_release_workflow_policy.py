from __future__ import annotations

import re
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release.yml"
DESKTOP_MANIFEST_PATH = REPOSITORY_ROOT / "crates/bir-desktop/Cargo.toml"
INSTALLER_WXS_PATH = REPOSITORY_ROOT / "installer.wxs"
JUSTFILE_PATH = REPOSITORY_ROOT / "justfile"


class ReleaseWorkflowPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.desktop_manifest = DESKTOP_MANIFEST_PATH.read_text(encoding="utf-8")
        cls.installer_wxs = INSTALLER_WXS_PATH.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE_PATH.read_text(encoding="utf-8")

    def job(self, name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
            self.workflow,
        )
        self.assertIsNotNone(match, f"missing release job: {name}")
        return match.group(0) if match is not None else ""

    def test_every_publishable_build_depends_on_exact_tag_preflight(self) -> None:
        preflight = self.job("preflight")
        self.assertIn('git rev-parse "refs/tags/$GITHUB_REF_NAME^{commit}"', preflight)
        self.assertIn('= "$GITHUB_SHA"', preflight)
        self.assertIn(
            "git ls-files --error-unmatch Cargo.lock package-lock.json",
            preflight,
        )
        self.assertIn(r"^v[0-9]+\.[0-9]+\.[0-9]+$", preflight)
        self.assertIn('TAG_VERSION="${GITHUB_REF_NAME#v}"', preflight)
        self.assertIn('"$TAG_VERSION" != "$CARGO_VERSION"', preflight)
        for required_gate in (
            "cargo fmt --all -- --check",
            "cargo clippy --locked --workspace -- -D warnings",
            "cargo test --locked --workspace",
            "npm run test:forms",
            "python3 -m unittest discover -s scripts/tests -v",
            "npm run verify:forms:offline",
            "npm run test:forms:visual",
        ):
            self.assertIn(required_gate, preflight)
        for job_name in ("build-macos", "build-windows", "build-linux"):
            self.assertIn("needs: preflight", self.job(job_name))
        self.assertIn(
            "needs: [preflight, build-macos, build-windows, build-linux]",
            self.job("release"),
        )

    def test_macos_public_artifact_is_signed_notarized_and_verified(self) -> None:
        macos = self.job("build-macos")
        for secret in (
            "APPLE_CERTIFICATE_P12",
            "APPLE_CERTIFICATE_PASSWORD",
            "APPLE_TEAM_ID",
            "APPLE_ID",
            "APPLE_APP_PASSWORD",
        ):
            self.assertIn(f"secrets.{secret}", macos)
        self.assertNotIn("if: env.APPLE_", macos)
        self.assertIn("codesign --force --options runtime", macos)
        self.assertIn("xcrun notarytool submit", macos)
        self.assertIn("xcrun stapler staple", macos)
        self.assertIn("xcrun stapler validate", macos)
        self.assertIn('cp assets/AppIcon.icns "$APP/Contents/Resources/"', macos)
        self.assertIn('hdiutil attach "$DMG" -nobrowse -readonly', macos)
        self.assertIn(
            'python3 scripts/audit_no_legacy.py --package-root "$MOUNTED_APP"',
            macos,
        )
        self.assertIn("security set-key-partition-list", macos)
        self.assertIn("security find-identity -v -p codesigning", macos)
        self.assertIn('--keychain "$KEYCHAIN_PATH"', macos)
        self.assertNotIn(
            'SIGNING_IDENTITY="Developer ID Application:',
            macos,
        )
        self.assertIn("if-no-files-found: error", macos)
        self.assertNotIn("target/release-artifacts/*.zip", macos)

    def test_windows_public_artifacts_require_authenticode_signatures(self) -> None:
        windows = self.job("build-windows")
        self.assertIn("secrets.WINDOWS_CERTIFICATE_PFX", windows)
        self.assertIn("secrets.WINDOWS_CERTIFICATE_PASSWORD", windows)
        self.assertIn("WINDOWS_SIGNTOOL sign /fd SHA256", windows)
        self.assertIn("WINDOWS_SIGNTOOL verify /pa /all /v", windows)
        self.assertIn("choco install openssl innosetup 7zip -y", windows)
        self.assertIn("Audit final Windows installer payloads", windows)
        self.assertIn("7z.exe", windows)
        self.assertIn("Start-Process msiexec.exe", windows)
        self.assertGreaterEqual(
            windows.count("python scripts/audit_no_legacy.py --package-root"),
            3,
        )
        self.assertIn(
            "Expected exactly one EXE and one MSI public release artifact",
            windows,
        )
        self.assertIn(
            "Public GitHub releases must not contain MSIX artifacts",
            windows,
        )
        self.assertNotIn("- name: Create MSIX Package", windows)
        self.assertIn("if-no-files-found: error", windows)
        self.assertNotIn("target/release-artifacts/*.zip", windows)

        upload = windows.split("- name: Upload Windows Artifact", maxsplit=1)[1]
        self.assertIn("target/release-artifacts/*-Setup.exe", upload)
        self.assertIn("target/release-artifacts/*.msi", upload)
        self.assertNotIn("target/release-artifacts/*.msix", upload)

        public_release = self.job("release")
        self.assertNotIn("artifacts/**/*.msix", public_release)

    def test_package_builds_are_deterministic_and_scope_consistent(self) -> None:
        windows = self.job("build-windows")
        self.assertIn("choco install wixtoolset --version=3.11.2 -y", windows)
        self.assertIn("-cg HarvestedComponents -ag ", windows)
        self.assertNotIn("-cg HarvestedComponents -gg ", windows)
        self.assertIn("candle.exe installer.wxs harvest.wxs -arch x64", windows)
        self.assertNotIn(" -sval", windows)
        self.assertIn('Root="HKLM"', self.installer_wxs)
        self.assertNotIn('Root="HKCU"', self.installer_wxs)
        self.assertIn('Win64="yes"', self.installer_wxs)

        linux = self.job("build-linux")
        self.assertIn(
            "cargo install --locked cargo-deb --version 3.7.0",
            linux,
        )
        self.assertLess(
            linux.index("mkdir -p target/release-artifacts"),
            linux.index("cargo deb --locked"),
        )
        self.assertIn(
            'maintainer = "Goldcoders Corp <support@goldcoders.dev>"',
            self.desktop_manifest,
        )
        self.assertIn("dpkg-deb --extract", linux)
        self.assertIn('tar xzf "$TARBALL"', linux)
        self.assertIn(
            'python3 scripts/audit_no_legacy.py --package-root "$DEB_ROOT"',
            linux,
        )
        self.assertIn(
            'python3 scripts/audit_no_legacy.py --package-root "$TAR_ROOT"',
            linux,
        )
        self.assertIn('dpkg-deb --extract "$DEB" "$AUDIT_ROOT"', self.justfile)
        self.assertIn('tar xzf "$TARBALL" -C "$AUDIT_ROOT"', self.justfile)

    def test_msix_is_store_only_and_still_blocked_on_certification(self) -> None:
        self.assertIn("Store-only MSIX candidate", self.justfile)
        self.assertIn(
            "BLOCKED: certify manifest artwork and packaged MSVC runtime behavior",
            self.justfile,
        )


if __name__ == "__main__":
    unittest.main()
