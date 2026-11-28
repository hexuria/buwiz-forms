"""scripts/e2e: the end-to-end runner's pure parts, and one recipe run against a stand-in gpui-agent (no app, no device).

Fixtures: fixtures/e2e/snapshot-macos-1601c.json is a real `gpui-agent snapshot` of the app's macOS window (a 1601C
return, from try/money-box-agent-input at ed434e02, docs/proof/money-box-agent-input/calls/ui-expanded.json).
fixtures/e2e/snapshot-phone-2551q.json is written by hand in the strict adapter's shape (no phone snapshot was
recorded on a proof branch): it has the `system-keyboard` node a host with feature software_keyboard reports.
"""
import importlib.util
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import types
import unittest
from pathlib import Path, PureWindowsPath
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("e2e_runner", ROOT / "scripts" / "e2e" / "runner.py")
E2E = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(E2E)
REAL_MOCK_LAYOUT_IS_PROVEN = E2E.mock_layout_is_proven
FIXTURES = Path(__file__).resolve().parent / "fixtures" / "e2e"


def snapshot(name):
    return json.loads((FIXTURES / name).read_text(encoding="utf-8"))


def write(folder, name, text):
    path = Path(folder) / name
    path.write_text(textwrap.dedent(text).lstrip(), encoding="utf-8")
    return path


def suite_of(folder, recipes, params=None):
    path = write(folder, "s.json", json.dumps({"v": 1, "params": params or {}, "recipes": recipes}))
    return E2E.load_suite(str(path))


def clean_env(**updates):
    """The least environment a cleared run() test must keep: Windows finds a home from USERPROFILE or
    HOMEDRIVE/HOMEPATH, both Windows and POSIX need PATH for launch commands, and POSIX Path.home reads HOME."""
    env = {}
    for name in ("USERPROFILE", "HOMEDRIVE", "HOMEPATH", "SYSTEMROOT", "TEMP", "TMP", "PATH", "HOME"):
        if os.environ.get(name) is not None:
            env[name] = os.environ[name]
    env.update(updates)
    return env


class WindowsPortabilityTests(unittest.TestCase):
    """The run()-shaped tests must keep the home and shell variables Windows needs."""

    def test_no_run_test_clears_the_environment_to_empty(self):
        text = Path(__file__).read_text(encoding="utf-8")
        self.assertFalse(any(line.strip() == "@mock.patch.dict(os.environ, {}, clear=True)"
                             for line in text.splitlines()))

    def test_clean_env_keeps_the_windows_and_posix_home_variables(self):
        kept = {"USERPROFILE": "C:\\Users\\runner", "HOMEDRIVE": "C:", "HOMEPATH": "\\Users\\runner",
                "SYSTEMROOT": "C:\\Windows", "TEMP": "C:\\Temp", "TMP": "C:\\Temp", "PATH": "/bin",
                "HOME": "/home/runner"}
        with mock.patch.dict(os.environ, kept, clear=True):
            env = clean_env()
        for name, value in kept.items():
            with self.subTest(name=name):
                self.assertEqual(env.get(name), value)

    @unittest.skipUnless(os.name == "nt", "Path.home reads USERPROFILE only on Windows")
    def test_a_homeless_environment_with_userprofile_still_runs_the_database_path_body(self):
        windowsish = {"USERPROFILE": "C:\\Users\\runner", "HOMEDRIVE": "C:", "HOMEPATH": "\\Users\\runner",
                      "SYSTEMROOT": "C:\\Windows", "TEMP": "C:\\Temp", "TMP": "C:\\Temp", "PATH": os.defpath}
        with mock.patch.dict(os.environ, windowsish, clear=True):
            db, folder = E2E.default_database("macos", "20261128-010203", accounts="mock", unique="u")
        self.assertTrue(db.is_absolute())
        self.assertTrue(folder.is_absolute())
        self.assertNotIn("HOME", windowsish)

    def test_the_posix_process_group_test_is_skipped_where_killpg_does_not_exist(self):
        text = Path(__file__).read_text(encoding="utf-8")
        at = text.rindex("\n    def test_posix_stop_kills_a_process_group_that_survives_the_launcher")
        before = text[:at]
        self.assertIn('@unittest.skipUnless(os.name == "posix"', before[-200:])


class VersionTests(unittest.TestCase):
    def test_versions_compare_as_numbers(self):
        self.assertTrue(E2E.version_at_least("0.4.10", "0.4.9"))
        self.assertTrue(E2E.version_at_least("0.4.1", "0.4.1"))
        self.assertFalse(E2E.version_at_least("0.4.0", "0.4.1"))
        self.assertFalse(E2E.version_at_least(None, "0.4.1"))
        self.assertFalse(E2E.version_at_least("main", "0.4.1"))

    def test_installed_version_comes_from_the_override_without_running_the_cli(self):
        calls = []

        def run(*args, **kwargs):
            calls.append(args)
            raise AssertionError("the override must answer first")

        self.assertEqual(
            E2E.installed_cli_version(
                "missing-gpui-agent",
                env={"E2E_GPUI_AGENT_CLI_VERSION": "v0.4.1"},
                run=run,
            ),
            "0.4.1",
        )
        self.assertEqual(calls, [])

    @unittest.skipUnless(os.name == "posix", "the stand-in CLI is a POSIX script")
    def test_installed_version_comes_from_the_binary_not_cargos_list(self):
        with tempfile.TemporaryDirectory() as folder:
            fake = Path(folder) / "gpui-agent"
            fake.write_text(
                f"#!{sys.executable}\n"
                "import sys\n"
                "if sys.argv[1:] == ['--version']:\n"
                "    print('gpui-agent-cli 0.5.0')\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
            calls = []

            def run(args, **kwargs):
                calls.append(args)
                if args[:3] == ["cargo", "install", "--list"]:
                    class Listed:
                        stdout = "gpui-agent-cli v0.4.1:\n    gpui-agent\n"
                        returncode = 0
                    return Listed()
                return subprocess.run(args, **kwargs)

            self.assertEqual(E2E.installed_cli_version(str(fake), env={}, run=run), "0.5.0")
            self.assertEqual(calls, [[str(fake), "--version"]])

    def test_a_missing_binary_is_unknown_and_refuses_needs_cli(self):
        with tempfile.TemporaryDirectory() as folder:
            missing = str(Path(folder) / "gpui-agent")
            version = E2E.installed_cli_version(missing, env={})
        self.assertEqual(version, "unknown")
        self.assertFalse(E2E.version_at_least(version, "0.5.0"))


class RecipeReadingTests(unittest.TestCase):
    def test_wants_steps_keep_quoted_text_and_drop_flags(self):
        with tempfile.TemporaryDirectory() as folder:
            path = write(folder, "r.wants", """
                # a comment with $not_a_param
                click new-tp-save --screenshot
                type new-tp-name "Juan Dela Cruz" --key-delay-ms 80
                wait_until tp-taxpayer-save --absent --timeout-ms 5000
                assert context-pill name="Tax profile: $name"
                invoke profile.list
            """)
            steps = E2E.recipe_steps(path)
            self.assertEqual([s["op"] for s in steps], ["click", "type", "wait_until", "assert", "invoke"])
            self.assertEqual(steps[1]["target"], "new-tp-name")
            self.assertEqual(steps[1]["text"], "Juan Dela Cruz")
            self.assertEqual(steps[1]["key_delay_ms"], "80")
            self.assertEqual(steps[2]["target"], "tp-taxpayer-save")
            self.assertEqual(steps[4]["name"], "profile.list")
            self.assertEqual(E2E.placeholders(path), {"name"})

    def test_json_steps_and_params(self):
        steps = E2E.recipe_steps(ROOT / "agent-recipes" / "e2e" / "money-boxes-paced.json")
        typed = [s for s in steps if s["op"] == "type"]
        self.assertTrue(typed and all(s["key_delay_ms"] == 80 for s in typed))
        self.assertEqual(E2E.placeholders(ROOT / "agent-recipes" / "e2e" / "money-boxes-paced.json"),
                         {"prior", "amount", "amount_shown"})

    def test_a_step_node_reference_is_no_param_and_keeps_its_step_in_the_same_slice(self):
        # gpui-agent 0.5.0: `$<step>.node` names the node a scroll_until step matched; the runner neither asks for
        # it as a param nor cuts a recipe between the step and its user.
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "find.json"
            path.write_text(json.dumps({"name": "find", "params": ["row"], "steps": [
                {"id": "find", "op": "scroll_until", "target": "list", "pattern": "$row", "dy": 200},
                {"id": "hold", "op": "long_press", "target": "${find}.node"},
                {"id": "press", "op": "click", "target": "$find.node"},
            ]}), encoding="utf-8")
            self.assertEqual(E2E.placeholders(path), {"row"})
            self.assertEqual(E2E.recipe_step_params(path), [{"row"}, set(), set()])
            steps, ids = E2E.recipe_steps(path), E2E.step_ids(path)
            self.assertEqual(E2E.needs_edges_cut(steps, ids, 1), [("find", "hold"), ("find", "press")])
            self.assertEqual(E2E.needs_edges_cut(steps, ids, 2), [("find", "press")])

    def test_bind_keeps_a_doubled_dollar_literal(self):
        self.assertEqual(E2E.bind("$table-add", {"table": "fp-table-x"}), "fp-table-x-add")
        self.assertEqual(E2E.bind("${a}b $$5 $missing", {"a": "A"}), "Ab $5 $missing")


class SafetyTests(unittest.TestCase):
    def test_send_steps_are_presses_on_queue_send_submit_or_file_and_send_invokes(self):
        send = [{"op": "click", "target": "fp-queue"}, {"op": "click", "target": "queue-continue"},
                {"op": "press", "target": "Submit"}, {"op": "click", "target": "File return"},
                {"op": "invoke", "name": "filing.send"}, {"op": "click", "target": "$button"}]
        for step in send:
            self.assertTrue(E2E.is_send_step(step, {"button": "fp-send"}), step)
        for step in [{"op": "click", "target": "tab-filings"}, {"op": "click", "target": "nav-filings"},
                     {"op": "click", "target": "tp-profile-years"}, {"op": "wait_until", "target": "fp-cancel-queue"},
                     {"op": "type", "target": "fp-frm2551Qv2018:txt15", "text": "1"},
                     {"op": "invoke", "name": "profile.save"}]:
            self.assertFalse(E2E.is_send_step(step), step)

    def test_secret_boxes_are_never_typed_into(self):
        for target in ("login-password", "pin-input", "settings-passcode", "agent-token", "$box"):
            self.assertTrue(E2E.is_secret_entry({"op": "type", "target": target}, {"box": "pin-new"}), target)
        self.assertTrue(E2E.is_secret_entry({"op": "paste", "target": "secret-key"}))
        self.assertFalse(E2E.is_secret_entry({"op": "click", "target": "pin-input"}))
        self.assertFalse(E2E.is_secret_entry({"op": "type", "target": "new-tp-name"}))

    def test_a_send_needs_the_dry_run_test_environment(self):
        self.assertTrue(E2E.send_env_ok({"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1"}))
        self.assertFalse(E2E.send_env_ok({"EBIR_TEST_ENV": "1"}))
        self.assertFalse(E2E.send_env_ok({"BIR_SFTP_DRY_RUN": "1"}))
        self.assertFalse(E2E.send_env_ok({"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1", "BIR_SFTP_LIVE": "1"}))

    def test_plan_refuses_a_send_recipe_without_allow_send_and_the_environment(self):
        entry = {"name": "queue", "file": "queue.wants", "send": True}
        context = {"target": "macos", "form_factor": "desktop", "features": [], "cli_version": "0.4.0",
                   "allow_send": False, "env": {"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1"}}
        self.assertEqual(E2E.plan_entry(entry, {}, context)[2], "refused")
        context["allow_send"] = True
        # The app must say it runs on a test database.
        self.assertEqual(E2E.plan_entry(entry, {}, context)[2], "refused")
        context["database"] = "real"
        self.assertEqual(E2E.plan_entry(entry, {}, context)[2], "refused")
        context["database"] = "test"
        self.assertEqual(E2E.plan_entry(entry, {}, context), ("queue.wants", None, None))
        context["env"] = {"EBIR_TEST_ENV": "1"}
        self.assertEqual(E2E.plan_entry(entry, {}, context)[2], "refused")

    def test_keys_code_boxes_and_camel_case_ids_are_caught(self):
        for step in [{"op": "key", "target": "auth-tp-pin", "words": ["auth-tp-pin", "1"]},
                     {"op": "type", "target": "gate-code-boxes"}, {"op": "type", "target": "auth-tp-totp"},
                     {"op": "set_value", "target": "settings-totp-cells"},
                     {"op": "type", "target": "setup-change-code-cells"}, {"op": "type", "target": "loginPassword"},
                     {"op": "type", "target": "x", "words": ["x", "--id", "otp-box"]},
                     {"op": "click", "target": "system-keyboard-key-1"}]:
            self.assertTrue(E2E.is_secret_entry(step), step)
        for step in [{"op": "type", "target": "new-tp-zip_code-search"}, {"op": "type", "target": "fp-frm1601FQ:txtAtcCode1"},
                     {"op": "type", "target": "fp-frm2551Qv2018:txt15", "text": "my pin"}]:
            self.assertFalse(E2E.is_secret_entry(step), step)
        self.assertTrue(E2E.is_send_step({"op": "click", "target": "fpQueue"}))
        self.assertTrue(E2E.is_send_step({"op": "click", "target": "x", "words": ["x", "--id", "form-submit"]}))

    def test_secret_words_inside_a_word_nested_targets_and_recovery_keys(self):
        for step in ({"op": "type", "target": "setup-change-newpin-cells"},
                     {"op": "type", "target": "settings-confirmPin"}, {"op": "type", "target": "auth-recovery"},
                     {"op": "type", "target": {"id": "auth-tp-pin"}}, {"op": "type", "target": "adminotp-box"}):
            self.assertTrue(E2E.is_secret_entry(step), step)
        for target in ("new-tp-zip_code-search", "tp-sec-branch-edit", "fp-frm1601FQ:txtAtcCode1", "spinner-box"):
            self.assertFalse(E2E.is_secret_entry({"op": "type", "target": target}), target)

    def test_a_resolved_id_is_checked_by_what_its_node_says(self):
        step = {"op": "type", "target": "$box"}
        params = {"box": "n42"}
        self.assertFalse(E2E.is_secret_entry(step, params))
        self.assertTrue(E2E.is_secret_entry(step, params, {"n42": "n42 Enter your PIN textbox"}))
        # A selector that holds the resolved id among other words.
        self.assertTrue(E2E.is_secret_entry({"op": "type", "target": "view:$box"}, params,
                                            {"n42": "n42 Enter your PIN textbox"}))
        with tempfile.TemporaryDirectory() as folder:
            path = write(folder, "r.wants", "type $box 1234\n")
            problems = E2E.step_problems(E2E.recipe_steps(path), params, "r.wants", {},
                                         {"n42": "n42 Authenticator code textbox"})
        self.assertEqual(len(problems), 1, problems)
        self.assertFalse(E2E.is_secret_text("Postal code"))
        self.assertTrue(E2E.is_secret_text("6-digit code"))

    def test_sign_in_presses_need_a_marked_recipe_and_the_local_mock(self):
        for target in ("gate-continue", "gate-resend", "signInContinue", "sign-in-continue", "auth-verify"):
            self.assertTrue(E2E.is_sign_in_step({"op": "click", "target": target}), target)
        for target in ("queue-continue", "removal-continue", "account-row"):
            self.assertFalse(E2E.is_sign_in_step({"op": "click", "target": target}), target)
        # Every action on a sign-in screen: Enter in the email box sends the code.
        for step in ({"op": "key", "target": "gate-email", "words": ["gate-email", "Enter"]},
                     {"op": "type", "target": "gate-email", "text": "a@example.com"},
                     {"op": "click", "target": "gate-change-email"}):
            self.assertTrue(E2E.is_sign_in_step(step), step)
        for step in ({"op": "wait_until", "target": "gate-email"}, {"op": "assert", "target": "gate-code-boxes"}):
            self.assertFalse(E2E.is_sign_in_step(step), step)
        entry = {"name": "s", "file": "s.wants", "sign_in": True}
        base = {"target": "macos", "form_factor": "desktop", "features": [], "cli_version": "0.4.1",
                "allow_send": False}
        self.assertEqual(E2E.plan_entry(entry, {}, {**base, "launched": False, "env": {}})[2], "refused")
        self.assertEqual(E2E.plan_entry(entry, {}, {**base, "launched": True,
                                                   "env": {"BUWIZ_SERVER_URL": "https://api.buwiz.com"}})[2], "refused")
        self.assertIsNone(E2E.plan_entry(entry, {}, {**base, "launched": True,
                                                    "env": {"BUWIZ_SERVER_URL": "http://localhost:3008"}})[2])
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "s.wants", "type gate-email a@example.com\nclick gate-continue\n")
            suite = suite_of(folder, [{"name": "s", "file": "s.wants"}])
            self.assertIn("sign-in screen", "\n".join(E2E.check_suite(suite)))

    def test_sign_in_recipes_are_limited_to_sign_in_and_setup_nodes(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "good.wants", """
                click gate-continue
                type gate-email a@example.com
                key gate-email Enter
                wait_until --timeout-ms 100 gate-code-boxes
                assert setup-root
                scroll setup-helpdesk
            """)
            suite = suite_of(folder, [{"name": "sign-in", "file": "good.wants", "sign_in": True}])
            self.assertEqual(E2E.check_suite(suite), [])

            for number, line in enumerate((
                    "click home",
                    "assert fp-queue",
                    "invoke nav.go",
                    "set_value setup-name x",
            ), 1):
                with self.subTest(line=line):
                    write(folder, f"bad{number}.wants", line + "\n")
                    suite = suite_of(folder, [{"name": "sign-in", "file": f"bad{number}.wants", "sign_in": True}])
                    problems = "\n".join(E2E.check_suite(suite))
                    self.assertIn("sign_in recipes may use only", problems)

    def test_a_sign_in_recipe_target_param_is_checked_in_both_directions(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "param.wants", "click $target\n")
            for target, allowed in (("gate-continue", True), ("fp-queue", False)):
                with self.subTest(target=target):
                    suite = suite_of(folder, [{"name": "sign-in", "file": "param.wants", "sign_in": True}],
                                     params={"target": target})
                    problems = "\n".join(E2E.check_suite(suite))
                    self.assertEqual("sign_in recipes may use only" not in problems, allowed, problems)

    def test_a_nested_sign_in_recipe_target_is_checked(self):
        step = {"op": "click", "target": {"id": "fp-queue"}}
        self.assertIn("fp-queue", E2E.sign_in_recipe_problem(step, {}))
        step = {"op": "click", "target": {"id": "gate-continue"}}
        self.assertIsNone(E2E.sign_in_recipe_problem(step, {}))

    def test_the_launch_environment_has_no_live_settings(self):
        env = E2E.launch_env({"BIR_SFTP_HOST": "sftp.example", "BIR_SFTP_PASSWORD": "pw", "BIR_SFTP_LIVE": "1",
                              "BIR_RELAY_URL": "https://relay.example", "BUWIZ_RELAY_SEND": "1",
                              "BUWIZ_SERVER_URL": "https://api.buwiz.com", "BUWIZ_ACCOUNT_URL": "http://127.0.0.1:3008",
                              "PATH": "/bin"}, "tok", "127.0.0.1:1")
        for name in E2E.LIVE_SEND_VARS + ("BUWIZ_SERVER_URL",):
            self.assertNotIn(name, env)
        self.assertEqual(env["BUWIZ_ACCOUNT_URL"], "http://127.0.0.1:3008")
        self.assertTrue(E2E.send_env_ok(env))
        self.assertEqual((env["GPUI_AGENT_TOKEN"], env["PATH"]), ("tok", "/bin"))


