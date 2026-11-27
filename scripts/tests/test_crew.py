"""crew/bin/crew, the portable launch crew runner, against fixture repositories."""

import configparser
import json
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
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

argv = sys.argv[1:]
record = {"argv": argv, "cwd": os.getcwd(), "git_lfs_skip_push": os.environ.get("GIT_LFS_SKIP_PUSH")}

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
        "[engine.devin]\ncommand = {0}\nconfig = {1}\n\n[engine.claude]\ncommand = {0}\n".format(command, config),
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
    print("\n".join(data.get("checks", {}).get(argv[2], [])))
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
        header = result.stdout.splitlines()[:3]
        self.assertEqual(len(header), 3)
        self.assertIn("'tools' worker", header[0])
        self.assertIn("Repository root:", header[1])
        self.assertIn("State folder:", header[2])

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
            "checks": {"7": ["ci-ok\tpass\t1\thttps://example.invalid"]},
        }
        gh_data.write_text(json.dumps(data), encoding="utf-8")
        self.set_queue([7])
        env = {"FAKE_GH_DATA": str(gh_data)}

        first = self.watch(env)
        second = self.watch(env)
        data["view"]["7"]["headRefOid"] = "head2"
        gh_data.write_text(json.dumps(data), encoding="utf-8")
        third = self.watch(env)
        data["checks"]["7"] = ["ci-ok\tfail\t1\thttps://example.invalid"]
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

    def lint_line(self, line):
        fixture = Path(self.temp.name) / "case"
        if fixture.exists():
            shutil.rmtree(str(fixture))
        fixture.mkdir()
        repo = write_fixture_repo(str(fixture), {"tools": role_file()})
        (repo / "crew" / "CHECK.md").write_text(line + "\n", encoding="utf-8")
        return run_crew(repo, "lint")

    def assert_bad(self, line):
        result = self.lint_line(line)
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("crew/CHECK.md:1:", result.stdout.replace("\\", "/"))

    def assert_good(self, line):
        result = self.lint_line(line)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

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
        self.assert_bad("token prefix sk-example")
        self.assert_good("token prefix sk example")

    def test_x_access_token_rule(self):
        self.assert_bad("header x-access-token present")
        self.assert_good("header x access token present")

    def test_password_digits_rule(self):
        self.assert_bad("password 1234")
        self.assert_good("password 123")


class SyncTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        roles = {
            "tools": role_file(name="tools", title="Tools worker", engine="devin", model="swe-test"),
            "docs": role_file(name="docs", title="Docs worker", engine="claude", model="claude-role"),
        }
        self.repo = write_fixture_repo(self.temp.name, roles)

    def test_sync_writes_claude_and_devin_agent_files(self):
        result = run_crew(self.repo, "sync")

        self.assertEqual(result.returncode, 0, result.stderr)
        for folder in (".claude", ".devin"):
            agent = self.repo / folder / "agents" / "tools.md"
            self.assertTrue(agent.exists())
            text = agent.read_text(encoding="utf-8")
            self.assertIn("name: tools\n", text)
            self.assertIn("description: Tools worker\n", text)
            body = text.split("---", 2)[2].strip().splitlines()
            self.assertEqual(len(body), 5)
            self.assertIn("crew/RULES.md", body[0])
            self.assertIn("crew/roles/tools.md", body[1])
            self.assertIn("python3 crew/bin/crew state path", body[2])
            self.assertIn("STATE.md", body[2])
            self.assertIn("before any work", "\n".join(body[:4]))
            self.assertIn("STATE.md", body[4])
            self.assertIn("before you stop", body[4].lower())
        devin_text = (self.repo / ".devin" / "agents" / "tools.md").read_text(encoding="utf-8")
        self.assertIn("model: swe-test\n", devin_text)
        claude_role_devin_text = (self.repo / ".devin" / "agents" / "docs.md").read_text(encoding="utf-8")
        self.assertNotIn("model:", claude_role_devin_text)
        claude_agent = (self.repo / ".claude" / "agents" / "tools.md").read_text(encoding="utf-8")
        self.assertNotIn("model:", claude_agent)

    def test_sync_check_passes_after_sync_and_names_a_difference(self):
        self.assertEqual(run_crew(self.repo, "sync").returncode, 0)
        good = run_crew(self.repo, "sync", "--check")
        self.assertEqual(good.returncode, 0, good.stderr + good.stdout)

        agent = self.repo / ".claude" / "agents" / "tools.md"
        agent.write_text(agent.read_text(encoding="utf-8") + "stale line\n", encoding="utf-8")
        bad = run_crew(self.repo, "sync", "--check")

        self.assertEqual(bad.returncode, 1)
        self.assertIn(".claude/agents/tools.md", bad.stdout.replace("\\", "/"))

    def test_sync_check_names_a_missing_devin_agent(self):
        missing = run_crew(self.repo, "sync", "--check")

        self.assertEqual(missing.returncode, 1)
        self.assertIn(".devin/agents/tools.md", missing.stdout.replace("\\", "/"))


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

    def test_state_init_and_push_create_the_orphan_branch_and_commit_as_hexuria(self):
        state = Path(self.temp.name) / "shared-state"
        remote = Path(self.temp.name) / "shared-remote.git"
        subprocess.run(["git", "init", "-q", "--bare", str(remote)], check=True)
        (self.repo / "crew.local.ini").write_text(
            "[crew]\nstate_dir = {0}\nstate_remote = {1}\n".format(state, remote), encoding="utf-8")

        init = run_crew(self.repo, "state", "init")
        self.assertEqual(init.returncode, 0, init.stderr)
        subprocess.run(["git", "config", "user.name", "hexuria"], cwd=str(state), check=True)
        subprocess.run(["git", "config", "user.email", "295542929+hexuria@users.noreply.github.com"],
                       cwd=str(state), check=True)
        (state / "roles" / "tools" / "outbox" / "001-result.md").write_text("done\n", encoding="utf-8")

        push = run_crew(self.repo, "state", "push")

        self.assertEqual(push.returncode, 0, push.stderr)
        branch = subprocess.run(["git", "--git-dir", str(remote), "rev-parse", "crew/state"],
                                capture_output=True, text=True, check=True).stdout.strip()
        author = subprocess.run(["git", "--git-dir", str(remote), "log", "-1", "--format=%an <%ae>", branch],
                                capture_output=True, text=True, check=True).stdout.strip()
        self.assertEqual(author, "hexuria <295542929+hexuria@users.noreply.github.com>")
        tree = subprocess.run(["git", "--git-dir", str(remote), "ls-tree", "-r", "--name-only", branch],
                              capture_output=True, text=True, check=True).stdout
        self.assertIn("roles/tools/outbox/001-result.md", tree)
        self.assertIn("queue.json", tree)


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
        for entry in ("Write({0})".format(tree), "Write({0})".format(role_dir), "Write({0})".format(gitdir),
                      "Exec(git)", "Exec(python3)", "Exec(gh pr view)", "Exec(gh issue create)"):
            self.assertIn(entry, permissions["allow"])
        self.assertEqual(permissions["deny"], [
            "Exec(gh pr merge)", "Exec(gh pr close)", "Exec(gh run cancel)", "Exec(gh release)",
            "Exec(gh secret)", "Exec(gh auth)", "Exec(cargo)", "Exec(nsh)", "Exec(rm -rf)",
            "Exec(git push --force)", "Exec(git tag)",
        ])
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
        self.assertEqual(tools, "Read,Edit,Write,Glob,Grep,Bash(git:*),Bash(python3:*),"
                                "Bash(gh pr view:*),Bash(gh pr list:*),Bash(gh pr checks:*),"
                                "Bash(gh pr diff:*),Bash(gh pr create:*),Bash(gh run view:*),"
                                "Bash(gh run list:*),Bash(gh issue list:*),Bash(gh issue view:*),"
                                "Bash(gh issue create:*)")
        self.assertIsNone(record["config_path"])
        self.assertIn("TASK FOR WORKER", record["stdin"])
        self.assertIn("Role body line.", record["stdin"])
        log = repo / ".crew" / "state" / "roles" / "tools" / "outbox" / "001-sample.run.log"
        self.assertEqual(log.read_text(encoding="utf-8").splitlines()[-1], "exit 0")


class AutomergeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"

    def write_gh(self, view, checks):
        record = Path(self.temp.name) / "automerge-gh-record.jsonl"
        source = r'''
import json
import os
import sys
view = {0!r}
checks = {1!r}
record_path = os.environ["GH_RECORD"]
argv = sys.argv[1:]
with open(record_path, "a", encoding="utf-8") as handle:
    handle.write(json.dumps(argv) + "\n")
merged = any(json.loads(line)[:2] == ["pr", "merge"] for line in open(record_path, encoding="utf-8"))
if argv[:2] == ["pr", "merge"]:
    print("merged")
elif argv[:2] == ["pr", "view"]:
    if merged:
        view = {{"state": "MERGED", "headRefOid": view["headRefOid"], "mergeStateStatus": "CLEAN",
                "mergeCommit": {{"oid": "merge123"}}}}
    print(json.dumps(view))
elif argv[:2] == ["pr", "checks"]:
    print("\n".join(checks))
'''.format(view, checks)
        write_path_python(self.bin, "gh", source)
        return record

    def automerge(self, record, *extra):
        env = {"PATH": str(self.bin), "GH_RECORD": str(record)}
        return run_crew(self.repo, "automerge", "7", "abc123", "--repo", "owner/repo", *extra, env=env)

    def test_clean_green_pull_request_merges_at_the_exact_head(self):
        record = self.write_gh({"state": "OPEN", "headRefOid": "abc123", "mergeStateStatus": "CLEAN"},
                               ["ci-ok\tpass", "Linux tests\tpass"])

        result = self.automerge(record)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("#7 MERGED merge123", result.stdout)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        merge = next(call for call in calls if call[:2] == ["pr", "merge"])
        self.assertIn("--match-head-commit", merge)
        self.assertIn("abc123", merge)
        self.assertIn("Merged by the orchestrator: reviewed, GitHub clean and green at this head.", merge)

    def test_moved_head_refuses_to_merge(self):
        record = self.write_gh({"state": "OPEN", "headRefOid": "different", "mergeStateStatus": "CLEAN"},
                               ["ci-ok\tpass"])

        result = self.automerge(record)

        self.assertEqual(result.returncode, 2)
        self.assertIn("head moved", result.stdout)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        self.assertFalse(any(call[:2] == ["pr", "merge"] for call in calls))

    def test_failing_checks_refuse_to_merge_and_print_the_failed_jobs(self):
        record = self.write_gh({"state": "OPEN", "headRefOid": "abc123", "mergeStateStatus": "CLEAN"},
                               ["ci-ok\tpass", "Windows tests\tfail"])

        result = self.automerge(record)

        self.assertEqual(result.returncode, 3)
        self.assertIn("Windows tests", result.stdout)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        self.assertFalse(any(call[:2] == ["pr", "merge"] for call in calls))


class BatchwatchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})
        self.bin = Path(self.temp.name) / "bin"

    def write_just(self, status_lines):
        source = """
import sys
args = sys.argv[1:]
if args and args[0] == 'ci-batch-status':
    print({0!r})
elif args and args[0] == 'ci-batch-merge':
    print('gh pr merge 7 --repo owner/repo --squash --match-head-commit abc123')
elif args and args[0] == 'ci-batch-verify':
    print('equal: tested tree')
""".format("\n".join(status_lines))
        write_path_python(self.bin, "just", source)

    def write_gh(self):
        record = Path(self.temp.name) / "gh-record.jsonl"
        source = r'''
import json
import os
import sys
with open(os.environ["GH_RECORD"], "a", encoding="utf-8") as handle:
    handle.write(json.dumps(sys.argv[1:]) + "\n")
if sys.argv[1:3] == ["pr", "merge"]:
    print("merged")
elif sys.argv[1:3] == ["pr", "view"]:
    print(json.dumps({"state": "MERGED", "mergeCommit": {"oid": "merge123"}}))
'''
        write_path_python(self.bin, "gh", source)
        return record

    def batchwatch(self, record, *extra):
        env = {"PATH": str(self.bin), "GH_RECORD": str(record)}
        return run_crew(self.repo, "batchwatch", "test/linux-windows/batch-7", "7", *extra, env=env)

    def test_green_batch_merges_with_the_body_and_verifies(self):
        self.write_just(["batch: green"])
        record = self.write_gh()

        result = self.batchwatch(record)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("batch: green", result.stdout)
        self.assertIn("equal: tested tree", result.stdout)
        calls = [json.loads(line) for line in record.read_text(encoding="utf-8").splitlines()]
        merge = next(call for call in calls if call[:2] == ["pr", "merge"])
        self.assertIn("--body", merge)
        self.assertIn("Merged by the orchestrator: reviewed; proven with main in one batch (test/linux-windows/batch-7).",
                      merge)
        self.assertIn("--match-head-commit", merge)

    def test_green_no_merge_stops_after_printing_green(self):
        self.write_just(["batch: green"])
        record = self.write_gh()

        result = self.batchwatch(record, "--no-merge")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("green", result.stdout)
        self.assertFalse(record.exists())

    def test_red_batch_prints_failed_jobs_and_merges_nothing(self):
        self.write_just(["batch: red", "    failed job: Linux tests (failure)  https://example.invalid"])
        record = self.write_gh()

        result = self.batchwatch(record)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed job: Linux tests", result.stdout)
        self.assertFalse(record.exists())


class SandboxRunTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file(engine="devin", model="sand-model")})
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
            name, str((self.repo / ".crew-prompt-001-sample.md").resolve())))
        self.assertEqual(lines[2], "git -C {0} fetch sandbox-{1}".format(tree, name))
        self.assertEqual(lines[3], "sbx rm --force {0}".format(name))

    def test_sandbox_without_sbx_refuses_and_names_the_playbook(self):
        result = run_crew(self.repo, "run", "tools", str(self.task), "--sandbox", env={"PATH": ""})

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("crew/playbooks/sandbox.md", result.stderr)
        self.assertEqual(len(result.stderr.strip().splitlines()), 1)


class StartTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = write_fixture_repo(self.temp.name, {"tools": role_file()})

    def test_start_print_claude_prints_the_interactive_command(self):
        result = run_crew(self.repo, "start", "--role", "tools", "--engine", "claude", "--print")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("claude "), result.stdout)
        self.assertIn("Re-instate from files: follow crew/playbooks/restart.md.", result.stdout)
        self.assertIn("Role body line.", result.stdout)

    def test_start_print_devin_prints_prompt_file_command(self):
        result = run_crew(self.repo, "start", "--role", "tools", "--engine", "devin", "--print")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(result.stdout.startswith("devin --prompt-file "), result.stdout)
        parts = result.stdout.strip().split()
        prompt_path = Path(parts[2])
        self.assertTrue(prompt_path.exists())
        self.assertIn("Re-instate from files: follow crew/playbooks/restart.md.",
                      prompt_path.read_text(encoding="utf-8"))


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

        self.assertIn("\ncrew *args:\n    {{PYTHON}} crew/bin/crew {{args}}\n", text)
        self.assertEqual(text.count("\ncrew *args:"), 1)

    def test_gitignore_keeps_local_state_private_but_allows_committed_agent_files(self):
        lines = (ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()

        self.assertIn("/.crew/", lines)
        self.assertIn("/crew.local.ini", lines)
        self.assertIn("/.claude/*", lines)
        self.assertNotIn("/.claude/", lines)
        self.assertLess(lines.index("/.claude/*"), lines.index("!/.claude/agents/"))
        self.assertLess(lines.index("/.claude/*"), lines.index("!/.claude/skills/"))

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


if __name__ == "__main__":
    unittest.main()
