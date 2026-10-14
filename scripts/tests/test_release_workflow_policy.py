from __future__ import annotations

import json
import re
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release.yml"
CI_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/ci.yml"
DESKTOP_MANIFEST_PATH = REPOSITORY_ROOT / "crates/bir-desktop/Cargo.toml"
INSTALLER_WXS_PATH = REPOSITORY_ROOT / "installer.wxs"
JUSTFILE_PATH = REPOSITORY_ROOT / "justfile"
LINUX_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-linux.sh"
WINDOWS_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-windows.ps1"
FINAL_SETUP_AUDIT_PATH = REPOSITORY_ROOT / "scripts/audit_final_setup.ps1"


class ReleaseWorkflowPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.desktop_manifest = DESKTOP_MANIFEST_PATH.read_text(encoding="utf-8")
        cls.installer_wxs = INSTALLER_WXS_PATH.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE_PATH.read_text(encoding="utf-8")
        cls.linux_setup = LINUX_SETUP_PATH.read_text(encoding="utf-8")
        cls.windows_setup = WINDOWS_SETUP_PATH.read_text(encoding="utf-8")
        cls.final_setup_audit = FINAL_SETUP_AUDIT_PATH.read_text(encoding="utf-8")

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
            "python -m unittest discover -s scripts/tests -v",
            "freeze_html.py --verify --out html-frozen --index rules/index.json",
            "stamp_frozen_names.py --self-test",
            "stamp_frozen_names.py --check-all",
            "python3 scripts/audit_no_legacy.py",
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
        self.assertIn("choco install openssl innosetup -y", windows)
        self.assertIn("Audit final Windows installer payloads", windows)
        self.assertNotIn("7z", windows.lower())
        self.assertIn("Start-Process msiexec.exe", windows)
        self.assertIn('WINDOWS_STAGE=$PKG', windows)
        self.assertGreaterEqual(windows.count("$PKG = $env:WINDOWS_STAGE"), 2)
        source_gate = windows.index("scripts/audit_installer_sources.py")
        compile_installer = windows.index("& $ISCC")
        self.assertLess(source_gate, compile_installer)
        self.assertIn("scripts/audit_final_setup.ps1", windows)
        self.assertIn(
            "python scripts/audit_no_legacy.py --package-root $msiRoot",
            windows,
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

    def test_local_setup_recipe_checks_prerequisites_and_sources_before_iscc(
        self,
    ) -> None:
        exe_recipe = self.justfile.split(
            "# Build the Inno Setup executable installer (Windows only)", maxsplit=1
        )[1].split("# Build a Store-only MSIX candidate", maxsplit=1)[0]
        self.assertIn("check-exe-prerequisites build-frozen-html-identity", exe_recipe)
        self.assertIn(
            "Configured Python is not a usable CPython interpreter",
            exe_recipe,
        )
        self.assertIn("Inno Setup compiler was not found", exe_recipe)
        self.assertIn(
            'ISCC := "C:\\\\Program Files (x86)\\\\Inno Setup 6\\\\ISCC.exe"',
            self.justfile,
        )
        self.assertIn('$iscc = "{{ISCC}}"', exe_recipe)
        self.assertIn('$ISCC = "{{ISCC}}"', exe_recipe)
        source_gate = exe_recipe.index("scripts/audit_installer_sources.py")
        compile_installer = exe_recipe.index("& $ISCC")
        self.assertLess(source_gate, compile_installer)
        self.assertNotIn("7z", exe_recipe.lower())
        self.assertNotIn("audit_final_setup.ps1", exe_recipe)

    def test_exe_recipe_and_prerequisite_are_windows_only(self) -> None:
        self.assertRegex(self.justfile, r"(?m)^\[windows\]\r?\nexe \*args=")
        self.assertRegex(
            self.justfile,
            r"(?m)^\[private\]\r?\n\[windows\]\r?\ncheck-exe-prerequisites:",
        )

    def test_final_setup_audit_checks_registration_name_and_after_uninstall(
        self,
    ) -> None:
        self.assertIn("$name -ieq 'eBIRForms_is1'", self.final_setup_audit)
        self.assertIn(
            "$remaining = Find-EbirFormsUninstallEntry",
            self.final_setup_audit,
        )
        self.assertIn("if ($remaining)", self.final_setup_audit)
        uninstall = self.final_setup_audit.index(
            "Start-Process -FilePath $uninstaller"
        )
        registration_check = self.final_setup_audit.index(
            "$remaining = Find-EbirFormsUninstallEntry"
        )
        self.assertLess(uninstall, registration_check)

    def test_ci_tests_python_policy_and_audits_a_real_unsigned_setup(self) -> None:
        self.assertIn(
            "python -m unittest discover -s scripts/tests -v",
            self.ci_workflow,
        )
        check = self.ci_workflow.split("  validation-rules-v1:", maxsplit=1)[0]
        # `changes` picks the systems; an app change runs all three.
        all_systems = re.search(r"(?m)^\s+all_systems='([^']+)'\s*$", self.ci_workflow)
        self.assertIsNotNone(all_systems)
        if all_systems is not None:
            self.assertEqual(
                [entry["os"] for entry in json.loads(all_systems.group(1))["include"]],
                ["macos-14", "ubuntu-22.04", "windows-latest"],
            )
        self.assertEqual(
            check.count("cargo install just --version 1.45.0 --locked"),
            1,
        )
        self.assertEqual(check.count("run: just --summary"), 1)
        just_install = re.search(
            r"(?ms)^    - name: Install just\n(.*?)(?=^    - |\Z)",
            check,
        )
        just_step = re.search(
            r"(?ms)^    - name: Validate justfile\n(.*?)(?=^    - |\Z)",
            check,
        )
        self.assertIsNotNone(just_install)
        self.assertIsNotNone(just_step)
        self.assertLess(
            check.index("    - name: Install Rust Toolchain"),
            check.index("    - name: Rust Cache"),
        )
        self.assertLess(
            check.index("    - name: Rust Cache"),
            check.index("    - name: Install just"),
        )
        self.assertLess(
            check.index("run: just --summary"),
            check.index("    - name: Test\n"),
        )
        for match in re.finditer(
            r"(?m)^\s*(?:run:\s*)?cargo (?:check|test)\b",
            check,
        ):
            self.assertGreater(match.start(), check.index("run: just --summary"))
        self.assertNotIn("choco install just -y", check)
        self.assertIn("choco install innosetup -y", check)
        self.assertIn("3010", check)
        self.assertIn("just exe", check)
        self.assertIn("scripts/audit_final_setup.ps1", check)

    def test_setup_scripts_keep_the_gpui_agent_revision_pinned(self) -> None:
        self.assertIn('local manifest="$SCRIPT_DIR/../Cargo.toml"', self.linux_setup)
        self.assertIn(
            "gpui-agent rev pin not found in repository-root Cargo.toml",
            self.linux_setup,
        )
        self.assertIn('cargo install --list 2>/dev/null', self.linux_setup)
        self.assertIn('--rev "$rev" gpui-agent-cli', self.linux_setup)
        self.assertNotIn("crates/bir-desktop/Cargo.toml", self.linux_setup)
        self.assertNotIn("--locked gpui-agent-cli", self.linux_setup)
        self.assertNotIn("installing from main", self.linux_setup)
        self.assertIn("'Cargo.toml'", self.windows_setup)
        self.assertIn("3010", self.windows_setup)

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