class AccountsMockTests(unittest.TestCase):
    def test_parse_args_accepts_accounts_mock_and_defaults_to_the_bypass(self):
        self.assertEqual(E2E.parse_args(["macos", "smoke", "--accounts", "mock"]).accounts, "mock")
        self.assertEqual(E2E.parse_args(["macos", "smoke"]).accounts, "bypass")

    def test_accounts_mock_refuses_no_launch(self):
        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.parse_args(["macos", "smoke", "--accounts", "mock", "--no-launch"])
        self.assertIn("--no-launch", str(raised.exception))

    def test_accounts_mock_on_android_is_refused_before_launch_or_write(self):
        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.parse_args(["android", "smoke", "--accounts", "mock"])
        self.assertIn("not supported", str(raised.exception))
        self.assertIn("cannot reach the device's app", str(raised.exception))
        self.assertIn("host path does not exist", str(raised.exception))

    def test_the_mock_launch_environment_has_no_bypass_and_points_at_the_mock(self):
        env = E2E.launch_env({"PATH": "/bin", "BIR_AUTH_BYPASS": "0", "BIR_SFTP_LIVE": "1",
                              "BUWIZ_SERVER_URL": "http://localhost:9999",
                              "BUWIZ_ACCOUNT_URL": "http://[::1]:9998"},
                             "tok", "127.0.0.1:1", accounts="mock")
        self.assertEqual(env["BIR_AUTH_BYPASS"], "0")
        self.assertEqual(env["BUWIZ_SERVER_URL"], E2E.MOCK_ACCOUNT_URL)
        self.assertEqual(env["BUWIZ_ACCOUNT_URL"], E2E.MOCK_ACCOUNT_URL)
        self.assertEqual((env["EBIR_TEST_ENV"], env["BIR_SFTP_DRY_RUN"], env["BUWIZ_NO_KEYCHAIN"]), ("1", "1", "1"))
        self.assertNotIn("BIR_SFTP_LIVE", env)
        self.assertEqual(env["E2E_ACCOUNTS"], "mock")
        self.assertEqual(env["E2E_MOCK_ACCOUNT_URL"], E2E.MOCK_ACCOUNT_URL)
        self.assertTrue(E2E.send_env_ok(env))
        bypass = E2E.launch_env({}, "tok", "127.0.0.1:1")
        self.assertEqual(bypass["BIR_AUTH_BYPASS"], "1")
        self.assertNotIn("E2E_MOCK_ACCOUNT_URL", bypass)

    def test_loopback_account_urls_are_accepted_and_replaced_by_the_mock_url(self):
        for url in ("http://127.0.0.1:1", "http://[::1]:2", "http://localhost:3", "https://localhost"):
            with self.subTest(url=url):
                env = E2E.launch_env({"BUWIZ_SERVER_URL": url, "BUWIZ_ACCOUNT_URL": url},
                                     "tok", "127.0.0.1:1", accounts="mock")
                self.assertEqual(env["BUWIZ_SERVER_URL"], E2E.MOCK_ACCOUNT_URL)
                self.assertEqual(env["BUWIZ_ACCOUNT_URL"], E2E.MOCK_ACCOUNT_URL)

    def test_accounts_mock_refuses_an_environment_auth_bypass(self):
        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.launch_env({"BIR_AUTH_BYPASS": "1"}, "tok", "127.0.0.1:1", accounts="mock")
        self.assertIn("BIR_AUTH_BYPASS=1", str(raised.exception))
        self.assertIn("--accounts mock", str(raised.exception))

    def test_account_state_files_are_pinned_beside_the_runs_own_database(self):
        env = {"BUWIZ_CLOUD_ACCOUNT_FILE": "elsewhere-account", "BUWIZ_PENDING_SIGN_IN_FILE": "elsewhere-pending"}
        db = Path("scratch") / "e2e-macos-20261128-010203.db"
        paths = E2E.pin_account_state(env, db)
        self.assertEqual(paths, (Path("scratch/e2e-macos-20261128-010203.db.buwiz-cloud-account.json"),
                                 Path("scratch/e2e-macos-20261128-010203.db.buwiz-pending-sign-in.json")))
        self.assertEqual(env["BUWIZ_CLOUD_ACCOUNT_FILE"], str(paths[0]))
        self.assertEqual(env["BUWIZ_PENDING_SIGN_IN_FILE"], str(paths[1]))

    def test_the_mock_default_database_is_in_a_per_run_device_folder(self):
        home = Path("home")
        db, folder = E2E.default_database("macos", "20261128-010203", accounts="mock", home=home, unique="u")
        home = home.resolve()
        self.assertEqual(folder, home / "BuwizNsh" / "agents" / "e2e-macos-20261128-010203-u")
        self.assertEqual(db, folder / "bir_data.db")
        plain, plain_folder = E2E.default_database("macos", "20261128-010203", home=home)
        self.assertEqual(plain, home / "BuwizNsh" / "agents" / "e2e-macos-20261128-010203.db")
        self.assertIsNone(plain_folder)

    def test_accounts_mock_refuses_every_non_loopback_account_url(self):
        for name in E2E.ACCOUNT_VARS:
            for url in ("https://api.buwiz.com", "http://localhost:80@evil.example"):
                with self.subTest(name=name, url=url):
                    with self.assertRaises(E2E.SuiteError) as raised:
                        E2E.launch_env({name: url}, "tok", "127.0.0.1:1", accounts="mock")
                    self.assertIn(name, str(raised.exception))
                    self.assertIn("loopback", str(raised.exception))

    def test_a_taken_mock_port_is_refused_outright(self):
        calls = []

        def answers(addr):
            self.assertEqual(addr, E2E.MOCK_ACCOUNT_ADDR)
            return True

        def popen(*args, **kwargs):
            calls.append((args, kwargs))
            raise AssertionError("a taken port must not start another mock")

        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=1,
                                       popen=popen, answers=answers, sleep=lambda seconds: None)
        self.assertEqual(calls, [])
        self.assertIn("already listens", str(raised.exception))
        self.assertIn("no health or version route", str(raised.exception))

    def test_the_mock_starts_the_android_scripts_way(self):
        class FakeProcess:
            pid = 4321
            returncode = None

            def poll(self):
                return None

        fake = FakeProcess()
        calls = []
        answers_seen = []
        sleeps = []

        def popen(command, **kwargs):
            calls.append((command, kwargs))
            return fake

        def answers(addr):
            answers_seen.append(addr)
            if len(answers_seen) > 2:
                (Path(folder) / "mock-account.log").write_text(
                    "[mock-account] listening on http://127.0.0.1:3008 (auto-approve: true)\n", encoding="utf-8")
                return True
            return False

        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            proc = E2E.start_mock_account("just", {"PATH": "/bin"}, Path(folder), log, timeout_s=5,
                                          popen=popen, answers=answers, sleep=sleeps.append)
            log_exists = (Path(folder) / "mock-account.log").exists()
        self.assertIs(proc, fake)
        self.assertEqual(calls[0][0][:3], ["just", "mock-account", "--auto-approve"])
        self.assertEqual(calls[0][0][3:], ["--bind", "127.0.0.1", "--port", "3008"])
        self.assertEqual(calls[0][1]["cwd"], E2E.ROOT)
        self.assertEqual(calls[0][1]["env"], {"PATH": "/bin"})
        if os.name == "posix":
            self.assertTrue(calls[0][1]["start_new_session"])
        self.assertEqual(len(sleeps), 1)
        self.assertTrue(log_exists)

    def test_a_mock_that_ends_before_listening_is_loud(self):
        class Ended:
            pid = 4321
            returncode = 2

            def poll(self):
                return 2

        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=5,
                                       popen=lambda *args, **kwargs: Ended(), answers=lambda addr: False,
                                       sleep=lambda seconds: None)
        self.assertIn("exit 2", str(raised.exception))

    def test_a_dead_mock_is_not_identified_by_a_foreign_listener(self):
        class Ended:
            pid = 4321
            returncode = 1

            def poll(self):
                return 1

        answers_seen = []
        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=5,
                                       popen=lambda *args, **kwargs: Ended(),
                                       answers=lambda addr: answers_seen.append(addr) or len(answers_seen) > 1,
                                       sleep=lambda seconds: None)
        self.assertIn("exit 1", str(raised.exception))

    def test_a_listener_without_the_childs_own_listening_line_is_not_ready(self):
        class Alive:
            pid = 4321
            returncode = None

            def poll(self):
                return None

        answers_seen = []
        proc = Alive()
        stops = []
        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            # The refusal stops the child. The child here is a stand-in with a made-up pid: the real stop would
            # signal whatever process group has that number on the machine running the tests, so it is replaced.
            with mock.patch.object(E2E, "stop_mock_account", side_effect=lambda p, log: stops.append(p)):
                with self.assertRaises(E2E.SuiteError) as raised:
                    E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=0,
                                           popen=lambda *args, **kwargs: proc,
                                           answers=lambda addr: answers_seen.append(addr) or len(answers_seen) > 1,
                                           sleep=lambda seconds: None)
        self.assertIn("listening line", str(raised.exception))
        self.assertEqual(stops, [proc])

    def test_ctrl_c_during_mock_start_stops_the_child(self):
        class Alive:
            pid = 4321
            returncode = None

            def poll(self):
                return None

        proc = Alive()
        stops = []

        def sleep(seconds):
            raise KeyboardInterrupt

        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            with mock.patch.object(E2E, "stop_mock_account", side_effect=lambda p, log: stops.append(p)) as stop:
                with self.assertRaises(KeyboardInterrupt):
                    E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=5,
                                           popen=lambda *args, **kwargs: proc, answers=lambda addr: False,
                                           sleep=sleep)
        self.assertEqual(stops, [proc])

    def test_a_mock_start_timeout_stops_the_child(self):
        class Alive:
            pid = 4321
            returncode = None

            def poll(self):
                return None

        proc = Alive()
        stops = []
        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(None, echo=False)
            with mock.patch.object(E2E, "stop_mock_account", side_effect=lambda p, log: stops.append(p)):
                with self.assertRaises(E2E.SuiteError) as raised:
                    E2E.start_mock_account("just", {}, Path(folder), log, timeout_s=0,
                                           popen=lambda *args, **kwargs: proc, answers=lambda addr: False,
                                           sleep=lambda seconds: None)
        self.assertIn("did not listen", str(raised.exception))
        self.assertEqual(stops, [proc])

    def test_the_mock_is_stopped_in_its_own_process_group(self):
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 4321
        log = E2E.RunLog(None, echo=False)
        if os.name == "posix":
            with mock.patch.object(E2E.os, "killpg") as killpg:
                E2E.stop_mock_account(proc, log)
            killpg.assert_any_call(4321, E2E.signal.SIGINT)
        else:
            E2E.stop_mock_account(proc, log)
            proc.send_signal.assert_called_once_with(getattr(E2E.signal, "CTRL_BREAK_EVENT", E2E.signal.SIGTERM))

    def test_nsh_run_agent_keeps_the_gate_on_for_accounts_mock(self):
        text = (E2E.ROOT / "justfile").read_text(encoding="utf-8")
        unix_at = text.index("[unix]\nnsh-run-agent")
        windows_at = text.index("[windows]\nnsh-run-agent")
        unix = text[unix_at:windows_at]
        windows = text[windows_at:]

        def bash_branch(script, start=0):
            marker = 'if [ "${E2E_ACCOUNTS:-bypass}" = mock ]; then'
            at = script.index(marker, start)
            else_at = script.index("else", at)
            fi_at = script.index("fi", else_at)
            return script[at:else_at], script[else_at:fi_at]

        mock_branch, bypass_branch = bash_branch(unix)
        self.assertIn("export BIR_AUTH_BYPASS=0", mock_branch)
        self.assertNotIn("export BIR_AUTH_BYPASS=", mock_branch.replace("export BIR_AUTH_BYPASS=0", ""))
        self.assertIn('BIR_DEVICE_DIR:?nsh-run-agent: mock mode needs BIR_DEVICE_DIR', mock_branch)
        self.assertIn('BIR_DATABASE_PATH:?nsh-run-agent: mock mode needs BIR_DATABASE_PATH', mock_branch)
        self.assertNotIn("unset BIR_AUTH_BYPASS", mock_branch)
        self.assertIn('export BUWIZ_SERVER_URL="$E2E_MOCK_ACCOUNT_URL"', mock_branch)
        self.assertIn('BUWIZ_ACCOUNT_URL="$E2E_MOCK_ACCOUNT_URL"', mock_branch)
        self.assertIn("export BIR_AUTH_BYPASS=1", bypass_branch)
        self.assertIn("unset BIR_DEVICE_DIR", bypass_branch)
        self.assertIn('export BUWIZ_CLOUD_ACCOUNT_FILE="$db.buwiz-cloud-account.json"', unix)
        self.assertIn('export BUWIZ_PENDING_SIGN_IN_FILE="$db.buwiz-pending-sign-in.json"', unix)
        self.assertLess(unix.index('export BIR_DATABASE_PATH="$db"'),
                        unix.index('export BUWIZ_CLOUD_ACCOUNT_FILE="$db.buwiz-cloud-account.json"'))

        android_at = unix.index('if [ "{{target}}" = android ]')
        ios_at = unix.index('if [ "{{target}}" = ios ]')
        android = unix[android_at:ios_at]
        ios = unix[ios_at:]
        self.assertIn("BUWIZ_SERVER_URL=$E2E_MOCK_ACCOUNT_URL;BUWIZ_ACCOUNT_URL=$E2E_MOCK_ACCOUNT_URL;", android)
        self.assertIn("${account_settings}", android)
        self.assertIn("${device_settings}", android)
        self.assertIn("BIR_DEVICE_DIR=${BIR_DEVICE_DIR", android)
        ios_mock, ios_bypass = bash_branch(ios)
        self.assertIn("export SIMCTL_CHILD_BIR_AUTH_BYPASS=0", ios_mock)
        self.assertIn("export SIMCTL_CHILD_BIR_DEVICE_DIR=", ios_mock)
        self.assertNotIn("unset SIMCTL_CHILD_BIR_AUTH_BYPASS", ios_mock)
        self.assertIn('SIMCTL_CHILD_BUWIZ_SERVER_URL="$E2E_MOCK_ACCOUNT_URL"', ios_mock)
        self.assertIn('SIMCTL_CHILD_BUWIZ_ACCOUNT_URL="$E2E_MOCK_ACCOUNT_URL"', ios_mock)
        for name in ("BIR_RELAY_URL", "BUWIZ_RELAY_SEND", "BUWIZ_TRUST_KEYS"):
            self.assertIn(f"SIMCTL_CHILD_{name}", ios_mock)
        self.assertIn("export SIMCTL_CHILD_BIR_AUTH_BYPASS=1", ios_bypass)
        self.assertIn("unset SIMCTL_CHILD_BIR_DEVICE_DIR", ios_bypass)

        ps_marker = "if ($env:E2E_ACCOUNTS -eq 'mock') {"
        ps_at = windows.index(ps_marker)
        ps_else = windows.index("} else {", ps_at)
        ps_end = windows.index("}", ps_else + len("} else {"))
        ps_mock, ps_bypass = windows[ps_at:ps_else], windows[ps_else:ps_end]
        self.assertIn("$env:BIR_AUTH_BYPASS = '0'", ps_mock)
        self.assertIn("mock mode needs BIR_DEVICE_DIR", ps_mock)
        self.assertIn("mock mode needs BIR_DATABASE_PATH", ps_mock)
        self.assertNotIn("Remove-Item Env:BIR_AUTH_BYPASS", ps_mock)
        self.assertIn("$env:BUWIZ_SERVER_URL = $env:E2E_MOCK_ACCOUNT_URL", ps_mock)
        self.assertIn("$env:BUWIZ_ACCOUNT_URL = $env:E2E_MOCK_ACCOUNT_URL", ps_mock)
        self.assertIn("$env:BIR_AUTH_BYPASS = '1'", ps_bypass)
        self.assertIn("Remove-Item Env:BIR_DEVICE_DIR", ps_bypass)
        self.assertIn("$env:BUWIZ_CLOUD_ACCOUNT_FILE = \"$($env:BIR_DATABASE_PATH).buwiz-cloud-account.json\"", windows)
        self.assertIn("$env:BUWIZ_PENDING_SIGN_IN_FILE = \"$($env:BIR_DATABASE_PATH).buwiz-pending-sign-in.json\"",
                      windows)
        self.assertIn("BUWIZ_SERVER_URL=$($env:E2E_MOCK_ACCOUNT_URL);BUWIZ_ACCOUNT_URL=$($env:E2E_MOCK_ACCOUNT_URL);",
                      windows)
        self.assertIn("$($accountSettings)", windows)
        self.assertIn("$($deviceSettings)", windows)
        self.assertIn("BIR_DATABASE_PATH=$($env:BIR_DATABASE_PATH);", windows)

    def test_android_first_run_leaves_the_sign_in_files_alone_in_bypass_mode(self):
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "", "")

        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E, "android_serial", return_value="emulator-5554"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.fresh_android({}, mock.Mock())
        self.assertEqual(calls[0][:5], ["adb", "-s", "emulator-5554", "shell", "run-as"])
        self.assertIn(E2E.ANDROID_TEST_DB, calls[0])
        self.assertNotIn(E2E.ANDROID_TEST_ACCOUNT, calls[0])
        self.assertNotIn(E2E.ANDROID_TEST_PENDING, calls[0])

    def test_android_mock_first_run_removes_the_saved_and_pending_sign_in(self):
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "", "")

        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E, "android_serial", return_value="emulator-5554"), \
                mock.patch.object(E2E, "write_android_dev_env"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.fresh_android({}, mock.Mock(), dev_env="PINNED\n")
        self.assertEqual(calls[0][:5], ["adb", "-s", "emulator-5554", "shell", "run-as"])
        self.assertIn(E2E.ANDROID_TEST_DB, calls[0])
        self.assertIn(E2E.ANDROID_TEST_ACCOUNT, calls[0])
        self.assertIn(E2E.ANDROID_TEST_PENDING, calls[0])

    def test_the_android_mock_dev_env_is_pinned_and_closes_the_bypass(self):
        text = E2E.android_mock_dev_env("127.0.0.1:17421", "/device/e2e-run")
        lines = text.splitlines()
        self.assertIn("BIR_AUTH_BYPASS=0", lines)
        self.assertNotIn("BIR_AUTH_BYPASS=1", lines)
        self.assertIn("BUWIZ_SERVER_URL=http://127.0.0.1:3008", lines)
        self.assertIn("BUWIZ_ACCOUNT_URL=http://127.0.0.1:3008", lines)
        self.assertIn("GPUI_AGENT_ADDR=127.0.0.1:17421", lines)
        self.assertIn("BIR_DEVICE_DIR=/device/e2e-run", lines)

    def test_android_mock_mode_overwrites_dev_env_with_the_pinned_set(self):
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs.get("input")))
            stdout = "package:/data/app/nsh" if command[4:6] == ["pm", "path"] else ""
            return subprocess.CompletedProcess(command, 0, stdout, "")

        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E, "android_serial", return_value="emulator-5554"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.fresh_android({}, mock.Mock(), dev_env="BIR_AUTH_BYPASS=\nBUWIZ_SERVER_URL=http://127.0.0.1:3008\n")
        writes = [(command, text) for command, text in calls if text is not None]
        self.assertEqual(len(writes), 1)
        self.assertIn("dev.env", writes[0][0][-1])
        self.assertEqual(writes[0][1], "BIR_AUTH_BYPASS=\nBUWIZ_SERVER_URL=http://127.0.0.1:3008\n")

    def test_the_android_mock_dev_env_restores_a_previous_file(self):
        previous = "BIR_AUTH_BYPASS=1\nEBIR_TEST_ENV=1\n"
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs.get("input")))
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            shell = command[-1]
            if "cat " + E2E.ANDROID_DEV_ENV in shell and "cat >" not in shell:
                return subprocess.CompletedProcess(command, 0, previous, "")
            return subprocess.CompletedProcess(command, 0, "", "")

        state = {}
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.write_android_dev_env("adb", "emulator-5554", "PINNED\n", log, state=state)
            E2E.restore_android_dev_env(state, log)
        writes = [text for command, text in calls if text is not None]
        self.assertEqual(writes, ["PINNED\n", previous])

    def test_a_captured_mock_dev_env_is_stale_and_removed_not_restored(self):
        previous = "BIR_AUTH_BYPASS=\nBUWIZ_SERVER_URL=http://127.0.0.1:3008\n"
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs.get("input")))
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            shell = command[-1]
            if "cat " + E2E.ANDROID_DEV_ENV in shell and "cat >" not in shell:
                return subprocess.CompletedProcess(command, 0, previous, "")
            return subprocess.CompletedProcess(command, 0, "", "")

        state = {}
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.write_android_dev_env("adb", "emulator-5554", "PINNED\n", log, state=state)
            E2E.restore_android_dev_env(state, log)
        writes = [text for command, text in calls if text is not None]
        self.assertEqual(writes, ["PINNED\n"])
        self.assertTrue(any("rm -f " + E2E.ANDROID_DEV_ENV in command[-1] for command, _ in calls), calls)
        self.assertTrue(any("stale mock dev.env" in line and "not restored" in line for line in log.lines),
                        log.lines)

    def test_an_interrupted_android_dev_env_write_restores_the_previous_file_at_once(self):
        previous = "BIR_AUTH_BYPASS=1\n"
        writes = []

        def run(command, **kwargs):
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            shell = command[-1]
            if "cat " + E2E.ANDROID_DEV_ENV in shell and "cat >" not in shell:
                return subprocess.CompletedProcess(command, 0, previous, "")
            if kwargs.get("input") == "PINNED\n":
                writes.append(kwargs.get("input"))
                raise KeyboardInterrupt
            writes.append(kwargs.get("input"))
            return subprocess.CompletedProcess(command, 0, "", "")

        state = {}
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.subprocess, "run", side_effect=run), \
                self.assertRaises(KeyboardInterrupt):
            E2E.write_android_dev_env("adb", "emulator-5554", "PINNED\n", log, state=state)
        self.assertEqual(writes, ["PINNED\n", previous])
        self.assertFalse(state["written"])

    def test_the_android_mock_dev_env_is_removed_when_none_existed_before(self):
        calls = []

        def run(command, **kwargs):
            calls.append((command, kwargs.get("input")))
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            shell = command[-1]
            if "cat " + E2E.ANDROID_DEV_ENV in shell and "cat >" not in shell:
                return subprocess.CompletedProcess(command, 44, "", "not found")
            return subprocess.CompletedProcess(command, 0, "", "")

        state = {}
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.write_android_dev_env("adb", "emulator-5554", "PINNED\n", log, state=state)
            E2E.restore_android_dev_env(state, log)
        self.assertTrue(any("rm -f " + E2E.ANDROID_DEV_ENV in command[-1] for command, _ in calls), calls)

    def test_a_bypass_android_run_refuses_a_dev_env_with_mock_settings(self):
        def run(command, **kwargs):
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            return subprocess.CompletedProcess(
                command, 0, "BIR_AUTH_BYPASS=\nBUWIZ_SERVER_URL=http://127.0.0.1:3008\n", "")

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run), \
                self.assertRaises(E2E.SuiteError) as raised:
            E2E.assert_android_dev_env_not_mock({"ANDROID_SERIAL": "emulator-5554"}, log)
        message = str(raised.exception)
        self.assertIn("dev.env", message)
        self.assertIn("mock", message)
        self.assertIn("remove it: adb -s emulator-5554 shell run-as com.longbridge.nsh.host rm -f "
                      "files/nsh/dev.goldcoders.bir/dev.env", message)
        self.assertNotIn("or run --accounts mock", message)
        self.assertNotIn("3008", message, "the refusal names the settings, not the file's contents")

    def test_a_bypass_android_dev_env_check_without_an_emulator_is_a_suite_error(self):
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E, "android_serial", return_value=None), \
                self.assertRaises(E2E.SuiteError) as raised:
            E2E.assert_android_dev_env_not_mock({}, E2E.RunLog(None, echo=False))
        self.assertIn("no Android emulator is online", str(raised.exception))

    def test_mock_dev_env_settings_are_found_by_key_without_repeating_values(self):
        mock = "BIR_AUTH_BYPASS=1\nBUWIZ_SERVER_URL=http://localhost:3008/\n"
        self.assertEqual(E2E.android_dev_env_mock_settings(mock), ["BUWIZ_SERVER_URL"])
        plain = "BIR_AUTH_BYPASS=1\nBUWIZ_SERVER_URL=http://localhost:3009\n"
        self.assertEqual(E2E.android_dev_env_mock_settings(plain), [])

    def test_a_dev_env_read_error_does_not_repeat_file_contents(self):
        def run(command, **kwargs):
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            return subprocess.CompletedProcess(command, 1, "BUWIZ_TRUST_KEYS=partial-secret\n", "")

        with mock.patch.object(E2E.subprocess, "run", side_effect=run), \
                self.assertRaises(E2E.SuiteError) as raised:
            E2E.read_android_dev_env("adb", "emulator-5554")
        self.assertNotIn("partial-secret", str(raised.exception))

    def test_a_bypass_android_run_accepts_a_plain_dev_env(self):
        def run(command, **kwargs):
            if command[4:6] == ["pm", "path"]:
                return subprocess.CompletedProcess(command, 0, "package:/data/app/nsh", "")
            return subprocess.CompletedProcess(command, 0, "BIR_AUTH_BYPASS=1\nEBIR_TEST_ENV=1\n", "")

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.assert_android_dev_env_not_mock({"ANDROID_SERIAL": "emulator-5554"}, log)

    def test_the_bypass_android_dev_env_check_runs_before_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            args = E2E.parse_args(["android", str(suite), "--db", str(folder / "scratch.db"),
                                   "--out", str(folder / "out")])

            class AliveProcess:
                def poll(self):
                    return None

            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "assert_android_dev_env_not_mock",
                                      side_effect=E2E.SuiteError("stale mock dev.env")) as guard, \
                    mock.patch.object(E2E, "fresh_android") as fresh, \
                    mock.patch.object(E2E, "launch", return_value=AliveProcess()) as launched, \
                    mock.patch.object(E2E, "wait_for", return_value=True), \
                    mock.patch.object(E2E, "app_info", side_effect=lambda *rest: {"device_dir": launched[0][1]["BIR_DEVICE_DIR"], "rooms_mode": True, "database_path": str(Path(launched[0][1]["BIR_DEVICE_DIR"]) / "rooms" / "room" / "bir_data.db")}), \
                    mock.patch.object(E2E, "run_entry", return_value={"name": "x", "status": "pass", "ms": 0}), \
                    mock.patch.object(E2E, "stop"), mock.patch.object(E2E, "close_device"):
                with self.assertRaises(E2E.SuiteError):
                    E2E.run(args)
            guard.assert_called_once()
            fresh.assert_not_called()
            launched.assert_not_called()

    def test_gate_preflight_requires_the_real_gate_node_and_names_bypass_sources(self):
        class PastGate:
            def snapshot(self):
                return {"tree": {"nodes": [{"id": "home", "role": "group"}]}}

        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.assert_sign_in_gate(PastGate())
        message = str(raised.exception)
        self.assertIn("the app started past the sign-in gate", message)
        self.assertIn("a bypass reached it", message)
        for source in (".env", "wrapper", "dev.env", "SIMCTL_CHILD"):
            self.assertIn(source, message)

    def test_gate_preflight_accepts_gate_root(self):
        class AtGate:
            def snapshot(self):
                return {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}}

        E2E.assert_sign_in_gate(AtGate())

    def test_gate_preflight_waits_for_a_late_gate(self):
        class Clock:
            def __init__(self):
                self.now = 0.0

            def sleep(self, seconds):
                self.now += seconds

        class Driver:
            def __init__(self):
                self.trees = [
                    {"tree": {"nodes": []}},
                    {"tree": {"nodes": []}},
                    {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}},
                ]

            def snapshot(self):
                return self.trees.pop(0)

        clock = Clock()
        driver = Driver()
        E2E.assert_sign_in_gate(driver, timeout_s=5, poll_s=0.5, sleep=clock.sleep, now=lambda: clock.now)
        self.assertEqual(driver.trees, [])
        self.assertEqual(clock.now, 1.0)

    def test_gate_preflight_refuses_home_at_once(self):
        class Driver:
            def __init__(self):
                self.snapshots = 0

            def snapshot(self):
                self.snapshots += 1
                return {"tree": {"nodes": [{"id": "home", "role": "group"}]}}

        driver = Driver()
        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.assert_sign_in_gate(driver, timeout_s=5, poll_s=0.5, sleep=lambda seconds: None,
                                    now=lambda: 0.0)
        self.assertEqual(driver.snapshots, 1)
        self.assertIn("home", str(raised.exception))
        self.assertIn("gate-root", str(raised.exception))

    def test_gate_preflight_refuses_when_nothing_shows_before_the_timeout(self):
        class Clock:
            def __init__(self):
                self.now = 0.0

            def sleep(self, seconds):
                self.now += seconds

        class Driver:
            def snapshot(self):
                return {"tree": {"nodes": []}}

        clock = Clock()
        with self.assertRaises(E2E.SuiteError) as raised:
            E2E.assert_sign_in_gate(Driver(), timeout_s=1, poll_s=0.5, sleep=clock.sleep,
                                    now=lambda: clock.now)
        self.assertGreaterEqual(clock.now, 1.0)
        self.assertIn("gate-root", str(raised.exception))

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_a_mock_run_refuses_a_past_gate_app_before_the_first_recipe(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            db = folder / "scratch.db"
            out = folder / "out"
            args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(out)])
            entries = []

            class AliveProcess:
                def poll(self):
                    return None

            class FakeAgent:
                def __init__(self, *args, **kwargs):
                    pass

                def hello(self):
                    return {"ready": True, "platform": "desktop", "features": []}

                def snapshot(self):
                    return {"tree": {"nodes": [{"id": "home", "role": "group"}]}}

            real_database = E2E.default_database

            def database(target, stamp, accounts="bypass"):
                return real_database(target, stamp, accounts=accounts, home=folder / "home", unique="test")

            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "default_database", side_effect=database), \
                    mock.patch.object(E2E, "start_mock_account", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "launch", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "wait_for_device_marker", return_value={}), \
                    mock.patch.object(E2E, "Agent", FakeAgent), \
                    mock.patch.object(E2E, "installed_cli_version", return_value="test"), \
                    mock.patch.object(E2E, "git_head", return_value="test"), \
                    mock.patch.object(E2E, "app_database", return_value="test"), \
                    mock.patch.object(E2E, "run_entry", side_effect=lambda *a: entries.append(a) or {"status": "pass"}), \
                    mock.patch.object(E2E, "stop"), mock.patch.object(E2E, "stop_mock_account"):
                with self.assertRaises(E2E.SuiteError) as raised:
                    E2E.run(args)
            self.assertIn("the app started past the sign-in gate", str(raised.exception))
            self.assertEqual(entries, [])

    def test_an_android_mock_is_refused_before_any_dev_env_write_or_launch(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            with mock.patch.object(E2E, "fresh_android") as fresh, \
                    mock.patch.object(E2E, "launch") as launch, \
                    mock.patch.object(E2E, "start_mock_account") as start_mock:
                with self.assertRaises(E2E.SuiteError):
                    E2E.parse_args(["android", str(suite), "--accounts", "mock", "--out", str(Path(folder) / "out")])
            fresh.assert_not_called()
            launch.assert_not_called()
            start_mock.assert_not_called()

    def run_fake_mock_suite(self, folder, recipes, events, database="test", info_override=None):
        folder = Path(folder)
        for name in {entry["file"] for entry in recipes}:
            write(folder, name, "click x\n")
        suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": recipes}))
        args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(folder / "out")])

        class AliveProcess:
            def poll(self):
                return None

        class FakeAgent:
            def __init__(self, *args, **kwargs):
                self.env = args[2] if len(args) > 2 and isinstance(args[2], dict) else {}

            def hello(self):
                return {"ready": True, "platform": "desktop", "features": []}

            def snapshot(self):
                return {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}}

            def json(self, *args, **kwargs):
                events.append("app.info")
                device = self.env.get("BIR_DEVICE_DIR", "")
                info = {"device_dir": device, "rooms_mode": True}
                if database:
                    info["database_path"] = str(Path(device) / "rooms" / "room" / "bir_data.db")
                if info_override is not None:
                    info = info_override
                return 0, {"ok": True, "result": info}, ""

        def run_entry(index, entry, *rest):
            events.append("recipe:" + entry["name"])
            return {"name": entry["name"], "status": "pass", "ms": 0}

        def default_database_for_test(target, stamp, accounts="bypass"):
            if accounts == "mock":
                device = (folder / "home" / "BuwizNsh" / "agents" / f"e2e-{target}-{stamp}-test").resolve()
                return device / "bir_data.db", device
            return (folder / "home" / "BuwizNsh" / "agents" / f"e2e-{target}-{stamp}.db").resolve(), None

        def fast_layout(data, context, log, need_database=False):
            return REAL_MOCK_LAYOUT_IS_PROVEN(data, context, log, need_database=need_database, timeout_s=0,
                               sleep=lambda seconds: None, now=lambda: 0.0)

        def launch(target, db, env, out, log, just):
            device = Path(env["BIR_DEVICE_DIR"])
            marker = {"device_dir": str(device.resolve()), "rooms": True, "pid": os.getpid(), "seam": 1}
            (device / E2E.DEVICE_MARKER_NAME).write_text(json.dumps(marker), encoding="utf-8")
            room = device / "rooms" / "room"
            room.mkdir(parents=True, exist_ok=True)
            (room / "bir_data.db").write_text("db", encoding="utf-8")
            return AliveProcess()

        patches = [mock.patch.object(E2E, "check_suite", return_value=[]),
                   mock.patch.object(E2E, "port_answers", return_value=False),
                   mock.patch.object(E2E, "default_database", side_effect=default_database_for_test),
                   mock.patch.object(E2E, "start_mock_account", return_value=AliveProcess()),
                   mock.patch.object(E2E, "launch", side_effect=launch),
                   mock.patch.object(E2E, "Agent", FakeAgent),
                   mock.patch.object(E2E, "installed_cli_version", return_value="test"),
                   mock.patch.object(E2E, "git_head", return_value="test"),
                   mock.patch.object(E2E, "mock_layout_is_proven", side_effect=fast_layout),
                   mock.patch.object(E2E, "run_entry", side_effect=run_entry),
                   mock.patch.object(E2E, "stop"), mock.patch.object(E2E, "stop_mock_account")]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        return args

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_a_mock_run_uses_and_removes_a_per_run_database_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(folder / "out")])
            launched = []

            class AliveProcess:
                def poll(self):
                    return None

            class FakeAgent:
                def __init__(self, *args, **kwargs):
                    pass

                def hello(self):
                    return {"ready": True, "platform": "desktop", "features": []}

                def snapshot(self):
                    return {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}}

            real_database = E2E.default_database

            def database(target, stamp, accounts="bypass"):
                return real_database(target, stamp, accounts=accounts, home=folder / "home", unique="test")

            def launch(target, db, env, out, log, just):
                launched.append((db, dict(env)))
                device = Path(env["BIR_DEVICE_DIR"])
                marker = {"device_dir": str(device.resolve()), "rooms": True, "pid": os.getpid(), "seam": 1}
                (device / E2E.DEVICE_MARKER_NAME).write_text(json.dumps(marker), encoding="utf-8")
                room = device / "rooms" / "room"
                room.mkdir(parents=True, exist_ok=True)
                (room / "bir_data.db").write_text("db", encoding="utf-8")
                return AliveProcess()

            os.environ["BIR_DATABASE_PATH"] = str(folder / "repository-dotenv-leak.db")
            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "default_database", side_effect=database), \
                    mock.patch.object(E2E, "start_mock_account", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "launch", side_effect=launch), \
                    mock.patch.object(E2E, "Agent", FakeAgent), \
                    mock.patch.object(E2E, "installed_cli_version", return_value="test"), \
                    mock.patch.object(E2E, "git_head", return_value="test"), \
                    mock.patch.object(E2E, "app_info", side_effect=lambda *rest: {
                        "device_dir": launched[0][1]["BIR_DEVICE_DIR"], "rooms_mode": True,
                        "database_path": str(Path(launched[0][1]["BIR_DEVICE_DIR"]) / "rooms" / "room" / "bir_data.db")}), \
                    mock.patch.object(E2E, "run_entry", return_value={"name": "x", "status": "pass", "ms": 0}), \
                    mock.patch.object(E2E, "stop"), mock.patch.object(E2E, "stop_mock_account"):
                self.assertEqual(E2E.run(args), 0)
            self.assertEqual(len(launched), 1)
            db, env = launched[0]
            device = Path(env["BIR_DEVICE_DIR"])
            self.assertTrue(device.name.startswith("e2e-macos-"), device)
            self.assertTrue(device.name.endswith("-test"), device)
            self.assertEqual(db, Path(env["BIR_DATABASE_PATH"]),
                         "the positional database must be the runner's own, never a .env value")
            self.assertEqual(db, device / "bir_data.db")
            self.assertFalse(device.exists(), "the per-run device folder goes with the run")

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_the_marker_wait_starts_after_the_windows_hello(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(folder / "out")])
            events = []

            class AliveProcess:
                def poll(self):
                    return None

            class FakeAgent:
                def __init__(self, *args, **kwargs):
                    pass

                def hello(self):
                    return {"ready": True, "platform": "desktop", "features": []}

                def snapshot(self):
                    return {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}}

            real_database = E2E.default_database

            def database(target, stamp, accounts="bypass"):
                return real_database(target, stamp, accounts=accounts, home=folder / "home", unique="test")

            def wait_for(agent, proc, timeout, log, what):
                events.append(what)
                if what == "window plane":
                    return agent.hello()
                return None

            def marker(*args, **kwargs):
                events.append("device-marker")
                if "window plane" not in events:
                    raise AssertionError("the device-marker wait started before the window plane hello")
                self.assertEqual(kwargs.get("timeout_s"), 10)
                return {}

            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "default_database", side_effect=database), \
                    mock.patch.object(E2E, "start_mock_account", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "launch", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "wait_for", side_effect=wait_for), \
                    mock.patch.object(E2E, "wait_for_device_marker", side_effect=marker), \
                    mock.patch.object(E2E, "Agent", FakeAgent), \
                    mock.patch.object(E2E, "installed_cli_version", return_value="test"), \
                    mock.patch.object(E2E, "git_head", return_value="test"), \
                    mock.patch.object(E2E, "mock_layout_is_proven", return_value=True), \
                    mock.patch.object(E2E, "run_entry", return_value={"name": "x", "status": "pass", "ms": 0}), \
                    mock.patch.object(E2E, "stop"), mock.patch.object(E2E, "stop_mock_account"):
                self.assertEqual(E2E.run(args), 0)
            self.assertLess(events.index("window plane"), events.index("device-marker"))

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_a_refused_preexisting_mock_folder_is_not_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(folder / "out")])
            device = folder / "home" / "BuwizNsh" / "agents" / "e2e-macos-test"
            device.mkdir(parents=True)
            sentinel = device / "keep"
            sentinel.write_text("not this run's folder", encoding="utf-8")

            def database(target, stamp, accounts="bypass"):
                return device / "bir_data.db", device

            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "default_database", side_effect=database), \
                    mock.patch.object(E2E, "start_mock_account") as start_mock:
                with self.assertRaises(E2E.SuiteError) as raised:
                    E2E.run(args)
            self.assertIn("already exists", str(raised.exception))
            start_mock.assert_not_called()
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "not this run's folder")
            self.assertTrue(device.exists())

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_the_rooms_database_proof_waits_for_sign_in_and_precedes_the_first_write_recipe(self):
        with tempfile.TemporaryDirectory() as folder:
            events = []
            recipes = [
                {"name": "sign-in-start", "file": "a.wants", "sign_in": True},
                {"name": "sign-in-finish", "file": "b.wants", "sign_in": True},
                {"name": "write", "file": "c.wants"},
            ]
            args = self.run_fake_mock_suite(folder, recipes, events)
            self.assertEqual(E2E.run(args), 0)
        self.assertEqual(events, ["recipe:sign-in-start", "recipe:sign-in-finish", "app.info", "recipe:write"])

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_a_mock_write_recipe_does_not_run_without_the_rooms_database_proof(self):
        with tempfile.TemporaryDirectory() as folder:
            events = []
            args = self.run_fake_mock_suite(folder, [{"name": "write", "file": "a.wants"}], events,
                                            database=None)
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.run(args)
        self.assertIn("database_path", str(raised.exception))
        self.assertEqual(events, ["app.info"])

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_the_post_sign_in_proof_refuses_a_bad_rooms_layout_or_outside_the_run_folder(self):
        cases = (
            ({}, "device_dir"),
            ({"device_dir": "/tmp"}, "rooms_mode"),
            ({"device_dir": "/tmp", "rooms_mode": True, "database_path": "/tmp/bir_data.db"}, "outside"),
        )
        for info, expected in cases:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as folder:
                events = []
                args = self.run_fake_mock_suite(folder, [{"name": "write", "file": "a.wants"}], events,
                                                info_override=info)
                with self.assertRaises(E2E.SuiteError) as raised:
                    E2E.run(args)
                self.assertIn(expected, str(raised.exception))
                self.assertEqual(events, ["app.info"])

    @mock.patch.dict(os.environ, clean_env(), clear=True)
    def test_a_recipe_failure_still_stops_the_mock(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            write(folder, "x.wants", "click x\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [{"name": "x", "file": "x.wants"}]}))
            args = E2E.parse_args(["macos", str(suite), "--accounts", "mock", "--out", str(folder / "out")])

            class AliveProcess:
                def poll(self):
                    return None

            class FakeAgent:
                def __init__(self, *args, **kwargs):
                    pass

                def hello(self):
                    return {"ready": True, "platform": "desktop", "features": []}

                def snapshot(self):
                    return {"tree": {"nodes": [{"id": "gate-root", "role": "group"}]}}

            real_database = E2E.default_database

            def database(target, stamp, accounts="bypass"):
                return real_database(target, stamp, accounts=accounts, home=folder / "home", unique="test")

            with mock.patch.object(E2E, "check_suite", return_value=[]), \
                    mock.patch.object(E2E, "port_answers", return_value=False), \
                    mock.patch.object(E2E, "default_database", side_effect=database), \
                    mock.patch.object(E2E, "start_mock_account", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "launch", return_value=AliveProcess()), \
                    mock.patch.object(E2E, "wait_for_device_marker", return_value={}), \
                    mock.patch.object(E2E, "Agent", FakeAgent), \
                    mock.patch.object(E2E, "installed_cli_version", return_value="test"), \
                    mock.patch.object(E2E, "git_head", return_value="test"), \
                    mock.patch.object(E2E, "mock_layout_is_proven", return_value=True), \
                    mock.patch.object(E2E, "run_entry", return_value={"name": "x", "status": "fail", "ms": 0,
                                                                       "steps": []}), \
                    mock.patch.object(E2E, "stop"), \
                    mock.patch.object(E2E, "stop_mock_account") as stop_mock:
                self.assertEqual(E2E.run(args), 1)
            stop_mock.assert_called_once()

    def test_the_fixture_addresses_and_code_sequence_are_invented(self):
        params = E2E.mock_account_params()
        self.assertEqual(params["account_a"], "account-a@example.com")
        self.assertEqual(params["account_b"], "account-b@example.com")
        self.assertEqual(params["code_1"], "246810")
        self.assertEqual(params["code_2"], "246811")
        self.assertEqual(params["code_8"], "246817")
        self.assertTrue(all(value.endswith("@example.com") for key, value in params.items()
                            if key.startswith("account_")))

    def test_check_suite_accepts_the_runner_account_params(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "sign-in.wants", "type gate-email $account_a\nwait_until --timeout-ms 100 gate-code-boxes\n")
            suite = suite_of(folder, [{"name": "sign-in", "file": "sign-in.wants", "sign_in": True}])
            without = "\n".join(E2E.check_suite(suite))
            self.assertIn("$account_a", without)
            self.assertEqual(E2E.check_suite(suite, extra_params=E2E.mock_account_params()), [])

    def test_mock_mode_allows_exact_runner_code_fixtures_only_in_the_gate_code_box(self):
        params = E2E.mock_account_params()
        fixture = {"op": "type", "target": "gate-code-boxes", "text": "$code_1"}
        self.assertFalse(E2E.is_secret_entry(fixture, params, accounts="mock"))
        self.assertTrue(E2E.is_secret_entry(fixture, params), "bypass mode still refuses the code box")
        for text in ("246810", "${code_1}x", "$code_9", "$account_a", ""):
            with self.subTest(text=text):
                step = {**fixture, "text": text}
                self.assertTrue(E2E.is_secret_entry(step, params, accounts="mock"))
        for target in ("auth-tp-totp", "setup-change-code-cells", "login-password", "x-gate-code-boxes"):
            with self.subTest(target=target):
                step = {**fixture, "target": target}
                self.assertTrue(E2E.is_secret_entry(step, params, accounts="mock"))
        for op in ("set_value", "paste", "key"):
            with self.subTest(op=op):
                step = {**fixture, "op": op}
                self.assertTrue(E2E.is_secret_entry(step, params, accounts="mock"))
        wrong = dict(params)
        wrong["code_1"] = "000000"
        self.assertTrue(E2E.is_secret_entry(fixture, wrong, accounts="mock"))

    def test_the_mock_code_exception_is_pinned_to_the_type_step_schema(self):
        params = E2E.mock_account_params()
        fixture = {"op": "type", "target": "gate-code-boxes", "text": "$code_1", "id": "s1", "needs": [],
                   "screenshot": False, "delivery": "semantic", "expect_fingerprint": 123, "key_delay_ms": 80}
        self.assertFalse(E2E.is_secret_entry(fixture, params, accounts="mock"))
        for extra in ({"value": "000000"}, {"key": "1"}, {"args": {"text": "000000"}}, {"note": "246810"}):
            with self.subTest(extra=extra):
                self.assertTrue(E2E.is_secret_entry({**fixture, **extra}, params, accounts="mock"))
        self.assertTrue(E2E.is_secret_entry({**fixture, "id": "246810"}, params, accounts="mock"))

    def test_check_suite_plumbs_the_mock_code_exception_and_bypass_still_refuses(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "sign-in.wants", "type gate-code-boxes $code_1\n")
            suite = suite_of(folder, [{"name": "sign-in", "file": "sign-in.wants", "sign_in": True}])
            bypass = "\n".join(E2E.check_suite(suite, extra_params=E2E.mock_account_params()))
            self.assertIn("code box", bypass)
            self.assertEqual(E2E.check_suite(suite, extra_params=E2E.mock_account_params(), accounts="mock"), [])

    def test_the_check_command_uses_the_mock_code_exception_only_in_mock_mode(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "sign-in.wants", "type gate-code-boxes $code_1\n")
            suite = write(folder, "s.json", json.dumps({"v": 1, "recipes": [
                {"name": "sign-in", "file": "sign-in.wants", "sign_in": True}]}))
            with mock.patch.object(sys, "stdout", io.StringIO()):
                self.assertEqual(E2E.check(str(suite), "missing-gpui-agent", accounts="mock"), 0)
                self.assertEqual(E2E.check(str(suite), "missing-gpui-agent"), 1)

    def test_runner_account_params_win_over_suite_and_entry_values(self):
        context = {"params": E2E.mock_account_params()}
        suite = {"params": {"account_a": "real@example.net", "tin": "12345678800000"}}
        entry = {"set": {"account_b": "real@example.net"}}
        params = E2E.recipe_params(suite, entry, context)
        self.assertEqual(params["account_a"], "account-a@example.com")
        self.assertEqual(params["account_b"], "account-b@example.com")
        self.assertEqual(params["tin"], "12345678800000")

    def test_android_points_at_the_mock_the_scripts_way_and_cleans_up(self):
        class Done:
            returncode = 0
            stdout = ""
            stderr = ""

        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return Done()

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            self.assertEqual(E2E.android_mock_reverse({"ANDROID_SERIAL": "emulator-5554"}, log),
                             ("adb", "emulator-5554"))
            E2E.remove_android_mock_reverse(("adb", "emulator-5554"), log)
        self.assertEqual(calls, [
            ["adb", "-s", "emulator-5554", "reverse", "--list"],
            ["adb", "-s", "emulator-5554", "reverse", "tcp:3008", "tcp:3008"],
            ["adb", "-s", "emulator-5554", "reverse", "--remove", "tcp:3008"],
        ])

    def test_an_existing_android_reverse_is_refused_not_clobbered(self):
        class Done:
            returncode = 0
            stdout = "List of all reverse connections:\n    tcp:3008 tcp:3008\n"
            stderr = ""

        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return Done()

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run), \
                self.assertRaises(E2E.SuiteError) as raised:
            E2E.android_mock_reverse({"ANDROID_SERIAL": "emulator-5554"}, log)
        self.assertIn("already exists", str(raised.exception))
        self.assertEqual(calls, [["adb", "-s", "emulator-5554", "reverse", "--list"]])

    def test_an_android_reverse_failure_is_loud(self):
        class Done:
            returncode = 1
            stderr = "closed"

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E, "android_adb", return_value="adb"), \
                mock.patch.object(E2E.subprocess, "run", return_value=Done()), \
                self.assertRaises(E2E.SuiteError) as raised:
            E2E.android_mock_reverse({"ANDROID_SERIAL": "emulator-5554"}, log)
        self.assertIn("adb reverse", str(raised.exception))


    def test_assert_and_wait_until_use_the_drivers_target_spellings(self):
        cases = (
            ("assert gate-root", "gate-root"),
            ("assert --id gate-root", "gate-root"),
            ("assert --target gate-root", "gate-root"),
            ("assert id=gate-root", "gate-root"),
            ("assert target=gate-root", "gate-root"),
            ("assert --name Root --visible true target=gate-root", "gate-root"),
            ("wait_until --timeout-ms 100 gate-root", "gate-root"),
            ("wait_until --timeout-ms 100 --target gate-root", "gate-root"),
            ("wait_until timeout_ms=100 id=gate-root --exists false", "gate-root"),
        )
        with tempfile.TemporaryDirectory() as folder:
            for number, (line, target) in enumerate(cases):
                with self.subTest(line=line):
                    path = write(folder, f"a{number}.wants", line + "\n")
                    self.assertEqual(E2E.recipe_steps(path)[0].get("target"), target)

    def test_sign_in_assert_and_wait_until_target_spellings_cannot_escape_the_allowlist(self):
        spellings = ("assert --id home", "assert --target home", "assert id=home", "assert target=home",
                     "wait_until --timeout-ms 100 --target home", "wait_until timeout_ms=100 id=home")
        with tempfile.TemporaryDirectory() as folder:
            for number, line in enumerate(spellings):
                with self.subTest(line=line):
                    path = write(folder, f"bad{number}.wants", line + "\n")
                    step = E2E.recipe_steps(path)[0]
                    self.assertEqual(E2E.sign_in_recipe_problem(step), "node 'home'")

    def test_the_first_run_setup_allow_list_is_explicit_and_excludes_the_writing_controls(self):
        for target in E2E.SIGN_IN_SETUP_CONTAINERS:
            with self.subTest(target=target):
                self.assertIsNone(E2E.sign_in_recipe_problem({"op": "assert", "target": target}))
                self.assertEqual(E2E.sign_in_recipe_problem({"op": "click", "target": target}), f"node {target!r}")
        for target in E2E.SIGN_IN_SETUP_ACTING_IDS:
            with self.subTest(target=target):
                self.assertIsNone(E2E.sign_in_recipe_problem({"op": "click", "target": target}))
        for target in ("setup-solo", "setup-pin-submit", "setup-key-done", "setup-change-confirm-btn", "setup-change-pin",
                       "setup-solo-card", "setup-helpdesk-card"):
            with self.subTest(target=target):
                self.assertEqual(E2E.sign_in_recipe_problem({"op": "click", "target": target}), f"node {target!r}")

    def test_a_literal_dollar_does_not_skip_the_sign_in_prefix_check(self):
        self.assertEqual(E2E.sign_in_recipe_problem({"op": "click", "target": "home$"}), "node 'home$'")
        self.assertEqual(E2E.sign_in_recipe_problem({"op": "click", "target": "home$$"}), "node 'home$'")
        self.assertIsNone(E2E.sign_in_recipe_problem({"op": "click", "target": "$resolved_later"}))

    def test_a_sign_in_recipe_keeps_the_send_step_check(self):
        problems = E2E.step_problems([{"op": "click", "target": "fp-queue"}], {}, "x.wants", {"sign_in": True})
        self.assertTrue(any("a send step" in problem for problem in problems), problems)

    def test_a_mock_send_step_uses_the_rooms_database_proof_instead_of_database_test(self):
        entry = {"name": "queue", "file": "q.wants", "send": True}
        context = {"target": "macos", "form_factor": "desktop", "features": [], "cli_version": "0.5.0",
                   "allow_send": True, "env": {"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1"},
                   "accounts": "mock", "database": None, "rooms_database_proven": False}
        self.assertEqual(E2E.plan_entry(entry, {}, context)[2], "refused")
        context["rooms_database_proven"] = True
        self.assertIsNone(E2E.plan_entry(entry, {}, context)[2])

    def test_mock_launch_env_sets_both_device_dir_and_a_database_inside_it(self):
        with tempfile.TemporaryDirectory() as folder:
            device = Path(folder).resolve() / "device"
            database = device / "bir_data.db"
            env = E2E.launch_env({"PATH": "/bin", "BIR_DATABASE_PATH": "/real/bir_data.db"}, "tok", "127.0.0.1:1",
                                 accounts="mock", device_dir=device, database_path=database)
            self.assertEqual(env["BIR_DEVICE_DIR"], str(device))
            self.assertEqual(env["BIR_DATABASE_PATH"], str(database))
            self.assertEqual(env["BIR_AUTH_BYPASS"], "0")
        env = E2E.launch_env({"PATH": "/bin", "BIR_DEVICE_DIR": "/elsewhere"}, "tok", "127.0.0.1:1")
        self.assertNotIn("BIR_DEVICE_DIR", env)

    def test_mock_launch_env_refuses_a_relative_or_outside_device_layout(self):
        with tempfile.TemporaryDirectory() as folder:
            device = Path(folder).resolve() / "device"
            for device_dir, database in ((Path("relative-device"), Path("relative-device/bir_data.db")),
                                         (device, Path(folder).resolve() / "outside.db")):
                with self.subTest(device_dir=device_dir, database=database):
                    with self.assertRaises(E2E.SuiteError):
                        E2E.launch_env({"PATH": "/bin"}, "tok", "127.0.0.1:1", accounts="mock",
                                       device_dir=device_dir, database_path=database)

    def test_platform_default_data_dirs_follow_the_platform_sources(self):
        home = Path("/home/example")
        self.assertEqual(E2E.platform_default_data_dir("macos", {}, home), home / "Library" / "Application Support" / "dev.goldcoders.bir")
        self.assertEqual(E2E.platform_default_data_dir("ios", {}, home), home / "Library" / "Application Support" / "dev.goldcoders.bir")
        self.assertEqual(E2E.platform_default_data_dir("linux", {}, home), home / ".local" / "share" / "dev.goldcoders.bir")
        xdg = Path(tempfile.gettempdir()).resolve() / "xdg"
        self.assertEqual(E2E.platform_default_data_dir("linux", {"XDG_DATA_HOME": str(xdg)}, home),
                         xdg / "dev.goldcoders.bir")
        self.assertEqual(E2E.platform_default_data_dir("linux", {"XDG_DATA_HOME": "relative"}, home), home / ".local" / "share" / "dev.goldcoders.bir")
        self.assertEqual(E2E.platform_default_data_dir("windows", {"LOCALAPPDATA": "C:\\Local"}, home), Path("C:\\Local/dev.goldcoders.bir"))
        self.assertEqual(E2E.platform_default_data_dir("windows", {}, home), home / "AppData" / "Local" / "dev.goldcoders.bir")
        self.assertEqual(E2E.platform_default_data_dir("android", {}, home), Path("/data/data/com.longbridge.nsh.host/files/nsh/dev.goldcoders.bir"))

    def test_the_rooms_layout_proof_requires_the_seam_and_the_per_run_folder(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder).resolve()
            default = folder / "platform-default"
            good = {"device_dir": str(folder), "rooms_mode": True}
            self.assertIsNone(E2E.mock_layout_problem(good, folder, default))
            for bad, missing in (({}, "device_dir"), ({"device_dir": str(folder)}, "rooms_mode"),
                                 ({"device_dir": str(folder), "rooms_mode": False}, "rooms_mode")):
                with self.subTest(bad=bad):
                    self.assertIn(missing, E2E.mock_layout_problem(bad, folder, default) or "")
            self.assertIn("outside", E2E.mock_layout_problem({"device_dir": str(folder.parent), "rooms_mode": True},
                                                             folder, default) or "")
            self.assertIn("default data folder", E2E.mock_layout_problem({"device_dir": str(default),
                                                                          "rooms_mode": True}, folder, default) or "")
            self.assertIn("database_path", E2E.mock_layout_problem(good, folder, default, need_database=True) or "")
            room = folder / "rooms" / "a"
            room.mkdir(parents=True)
            (room / "bir_data.db").write_text("db", encoding="utf-8")
            inside = {**good, "database_path": str(room / "bir_data.db")}
            self.assertIsNone(E2E.mock_layout_problem(inside, folder, default, need_database=True))
            outside = {**good, "database_path": str(folder.parent / "bir_data.db")}
            self.assertIn("database", E2E.mock_layout_problem(outside, folder, default, need_database=True) or "")

    def test_the_database_proof_refuses_a_single_room_database(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder).resolve()
            good = {"device_dir": str(folder), "rooms_mode": True}
            for relative in ("bir_data.db", "rooms/bir_data.db", "rooms/room/other.db",
                             "rooms/room/nested/bir_data.db"):
                with self.subTest(relative=relative):
                    path = folder.joinpath(*relative.split("/"))
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text("db", encoding="utf-8")
                    problem = E2E.mock_layout_problem({**good, "database_path": str(path)}, folder,
                                                      need_database=True)
                    self.assertIsNotNone(problem, relative)
                    self.assertIn("single-room", problem)
            missing = folder / "rooms" / "room" / "bir_data.db"
            missing.parent.mkdir(parents=True, exist_ok=True)
            problem = E2E.mock_layout_problem({**good, "database_path": str(missing)}, folder, need_database=True)
            self.assertIsNotNone(problem)
            self.assertIn("does not exist", problem)
            missing.write_text("db", encoding="utf-8")
            self.assertIsNone(E2E.mock_layout_problem({**good, "database_path": str(missing)}, folder,
                                                      need_database=True))

    def test_a_suite_plant_entry_is_mock_only_and_names_the_legacy_fixture_generator(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "x.wants", "click gate-root\n")
            for plant, accounts, expected in (
                ("legacy-fixture", "bypass", "without mock"),
                ("other-generator", "mock", "legacy-fixture"),
                ("legacy-fixture", "mock", None),
            ):
                with self.subTest(plant=plant, accounts=accounts):
                    suite = suite_of(folder, [{"name": "x", "file": "x.wants", "sign_in": True}])
                    suite["plant"] = plant
                    problems = "\n".join(E2E.check_suite(suite, accounts=accounts))
                    if expected is None:
                        self.assertNotIn('suite "plant"', problems)
                    else:
                        self.assertIn(expected, problems)

    def test_planting_the_legacy_fixture_uses_only_the_run_database_path(self):
        self.assertTrue(E2E.just_has_recipe("e2e"))
        self.assertFalse(E2E.just_has_recipe("legacy-fixture"), "this checkout is before the app-side seam")
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "", "")

        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            log = E2E.RunLog(None, echo=False)
            with mock.patch.object(E2E, "just_has_recipe", return_value=True):
                E2E.plant_legacy_fixture("just", folder, log, run=run)
            self.assertEqual(calls, [["just", "legacy-fixture", str(folder / "bir_data.db")]])
            with mock.patch.object(E2E, "just_has_recipe", return_value=False), \
                    self.assertRaises(E2E.SuiteError) as missing:
                E2E.plant_legacy_fixture("just", folder, log, run=run)
            self.assertIn("legacy-fixture", str(missing.exception))

            def failing(command, **kwargs):
                return subprocess.CompletedProcess(command, 1, "", "no fixture")

            with mock.patch.object(E2E, "just_has_recipe", return_value=True), \
                    self.assertRaises(E2E.SuiteError) as failed:
                E2E.plant_legacy_fixture("just", folder, log, run=failing)
            self.assertIn("no fixture", str(failed.exception))

            def slow(command, **kwargs):
                raise subprocess.TimeoutExpired(command, kwargs.get("timeout"))

            with mock.patch.object(E2E, "just_has_recipe", return_value=True), \
                    self.assertRaises(E2E.SuiteError) as timed_out:
                E2E.plant_legacy_fixture("just", folder, log, run=slow)
            self.assertIn("120", str(timed_out.exception))

    def test_windows_commands_are_printed_with_windows_quoting(self):
        command = ["C:\\Android SDK\\platform-tools\\adb.exe", "-s", "emulator-5554", "shell", "run-as",
                   E2E.ANDROID_HOST, "rm", "-f", E2E.ANDROID_DEV_ENV]
        with mock.patch.object(E2E, "os", types.SimpleNamespace(name="nt")):
            self.assertEqual(E2E.command_for_user(command),
                             "& 'C:\\Android SDK\\platform-tools\\adb.exe' -s emulator-5554 shell run-as "
                             "com.longbridge.nsh.host rm -f files/nsh/dev.goldcoders.bir/dev.env")
        with mock.patch.object(E2E, "os", types.SimpleNamespace(name="posix")):
            self.assertEqual(E2E.command_for_user(command), E2E.shlex.join(command))


