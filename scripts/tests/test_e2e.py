"""scripts/e2e: the end-to-end runner's pure parts, and one recipe run against a stand-in gpui-agent (no app, no device).

Fixtures: fixtures/e2e/snapshot-macos-1601c.json is a real `gpui-agent snapshot` of the app's macOS window (a 1601C
return, from try/money-box-agent-input at ed434e02, docs/proof/money-box-agent-input/calls/ui-expanded.json).
fixtures/e2e/snapshot-phone-2551q.json is written by hand in the strict adapter's shape (no phone snapshot was
recorded on a proof branch): it has the `system-keyboard` node a host with feature software_keyboard reports.
"""
import importlib.util
import json
import os
import stat
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("e2e_runner", ROOT / "scripts" / "e2e" / "runner.py")
E2E = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(E2E)
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


class VersionTests(unittest.TestCase):
    def test_versions_compare_as_numbers(self):
        self.assertTrue(E2E.version_at_least("0.4.10", "0.4.9"))
        self.assertTrue(E2E.version_at_least("0.4.1", "0.4.1"))
        self.assertFalse(E2E.version_at_least("0.4.0", "0.4.1"))
        self.assertFalse(E2E.version_at_least(None, "0.4.1"))
        self.assertFalse(E2E.version_at_least("main", "0.4.1"))

    def test_installed_version_comes_from_the_override_then_cargo_then_the_lock(self):
        self.assertEqual(E2E.installed_cli_version({"E2E_GPUI_AGENT_CLI_VERSION": "v0.4.1"}), "0.4.1")

        class Listed:
            stdout = "gpui-agent-cli v0.4.0:\n    gpui-agent\nripgrep v14.1.0:\n    rg\n"

        self.assertEqual(E2E.installed_cli_version({}, run=lambda *a, **k: Listed()), "0.4.0")

        def no_cargo(*a, **k):
            raise OSError("no cargo")

        self.assertEqual(E2E.installed_cli_version({}, run=no_cargo), E2E.pinned_gpui_agent_version())
        self.assertRegex(E2E.pinned_gpui_agent_version() or "", r"^\d+\.\d+\.\d+$")


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


class FileTests(unittest.TestCase):
    def test_files_other_programs_wrote_are_masked_and_the_scratch_database_removed(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            log = E2E.RunLog(None, secrets_=["s3cret"], echo=False)
            receipt = write(folder, "receipt.json", '{"error": "token s3cret refused"}')
            E2E.redact_file(receipt, log)
            self.assertNotIn("s3cret", receipt.read_text())
            db = folder / "e2e.db"
            for path in (db, Path(f"{db}-wal"), Path(f"{db}-shm")):
                path.write_text("x")
            E2E.remove_scratch(db, log)
            self.assertEqual(sorted(p.name for p in folder.iterdir()), ["receipt.json"])

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

    def test_a_step_that_scrolled_without_may_scroll_fails(self):
        # The plug scrolls an occluded press's target clear and retries (#28): a step written to fail when a layout
        # moved its target under something could pass that way, so it fails unless the suite allows it.
        phone = {"target": "android", "form_factor": "phone"}
        receipt = {"ok": True, "steps": [
            {"id": "s13", "ok": True, "result": {"changed": True}},
            {"id": "s14", "ok": True, "result": {"scrolled": 1}},
        ]}
        scrolled, failure = E2E.scroll_check(receipt, {"name": "money-boxes"}, phone)
        self.assertEqual(scrolled, {"s14": 1})
        self.assertEqual(failure, ("s14", "this step scrolled 1 time(s) to reach its target; add 's14' to this suite "
                                          "entry's \"may_scroll\" if that is intended"))

    def test_a_step_allowed_to_scroll_passes_and_its_count_is_recorded(self):
        phone = {"target": "android", "form_factor": "phone"}
        receipt = {"ok": True, "steps": [{"id": "s14", "ok": True, "result": {"scrolled": 2}}]}
        for may_scroll in (["s14"], {"phone": ["s14"]}, {"android": ["s14"]}):
            with self.subTest(may_scroll=may_scroll):
                scrolled, failure = E2E.scroll_check(receipt, {"may_scroll": may_scroll}, phone)
                self.assertEqual((scrolled, failure), ({"s14": 2}, None))
        # Allowed on a phone is not allowed on a desktop.
        desktop = {"target": "macos", "form_factor": "desktop"}
        self.assertIsNotNone(E2E.scroll_check(receipt, {"may_scroll": {"phone": ["s14"]}}, desktop)[1])

    def test_a_step_refused_after_scrolling_records_its_scrolls(self):
        # The plug answers only an error then ("occluded_by: 9 after 2 scroll(s) towards the middle"): the count is
        # read from it, so a failed step's scrolls show in the report too.
        context = {"target": "android", "form_factor": "phone"}
        receipt = {"ok": False, "steps": [
            {"id": "s5", "ok": False, "error": "occluded_by: cover after 2 scroll(s) towards the middle"},
        ]}
        self.assertEqual(E2E.scroll_check(receipt, {"may_scroll": ["s5"]}, context), ({"s5": 2}, None))
        self.assertEqual(E2E.scroll_check(receipt, {}, context)[0], {"s5": 2})
        refused = {"ok": False, "steps": [{"id": "s5", "ok": False, "error": "occluded_by: cover"}]}
        self.assertEqual(E2E.scroll_check(refused, {}, context), ({}, None))

    def test_a_step_that_did_not_scroll_is_untouched(self):
        context = {"target": "linux", "form_factor": "desktop"}
        for result in (None, {}, {"changed": True}, {"scrolled": 0}, {"scrolled": True}, "text"):
            with self.subTest(result=result):
                receipt = {"ok": True, "steps": [{"id": "s1", "ok": True, "result": result}]}
                self.assertEqual(E2E.scroll_check(receipt, {}, context), ({}, None))
        self.assertEqual(E2E.scroll_check(None, {}, context), ({}, None))

    def test_may_scroll_names_steps_its_recipes_have(self):
        with tempfile.TemporaryDirectory() as folder:
            write(folder, "press.json", json.dumps({"name": "p", "steps": [
                {"id": "s14", "op": "click", "target": "x"}]}))
            good = suite_of(folder, [{"name": "p", "phone": "press.json", "may_scroll": {"phone": ["s14"]}}])
            self.assertEqual(E2E.check_suite(good), [])
            for may_scroll, expected in (({"phone": ["s99"]}, "may_scroll names 's99'"),
                                         ({"phone": "s14"}, "may_scroll is a list of step ids"),
                                         (True, "may_scroll is a list of step ids")):
                suite = suite_of(folder, [{"name": "p", "phone": "press.json", "may_scroll": may_scroll}])
                self.assertIn(expected, "\n".join(E2E.check_suite(suite)))

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
