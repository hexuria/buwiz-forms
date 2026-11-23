"""Signed NSH update publishing."""
import importlib.util
import os
from pathlib import Path
import unittest
from unittest.mock import patch

PATH = Path(__file__).resolve().parents[1] / "publish_update_manifest.py"
SPEC = importlib.util.spec_from_file_location("publisher", PATH)
publisher = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(publisher)


class PublishTests(unittest.TestCase):
    def test_missing_key_does_not_contact_release(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(publisher, "run") as run:
            with patch("builtins.print") as output:
                publisher.publish("v0.2.0", "owner/repo")
            run.assert_not_called()
            self.assertIn("::warning::", output.call_args.args[0])

    def test_archive_names(self):
        for platform, ext in [("linux-x86_64", "tar.gz"), ("macos-aarch64", "tar.gz"), ("windows-x86_64", "zip")]:
            self.assertTrue(publisher.is_archive(f"eBIRForms-0.2.0-{platform}.{ext}", "0.2.0"))
        for name in ["other-0.2.0-linux-x86_64.tar.gz", "eBIRForms-Linux-x64-0.2.0.tar.gz",
                     "eBIRForms-0.1.0-linux-x86_64.tar.gz", "latest.json"]:
            self.assertFalse(publisher.is_archive(name, "0.2.0"))

    def test_signed_manifest_and_signature_uploaded_together(self):
        commands = []
        def run(args):
            commands.append(args)
            if args[:2] == ["gh", "api"]:
                return '[{"name":"eBIRForms-0.2.0-linux-x86_64.tar.gz"}]'
            if args[:2] == ["nsh", "update-manifest"]:
                key = Path(args[args.index("--key") + 1])
                self.assertEqual(key.read_text(), "throwaway")
                if os.name == "posix":
                    self.assertEqual(key.stat().st_mode & 0o777, 0o600)
            return ""
        with patch.dict(os.environ, {"NSH_UPDATE_KEY": "throwaway"}), patch.object(publisher, "run", side_effect=run):
            publisher.publish("v0.2.0", "owner/repo")
        manifest = next(c for c in commands if c[0] == "nsh")
        self.assertIn("--base-url", manifest)
        self.assertEqual(commands[-1][:3], ["gh", "release", "upload"])
        self.assertTrue(commands[-1][-2].endswith("latest.json"))
        self.assertTrue(commands[-1][-1].endswith("latest.json.sig"))
        self.assertFalse(Path(manifest[manifest.index("--key") + 1]).exists())

    def test_empty_feed_and_cli_failure_fail_closed(self):
        with patch.dict(os.environ, {"NSH_UPDATE_KEY": "throwaway"}):
            with patch.object(publisher, "run", return_value="[]"):
                with self.assertRaisesRegex(RuntimeError, "No NSH updater archives"):
                    publisher.publish("v0.2.0", "owner/repo")
            with patch.object(publisher, "run", side_effect=RuntimeError("failed")):
                with self.assertRaisesRegex(RuntimeError, "failed"):
                    publisher.publish("v0.2.0", "owner/repo")
