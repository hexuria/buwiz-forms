from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release.yml"
LINUX_RELEASE_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release-linux.yml"
WINDOWS_RELEASE_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release-windows.yml"
MACOS_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/release-macos.yml"
MACOS_PACKAGE_SCRIPT_PATH = REPOSITORY_ROOT / "scripts/package_macos_release.sh"
CI_WORKFLOW_PATH = REPOSITORY_ROOT / ".github/workflows/ci.yml"
DESKTOP_MANIFEST_PATH = REPOSITORY_ROOT / "crates/bir-desktop/Cargo.toml"
LINUX_BUILD_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/build-packages.sh"
LINUX_SMOKE_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/smoke-test.sh"
LINUX_INSTALL_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/install-linux.sh"
INSTALLER_WXS_PATH = REPOSITORY_ROOT / "installer.wxs"
JUSTFILE_PATH = REPOSITORY_ROOT / "justfile"
LINUX_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-linux.sh"
WINDOWS_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-windows.ps1"
FINAL_SETUP_AUDIT_PATH = REPOSITORY_ROOT / "scripts/audit_final_setup.ps1"
PACKAGE_MSIX_PATH = REPOSITORY_ROOT / "scripts/package_msix.ps1"
SMOKE_MSIX_PATH = REPOSITORY_ROOT / "scripts/smoke_msix.ps1"


class ReleaseWorkflowPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.linux_release_workflow = LINUX_RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.windows_workflow = WINDOWS_RELEASE_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.macos_workflow = MACOS_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.macos_package_script = MACOS_PACKAGE_SCRIPT_PATH.read_text(
            encoding="utf-8"
        )
        cls.ci_workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
        cls.desktop_manifest = DESKTOP_MANIFEST_PATH.read_text(encoding="utf-8")
        cls.linux_build_script = LINUX_BUILD_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.linux_smoke_script = LINUX_SMOKE_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.linux_install_script = LINUX_INSTALL_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.installer_wxs = INSTALLER_WXS_PATH.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE_PATH.read_text(encoding="utf-8")
        cls.linux_setup = LINUX_SETUP_PATH.read_text(encoding="utf-8")
        cls.windows_setup = WINDOWS_SETUP_PATH.read_text(encoding="utf-8")
        cls.final_setup_audit = FINAL_SETUP_AUDIT_PATH.read_text(encoding="utf-8")
        cls.package_msix = PACKAGE_MSIX_PATH.read_text(encoding="utf-8")
        cls.smoke_msix = SMOKE_MSIX_PATH.read_text(encoding="utf-8")

    def job(self, name: str, workflow: str | None = None) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
            self.workflow if workflow is None else workflow,
        )
        self.assertIsNotNone(match, f"missing release job: {name}")
        return match.group(0) if match is not None else ""

    def windows_job(self, name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [a-z][a-z0-9-]*:\n|\Z)",
            self.windows_workflow,
        )
        self.assertIsNotNone(match, f"missing Windows release job: {name}")
        return match.group(0) if match is not None else ""

    def assert_preflight_gates(self, workflow: str) -> None:
        preflight = self.job("preflight", workflow)
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

    def test_every_publishable_build_depends_on_exact_tag_preflight(self) -> None:
        self.assert_preflight_gates(self.workflow)
        for job_name in ("build-windows", "build-linux"):
            self.assertIn("needs: preflight", self.job(job_name))
        self.assertIn(
            "needs: [preflight, build-windows, build-linux]",
            self.job("release"),
        )
        self.assertNotIn("build-macos:", self.workflow)
        self.assertNotIn("*.dmg", self.workflow)

    def test_macos_workflow_preflight_and_publish_shape(self) -> None:
        self.assert_preflight_gates(self.macos_workflow)
        self.assertIn("pull_request:", self.macos_workflow)
        self.assertIn(".github/workflows/release-macos.yml", self.macos_workflow)
        self.assertIn("scripts/package_macos_release.sh", self.macos_workflow)
        build_macos = self.job("build-macos", self.macos_workflow)
        self.assertIn("needs: preflight", build_macos)
        self.assertIn(
            "github.event.pull_request.head.repo.full_name == github.repository",
            build_macos,
        )
        publish = self.job("publish", self.macos_workflow)
        self.assertIn("needs: [preflight, build-macos]", publish)
        self.assertIn("github.event_name == 'push'", publish)
        self.assertIn("contents: write", publish)
        without_publish = self.macos_workflow.replace(publish, "")
        self.assertNotIn("contents: write", without_publish)
        self.assertIn("contents: read", self.macos_workflow)

    def test_macos_public_artifact_is_signed_notarized_and_verified(self) -> None:
        macos = self.job("build-macos", self.macos_workflow)
        script = self.macos_package_script
        for secret in (
            "APPLE_CERTIFICATE_P12",
            "APPLE_CERTIFICATE_PASSWORD",
            "APPLE_API_KEY_P8",
            "APPLE_API_KEY_ID",
            "APPLE_API_ISSUER_ID",
        ):
            self.assertIn(f"secrets.{secret}", macos)
        self.assertNotIn("if: env.APPLE_", macos)
        self.assertIn("REQUIRE_NOTARIZATION=1", self.macos_workflow)
        self.assertIn("security set-key-partition-list", macos)
        self.assertIn("security find-identity -v -p codesigning", macos)
        self.assertIn('--keychain "$KEYCHAIN_PATH"', script)
        self.assertNotIn(
            'SIGNING_IDENTITY="Developer ID Application:',
            macos + script,
        )
        self.assertIn("Goldcoders Corp (5KZ8MD34QW)", macos)
        self.assertIn("codesign --force --options runtime", script)
        self.assertIn(
            "--entitlements assets/macos/entitlements.mac.plist", script
        )
        self.assertIn("xcrun notarytool submit", script)
        self.assertIn("--output-format json", script)
        self.assertIn('result.get("status") == "Accepted"', script)
        self.assertIn('xcrun stapler staple "$APP"', script)
        self.assertIn('xcrun stapler staple "$DMG"', script)
        self.assertIn('xcrun stapler validate "$DMG"', script)
        self.assertIn('spctl --assess --type execute --verbose=2 "$APP"', script)
        self.assertIn('cp assets/AppIcon.icns "$APP/Contents/Resources/"', script)
        self.assertIn('hdiutil attach "$DMG" -nobrowse -readonly', script)
        self.assertIn(
            'python3 scripts/audit_no_legacy.py --package-root "$MOUNTED_APP"',
            script,
        )
        self.assertIn("if-no-files-found: error", macos)
        self.assertNotIn("*.zip", macos)

    def test_package_script_refuses_adhoc_when_notarization_is_required(
        self,
    ) -> None:
        if sys.platform == "win32":
            self.skipTest("bash on Windows may resolve to WSL's System32 bash")
        bash = shutil.which("bash")
        if bash is None:
            self.skipTest("bash is required to exercise package_macos_release.sh")
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = Path(tmp) / "release-artifacts"
            env = dict(os.environ)
            env.update(
                {
                    "VERSION": "1.2.3",
                    "REQUIRE_NOTARIZATION": "1",
                    "SIGNING_IDENTITY": "-",
                    "OUT_DIR": str(out_dir),
                }
            )
            for name in (
                "NOTARY_KEY_PATH",
                "NOTARY_KEY_ID",
                "NOTARY_ISSUER_ID",
            ):
                env.pop(name, None)
            result = subprocess.run(
                [bash, str(MACOS_PACKAGE_SCRIPT_PATH)],
                env=env,
                cwd=REPOSITORY_ROOT,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("REQUIRE_NOTARIZATION", result.stderr)
            self.assertFalse(
                out_dir.exists(),
                "script must fail before creating any output",
            )

    def test_windows_public_artifacts_require_authenticode_signatures(self) -> None:
        windows = self.windows_job("installers")
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

        public_windows_job = self.job("build-windows")
        self.assertIn("uses: ./.github/workflows/release-windows.yml", public_windows_job)
        self.assertIn("release: true", public_windows_job)
        self.assertIn("secrets: inherit", public_windows_job)
        self.assertIn("needs: preflight", public_windows_job)

        public_release = self.job("release")
        self.assertNotIn("artifacts/**/*.msix", public_release)

    def test_reusable_windows_workflow_gates_signing_and_keeps_msix_private(self) -> None:
        windows = self.windows_job("installers")
        signing = windows.split("- name: Resolve Windows signing mode", maxsplit=1)[1].split(
            "\n      - name:", maxsplit=1
        )[0]
        self.assertIn("inputs.release", signing)
        self.assertIn("Missing required Windows release secret", signing)
        self.assertIn("WINDOWS_CERTIFICATE_PFX", signing)
        self.assertIn("WINDOWS_CERTIFICATE_PASSWORD", signing)

        msix = self.windows_job("msix")
        self.assertIn("continue-on-error: ${{ inputs.release == true }}", msix)
        self.assertIn("powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/package_msix.ps1", msix)
        self.assertIn("powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_msix.ps1", msix)
        self.assertIn("name: windows-msix", msix)
        self.assertNotIn("name: windows-release", msix)
        self.assertIn("if: always()", msix)

    def test_msix_packaging_and_smoke_scripts_enforce_runtime_and_registration_policies(
        self,
    ) -> None:
        self.assertIn("dumpbin", self.package_msix)
        self.assertIn("/dependents", self.package_msix)
        self.assertIn("Get-AppxPackage -Name $identityName", self.smoke_msix)
        self.assertIn("Refusing to smoke-test because", self.smoke_msix)
        self.assertIn("EnableLUA", self.smoke_msix)
        self.assertIn(
            "& $makeAppx unpack /p $packagePath /d $layoutPath /o",
            self.smoke_msix,
        )
        self.assertIn("RequireActivation", self.smoke_msix)
        self.assertIn("smoke-msix-layout-stderr.log", self.windows_job("msix"))

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

    def test_msix_recipe_uses_shared_packaging_script(self) -> None:
        msix_recipe = self.justfile.split(
            "# Build a Store-only MSIX candidate", maxsplit=1
        )[1].split("# Sign the Store-identity MSIX", maxsplit=1)[0]
        self.assertIn(
            'Write-Warning "STORE-ONLY MSIX candidate; not a public GitHub release artifact"',
            msix_recipe,
        )
        self.assertIn(
            'Write-Warning "BLOCKED: certify manifest artwork and packaged MSVC runtime behavior before Store submission"',
            msix_recipe,
        )
        self.assertIn("scripts/package_msix.ps1", msix_recipe)

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
        windows = self.windows_job("installers")
        self.assertIn("choco install wixtoolset --version=3.11.2 -y", windows)
        self.assertIn("function Find-WixBin", windows)
        self.assertIn('"WIX_BIN=$wixBin" >> $env:GITHUB_ENV', windows)
        self.assertIn("$env:WIX_BIN", windows)
        self.assertIn("heat.exe", windows)
        self.assertIn("-cg HarvestedComponents -ag ", windows)
        self.assertNotIn("-cg HarvestedComponents -gg ", windows)
        self.assertIn("candle.exe installer.wxs harvest.wxs -arch x64", windows)
        light_commands = [
            line.strip()
            for line in windows.splitlines()
            if line.strip().startswith("light.exe ")
        ]
        self.assertEqual(len(light_commands), 1)
        self.assertIn(
            "-sice:ICE38 -sice:ICE43 -sice:ICE57",
            light_commands[0],
        )
        self.assertEqual(
            re.findall(r"-sice:\S+", self.windows_workflow),
            ["-sice:ICE38", "-sice:ICE43", "-sice:ICE57"],
        )
        self.assertNotIn("-sval", self.windows_workflow)
        self.assertIn('Root="HKLM"', self.installer_wxs)
        self.assertNotIn('Root="HKCU"', self.installer_wxs)
        self.assertIn('Win64="yes"', self.installer_wxs)

        linux_job = self.job("build-linux")
        self.assertIn("uses: ./.github/workflows/release-linux.yml", linux_job)
        self.assertIn("tag: ${{ github.ref_name }}", linux_job)
        linux = self.linux_release_workflow
        self.assertIn('if [[ -n "$RELEASE_TAG" ]]; then', linux)
        self.assertNotIn('GITHUB_EVENT_NAME" == workflow_call', linux)
        self.assertIn(
            "group: release-linux-${{ github.event_name }}-${{ github.ref }}",
            linux,
        )
        self.assertIn("ubuntu-22.04-arm", linux)
        self.assertIn("x86_64-unknown-linux-gnu", linux)
        self.assertIn("aarch64-unknown-linux-gnu", linux)
        self.assertIn("cargo build --locked --release --target", linux)
        self.assertIn("build-packages.sh --version", linux)
        self.assertIn("smoke-test.sh --out", linux)
        self.assertIn("name: linux-release-${{ matrix.arch }}", linux)
        self.assertIn("cargo install --locked", self.linux_build_script)
        self.assertIn("cargo-deb 3.7.0", self.linux_build_script)
        self.assertIn("cargo-generate-rpm 0.21.0", self.linux_build_script)
        self.assertLess(
            self.linux_build_script.index('mkdir -p "$OUT"'),
            self.linux_build_script.index("cargo deb --locked"),
        )
        self.assertIn(
            'maintainer = "Goldcoders Corp <support@goldcoders.dev>"',
            self.desktop_manifest,
        )
        self.assertIn("dpkg-deb --extract", self.linux_build_script)
        self.assertIn("tar -xzf", self.linux_build_script)
        self.assertIn(
            '--package-root "$deb_root"',
            self.linux_build_script,
        )
        self.assertIn(
            '--package-root "$rpm_root"',
            self.linux_build_script,
        )
        self.assertIn("--from-dir /packages", self.linux_smoke_script)
        self.assertIn("SHA256SUMS-linux-${ARCH}.txt", self.linux_install_script)
        self.assertIn("ebirforms-bin.PKGBUILD", self.linux_build_script)
        self.assertIn("https://github.com/hexuria/buwiz-forms/releases/download/v", self.linux_build_script)
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
