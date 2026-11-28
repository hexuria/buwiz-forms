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
LINUX_BUILD_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/build-packages.sh"
LINUX_SMOKE_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/smoke-test.sh"
LINUX_INSTALL_SCRIPT_PATH = REPOSITORY_ROOT / "packaging/linux/install-linux.sh"
JUSTFILE_PATH = REPOSITORY_ROOT / "justfile"
LINUX_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-linux.sh"
WINDOWS_SETUP_PATH = REPOSITORY_ROOT / "scripts/setup-windows.ps1"
FINAL_SETUP_AUDIT_PATH = REPOSITORY_ROOT / "scripts/audit_final_setup.ps1"
PACKAGE_MSIX_PATH = REPOSITORY_ROOT / "scripts/package_msix.ps1"
NSH_WINDOWS_PACKAGE_PATH = REPOSITORY_ROOT / "scripts/package_nsh_windows.ps1"
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
        cls.linux_build_script = LINUX_BUILD_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.linux_smoke_script = LINUX_SMOKE_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.linux_install_script = LINUX_INSTALL_SCRIPT_PATH.read_text(encoding="utf-8")
        cls.justfile = JUSTFILE_PATH.read_text(encoding="utf-8")
        cls.linux_setup = LINUX_SETUP_PATH.read_text(encoding="utf-8")
        cls.windows_setup = WINDOWS_SETUP_PATH.read_text(encoding="utf-8")
        cls.final_setup_audit = FINAL_SETUP_AUDIT_PATH.read_text(encoding="utf-8")
        cls.package_msix = PACKAGE_MSIX_PATH.read_text(encoding="utf-8")
        cls.nsh_windows_package = NSH_WINDOWS_PACKAGE_PATH.read_text(encoding="utf-8")
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
        variable = "NSH_VERSION" if workflow == self.workflow else "CARGO_VERSION"
        self.assertIn(f'"$TAG_VERSION" != "${variable}"', preflight)
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

    def test_nsh_signed_updates_and_dispatch_safety(self) -> None:
        self.assertIn("workflow_dispatch:", self.workflow)
        self.assertIn("nsh/nsh.json", self.job("preflight"))
        self.assertIn("github.event_name == 'push'", self.job("release"))
        for workflow, job in ((self.workflow, "release"), (self.macos_workflow, "publish")):
            publish = self.job(job, workflow)
            self.assertIn("secrets.NSH_UPDATE_KEY", publish)
            self.assertIn("hexuria/nsh", publish)
            self.assertIn("8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf", publish)
            self.assertIn("cargo install --locked --path", publish)
        # The NSH app reads forms/: a change there runs its type check and tests (scripts/ci_plan.py decides for
        # both ci.yml and nsh.yml).
        plan = (REPOSITORY_ROOT / "scripts/ci_plan.py").read_text()
        self.assertIn("^(nsh/|forms/|", plan)

    @unittest.skipIf(sys.platform == "win32", "release preflight runs on Ubuntu")
    def test_nsh_version_preflight_accepts_dispatch_and_rejects_bad_tags(self) -> None:
        step = self.job("preflight").split("        run: |\n", 1)[1].split("\n      - name:", 1)[0]
        script = "\n".join(line[10:] for line in step.splitlines())
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "nsh").mkdir()
            config = root / "nsh/nsh.json"
            config.write_text(json.dumps({"version": "0.2.0"}))
            git = root / "git"
            git.write_text('#!/bin/sh\ncase "$1" in\nrev-parse) echo abc;;\ntag) echo "$GITHUB_REF_NAME";;\nesac\n')
            git.chmod(0o755)
            env = dict(os.environ, PATH=f"{root}{os.pathsep}{os.environ['PATH']}",
                       GITHUB_EVENT_NAME="push", GITHUB_REF_TYPE="tag", GITHUB_SHA="abc",
                       GITHUB_OUTPUT=str(root / "output"))
            def check(tag, event="push"):
                return subprocess.run(["bash", "-e", "-o", "pipefail", "-c", script],
                                      cwd=root, env=dict(env, GITHUB_REF_NAME=tag, GITHUB_EVENT_NAME=event),
                                      capture_output=True, text=True)
            self.assertEqual(check("v0.2.0").returncode, 0)
            self.assertEqual(check("feat/nsh-release-g", "workflow_dispatch").returncode, 0)
            self.assertIn("does not match nsh/nsh.json", check("v0.1.0").stderr)
            self.assertIn("exact vMAJOR.MINOR.PATCH", check("v0.2.0-test").stderr)
            config.write_text(json.dumps({"version": ""}))
            self.assertNotEqual(check("v0.2.0").returncode, 0)
            config.write_text("not json")
            self.assertNotEqual(check("v0.2.0").returncode, 0)

    def test_every_publishable_build_depends_on_exact_tag_preflight(self) -> None:
        self.assert_preflight_gates(self.workflow)
        preflight = self.job("preflight")
        self.assertIn('"nsh/nsh.json"', preflight)
        self.assertIn('"$TAG_VERSION" != "$NSH_VERSION"', preflight)
        for job_name in ("build-windows", "build-linux"):
            self.assertIn("needs: preflight", self.job(job_name))
        self.assertIn("needs: preflight", self.job("build-windows-store"))
        self.assertIn(
            "needs: [preflight, build-windows, build-windows-store, build-linux]",
            self.job("release"),
        )
        self.assertNotIn("build-macos:", self.workflow)
        self.assertNotIn("*.dmg", self.workflow)

    def test_macos_workflow_preflight_and_publish_shape(self) -> None:
        self.assert_preflight_gates(self.macos_workflow)
        # A pull request never starts a GitHub macOS job by itself (docs/CI.md).
        self.assertNotIn("pull_request", self.macos_workflow)
        self.assertNotIn("pull_request", self.linux_release_workflow)
        self.assertIn("scripts/package_macos_release.sh", self.macos_workflow)
        preflight = self.job("preflight", self.macos_workflow)
        self.assertIn("NSH_VERSION=", preflight)
        self.assertIn('json.load(open(sys.argv[1], encoding="utf-8"))', preflight)
        self.assertNotIn('NSH_VERSION="$(awk', preflight)
        self.assertIn('"$TAG_VERSION" != "$NSH_VERSION"', preflight)
        self.assertIn("nsh/nsh.json version $NSH_VERSION", preflight)
        self.assertIn("Cargo.lock package-lock.json nsh/Cargo.lock", preflight)
        self.assertEqual(self.justfile.count("scripts/set_release_version.py"), 2)
        build_macos = self.job("build-macos", self.macos_workflow)
        self.assertIn("needs: preflight", build_macos)
        # Tag and manual runs only, so the old fork-pull-request skip is gone with the trigger.
        self.assertNotIn("\n    if:", build_macos.split("    steps:", 1)[0])
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
        self.assertIn("nsh package --target macos --arch universal", script)
        self.assertIn('"$APP/Contents/Resources/AppIcon.icns"', script)
        self.assertIn(
            "codesign --verify --deep --strict --verbose=2 \"$APP\"", script
        )
        # Update archives are made again from the signed, stapled app.
        self.assertLess(
            script.index('xcrun stapler staple "$APP"'),
            script.index('tar -czf "$OUT_DIR/$archive"'),
        )
        self.assertNotIn("crates/bir-shell", macos + script)
        self.assertIn("buwiz-forms/target/release-artifacts/*.tar.gz", macos)
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

    def test_windows_artifacts_are_audited_and_signed_when_configured(self) -> None:
        windows = self.windows_job("installers")
        script = self.nsh_windows_package
        for secret in ("WINDOWS_CERTIFICATE_PFX", "WINDOWS_CERTIFICATE_PASSWORD"):
            self.assertIn(f"inputs.release && !inputs.store && secrets.{secret}", windows)
        self.assertIn("$hasPfx -xor $hasPassword", script)
        self.assertIn("::warning title=Unsigned Windows artifacts::", script)
        self.assertIn("sign /fd SHA256 /td SHA256", script)
        self.assertIn("verify /pa /all /v", script)
        self.assertLess(script.index("Sign-WindowsArtifact $exe"), script.index("& $iscc /Q"))
        self.assertLess(script.index("audit_installer_sources.py"), script.index("& $iscc /Q"))
        self.assertIn("audit_final_setup.ps1') -Installer $setup -Nsh", script)
        self.assertIn("Start-Process msiexec.exe", script)
        self.assertIn("makeappx.exe') unpack", script)
        self.assertIn("audit_no_legacy.py", script)
        self.assertNotIn("7z", script)
        self.assertLess(script.index("CreateFromDirectory"), script.index("New-Item -ItemType File -Path (Join-Path $stage $marker)"))

        self.assertIn("if-no-files-found: error", windows)
        upload = windows.split("- name: Upload Windows Artifact", maxsplit=1)[1]
        for artifact in ("*-Setup.exe", "*.msi", "*.msix", "*-windows-x86_64.zip", "SHA256SUMS-windows-x64.txt"):
            self.assertIn(f"target/release-artifacts/{artifact}", upload)

        public_windows_job = self.job("build-windows")
        self.assertIn("uses: ./.github/workflows/release-windows.yml", public_windows_job)
        self.assertIn("release: ${{ github.event_name == 'push' }}", public_windows_job)
        self.assertIn("secrets: inherit", public_windows_job)
        self.assertIn("needs: preflight", public_windows_job)

        public_release = self.job("release")
        for artifact in ("*-Setup.exe", "*.msi", "*.msix", "*-windows-x86_64.zip", "SHA256SUMS-windows-*.txt"):
            self.assertIn(f"artifacts/**/{artifact}", public_release)

    def test_the_store_msix_gets_the_google_client_only_with_store_gmail(self) -> None:
        store = self.job("build-windows-store")
        self.assertIn("uses: ./.github/workflows/release-windows.yml", store)
        self.assertIn("store: true", store)
        self.assertIn("release: ${{ github.event_name == 'push' }}", store)
        # Named secrets, never inherit: no signing certificate, and the client only behind the variable.
        self.assertNotIn("secrets: inherit", store)
        self.assertNotIn("WINDOWS_CERTIFICATE", store)
        for secret in ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"):
            self.assertIn(f"{secret}: ${{{{ vars.STORE_GMAIL == '1' && secrets.{secret} || '' }}}}", store)
        # The direct downloads keep what 0.2.x shipped.
        self.assertIn("secrets: inherit", self.job("build-windows"))
        self.assertNotIn("store:", self.job("build-windows"))

        windows = self.windows_job("installers")
        for secret in ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"):
            self.assertIn(
                f"{secret}: ${{{{ inputs.release && (!inputs.store || vars.STORE_GMAIL == '1') && secrets.{secret} || '' }}}}",
                windows,
            )
        for secret in ("WINDOWS_CERTIFICATE_PFX", "WINDOWS_CERTIFICATE_PASSWORD"):
            self.assertIn(f"inputs.release && !inputs.store && secrets.{secret}", windows)
        self.assertIn("-MsixOnly:$${{ inputs.store }} -NoMsix:$${{ inputs.release && !inputs.store }}", windows)

        check = windows.split("- name: Check the Store package against STORE_GMAIL", maxsplit=1)[1]
        check = check.split("- name:", maxsplit=1)[0]
        self.assertIn("if: ${{ inputs.store }}", check)
        # The same condition that gave the build the client decides which way the package is checked.
        self.assertIn("GMAIL_BUILT_IN: ${{ inputs.release && vars.STORE_GMAIL == '1' }}", check)
        self.assertIn("{ '--present' } else { '--absent' }", check)
        self.assertLess(check.index("audit_google_client.py --self-test"), check.index("audit_google_client.py $expect"))
        self.assertLess(
            windows.index("- name: Package and audit the NSH Windows installers"),
            windows.index("- name: Check the Store package against STORE_GMAIL"),
        )
        store_upload = windows.split("- name: Upload the Store package", maxsplit=1)[1]
        self.assertIn("if: ${{ inputs.store }}", store_upload)
        self.assertIn("name: windows-store", store_upload)
        self.assertIn("SHA256SUMS-windows-store-x64.txt", store_upload)
        self.assertIn("SHA256SUMS-windows-store-x64.txt", self.nsh_windows_package)
        self.assertIn("artifacts/**/SHA256SUMS-windows-*.txt", self.job("release"))

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
        self.assertIn("ebirforms-no-self-update", self.package_msix)

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
        check = self.job("check", self.ci_workflow)
        # `changes` picks the systems (scripts/ci_plan.py); the test lane can run all three.
        plan = (REPOSITORY_ROOT / "scripts/ci_plan.py").read_text()
        self.assertIn('"macos": {"os": "macos-15", "target": "aarch64-apple-darwin"}', plan)
        self.assertIn('"linux": {"os": "ubuntu-22.04", "target": "x86_64-unknown-linux-gnu"}', plan)
        self.assertIn('"windows": {"os": "windows-latest", "target": "x86_64-pc-windows-msvc"}', plan)
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
        setup = self.job("windows-setup", self.ci_workflow)
        self.assertIn("uses: ./.github/workflows/release-windows.yml", setup)
        self.assertNotIn("secrets: inherit", setup)
        self.assertIn("needs.changes.outputs.installer", setup)
        self.assertNotIn("check_pe_stack_reserve.py", check)

    def test_setup_scripts_keep_the_gpui_agent_version_pinned(self) -> None:
        self.assertIn('prebuilt_pin GPUI_AGENT_CLI_VERSION', self.linux_setup)
        self.assertIn(
            "GPUI_AGENT_CLI_VERSION not found",
            self.linux_setup,
        )
        self.assertIn('prebuilt_report_pinned gpui-agent gpui-agent-cli gpui-agent', self.linux_setup)
        self.assertIn(
            'install_pinned_tool gpui-agent-cli "$version"', self.linux_setup
        )
        self.assertNotIn("gpui-agent?rev=", self.linux_setup)
        self.assertIn(
            "cargo build --locked -p bir-cli",
            self.linux_setup,
        )
        self.assertNotIn("--locked gpui-agent-cli", self.linux_setup)
        self.assertNotIn("installing from main", self.linux_setup)
        self.assertIn("Read-PrebuiltPin 'GPUI_AGENT_CLI_VERSION'", self.windows_setup)
        self.assertRegex(
            self.windows_setup,
            r"Install-PinnedTool\s+'gpui-agent-cli'\s+\$gpuiVersion",
        )
        self.assertIn("3010", self.windows_setup)



if __name__ == "__main__":
    unittest.main()
