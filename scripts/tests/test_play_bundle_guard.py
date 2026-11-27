"""scripts/play_bundle_guard.py: the checks `just play-bundle` makes before it builds a store bundle."""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play_bundle_guard as guard  # noqa: E402

PROFILE = {
    "CARGO_PROFILE_RELEASE_LTO": "fat",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_RELEASE_OPT_LEVEL": "s",
}


def dump(values):
    return "".join(f"{key}={value}\n" for key, value in values.items())


class DotenvTests(unittest.TestCase):
    def test_a_checkout_without_dotenv_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertIsNone(guard.dotenv_refusal(Path(tmp), allow=False))

    def test_a_dotenv_in_the_checkout_or_above_is_refused_unless_allowed(self):
        with tempfile.TemporaryDirectory() as tmp:
            checkout = Path(tmp) / "work/buwiz-forms"
            checkout.mkdir(parents=True)
            for where in (checkout, Path(tmp) / "work"):
                with self.subTest(where=where.name):
                    (where / ".env").write_text("BUWIZ_SERVER_URL=http://localhost:3008\n")
                    refusal = guard.dotenv_refusal(checkout, allow=False)
                    self.assertIn(str(where / ".env"), refusal or "")
                    self.assertIn("--allow-dotenv", refusal)
                    self.assertIsNone(guard.dotenv_refusal(checkout, allow=True))
                    (where / ".env").unlink()

    def test_the_command_exits_non_zero_on_a_refusal(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / ".env").write_text("X=1\n")
            self.assertEqual(guard.main(["dotenv", tmp]), 1)
            self.assertEqual(guard.main(["dotenv", tmp, "--allow-dotenv"]), 0)


class WrapperTests(unittest.TestCase):
    def wrapper(self, tmp, app_dir, mounts):
        path = Path(tmp) / "nsh2404.sh"
        path.write_text(f"#!/usr/bin/env bash\napp_dir={app_dir}\nmounts=({mounts})\nexec docker run ...\n")
        return path

    def test_the_wrapper_for_this_checkout_with_tools_in_home_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            wrapper = self.wrapper(tmp, f"{home}/app/nsh", f"-v {home}:{home} ")
            self.assertIsNone(guard.wrapper_refusal(wrapper, home / "app/nsh", home / "BuwizNsh/tools", home))

    def test_a_wrapper_written_for_another_checkout_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            wrapper = self.wrapper(tmp, f"{home}/other/nsh", f"-v {home}:{home} ")
            refusal = guard.wrapper_refusal(wrapper, home / "app/nsh", home / "tools", home)
            self.assertIn("not this checkout's", refusal or "")

    def test_a_quoted_path_is_read_as_the_shell_reads_it(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "home"
            wrapper = self.wrapper(tmp, f"{home}/my\\ app/nsh", f"-v {home}:{home} ")
            self.assertIsNone(guard.wrapper_refusal(wrapper, home / "my app/nsh", home / "tools", home))

    def test_tools_outside_home_need_a_mount(self):
        with tempfile.TemporaryDirectory() as tmp:
            home, tools = Path(tmp) / "home", Path(tmp) / "disk/tools"
            unmounted = self.wrapper(tmp, f"{home}/app/nsh", f"-v {home}:{home} ")
            self.assertIn("cannot see them", guard.wrapper_refusal(unmounted, home / "app/nsh", tools, home) or "")
            mounted = self.wrapper(tmp, f"{home}/app/nsh", f"-v {home}:{home} -v {tools}:{tools} ")
            self.assertIsNone(guard.wrapper_refusal(mounted, home / "app/nsh", tools, home))

    def test_a_missing_wrapper_is_refused(self):
        with tempfile.TemporaryDirectory() as tmp:
            refusal = guard.wrapper_refusal(Path(tmp) / "none.sh", Path(tmp), Path(tmp), Path(tmp))
            self.assertIn("setup-linux-nsh.sh", refusal or "")


class ContainerEnvTests(unittest.TestCase):
    def test_the_release_profile_inside_the_container_passes(self):
        self.assertEqual(guard.container_env_refusals(dump({**PROFILE, "PATH": "/bin"}), PROFILE), [])

    def test_a_profile_that_did_not_reach_the_container_is_refused(self):
        # nsh2404.sh before it passed the profile: the container had none of it.
        problems = guard.container_env_refusals(dump({"PATH": "/bin"}), PROFILE)
        self.assertEqual(len(problems), 3, problems)
        self.assertTrue(all("did not reach the build" in p for p in problems), problems)
        changed = guard.container_env_refusals(dump({**PROFILE, "CARGO_PROFILE_RELEASE_OPT_LEVEL": "3"}), PROFILE)
        self.assertEqual(len(changed), 1, changed)
        self.assertIn("CARGO_PROFILE_RELEASE_OPT_LEVEL", changed[0])

    def test_a_recipe_that_set_no_profile_is_refused(self):
        problems = guard.container_env_refusals(dump(PROFILE), {})
        self.assertEqual(len(problems), 3, problems)

    def test_what_would_make_a_development_binary_or_bake_an_address_is_refused(self):
        for key in ("BUWIZ_ANDROID_DEV", "BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL"):
            with self.subTest(key=key):
                problems = guard.container_env_refusals(dump({**PROFILE, key: "1"}), PROFILE)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn(key, problems[0])
        # Set but empty is nothing to bake.
        self.assertEqual(guard.container_env_refusals(dump({**PROFILE, "BUWIZ_SERVER_URL": ""}), PROFILE), [])


if __name__ == "__main__":
    unittest.main()
