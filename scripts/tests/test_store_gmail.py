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

    @unittest.skipIf(
        sys.platform == "win32",
        "the recipes that eval this line run only on macOS (Mac App Store and iPhone packages); on Windows bash would "
        "also get print's \\r\\n line endings",
    )
    def test_off_drops_both_variables_in_the_shell(self):
        env = {**os.environ, "GOOGLE_CLIENT_ID": "x", "GOOGLE_CLIENT_SECRET": "y", "STORE_GMAIL": ""}
        script = f'eval "$({sys.executable} {ROOT / "scripts/store_gmail.py"} env)"; echo "id=${{GOOGLE_CLIENT_ID-unset}} secret=${{GOOGLE_CLIENT_SECRET-unset}}"'
        out = subprocess.run(["bash", "-c", script], env=env, capture_output=True, text=True, check=True)
        self.assertEqual(out.stdout.strip(), "id=unset secret=unset")
        self.assertIn("without Gmail Connect", out.stderr)

    @unittest.skipIf(
        sys.platform == "win32",
        "the recipes that eval this line run only on macOS (Mac App Store and iPhone packages); on Windows bash would "
        "also get print's \\r\\n line endings",
    )
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

    def test_no_release_build_gets_the_google_client_without_the_switch(self):
        # The one place the rule is stated (owner, 2026-11-28: 1.0.0 ships no Google client in any build). Every
        # reference to the client's repository secrets in any workflow either passes it behind STORE_GMAIL == '1' or only
        # checks that both are set (HAS_CLIENT); no build path, Store or direct, on any platform, gets it ungated. The
        # direct downloads never run store_gmail.py: they are checked by audit_google_client.py
        # (test_release_workflow_policy.py checks those steps).
        package = (ROOT / "scripts/package_macos_release.sh").read_text(encoding="utf-8")
        self.assertNotIn("store_gmail", package)
        self.assertNotIn("STORE_GMAIL", package)
        # Case-insensitive: GitHub's secret names and Windows environment names both ignore case.
        gated = re.compile(r"vars\.STORE_GMAIL == '1' && secrets\.GOOGLE_CLIENT_(ID|SECRET) \|\| ''", re.IGNORECASE)
        presence_check = "HAS_CLIENT: ${{ secrets.GOOGLE_CLIENT_ID != '' && secrets.GOOGLE_CLIENT_SECRET != '' }}"
        reference = re.compile(r"secrets\s*(\.|\[\s*['\"])GOOGLE_CLIENT_", re.IGNORECASE)
        gated_count = {}
        for path in sorted((ROOT / ".github/workflows").glob("*.y*ml")):
            text = path.read_text(encoding="utf-8")
            gated_count[path.name] = 0
            for number, line in enumerate(text.splitlines(), 1):
                if not reference.search(line):
                    continue
                with self.subTest(workflow=path.name, line=number):
                    if presence_check in line:
                        continue
                    self.assertEqual(len(reference.findall(line)), 1, line)
                    self.assertRegex(line, gated)
                    gated_count[path.name] += 1
        for name in ("release.yml", "release-macos.yml", "release-windows.yml", "release-linux.yml"):
            with self.subTest(workflow=name):
                # The detector's positive control: each release workflow does pass the client, behind the switch.
                self.assertEqual(gated_count.get(name, 0) % 2, 0)
                self.assertGreaterEqual(gated_count.get(name, 0), 2)
                self.assertNotIn("store_gmail.py", (ROOT / ".github/workflows" / name).read_text(encoding="utf-8"))
        # bir-core reads the ENV names, so whatever reaches GOOGLE_CLIENT_ID / GOOGLE_CLIENT_SECRET is checked too, not
        # only the secrets' names: every assignment is the same-named secret behind the switch (or a key with no value,
        # as in workflow_call's list of accepted secrets), never another secret, env.X or a literal; no step writes them
        # into a shell or $GITHUB_ENV; and no workflow reaches the secrets in a way these patterns cannot see.
        # Anywhere on the line (a flow mapping `env: { GOOGLE_CLIENT_ID: ... }` too), in any case (Windows matches
        # environment names without case, so `google_client_id:` reaches option_env!("GOOGLE_CLIENT_ID")).
        assignment = re.compile(r"(GOOGLE_CLIENT_(?:ID|SECRET))\s*:", re.IGNORECASE)
        hidden = (
            re.compile(r"toJSON\(\s*secrets\s*\)", re.IGNORECASE),
            re.compile(r"secrets\s*\[\s*(?!['\"])"),
        )
        assignments = 0
        for path in sorted((ROOT / ".github/workflows").glob("*.y*ml")):
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                with self.subTest(workflow=path.name, line=number):
                    for pattern in hidden:
                        self.assertIsNone(pattern.search(line), line)
                    self.assertNotRegex(line, re.compile(r"GOOGLE_CLIENT_(ID|SECRET)\s*=", re.IGNORECASE))
                    for match in assignment.finditer(line):
                        name = match.group(1).upper()
                        value = line[match.end() :].strip()
                        if value == "" and line[: match.start()].strip() == "":
                            continue  # a key with no value: workflow_call's list of accepted secrets
                        allowed = (
                            f"${{{{ vars.STORE_GMAIL == '1' && secrets.{name} || '' }}}}",
                            f"${{{{ inputs.release && vars.STORE_GMAIL == '1' && secrets.{name} || '' }}}}",
                            "''",
                        )
                        # Nothing may follow the allowed value but the flow mapping's closing braces or, after a comma,
                        # the next key (checked on its own if it is one of these names).
                        after = r"\s*(?:\}\s*)*(?:,\s*[A-Za-z_][\w-]*\s*:.*)?"
                        self.assertTrue(
                            any(re.fullmatch(re.escape(candidate) + after, value, re.IGNORECASE) for candidate in allowed),
                            line,
                        )
                        assignments += 1
        # The detector's positive control: it saw the assignments the release workflows make.
        self.assertGreaterEqual(assignments, 10)
        # The Store job keeps its shape: named secrets, never inherit, no signing certificate.
        release = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        store = release.split("  build-windows-store:", 1)[1].split("\n  build-linux:", 1)[0]
        self.assertIn("store: true", store)
        self.assertNotIn("secrets: inherit", store)
        self.assertNotIn("WINDOWS_CERTIFICATE", store)
        self.assertIn("NSH_REPO_TOKEN: ${{ secrets.NSH_REPO_TOKEN }}", store)

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

    def test_a_refused_archive_inside_a_zip_is_refused(self):
        # A: a .pkg or .tar.gz inside a zip is not read as empty.
        for inner in ("inner.pkg", "x.tar.gz"):
            with self.subTest(inner=inner), tempfile.TemporaryDirectory() as tmp:
                archive = Path(tmp) / "eBIRForms.zip"
                with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as z:
                    z.writestr(inner, b"compressed bytes")
                ok, message = store_gmail.check(archive, {})
                self.assertFalse(ok)
                self.assertIn(f"eBIRForms.zip!{inner}", message)
                self.assertIn("cannot read inside", message)

    def test_a_file_that_looks_like_a_zip_but_breaks_is_refused(self):
        # B: a truncated deflated zip is not passed after the raw scan.
        with tempfile.TemporaryDirectory() as tmp:
            whole = Path(tmp) / "whole.zip"
            with zipfile.ZipFile(whole, "w", zipfile.ZIP_DEFLATED) as z:
                z.writestr("Payload/eBIRForms.app/eBIRForms", FAKE_ID * 50)
            broken = Path(tmp) / "eBIRForms.ipa"
            broken.write_bytes(whole.read_bytes()[: len(whole.read_bytes()) // 2])
            ok, message = store_gmail.check(broken, {})
            self.assertFalse(ok)
            self.assertIn("looks like a zip but cannot be read", message)
            not_a_zip = Path(tmp) / "renamed.zip"
            not_a_zip.write_bytes(b"just bytes")
            self.assertFalse(store_gmail.check(not_a_zip, {})[0])

    def test_an_unreadable_member_is_a_clean_failure_not_a_traceback(self):
        # C: a member that cannot be read (here a damaged entry: its bytes no longer match its CRC) ends in a FAIL line.
        with tempfile.TemporaryDirectory() as tmp:
            archive = Path(tmp) / "eBIRForms.ipa"
            payload = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ" * 4
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_STORED) as z:
                z.writestr("Payload/eBIRForms.app/eBIRForms", payload)
            raw = archive.read_bytes()
            at = raw.index(payload)
            archive.write_bytes(raw[:at] + b"Z" + raw[at + 1:])
            out = subprocess.run([sys.executable, str(ROOT / "scripts/store_gmail.py"), "check", str(archive)],
                                 capture_output=True, env={**os.environ, "STORE_GMAIL": ""})
            self.assertEqual(out.returncode, 1)
            self.assertTrue(out.stderr.startswith(b"FAIL "), out.stderr)
            self.assertNotIn(b"Traceback", out.stderr)
            self.assertIn(b"cannot be read", out.stderr)

    def test_the_last_line_names_what_was_not_read_inside(self):
        with tempfile.TemporaryDirectory() as tmp:
            app = fake_app(Path(tmp))
            (app / "Contents" / "Resources").mkdir()
            (app / "Contents" / "Resources" / "Assets.car").write_bytes(b"lzfse")
            (app / "Contents" / "Resources" / "icon.png").write_bytes(b"png")
            ok, message = store_gmail.check(app, {})
            self.assertTrue(ok)
            last = message.splitlines()[-1]
            self.assertTrue(last.startswith("Scanned 4 files"), last)
            self.assertIn(".car (1)", last)
            self.assertIn(".png (1)", last)

    def test_the_command_line_prints_plain_ascii(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = subprocess.run([sys.executable, str(ROOT / "scripts/store_gmail.py"), "check", str(fake_app(Path(tmp)))],
                                 capture_output=True, env={**os.environ, "STORE_GMAIL": ""})
            self.assertEqual(out.returncode, 0)
            out.stderr.decode("ascii")
            self.assertTrue(out.stderr.startswith(b"OK "))

if __name__ == "__main__":
    unittest.main()