class RoomsMarkerProofTests(unittest.TestCase):
    def marker(self, folder, **changes):
        data = {"device_dir": str(Path(folder).resolve()), "rooms": True, "pid": 4321, "seam": 1}
        data.update(changes)
        return data

    def write_marker(self, folder, data):
        path = Path(folder) / ".ebirforms-test-device.json"
        if isinstance(data, str):
            path.write_text(data, encoding="utf-8")
        else:
            path.write_text(json.dumps(data), encoding="utf-8")
        return path

    def test_prepare_mock_device_dir_refuses_an_existing_folder_or_stale_marker(self):
        with tempfile.TemporaryDirectory() as folder:
            existing = Path(folder) / "run"
            existing.mkdir()
            self.write_marker(existing, self.marker(existing))
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.prepare_mock_device_dir(existing)
            self.assertIn("stale", str(raised.exception).lower())
            self.assertTrue((existing / ".ebirforms-test-device.json").exists())
            fresh = Path(folder) / "fresh"
            E2E.prepare_mock_device_dir(fresh)
            self.assertTrue(fresh.is_dir())

    def test_the_launch_marker_is_proved_when_it_appears_late_inside_the_bound(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder).resolve()
            proc = mock.Mock()
            proc.poll.return_value = None
            log = E2E.RunLog(None, echo=False)
            clock = [0.0]
            writes = []

            def sleep(seconds):
                clock[0] += seconds
                if len(writes) == 1:
                    self.write_marker(folder, self.marker(folder))
                writes.append(seconds)

            log_path = Path(folder) / "app.log"
            log_path.write_text("", encoding="utf-8")
            found = E2E.wait_for_device_marker(folder, proc, log_path, log, timeout_s=1, poll_s=0.25,
                                               sleep=sleep, now=lambda: clock[0])
            self.assertEqual(found["device_dir"], str(folder))
            self.assertTrue(any("device marker" in line for line in log.lines), log.lines)

    def test_a_partial_launch_marker_is_retried_until_complete(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder).resolve()
            self.write_marker(folder, "")
            proc = mock.Mock()
            proc.poll.return_value = None
            clock = [0.0]
            writes = []

            def sleep(seconds):
                clock[0] += seconds
                if not writes:
                    self.write_marker(folder, self.marker(folder))
                writes.append(seconds)

            found = E2E.wait_for_device_marker(folder, proc, folder / "app.log", E2E.RunLog(None, echo=False),
                                               timeout_s=1, poll_s=0.25, sleep=sleep, now=lambda: clock[0])
            self.assertEqual(found["device_dir"], str(folder))
            self.assertTrue(writes)

    def test_a_locked_launch_marker_is_retried_until_it_can_be_read(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder).resolve()
            marker = self.write_marker(folder, self.marker(folder))
            proc = mock.Mock()
            proc.poll.return_value = None
            real_read_text = Path.read_text
            locked = []

            def read_text(path, *args, **kwargs):
                if path == marker and len(locked) < 2:
                    locked.append(path)
                    raise PermissionError("being replaced")
                return real_read_text(path, *args, **kwargs)

            clock = [0.0]

            def sleep(seconds):
                clock[0] += seconds

            with mock.patch.object(Path, "read_text", read_text):
                found = E2E.wait_for_device_marker(folder, proc, folder / "app.log", E2E.RunLog(None, echo=False),
                                                   timeout_s=1, poll_s=0.25, sleep=sleep, now=lambda: clock[0])
            self.assertEqual(found["device_dir"], str(folder))
            self.assertEqual(len(locked), 2)

    def test_windows_extended_prefixes_are_normalised_before_comparison(self):
        for raw, plain in ((r"\\?\C:\Runs\one", r"C:\Runs\one"),
                           (r"\\?\UNC\server\share\one", r"\\server\share\one"),
                           (r"C:\Runs\one", r"C:\Runs\one")):
            with self.subTest(raw=raw):
                self.assertEqual(E2E.mock_layout_path(PureWindowsPath(raw)), PureWindowsPath(plain))
        folder = PureWindowsPath(r"C:\Runs\one")
        marker = {"device_dir": r"\\?\C:\Runs\one", "rooms": True, "pid": 4321, "seam": 1}
        self.assertIsNone(E2E.device_marker_problem(marker, folder))

    def test_a_missing_launch_marker_is_the_exact_pre_seam_refusal(self):
        with tempfile.TemporaryDirectory() as folder:
            proc = mock.Mock()
            proc.poll.return_value = None
            clock = [0.0]

            def sleep(seconds):
                clock[0] += seconds

            log_path = Path(folder) / "app.log"
            log_path.write_text("", encoding="utf-8")
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.wait_for_device_marker(Path(folder), proc, log_path, E2E.RunLog(None, echo=False),
                                           timeout_s=0.5, poll_s=0.25, sleep=sleep, now=lambda: clock[0])
            self.assertEqual(str(raised.exception),
                             "this app build has no BIR_DEVICE_DIR seam: mock sign-in runs need an app with it "
                             "(see docs/proof/E2E.md)")

    def test_bad_launch_markers_are_refused(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root).resolve()
            folder = root / "run"
            folder.mkdir()
            cases = (
                ("wrong folder", self.marker(folder, device_dir=str(root)), "not the per-run folder"),
                ("rooms false", self.marker(folder, rooms=False), "rooms"),
                ("seam missing", {k: v for k, v in self.marker(folder).items() if k != "seam"}, "seam"),
                ("unparsable", "not json", "JSON"),
                ("bad pid", self.marker(folder, pid=0), "pid"),
            )
            for name, data, expected in cases:
                with self.subTest(name=name):
                    marker = self.write_marker(folder, data)
                    proc = mock.Mock()
                    proc.poll.return_value = None
                    clock = [0.0]

                    def sleep(seconds):
                        clock[0] += seconds

                    with self.assertRaises(E2E.SuiteError) as raised:
                        E2E.wait_for_device_marker(folder, proc, folder / "app.log", E2E.RunLog(None, echo=False),
                                                   timeout_s=1, poll_s=0.1, sleep=sleep,
                                                   now=lambda: clock[0])
                    self.assertIn(expected, str(raised.exception))
                    marker.unlink()

    def test_an_early_launcher_exit_reports_the_apps_own_error_line(self):
        with tempfile.TemporaryDirectory() as folder:
            log_path = Path(folder) / "app.log"
            log_path.write_text("build noise\ninvalid BIR_DEVICE_DIR: relative path\n", encoding="utf-8")
            proc = mock.Mock()
            proc.poll.return_value = 2
            proc.returncode = 2
            with self.assertRaises(E2E.SuiteError) as raised:
                E2E.wait_for_device_marker(Path(folder), proc, log_path, E2E.RunLog(None, echo=False),
                                           timeout_s=30, poll_s=0.1, sleep=lambda seconds: None,
                                           now=lambda: 0.0)
            self.assertIn("invalid BIR_DEVICE_DIR: relative path", str(raised.exception))
            self.assertNotIn("no BIR_DEVICE_DIR seam", str(raised.exception))

    def test_the_post_sign_in_database_proof_retries_until_the_room_is_open(self):
        folder = Path(tempfile.gettempdir()).resolve() / "e2e-proof-room"
        room = folder / "rooms" / "room"
        room.mkdir(parents=True, exist_ok=True)
        self.addCleanup(shutil.rmtree, folder, True)
        (room / "bir_data.db").write_text("db", encoding="utf-8")
        answers = [
            {"device_dir": str(folder), "rooms_mode": True},
            {"device_dir": str(folder), "rooms_mode": True,
             "database_path": str(folder / "rooms" / "room" / "bir_data.db")},
        ]

        class Data:
            def __init__(self):
                self.calls = 0

            def json(self, *args, **kwargs):
                answer = answers[min(self.calls, len(answers) - 1)]
                self.calls += 1
                return 0, {"result": answer}, ""

        data = Data()
        context = {"device_dir": folder, "target": "macos"}
        clock = [0.0]

        def sleep(seconds):
            clock[0] += seconds

        self.assertTrue(E2E.mock_layout_is_proven(data, context, E2E.RunLog(None, echo=False), need_database=True,
                                                  timeout_s=1, poll_s=0.25, sleep=sleep,
                                                  now=lambda: clock[0]))
        self.assertEqual(data.calls, 2)
        self.assertEqual(context["database_path"], str(folder / "rooms" / "room" / "bir_data.db"))


