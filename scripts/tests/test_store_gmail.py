"""Gmail Connect in the store builds: off unless STORE_GMAIL=1 (scripts/store_gmail.py).

The switch drops the Google client id and secret from the store build's environment, a check after packaging proves the
built app holds neither (and, with the switch on, that the same detector finds both: the positive control), and the
Mac App Store entitlements carry network.server only when something in the build listens.
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

import store_gmail  # noqa: E402

JUSTFILE = (ROOT / "justfile").read_text(encoding="utf-8")
# Made-up values in Google's shapes, put together at run time so no secret scanner reads one in this file; no real id
# or secret is in this repository.
FAKE_ID = b"-".join([b"123456789012", b"abcdefghijklmnopqrstuvwxyz012345"]) + b".apps." + b"googleusercontent.com"
FAKE_SECRET = b"-".join([b"GOC" + b"SPX", b"AbCdEfGhIjKlMnOpQrStUvWxYz01"])


def recipe(name):
    """The body of a justfile recipe, from its header to the next recipe."""
    match = re.search(rf"^{re.escape(name)}[ :].*?(?=^\S)", JUSTFILE, re.S | re.M)
    assert match, f"no recipe {name}"
    return match.group(0)


def fake_app(root: Path, *payloads: bytes) -> Path:
    app = root / "eBIRForms.app"
    (app / "Contents" / "MacOS").mkdir(parents=True)
    (app / "Contents" / "MacOS" / "eBIRForms").write_bytes(b"\x00binary\x00" + b"\x00".join(payloads) + b"\x00end")
    (app / "Contents" / "Info.plist").write_bytes(b"<plist/>")
    return app


class SwitchTests(unittest.TestCase):
    def test_only_one_turns_it_on(self):
        for value, on in (("1", True), ("", False), ("0", False), ("true", False), ("yes", False)):
            with self.subTest(value=value):
                self.assertEqual(store_gmail.gmail_on({"STORE_GMAIL": value}), on)
        self.assertFalse(store_gmail.gmail_on({}))

    def test_off_drops_both_variables_in_the_shell(self):
        env = {**os.environ, "GOOGLE_CLIENT_ID": "x", "GOOGLE_CLIENT_SECRET": "y", "STORE_GMAIL": ""}
        script = f'eval "$({sys.executable} {ROOT / "scripts/store_gmail.py"} env)"; echo "id=${{GOOGLE_CLIENT_ID-unset}} secret=${{GOOGLE_CLIENT_SECRET-unset}}"'
        out = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True, check=True)
        self.assertEqual(out.stdout.strip(), "id=unset secret=unset")
        self.assertIn("without Gmail Connect", out.stderr)

    def test_on_keeps_both_variables(self):
        env = {**os.environ, "GOOGLE_CLIENT_ID": "x", "GOOGLE_CLIENT_SECRET": "y", "STORE_GMAIL": "1"}
        script = f'eval "$({sys.executable} {ROOT / "scripts/store_gmail.py"} env)"; echo "id=${{GOOGLE_CLIENT_ID-unset}} secret=${{GOOGLE_CLIENT_SECRET-unset}}"'
        out = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True, check=True)
        self.assertEqual(out.stdout.strip(), "id=x secret=y")

    def test_a_value_that_is_not_one_says_it_is_off(self):
        line, note = store_gmail.env_lines({"STORE_GMAIL": "true"})
        self.assertEqual(line, "unset GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET")
        self.assertIn("is not 1", note)


class CheckTests(unittest.TestCase):
    def test_off_passes_a_build_without_them(self):
        with tempfile.TemporaryDirectory() as tmp:
            ok, _ = store_gmail.check(fake_app(Path(tmp)), {})
            self.assertTrue(ok)

    def test_off_refuses_a_build_with_the_id_or_the_secret(self):
        for payload in (FAKE_ID, FAKE_SECRET):
            with self.subTest(payload=payload[:10]), tempfile.TemporaryDirectory() as tmp:
                ok, message = store_gmail.check(fake_app(Path(tmp), payload), {})
                self.assertFalse(ok)
                self.assertIn("Contents/MacOS/eBIRForms", message)

    def test_on_needs_both_in_the_build_the_positive_control(self):
        with tempfile.TemporaryDirectory() as tmp:
            ok, _ = store_gmail.check(fake_app(Path(tmp), FAKE_ID, FAKE_SECRET), {"STORE_GMAIL": "1"})
            self.assertTrue(ok)
        with tempfile.TemporaryDirectory() as tmp:
            ok, message = store_gmail.check(fake_app(Path(tmp), FAKE_ID), {"STORE_GMAIL": "1"})
            self.assertFalse(ok)
            self.assertIn("client secret", message)
        with tempfile.TemporaryDirectory() as tmp:
            ok, message = store_gmail.check(fake_app(Path(tmp)), {"STORE_GMAIL": "1"})
            self.assertFalse(ok)

    def test_messages_never_carry_a_value(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = fake_app(Path(tmp), FAKE_ID, FAKE_SECRET)
            for env in ({}, {"STORE_GMAIL": "1"}):
                _, message = store_gmail.check(app, env)
                self.assertNotIn(FAKE_ID.decode(), message)
                self.assertNotIn(FAKE_SECRET.decode(), message)

    def test_a_symlink_is_not_followed(self):
        with tempfile.TemporaryDirectory() as tmp:
            outside = Path(tmp) / "outside.bin"
            outside.write_bytes(FAKE_ID)
            app = fake_app(Path(tmp))
            (app / "Contents" / "link").symlink_to(outside)
            ok, _ = store_gmail.check(app, {})
            self.assertTrue(ok)


class EntitlementTests(unittest.TestCase):
    def written(self, env, agent):
        with tempfile.TemporaryDirectory() as tmp:
            dst = Path(tmp) / "store.entitlements"
            store_gmail.entitlements(ROOT / "entitlements.plist", dst, env, agent)
            with dst.open("rb") as handle:
                return plistlib.load(handle)

    def test_off_leaves_network_server_out_and_keeps_the_rest(self):
        with (ROOT / "entitlements.plist").open("rb") as handle:
            source = plistlib.load(handle)
        out = self.written({}, agent=False)
        self.assertNotIn("com.apple.security.network.server", out)
        self.assertEqual({k: v for k, v in source.items() if k != "com.apple.security.network.server"}, out)

    def test_gmail_or_the_agent_keeps_it(self):
        self.assertTrue(self.written({"STORE_GMAIL": "1"}, agent=False)["com.apple.security.network.server"])
        self.assertTrue(self.written({}, agent=True)["com.apple.security.network.server"])


class RecipeTests(unittest.TestCase):
    def test_both_store_recipes_drop_the_variables_and_check_the_app(self):
        for name in ("ios-appstore", "_package-mac-appstore"):
            with self.subTest(recipe=name):
                body = recipe(name)
                self.assertIn('drop="$({{PYTHON}} scripts/store_gmail.py env)"\n    eval "$drop"', body)
                self.assertNotIn('eval "$({{PYTHON}}', body)
                self.assertIn('{{PYTHON}} scripts/store_gmail.py check "$APP"', body)
                # The check runs on the finished bundle, before it is signed for the last time.
                self.assertLess(body.index("store_gmail.py check"), body.rindex("codesign --force"))

    def test_the_ios_recipe_drops_them_after_reading_env(self):
        body = recipe("ios-appstore")
        self.assertLess(body.index("source .env"), body.index("store_gmail.py env"))
        self.assertLess(body.index("store_gmail.py env"), body.index("nsh package"))

    def test_the_mac_store_package_signs_with_the_switched_entitlements(self):
        body = recipe("_package-mac-appstore")
        self.assertIn('scripts/store_gmail.py entitlements "$root/entitlements.plist" "$ENTITLEMENTS"', body)
        self.assertIn('--entitlements "$ENTITLEMENTS"', body)
        self.assertNotIn('cp "$root/entitlements.plist"', body)
        self.assertLess(body.index("store_gmail.py env"), body.index("nsh package"))

    def test_a_failing_switch_stops_the_build(self):
        # The recipes' two lines, with the script failing: set -e must stop before anything after them runs. (eval "$(...)"
        # would evaluate nothing, return 0 and go on with both GOOGLE_ values still set.)
        body = recipe("ios-appstore")
        lines = [line.strip() for line in body.splitlines() if "store_gmail.py env" in line or line.strip() == 'eval "$drop"']
        self.assertEqual(len(lines), 2, lines)
        script = "set -euo pipefail\nGOOGLE_CLIENT_ID=x\n" + "\n".join(lines).replace("{{PYTHON}}", "false &&") + "\necho went-on\n"
        out = subprocess.run(["bash", "-c", script], capture_output=True, text=True)
        self.assertNotEqual(out.returncode, 0)
        self.assertNotIn("went-on", out.stdout)

    def test_the_direct_downloads_are_untouched(self):
        # Only the store artifacts follow the switch; the direct downloads keep shipping Gmail Connect.
        package = (ROOT / "scripts/package_macos_release.sh").read_text(encoding="utf-8")
        self.assertNotIn("store_gmail", package)
        self.assertNotIn("STORE_GMAIL", package)
        workflows = ROOT / ".github/workflows"
        for name in ("release.yml", "release-macos.yml", "release-windows.yml", "release-linux.yml"):
            text = (workflows / name).read_text(encoding="utf-8")
            with self.subTest(workflow=name):
                self.assertNotIn("store_gmail", text)
                self.assertNotIn("STORE_GMAIL", text)
                self.assertIn("GOOGLE_CLIENT_ID: ${{", text)
        top = (workflows / "release.yml").read_text(encoding="utf-8").split("\njobs:", 1)[0]
        self.assertIn("  GOOGLE_CLIENT_ID: ${{ secrets.GOOGLE_CLIENT_ID }}", top)
        self.assertIn("  GOOGLE_CLIENT_SECRET: ${{ secrets.GOOGLE_CLIENT_SECRET }}", top)


class ArchiveTests(unittest.TestCase):
    def test_a_zip_archive_is_read_member_by_member(self):
        for suffix in (".ipa", ".aab", ".msix"):
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as tmp:
                archive = Path(tmp) / f"eBIRForms{suffix}"
                with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
                    z.writestr("Payload/eBIRForms.app/eBIRForms", b"\x00" + FAKE_ID + b"\x00" + FAKE_SECRET)
                ok, message = store_gmail.check(archive, {})
                self.assertFalse(ok)
                self.assertIn("!Payload/eBIRForms.app/eBIRForms", message)
                ok, _ = store_gmail.check(archive, {"STORE_GMAIL": "1"})
                self.assertTrue(ok)

    def test_a_clean_zip_archive_passes_with_the_switch_off(self):
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "eBIRForms.ipa"
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
                z.writestr("Payload/eBIRForms.app/eBIRForms", b"nothing here")
            self.assertTrue(store_gmail.check(archive, {})[0])

    def test_an_archive_it_cannot_read_is_refused_not_passed(self):
        for suffix in (".pkg", ".dmg", ".msixbundle"):
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as tmp:
                archive = Path(tmp) / f"eBIRForms{suffix}"
                archive.write_bytes(b"xar!compressed")
                for env in ({}, {"STORE_GMAIL": "1"}):
                    ok, message = store_gmail.check(archive, env)
                    self.assertFalse(ok)
                    self.assertIn("cannot read inside", message)

    def test_the_command_line_prints_plain_ascii(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = subprocess.run([sys.executable, str(ROOT / "scripts/store_gmail.py"), "check", str(fake_app(Path(tmp)))],
                                 capture_output=True, env={**os.environ, "STORE_GMAIL": ""})
            self.assertEqual(out.returncode, 0)
            out.stderr.decode("ascii")
            self.assertTrue(out.stderr.startswith(b"OK "))

if __name__ == "__main__":
    unittest.main()
