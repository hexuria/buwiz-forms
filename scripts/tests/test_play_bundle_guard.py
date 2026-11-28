"""scripts/play_bundle_guard.py: the checks `just play-bundle` makes before it builds a store bundle."""

import contextlib
import io
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import play_bundle_guard as guard  # noqa: E402

# `just play-bundle` is a [unix] recipe (the [windows] one only refuses), so the guard reads POSIX paths and runs
# under bash on a Mac or Linux; a Windows runner's drive letters and short names are not its input.
UNIX_ONLY = unittest.skipIf(sys.platform == "win32", "play-bundle runs only on a Mac or Linux")

PROFILE = {
    "CARGO_PROFILE_RELEASE_LTO": "fat",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "1",
    "CARGO_PROFILE_RELEASE_OPT_LEVEL": "s",
}


def dump(values):
    return "".join(f"{key}={value}\n" for key, value in values.items())


@UNIX_ONLY
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


@UNIX_ONLY
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


@UNIX_ONLY
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


@UNIX_ONLY
class ArgsTests(unittest.TestCase):
    def test_a_number_and_the_flag_in_any_order(self):
        self.assertEqual(guard.parse_args([]), (None, False))
        self.assertEqual(guard.parse_args(["28"]), ("28", False))
        self.assertEqual(guard.parse_args(["--allow-dotenv", "28"]), ("28", True))
        self.assertEqual(guard.parse_args(["2100000000"]), ("2100000000", False))

    def test_anything_else_is_refused(self):
        for args in (["0"], ["028"], ["2100000001"], ["28", "29"], ["-1"], ["1e3"], ["28;rm"], ["*"], ["--allow"],
                     [""], ["28 29"]):
            with self.subTest(args=args), self.assertRaises(guard.Refusal):
                guard.parse_args(args)

    def test_the_recipe_reads_one_line(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(guard.main(["args", "--", "28", "--allow-dotenv"]), 0)
        self.assertEqual(out.getvalue(), "28 1\n")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(guard.main(["args"]), 0)
        self.assertEqual(out.getvalue(), "- 0\n")
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(guard.main(["args", "--", "$(touch x)"]), 1)

    def test_the_recipe_passes_its_arguments_without_word_splitting_or_globbing(self):
        recipe = (Path(__file__).resolve().parents[2] / "justfile").read_text(encoding="utf-8")
        start = recipe.index("[positional-arguments]\nplay-bundle *args:")
        body = recipe[start:recipe.index("\n\n", start)]
        self.assertIn('play_bundle_guard.py args -- "$@"', body)
        self.assertNotIn("{{args}}", body)
        self.assertNotIn("{{quote(args)}}", body)


@UNIX_ONLY
class DotenvReportTests(unittest.TestCase):
    def test_names_only_and_the_google_client_only_with_store_gmail(self):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / ".env").write_text("GOOGLE_CLIENT_ID=secret-id\nexport GOOGLE_CLIENT_SECRET=value-of-the-secret\n"
                                            "BUWIZ_SERVER_URL=http://x\nOTHER=1\n")
            with mock.patch.dict(os.environ, {"STORE_GMAIL": ""}):
                report = guard.dotenv_report(Path(tmp))
            self.assertIn("bakes in: nothing", report)
            self.assertIn("GOOGLE_CLIENT_ID", report)
            self.assertNotIn("secret-id", report)
            with mock.patch.dict(os.environ, {"STORE_GMAIL": "1"}):
                report = guard.dotenv_report(Path(tmp))
            self.assertIn("bakes in: GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET", report)
            self.assertNotIn("value-of-the-secret", report)