class SignInBoundaryParserTests(unittest.TestCase):
    def problems_for(self, folder, line):
        name = "case.wants"
        write(folder, name, line + "\n")
        suite = suite_of(folder, [{"name": "sign-in", "file": name, "sign_in": True}])
        return "\n".join(E2E.check_suite(suite, extra_params=E2E.mock_account_params(), accounts="mock"))

    def test_parser_ambiguity_probes_are_refused(self):
        probes = (
            "assert --name --screenshot gate-root home",
            r"assert gate-root\ --id home",
            "assert gate-root --id home",
        )
        with tempfile.TemporaryDirectory() as folder:
            for line in probes:
                with self.subTest(line=line):
                    self.assertIn("sign_in recipes may use only", self.problems_for(folder, line))

    def test_a_control_character_cannot_split_a_sign_in_step_differently(self):
        with tempfile.TemporaryDirectory() as folder:
            for control in ("\x1c", "\x1d", "\x1e", "\x85"):
                line = f"assert gate-root --name x{control}#y --id home"
                with self.subTest(control=repr(control)):
                    problems = self.problems_for(folder, line)
                    self.assertIn("control character", problems)
                    steps = E2E.recipe_steps(Path(folder) / "case.wants")
                    self.assertEqual(steps[0].get("target"), "home")
            self.assertEqual("", self.problems_for(folder, "click\tgate-continue"))

    def test_json_sign_in_steps_refuse_control_characters_except_tab(self):
        step = {"op": "assert", "target": "gate-root\x1c"}
        self.assertEqual(E2E.sign_in_recipe_problem(step), "a control character")
        step = {"op": "type", "target": "gate-email", "text": "a@example.com\x85"}
        self.assertEqual(E2E.sign_in_recipe_problem(step), "a control character")
        step = {"op": "type", "target": "gate-email", "text": "a@example.com\t"}
        self.assertIsNone(E2E.sign_in_recipe_problem(step))

    def test_setup_containers_are_observation_only_and_pointer_modifiers_are_refused(self):
        containers = ("setup-root", "setup-card", "setup-pin-card", "setup-key-card", "setup-taxpayer")
        with tempfile.TemporaryDirectory() as folder:
            for target in containers:
                with self.subTest(target=target):
                    self.assertIn("sign_in recipes may use only", self.problems_for(folder, f"click {target}"))
                    self.assertIn("sign_in recipes may use only", self.problems_for(folder, f"scroll {target}"))
                    self.assertEqual("", self.problems_for(folder, f"assert {target}"))
                    self.assertEqual("", self.problems_for(folder,
                                                           f"wait_until --timeout-ms 100 {target}"))
            for line in ("press setup-helpdesk --within 1,2", "click --delivery virtual setup-helpdesk"):
                with self.subTest(line=line):
                    self.assertIn("sign_in recipes may use only", self.problems_for(folder, line))

    def test_json_sign_in_steps_refuse_within_and_delivery_fields(self):
        for step in ({"op": "press", "target": "setup-helpdesk", "within": "1,2"},
                     {"op": "click", "target": "setup-helpdesk", "delivery": "virtual"}):
            with self.subTest(step=step):
                self.assertIn("sign_in recipes may use only",
                              "\n".join(E2E.step_problems([step], {}, "x.json", {"sign_in": True})))

    def test_sign_in_steps_refuse_step_level_flags_on_acting_ops(self):
        with tempfile.TemporaryDirectory() as folder:
            for line in ("click --screenshot setup-helpdesk", "click --fp 9 setup-helpdesk",
                         "type --key-delay-ms 20 setup-helpdesk secret"):
                with self.subTest(line=line):
                    self.assertIn("sign_in recipes may use only", self.problems_for(folder, line))
        for field, step in (
            ("screenshot", {"op": "click", "target": "setup-helpdesk", "screenshot": True}),
            ("key_delay_ms", {"op": "type", "target": "setup-helpdesk", "text": "x", "key_delay_ms": 20}),
            ("expect_fingerprint", {"op": "click", "target": "setup-helpdesk", "expect_fingerprint": 9}),
        ):
            with self.subTest(field=field):
                self.assertIn("sign_in recipes may use only",
                              "\n".join(E2E.step_problems([step], {}, "x.json", {"sign_in": True})))

    def test_json_sign_in_assertions_refuse_unmodeled_assertion_fields(self):
        for field in ("screenshot", "value", "role", "checked", "exists", "visible", "in_viewport", "absent", "state",
                      "states", "fp", "key_delay_ms"):
            step = {"op": "assert", "target": "gate-root", field: True}
            with self.subTest(field=field):
                self.assertIn("sign_in recipes may use only",
                              "\n".join(E2E.step_problems([step], {}, "x.json", {"sign_in": True})))

    def test_the_setup_ids_are_exactly_the_ids_in_the_tsx(self):
        text = (E2E.ROOT / "nsh" / "screens" / "auth" / "Setup.tsx").read_text(encoding="utf-8")
        found = set(re.findall(r'testID="([^"]+)"', text))
        self.assertEqual(found, {"setup-root", "setup-card", "setup-solo", "setup-helpdesk", "setup-pin-card",
                                 "setup-admin-pin", "setup-back", "setup-pin-submit", "setup-key-card",
                                 "setup-key", "setup-key-done", "setup-taxpayer"})
        self.assertNotIn("setup-solo-card", found)
        self.assertNotIn("setup-helpdesk-card", found)


