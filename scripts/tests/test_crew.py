"""crew/bin/crew, the portable launch crew runner, against fixture repositories."""

import configparser
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CREW = ROOT / "crew" / "bin" / "crew"


def run_crew(repo, *args, env=None):
    merged = os.environ.copy()
    if env:
        merged.update(env)
    return subprocess.run([sys.executable, str(repo / "crew" / "bin" / "crew"), *args],
                          cwd=str(repo), capture_output=True, text=True, env=merged)


def write_fixture_repo(temp, roles=None, rules="# Rules\n"):
    repo = Path(temp) / "repo"
    (repo / "crew" / "bin").mkdir(parents=True)
    (repo / "crew" / "roles").mkdir(parents=True)
    shutil.copyfile(CREW, repo / "crew" / "bin" / "crew")
    if rules is not None:
        (repo / "crew" / "RULES.md").write_text(rules, encoding="utf-8")
    (repo / "crew" / "REPOS.md").write_text(
        "| Repository | What it is |\n| --- | --- |\n| hexuria/buwiz-forms | The app. |\n",
        encoding="utf-8")
    skill = repo / ".claude" / "skills" / "crew" / "SKILL.md"
    skill.parent.mkdir(parents=True)
    skill.write_text("---\nname: crew\ndisable-model-invocation: true\ntriggers: [user]\n---\n",
                     encoding="utf-8")
    for name, text in (roles or {}).items():
        (repo / "crew" / "roles" / (name + ".md")).write_text(text, encoding="utf-8")
    return repo


def role_file(name="tools", title="Tools worker", engine="devin", model="swe-2-1-max",
              needs="", repos="", extra="", body="Role body line.\n"):
    lines = ["---", "name: " + name, "title: " + title, "engine: " + engine, "model: " + model,
             "needs: " + needs, "repos: " + repos]
    if extra:
        lines.extend(extra.splitlines())
    lines.extend(["---", "", body])
    return "\n".join(lines)


def shell_join(parts):
    if os.name == "nt":
        return subprocess.list2cmdline([str(part) for part in parts])
    return " ".join(shlex.quote(str(part)) for part in parts)


def write_fake_engine(folder):
    path = Path(folder) / "fake_engine.py"
    path.write_text(r'''
import json
import os
import sys
import time

argv = sys.argv[1:]
record = {"argv": argv, "cwd": os.getcwd(), "git_lfs_skip_push": os.environ.get("GIT_LFS_SKIP_PUSH"),
          "env": dict(os.environ)}

def after(flag):
    return argv[argv.index(flag) + 1] if flag in argv and argv.index(flag) + 1 < len(argv) else None

record["model"] = after("--model")
record["prompt_file"] = after("--prompt-file")
record["config_path"] = after("--config")
if record["prompt_file"]:
    with open(record["prompt_file"], encoding="utf-8") as handle:
        record["prompt"] = handle.read()
else:
    record["stdin"] = sys.stdin.read()
if record["config_path"]:
    with open(record["config_path"], encoding="utf-8") as handle:
        record["config"] = json.load(handle)
record_path = os.environ.get("FAKE_ENGINE_RECORD")
if record_path:
    with open(record_path, "w", encoding="utf-8") as handle:
        json.dump(record, handle)
if os.environ.get("FAKE_ENGINE_SLEEP"):
    time.sleep(float(os.environ["FAKE_ENGINE_SLEEP"]))
print("fake engine ran")
sys.exit(int(os.environ.get("FAKE_ENGINE_EXIT", "0")))
'''.lstrip(), encoding="utf-8")
    return path


def init_git_repo(repo):
    subprocess.run(["git", "init", "-q"], cwd=str(repo), check=True)


def write_run_fixture(test, engine="devin", model="role-model"):
    repo = write_fixture_repo(test.temp.name, {"tools": role_file(engine=engine, model=model)})
    init_git_repo(repo)
    fake = write_fake_engine(test.temp.name)
    command = shell_join([sys.executable, fake])
    config = Path(test.temp.name) / "devin-config.json"
    config.write_text(json.dumps({"user_setting": {"kept": True}, "permissions": {"allow": ["old"], "deny": ["old"]}}),
                      encoding="utf-8")
    (repo / "crew.local.ini").write_text(
        "[engine.devin]\ncommand = {0}\nconfig = {1}\n\n[engine.claude]\ncommand = {0}\n\n"
        "[worker]\npass_env = FAKE_ENGINE_RECORD,FAKE_ENGINE_EXIT\n".format(command, config),
        encoding="utf-8")
    task = repo / "001-sample.md"
    task.write_text("TASK FOR WORKER\n", encoding="utf-8")
    record = Path(test.temp.name) / "engine-record.json"
    return repo, task, record, config


def write_path_python(folder, name, source):
    """A PATH-visible command that starts its Python implementation through sys.executable."""
    folder = Path(folder)
    folder.mkdir(parents=True, exist_ok=True)
    program = folder / (name + ".py")
    program.write_text(source, encoding="utf-8")
    if os.name == "nt":
        shim = folder / (name + ".cmd")
        shim.write_text('@echo off\r\n"{0}" "{1}" %*\r\n'.format(sys.executable, program), encoding="utf-8")
    else:
        shim = folder / name
        shim.write_text('#!/bin/sh\nexec "{0}" "{1}" "$@"\n'.format(sys.executable, program), encoding="utf-8")
        os.chmod(str(shim), 0o755)
    return shim


def write_versioned_cli(folder, name, version):
    return write_path_python(folder, name, "import sys\nprint('{0} version {1}')\n".format(name, version))


def write_fleet_printer(folder, lines):
    path = Path(folder) / "fake_fleet.py"
    source = "import sys\nsys.stdout.write({0!r})\n".format("".join(line + "\n" for line in lines))
    path.write_text(source, encoding="utf-8")
    return path


def write_fake_gh(folder):
    data_path = Path(folder) / "gh-data.json"
    source = r'''
import json
import os
import sys

with open(os.environ["FAKE_GH_DATA"], encoding="utf-8") as handle:
    data = json.load(handle)
argv = sys.argv[1:]
if argv[:2] == ["pr", "view"]:
    print(json.dumps(data.get("view", {}).get(argv[2], {})))
elif argv[:2] == ["pr", "checks"]:
    rows = data.get("checks", {}).get(argv[2], [])
    print(json.dumps(rows))
    if any(row.get("bucket") == "fail" for row in rows):
        sys.exit(1)
    if any(row.get("bucket") == "pending" for row in rows):
        sys.exit(8)
'''
    write_path_python(folder, "gh", source)
    return data_path


class SettingsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def state_path(self, env=None):
        result = run_crew(self.repo, "state", "path", env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        return Path(result.stdout.strip())

    def test_default_state_dir_is_under_the_repo(self):
        self.assertEqual(self.state_path(), (self.repo / ".crew" / "state").resolve())

    def test_relative_state_dir_in_the_file_is_under_the_repo(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = elsewhere/state\n", encoding="utf-8")

        self.assertEqual(self.state_path(), (self.repo / "elsewhere" / "state").resolve())

    def test_environment_state_dir_overrides_the_file(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = elsewhere/state\n", encoding="utf-8")
        override = Path(self.temp.name) / "env-state"

        self.assertEqual(self.state_path({"CREW_STATE_DIR": str(override)}), override)

    def test_tilde_state_dir_expands_to_the_home(self):
        home = Path(self.temp.name) / "home"
        home.mkdir()
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = ~/crew-state\n", encoding="utf-8")
        env = {"HOME": str(home), "USERPROFILE": str(home)}

        self.assertEqual(self.state_path(env), home / "crew-state")

    def test_crew_engine_environment_overrides_the_file(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nengine = devin\n", encoding="utf-8")

        result = run_crew(self.repo, "start", "--role", "tools", "--print", env={"CREW_ENGINE": "claude", "PATH": ""})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("claude "), result.stdout)

    def test_engine_in_the_file_is_used_when_no_environment_override(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nengine = devin\n", encoding="utf-8")

        result = run_crew(self.repo, "start", "--role", "tools", "--print", env={"PATH": ""})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("devin --prompt-file "), result.stdout)


class PromptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_prompt_order_is_rules_role_state_then_task(self):
        repo = write_fixture_repo(self.temp.name, {"tools": role_file(body="ROLE BODY\n")}, rules="RULES BODY\n")
        state = repo / ".crew" / "state" / "roles" / "tools"
        state.mkdir(parents=True)
        (state / "STATE.md").write_text("STATE BODY\n", encoding="utf-8")
        task = repo / "task.md"
        task.write_text("TASK BODY\n", encoding="utf-8")

        result = run_crew(repo, "prompt", "tools", str(task))

        self.assertEqual(result.returncode, 0, result.stderr)
        positions = [result.stdout.index(text) for text in ("RULES BODY", "ROLE BODY", "STATE BODY", "TASK BODY")]
        self.assertEqual(positions, sorted(positions))
        header = result.stdout.splitlines()[:4]
        self.assertEqual(len(header), 4)
        self.assertIn("'tools' worker", header[0])
        self.assertIn("Repository root:", header[1])
        self.assertIn("State folder:", header[2])
        self.assertIn("run the crew launcher (`crew/bin/crew`) with Python", header[3])

    def test_missing_role_is_an_error_naming_the_file(self):
        repo = write_fixture_repo(self.temp.name, {})

        result = run_crew(repo, "prompt", "missing")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(repo / "crew" / "roles" / "missing.md"), result.stderr)

    def test_missing_rules_is_an_error_naming_the_file(self):
        repo = write_fixture_repo(self.temp.name, {"tools": role_file()}, rules=None)

        result = run_crew(repo, "prompt", "tools")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(repo / "crew" / "RULES.md"), result.stderr)


class StatusTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def test_status_reports_counts_newest_result_and_state_first_line(self):
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        role = self.repo / ".crew" / "state" / "roles" / "tools"
        (role / "inbox" / "001-a.md").write_text("task\n", encoding="utf-8")
        (role / "inbox" / "002-b.md").write_text("task\n", encoding="utf-8")
        (role / "outbox" / "001-a.md").write_text("result\n", encoding="utf-8")
        newer = role / "outbox" / "002-b.md"
        newer.write_text("result\n", encoding="utf-8")
        now = 2000000000
        os.utime(str(role / "outbox" / "001-a.md"), (now - 20, now - 20))
        os.utime(str(newer), (now, now))
        (role / "STATE.md").write_text("tools state first line\nrest\n", encoding="utf-8")

        result = run_crew(self.repo, "status")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("tools", result.stdout)
        self.assertIn("tasks 2", result.stdout)
        self.assertIn("results 2", result.stdout)
        self.assertIn("002-b.md", result.stdout)
        self.assertIn("tools state first line", result.stdout)


class WatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        self.bin = Path(self.temp.name) / "bin"

    def watch(self, extra_env=None):
        env = {"PATH": str(self.bin)}
        if extra_env:
            env.update(extra_env)
        return run_crew(self.repo, "watch", "--holder", "conductor", "--repo", "owner/repo", "--once", env=env)

    def set_queue(self, waiting):
        (self.repo / ".crew" / "state" / "queue.json").write_text(
            json.dumps({"waiting": waiting, "batch": None}) + "\n", encoding="utf-8")

    def test_held_machine_is_reported_on_every_wake_and_never_remembered(self):
        fleet = write_fleet_printer(self.temp.name, [
            "m1 | linux | waiting_for_user | busy | conductor | 12 | task",
            "m2 | linux | working | busy | conductor | 12 | task",
        ])
        (self.repo / "crew.local.ini").write_text("[fleet]\ncommand = {0}\n".format(
            shell_join([sys.executable, fleet])), encoding="utf-8")

        first = self.watch()
        second = self.watch()

        for result in (first, second):
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("machine m1 | linux | waiting_for_user", result.stdout)
            self.assertNotIn("m2 | linux | working", result.stdout)
        seen = (self.repo / ".crew" / "state" / ".watch-seen").read_text(encoding="utf-8")
        self.assertNotIn("m1", seen)

    def test_waiting_pr_ci_is_reported_once_per_head_and_state(self):
        gh_data = write_fake_gh(self.bin)
        data = {
            "view": {"7": {"headRefOid": "head1", "isDraft": False, "state": "OPEN"}},
            "checks": {"7": [{"name": "ci-ok", "state": "SUCCESS", "bucket": "pass", "link": "https://example.invalid"}]},
        }
        gh_data.write_text(json.dumps(data), encoding="utf-8")
        self.set_queue([7])
        env = {"FAKE_GH_DATA": str(gh_data)}

        first = self.watch(env)
        second = self.watch(env)
        data["view"]["7"]["headRefOid"] = "head2"
        gh_data.write_text(json.dumps(data), encoding="utf-8")
        third = self.watch(env)
        data["checks"]["7"] = [{"name": "ci-ok", "state": "FAILURE", "bucket": "fail", "link": "https://example.invalid"}]
        gh_data.write_text(json.dumps(data), encoding="utf-8")
        fourth = self.watch(env)

        self.assertIn("pull request #7 head1 ci-ok pass", first.stdout)
        self.assertEqual(second.stdout, "")
        self.assertIn("pull request #7 head2 ci-ok pass", third.stdout)
        self.assertIn("pull request #7 head2 ci-ok fail", fourth.stdout)
        for result in (first, second, third, fourth):
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_finished_worker_log_is_reported_once(self):
        outbox = self.repo / ".crew" / "state" / "roles" / "tools" / "outbox"
        result_file = outbox / "001-sample.md"
        result_file.write_text("result\n", encoding="utf-8")
        (outbox / "001-sample.run.log").write_text("engine output\nexit 0\n", encoding="utf-8")

        first = self.watch()
        second = self.watch()

        self.assertIn("result ", first.stdout)
        self.assertIn("001-sample.md", first.stdout)
        self.assertEqual(second.stdout, "")

    def test_once_without_events_exits_zero_and_prints_nothing(self):
        self.set_queue([])

        result = self.watch()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "")


class LintRuleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def lint_line(self, line, config=""):
        fixture = Path(self.temp.name) / "case"
        if fixture.exists():
            shutil.rmtree(str(fixture))
        fixture.mkdir()
        repo = write_fixture_repo(str(fixture), {"tools": role_file()})
        if config:
            (repo / "crew.local.ini").write_text(config, encoding="utf-8")
        (repo / "crew" / "CHECK.md").write_text(line + "\n", encoding="utf-8")
        return run_crew(repo, "lint")

    def assert_bad(self, line, config=""):
        result = self.lint_line(line, config=config)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("crew/CHECK.md:1:", result.stdout.replace("\\", "/"))
        return result

    def assert_good(self, line, config=""):
        result = self.lint_line(line, config=config)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def test_email_rule(self):
        self.assert_bad("write to owner@example.com")
        self.assert_good("write to support@buwiz.com or 295542929+hexuria@users.noreply.github.com")

    def test_absolute_path_rule(self):
        self.assert_bad("open /Users/alice/file.txt")
        self.assert_good("open Users/alice/file.txt")

    def test_devin_and_claude_url_rule(self):
        self.assert_bad("see https://app.devin.ai/sessions/example")
        self.assert_good("see https://devin.ai/docs")

    def test_chan_rule(self):
        self.assert_bad("channel id chan_123")
        self.assert_good("channel id channel-123")

    def test_devin_hex_rule(self):
        self.assert_bad("id devin-0123456789abcdef0123")
        self.assert_good("id devin-0123")

    def test_ghp_rule(self):
        self.assert_bad("token prefix ghp_example")
        self.assert_good("token prefix ghp example")

    def test_github_pat_rule(self):
        self.assert_bad("token prefix github_pat_example")
        self.assert_good("token prefix github pat example")

    def test_sk_rule(self):
        self.assert_bad("key " + "s" + "k-" + "abcdefghijklmnop1234")
        self.assert_good("token prefix sk example")
        # Ordinary words that hold the same letters are not keys.
        self.assert_good("crew run <role> <task-file> --sandbox")
        self.assert_good("a risk-free change to the task-name")

    def test_x_access_token_rule(self):
        self.assert_bad("header x-access-token present")
        self.assert_good("header x access token present")

    def test_password_digits_rule(self):
        self.assert_bad("password 1234")
        self.assert_good("password 123")

    def test_tin_shape_rule_with_documented_placeholders_allowed(self):
        self.assert_bad("TIN 321-654-987")
        self.assert_bad("TIN 321-654-987-001")
        self.assert_good("TIN placeholders 000-000-000 and 123-456-789")
        self.assert_good("not a TIN 321-654-98")

    def test_aws_and_google_key_shapes(self):
        self.assert_bad("aws key AKIA" + "A" * 16)
        self.assert_good("aws key AKIA" + "A" * 15)
        self.assert_bad("google key AIza" + "a" * 35)
        self.assert_good("google key AIza" + "a" * 34)

    def test_jwt_pem_slack_and_stripe_shapes(self):
        self.assert_bad("jwt eyJabcDEF123.abcDEF12345.abcDEF12345")
        self.assert_good("jwt text eyJabc without parts")
        self.assert_bad("-----BEGIN PRIVATE KEY-----")
        self.assert_good("begin private key text")
        self.assert_bad("slack xoxb-1234567890")
        self.assert_bad("slack xoxs-1234567890")
        self.assert_good("slack xox-1234567890")
        self.assert_bad("stripe sk_live_123456")
        self.assert_bad("stripe rk_live_123456")
        self.assert_good("stripe sk_test_123456")

    def test_bearer_and_github_token_shapes(self):
        self.assert_bad("Authorization: Bearer " + "a" * 20)
        self.assert_good("Authorization: Bearer " + "a" * 19)
        for prefix in ("gho_", "ghs_", "ghu_", "ghr_"):
            with self.subTest(prefix=prefix):
                self.assert_bad("token " + prefix + "example")
        self.assert_good("token ghx_example")

    def test_forward_slash_windows_and_private_var_paths(self):
        self.assert_bad("open C:/Users/alice/file.txt")
        self.assert_bad("open /private/var/folders/example")
        self.assert_good("open C:/Users or private/var without a user path shape")

    def test_pin_otp_passcode_password_and_signin_code_digit_rules(self):
        for word in ("PIN", "OTP", "passcode", "password", "sign-in code"):
            with self.subTest(word=word):
                self.assert_bad(word + " 1234")
                self.assert_bad(word + ": 12345678")
                self.assert_good(word + ": 123456789")
        self.assert_good("pinning issue 1234")

    def test_private_deny_words_are_whole_word_case_insensitive_and_redacted(self):
        config = "[lint]\ndeny_words = alice-machine, Bob Person\n"
        result = self.assert_bad("built on ALICE-MACHINE", config=config)
        self.assertNotIn("alice-machine", result.stdout.lower())
        self.assertIn("al", result.stdout)
        self.assertIn("13", result.stdout)
        self.assert_bad("signed by bob person", config=config)
        self.assert_good("built on alice-machine2", config=config)

    def test_repository_name_rule_allows_repos_table_placeholders_and_public_additions(self):
        self.assert_bad("clone acme/private-repo")
        self.assert_good("clone hexuria/buwiz-forms and owner/name and owner/repo")
        self.assert_good("clone hexuria/buwiz-forms-html")

    def test_repository_name_rule_does_not_flag_repository_paths_or_branch_names(self):
        self.assert_good("run crew/bin/crew then scripts/tests/test_crew.py and docs/CI.md")
        self.assert_good("branches fix/example feat/example ci/example test/example origin/main")


class SyncTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        roles = {
            "tools": role_file(name="tools", title="Tools worker", engine="devin", model="swe-test"),
            "docs": role_file(name="docs", title="Docs worker", engine="claude", model="claude-role"),
            "reviewer": role_file(name="reviewer", title="Reviews pull requests", engine="claude", model=""),
            "limited": role_file(name="limited", title="Limited worker", engine="devin", model="",
                                 extra="tools: Read, Grep, Glob"),
        }
        self.repo = write_fixture_repo(self.temp.name, roles)

    def test_sync_writes_prefixed_claude_and_devin_agent_files(self):
        result = run_crew(self.repo, "sync")

        self.assertEqual(result.returncode, 0, result.stderr)
        for folder in (".claude", ".devin"):
            agent = self.repo / folder / "agents" / "crew-tools.md"
            self.assertTrue(agent.exists())
            self.assertFalse((self.repo / folder / "agents" / "tools.md").exists())
            text = agent.read_text(encoding="utf-8")
            self.assertIn("name: crew-tools\n", text)
            self.assertIn("description: Crew role (use only while running the crew): Tools worker\n", text)
            body = text.split("---", 2)[2].strip().splitlines()
            self.assertEqual(len(body), 5)
            self.assertIn("crew/RULES.md", body[0])
            self.assertIn("crew/roles/tools.md", body[1])
            self.assertIn("python3 crew/bin/crew state path", body[2])
            self.assertIn("STATE.md", body[2])
            self.assertIn("before any work", "\n".join(body[:4]))
            self.assertIn("STATE.md", body[4])
            self.assertIn("before you stop", body[4].lower())
        devin_text = (self.repo / ".devin" / "agents" / "crew-tools.md").read_text(encoding="utf-8")
        self.assertIn("model: swe-test\n", devin_text)
        claude_role_devin_text = (self.repo / ".devin" / "agents" / "crew-docs.md").read_text(encoding="utf-8")
        self.assertNotIn("model:", claude_role_devin_text)
        claude_agent = (self.repo / ".claude" / "agents" / "crew-tools.md").read_text(encoding="utf-8")
        self.assertNotIn("model:", claude_agent)
        self.assertNotIn("tools:", claude_agent)

    def test_sync_gives_reviewer_read_only_tools_and_copies_a_role_tools_line(self):
        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)

        reviewer_claude = (self.repo / ".claude" / "agents" / "crew-reviewer.md").read_text(encoding="utf-8")
        reviewer_devin = (self.repo / ".devin" / "agents" / "crew-reviewer.md").read_text(encoding="utf-8")
        limited_claude = (self.repo / ".claude" / "agents" / "crew-limited.md").read_text(encoding="utf-8")
        limited_devin = (self.repo / ".devin" / "agents" / "crew-limited.md").read_text(encoding="utf-8")

        self.assertIn("tools: Read, Grep, Glob\n", reviewer_claude)
        self.assertIn("allowed-tools:\n  - read\n  - grep\n  - glob\n", reviewer_devin)
        self.assertIn("tools: Read, Grep, Glob\n", limited_claude)
        self.assertIn("allowed-tools:\n  - read\n  - grep\n  - glob\n", limited_devin)

    def test_sync_removes_the_old_unprefixed_generated_files(self):
        agents = self.repo / ".claude" / "agents"
        agents.mkdir(parents=True)
        legacy = agents / "tools.md"
        legacy.write_text("old generated file\n", encoding="utf-8")

        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)

        self.assertFalse(legacy.exists())
        self.assertTrue((agents / "crew-tools.md").exists())

    def test_sync_check_passes_after_sync_and_names_a_difference(self):
        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)
        good = run_crew(self.repo, "sync", "--check")
        self.assertEqual(good.returncode, 0, good.stderr + good.stdout)

        agent = self.repo / ".claude" / "agents" / "crew-tools.md"
        agent.write_text(agent.read_text(encoding="utf-8") + "stale line\n", encoding="utf-8")
        bad = run_crew(self.repo, "sync", "--check")

        self.assertEqual(bad.returncode, 1)
        self.assertIn(".claude/agents/crew-tools.md", bad.stdout.replace("\\", "/"))

    def test_sync_check_names_a_missing_devin_agent(self):
        missing = run_crew(self.repo, "sync", "--check")

        self.assertEqual(missing.returncode, 1)
        self.assertIn(".devin/agents/crew-tools.md", missing.stdout.replace("\\", "/"))

    def test_sync_check_names_missing_skill_guard_lines(self):
        skill = self.repo / ".claude" / "skills" / "crew" / "SKILL.md"
        skill.write_text("---\nname: crew\n---\n", encoding="utf-8")
        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)

        result = run_crew(self.repo, "sync", "--check")

        self.assertEqual(result.returncode, 1)
        self.assertIn("disable-model-invocation: true", result.stdout)
        self.assertIn("triggers: [user]", result.stdout)
        self.assertIn(".claude/skills/crew/SKILL.md", result.stdout.replace("\\", "/"))


class StateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def test_state_init_creates_the_layout_and_never_overwrites(self):
        first = run_crew(self.repo, "state", "init")

        self.assertEqual(first.returncode, 0, first.stderr)
        state = self.repo / ".crew" / "state"
        role = state / "roles" / "tools"
        self.assertTrue((role / "STATE.md").exists())
        self.assertTrue((role / "inbox").is_dir())
        self.assertTrue((role / "outbox").is_dir())
        self.assertEqual((state / "queue.json").read_text(encoding="utf-8"), '{"waiting": [], "batch": null}\n')
        self.assertTrue((state / "LOG.md").exists())
        self.assertTrue((state / "HELD.md").exists())
        self.assertTrue((state / ".watch-seen").exists())
        self.assertEqual((state / ".gitignore").read_text(encoding="utf-8"), "*.run.log\n.watch-seen\n*.tmp\n")
        (role / "STATE.md").write_text("custom state\n", encoding="utf-8")
        (state / "queue.json").write_text('{"waiting": [7], "batch": null}\n', encoding="utf-8")

        second = run_crew(self.repo, "state", "init")

        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual((role / "STATE.md").read_text(encoding="utf-8"), "custom state\n")
        self.assertEqual((state / "queue.json").read_text(encoding="utf-8"), '{"waiting": [7], "batch": null}\n')

    def test_state_push_refuses_a_non_noreply_identity(self):
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        subprocess.run(["git", "init", "-q", str(state)], check=True)
        subprocess.run(["git", "config", "user.name", "Somebody"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "somebody@example.com"], cwd=str(state), check=True)
        (self.repo / "crew.local.ini").write_text(
            "[crew]\nstate_dir = {0}\nstate_remote = {1}\n".format(state, remote), encoding="utf-8")

        result = run_crew(self.repo, "state", "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("users.noreply.github.com", result.stderr)
        log = subprocess.run(["git", "log", "--format=%s"], cwd=str(state), capture_output=True, text=True)
        self.assertEqual(log.stdout, "")

    def test_state_init_and_push_create_the_orphan_branch_with_the_checkouts_own_noreply_identity(self):
        state = Path(self.temp.name) / "shared-state"
        remote = Path(self.temp.name) / "shared-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        (self.repo / "crew.local.ini").write_text(
            "[crew]\nstate_dir = {0}\nstate_remote = {1}\n".format(state, remote), encoding="utf-8")

        init = run_crew(self.repo, "state", "init")
        self.assertEqual(init.returncode, 0, init.stderr)
        subprocess.run(["git", "config", "user.name", "Crew Maintainer"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "123456+maintainer@users.noreply.github.com"],
                       cwd=str(state), check=True)
        (state / "roles" / "tools" / "outbox" / "001-result.md").write_text("done\n", encoding="utf-8")
        (state / "roles" / "tools" / "outbox" / "001-result.run.log").write_text("local only\n", encoding="utf-8")

        push = run_crew(self.repo, "state", "push")

        self.assertEqual(push.returncode, 0, push.stderr)
        branch = subprocess.run(["git", "--git-dir", str(remote), "rev-parse", "crew/state"],
                                capture_output=True, text=True, check=True).stdout.strip()
        author = subprocess.run(["git", "--git-dir", str(remote), "log", "-1", "--format=%an <%ae>", branch],
                                capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(author, "Crew Maintainer <123456+maintainer@users.noreply.github.com>")
        tree = subprocess.run(["git", "--git-dir", str(remote), "ls-tree", "-r", "--name-only", branch],
                              capture_output=True, text=True, check=True).stdout
        self.assertIn("roles/tools/outbox/001-result.md", tree)
        self.assertIn("queue.json", tree)
        self.assertIn(".gitignore", tree)
        self.assertNotIn("001-result.run.log", tree)
        self.assertNotIn(".watch-seen", tree)

    def write_state_config(self, state, remote="", branch="crew/state"):
        (self.repo / "crew.local.ini").write_text(
            "[crew]\nstate_dir = {0}\nstate_remote = {1}\nstate_branch = {2}\n".format(state, remote, branch),
            encoding="utf-8")

    def test_state_branch_outside_the_crew_prefixes_is_refused_at_init_pull_and_push(self):
        state = Path(self.temp.name) / "state-checkout"
        for branch in ("main", "master", "feature/state"):
            for command in ("init", "pull", "push"):
                with self.subTest(branch=branch, command=command):
                    if (self.repo / "crew.local.ini").exists():
                        (self.repo / "crew.local.ini").unlink()
                    self.write_state_config(state, "state-remote", branch)
                    result = run_crew(self.repo, "state", command)
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("state_branch", result.stderr)

    def test_state_branch_may_start_with_crew_state(self):
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote), "crew-state")

        result = run_crew(self.repo, "state", "init")

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_state_remote_naming_the_code_checkouts_remote_is_refused_at_init(self):
        init_git_repo(self.repo)
        remote = Path(self.temp.name) / "code-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        subprocess.run(["git", "remote", "add", "origin", str(remote)], cwd=str(self.repo), check=True)
        state = Path(self.temp.name) / "state-checkout"
        self.write_state_config(state, "origin")

        result = run_crew(self.repo, "state", "init")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("state_remote", result.stderr)
        self.assertFalse(state.exists())

    def test_state_remote_is_rechecked_before_every_push(self):
        init_git_repo(self.repo)
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote))
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        subprocess.run(["git", "config", "user.name", "Crew Maintainer"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "123456+maintainer@users.noreply.github.com"],
                       cwd=str(state), check=True)
        subprocess.run(["git", "remote", "add", "origin", str(remote)], cwd=str(self.repo), check=True)

        result = run_crew(self.repo, "state", "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("state_remote", result.stderr)

    def test_state_push_runs_the_privacy_check_over_every_staged_file(self):
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote))
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        subprocess.run(["git", "config", "user.name", "Crew Maintainer"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "123456+maintainer@users.noreply.github.com"],
                       cwd=str(state), check=True)
        result_file = state / "roles" / "tools" / "outbox" / "001-result.md"
        result_file.write_text("token ghp_example\n", encoding="utf-8")

        result = run_crew(self.repo, "state", "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("roles/tools/outbox/001-result.md:1:", result.stderr.replace("\\", "/"))
        self.assertIn("secret", result.stderr)


class DoctorTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"

    def doctor(self):
        env = {"PATH": str(self.bin)}
        return run_crew(self.repo, "doctor", env=env)

    def write_gh(self, signed_in=True):
        source = """
import sys
if sys.argv[1:3] == ['auth', 'status']:
    print('dummy-auth-token', file=sys.stderr)
    sys.exit(0 if {0} else 1)
print('gh version 2.0.0')
""".format("True" if signed_in else "False")
        return write_path_python(self.bin, "gh", source)

    def test_fleet_tier_reports_engine_version_gh_fleet_sbx_state_and_tier(self):
        write_versioned_cli(self.bin, "claude", "1.2.3")
        self.write_gh(signed_in=True)
        write_versioned_cli(self.bin, "sbx", "0.1")
        fleet = write_fake_engine(self.temp.name)
        (self.repo / "crew.local.ini").write_text("[fleet]\ncommand = {0}\n".format(
            shell_join([sys.executable, fleet])), encoding="utf-8")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("engine claude:", result.stdout)
        self.assertIn("claude version 1.2.3", result.stdout)
        self.assertIn("gh: signed in", result.stdout)
        self.assertIn("fleet: set", result.stdout)
        self.assertIn("sbx: found", result.stdout)
        self.assertIn("state:", result.stdout)
        self.assertIn("tier: fleet", result.stdout)
        self.assertNotIn("dummy-auth-token", result.stdout + result.stderr)

    def test_workers_tier_when_devin_is_found_without_a_fleet(self):
        write_versioned_cli(self.bin, "devin", "3000.0")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("engine devin:", result.stdout)
        self.assertIn("tier: workers", result.stdout)

    def test_solo_tier_when_only_claude_is_found(self):
        write_versioned_cli(self.bin, "claude", "1.0")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("fleet: not set", result.stdout)
        self.assertIn("tier: solo", result.stdout)

    def test_no_engine_exits_nonzero(self):
        result = self.doctor()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tier: none", result.stdout)

    def test_doctor_shows_the_merge_and_tag_authority_settings(self):
        write_versioned_cli(self.bin, "claude", "1.0")
        (self.repo / "crew.local.ini").write_text("[crew]\nmerge = yes\ntag = no\n", encoding="utf-8")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("merge: yes", result.stdout)
        self.assertIn("tag: no", result.stdout)


class RunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_devin_run_logs_exit_passes_model_worktree_and_removes_temp_files(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        env = {"FAKE_ENGINE_RECORD": str(record_path), "FAKE_ENGINE_EXIT": "7"}

        result = run_crew(repo, "run", "tools", str(task), "--model", "override-model",
                          "--worktree", str(repo), env=env)

        self.assertEqual(result.returncode, 7, result.stderr)
        log = repo / ".crew" / "state" / "roles" / "tools" / "outbox" / "001-sample.run.log"
        self.assertTrue(log.exists())
        self.assertIn("fake engine ran", log.read_text(encoding="utf-8"))
        self.assertEqual(log.read_text(encoding="utf-8").splitlines()[-1], "exit 7")
        record = json.loads(record_path.read_text(encoding="utf-8"))
        argv = record["argv"]
        self.assertEqual(record["model"], "override-model")
        self.assertEqual(Path(record["cwd"]).resolve(), repo.resolve())
        self.assertEqual(record["git_lfs_skip_push"], "1")
        for flag in ("-p", "--config", "--permission-mode", "--respect-workspace-trust", "--prompt-file"):
            self.assertIn(flag, argv)
        self.assertEqual(argv[argv.index("--permission-mode") + 1], "accept-edits")
        self.assertEqual(argv[argv.index("--respect-workspace-trust") + 1], "false")
        self.assertFalse(Path(record["config_path"]).exists())
        self.assertFalse(Path(record["prompt_file"]).exists())
        permissions = record["config"]["permissions"]
        tree = str(repo.resolve())
        role_dir = str((repo / ".crew" / "state" / "roles" / "tools").resolve())
        gitdir = str((repo / ".git").resolve())
        for entry in ("Write({0})".format(tree), "Write({0})".format(role_dir),
                      "Write({0}/objects/**)".format(gitdir), "Write({0}/refs/**)".format(gitdir),
                      "Write({0}/index)".format(gitdir), "Write({0}/HEAD)".format(gitdir),
                      "Write({0}/logs/**)".format(gitdir),
                      "Exec(git status)", "Exec(git commit)", "Exec(python3 crew/bin/crew push)",
                      "Exec(gh pr view)", "Exec(gh issue create)"):
            self.assertIn(entry, permissions["allow"])
        self.assertNotIn("Write({0})".format(gitdir), permissions["allow"])
        self.assertNotIn("Exec(git)", permissions["allow"])
        self.assertNotIn("Exec(python3)", permissions["allow"])
        for denied in ("old", "Exec(git push)", "Exec(gh pr merge)", "Exec(gh pr close)",
                       "Exec(gh run cancel)", "Exec(gh release)", "Exec(cargo)", "Exec(nsh)",
                       "Exec(git tag)", "Exec(git config)", "Write({0}/config)".format(gitdir),
                       "Write({0}/hooks)".format(gitdir)):
            self.assertIn(denied, permissions["deny"])
        self.assertEqual(record["config"]["user_setting"], {"kept": True})
        self.assertNotIn("old", permissions["allow"])
        self.assertIn("TASK FOR WORKER", record["prompt"])

    def test_claude_run_uses_stdin_model_permission_mode_and_allowed_tools(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="claude", model="claude-role-model")
        env = {"FAKE_ENGINE_RECORD": str(record_path)}

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(repo), env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        argv = record["argv"]
        self.assertEqual(argv[0], "-p")
        self.assertEqual(record["model"], "claude-role-model")
        self.assertEqual(argv[argv.index("--permission-mode") + 1], "acceptEdits")
        tools = argv[argv.index("--allowedTools") + 1]
        expected_git = ",".join("Bash(git {0}:*)".format(name) for name in (
            "status", "diff", "log", "show", "add", "commit", "switch", "checkout", "fetch", "merge",
            "rebase", "stash", "rev-parse", "ls-files", "branch", "restore"))
        self.assertEqual(tools, "Read,Edit,Write,Glob,Grep," + expected_git + ","
                                "Bash(python3 crew/bin/crew push:*),"
                                "Bash(gh pr view:*),Bash(gh pr list:*),Bash(gh pr checks:*),"
                                "Bash(gh pr diff:*),Bash(gh pr create:*),Bash(gh run view:*),"
                                "Bash(gh run list:*),Bash(gh issue list:*),Bash(gh issue view:*),"
                                "Bash(gh issue create:*)")
        self.assertNotIn("Bash(git:*)", tools)
        self.assertNotIn("Bash(python3:*)", tools)
        disallowed = argv[argv.index("--disallowedTools") + 1]
        self.assertEqual(disallowed, "Bash(git push:*),Bash(gh pr merge:*),Bash(gh pr close:*),"
                                     "Bash(gh run cancel:*),Bash(gh release:*),Bash(git tag:*),"
                                     "Bash(cargo:*),Bash(nsh:*)")
        self.assertIn("--setting-sources=", argv)
        self.assertIsNone(record["config_path"])
        self.assertIn("TASK FOR WORKER", record["stdin"])
        self.assertIn("Role body line.", record["stdin"])
        log = repo / ".crew" / "state" / "roles" / "tools" / "outbox" / "001-sample.run.log"
        self.assertEqual(log.read_text(encoding="utf-8").splitlines()[-1], "exit 0")

    def test_worker_environment_is_scrubbed_with_named_pass_env_and_a_restricted_gh_token(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        config = repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8") +
                          "gh_token_env = RESTRICTED_CREW_GH\n", encoding="utf-8")
        env = {
            "FAKE_ENGINE_RECORD": str(record_path),
            "RANDOM_CREW_VAR": "not-passed",
            "SECRET_THING": "not-passed",
            "EXAMPLE_API_KEY": "not-passed",
            "GH_TOKEN": "host-token-not-passed",
            "RESTRICTED_CREW_GH": "restricted-worker-token",
            "LC_CREW_TEST": "passed-locale",
            "CLAUDE_CONFIG_DIR": str(repo / "claude-config"),
        }

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(repo), env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        worker_env = record["env"]
        self.assertEqual(worker_env.get("GIT_LFS_SKIP_PUSH"), "1")
        self.assertEqual(worker_env.get("LC_CREW_TEST"), "passed-locale")
        self.assertEqual(worker_env.get("CLAUDE_CONFIG_DIR"), str(repo / "claude-config"))
        self.assertEqual(worker_env.get("GH_TOKEN"), "restricted-worker-token")
        self.assertIn("PATH", worker_env)
        for name in ("RANDOM_CREW_VAR", "SECRET_THING", "EXAMPLE_API_KEY"):
            self.assertNotIn(name, worker_env)
        self.assertNotEqual(worker_env.get("GH_TOKEN"), "host-token-not-passed")

    @unittest.skipIf(os.name == "nt", "POSIX signal delivery test")
    def test_sigterm_removes_the_devin_temp_config_and_prompt(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        config = repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            "FAKE_ENGINE_EXIT", "FAKE_ENGINE_EXIT,FAKE_ENGINE_SLEEP"), encoding="utf-8")
        env = os.environ.copy()
        env.update({"FAKE_ENGINE_RECORD": str(record_path), "FAKE_ENGINE_SLEEP": "30"})
        process = subprocess.Popen([sys.executable, str(repo / "crew" / "bin" / "crew"), "run", "tools",
                                    str(task), "--worktree", str(repo)], cwd=str(repo), env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.time() + 10
            while time.time() < deadline and not record_path.exists():
                time.sleep(0.05)
            self.assertTrue(record_path.exists(), "the fake engine did not start")
            record = json.loads(record_path.read_text(encoding="utf-8"))
            process.send_signal(signal.SIGTERM)
            process.wait(timeout=10)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
        self.assertNotEqual(process.returncode, 0)
        self.assertFalse(Path(record["config_path"]).exists())
        self.assertFalse(Path(record["prompt_file"]).exists())


class AutomergeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"
        self.enable_merge()
        self.write_verdict()

    def enable_merge(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nmerge = yes\n", encoding="utf-8")

    def write_verdict(self, head="abc123", engine="claude", verdict="MERGE"):
        outbox = self.repo / ".crew" / "state" / "roles" / "reviewer" / "outbox"
        outbox.mkdir(parents=True, exist_ok=True)
        (outbox / "owner-repo-7.verdict").write_text(
            "verdict: {0}\nhead: {1}\nengine: {2}\nmodel: test-model\n".format(verdict, head, engine),
            encoding="utf-8")

    def base_view(self, **changes):
        view = {
            "state": "OPEN",
            "isDraft": False,
            "baseRefName": "main",
            "headRefOid": "abc123",
            "mergeStateStatus": "CLEAN",
            "mergeCommit": None,
            "files": [{"path": "docs/example.md"}],
            "statusCheckRollup": [
                {"__typename": "CheckRun", "name": "ci-ok", "status": "COMPLETED", "conclusion": "SUCCESS"},
            ],
        }
        view.update(changes)
        return view

    def write_gh(self, view=None, checks=None):
        record = Path(self.temp.name) / "automerge-gh-record.jsonl"
        data_path = Path(self.temp.name) / "automerge-gh-data.json"
        data = {
            "view": view if view is not None else self.base_view(),
            "checks": checks if checks is not None else [
                {"name": "ci-ok", "state": "SUCCESS", "bucket": "pass", "link": "https://example.invalid/ci"},
                {"name": "Linux tests", "state": "SUCCESS", "bucket": "pass", "link": "https://example.invalid/linux"},
            ],
            "merged": False,
        }
        data_path.write_text(json.dumps(data), encoding="utf-8")
        source = r'''
import json
import os
import sys

argv = sys.argv[1:]
record_path = os.environ["GH_RECORD"]
data_path = os.environ["FAKE_GH_DATA"]
with open(record_path, "a", encoding="utf-8") as handle:
    handle.write(json.dumps(argv) + "\n")
with open(data_path, encoding="utf-8") as handle:
    data = json.load(handle)
if argv[:2] == ["repo", "view"]:
    print(json.dumps({"defaultBranchRef": {"name": "main"}}))
elif argv[:2] == ["pr", "merge"]:
    print("merged")
    data["merged"] = True
    with open(data_path, "w", encoding="utf-8") as handle:
        json.dump(data, handle)
    sys.exit(int(data.get("merge_exit", 0)))
elif argv[:2] == ["pr", "view"]:
    view = json.loads(json.dumps(data["view"]))
    if data.get("merged"):
        view.update({"state": "MERGED", "mergeStateStatus": "CLEAN",
                     "mergeCommit": {"oid": "merge123"}})
    print(json.dumps(view))
elif argv[:2] == ["pr", "checks"]:
    checks = data.get("checks", [])
    print(json.dumps(checks))
    if any(row.get("bucket") == "fail" for row in checks):
        sys.exit(1)
    if any(row.get("bucket") == "pending" for row in checks):
        sys.exit(8)
'''
        write_path_python(self.bin, "gh", source)
        return record, data_path

    def automerge(self, record, data_path, *extra):
        env = {"PATH": str(self.bin), "GH_RECORD": str(record), "FAKE_GH_DATA": str(data_path)}
        return run_crew(self.repo, "automerge", "7", "abc123", "--repo", "owner/repo",
                        "--interval", "0", *extra, env=env)

    def merge_calls(self, record):
        if not record.exists():
            return []
        return [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()
                if json.loads(line)[:2] == ["pr", "merge"]]

    def test_clean_green_pull_request_merges_at_the_exact_head(self):
        record, data_path = self.write_gh()

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn("#7 MERGED merge123", result.stdout)
        merge = self.merge_calls(record)[0]
        self.assertIn("--match-head-commit", merge)
        self.assertIn("abc123", merge)
        checks_call = next(json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()
                           if json.loads(line)[:2] == ["pr", "checks"])
        self.assertIn("--json", checks_call)
        self.assertIn("name,state,bucket,link", checks_call)

    def test_missing_merge_authority_refuses_before_contacting_github(self):
        (self.repo / "crew.local.ini").write_text("[crew]\n", encoding="utf-8")
        record, data_path = self.write_gh()

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("[crew] merge = yes", result.stderr)
        self.assertFalse(record.exists())

    def test_draft_pull_request_refuses(self):
        record, data_path = self.write_gh(view=self.base_view(isDraft=True))

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("draft", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_base_must_be_the_repository_default_branch(self):
        record, data_path = self.write_gh(view=self.base_view(baseRefName="feature/base"))

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("default branch", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_closed_unmerged_pull_request_returns_nonzero(self):
        record, data_path = self.write_gh(view=self.base_view(state="CLOSED"))

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CLOSED", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_moved_head_refuses_to_merge(self):
        record, data_path = self.write_gh(view=self.base_view(headRefOid="different"))

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 2)
        self.assertIn("head moved", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_failing_checks_are_seen_even_when_gh_exits_one(self):
        checks = [
            {"name": "ci-ok", "state": "SUCCESS", "bucket": "pass", "link": ""},
            {"name": "Windows tests", "state": "FAILURE", "bucket": "fail", "link": ""},
        ]
        record, data_path = self.write_gh(checks=checks)

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 3)
        self.assertIn("Windows tests", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_a_cancelled_ci_ok_is_a_failure_not_a_pass(self):
        checks = [{"name": "ci-ok", "state": "CANCELLED", "bucket": "cancel", "link": ""}]
        record, data_path = self.write_gh(checks=checks)

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 3)
        self.assertIn("ci-ok", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_pending_checks_are_seen_even_when_gh_exits_eight(self):
        checks = [{"name": "ci-ok", "state": "", "bucket": "pending", "link": ""}]
        record, data_path = self.write_gh(checks=checks)

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 4)
        self.assertIn("timed out", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_ci_ok_must_pass_at_the_given_head_in_the_pr_view_rollup(self):
        view = self.base_view(statusCheckRollup=[])
        record, data_path = self.write_gh(view=view)

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("ci-ok", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_a_running_batch_in_queue_json_refuses_to_merge(self):
        state = self.repo / ".crew" / "state"
        state.mkdir(parents=True, exist_ok=True)
        (state / "queue.json").write_text(
            json.dumps({"waiting": [], "batch": {"branch": "test/linux-windows/batch-7", "prs": [7]}}),
            encoding="utf-8")
        record, data_path = self.write_gh()

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("batch", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_no_verdict_refuses_to_merge(self):
        (self.repo / ".crew" / "state" / "roles" / "reviewer" / "outbox" /
         "owner-repo-7.verdict").unlink()
        record, data_path = self.write_gh()

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("verdict", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_a_stale_verdict_head_refuses_to_merge(self):
        self.write_verdict(head="other-head")
        record, data_path = self.write_gh()

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("stale", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_a_devin_verdict_on_a_sensitive_path_refuses_to_merge(self):
        self.write_verdict(engine="devin")
        view = self.base_view(files=[{"path": "crates/bir-core/src/account.rs"}])
        record, data_path = self.write_gh(view=view)

        result = self.automerge(record, data_path)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sensitive", result.stdout)
        self.assertFalse(self.merge_calls(record))

    def test_a_devin_verdict_on_a_docs_only_pull_request_may_merge(self):
        self.write_verdict(engine="devin")
        record, data_path = self.write_gh(view=self.base_view(files=[{"path": "docs/example.md"}]))

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertTrue(self.merge_calls(record))

    def test_a_claude_verdict_on_a_sensitive_path_may_merge(self):
        view = self.base_view(files=[{"path": "nsh/src/auth_session.rs"}])
        record, data_path = self.write_gh(view=view)

        result = self.automerge(record, data_path)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertTrue(self.merge_calls(record))


class BatchwatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"
        (self.repo / "crew.local.ini").write_text("[crew]\nmerge = yes\n", encoding="utf-8")

    def write_verdict(self, number="7", head="abc123", engine="claude"):
        outbox = self.repo / ".crew" / "state" / "roles" / "reviewer" / "outbox"
        outbox.mkdir(parents=True, exist_ok=True)
        (outbox / "owner-repo-{0}.verdict".format(number)).write_text(
            "verdict: MERGE\nhead: {0}\nengine: {1}\nmodel: test-model\n".format(head, engine),
            encoding="utf-8")

    def write_just(self, status_lines, merge_lines=None, merge_exit=0, verify_exit=0):
        merge_lines = merge_lines if merge_lines is not None else [
            "gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123",
        ]
        record = Path(self.temp.name) / "just-record.jsonl"
        source = '''
import json
import os
import sys
args = sys.argv[1:]
with open(os.environ['JUST_RECORD'], 'a', encoding='utf-8') as handle:
    handle.write(json.dumps(args) + '\\n')
if args and args[0] == 'ci-batch-status':
    print({0!r})
elif args and args[0] == 'ci-batch-merge':
    print('\\n'.join({1!r}))
    sys.exit({2})
elif args and args[0] == 'ci-batch-verify':
    print('equal: tested tree')
    sys.exit({3})
'''.format("\n".join(status_lines), merge_lines, merge_exit, verify_exit)
        write_path_python(self.bin, "just", source)
        return record

    def write_gh(self, fail_numbers=(), files_by_number=None):
        record = Path(self.temp.name) / "gh-record.jsonl"
        files_by_number = files_by_number or {}
        source = r'''
import json
import os
import sys
argv = sys.argv[1:]
with open(os.environ["GH_RECORD"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(argv) + "\n")
if argv[:2] == ["pr", "merge"]:
    print("merged")
    sys.exit(1 if argv[2] in {0!r} else 0)
elif argv[:2] == ["pr", "view"]:
    files = {1!r}.get(argv[2], [{{"path": "docs/example.md"}}])
    print(json.dumps({{"state": "MERGED", "mergeCommit": {{"oid": "merge123"}}, "files": files}}))
'''.format(tuple(fail_numbers), files_by_number)
        write_path_python(self.bin, "gh", source)
        return record

    def batchwatch(self, record, just_record, prs=("7",), *extra):
        env = {"PATH": str(self.bin), "GH_RECORD": str(record), "JUST_RECORD": str(just_record)}
        return run_crew(self.repo, "batchwatch", "test/linux-windows/batch-7", *prs, *extra, env=env)

    def gh_calls(self, record, prefix):
        if not record.exists():
            return []
        return [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()
                if json.loads(line)[:2] == prefix]

    def just_calls(self, record, name):
        if not record.exists():
            return []
        return [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()
                if json.loads(line) and json.loads(line)[0] == name]

    def test_green_batch_merges_with_the_body_and_verifies(self):
        self.write_verdict()
        just_record = self.write_just(["batch: green"])
        record = self.write_gh()

        result = self.batchwatch(record, just_record)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn("batch: green", result.stdout)
        self.assertIn("equal: tested tree", result.stdout)
        merge = self.gh_calls(record, ["pr", "merge"])[0]
        self.assertIn("--body", merge)
        self.assertIn("Merged by the orchestrator: reviewed; proven with main in one batch (test/linux-windows/batch-7).",
                      merge)
        self.assertIn("--match-head-commit", merge)
        self.assertTrue(self.just_calls(just_record, "ci-batch-verify"))

    def test_green_no_merge_stops_after_printing_green(self):
        just_record = self.write_just(["batch: green"])
        record = self.write_gh()

        result = self.batchwatch(record, just_record, ("7",), "--no-merge")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("green", result.stdout)
        self.assertFalse(record.exists())
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_without_merge_authority_batchwatch_prints_the_commands_and_merges_nothing(self):
        (self.repo / "crew.local.ini").write_text("[crew]\n", encoding="utf-8")
        just_record = self.write_just(["batch: green"])
        record = self.write_gh()

        result = self.batchwatch(record, just_record)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn("[crew] merge = yes", result.stdout)
        self.assertIn("gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123", result.stdout)
        self.assertFalse(record.exists())
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_ci_batch_merge_failure_stops_before_merging_or_verifying(self):
        self.write_verdict()
        just_record = self.write_just(["batch: green"], merge_exit=5)
        record = self.write_gh()

        result = self.batchwatch(record, just_record)

        self.assertEqual(result.returncode, 5)
        self.assertFalse(self.gh_calls(record, ["pr", "merge"]))
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_a_failed_first_merge_reports_merged_none_and_every_pull_request_not_merged(self):
        self.write_verdict("7", "abc123")
        self.write_verdict("8", "def456")
        merge_lines = [
            "gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123",
            "gh pr merge 8 --repo owner/repo --squash --match-head-commit def456",
        ]
        just_record = self.write_just(["batch: green"], merge_lines=merge_lines)
        record = self.write_gh(fail_numbers=("7",))

        result = self.batchwatch(record, just_record, ("7", "8"))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("merged: none", result.stdout)
        self.assertIn("not merged: #7, #8", result.stdout)
        self.assertEqual(len(self.gh_calls(record, ["pr", "merge"])), 1)
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_a_failed_later_merge_reports_what_merged_and_never_verifies(self):
        self.write_verdict("7", "abc123")
        self.write_verdict("8", "def456")
        merge_lines = [
            "gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123",
            "gh pr merge 8 --repo owner/repo --squash --match-head-commit def456",
        ]
        just_record = self.write_just(["batch: green"], merge_lines=merge_lines)
        record = self.write_gh(fail_numbers=("8",))

        result = self.batchwatch(record, just_record, ("7", "8"))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("merged: #7", result.stdout)
        self.assertIn("not merged: #8", result.stdout)
        self.assertEqual(len(self.gh_calls(record, ["pr", "merge"])), 2)
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_batchwatch_requires_a_claude_verdict_for_a_sensitive_path(self):
        self.write_verdict(engine="devin")
        just_record = self.write_just(["batch: green"])
        record = self.write_gh(files_by_number={"7": [{"path": "crates/bir-core/src/print_filing.rs"}]})

        result = self.batchwatch(record, just_record)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("sensitive", result.stdout)
        self.assertFalse(self.gh_calls(record, ["pr", "merge"]))

    def test_red_batch_prints_failed_jobs_and_merges_nothing(self):
        just_record = self.write_just(["batch: red", "    failed job: Linux tests (failure)  https://example.invalid"])
        record = self.write_gh()

        result = self.batchwatch(record, just_record)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed job: Linux tests", result.stdout)
        self.assertFalse(record.exists())


class SandboxRunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file(engine="devin", model="sand-model")})
        (self.repo / "crew.local.ini").write_text("[worker]\npass_env = SBX_RECORD,GIT_RECORD\n", encoding="utf-8")
        self.task = self.repo / "001-sample.md"
        self.task.write_text("TASK\n", encoding="utf-8")

    def test_sandbox_print_shows_the_four_documented_commands_and_runs_nothing(self):
        result = run_crew(self.repo, "run", "tools", str(self.task), "--sandbox", "--print", env={"PATH": ""})

        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(len(lines), 4, result.stdout)
        tree = str(self.repo.resolve())
        name = "crew-tools-001-sample"
        self.assertEqual(lines[0], "sbx create --name {0} --clone devin {1}".format(name, tree))
        self.assertEqual(lines[1], "sbx exec {0} -- devin -p --model sand-model --prompt-file {1}".format(
            name, str((self.repo / ".crew" / "state" / "prompts" / "001-sample.md").resolve())))
        self.assertEqual(lines[2], "git -C {0} fetch sandbox-{1}".format(tree, name))
        self.assertEqual(lines[3], "sbx rm --force {0}".format(name))

    def test_sandbox_without_sbx_refuses_and_names_the_playbook(self):
        result = run_crew(self.repo, "run", "tools", str(self.task), "--sandbox", env={"PATH": ""})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("crew/playbooks/sandbox.md", result.stderr)
        self.assertEqual(len(result.stderr.strip().splitlines()), 1)

    def write_sandbox_bin(self, create_exit=0, exec_exit=0):
        bin_dir = Path(self.temp.name) / "bin"
        sbx_record = Path(self.temp.name) / "sbx-record.jsonl"
        git_record = Path(self.temp.name) / "git-record.jsonl"
        sbx_source = """
import json
import os
import sys
argv = sys.argv[1:]
with open(os.environ['SBX_RECORD'], 'a', encoding='utf-8') as handle:
    handle.write(json.dumps({{'argv': argv, 'random': os.environ.get('RANDOM_CREW_VAR'),
                             'lfs': os.environ.get('GIT_LFS_SKIP_PUSH')}}) + '\\n')
if argv and argv[0] == 'create':
    sys.exit({0})
if argv and argv[0] == 'exec':
    print('sandbox worker output')
    sys.exit({1})
sys.exit(0)
""".format(create_exit, exec_exit)
        write_path_python(bin_dir, "sbx", sbx_source)
        git_source = """
import json
import os
import sys
with open(os.environ['GIT_RECORD'], 'a', encoding='utf-8') as handle:
    handle.write(json.dumps(sys.argv[1:]) + '\\n')
sys.exit(0)
"""
        write_path_python(bin_dir, "git", git_source)
        return bin_dir, sbx_record, git_record

    def read_json_lines(self, path):
        if not path.exists():
            return []
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]

    def test_sandbox_fetches_after_a_nonzero_worker_exit_and_removes_only_a_created_sandbox(self):
        bin_dir, sbx_record, git_record = self.write_sandbox_bin(exec_exit=7)
        env = {"PATH": str(bin_dir), "SBX_RECORD": str(sbx_record), "GIT_RECORD": str(git_record),
               "RANDOM_CREW_VAR": "not-passed"}

        result = run_crew(self.repo, "run", "tools", str(self.task), "--sandbox", env=env)

        self.assertEqual(result.returncode, 7, result.stderr)
        sbx_calls = self.read_json_lines(sbx_record)
        self.assertEqual([call["argv"][0] for call in sbx_calls], ["create", "exec", "rm"])
        self.assertTrue(any(call[:2] == ["-C", str(self.repo.resolve())] and "fetch" in call
                            for call in self.read_json_lines(git_record)))
        self.assertFalse((self.repo / ".crew" / "state" / "prompts" / "001-sample.md").exists())
        create = sbx_calls[0]
        self.assertIsNone(create["random"])
        self.assertEqual(create["lfs"], "1")

    def test_sandbox_create_failure_is_not_removed_and_is_not_fetched(self):
        bin_dir, sbx_record, git_record = self.write_sandbox_bin(create_exit=3)
        env = {"PATH": str(bin_dir), "SBX_RECORD": str(sbx_record), "GIT_RECORD": str(git_record)}

        result = run_crew(self.repo, "run", "tools", str(self.task), "--sandbox", env=env)

        self.assertEqual(result.returncode, 3, result.stderr)
        sbx_calls = self.read_json_lines(sbx_record)
        self.assertEqual([call["argv"][0] for call in sbx_calls], ["create"])
        self.assertEqual(self.read_json_lines(git_record), [])


class StartTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def test_start_print_claude_prints_the_resolved_interactive_command(self):
        shim = write_versioned_cli(Path(self.temp.name) / "bin", "claude", "1.0")
        result = run_crew(self.repo, "start", "--role", "tools", "--engine", "claude", "--print",
                          env={"PATH": str(Path(self.temp.name) / "bin")})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(shim), result.stdout)
        self.assertIn("Re-instate from files: follow crew/playbooks/restart.md.", result.stdout)
        self.assertIn("Role body line.", result.stdout)

    def test_start_print_devin_prints_the_resolved_prompt_file_command(self):
        shim = write_versioned_cli(Path(self.temp.name) / "bin", "devin", "3000.0")
        result = run_crew(self.repo, "start", "--role", "tools", "--engine", "devin", "--print",
                          env={"PATH": str(Path(self.temp.name) / "bin")})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(shim), result.stdout)
        self.assertIn("--prompt-file ", result.stdout)
        parts = result.stdout.strip().split()
        prompt_path = Path(parts[2])
        self.assertTrue(prompt_path.exists())
        self.assertIn("Re-instate from files: follow crew/playbooks/restart.md.",
                      prompt_path.read_text(encoding="utf-8"))


class CrewPushTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.remote = Path(self.temp.name) / "remote.git"
        subprocess.run(["git", "init", "-q", "--bare", "--initial-branch=main", str(self.remote)], check=True)
        subprocess.run(["git", "init", "-q", "--initial-branch=main"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "config", "user.name", "Crew Maintainer"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "config", "user.email", "123456+maintainer@users.noreply.github.com"],
                       cwd=str(self.repo), check=True)
        (self.repo / "base.txt").write_text("base\n", encoding="utf-8")
        subprocess.run(["git", "add", "base.txt"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "base"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "remote", "add", "origin", str(self.remote)], cwd=str(self.repo), check=True)
        subprocess.run(["git", "push", "-q", "origin", "main"], cwd=str(self.repo), check=True)

    def switch_branch(self, name):
        subprocess.run(["git", "checkout", "-q", "-b", name], cwd=str(self.repo), check=True)
        (self.repo / "work.txt").write_text(name + "\n", encoding="utf-8")
        subprocess.run(["git", "add", "work.txt"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "work"], cwd=str(self.repo), check=True)

    def test_push_pushes_the_current_feature_branch_to_origin_under_the_same_name(self):
        self.switch_branch("feat/worker-change")

        result = run_crew(self.repo, "push")

        self.assertEqual(result.returncode, 0, result.stderr)
        local = subprocess.run(["git", "rev-parse", "feat/worker-change"], cwd=str(self.repo),
                               capture_output=True, text=True, check=True).stdout.strip()
        remote = subprocess.run(["git", "--git-dir", str(self.remote), "rev-parse", "refs/heads/feat/worker-change"],
                                capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(local, remote)

    def test_push_refuses_main_master_buwiz_next_bad_prefixes_force_plus_tags_and_detached_head(self):
        cases = []
        for branch in ("main", "master", "buwiz-next", "feature/worker-change"):
            subprocess.run(["git", "checkout", "-q", "-B", branch], cwd=str(self.repo), check=True)
            result = run_crew(self.repo, "push")
            cases.append((branch, result))
        self.switch_branch("feat/plus+branch")
        plus = run_crew(self.repo, "push")
        force = run_crew(self.repo, "push", "--force")
        subprocess.run(["git", "checkout", "-q", "--detach", "HEAD"], cwd=str(self.repo), check=True)
        detached = run_crew(self.repo, "push")

        for branch, result in cases:
            self.assertNotEqual(result.returncode, 0, branch)
        for result in (plus, force, detached):
            self.assertNotEqual(result.returncode, 0)

    def test_push_refuses_a_branch_name_that_is_also_a_tag(self):
        self.switch_branch("feat/tagged")
        subprocess.run(["git", "tag", "feat/tagged"], cwd=str(self.repo), check=True)

        result = run_crew(self.repo, "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tag", result.stderr)


class VerdictCommandTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def test_verdict_prints_the_reviewers_key_value_file(self):
        outbox = self.repo / ".crew" / "state" / "roles" / "reviewer" / "outbox"
        outbox.mkdir(parents=True, exist_ok=True)
        text = "verdict: MERGE\nhead: abc123\nengine: claude\nmodel: test-model\n"
        (outbox / "owner-repo-7.verdict").write_text(text, encoding="utf-8")

        result = run_crew(self.repo, "verdict", "owner/repo", "7")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, text)

    def test_verdict_missing_is_nonzero(self):
        result = run_crew(self.repo, "verdict", "owner/repo", "7")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("verdict", result.stderr)


class FrontmatterTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_good_frontmatter_composes_the_role_body(self):
        repo = write_fixture_repo(self.temp.name, {"tools": role_file(body="Do the tools work.\n")})

        result = run_crew(repo, "prompt", "tools")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Do the tools work.", result.stdout)
        self.assertNotIn("title: Tools worker", result.stdout)

    def test_missing_frontmatter_is_an_error_naming_the_role_file(self):
        repo = write_fixture_repo(self.temp.name, {"tools": "No frontmatter here.\n"})

        result = run_crew(repo, "prompt", "tools")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(repo / "crew" / "roles" / "tools.md"), result.stderr)

    def test_unknown_frontmatter_keys_are_kept_and_ignored(self):
        text = role_file(extra="future-key: future value", body="Unknown keys do not matter.\n")
        repo = write_fixture_repo(self.temp.name, {"tools": text})

        result = run_crew(repo, "prompt", "tools")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Unknown keys do not matter.", result.stdout)


class RepoPlumbingTests(unittest.TestCase):
    def test_justfile_has_one_portable_crew_recipe(self):
        text = (ROOT / "justfile").read_text(encoding="utf-8")

        self.assertIn("\ncrew *args:\n    {{PYTHON}} crew/bin/crew {{quote(args)}}\n", text)
        self.assertEqual(text.count("\ncrew *args:"), 1)

    def test_gitignore_keeps_local_state_private_and_unignores_only_crew_files_by_name(self):
        lines = (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()

        self.assertIn("/.crew/", lines)
        self.assertIn("/crew.local.ini", lines)
        self.assertIn("/.claude/*", lines)
        self.assertNotIn("/.claude/", lines)
        for pattern in ("!/.claude/agents/", "/.claude/agents/*", "!/.claude/agents/crew-*.md",
                        "!/.claude/skills/", "/.claude/skills/*", "!/.claude/skills/crew/",
                        "/.devin/agents/*", "!/.devin/agents/crew-*.md"):
            self.assertIn(pattern, lines)
        self.assertLess(lines.index("/.claude/*"), lines.index("!/.claude/agents/"))
        self.assertLess(lines.index("!/.claude/agents/"), lines.index("/.claude/agents/*"))
        self.assertLess(lines.index("/.claude/agents/*"), lines.index("!/.claude/agents/crew-*.md"))
        self.assertLess(lines.index("!/.claude/skills/"), lines.index("/.claude/skills/*"))
        self.assertLess(lines.index("/.claude/skills/*"), lines.index("!/.claude/skills/crew/"))

    def test_local_example_ini_documents_every_setting(self):
        path = ROOT / "crew" / "local.example.ini"
        text = path.read_text(encoding="utf-8")
        parser = configparser.ConfigParser(interpolation=None)
        parser.read_string(text)

        self.assertEqual(parser["crew"]["state_dir"], ".crew/state")
        self.assertIn("state_remote", parser["crew"])
        self.assertEqual(parser["crew"]["state_branch"], "crew/state")
        self.assertEqual(parser["crew"]["holder"], "conductor")
        self.assertIn("engine", parser["crew"])
        self.assertIn("merge", parser["crew"])
        self.assertIn("tag", parser["crew"])
        self.assertIn("pass_env", parser["worker"])
        self.assertIn("gh_token_env", parser["worker"])
        self.assertIn("deny_words", parser["lint"])
        self.assertEqual(parser["engine.devin"]["command"], "devin")
        self.assertEqual(parser["engine.devin"]["model"], "swe-2-1-max")
        self.assertEqual(parser["engine.devin"]["config"], "~/.config/devin/config.json")
        self.assertEqual(parser["engine.claude"]["command"], "claude")
        self.assertIn("model", parser["engine.claude"])
        self.assertIn("command", parser["fleet"])
        for line in text.splitlines():
            if "=" in line and not line.lstrip().startswith("#"):
                previous = text.splitlines()[text.splitlines().index(line) - 1]
                self.assertTrue(previous.lstrip().startswith("#"), line)

    def test_real_tree_lint_and_sync_check_pass(self):
        for args in (("lint",), ("sync", "--check")):
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, str(CREW), *args], cwd=str(ROOT),
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
