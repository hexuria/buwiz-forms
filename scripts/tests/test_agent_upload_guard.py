"""An agent build is never uploaded to App Store Connect by accident (scripts/agent_upload_guard.py).

The guard reads the uploaded artifact itself: its Info.plist mark, its executable (the agent's marker string), on the
Mac its signed entitlements (network.server without STORE_GMAIL=1), and its provenance. Only `just testflight-agent`
lets an agent build up, through a recipe argument; ALLOW_AGENT_UPLOAD in the environment is ignored.
"""

import os
import plistlib
import re
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

import agent_upload_guard as guard  # noqa: E402

JUSTFILE = (ROOT / "justfile").read_text(encoding="utf-8")
GUARD = ROOT / "scripts/agent_upload_guard.py"
STORE = {"CFBundleExecutable": "eBIRForms"}
AGENT = {"CFBundleExecutable": "eBIRForms", "NSHAgentBuild": True}
PLAIN = b"\x00the app\x00"
WITH_DRIVER = b"\x00the app\x00" + guard.AGENT_MARKER + b"\x00"
SANDBOX = {"com.apple.security.app-sandbox": True, "com.apple.security.network.client": True}
LISTENS = {**SANDBOX, "com.apple.security.network.server": True}


def recipe(name):
    match = re.search(rf"^{re.escape(name)}[ :].*?(?=^\S)", JUSTFILE, re.S | re.M)
    assert match, f"no recipe {name}"
    return match.group(0)