class ProcessControlTests(unittest.TestCase):
    def test_the_e2e_docs_name_the_cancellation_boundary_and_fresh_queue_database(self):
        text = (E2E.ROOT / "docs" / "proof" / "E2E.md").read_text(encoding="utf-8")
        self.assertIn("SIGTERM and SIGHUP", text)
        self.assertIn("Verification boundary", text)
        self.assertIn("`queue` and `queue-send` recipes need a fresh database", text)
        self.assertNotIn("taskkill /F /IM mock_account.exe", text)
        self.assertIn("taskkill /PID <pid> /T /F", text)
        self.assertIn("adb -s <serial> shell run-as com.longbridge.nsh.host rm -f "
                      "files/nsh/dev.goldcoders.bir/dev.env", text)
        self.assertIn("adb -s <serial> reverse --remove tcp:3008", text)

    def test_agent_instructions_name_the_mock_bypass_exception(self):
        sentence = ("One exception: the e2e runner's `--accounts mock` mode may set `BIR_AUTH_BYPASS` to `0` instead "
                    "of `1` only because it pins sign-in to a loopback mock account server; the other three safety "
                    "variables are unchanged.")
        for file in ("AGENTS.md", "crew/RULES.md", "crew/playbooks/machine-order.md", "nsh/AGENTS.md"):
            with self.subTest(file=file):
                self.assertIn(sentence, (E2E.ROOT / file).read_text(encoding="utf-8"))

    @unittest.skipUnless(os.name == "posix", "SIGHUP is POSIX-only")
    def test_sigterm_and_sighup_are_converted_to_keyboard_interrupts(self):
        before = {name: E2E.signal.getsignal(getattr(E2E.signal, name)) for name in ("SIGTERM", "SIGHUP")}
        with E2E.termination_as_keyboard_interrupt():
            for name in ("SIGTERM", "SIGHUP"):
                handler = E2E.signal.getsignal(getattr(E2E.signal, name))
                self.assertTrue(callable(handler), name)
                self.assertNotEqual(handler, before[name], name)
        for name, handler in before.items():
            self.assertEqual(E2E.signal.getsignal(getattr(E2E.signal, name)), handler)

    @unittest.skipUnless(os.name == "posix", "os.killpg does not exist on Windows")
    def test_posix_stop_kills_a_process_group_that_survives_the_launcher(self):
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 4321
        proc.wait.return_value = 0
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.os, "killpg") as killpg:
            E2E.stop(proc, log)
        self.assertEqual([call.args for call in killpg.call_args_list], [
            (4321, E2E.signal.SIGINT), (4321, 0), (4321, E2E.signal.SIGKILL)])

    def test_windows_stop_uses_taskkill_for_the_whole_process_tree(self):
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 4321
        proc.wait.side_effect = [subprocess.TimeoutExpired("cmd", 30), None]
        runs = []

        def run(command, **kwargs):
            runs.append(command)
            return subprocess.CompletedProcess(command, 0, "", "")

        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.os, "name", "nt"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=run):
            E2E.stop(proc, log)
        self.assertEqual(runs[0][:2], ["taskkill", "/PID"])
        self.assertIn("4321", runs[0])
        self.assertIn("/T", runs[0])
        self.assertIn("/F", runs[0])

    def test_windows_stop_uses_taskkill_after_a_graceful_ctrl_break(self):
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 4321
        proc.wait.return_value = 0
        runs = []
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.os, "name", "nt"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=lambda command, **kwargs: runs.append(command)):
            E2E.stop(proc, log)
        proc.send_signal.assert_called_once_with(getattr(E2E.signal, "CTRL_BREAK_EVENT", E2E.signal.SIGTERM))
        self.assertEqual(len(runs), 1)
        self.assertEqual(runs[0][:2], ["taskkill", "/PID"])
        self.assertIn("4321", runs[0])

    def test_windows_stop_uses_taskkill_even_when_the_launcher_already_exited(self):
        proc = mock.Mock()
        proc.poll.return_value = 0
        proc.pid = 4321
        runs = []
        log = E2E.RunLog(None, echo=False)
        with mock.patch.object(E2E.os, "name", "nt"), \
                mock.patch.object(E2E.subprocess, "run", side_effect=lambda command, **kwargs: runs.append(command)):
            E2E.stop(proc, log)
        self.assertEqual(len(runs), 1)
        self.assertEqual(runs[0][:2], ["taskkill", "/PID"])
        self.assertIn("4321", runs[0])

    @unittest.skipUnless(os.name == "nt", "the Windows twin of the POSIX process-group test")
    def test_windows_stop_by_pid_on_every_exit_path(self):
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 4321
        proc.wait.return_value = 0
        with mock.patch.object(E2E.subprocess, "run") as run:
            E2E.stop(proc, E2E.RunLog(None, echo=False))
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0][:2], ["taskkill", "/PID"])


