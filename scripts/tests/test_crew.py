"""crew/bin/crew, the portable launch crew runner, against fixture repositories."""

import configparser
import importlib.machinery
import importlib.util
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest
import unittest.mock
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CREW = ROOT / "crew" / "bin" / "crew"
COMMIT_IDENTITY = "Crew Maintainer <123456+maintainer@users.noreply.github.com>"


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


def crew_path(path):
    return str(Path(path).resolve())


def first_command_word(text):
    text = text.lstrip()
    if text[:1] in ("'", '"'):
        quote = text[0]
        end = text.find(quote, 1)
        return text[1:end] if end != -1 else text[1:]
    return text.split(None, 1)[0] if text else ""


def permission_path(path):
    return crew_path(path).replace(os.sep, "/")


def python_word():
    return "python" if os.name == "nt" else "python3"


def git_dir_for(tree, flag):
    result = subprocess.run(["git", "-C", str(tree), "rev-parse", flag], capture_output=True,
                            text=True, check=True)
    path = Path(result.stdout.strip())
    if not path.is_absolute():
        path = Path(tree) / path
    return Path(crew_path(path))


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


def commit_fixture_repo(repo):
    subprocess.run(["git", "config", "user.name", "Crew Maintainer"], cwd=str(repo), check=True)
    subprocess.run(["git", "config", "user.email", "123456+maintainer@users.noreply.github.com"],
                   cwd=str(repo), check=True)
    subprocess.run(["git", "add", "crew"], cwd=str(repo), check=True)
    subprocess.run(["git", "commit", "-q", "-m", "fixture"], cwd=str(repo), check=True)


def add_linked_worktree(test, repo):
    tree = Path(test.temp.name) / "worker-tree"
    subprocess.run(["git", "worktree", "add", "-q", str(tree)], cwd=str(repo), check=True)
    return tree