def fake_ipa(folder: Path, info: dict, executable: bytes) -> Path:
    ipa = folder / "eBIRForms-iOS-1.0.0.ipa"
    with zipfile.ZipFile(ipa, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("Payload/eBIRForms.app/Info.plist", plistlib.dumps(info))
        z.writestr("Payload/eBIRForms.app/eBIRForms", executable)
    return ipa


class VerdictTests(unittest.TestCase):
    def test_a_store_build_passes(self):
        self.assertEqual(guard.verdict("mac", STORE, PLAIN, SANDBOX, "agent=0", {}, False), (True, []))
        self.assertEqual(guard.verdict("ios", STORE, PLAIN, None, None, {}, False), (True, []))

    def test_each_sign_alone_refuses_a_mac_upload(self):
        cases = {
            "mark": (AGENT, PLAIN, SANDBOX, None),
            "marker string": (STORE, WITH_DRIVER, SANDBOX, None),
            "network.server": (STORE, PLAIN, LISTENS, None),
            "provenance": (STORE, PLAIN, SANDBOX, "build 31, agent=1 (agent build: internal TestFlight only, never for review)"),
            "unreadable entitlements": (STORE, PLAIN, None, None),
        }
        for name, (info, exe, ent, prov) in cases.items():
            with self.subTest(sign=name):
                ok, signs = guard.verdict("mac", info, exe, ent, prov, {}, False)
                self.assertFalse(ok)
                self.assertEqual(len(signs), 1, signs)

    def test_network_server_with_gmail_on_is_a_gmail_build_not_an_agent_build(self):
        # The positive control of the entitlement check: the same entitlement passes once STORE_GMAIL=1 explains it.
        self.assertEqual(guard.verdict("mac", STORE, PLAIN, LISTENS, None, {"STORE_GMAIL": "1"}, False), (True, []))
        self.assertFalse(guard.verdict("mac", STORE, PLAIN, LISTENS, None, {"STORE_GMAIL": "true"}, False)[0])

    def test_allow_agent_lets_a_mac_agent_build_up_but_never_an_ios_one(self):
        ok, signs = guard.verdict("mac", AGENT, WITH_DRIVER, LISTENS, "agent=1", {}, True)
        self.assertTrue(ok)
        self.assertEqual(len(signs), 4)
        self.assertFalse(guard.verdict("ios", AGENT, WITH_DRIVER, None, None, {}, True)[0])


class CommandLineTests(unittest.TestCase):
    def run_guard(self, *args, env=None):
        return subprocess.run([sys.executable, str(GUARD), *args], capture_output=True, text=True,
                              env={**os.environ, **(env or {})})

    def test_an_ios_agent_build_is_refused_even_with_the_permission_exported(self):
        with tempfile.TemporaryDirectory() as tmp:
            for info, exe in ((AGENT, PLAIN), (STORE, WITH_DRIVER)):
                ipa = fake_ipa(Path(tmp), info, exe)
                out = self.run_guard("check", "ios", str(ipa), env={"ALLOW_AGENT_UPLOAD": "1"})
                self.assertEqual(out.returncode, 1, out.stderr)
                self.assertTrue(out.stderr.startswith("FAIL "), out.stderr)

    def test_a_clean_ipa_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.run_guard("check", "ios", str(fake_ipa(Path(tmp), STORE, PLAIN)))
            self.assertEqual(out.returncode, 0, out.stderr)

    def test_the_permission_is_a_flag_and_only_for_the_mac(self):
        with tempfile.TemporaryDirectory() as tmp:
            ipa = fake_ipa(Path(tmp), AGENT, PLAIN)
            self.assertEqual(self.run_guard("check", "ios", str(ipa), "--allow-agent").returncode, 2)

    def test_an_unreadable_artifact_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            broken = Path(tmp) / "eBIRForms-iOS-1.0.0.ipa"
            broken.write_bytes(b"not a zip")
            out = self.run_guard("check", "ios", str(broken))
            self.assertEqual(out.returncode, 1)
            self.assertIn("could not be read", out.stderr)

    @unittest.skipUnless(sys.platform == "darwin", "reads a .app's signature with codesign")
    def test_a_mac_app_with_the_permission_exported_is_still_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = Path(tmp) / "eBIRForms.app"
            (app / "Contents" / "MacOS").mkdir(parents=True)
            (app / "Contents" / "Info.plist").write_bytes(plistlib.dumps(AGENT))
            (app / "Contents" / "MacOS" / "eBIRForms").write_bytes(PLAIN)
            out = self.run_guard("check", "mac", str(app), env={"ALLOW_AGENT_UPLOAD": "1"})
            self.assertEqual(out.returncode, 1, out.stderr)


class RecipeTests(unittest.TestCase):
    def test_only_an_agent_build_is_marked_before_signing_and_its_provenance_says_so(self):
        body = recipe("_package-mac-appstore")
        self.assertIn('if [ "$NSH_MAS_AGENT" = "1" ]; then\n        /usr/libexec/PlistBuddy -c "Add :NSHAgentBuild bool true" "$PLIST"', body)
        self.assertLess(body.index("NSHAgentBuild"), body.index("codesign --force"))
        self.assertIn("never for review", body)
        self.assertIn('agent=$NSH_MAS_AGENT$agent_note" > "${PKG_PATH%.pkg}.provenance.txt"', body)

    def test_the_mac_upload_reads_the_pkg_and_takes_the_permission_only_as_an_argument(self):
        body = recipe("testflight-upload")
        self.assertTrue(body.startswith('testflight-upload allow_agent="":'))
        self.assertEqual(body.count("unset ALLOW_AGENT_UPLOAD"), 2)
        self.assertLess(body.index("source .env"), body.rindex("unset ALLOW_AGENT_UPLOAD"))
        check = body.index('scripts/agent_upload_guard.py check mac "$PKG_PATH"')
        self.assertLess(check, body.index('"$TRANSPORTER" -m upload'))
        self.assertIn('[ "{{allow_agent}}" = "yes" ] && allow_flag="--allow-agent"', body)
        self.assertNotIn("${ALLOW_AGENT_UPLOAD", body)

    def test_only_testflight_agent_passes_the_permission(self):
        self.assertIn("just testflight-upload yes", recipe("testflight-agent"))
        callers = [line.strip() for line in JUSTFILE.splitlines() if "testflight-upload yes" in line]
        self.assertEqual(callers, ["just testflight-upload yes"])

    def test_ios_store_builds_drop_an_inherited_cargo_and_refuse_the_driver(self):
        build = recipe("ios-appstore")
        self.assertIn("unset CARGO", build)
        self.assertLess(build.index("unset CARGO"), build.index("nsh package"))
        self.assertLess(build.index('agent_upload_guard.py check ios "$APP"'), build.rindex("codesign --force"))
        upload = recipe("ios-appstore-upload")
        self.assertLess(upload.index('agent_upload_guard.py check ios "$IPA"'), upload.index("publish ios"))


if __name__ == "__main__":
    unittest.main()