class FileTests(unittest.TestCase):
    def test_files_other_programs_wrote_are_masked_and_the_scratch_database_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            log = E2E.RunLog(None, secrets_=["s3cret"], echo=False)
            receipt = write(folder, "receipt.json", '{"error": "token s3cret refused"}')
            E2E.redact_file(receipt, log)
            self.assertNotIn("s3cret", receipt.read_text())
            db = folder / "e2e.db"
            state_files = [Path(f"{db}.buwiz-cloud-account.json"), Path(f"{db}.buwiz-pending-sign-in.json"),
                           folder / "buwiz-cloud-account.json", folder / "buwiz-pending-sign-in.json"]
            for path in (db, Path(f"{db}-wal"), Path(f"{db}-shm"), *state_files):
                path.write_text("x")
            E2E.remove_scratch(db, log)
            self.assertEqual(sorted(p.name for p in folder.iterdir()), [
                "buwiz-cloud-account.json", "buwiz-pending-sign-in.json", "e2e.db.buwiz-cloud-account.json",
                "e2e.db.buwiz-pending-sign-in.json", "receipt.json"])

    def test_the_mock_scratch_folder_takes_the_device_store_with_it(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder) / "agents" / "e2e-macos-20261128-010203-u"
            room = root / "rooms" / "room"
            room.mkdir(parents=True)
            for path in (root / "bir_data.db", root / "device.db", root / "buwiz-install-id",
                         root / "buwiz_submit_device_key.txt", root / "bir_data.db.buwiz-cloud-account.json",
                         root / "bir_data.db.buwiz-pending-sign-in.json", room / "bir_data.db"):
                path.write_text("x")
            log = E2E.RunLog(None, echo=False)
            self.assertTrue(E2E.remove_mock_scratch(root, log))
            self.assertFalse(root.exists())
            self.assertTrue(any("mock device store and scratch database removed" in line for line in log.lines),
                            log.lines)

    def test_the_mock_sign_in_pin_shadows_stale_defaults_and_cleanup_removes_only_the_pin(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            db = folder / "e2e.db"
            defaults = (folder / "buwiz-cloud-account.json", folder / "buwiz-pending-sign-in.json")
            for path in defaults:
                path.write_text("stale shared sign-in")
            env = {}
            pinned = E2E.pin_account_state(env, db)
            self.assertNotEqual(pinned, defaults)
            self.assertEqual(env["BUWIZ_CLOUD_ACCOUNT_FILE"], str(pinned[0]))
            self.assertEqual(env["BUWIZ_PENDING_SIGN_IN_FILE"], str(pinned[1]))
            for path in pinned:
                path.write_text("this run's sign-in")
            E2E.remove_files(pinned, E2E.RunLog(None, echo=False))
            self.assertEqual([path.read_text() for path in defaults], ["stale shared sign-in"] * 2)
            self.assertFalse(any(path.exists() for path in pinned))

    def test_on_windows_a_scratch_database_still_held_is_retried_elsewhere_reported_at_once(self):
        real_unlink = Path.unlink
        for system, removed, tries in (("nt", True, 3), ("posix", False, 1)):
            with tempfile.TemporaryDirectory() as folder:
                db = Path(folder) / "e2e.db"
                db.write_text("x")
                calls = []

                def unlink(path, *args, **kwargs):
                    if path.name == "e2e.db":
                        calls.append(path)
                        if len(calls) < 3:
                            raise PermissionError("used by another process")
                    return real_unlink(path, *args, **kwargs)

                log = E2E.RunLog(None, echo=False)
                with mock.patch.object(Path, "unlink", unlink), mock.patch.object(E2E.time, "sleep"), \
                        mock.patch.object(E2E, "os", mock.Mock(name=system)) as fake_os:
                    fake_os.name = system
                    E2E.remove_scratch(db, log)
                self.assertEqual((db.exists(), len(calls)), (not removed, tries), system)
                self.assertEqual(any("scratch database removed" in line for line in log.lines), removed, system)
                self.assertEqual(any("not removed" in line for line in log.lines), not removed, system)

    def test_app_database_reads_app_info(self):
        class Data:
            def json(self, *args, timeout=0):
                return 0, {"ok": True, "result": {"app": "bir-shell", "database": "test"}}, ""

        class Silent:
            def json(self, *args, timeout=0):
                return 1, None, "approval required: the app is locked"

        log = E2E.RunLog(None, echo=False)
        self.assertEqual(E2E.app_database(Data(), log), "test")
        self.assertIsNone(E2E.app_database(Silent(), log))


class SuiteTests(unittest.TestCase):
    def test_the_smoke_suite_can_run(self):
        suite = E2E.load_suite("smoke")
        self.assertEqual(E2E.check_suite(suite), [])
        names = [entry["name"] for entry in suite["recipes"]]
        for flow in ("sign-up", "tax-year", "navigate", "birth-date", "money-boxes", "delete-row", "queue", "sign-out"):
            self.assertIn(flow, names)
        queue = next(entry for entry in suite["recipes"] if entry["name"] == "queue")
        self.assertTrue(queue["send"])

    def test_problems_are_named(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "send.wants", "click fp-queue\n")
            write(folder, "secret.wants", "type login-password hunter2\n")
            write(folder, "param.wants", "click $missing\n")
            write(folder, "paced.json", json.dumps({"name": "p", "steps": [
                {"id": "a", "op": "type", "target": "x", "text": "1", "key_delay_ms": 80}]}))
            write(folder, "clicky.json", json.dumps({"name": "c", "steps": [
                {"id": "a", "op": "click", "target": "x", "key_delay_ms": 80}]}))
            suite = suite_of(folder, [
                {"name": "a", "file": "send.wants"},
                {"name": "b", "file": "secret.wants"},
                {"name": "c", "file": "param.wants"},
                {"name": "d", "file": "paced.json"},
                {"name": "e", "file": "clicky.json", "needs_cli": "0.4.1", "fallback": "send.wants", "send": True},
                {"name": "a", "file": "nowhere.wants"},
            ])
            problems = "\n".join(E2E.check_suite(suite))
        for expected in ("a send step", "password", "$missing", "mark it \"needs_cli\"", "only type is paced",
                         "unique name", "does not exist"):
            self.assertIn(expected, problems)

    def test_a_parameter_used_at_or_before_its_after_step_is_refused_by_check(self):
        cases = [
            ("click $table-add\nclick expose-table\nclick $table-delete\n", "s1"),
            ("click expose-table\nclick $table-add\n", "s2"),
        ]
        for text, used_at in cases:
            with self.subTest(used_at=used_at), tempfile.TemporaryDirectory() as folder:
                write(folder, "late.wants", text)
                suite = suite_of(folder, [{"name": "late", "file": "late.wants", "resolve": {
                    "table": {"id_regex": "^(fp-table-.+)-add$", "group": 1, "after": "s2"}}}])
                problems = "\n".join(E2E.check_suite(suite))
            self.assertIn(f"uses $table at {used_at}, at or before its resolve's after step s2", problems)

    def test_a_json_after_that_cuts_a_needs_edge_is_refused_by_check(self):
        recipe = {"v": 1, "name": "cut", "steps": [
            {"id": "expose", "op": "click", "target": "expose-table"},
            {"id": "prepare", "op": "click", "target": "prepare-table"},
            {"id": "use", "op": "click", "target": "$table-add", "needs": ["prepare"]},
        ]}
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "cut.json", json.dumps(recipe))
            suite = suite_of(folder, [{"name": "cut", "file": "cut.json", "resolve": {
                "table": {"id_regex": "^(fp-table-.+)-add$", "group": 1, "after": "prepare"}}}])
            problems = "\n".join(E2E.check_suite(suite))
            self.assertIn("$table resolves after 'prepare', cutting the needs edge prepare -> use", problems)

            suite = suite_of(folder, [{"name": "cut", "file": "cut.json", "resolve": {
                "table": {"id_regex": "^(fp-table-.+)-add$", "group": 1, "after": "expose"}}}])
            self.assertEqual(E2E.check_suite(suite), [])

    def test_variants_and_fallbacks_follow_the_host(self):
        entry = {"name": "m", "desktop": "d.json", "phone": "p.json", "windows": "w.json", "linux": "narrow.json",
                 "needs_cli": "0.4.1", "needs_feature": {"phone": "software_keyboard"}, "fallback": "plain.wants"}
        base = {"features": [], "allow_send": False, "env": {}}
        cases = [
            ({"target": "macos", "form_factor": "desktop", "cli_version": "0.4.1"}, "d.json", None),
            ({"target": "windows", "form_factor": "desktop", "cli_version": "0.4.1"}, "w.json", None),
            # The Linux window opens 420 px wide: a desktop host showing the narrow layout takes its own variant.
            ({"target": "linux", "form_factor": "desktop", "cli_version": "0.4.1"}, "narrow.json", None),
            ({"target": "macos", "form_factor": "desktop", "cli_version": "0.4.0"}, "plain.wants", "fallback"),
            ({"target": "ios", "form_factor": "phone", "cli_version": "0.4.1"}, "plain.wants", "fallback"),
            ({"target": "ios", "form_factor": "phone", "cli_version": "0.4.1", "features": ["software_keyboard"]},
             "p.json", None),
        ]
        for context, file, note in cases:
            got_file, got_note, status = E2E.plan_entry(entry, {}, {**base, **context})
            self.assertEqual(got_file, file, context)
            self.assertIsNone(status)
            self.assertEqual(bool(got_note and got_note.startswith(note or "")), bool(note), (context, got_note))
        bare = {key: value for key, value in entry.items() if key != "fallback"}
        file, note, status = E2E.plan_entry(bare, {}, {**base, "target": "linux", "form_factor": "desktop",
                                                       "cli_version": "0.4.0"})
        self.assertEqual((file, status), (None, "needs"))
        self.assertIn("needs gpui-agent-cli 0.4.1", note)
        self.assertEqual(E2E.plan_entry({"name": "x", "desktop": "d"}, {}, {**base, "target": "ios",
                                                                         "form_factor": "phone"})[2], "skipped")
        # Any target may have its own file (Android's sign-up has no picker sheets), and a fallback may differ by form factor.
        own = {"name": "s", "desktop": "d.wants", "phone": "p.json", "android": "a.wants"}
        self.assertEqual(E2E.plan_entry(own, {}, {**base, "target": "android", "form_factor": "phone"})[0], "a.wants")
        self.assertEqual(E2E.plan_entry(own, {}, {**base, "target": "ios", "form_factor": "phone"})[0], "p.json")
        split = {**entry, "fallback": {"phone": "phone.json", "file": "plain.wants"}}
        for target, factor, file in (("android", "phone", "phone.json"), ("macos", "desktop", "plain.wants")):
            self.assertEqual(E2E.plan_entry(split, {}, {**base, "target": target, "form_factor": factor,
                                                        "cli_version": "0.4.0"})[0], file, target)
        self.assertEqual(sorted(E2E.entry_files(own)), ["a.wants", "d.wants", "p.json"])