@UNIX_ONLY
class MoreContainerEnvTests(unittest.TestCase):
    def test_another_release_profile_setting_or_rustflags_is_refused(self):
        for key, value in (("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS", "true"),
                           ("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS", "true"),
                           ("RUSTFLAGS", "-C debug-assertions"),
                           ("CARGO_ENCODED_RUSTFLAGS", "-Cdebug-assertions"),
                           ("CARGO_BUILD_RUSTFLAGS", "-C opt-level=0"),
                           ("CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS", "-C debug-assertions"),
                           ("RUSTC_WRAPPER", "sccache"),
                           ("RUSTC_WORKSPACE_WRAPPER", "x")):
            with self.subTest(key=key):
                problems = guard.container_env_refusals(dump({**PROFILE, key: value}), PROFILE)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn(key, problems[0])

    def test_an_unreadable_environment_is_one_clear_line(self):
        self.assertEqual(guard.container_env_refusals("", PROFILE),
                         ["the build environment could not be read (see the error above)"])

    def test_the_google_client_only_with_store_gmail(self):
        values = dump({**PROFILE, "GOOGLE_CLIENT_ID": "x", "GOOGLE_CLIENT_SECRET": "y"})
        with mock.patch.dict(os.environ, {"STORE_GMAIL": ""}):
            self.assertEqual(len(guard.container_env_refusals(values, PROFILE)), 2)
        with mock.patch.dict(os.environ, {"STORE_GMAIL": "1"}):
            self.assertEqual(guard.container_env_refusals(values, PROFILE), [])


@UNIX_ONLY
class CargoConfigTests(unittest.TestCase):
    def refusals(self, text):
        return guard.cargo_config_refusals(Path("config.toml"), text)

    def test_what_env_cannot_show_is_refused(self):
        for text in ('[profile.release]\ndebug-assertions = true\n',
                     '[profile.release.package."*"]\nopt-level = 0\n',
                     'profile.release.lto = false\n',
                     '[profile]\nrelease = { debug-assertions = true }\n',
                     '[build]\nrustflags = ["-C", "debug-assertions"]\n',
                     'build.rustflags = ["-C", "opt-level=0"]\n',
                     "[target.'cfg(target_os = \"android\")']\nrustflags = [\"-Cdebug-assertions\"]\n",
                     '[target.aarch64-linux-android]\nrustflags = ["-Cdebug-assertions"]\n',
                     '[env]\nBUWIZ_SERVER_URL = "http://localhost:3008"\n',
                     '[env]\nGOOGLE_CLIENT_ID = { value = "x", force = true }\n',
                     'env.BUWIZ_ANDROID_DEV = "1"\n',
                     'include = ["other.toml"]\n',
                     '[build]\nrustc-wrapper = "sccache"\n',
                     'build.rustc-workspace-wrapper = "x"\n',
                     'build = { rustc-wrapper = "x" }\n',
                     '[target."aarch64.custom-android"]\nrustflags = ["-Cdebug-assertions"]\n'):
            with self.subTest(text=text):
                self.assertTrue(self.refusals(text), text)

    def test_ordinary_settings_pass(self):
        text = ('[build]\njobs = 4\n[net]\ngit-fetch-with-cli = true\n[env]\nOPENSSL_STATIC = "1"\n'
                '[target.aarch64-linux-android]\nlinker = "clang"\n[profile.dev]\ndebug = 0\n# [profile.release]\n')
        self.assertEqual(self.refusals(text), [])

    def test_the_files_cargo_reads_are_the_folders_above_and_cargo_home(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text("[profile.release]\nlto = false\n")
            (root / "home").mkdir()
            (root / "home/config").write_text("[env]\nBUWIZ_ACCOUNT_URL = \"x\"\n")
            build = root / "work/app/nsh"
            build.mkdir(parents=True)
            files = guard.cargo_config_files([build], root / "home")
            self.assertEqual(files, [(root / ".cargo/config.toml").resolve(), root / "home/config"])
            with contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(guard.main(["cargo-config", str(build), "--cargo-home", str(root / "home")]), 1)
            self.assertIn(".cargo/config.toml:1: [profile.release]", err.getvalue())
            self.assertIn("home/config:2: [env] BUWIZ_ACCOUNT_URL", err.getvalue())


@UNIX_ONLY
class CompileTimeNamesTests(unittest.TestCase):
    def test_the_list_holds_every_variable_the_sources_bake_in(self):
        root = Path(__file__).resolve().parents[2]
        found = set()
        for folder in (root / "crates", root / "nsh/src"):
            for path in folder.rglob("*.rs"):
                found |= set(re.findall(r'option_env!\("([A-Z0-9_]+)"\)', path.read_text(encoding="utf-8")))
        self.assertTrue(found)
        self.assertEqual(found - set(guard.COMPILE_TIME), set())


if __name__ == "__main__":
    unittest.main()
