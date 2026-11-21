import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest


SCRIPT = (Path(__file__).resolve().parents[2] / "scripts/sign-in-android-windows.ps1").read_text()
FUNCTION = SCRIPT[SCRIPT.index("function Get-AccountUiState"):SCRIPT.index("$adb =")]
PACKAGE = "dev.goldcoders.bir.nsh"


@unittest.skipUnless(os.name == "nt" and shutil.which("powershell"), "Windows PowerShell required")
class AndroidSignInTests(unittest.TestCase):
    def test_emulator_lock_screen_is_disabled_before_dismissal(self):
        block = SCRIPT[SCRIPT.index('& $adb -s $Serial shell locksettings'):SCRIPT.index('$deadline =')]
        for code in (0, 1):
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Fake-Adb { $args -join ' '; $global:LASTEXITCODE=[int]$env:TEST_EXIT }; $adb='Fake-Adb'; $Serial='emulator-5554'; " + block],
                env={**os.environ, 'TEST_EXIT': str(code)}, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, code, result.stderr)
            if code:
                self.assertIn('Could not disable the development emulator lock screen', result.stderr)
                self.assertNotIn('dismiss-keyguard', result.stdout)
            else:
                self.assertIn('KEYCODE_WAKEUP', result.stdout)
                self.assertIn('dismiss-keyguard', result.stdout)

    def test_system_ui_anr_waits_instead_of_closing_shell(self):
        block = SCRIPT[SCRIPT.index('        $systemUiAnr ='):SCRIPT.index("        } elseif ($state.State")]+ '        }'
        for button, bounds, tapped in (('aerr_wait', '[100,200][300,400]', True),
                                       ('aerr_close', '[100,200][300,400]', False),
                                       ('aerr_wait', 'invalid', False), ('', '', False)):
            xml = '<hierarchy><node text="System UI isn\'t responding" />' + f'<node resource-id="android:id/{button}" bounds="{bounds}" /></hierarchy>'
            result = subprocess.run(
                ['powershell', '-NoProfile', '-NonInteractive', '-Command',
                 "$ErrorActionPreference='Stop'; function Fake-Adb { $args -join ' ' }; $adb='Fake-Adb'; $Serial='emulator-5554'; [xml]$ui=$env:TEST_UI; " + block],
                env={**os.environ, 'TEST_UI': xml}, text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual('shell input tap 200 300' in result.stdout, tapped)

    def parse(self, xml):
        return subprocess.run(
            ["powershell", "-NoProfile", "-NonInteractive", "-Command",
             "$ErrorActionPreference='Stop'; " + FUNCTION +
             "Get-AccountUiState ([xml]$env:TEST_UI) | ConvertTo-Json -Compress"],
            env={**os.environ, "TEST_UI": xml}, text=True, capture_output=True,
        )

    def test_gate_uses_accessible_button_bounds(self):
        result = self.parse(f'<hierarchy><node package="{PACKAGE}" text="Welcome to eBIRForms" />'
                            f'<node package="{PACKAGE}" content-desc="Sign in" clickable="true" '
                            'enabled="true" bounds="[108,1226][972,1342]" /></hierarchy>')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), {"State": "gate", "X": 540, "Y": 1284})

    def test_already_signed_in(self):
        result = self.parse(f'<hierarchy><node package="{PACKAGE}" text="Who will use eBIRForms on this device?" /></hierarchy>')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["State"], "ready")

    def test_signed_in_empty_home_is_not_account_gate(self):
        fixture = Path(__file__).with_name('android_signed_in_home.xml').read_text()
        result = self.parse(fixture)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["State"], "ready")

    def test_gate_waiting_for_approval_is_not_ready(self):
        result = self.parse(f'<hierarchy><node package="{PACKAGE}" text="Welcome to eBIRForms" />'
                            f'<node package="{PACKAGE}" content-desc="Waiting…" enabled="false" /></hierarchy>')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["State"], "blocked")

    def test_empty_splash_and_other_apps_are_not_success(self):
        for xml in ('<hierarchy />', f'<hierarchy><node package="{PACKAGE}" text="" content-desc="" /></hierarchy>',
                    '<hierarchy><node package="com.android.chrome" text="Welcome" /></hierarchy>'):
            result = self.parse(xml)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)["State"], "blocked")

    def test_invalid_bounds_are_rejected(self):
        result = self.parse(f'<hierarchy><node package="{PACKAGE}" text="Welcome to eBIRForms" />'
                            f'<node package="{PACKAGE}" content-desc="Sign in" clickable="true" '
                            'enabled="true" bounds="invalid" /></hierarchy>')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Sign in button has invalid bounds", result.stderr)


if __name__ == "__main__":
    unittest.main()