def write_run_fixture(test, engine="devin", model="role-model", role_name="tools"):
    repo = write_fixture_repo(test.temp.name, {role_name: role_file(name=role_name, engine=engine, model=model)})
    init_git_repo(repo)
    commit_fixture_repo(repo)
    fake = write_fake_engine(test.temp.name)
    command = shell_join([sys.executable, fake])
    config = Path(test.temp.name) / "devin-config.json"
    config.write_text(json.dumps({"user_setting": {"kept": True}, "permissions": {"allow": ["old"], "deny": ["old"]}}),
                      encoding="utf-8")
    (repo / "crew.local.ini").write_text(
        "[crew]\ncommit_identity = {2}\n\n[engine.devin]\ncommand = {0}\nconfig = {1}\n\n"
        "[engine.claude]\ncommand = {0}\n\n[worker]\npass_env = FAKE_ENGINE_RECORD,FAKE_ENGINE_EXIT\n".format(
            command, config, COMMIT_IDENTITY),
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
        with open(str(shim), "w", encoding="utf-8", newline="") as handle:
            handle.write('@echo off\r\n"{0}" "{1}" %*\r\n'.format(sys.executable, program))
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
elif argv[:2] == ["pr", "list"]:
    repo = argv[argv.index("--repo") + 1] if "--repo" in argv else ""
    print(json.dumps(data.get("list", {}).get(repo, [])))
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
        return crew_path(result.stdout.strip())

    def test_default_state_dir_is_under_the_repo(self):
        self.assertEqual(self.state_path(), crew_path(self.repo / ".crew" / "state"))

    def test_relative_state_dir_in_the_file_is_under_the_repo(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = elsewhere/state\n", encoding="utf-8")

        self.assertEqual(self.state_path(), crew_path(self.repo / "elsewhere" / "state"))

    def test_environment_state_dir_overrides_the_file(self):
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = elsewhere/state\n", encoding="utf-8")
        override = Path(self.temp.name) / "env-state"

        self.assertEqual(self.state_path({"CREW_STATE_DIR": str(override)}), crew_path(override))

    def test_tilde_state_dir_expands_to_the_home(self):
        home = Path(self.temp.name) / "home"
        home.mkdir()
        (self.repo / "crew.local.ini").write_text("[crew]\nstate_dir = ~/crew-state\n", encoding="utf-8")
        env = {"HOME": str(home), "USERPROFILE": str(home)}

        self.assertEqual(self.state_path(env), crew_path(home / "crew-state"))

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
        self.assertIn("Your worktree:", header[1])
        self.assertIn(crew_path(repo), header[1])
        self.assertIn("State folder:", header[2])
        self.assertIn("run the crew launcher (`crew/bin/crew`) with Python", header[3])
        self.assertNotIn("Repository root:", result.stdout)

    def test_missing_role_is_an_error_naming_the_file(self):
        repo = write_fixture_repo(self.temp.name, {})

        result = run_crew(repo, "prompt", "missing")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(crew_path(repo / "crew" / "roles" / "missing.md"), result.stderr)

    def test_missing_rules_is_an_error_naming_the_file(self):
        repo = write_fixture_repo(self.temp.name, {"tools": role_file()}, rules=None)

        result = run_crew(repo, "prompt", "tools")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(crew_path(repo / "crew" / "RULES.md"), result.stderr)


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

        result = run_crew(self.repo, "status", env={"PATH": ""})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("tools", result.stdout)
        self.assertIn("tasks 2", result.stdout)
        self.assertIn("results 2", result.stdout)
        self.assertIn("002-b.md", result.stdout)
        self.assertIn("tools state first line", result.stdout)

    def test_status_lists_open_pull_requests_with_no_verdict_or_a_stale_verdict_head(self):
        (self.repo / "crew" / "REPOS.md").write_text(
            "| Repository | What it is |\n| --- | --- |\n"
            "| hexuria/buwiz-forms | The app. |\n| hexuria/nsh | The runtime. |\n",
            encoding="utf-8")
        bin_dir = Path(self.temp.name) / "bin"
        gh_data = write_fake_gh(bin_dir)
        gh_data.write_text(json.dumps({
            "list": {
                "hexuria/buwiz-forms": [
                    {"number": 7, "headRefOid": "head-seven"},
                    {"number": 8, "headRefOid": "head-eight"},
                ],
                "hexuria/nsh": [
                    {"number": 3, "headRefOid": "head-three"},
                    {"number": 4, "headRefOid": "head-four"},
                ],
            }
        }), encoding="utf-8")
        outbox = self.repo / ".crew" / "state" / "roles" / "reviewer" / "outbox"
        outbox.mkdir(parents=True, exist_ok=True)
        (outbox / "hexuria-buwiz-forms-8.verdict").write_text(
            "verdict: MERGE\nhead: head-eight\nengine: claude\nmodel: test\nnote: current\n",
            encoding="utf-8")
        (outbox / "hexuria-nsh-3.verdict").write_text(
            "verdict: MERGE\nhead: old-head\nengine: claude\nmodel: test\nnote: stale\n",
            encoding="utf-8")
        (outbox / "hexuria-nsh-4.verdict").write_text(
            "verdict: FIX FIRST\nhead: head-four\nengine: claude\nmodel: test\nnote: current\n",
            encoding="utf-8")

        result = run_crew(self.repo, "status",
                          env={"PATH": str(bin_dir), "FAKE_GH_DATA": str(gh_data)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("needs a review: hexuria/buwiz-forms#7 head head-seven (no verdict)", result.stdout)
        self.assertIn("needs a review: hexuria/nsh#3 head head-three (verdict head old-head)", result.stdout)
        self.assertNotIn("hexuria/buwiz-forms#8", result.stdout)
        self.assertNotIn("hexuria/nsh#4", result.stdout)


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

    def test_queue_key_with_a_note_is_accepted_as_the_waiting_list(self):
        gh_data = write_fake_gh(self.bin)
        gh_data.write_text(json.dumps({
            "view": {"7": {"headRefOid": "head1", "isDraft": False, "state": "OPEN"}},
            "checks": {"7": [{"name": "ci-ok", "state": "SUCCESS", "bucket": "pass", "link": "https://example.invalid"}]},
        }), encoding="utf-8")
        (self.repo / ".crew" / "state" / "queue.json").write_text(
            json.dumps({"queue": [7], "batch": None, "note": "waiting for the gate"}) + "\n",
            encoding="utf-8")

        result = self.watch({"FAKE_GH_DATA": str(gh_data)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("pull request #7 head1 ci-ok pass", result.stdout)

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
        self.assert_good("clone hexuria/buwiz-forms and owner/name and owner/repo")
        self.assert_good("clone hexuria/buwiz-forms-html")

    def test_repository_name_rule_does_not_flag_repository_paths_or_branch_names(self):
        self.assert_good("run crew/bin/crew then scripts/tests/test_crew.py and docs/CI.md")
        self.assert_good("branches fix/example feat/example ci/example test/example origin/main")

    def test_repository_name_rule_requires_a_github_shape_and_known_owner_context(self):
        self.assert_good("pass/fail")
        self.assert_good("Linux/Windows")
        self.assert_good("date 11/28")
        self.assert_good("and/or")
        self.assert_good("clone someone/private-repo")
        self.assert_bad("hexuria/buwiz-forms is public; someone/private-repo is not")
        self.assert_bad("unknown repository hexuria/private-repo")

    def test_repository_name_rule_reads_known_owners_from_the_checkouts_remotes(self):
        fixture = Path(self.temp.name) / "remote-owner-case"
        fixture.mkdir()
        repo = write_fixture_repo(str(fixture), {"tools": role_file()})
        init_git_repo(repo)
        subprocess.run(["git", "remote", "add", "origin", "https://github.com/remoteowner/public-code.git"],
                       cwd=str(repo), check=True)
        (repo / "crew" / "CHECK.md").write_text(
            "remoteowner/public-code is the checkout; remoteowner/private-repo is not\n", encoding="utf-8")

        result = run_crew(repo, "lint")

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("private repository name", result.stdout)


class SyncTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        roles = {
            "tools": role_file(name="tools", title="Tools worker", engine="devin", model="swe-test"),
            "docs": role_file(name="docs", title="Docs worker", engine="claude", model="claude-role"),
            "reviewer": role_file(name="reviewer", title="Reviews pull requests", engine="claude", model="",
                                  extra="tools: Read, Grep, Glob"),
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
            self.assertEqual(len(body), 6)
            self.assertEqual(body[5], "<!-- generated by crew sync -->")
            self.assertIn("crew/RULES.md", body[0])
            self.assertIn("crew/roles/tools.md", body[1])
            self.assertIn("run the crew launcher: `crew/bin/crew` with Python (`python3 crew/bin/crew …`; on Windows `python crew/bin/crew …`)", body[2])
            self.assertIn("state path", body[2])
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

        self.assertIn("tools: Read, Grep, Glob, Bash\n", reviewer_claude)
        self.assertNotIn("Bash(git fetch", reviewer_claude)
        self.assertNotIn("Bash(git push", reviewer_claude)
        self.assertNotIn("Bash(gh pr merge", reviewer_claude)
        self.assertIn("use Bash", reviewer_claude)
        self.assertIn("read-only git and gh commands", reviewer_claude)
        self.assertIn("write only your own outbox", reviewer_claude)
        self.assertIn("allowed-tools:\n  - read\n  - grep\n  - glob\n", reviewer_devin)
        self.assertIn("tools: Read, Grep, Glob\n", limited_claude)
        self.assertIn("allowed-tools:\n  - read\n  - grep\n  - glob\n", limited_devin)

    def test_sync_removes_only_marked_old_unprefixed_generated_files(self):
        agents = self.repo / ".claude" / "agents"
        agents.mkdir(parents=True)
        legacy = agents / "tools.md"
        legacy.write_text("old generated file\n<!-- generated by crew sync -->\n", encoding="utf-8")
        unmarked = agents / "docs.md"
        unmarked.write_text("a person's file\n", encoding="utf-8")

        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)

        self.assertFalse(legacy.exists())
        self.assertEqual(unmarked.read_text(encoding="utf-8"), "a person's file\n")
        self.assertTrue((agents / "crew-tools.md").exists())

    def test_sync_marks_generated_files_and_refuses_to_overwrite_an_unmarked_target(self):
        agents = self.repo / ".claude" / "agents"
        agents.mkdir(parents=True)
        target = agents / "crew-tools.md"
        target.write_text("a person's file\n", encoding="utf-8")

        result = run_crew(self.repo, "sync")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("crew-tools.md", result.stderr.replace("\\", "/"))
        self.assertIn("generated by crew sync", result.stderr)
        self.assertEqual(target.read_text(encoding="utf-8"), "a person's file\n")
        devin_generated = self.repo / ".devin" / "agents" / "crew-docs.md"
        self.assertTrue(devin_generated.exists())
        self.assertIn("<!-- generated by crew sync -->", devin_generated.read_text(encoding="utf-8"))

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
        self.assertEqual((state / "queue.json").read_text(encoding="utf-8"), '{"queue": [], "batch": null}\n')
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

    def test_state_push_refuses_a_non_noreply_commit_identity_setting(self):
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote), "crew-state")
        config = self.repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            COMMIT_IDENTITY, "Crew Maintainer <somebody@example.com>"), encoding="utf-8")
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)

        result = run_crew(self.repo, "state", "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("commit_identity", result.stderr)
        self.assertIn("users.noreply.github.com", result.stderr)
        log = subprocess.run(["git", "log", "--format=%s"], cwd=str(state), capture_output=True, text=True)
        self.assertEqual(log.stdout, "")

    def test_state_init_and_push_create_the_orphan_branch_with_the_checkouts_own_noreply_identity(self):
        state = Path(self.temp.name) / "shared-state"
        remote = Path(self.temp.name) / "shared-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote), "crew-state")

        init = run_crew(self.repo, "state", "init")
        self.assertEqual(init.returncode, 0, init.stderr)
        subprocess.run(["git", "config", "user.name", "Wrong Person"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "999999+wrong@users.noreply.github.com"],
                       cwd=str(state), check=True)
        (state / "roles" / "tools" / "outbox" / "001-result.md").write_text("done\n", encoding="utf-8")
        (state / "roles" / "tools" / "outbox" / "001-result.run.log").write_text("local only\n", encoding="utf-8")

        push = run_crew(self.repo, "state", "push")

        self.assertEqual(push.returncode, 0, push.stderr)
        branch = subprocess.run(["git", "--git-dir", str(remote), "rev-parse", "crew-state"],
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
            "[crew]\nstate_dir = {0}\nstate_remote = {1}\nstate_branch = {2}\ncommit_identity = {3}\n".format(
                state, remote, branch, COMMIT_IDENTITY),
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

    def test_state_push_checks_the_state_checkouts_real_origin_and_push_url_case_insensitively(self):
        init_git_repo(self.repo)
        code_url = "https://github.com/hexuria/buwiz-forms.git"
        subprocess.run(["git", "remote", "add", "origin", code_url], cwd=str(self.repo), check=True)
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote), "crew-state")
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        subprocess.run(["git", "remote", "set-url", "--push", "origin", "https://github.com/HEXURIA/BUWIZ-FORMS.git"],
                       cwd=str(state), check=True)

        result = run_crew(self.repo, "state", "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("state_remote", result.stderr)

    def test_state_remote_ssh_url_with_a_port_is_the_same_repository_as_scp_style(self):
        init_git_repo(self.repo)
        subprocess.run(["git", "remote", "add", "origin", "ssh://git@example.com:443/owner/code.git"],
                       cwd=str(self.repo), check=True)
        state = Path(self.temp.name) / "state-checkout"
        self.write_state_config(state, "git@example.com:owner/code.git", "crew-state")

        result = run_crew(self.repo, "state", "init")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("state_remote", result.stderr)
        self.assertFalse(state.exists())

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

    def test_state_push_appends_a_log_line_only_when_the_conductors_state_changed(self):
        state = Path(self.temp.name) / "state-checkout"
        remote = Path(self.temp.name) / "state-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        self.write_state_config(state, str(remote), "crew-state")
        self.assertEqual(run_crew(self.repo, "state", "init").returncode, 0)
        conductor_state = state / "roles" / "conductor" / "STATE.md"
        conductor_state.parent.mkdir(parents=True, exist_ok=True)
        conductor_state.write_text("first\n", encoding="utf-8")

        first = run_crew(self.repo, "state", "push")

        self.assertEqual(first.returncode, 0, first.stderr)
        log = (state / "LOG.md").read_text(encoding="utf-8")
        self.assertEqual(log.count("conductor: STATE.md changed (crew state push)"), 1, log)

        unchanged = run_crew(self.repo, "state", "push")

        self.assertEqual(unchanged.returncode, 0, unchanged.stderr)
        log = (state / "LOG.md").read_text(encoding="utf-8")
        self.assertEqual(log.count("conductor: STATE.md changed (crew state push)"), 1, log)
        conductor_state.write_text("second\n", encoding="utf-8")

        second = run_crew(self.repo, "state", "push")

        self.assertEqual(second.returncode, 0, second.stderr)
        log = (state / "LOG.md").read_text(encoding="utf-8")
        self.assertEqual(log.count("conductor: STATE.md changed (crew state push)"), 2, log)
        remote_log = subprocess.run(["git", "--git-dir", str(remote), "show", "crew-state:LOG.md"],
                                    capture_output=True, text=True, check=True).stdout
        self.assertIn("conductor: STATE.md changed (crew state push)", remote_log)

    def visibility_gh_path(self, label, answer):
        bin_dir = Path(self.temp.name) / ("bin-" + label)
        if answer is None:
            source = "import sys\nsys.exit(1)\n"
        else:
            source = "print({0!r})\n".format(answer)
        write_path_python(bin_dir, "gh", source)
        return str(bin_dir) + os.pathsep + os.environ.get("PATH", "")

    def state_push_with_visibility(self, label, answer):
        state = Path(self.temp.name) / ("state-" + label)
        remote = Path(self.temp.name) / ("remote-" + label + ".git")
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        state.mkdir()
        subprocess.run(["git", "init", "-q"], cwd=str(state), check=True)
        subprocess.run(["git", "remote", "add", "origin", str(remote)], cwd=str(state), check=True)
        self.write_state_config(state, "https://github.com/hexuria/crew-state.git", "crew-state")
        result_file = state / "roles" / "tools" / "outbox" / "001-result.md"
        result_file.parent.mkdir(parents=True, exist_ok=True)
        result_file.write_text("token ghp_example\n", encoding="utf-8")
        env = {"PATH": self.visibility_gh_path(label, answer)}
        return run_crew(self.repo, "state", "push", env=env)

    def test_state_push_skips_the_privacy_check_only_for_a_private_github_state_remote(self):
        private = self.state_push_with_visibility("private", "private")

        self.assertEqual(private.returncode, 0, private.stderr)
        self.assertIn("state remote is private: public-safety check skipped", private.stdout)

        for label, answer in (("public", "public"), ("unknown", None)):
            with self.subTest(answer=answer):
                result = self.state_push_with_visibility(label, answer)

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("secret", result.stderr)
                self.assertNotIn("public-safety check skipped", result.stdout)


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
        self.assertIn(crew_path(self.repo / ".crew" / "state"), result.stdout)
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

    def test_doctor_prints_the_not_a_gate_note_and_no_merge_or_tag_lines(self):
        write_versioned_cli(self.bin, "claude", "1.0")
        (self.repo / "crew.local.ini").write_text("[crew]\nmerge = yes\ntag = no\n", encoding="utf-8")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        note = "a note for people and dashboards, not a gate: GitHub's own permissions and branch protection decide who can merge"
        self.assertIn(note, " ".join(result.stdout.split()))
        self.assertNotIn("merge: yes", result.stdout)
        self.assertNotIn("tag: no", result.stdout)

    def test_doctor_states_the_environment_scrub_truth(self):
        write_versioned_cli(self.bin, "claude", "1.0")

        result = self.doctor()

        self.assertEqual(result.returncode, 0, result.stderr)
        truth = ("the scrub removes tokens from the ENVIRONMENT only; credentials git and gh keep under "
                 "the home folder remain reachable to a worker; the fence is a restricted token, "
                 "branch protection or the sandbox")
        self.assertIn(truth, " ".join(result.stdout.split()))

    def linked_tree(self):
        init_git_repo(self.repo)
        commit_fixture_repo(self.repo)
        return add_linked_worktree(self, self.repo)

    def test_doctor_prints_the_worktree_settings_file_it_used(self):
        write_versioned_cli(self.bin, "claude", "1.0")
        tree = self.linked_tree()
        (self.repo / "crew.local.ini").write_text("[crew]\nengine = claude\n", encoding="utf-8")
        (tree / "crew.local.ini").write_text("[crew]\nengine = claude\n", encoding="utf-8")

        result = run_crew(tree, "doctor", env={"PATH": str(self.bin)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("crew.local.ini: {0}".format(crew_path(tree / "crew.local.ini")), result.stdout)

    def test_doctor_prints_the_main_worktree_settings_file_it_used(self):
        write_versioned_cli(self.bin, "claude", "1.0")
        tree = self.linked_tree()
        (self.repo / "crew.local.ini").write_text("[crew]\nengine = claude\n", encoding="utf-8")

        result = run_crew(tree, "doctor", env={"PATH": str(self.bin)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("crew.local.ini: {0}".format(crew_path(self.repo / "crew.local.ini")), result.stdout)

    def test_doctor_prints_where_it_looked_when_no_settings_file_is_found(self):
        write_versioned_cli(self.bin, "claude", "1.0")
        tree = self.linked_tree()

        result = run_crew(tree, "doctor", env={"PATH": str(self.bin)})

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("crew.local.ini: none found, looked in: {0}, {1}".format(
            crew_path(tree / "crew.local.ini"), crew_path(self.repo / "crew.local.ini")), result.stdout)


class RunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_run_without_worktree_refuses_before_starting_the_engine(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")

        result = run_crew(repo, "run", "tools", str(task), env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("--worktree", result.stderr)
        self.assertFalse(record_path.exists())

    def test_run_with_the_conductors_own_checkout_as_worktree_refuses(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(repo),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("conductor", result.stderr)
        self.assertFalse(record_path.exists())

    def test_run_with_a_subfolder_of_the_conductors_checkout_refuses(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        subfolder = repo / "crates"
        subfolder.mkdir()

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(subfolder),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("top level", result.stderr)
        self.assertFalse(record_path.exists())

    def test_run_with_an_unrelated_git_checkout_refuses(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        other = Path(self.temp.name) / "other-checkout"
        other.mkdir()
        init_git_repo(other)

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(other),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("same repository", result.stderr)
        self.assertFalse(record_path.exists())

    def test_devin_run_logs_exit_passes_model_worktree_and_removes_temp_files(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        tree_path = add_linked_worktree(self, repo)
        env = {"FAKE_ENGINE_RECORD": str(record_path), "FAKE_ENGINE_EXIT": "7"}

        result = run_crew(repo, "run", "tools", str(task), "--model", "override-model",
                          "--worktree", str(tree_path), env=env)

        self.assertEqual(result.returncode, 7, result.stderr)
        log = repo / ".crew" / "state" / "roles" / "tools" / "outbox" / "001-sample.run.log"
        self.assertTrue(log.exists())
        self.assertIn("fake engine ran", log.read_text(encoding="utf-8"))
        self.assertEqual(log.read_text(encoding="utf-8").splitlines()[-1], "exit 7")
        record = json.loads(record_path.read_text(encoding="utf-8"))
        argv = record["argv"]
        self.assertEqual(record["model"], "override-model")
        self.assertEqual(crew_path(record["cwd"]), crew_path(tree_path))
        self.assertEqual(record["git_lfs_skip_push"], "1")
        for flag in ("-p", "--config", "--permission-mode", "--respect-workspace-trust", "--prompt-file"):
            self.assertIn(flag, argv)
        self.assertEqual(argv[argv.index("--permission-mode") + 1], "accept-edits")
        self.assertEqual(argv[argv.index("--respect-workspace-trust") + 1], "false")
        self.assertFalse(Path(record["config_path"]).exists())
        self.assertFalse(Path(record["prompt_file"]).exists())
        permissions = record["config"]["permissions"]
        tree = permission_path(tree_path)
        role_dir = permission_path(repo / ".crew" / "state" / "roles" / "tools")
        common = permission_path(git_dir_for(tree_path, "--git-common-dir"))
        admin = permission_path(git_dir_for(tree_path, "--git-dir"))
        for entry in ("Write({0})".format(tree), "Write({0})".format(role_dir),
                      "Exec(git status)", "Exec(git diff)", "Exec(git log)", "Exec(git show)",
                      "Exec(git commit)", "Exec(gh pr view)", "Exec(gh issue create)"):
            self.assertIn(entry, permissions["allow"])
        for python in ("python3", "python"):
            for suffix in ("-m unittest", "scripts/tests/", "crew/bin/crew lint",
                           "crew/bin/crew sync --check", "crew/bin/crew push"):
                self.assertIn("Exec({0} {1})".format(python, suffix), permissions["allow"])
        self.assertFalse(any(entry.startswith("Exec(sed") for entry in permissions["allow"]))
        self.assertNotIn("Exec(find)", permissions["allow"])
        self.assertFalse(any(entry.startswith("Write({0}".format(common)) for entry in permissions["allow"]))
        self.assertNotIn("Exec(git)", permissions["allow"])
        self.assertNotIn("Exec({0})".format(python_word()), permissions["allow"])
        self.assertNotIn("Exec(git fetch)", permissions["allow"])
        self.assertNotIn("Exec(git rebase)", permissions["allow"])
        path_denies = (
            "Write({0}/.git)".format(tree), "Write({0}/.git/**)".format(tree),
            "Write({0}/**/.git)".format(tree), "Write({0}/**/.git/**)".format(tree),
            "Write({0})".format(admin), "Write({0}/**)".format(admin),
            "Write({0})".format(common), "Write({0}/**)".format(common),
            "Write({0}/crew/bin)".format(tree), "Write({0}/crew/bin/**)".format(tree),
        )
        for denied in path_denies + (
                "old", "Exec(git push)", "Exec(gh pr merge)", "Exec(gh pr close)",
                "Exec(gh run cancel)", "Exec(gh release)", "Exec(cargo)", "Exec(nsh)",
                "Exec(git tag)", "Exec(git fetch)", "Exec(git rebase)",
                "Exec(git * --output*)", "Exec(git * -o *)", "Exec(git format-patch*)",
                "Exec(git archive*)", "Exec(git bundle*)", "Exec(git config*)", "Exec(git worktree*)"):
            self.assertIn(denied, permissions["deny"])
        self.assertEqual(record["config"]["user_setting"], {"kept": True})
        self.assertNotIn("old", permissions["allow"])
        self.assertIn("TASK FOR WORKER", record["prompt"])

    def test_claude_run_uses_stdin_model_permission_mode_and_allowed_tools(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="claude", model="claude-role-model")
        tree_path = add_linked_worktree(self, repo)
        env = {"FAKE_ENGINE_RECORD": str(record_path)}

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(tree_path), env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        argv = record["argv"]
        self.assertEqual(argv[0], "-p")
        self.assertEqual(record["model"], "claude-role-model")
        self.assertEqual(argv[argv.index("--permission-mode") + 1], "acceptEdits")
        tools = argv[argv.index("--allowedTools") + 1]
        tree = permission_path(tree_path)
        role_dir_path = crew_path(repo / ".crew" / "state" / "roles" / "tools")
        role_dir = permission_path(role_dir_path)
        def path_forms(path):
            normal = permission_path(path)
            alternate = ("/" + normal) if normal.startswith("/") else ("//" + normal)
            return (normal, alternate)

        for form in path_forms(tree_path):
            self.assertIn("Edit({0}/**)".format(form), tools)
            self.assertIn("Write({0}/**)".format(form), tools)
        for form in path_forms(role_dir_path):
            self.assertIn("Edit({0}/**)".format(form), tools)
            self.assertIn("Write({0}/**)".format(form), tools)
        for name in ("status", "diff", "log", "show", "add", "commit", "switch", "checkout", "merge",
                     "stash", "rev-parse", "ls-files", "branch", "restore"):
            self.assertIn("Bash(git {0}:*)".format(name), tools)
        self.assertIn("Bash(git fetch origin)", tools)
        for python in ("python3", "python"):
            for suffix in ("-m unittest:*", "scripts/tests/:*", "crew/bin/crew lint",
                           "crew/bin/crew sync --check", "crew/bin/crew push:*"):
                self.assertIn("Bash({0} {1})".format(python, suffix), tools)
        for entry in ("Bash(gh pr view:*)", "Bash(gh pr list:*)", "Bash(gh pr checks:*)",
                      "Bash(gh pr diff:*)", "Bash(gh pr create:*)", "Bash(gh run view:*)",
                      "Bash(gh run list:*)", "Bash(gh issue list:*)", "Bash(gh issue view:*)",
                      "Bash(gh issue create:*)"):
            self.assertIn(entry, tools)
        self.assertNotIn("Bash(git:*)", tools)
        self.assertNotIn("Bash(python3:*)", tools)
        self.assertNotIn("Bash(python:*)", tools)
        self.assertNotIn("Bash(git rebase:*)", tools)
        self.assertNotIn("Bash(git fetch:*)", tools)
        disallowed = argv[argv.index("--disallowedTools") + 1]
        common = permission_path(git_dir_for(tree_path, "--git-common-dir"))
        admin = permission_path(git_dir_for(tree_path, "--git-dir"))
        for entry in ("Edit(crew/bin/**)", "Write(crew/bin/**)", "Edit(.git/**)", "Write(.git/**)",
                      "Edit(**/.git/**)", "Write(**/.git/**)",
                      "Edit({0}/crew/bin/**)".format(tree), "Write({0}/crew/bin/**)".format(tree),
                      "Edit({0})".format(admin), "Write({0}/**)".format(admin),
                      "Edit({0})".format(common), "Write({0}/**)".format(common),
                      "Bash(git push:*)", "Bash(gh pr merge:*)", "Bash(gh pr close:*)",
                      "Bash(gh run cancel:*)", "Bash(gh release:*)", "Bash(git tag:*)",
                      "Bash(cargo:*)", "Bash(nsh:*)",
                      "Bash(git * --output*)", "Bash(git * -o *)", "Bash(git format-patch*)",
                      "Bash(git archive*)", "Bash(git bundle*)", "Bash(git config*)", "Bash(git worktree*)"):
            self.assertIn(entry, disallowed)
        for form in path_forms(tree_path):
            self.assertIn("Edit({0}/crew/bin/**)".format(form), disallowed)
            self.assertIn("Write({0}/crew/bin/**)".format(form), disallowed)
        for base in (admin, common):
            self.assertIn("Edit({0})".format(base), disallowed)
            alternate = ("/" + base) if base.startswith("/") else ("//" + base)
            self.assertIn("Edit({0})".format(alternate), disallowed)
            self.assertIn("Write({0}/**)".format(alternate), disallowed)
        self.assertIn("--setting-sources=", argv)
        self.assertEqual(crew_path(argv[argv.index("--add-dir") + 1]), crew_path(role_dir_path))
        self.assertIsNone(record["config_path"])
        self.assertIn("TASK FOR WORKER", record["stdin"])
        self.assertIn("Role body line.", record["stdin"])
        log = repo / ".crew" / "state" / "roles" / "tools" / "outbox" / "001-sample.run.log"
        self.assertEqual(log.read_text(encoding="utf-8").splitlines()[-1], "exit 0")

    def test_worker_prompt_names_the_worktree_and_not_the_conductors_root(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        outside_state = Path(self.temp.name) / "shared-state"
        config = repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            "commit_identity = " + COMMIT_IDENTITY,
            "commit_identity = {0}\nstate_dir = {1}".format(COMMIT_IDENTITY, outside_state)),
            encoding="utf-8")
        tree_path = add_linked_worktree(self, repo)

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(tree_path),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        prompt = record["prompt"]
        self.assertIn("Your worktree: {0}".format(crew_path(tree_path)), prompt)
        self.assertIn("State folder: {0}".format(outside_state / "roles" / "tools"), prompt)
        self.assertNotIn("Repository root:", prompt)
        self.assertNotIn(crew_path(repo), prompt)

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
            "CREW_ENGINE": "devin",
            "SYSTEMROOT": "C:\\Windows",
            "COMSPEC": "C:\\Windows\\System32\\cmd.exe",
            "PATHEXT": ".COM;.EXE;.BAT",
            "TEMP": str(repo / "tmp"),
            "TMP": str(repo / "tmp"),
            "LOCALAPPDATA": str(repo / "local-app-data"),
            "APPDATA": str(repo / "app-data"),
            "USERPROFILE": str(repo / "profile"),
            "HOMEDRIVE": "C:",
            "HOMEPATH": "\\Users\\worker",
            "SSH_AUTH_SOCK": str(repo / "ssh-agent.sock"),
        }

        tree_path = add_linked_worktree(self, repo)
        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(tree_path), env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        worker_env = record["env"]
        self.assertEqual(worker_env.get("GIT_LFS_SKIP_PUSH"), "1")
        self.assertEqual(worker_env.get("LC_CREW_TEST"), "passed-locale")
        self.assertEqual(crew_path(worker_env.get("CLAUDE_CONFIG_DIR")), crew_path(repo / "claude-config"))
        self.assertEqual(worker_env.get("GH_TOKEN"), "restricted-worker-token")
        self.assertEqual(worker_env.get("GIT_AUTHOR_NAME"), "Crew Maintainer")
        self.assertEqual(worker_env.get("GIT_AUTHOR_EMAIL"), "123456+maintainer@users.noreply.github.com")
        self.assertEqual(worker_env.get("GIT_COMMITTER_NAME"), "Crew Maintainer")
        self.assertEqual(worker_env.get("GIT_COMMITTER_EMAIL"), "123456+maintainer@users.noreply.github.com")
        self.assertEqual(worker_env.get("CREW_COMMIT_IDENTITY"), COMMIT_IDENTITY)
        self.assertEqual(crew_path(worker_env.get("CREW_STATE_DIR")), crew_path(repo / ".crew" / "state"))
        self.assertEqual(worker_env.get("CREW_ENGINE"), "devin")
        for name in ("SYSTEMROOT", "COMSPEC", "PATHEXT", "TEMP", "TMP", "LOCALAPPDATA", "APPDATA",
                     "USERPROFILE", "HOMEDRIVE", "HOMEPATH"):
            self.assertIn(name, worker_env)
        self.assertNotIn("SSH_AUTH_SOCK", worker_env)
        self.assertIn("PATH", worker_env)
        for name in ("RANDOM_CREW_VAR", "SECRET_THING", "EXAMPLE_API_KEY"):
            self.assertNotIn(name, worker_env)
        self.assertNotEqual(worker_env.get("GH_TOKEN"), "host-token-not-passed")

    def test_ssh_auth_sock_is_passed_only_when_pass_env_names_it(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        config = repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            "pass_env = FAKE_ENGINE_RECORD,FAKE_ENGINE_EXIT",
            "pass_env = FAKE_ENGINE_RECORD,FAKE_ENGINE_EXIT,SSH_AUTH_SOCK"), encoding="utf-8")
        tree_path = add_linked_worktree(self, repo)
        sock = str(repo / "ssh-agent.sock")

        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(tree_path),
                          env={"FAKE_ENGINE_RECORD": str(record_path), "SSH_AUTH_SOCK": sock})

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertEqual(crew_path(record["env"].get("SSH_AUTH_SOCK")), crew_path(sock))

    def run_with_identity_line(self, identity_line):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        tree_path = add_linked_worktree(self, repo)
        config = repo / "crew.local.ini"
        text = config.read_text(encoding="utf-8")
        text = text.replace("commit_identity = " + COMMIT_IDENTITY, identity_line)
        config.write_text(text, encoding="utf-8")
        result = run_crew(repo, "run", "tools", str(task), "--worktree", str(tree_path),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})
        return result, record_path

    def test_run_refuses_a_missing_commit_identity(self):
        result, record_path = self.run_with_identity_line("")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("commit_identity", result.stderr)
        self.assertFalse(record_path.exists())

    def test_run_refuses_a_non_noreply_commit_identity(self):
        result, record_path = self.run_with_identity_line(
            "commit_identity = Crew Maintainer <somebody@example.com>")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("commit_identity", result.stderr)
        self.assertFalse(record_path.exists())

    @unittest.skipIf(os.name == "nt", "POSIX signal delivery test")
    def test_sigterm_removes_the_devin_temp_config_and_prompt(self):
        repo, task, record_path, _config = write_run_fixture(self, engine="devin", model="role-model")
        config = repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            "FAKE_ENGINE_EXIT", "FAKE_ENGINE_EXIT,FAKE_ENGINE_SLEEP"), encoding="utf-8")
        env = os.environ.copy()
        env.update({"FAKE_ENGINE_RECORD": str(record_path), "FAKE_ENGINE_SLEEP": "30"})
        tree_path = add_linked_worktree(self, repo)
        process = subprocess.Popen([sys.executable, str(repo / "crew" / "bin" / "crew"), "run", "tools",
                                    str(task), "--worktree", str(tree_path)], cwd=str(repo), env=env,
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


class ReadOnlyConductorProfileTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def fixture(self, engine):
        return write_run_fixture(self, engine=engine, model="role-model", role_name="conductor")

    def assert_devin_read_only_profile(self, permissions, tree_path, repo):
        tree = permission_path(tree_path)
        state = permission_path(repo / ".crew" / "state")
        outbox = permission_path(repo / ".crew" / "state" / "roles" / "conductor" / "outbox")
        for python in ("python3", "python"):
            for suffix in ("doctor", "status", "state path", "lint", "sync --check"):
                self.assertIn("Exec({0} crew/bin/crew {1})".format(python, suffix), permissions["allow"])
        for verb in ("log", "show", "diff", "status", "ls-remote", "rev-parse", "merge-tree"):
            self.assertIn("Exec(git {0})".format(verb), permissions["allow"])
            self.assertIn("Exec(git -C {0} {1})".format(tree, verb), permissions["allow"])
            self.assertIn("Exec(git -C {0} {1})".format(state, verb), permissions["allow"])
        for entry in ("Exec(gh pr list)", "Exec(gh pr view)", "Exec(gh pr checks)", "Exec(gh pr diff)",
                      "Exec(gh run list)", "Exec(gh run view)", "Exec(gh release view)",
                      "Exec(gh issue view)", "Exec(gh api)"):
            self.assertIn(entry, permissions["allow"])
        self.assertIn("Write({0})".format(outbox), permissions["allow"])
        self.assertNotIn("Write({0})".format(tree), permissions["allow"])
        for absent in ("Exec(gh pr merge)", "Exec(gh pr create)", "Exec(gh issue create)",
                       "Exec(gh api -X)", "Exec(git push)"):
            self.assertNotIn(absent, permissions["allow"])
        for denied in ("Exec(gh pr merge)", "Exec(git push)", "Exec(gh api * -X*)",
                       "Exec(gh api * -f*)", "Exec(gh api * -F*)", "Exec(gh api * --input*)"):
            self.assertIn(denied, permissions["deny"])

    def test_devin_run_conductor_uses_the_read_only_profile(self):
        repo, task, record_path, _config = self.fixture("devin")
        tree_path = add_linked_worktree(self, repo)

        result = run_crew(repo, "run", "conductor", str(task), "--worktree", str(tree_path),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assert_devin_read_only_profile(record["config"]["permissions"], tree_path, repo)
        argv = record["argv"]
        if "--permission-mode" in argv:
            self.assertNotEqual(argv[argv.index("--permission-mode") + 1], "accept-edits")

    def test_claude_run_conductor_uses_the_read_only_profile(self):
        repo, task, record_path, _config = self.fixture("claude")
        tree_path = add_linked_worktree(self, repo)

        result = run_crew(repo, "run", "conductor", str(task), "--worktree", str(tree_path),
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        argv = record["argv"]
        tools = argv[argv.index("--allowedTools") + 1]
        disallowed = argv[argv.index("--disallowedTools") + 1]
        tree = permission_path(tree_path)
        state = permission_path(repo / ".crew" / "state")
        outbox_path = repo / ".crew" / "state" / "roles" / "conductor" / "outbox"
        outbox = permission_path(outbox_path)
        alternate_outbox = ("/" + outbox) if outbox.startswith("/") else ("//" + outbox)
        for python in ("python3", "python"):
            for suffix in ("doctor", "status", "state path", "lint", "sync --check"):
                self.assertIn("Bash({0} crew/bin/crew {1})".format(python, suffix), tools)
        for verb in ("log", "show", "diff", "status", "ls-remote", "rev-parse", "merge-tree"):
            self.assertIn("Bash(git {0}:*)".format(verb), tools)
            self.assertIn("Bash(git -C {0} {1}:*)".format(tree, verb), tools)
            self.assertIn("Bash(git -C {0} {1}:*)".format(state, verb), tools)
        for entry in ("Bash(gh pr list:*)", "Bash(gh pr view:*)", "Bash(gh pr checks:*)",
                      "Bash(gh pr diff:*)", "Bash(gh run list:*)", "Bash(gh run view:*)",
                      "Bash(gh release view:*)", "Bash(gh issue view:*)", "Bash(gh api:*)"):
            self.assertIn(entry, tools)
        for form in (outbox, alternate_outbox):
            self.assertIn("Edit({0}/**)".format(form), tools)
            self.assertIn("Write({0}/**)".format(form), tools)
        self.assertNotIn("Edit({0}/**)".format(tree), tools)
        self.assertNotIn("Write({0}/**)".format(tree), tools)
        for absent in ("Bash(gh pr merge:*)", "Bash(gh pr create:*)", "Bash(gh issue create:*)",
                       "Bash(gh api -X:*)", "Bash(git push:*)"):
            self.assertNotIn(absent, tools)
        for denied in ("Bash(gh pr merge:*)", "Bash(git push:*)", "Bash(gh api * -X*)",
                       "Bash(gh api * -f*)", "Bash(gh api * -F*)", "Bash(gh api * --input*)"):
            self.assertIn(denied, disallowed)
        if "--permission-mode" in argv:
            self.assertNotEqual(argv[argv.index("--permission-mode") + 1], "acceptEdits")

    def test_devin_start_read_only_uses_the_read_only_profile(self):
        repo, _task, record_path, _config = self.fixture("devin")

        result = run_crew(repo, "start", "--read-only", "--engine", "devin",
                          env={"FAKE_ENGINE_RECORD": str(record_path)})

        self.assertEqual(result.returncode, 0, result.stderr)
        record = json.loads(record_path.read_text(encoding="utf-8"))
        self.assertIsNotNone(record["config_path"])
        self.assert_devin_read_only_profile(record["config"]["permissions"], repo, repo)
        self.assertFalse(Path(record["config_path"]).exists())
        argv = record["argv"]
        if "--permission-mode" in argv:
            self.assertNotEqual(argv[argv.index("--permission-mode") + 1], "accept-edits")

    def test_claude_start_print_read_only_carries_the_permission_lists(self):
        repo, _task, _record_path, _config = self.fixture("claude")

        result = run_crew(repo, "start", "--read-only", "--engine", "claude", "--print")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--allowedTools", result.stdout)
        self.assertIn("--disallowedTools", result.stdout)
        self.assertIn("Bash(gh api:*)", result.stdout)
        self.assertIn("Bash(gh pr merge:*)", result.stdout)
        self.assertNotIn("acceptEdits", result.stdout)


class RemovedMergeCommandTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"

    def test_automerge_command_is_gone(self):
        result = run_crew(self.repo, "automerge", "7", "abc123")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid choice", result.stderr)
        self.assertIn("automerge", result.stderr)

    def test_batchwatch_no_merge_option_is_gone(self):
        write_path_python(self.bin, "just", "print('batch: green')\n")

        result = run_crew(self.repo, "batchwatch", "test/linux-windows/batch-7", "7", "--no-merge",
                          env={"PATH": str(self.bin)})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unrecognized arguments", result.stderr)
        self.assertIn("--no-merge", result.stderr)

    def test_verdict_help_says_the_note_is_not_a_gate(self):
        result = run_crew(self.repo, "verdict", "--help")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("a note for people and dashboards, not a gate: GitHub's own permissions and "
                      "branch protection decide who can merge", result.stdout)


class BatchwatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"
        (self.repo / "crew.local.ini").write_text("[crew]\nmerge = yes\n", encoding="utf-8")

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

    def write_gh(self):
        record = Path(self.temp.name) / "gh-record.jsonl"
        source = r'''
import json
import os
import sys
with open(os.environ["GH_RECORD"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(sys.argv[1:]) + "\n")
'''
        write_path_python(self.bin, "gh", source)
        return record

    def batchwatch(self, just_record, prs=("7",), gh_record=None, *extra):
        env = {"PATH": str(self.bin), "JUST_RECORD": str(just_record)}
        if gh_record is not None:
            env["GH_RECORD"] = str(gh_record)
        return run_crew(self.repo, "batchwatch", "test/linux-windows/batch-7", *prs, *extra, env=env)

    def just_calls(self, record, name):
        if not record.exists():
            return []
        return [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()
                if json.loads(line) and json.loads(line)[0] == name]

    def test_green_batch_prints_the_merge_commands_and_stops(self):
        merge_lines = [
            "gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123",
            "gh pr merge 8 --repo owner/repo --squash --match-head-commit def456",
        ]
        just_record = self.write_just(["batch: green"], merge_lines=merge_lines)
        gh_record = self.write_gh()

        result = self.batchwatch(just_record, ("7", "8"), gh_record)

        self.assertEqual(result.returncode, 0, result.stderr + result.stdout)
        self.assertIn("batch: green", result.stdout)
        for line in merge_lines:
            self.assertIn(line, result.stdout)
        self.assertTrue(self.just_calls(just_record, "ci-batch-merge"))
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))
        self.assertFalse(gh_record.exists())

    def test_green_batch_returns_the_merge_command_printers_failure(self):
        just_record = self.write_just(["batch: green"], merge_exit=5)
        gh_record = self.write_gh()

        result = self.batchwatch(just_record, gh_record=gh_record)

        self.assertEqual(result.returncode, 5)
        self.assertFalse(gh_record.exists())
        self.assertFalse(self.just_calls(just_record, "ci-batch-verify"))

    def test_red_batch_prints_failed_jobs_and_never_asks_for_merge_commands(self):
        just_record = self.write_just(["batch: red", "    failed job: Linux tests (failure)  https://example.invalid"])
        gh_record = self.write_gh()

        result = self.batchwatch(just_record, gh_record=gh_record)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed job: Linux tests", result.stdout)
        self.assertFalse(self.just_calls(just_record, "ci-batch-merge"))
        self.assertFalse(gh_record.exists())


class SandboxRunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file(engine="devin", model="sand-model")})
        init_git_repo(self.repo)
        commit_fixture_repo(self.repo)
        self.tree = add_linked_worktree(self, self.repo)
        (self.repo / "crew.local.ini").write_text(
            "[crew]\ncommit_identity = {0}\n\n[worker]\npass_env = SBX_RECORD,GIT_RECORD\n".format(COMMIT_IDENTITY),
            encoding="utf-8")
        self.task = self.repo / "001-sample.md"
        self.task.write_text("TASK\n", encoding="utf-8")

    def validation_git_env(self, bin_dir):
        git_source = """
import os
import sys
argv = sys.argv[1:]
if 'rev-parse' in argv:
    names = {'--git-common-dir': 'TEST_COMMON_DIR', '--git-dir': 'TEST_ADMIN_DIR',
             '--show-toplevel': 'TEST_TOPLEVEL'}
    print(os.environ.get(names.get(argv[-1], 'TEST_ADMIN_DIR'), ''))
"""
        git_shim = write_path_python(bin_dir, "git", git_source)
        env = {"PATH": str(bin_dir),
               "TEST_COMMON_DIR": str(git_dir_for(self.tree, "--git-common-dir")),
               "TEST_ADMIN_DIR": str(git_dir_for(self.tree, "--git-dir")),
               "TEST_TOPLEVEL": str(self.tree)}
        return env, git_shim

    def test_sandbox_print_shows_the_four_documented_commands_and_runs_nothing(self):
        env, git_shim = self.validation_git_env(Path(self.temp.name) / "bin-print")
        result = run_crew(self.repo, "run", "tools", str(self.task), "--worktree", str(self.tree),
                          "--sandbox", "--print", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(len(lines), 4, result.stdout)
        tree = crew_path(self.tree)
        prompt = crew_path(self.tree / ".crew" / "state" / "prompts" / "001-sample.md")
        name = "crew-tools-001-sample"
        self.assertEqual(lines[0], "sbx create --name {0} --clone devin {1}".format(name, tree))
        self.assertEqual(lines[1], "sbx exec {0} -- devin -p --model sand-model --prompt-file {1}".format(
            name, prompt))
        actual_git = first_command_word(lines[2])
        self.assertEqual(os.path.normcase(crew_path(actual_git)), os.path.normcase(crew_path(git_shim)))
        self.assertTrue(lines[2].endswith(" -C {0} fetch sandbox-{1}".format(tree, name)), lines[2])
        self.assertEqual(lines[3], "sbx rm --force {0}".format(name))

    def test_sandbox_without_sbx_refuses_and_names_the_playbook(self):
        env, _git_shim = self.validation_git_env(Path(self.temp.name) / "bin-no-sbx")

        result = run_crew(self.repo, "run", "tools", str(self.task), "--worktree", str(self.tree),
                          "--sandbox", env=env)

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
argv = sys.argv[1:]
with open(os.environ['GIT_RECORD'], 'a', encoding='utf-8') as handle:
    handle.write(json.dumps(argv) + '\\n')
if 'rev-parse' in argv:
    flag = argv[-1]
    if flag == '--git-common-dir':
        print(os.environ.get('TEST_COMMON_DIR', ''))
    elif flag == '--git-dir':
        print(os.environ.get('TEST_ADMIN_DIR', ''))
    elif flag == '--show-toplevel':
        print(os.environ.get('TEST_TOPLEVEL', ''))
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
               "RANDOM_CREW_VAR": "not-passed",
               "TEST_COMMON_DIR": str(git_dir_for(self.tree, "--git-common-dir")),
               "TEST_ADMIN_DIR": str(git_dir_for(self.tree, "--git-dir")),
               "TEST_TOPLEVEL": str(self.tree)}

        result = run_crew(self.repo, "run", "tools", str(self.task), "--worktree", str(self.tree), "--sandbox", env=env)

        self.assertEqual(result.returncode, 7, result.stderr)
        sbx_calls = self.read_json_lines(sbx_record)
        self.assertEqual([call["argv"][0] for call in sbx_calls], ["create", "exec", "rm"])
        self.assertTrue(any(len(call) > 1 and call[0] == "-C" and
                            crew_path(call[1]) == crew_path(self.tree) and "fetch" in call
                            for call in self.read_json_lines(git_record)))
        self.assertFalse((self.tree / ".crew" / "state" / "prompts" / "001-sample.md").exists())
        create = sbx_calls[0]
        self.assertIsNone(create["random"])
        self.assertEqual(create["lfs"], "1")

    def test_sandbox_create_failure_is_not_removed_and_is_not_fetched(self):
        bin_dir, sbx_record, git_record = self.write_sandbox_bin(create_exit=3)
        env = {"PATH": str(bin_dir), "SBX_RECORD": str(sbx_record), "GIT_RECORD": str(git_record),
               "TEST_COMMON_DIR": str(git_dir_for(self.tree, "--git-common-dir")),
               "TEST_ADMIN_DIR": str(git_dir_for(self.tree, "--git-dir")),
               "TEST_TOPLEVEL": str(self.tree)}

        result = run_crew(self.repo, "run", "tools", str(self.task), "--worktree", str(self.tree), "--sandbox", env=env)

        self.assertEqual(result.returncode, 3, result.stderr)
        sbx_calls = self.read_json_lines(sbx_record)
        self.assertEqual([call["argv"][0] for call in sbx_calls], ["create"])
        self.assertFalse(any("fetch" in call for call in self.read_json_lines(git_record)))


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
        program = first_command_word(result.stdout)
        self.assertEqual(os.path.normcase(crew_path(program)), os.path.normcase(crew_path(shim)))
        self.assertIn("Re-instate from files: follow crew/playbooks/restart.md.", result.stdout)
        self.assertIn("Role body line.", result.stdout)

    def test_start_print_devin_prints_the_resolved_prompt_file_command(self):
        shim = write_versioned_cli(Path(self.temp.name) / "bin", "devin", "3000.0")
        result = run_crew(self.repo, "start", "--role", "tools", "--engine", "devin", "--print",
                          env={"PATH": str(Path(self.temp.name) / "bin")})

        self.assertEqual(result.returncode, 0, result.stderr)
        program = first_command_word(result.stdout)
        self.assertEqual(os.path.normcase(crew_path(program)), os.path.normcase(crew_path(shim)))
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
        subprocess.run(["git", "add", "base.txt", "crew"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "base"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "remote", "add", "origin", str(self.remote)], cwd=str(self.repo), check=True)
        subprocess.run(["git", "push", "-q", "origin", "main"], cwd=str(self.repo), check=True)
        (self.repo / "crew.local.ini").write_text("[crew]\ncommit_identity = {0}\n".format(COMMIT_IDENTITY),
                                                  encoding="utf-8")

    def git_recorder(self):
        real_git = shutil.which("git")
        bin_dir = Path(self.temp.name) / "git-bin"
        record = Path(self.temp.name) / "git-record.jsonl"
        source = """
import json
import os
import subprocess
import sys
argv = sys.argv[1:]
with open(os.environ['GIT_RECORD'], 'a', encoding='utf-8') as handle:
    handle.write(json.dumps(argv) + '\\n')
result = subprocess.run([{0!r}] + argv)
sys.exit(result.returncode)
""".format(real_git)
        write_path_python(bin_dir, "git", source)
        env = {"PATH": str(bin_dir) + os.pathsep + os.environ.get("PATH", ""), "GIT_RECORD": str(record)}
        return record, env

    def recorded_git_calls(self, record):
        if not record.exists():
            return []
        return [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]

    def switch_branch(self, name):
        subprocess.run(["git", "checkout", "-q", "-b", name], cwd=str(self.repo), check=True)
        (self.repo / "work.txt").write_text(name + "\n", encoding="utf-8")
        subprocess.run(["git", "add", "work.txt"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "work"], cwd=str(self.repo), check=True)

    def linked_feature_worktree(self, branch="feat/linked-push"):
        tree = Path(self.temp.name) / "linked-push"
        subprocess.run(["git", "worktree", "add", "-q", "-b", branch, str(tree)], cwd=str(self.repo), check=True)
        (tree / "linked.txt").write_text("linked worktree commit\n", encoding="utf-8")
        subprocess.run(["git", "add", "linked.txt"], cwd=str(tree), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "linked work"], cwd=str(tree), check=True)
        return tree

    def assert_printed_refspec_and_no_push(self, result, branch, record=None):
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), "HEAD:refs/heads/" + branch)
        remote_ref = subprocess.run(["git", "--git-dir", str(self.remote), "rev-parse", "--verify",
                                     "refs/heads/" + branch], capture_output=True, text=True, check=False)
        self.assertNotEqual(remote_ref.returncode, 0)
        if record is not None:
            self.assertFalse(any(call and call[0] == "push" for call in self.recorded_git_calls(record)))

    def test_push_print_in_a_linked_worktree_uses_the_main_checkouts_identity_file(self):
        tree = self.linked_feature_worktree()
        record, env = self.git_recorder()

        result = run_crew(tree, "push", "--print", env=env)

        self.assert_printed_refspec_and_no_push(result, "feat/linked-push", record)

    def test_push_print_in_a_linked_worktree_uses_the_commit_identity_environment_alone(self):
        tree = self.linked_feature_worktree()
        (self.repo / "crew.local.ini").unlink()
        record, env = self.git_recorder()
        env["CREW_COMMIT_IDENTITY"] = COMMIT_IDENTITY

        result = run_crew(tree, "push", "--print", env=env)

        self.assert_printed_refspec_and_no_push(result, "feat/linked-push", record)

    def test_push_print_identity_environment_wins_over_the_main_checkouts_file(self):
        tree = self.linked_feature_worktree()
        config = self.repo / "crew.local.ini"
        config.write_text(config.read_text(encoding="utf-8").replace(
            COMMIT_IDENTITY, "Wrong Person <999999+wrong@users.noreply.github.com>"), encoding="utf-8")
        record, env = self.git_recorder()
        env["CREW_COMMIT_IDENTITY"] = COMMIT_IDENTITY

        result = run_crew(tree, "push", "--print", env=env)

        self.assert_printed_refspec_and_no_push(result, "feat/linked-push", record)

    def test_push_pushes_the_current_feature_branch_to_origin_under_the_same_name(self):
        self.switch_branch("feat/worker-change")
        record, env = self.git_recorder()

        result = run_crew(self.repo, "push", env=env)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(["push", "--no-follow-tags", "origin", "HEAD:refs/heads/feat/worker-change"],
                      self.recorded_git_calls(record))
        local = subprocess.run(["git", "rev-parse", "feat/worker-change"], cwd=str(self.repo),
                               capture_output=True, text=True, check=True).stdout.strip()
        remote = subprocess.run(["git", "--git-dir", str(self.remote), "rev-parse", "refs/heads/feat/worker-change"],
                                capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(local, remote)

    def test_push_refuses_main_master_buwiz_next_bad_prefixes_force_plus_tags_and_detached_head(self):
        cases = []
        for branch in ("main", "master", "buwiz-next", "feature/worker-change", "test/linux-windows/pr-7"):
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

    def test_push_refuses_a_missing_or_non_noreply_commit_identity_setting(self):
        self.switch_branch("feat/identity-setting")
        for replacement in ("", "commit_identity = Crew Maintainer <somebody@example.com>"):
            with self.subTest(replacement=replacement):
                config = self.repo / "crew.local.ini"
                original = config.read_text(encoding="utf-8")
                config.write_text(original.replace("commit_identity = " + COMMIT_IDENTITY, replacement),
                                  encoding="utf-8")

                result = run_crew(self.repo, "push")

                self.assertNotEqual(result.returncode, 0)
                self.assertIn("commit_identity", result.stderr)
                config.write_text(original, encoding="utf-8")

    def test_push_refuses_a_commit_not_written_by_the_configured_identity_and_masks_the_addresses(self):
        self.switch_branch("feat/identity-check")
        (self.repo / "work.txt").write_text("wrong identity\n", encoding="utf-8")
        env = os.environ.copy()
        env.update({"GIT_AUTHOR_NAME": "Wrong Person", "GIT_AUTHOR_EMAIL": "wrong@example.com",
                    "GIT_COMMITTER_NAME": "Wrong Person", "GIT_COMMITTER_EMAIL": "wrong@example.com"})
        subprocess.run(["git", "add", "work.txt"], cwd=str(self.repo), check=True)
        subprocess.run(["git", "commit", "-q", "-m", "wrong identity"], cwd=str(self.repo), env=env, check=True)
        offending = subprocess.run(["git", "rev-parse", "HEAD"], cwd=str(self.repo), capture_output=True,
                                   text=True, check=True).stdout.strip()

        result = run_crew(self.repo, "push")

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(offending[:12], result.stderr)
        self.assertIn("@example.com", result.stderr)
        self.assertIn("author", result.stderr)
        self.assertIn("committer", result.stderr)
        self.assertNotIn("wrong@example.com", result.stderr)

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
        self.assertIn(crew_path(repo / "crew" / "roles" / "tools.md"), result.stderr)

    def test_unknown_frontmatter_keys_are_kept_and_ignored(self):
        text = role_file(extra="future-key: future value", body="Unknown keys do not matter.\n")
        repo = write_fixture_repo(self.temp.name, {"tools": text})

        result = run_crew(repo, "prompt", "tools")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Unknown keys do not matter.", result.stdout)


class RepoPlumbingTests(unittest.TestCase):
    def test_permission_lists_say_they_stop_accidents_not_unsandboxed_writes(self):
        text = CREW.read_text(encoding="utf-8")
        comment = ("these stop an honest worker's accident; a worker outside a sandbox can write what "
                   "its operating-system account can")

        self.assertGreaterEqual(text.count(comment), 2)

    def test_justfile_has_one_portable_crew_recipe(self):
        text = (ROOT / "justfile").read_text(encoding="utf-8")

        self.assertIn("\n[positional-arguments]\ncrew *args:\n    {{PYTHON}} crew/bin/crew \"$@\"\n", text)
        self.assertEqual(text.count("\ncrew *args:"), 1)
        self.assertNotIn("{{quote(args)}}", text)

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
        self.assertEqual(parser["crew"]["state_branch"], "crew-state")
        self.assertEqual(parser["crew"]["holder"], "conductor")
        self.assertIn("engine", parser["crew"])
        self.assertIn("commit_identity", parser["crew"])
        self.assertNotIn("merge", parser["crew"])
        self.assertNotIn("tag", parser["crew"])
        self.assertNotIn("tier", parser["crew"])
        self.assertNotIn("local_builds", parser["crew"])
        self.assertIn("CREW_STATE_DIR", text)
        self.assertIn("CREW_ENGINE", text)
        self.assertIn("CREW_COMMIT_IDENTITY", text)
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

    def test_crew_readme_and_playbooks_document_the_restart_drill_contract(self):
        readme = (ROOT / "crew" / "README.md").read_text(encoding="utf-8")
        restart = (ROOT / "crew" / "playbooks" / "restart.md").read_text(encoding="utf-8")
        state = (ROOT / "crew" / "playbooks" / "state.md").read_text(encoding="utf-8")

        self.assertIn("this worktree first, then the MAIN worktree", readme)
        self.assertIn("at the state folder's ROOT", restart)
        self.assertIn("read_messages advances its last-seen mark", restart)
        self.assertIn('the conducting role is "conductor"', restart)
        self.assertIn('a session may be NAMED differently (for example "orchestrator")', restart)
        self.assertIn('{"queue": [], "batch": null}', state)
        self.assertIn("crew state push` appends one line", state)
        self.assertIn("roles/reviewer/outbox/<owner>-<repo>-<n>.verdict", state)
        self.assertIn("verdict: MERGE", state)
        self.assertIn("head: <full sha read>", state)
        self.assertIn("machine ticket copied into a state file is one pipe-separated row", state)

    def test_real_tree_lint_and_sync_check_pass(self):
        for args in (("lint",), ("sync", "--check")):
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, str(CREW), *args], cwd=str(ROOT),
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


class JustfileCrewRecipeTests(unittest.TestCase):
    def test_the_crew_recipe_passes_arguments_as_separate_words(self):
        text = (ROOT / "justfile").read_text(encoding="utf-8")
        match = re.search(r"(?ms)^\[positional-arguments\]\ncrew \*args:\n    \{\{PYTHON\}\} crew/bin/crew \"\$@\"\s*$", text)

        self.assertIsNotNone(match, "the crew recipe must pass its positional arguments as \"$@\"")
        self.assertNotIn("crew/bin/crew {{args}}", text)
        self.assertNotIn("crew/bin/crew {{quote(args)}}", text)


class ExampleSettingsTests(unittest.TestCase):
    def test_the_example_file_matches_the_launcher_settings_surface(self):
        text = (ROOT / "crew" / "local.example.ini").read_text(encoding="utf-8")
        parser = configparser.ConfigParser(interpolation=None)
        parser.read_string(text)

        self.assertIn("commit_identity", parser["crew"])
        self.assertIn("CREW_STATE_DIR", text)
        self.assertIn("CREW_ENGINE", text)
        self.assertIn("CREW_COMMIT_IDENTITY", text)
        for section in parser.sections():
            for key in parser[section]:
                self.assertNotIn(key, ("tier", "local_builds", "merge", "tag"))


class PythonPortabilityTests(unittest.TestCase):
    def load_launcher(self):
        loader = importlib.machinery.SourceFileLoader("crew_launcher", str(CREW))
        spec = importlib.util.spec_from_loader("crew_launcher", loader)
        module = importlib.util.module_from_spec(spec)
        loader.exec_module(module)
        return module

    def test_generated_agent_text_is_identical_on_windows_and_posix(self):
        module = self.load_launcher()
        role = module.Role(name="tools", title="Tools", engine="devin", model="", needs=[], repos=[],
                           body="body\n", values={}, path=Path("crew/roles/tools.md"))

        with unittest.mock.patch.object(module.os, "name", "nt"):
            windows = module.agent_file_text(role)
        with unittest.mock.patch.object(module.os, "name", "posix"):
            posix = module.agent_file_text(role)

        self.assertEqual(windows, posix)
        self.assertIn("run the crew launcher: `crew/bin/crew` with Python (`python3 crew/bin/crew …`; on Windows `python crew/bin/crew …`)", windows)

    def test_cmd_processes_go_through_comspec_with_the_windows_environment_on_any_system(self):
        module = self.load_launcher()
        comspec = r"C:\Windows\System32\cmd.exe"
        engine = r"C:\Tools\CLAUDE.CMD"
        argv, _child_env = module.process_command([engine, "--version"], {"COMSPEC": comspec})

        self.assertEqual(argv, [comspec, "/c", engine, "--version"])
        bat_argv, _child_env = module.process_command([r"C:\Tools\engine.BAT", "-p"], {"COMSPEC": comspec})
        self.assertEqual(bat_argv[:2], [comspec, "/c"])
        exe_argv, _child_env = module.process_command([r"C:\Tools\engine.EXE", "-p"], {"COMSPEC": comspec})
        self.assertEqual(exe_argv, [r"C:\Tools\engine.EXE", "-p"])
        ambient = {"COMSPEC": comspec, "SYSTEMROOT": r"C:\Windows", "PATHEXT": ".COM;.EXE;.BAT;.CMD"}
        child_env = module.process_environment({"CUSTOM": "1"}, ambient=ambient)
        self.assertEqual(child_env["COMSPEC"], comspec)
        self.assertEqual(child_env["SYSTEMROOT"], r"C:\Windows")
        self.assertEqual(child_env["PATHEXT"], ".COM;.EXE;.BAT;.CMD")
        self.assertEqual(child_env["CUSTOM"], "1")

        completed = subprocess.CompletedProcess([], 0, stdout="claude version 1.2.3\n", stderr="")
        with unittest.mock.patch.object(module, "executable_for", return_value=(engine, ["claude"])):
            with unittest.mock.patch.object(module.subprocess, "run", return_value=completed) as run:
                path, version = module.version_line("claude")

        self.assertEqual(path, engine)
        self.assertEqual(version, "claude version 1.2.3")
        version_argv = run.call_args[0][0]
        self.assertEqual(version_argv[1:], ["/c", engine, "--version"])
        self.assertTrue(version_argv[0].lower().endswith("cmd.exe"), version_argv)


if __name__ == "__main__":
    unittest.main()