class SnapshotTests(unittest.TestCase):
    def test_every_node_of_a_real_snapshot_is_walked(self):
        nodes = list(E2E.tree_nodes(snapshot("snapshot-macos-1601c.json")))
        self.assertEqual(len(nodes), 109)
        self.assertEqual(nodes[0]["id"], "tb-sidebar")

    def test_resolve_picks_by_role_name_and_id_pattern(self):
        tree = snapshot("snapshot-macos-1601c.json")
        self.assertEqual(E2E.resolve({"role": ["textbox", "textfield"], "name": "Total amount of compensation"}, tree),
                         "fp-frm1601c:txtTax14")
        self.assertEqual(E2E.resolve({"id_regex": r"^fp-(queue)$", "group": 1}, tree), "queue")
        self.assertIsNone(E2E.resolve({"role": "textbox", "name": "Total amount of compensation"}, tree))
        phone = snapshot("snapshot-phone-2551q.json")
        self.assertEqual(E2E.resolve({"id_regex": "^(fp-table-.+)-add$", "group": 1}, phone),
                         "fp-table-frm2551Qv2018:txtSched1Atc1")
        self.assertEqual(E2E.resolve({"id": "system-keyboard", "role": "keyboard"}, phone), "system-keyboard")


class ReportTests(unittest.TestCase):
    def test_the_failing_step_and_its_wants_line(self):
        receipt = {"ok": False, "steps": [{"id": "s1", "ok": True}, {"id": "s2", "ok": False, "error": "occluded_by: x",
                                                                    "error_code": "occluded"}]}
        self.assertEqual(E2E.failing_step(receipt), ("s2", "occluded_by: x", "occluded"))
        self.assertEqual(E2E.failing_step({"ok": False, "error": "connect_failed", "error_code": "connect_failed"}),
                         (None, "connect_failed", "connect_failed"))
        with tempfile.TemporaryDirectory() as folder:
            path = write(folder, "r.wants", "# head\nclick a\n\nclick b --screenshot\n")
            self.assertEqual(E2E.wants_line_of(path, "s2"), "click b --screenshot")
            self.assertIsNone(E2E.wants_line_of(path, "s9"))

    def test_the_run_log_never_holds_the_token(self):
        token = "f00dfeedcafe1234"
        with tempfile.TemporaryDirectory() as folder:
            log = E2E.RunLog(Path(folder) / "run.log", secrets_=[token], echo=False)
            line = log.line("launch", f"GPUI_AGENT_TOKEN={token}", f"error mentioning {token}", 12.7)
            self.assertNotIn(token, line)
            self.assertNotIn(token, (Path(folder) / "run.log").read_text(encoding="utf-8"))
            self.assertRegex(line, r"\tlaunch\t.*\t12ms$")

    def test_report_lists_each_recipe(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder)
            (out / "01-a").mkdir()
            (out / "01-a" / "failure.png").write_bytes(b"png")
            E2E.write_report(out, {"suite": "smoke", "target": "ios", "features": ["press"]}, [
                {"name": "a", "status": "known gap", "ms": 5, "step": "s3", "line": "click x", "error": "e | f",
                 "known_gap": "gap 1", "screenshot": str(out / "01-a" / "failure.png")},
                {"name": "b", "status": "needs", "ms": 0, "note": "needs gpui-agent-cli 0.4.1"}])
            text = (out / "report.md").read_text(encoding="utf-8")
            self.assertIn("| 1 | a | known gap | 5 | s3 `click x` | e / f (known gap: gap 1) [screenshot](01-a/failure.png) |",
                          text)
            self.assertIn("| 2 | b | needs | 0 |  | needs gpui-agent-cli 0.4.1 |", text)
            self.assertEqual(json.loads((out / "report.json").read_text())["recipes"][1]["status"], "needs")


