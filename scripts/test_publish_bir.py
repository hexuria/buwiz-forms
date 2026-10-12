#!/usr/bin/env python3
"""Error-path checks for publish_bir. Does not tag, push, or cargo-publish."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("publish_bir.py")
spec = importlib.util.spec_from_file_location("publish_bir", SCRIPT)
assert spec and spec.loader
publish_bir = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publish_bir)


class PublishBirTest(unittest.TestCase):
    def test_bump_patch(self) -> None:
        self.assertEqual(publish_bir.bump_patch("0.1.0"), "0.1.1")
        self.assertEqual(publish_bir.bump_patch("1.2.9"), "1.2.10")

    def test_reject_downgrade(self) -> None:
        with self.assertRaises(SystemExit) as raised:
            publish_bir.ensure_not_older("0.1.0", "0.0.9")
        self.assertIn("refusing", str(raised.exception))

    def test_same_version_is_not_a_downgrade(self) -> None:
        publish_bir.ensure_not_older("0.1.0", "0.1.0")

    def test_reject_non_semver(self) -> None:
        with self.assertRaises(SystemExit):
            publish_bir.parse_semver("1.2")
        with self.assertRaises(SystemExit):
            publish_bir.parse_semver("01.2.3")

    def test_lock_records_bir_version(self) -> None:
        text = '[[package]]\nname = "bir"\nversion = "0.1.0"\ndependencies = [\n'
        self.assertTrue(publish_bir.lock_has_bir_version("0.1.0", text))
        self.assertFalse(publish_bir.lock_has_bir_version("0.1.1", text))
        self.assertIsNone(publish_bir.lock_bir_version("name = \"bir-desktop\"\nversion = \"0.1.0\"\n"))

    def test_remote_prefers_origin_then_public(self) -> None:
        self.assertEqual(publish_bir.publish_remote(["public", "origin"]), "origin")
        self.assertEqual(publish_bir.publish_remote(["public"]), "public")
        with self.assertRaises(SystemExit) as raised:
            publish_bir.publish_remote(["upstream"])
        self.assertIn("origin or public", str(raised.exception))

    def test_repo_lock_matches_manifest(self) -> None:
        version = publish_bir.read_version()
        self.assertTrue(
            publish_bir.lock_has_bir_version(version),
            f"Cargo.lock bir version != {version}",
        )


if __name__ == "__main__":
    unittest.main()