LATE_FAKE_CLI = r'''
import json, sys
from pathlib import Path
args = sys.argv[1:]
log = open(sys.argv[0] + ".calls", "a")
log.write(json.dumps(args) + "\n")
rest = args[2:]
if rest and rest[0] == "--reply-timeout-ms":
    rest = rest[2:]
op = rest[0]
if op == "snapshot":
    print(open(sys.argv[0] + ".tree").read())
elif op == "screenshot":
    open(args[args.index("--out") + 1], "wb").write(b"\x89PNG")
    print("{}")
elif op == "recipe":
    path = Path(rest[rest.index("run") + 1])
    text = path.read_text()
    if "expose-table" in text:
        Path(sys.argv[0] + ".tree").write_text(Path(sys.argv[0] + ".after").read_text())
    if path.suffix == ".json":
        steps = json.loads(text)["steps"]
        ids = [step.get("id") for step in steps]
        bodies = [json.dumps(step) for step in steps]
    else:
        lines = [line for line in text.splitlines() if line.strip() and not line.lstrip().startswith("#")]
        ids = ["s" + str(n) for n in range(1, len(lines) + 1)]
        bodies = lines
    done = []
    failure = None
    for index, (step_id, body) in enumerate(zip(ids, bodies), 1):
        if "die-before-receipt" in body:
            print(f"error: {path}: line {index}: died before receipt", file=sys.stderr)
            sys.exit(1)
        failed = "fail-here" in body
        done.append({"id": step_id, "ok": not failed, "elapsed_ms": 1})
        if failed:
            done[-1]["error"] = "no such node"
            done[-1]["error_code"] = "no_node"
            failure = done[-1]
            break
    receipt = {"v": 1, "ok": failure is None, "recipe": path.name, "steps": done,
               "fingerprint": f"fp-{path.stem}-{len(ids)}", "session_reused": True,
               "elapsed_ms": 5 + len(ids)}
    print(json.dumps(receipt))
    sys.exit(1 if failure else 0)
'''


def install_late_cli(folder, before, after):
    fake = Path(folder) / "gpui-agent-late"
    fake.write_text(f"#!{sys.executable}\n" + LATE_FAKE_CLI, encoding="utf-8")
    fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
    Path(str(fake) + ".tree").write_text(json.dumps(before), encoding="utf-8")
    Path(str(fake) + ".after").write_text(json.dumps(after), encoding="utf-8")
    return fake


def cli_calls(fake):
    return [json.loads(line) for line in Path(str(fake) + ".calls").read_text().splitlines()]


def recipe_call_path(call):
    return Path(call[call.index("run") + 1])


def tree_with_table():
    return {"tree": {"nodes": [
        {"id": "root", "role": "window"},
        {"id": "fp-table-x-add", "role": "button", "name": "Add row", "visible": True},
    ]}}


EMPTY_TREE = {"tree": {"nodes": [{"id": "root", "role": "window"}]}}


FAKE_CLI = r'''
import json, sys
args = sys.argv[1:]
log = open(sys.argv[0] + ".calls", "a")
log.write(json.dumps(args) + "\n")
rest = args[2:]
if rest and rest[0] == "--reply-timeout-ms":
    rest = rest[2:]
op = rest[0]
if op == "snapshot":
    print(open(sys.argv[0] + ".tree").read())
elif op == "screenshot":
    open(args[args.index("--out") + 1], "wb").write(b"\x89PNG")
    print("{}")
elif op == "recipe":
    receipt = {"v": 1, "ok": False, "recipe": "r", "steps": [
        {"id": "s1", "ok": True, "elapsed_ms": 3},
        {"id": "s2", "ok": False, "elapsed_ms": 4, "error": "occluded_by: software keyboard (a press on x)",
         "error_code": "host_error"}]}
    print(json.dumps(receipt))
    sys.exit(1)
'''


@unittest.skipUnless(os.name == "posix", "the stand-in CLI is a POSIX script")
class RunEntryTests(unittest.TestCase):
    def test_a_failed_recipe_reports_its_step_screenshot_and_tree(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = folder / "gpui-agent"
            fake.write_text(f"#!{sys.executable}\n" + FAKE_CLI, encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
            Path(str(fake) + ".tree").write_text((FIXTURES / "snapshot-phone-2551q.json").read_text())
            write(folder, "delete.wants", "click $table-add\nclick $table-delete\n")
            suite = suite_of(folder, [{"name": "delete-row", "file": "delete.wants",
                                       "resolve": {"table": {"id_regex": "^(fp-table-.+)-add$", "group": 1}},
                                       "known_gap": {"phone": "gap 1"}}])
            out = folder / "out"
            log = E2E.RunLog(out / "run.log", secrets_=["tok"], echo=False)
            out.mkdir()
            agent = E2E.Agent(str(fake), "127.0.0.1:1", dict(os.environ), log)
            context = {"target": "android", "form_factor": "phone", "features": ["software_keyboard"],
                       "cli_version": "0.4.1", "allow_send": False, "env": {}}
            result = E2E.run_entry(1, suite["recipes"][0], suite, context, agent, log, out)
            self.assertEqual(result["status"], "known gap")
            self.assertEqual(result["step"], "s2")
            self.assertEqual(result["line"], "click $table-delete")
            self.assertIn("software keyboard", result["error"])
            self.assertTrue(Path(result["screenshot"]).exists())
            self.assertTrue(Path(result["tree"]).exists())
            calls = [json.loads(line) for line in Path(str(fake) + ".calls").read_text().splitlines()]
            run = next(call for call in calls if "recipe" in call)
            self.assertIn("table=fp-table-frm2551Qv2018:txtSched1Atc1", run)
            self.assertIn("--screenshot-flagged", run)
            self.assertIn("resolve\t$table\tfp-table-frm2551Qv2018:txtSched1Atc1", "\n".join(log.lines))

    def run_late_entry(self, folder, fake, entry, recipe_text):
        write(folder, entry["file"], recipe_text)
        suite = suite_of(folder, [entry])
        out = folder / "out"
        out.mkdir()
        log = E2E.RunLog(out / "run.log", secrets_=["tok"], echo=False)
        agent = E2E.Agent(str(fake), "127.0.0.1:1", dict(os.environ), log)
        context = {"target": "macos", "form_factor": "desktop", "features": [], "cli_version": "0.4.1",
                   "allow_send": False, "env": {}}
        return E2E.run_entry(1, entry, suite, context, agent, log, out), suite, out, log

    def late_entry(self, after="s1", name="late", file="late.wants"):
        spec = {"id_regex": "^(fp-table-.+)-add$", "group": 1}
        if after:
            spec["after"] = after
        return {"name": name, "file": file, "resolve": {"table": spec}}

    def test_late_resolve_finds_the_node_only_after_the_step(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, tree_with_table())
            entry = self.late_entry()
            result, suite, out, log = self.run_late_entry(folder, fake, entry,
                                                          "click expose-table\nclick $table-add\n")
            self.assertEqual(E2E.check_suite(suite), [])
            self.assertEqual(result["status"], "pass")
            calls = cli_calls(fake)
            self.assertEqual([call[2] for call in calls], ["recipe", "snapshot", "recipe"])
            recipes = [call for call in calls if call[2] == "recipe"]
            first, second = recipe_call_path(recipes[0]), recipe_call_path(recipes[1])
            self.assertTrue(first.is_relative_to(out))
            self.assertTrue(second.is_relative_to(out))
            self.assertEqual(first.read_text(), "click expose-table\n")
            self.assertEqual(second.read_text(), "click $table-add\n")
            self.assertIn("table=fp-table-x", recipes[1])

    def test_resolve_without_after_still_resolves_before_one_unsplit_run(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, tree_with_table(), tree_with_table())
            entry = self.late_entry(after=None)
            result, suite, out, log = self.run_late_entry(folder, fake, entry, "click $table-add\n")
            self.assertEqual(result["status"], "pass")
            calls = cli_calls(fake)
            self.assertEqual([call[2] for call in calls], ["snapshot", "recipe"])
            recipes = [call for call in calls if call[2] == "recipe"]
            self.assertEqual(str(recipe_call_path(recipes[0])), str((folder / "late.wants").resolve()))
            self.assertIn("table=fp-table-x", recipes[0])

    def test_nothing_found_after_the_step_is_setup_with_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, EMPTY_TREE)
            entry = self.late_entry()
            result, suite, out, log = self.run_late_entry(folder, fake, entry,
                                                          "click expose-table\nclick $table-add\n")
            self.assertEqual(result["status"], "setup")
            self.assertEqual(result["step"], "s1")
            self.assertIn("$table", result["error"])
            self.assertIn("after s1", result["error"])
            self.assertIn("no node matches", result["error"])
            self.assertTrue(Path(result["screenshot"]).exists())
            self.assertTrue(Path(result["tree"]).exists())
            calls = cli_calls(fake)
            self.assertEqual([call[2] for call in calls].count("recipe"), 1)

    def test_a_json_recipe_splits_at_the_named_step(self):
        recipe = {"v": 1, "name": "late-json", "app": "bir-shell", "params": ["table"], "steps": [
            {"id": "expand", "op": "click", "target": "expose-table"},
            {"id": "add", "op": "click", "target": "$table-add"},
        ]}
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, tree_with_table())
            entry = self.late_entry(after="expand", name="json-late", file="late.json")
            result, suite, out, log = self.run_late_entry(folder, fake, entry, json.dumps(recipe))
            self.assertEqual(result["status"], "pass")
            calls = cli_calls(fake)
            self.assertEqual([call[2] for call in calls], ["recipe", "snapshot", "recipe"])
            recipes = [call for call in calls if call[2] == "recipe"]
            first, second = recipe_call_path(recipes[0]), recipe_call_path(recipes[1])
            self.assertEqual([step["id"] for step in json.loads(first.read_text())["steps"]], ["expand"])
            self.assertEqual([step["id"] for step in json.loads(second.read_text())["steps"]], ["add"])
            self.assertIn("table=fp-table-x", recipes[1])
            receipt = json.loads((out / "01-json-late" / "receipt.json").read_text())
            self.assertEqual([step["id"] for step in receipt["steps"]], ["expand", "add"])

    def test_a_split_wants_receipt_and_report_keep_the_original_step_ids(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, tree_with_table())
            entry = self.late_entry(name="report-ids")
            result, suite, out, log = self.run_late_entry(
                folder, fake, entry, "click expose-table\nclick $table-add\nclick fail-here\n")
            self.assertEqual(result["status"], "fail")
            self.assertEqual(result["step"], "s3")
            self.assertEqual(result["line"], "click fail-here")
            receipt = json.loads((out / "01-report-ids" / "receipt.json").read_text())
            self.assertEqual([step["id"] for step in receipt["steps"]], ["s1", "s2", "s3"])

    def test_a_merged_split_receipt_lists_fingerprints_sums_elapsed_and_marks_split(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, tree_with_table())
            entry = self.late_entry(name="merged-receipt")
            result, suite, out, log = self.run_late_entry(
                folder, fake, entry, "click expose-table\nclick $table-add\nclick done\n")
            self.assertEqual(result["status"], "pass")
            receipt = json.loads((out / "01-merged-receipt" / "receipt.json").read_text())
            self.assertEqual(receipt["fingerprints"], ["fp-slice-1-1", "fp-slice-2-2"])
            self.assertEqual(receipt["fingerprint"], "fp-slice-2-2")
            self.assertEqual(receipt["elapsed_ms"], 13)
            self.assertIs(receipt["split"], True)
            self.assertIn("session_reused", receipt)

    def test_a_slice_that_dies_before_its_receipt_reports_the_original_step_and_file(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            fake = install_late_cli(folder, EMPTY_TREE, tree_with_table())
            entry = self.late_entry(name="lost-receipt")
            result, suite, out, log = self.run_late_entry(
                folder, fake, entry, "click expose-table\nclick $table-add\nclick die-before-receipt\n")
            self.assertEqual(result["status"], "fail")
            self.assertEqual(result["step"], "s3")
            self.assertEqual(result["line"], "click die-before-receipt")
            self.assertIn("late.wants", result["error"])
            self.assertIn("step s3", result["error"])
            self.assertNotIn("slice-2.wants", result["error"])
            self.assertNotIn("line 2", result["error"])


if __name__ == "__main__":
    unittest.main()
