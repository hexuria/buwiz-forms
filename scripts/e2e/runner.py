"""End-to-end suites for the app, run through gpui-agent recipes, the same on every platform.

    python3 scripts/e2e <target> [suite] [options]      (or: just e2e <target> [suite] [options])
    python3 scripts/e2e check [suite]                   (no app: every recipe of the suite is checked)

<target> is macos, linux, windows, ios (the Simulator) or android (the emulator). A run:

1. checks the suite (every recipe file parses, its params are supplied, the safety rules below hold);
2. starts the app with `just nsh-run-agent <target> <scratch db>` (a debug build, dry run, no keychain), a token it
   makes itself unless GPUI_AGENT_TOKEN is set, never printed;
3. waits for the window's control plane (GPUI_AGENT_ADDR, default 127.0.0.1:17421) and the data plane (the next port);
4. runs each recipe of the suite with `gpui-agent recipe run`, the variant for the form factor the app reports
   (phone or desktop, a windows variant where one exists), its generated ids read from a snapshot first;
5. writes OUT/report.json, OUT/report.md and OUT/run.log: per recipe pass / fail, time, the failing step and its
   error, a screenshot and the tree on failure. OUT is ~/BuwizNsh/evidence/e2e/<target>-<time> unless --out.

Everything goes through gpui-agent: no OS input, no fixed coordinates. Typing at a person's pace is the recipe's own
`key_delay_ms` (gpui-agent-cli 0.4.1); the software keyboard is the `system-keyboard` node a host with feature
`software_keyboard` puts in the tree. A recipe that needs either runs its `fallback` on an older client or host, or
is reported as "needs ...", never silently passed.

Safety, enforced here: a recipe whose steps press Queue / Send / Submit / File (or invoke a send) must be marked
"send" in its suite, and runs only with --allow-send while EBIR_TEST_ENV=1 and BIR_SFTP_DRY_RUN=1 are set and
BIR_SFTP_LIVE is not; a step that types into a box whose id or name says password, PIN, passcode, secret or token is
refused; the token never reaches a log or the screen.
"""
import argparse
import datetime as dt
import json
import os
import re
import secrets
import shlex
import shutil
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "agent-recipes"
SUITES = RECIPES / "e2e"
SCHEMA = RECIPES / "bir.schema.json"
TARGETS = ("macos", "linux", "windows", "ios", "android")
DEFAULT_ADDR = "127.0.0.1:17421"
ANDROID_HOST = "com.longbridge.nsh.host"
# The Android development build's guarded test database (EBIR_TEST_ENV): a fresh run removes it.
ANDROID_TEST_DB = "files/nsh/dev.goldcoders.bir/test-data/bir_data.db"

SEND_WORDS = {"queue", "send", "submit", "file", "filing"}
SECRET_WORDS = {"password", "passcode", "pin", "secret", "token"}
TYPING_OPS = {"type", "set_value", "paste"}
TARGET_OPS = {"click", "press", "type", "set_value", "key", "paste", "long_press", "assert", "wait_until", "scroll",
              "hover", "swipe"}
PRESS_OPS = {"click", "press", "long_press", "key"}
PARAM = re.compile(r"\$(?:\{([A-Za-z_][A-Za-z0-9_]*)\}|([A-Za-z_][A-Za-z0-9_]*))")


class SuiteError(Exception):
    """A suite or recipe that cannot run as written."""


# --- Versions -----------------------------------------------------------------------------------------------------

def parse_version(text):
    """'0.4.1' -> (0, 4, 1); None for anything else."""
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", (text or "").strip())
    return tuple(int(part) for part in match.groups()) if match else None


def version_at_least(have, need):
    have, need = parse_version(have), parse_version(need)
    return bool(have and need and have >= need)


def installed_cli_version(env=None, run=subprocess.run):
    """The gpui-agent-cli version on this machine: E2E_GPUI_AGENT_CLI_VERSION, else `cargo install --list`, else the
    version Cargo.lock pins (the setup scripts install exactly that one)."""
    env = os.environ if env is None else env
    if parse_version(env.get("E2E_GPUI_AGENT_CLI_VERSION")):
        return env["E2E_GPUI_AGENT_CLI_VERSION"].lstrip("v")
    try:
        listed = run(["cargo", "install", "--list"], capture_output=True, text=True, timeout=30).stdout
        match = re.search(r"^gpui-agent-cli v(\d+\.\d+\.\d+)", listed, re.M)
        if match:
            return match.group(1)
    except (OSError, subprocess.SubprocessError):
        pass
    return pinned_gpui_agent_version()


def pinned_gpui_agent_version(lock=ROOT / "Cargo.lock"):
    try:
        text = lock.read_text(encoding="utf-8")
    except OSError:
        return None
    match = re.search(r'name = "gpui-agent"\nversion = "(\d+\.\d+\.\d+)"', text)
    return match.group(1) if match else None


# --- Recipes: what their steps do --------------------------------------------------------------------------------

def recipe_steps(path):
    """The steps of a recipe file as dicts with at least `op` (and `target` where the op has one); `.wants` lines are
    read the way gpui-agent reads them (one op per line, `#` comments, quotes keep spaces)."""
    text = Path(path).read_text(encoding="utf-8")
    if str(path).endswith(".json") or text.lstrip().startswith("{"):
        try:
            data = json.loads(text)
        except ValueError as error:
            raise SuiteError(f"{path}: not JSON: {error}") from None
        steps = data.get("steps")
        if not isinstance(steps, list) or not steps:
            raise SuiteError(f"{path}: no steps")
        return [dict(step) for step in steps]
    steps = []
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        try:
            tokens = shlex.split(line, comments=False, posix=True)
        except ValueError as error:
            raise SuiteError(f"{path}:{number}: {error}") from None
        op = tokens[0].replace("-", "_")
        step = {"op": op, "line": number}
        rest = []
        skip = False
        for index, token in enumerate(tokens[1:], 1):
            if skip:
                skip = False
                continue
            if token in ("--delivery", "--fp", "--timeout-ms", "--key-delay-ms", "--scope", "--id"):
                if token == "--key-delay-ms" and index + 1 < len(tokens):
                    step["key_delay_ms"] = tokens[index + 1]
                skip = True
                continue
            if token.startswith("--"):
                continue
            rest.append(token)
        if op in TARGET_OPS and rest:
            step["target"] = rest[0]
            if op in TYPING_OPS and len(rest) > 1:
                step["text"] = rest[1]
        if op == "invoke" and rest:
            step["name"] = rest[0]
        steps.append(step)
    if not steps:
        raise SuiteError(f"{path}: no steps")
    return steps


def placeholders(path):
    """The $params a recipe file uses (comments excluded)."""
    text = Path(path).read_text(encoding="utf-8")
    if not str(path).endswith(".json"):
        text = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    text = text.replace("$$", "")
    return {a or b for a, b in PARAM.findall(text)}


def bind(text, params):
    """`text` with $name / ${name} replaced from `params` (unknown names stay as written)."""
    if not isinstance(text, str):
        return text
    return PARAM.sub(lambda m: str(params.get(m.group(1) or m.group(2), m.group(0))), text.replace("$$", "\0")).replace(
        "\0", "$")


def words(text):
    return {word for word in re.split(r"[^a-z0-9]+", str(text or "").lower()) if word}


def is_send_step(step, params=None):
    """A step that would queue, send, submit or file a return: a press on a control so named, or such an invoke."""
    params = params or {}
    op = step.get("op", "")
    if op in PRESS_OPS:
        return bool(words(bind(step.get("target"), params)) & SEND_WORDS)
    if op == "invoke":
        return bool(words(bind(step.get("name"), params)) & SEND_WORDS)
    return False


def is_secret_entry(step, params=None):
    """A step that would type into a password, PIN, passcode, secret or token box."""
    if step.get("op") not in TYPING_OPS:
        return False
    return bool(words(bind(step.get("target"), params or {})) & SECRET_WORDS)


def send_env_ok(env):
    """The environment of a dry-run test build: EBIR_TEST_ENV=1, BIR_SFTP_DRY_RUN=1, no BIR_SFTP_LIVE."""
    return env.get("EBIR_TEST_ENV") == "1" and env.get("BIR_SFTP_DRY_RUN") == "1" and not env.get("BIR_SFTP_LIVE")


# --- Suites -------------------------------------------------------------------------------------------------------

VARIANT_KEYS = ("phone", "desktop", "windows", "file")


def load_suite(name, suites=SUITES):
    path = Path(name)
    if not path.suffix:
        path = suites / f"{name}.json"
    try:
        suite = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise SuiteError(f"no suite {name}: {error}") from None
    except ValueError as error:
        raise SuiteError(f"{path}: not JSON: {error}") from None
    if suite.get("v") != 1 or not isinstance(suite.get("recipes"), list) or not suite["recipes"]:
        raise SuiteError(f"{path}: a suite is {{\"v\": 1, \"recipes\": [...]}}")
    suite["_dir"] = path.parent
    suite["_path"] = path
    return suite


def variant(entry, form_factor, target):
    """The recipe file of `entry` for this host: `windows` on Windows when given, then `phone` / `desktop` by the form
    factor, then `file` (the same everywhere). None when the entry has no variant for it."""
    if target == "windows" and entry.get("windows"):
        return entry["windows"]
    if entry.get(form_factor):
        return entry[form_factor]
    return entry.get("file")


def entry_files(entry):
    names = [entry[key] for key in VARIANT_KEYS if entry.get(key)]
    if entry.get("fallback"):
        names += [value for value in entry["fallback"].values()] if isinstance(entry["fallback"], dict) else [
            entry["fallback"]]
    return names


def check_suite(suite):
    """Every problem of `suite` that would stop it before a host sees it, as sentences (empty: it can run)."""
    problems = []
    base = suite["_dir"]
    params = dict(suite.get("params") or {})
    seen = set()
    for index, entry in enumerate(suite["recipes"]):
        name = entry.get("name")
        where = f"recipe {index + 1} ({name or 'no name'})"
        if not name or name in seen:
            problems.append(f"{where}: needs a unique name")
        seen.add(name)
        files = entry_files(entry)
        if not files:
            problems.append(f"{where}: names no recipe file (file, phone, desktop or windows)")
        supplied = set(params) | set(entry.get("set") or {}) | set(entry.get("resolve") or {})
        for file in files:
            path = (base / file).resolve()
            if not path.is_file():
                problems.append(f"{where}: {file} does not exist")
                continue
            try:
                steps = recipe_steps(path)
            except SuiteError as error:
                problems.append(f"{where}: {error}")
                continue
            missing = sorted(placeholders(path) - supplied)
            if missing:
                problems.append(f"{where}: {file} uses ${', $'.join(missing)}, which the suite does not supply")
            bound = {**params, **(entry.get("set") or {})}
            for step in steps:
                if step.get("key_delay_ms") is not None and step.get("op") != "type":
                    problems.append(f"{where}: {file}: key_delay_ms on {step.get('op')} (only type is paced)")
                if is_send_step(step, bound) and not entry.get("send"):
                    problems.append(f"{where}: {file} presses {step.get('target') or step.get('name')!r}, a send "
                                    "step, but the suite does not mark the recipe \"send\": true")
                if is_secret_entry(step, bound):
                    problems.append(f"{where}: {file} types into {step.get('target')!r}: never into a password, "
                                    "PIN, passcode, secret or token box")
            if any(step.get("key_delay_ms") is not None for step in steps) and not entry.get("needs_cli"):
                problems.append(f"{where}: {file} paces typing (key_delay_ms): mark it \"needs_cli\": \"0.4.1\" "
                                "with a fallback")
    return problems


# --- The run log, with the token kept out ---------------------------------------------------------------------------

class RunLog:
    """One line per action: time, verb, target, result, milliseconds. Every secret given is masked in it."""

    def __init__(self, path=None, secrets_=(), echo=True):
        self.path = Path(path) if path else None
        self.secrets = [s for s in secrets_ if s]
        self.echo = echo
        self.lines = []

    def redact(self, text):
        text = str(text)
        for secret in self.secrets:
            text = text.replace(secret, "•" * 8)
        return text

    def line(self, verb, target, result, ms):
        stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"
        text = self.redact("\t".join([stamp, verb, str(target), " ".join(str(result).split()), f"{int(ms)}ms"]))
        self.lines.append(text)
        if self.path:
            with self.path.open("a", encoding="utf-8") as out:
                out.write(text + "\n")
        if self.echo:
            print(text, flush=True)
        return text


# --- gpui-agent ---------------------------------------------------------------------------------------------------

class Agent:
    """The gpui-agent command line against one control plane."""

    def __init__(self, cli, addr, env, log, reply_timeout_ms=None):
        self.cli, self.addr, self.env, self.log = cli, addr, env, log
        self.reply_timeout_ms = reply_timeout_ms

    def command(self, *args):
        base = [self.cli, "--addr", self.addr]
        if self.reply_timeout_ms:
            base += ["--reply-timeout-ms", str(self.reply_timeout_ms)]
        return base + [str(arg) for arg in args]

    def run(self, *args, timeout=600):
        try:
            done = subprocess.run(self.command(*args), capture_output=True, text=True, env=self.env, timeout=timeout)
        except subprocess.TimeoutExpired:
            return 124, "", f"gpui-agent {args[0]} did not answer in {timeout} s"
        except OSError as error:
            return 127, "", f"cannot run {self.cli}: {error}"
        return done.returncode, done.stdout, done.stderr

    def json(self, *args, timeout=60):
        code, out, err = self.run(*args, timeout=timeout)
        return code, first_json(out), (err or out).strip()

    def hello(self):
        code, data, _ = self.json("hello", timeout=15)
        if code or not isinstance(data, dict):
            return None
        return data.get("hello", data)

    def snapshot(self):
        code, data, err = self.json("snapshot", timeout=60)
        if code or not isinstance(data, dict):
            raise SuiteError(f"snapshot failed: {self.log.redact(err)[-300:]}")
        return data


def first_json(text):
    """The first JSON object in `text` (the CLI prints one; stray lines around it are ignored)."""
    text = text or ""
    start = text.find("{")
    while start >= 0:
        try:
            return json.JSONDecoder().raw_decode(text[start:])[0]
        except ValueError:
            start = text.find("{", start + 1)
    return None


def tree_nodes(snapshot):
    """Every node of a snapshot, depth first in tree order: each object with an `id` and a `role`, wherever the
    CLI's reply nests the tree (`{"tree": {"nodes": [...]}}`, `{"nodes": [...]}`, ...)."""
    stack = [snapshot]
    while stack:
        item = stack.pop()
        if isinstance(item, dict):
            if "id" in item and "role" in item:
                yield item
            stack.extend(reversed(list(item.values())))
        elif isinstance(item, list):
            stack.extend(reversed(item))


def for_host(value, context):
    """A suite field that may differ per host: a plain value, or a map keyed by target (macos, ios, ...) or form factor
    (phone, desktop), the target first. None when the map names neither."""
    if not isinstance(value, dict):
        return value
    return value.get(context["target"], value.get(context["form_factor"]))


def resolve(spec, snapshot):
    """The value a resolve spec picks from a snapshot: the id (or the `group` of `id_regex`) of the first node that
    matches every given field: `id`, `id_regex`, `role` (one or a list), `name`, `name_regex`, `visible`. None when none does."""
    for node in tree_nodes(snapshot):
        if "id" in spec and node.get("id") != spec["id"]:
            continue
        match = None
        if "id_regex" in spec:
            match = re.search(spec["id_regex"], str(node.get("id", "")))
            if not match:
                continue
        roles = spec.get("role")
        if roles is not None and node.get("role") not in (roles if isinstance(roles, list) else [roles]):
            continue
        if "name" in spec and node.get("name") != spec["name"]:
            continue
        if "name_regex" in spec and not re.search(spec["name_regex"], str(node.get("name", ""))):
            continue
        if "visible" in spec and bool(node.get("visible", True)) != bool(spec["visible"]):
            continue
        if match and spec.get("group") is not None:
            return match.group(spec["group"])
        return node.get("id")
    return None


def failing_step(receipt):
    """(id, error, error_code) of the step a partial receipt failed at, or of the run itself."""
    if not isinstance(receipt, dict):
        return None, "no receipt", None
    for step in receipt.get("steps") or []:
        if not step.get("ok"):
            return step.get("id"), step.get("error"), step.get("error_code")
    return receipt.get("step"), receipt.get("error"), receipt.get("error_code")


def wants_line_of(path, step_id):
    """For a `.wants` recipe, the source line of step `sN` (gpui-agent numbers them s1, s2, ... in file order)."""
    match = re.fullmatch(r"s(\d+)", str(step_id or ""))
    if not match or str(path).endswith(".json"):
        return None
    lines = [line for line in Path(path).read_text(encoding="utf-8").splitlines()
             if line.strip() and not line.lstrip().startswith("#")]
    index = int(match.group(1)) - 1
    return lines[index].strip() if 0 <= index < len(lines) else None


# --- One recipe -----------------------------------------------------------------------------------------------------

def plan_entry(entry, suite, context):
    """What to do with `entry` on this host: (file or None, note or None, status-if-not-run or None)."""
    form_factor, target = context["form_factor"], context["target"]
    file = variant(entry, form_factor, target)
    if not file:
        return None, f"no {form_factor} variant", "skipped"
    reasons = []
    need_cli = entry.get("needs_cli")
    if need_cli and not version_at_least(context.get("cli_version"), need_cli):
        reasons.append(f"needs gpui-agent-cli {need_cli} (this machine has {context.get('cli_version') or 'unknown'})")
    need_feature = for_host(entry.get("needs_feature"), context)
    if need_feature and need_feature not in context.get("features", ()):
        host = entry.get("needs_host") or f"a host listing {need_feature}"
        reasons.append(f"needs {host}")
    if reasons:
        fallback = entry.get("fallback")
        if isinstance(fallback, dict):
            fallback = fallback.get(form_factor) or fallback.get("file")
        if fallback:
            return fallback, "fallback: " + "; ".join(reasons), None
        return None, "; ".join(reasons), "needs"
    if entry.get("send"):
        if not context.get("allow_send"):
            return None, "presses a send control: run with --allow-send", "refused"
        if not send_env_ok(context["env"]):
            return None, "presses a send control: needs EBIR_TEST_ENV=1 and BIR_SFTP_DRY_RUN=1, no BIR_SFTP_LIVE", \
                "refused"
    return file, None, None


def run_entry(index, entry, suite, context, agent, log, out):
    started = time.monotonic()
    result = {"name": entry["name"], "status": None, "ms": 0, "note": None, "file": None}
    file, note, status = plan_entry(entry, suite, context)
    result["note"] = note
    if status:
        result["status"] = status
        log.line("recipe", entry["name"], f"{status}: {note}", 0)
        return result
    path = (suite["_dir"] / file).resolve()
    result["file"] = str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path)
    folder = out / f"{index:02d}-{entry['name']}"
    folder.mkdir(parents=True, exist_ok=True)
    params = {**(suite.get("params") or {}), **(entry.get("set") or {})}
    for param, spec in (entry.get("resolve") or {}).items():
        t0 = time.monotonic()
        try:
            value = resolve(spec, agent.snapshot())
        except SuiteError as error:
            value, why = None, str(error)
        else:
            why = "no node matches " + json.dumps(spec)
        log.line("resolve", f"${param}", value or why, (time.monotonic() - t0) * 1000)
        if not value:
            result.update(status="setup", step=None, error=f"${param}: {why}")
            result["ms"] = round((time.monotonic() - started) * 1000)
            return result
        params[param] = value
    used = placeholders(path)
    args = ["recipe", "--schema", SCHEMA, "run", path]
    for key in sorted(used):
        args += ["--set", f"{key}={params[key]}"]
    args += ["--receipt-out", folder / "receipt.json", "--screenshot-dir", folder, "--screenshot-flagged"]
    if entry.get("yes"):
        args.append("--yes")
    recorder = context["recorder"](folder) if context.get("recorder") else None
    code, stdout, stderr = agent.run(*args, timeout=entry.get("timeout_s", 900))
    if recorder:
        result["video"] = recorder.stop()
    receipt = first_json(stdout)
    result["ms"] = round((time.monotonic() - started) * 1000)
    if code == 0 and isinstance(receipt, dict) and receipt.get("ok"):
        result["status"] = "pass"
        log.line("recipe", entry["name"], "pass" + (f" ({note})" if note else ""), result["ms"])
        return result
    step, error, error_code = failing_step(receipt)
    if not error:
        error = log.redact(stderr.strip() or stdout.strip() or f"exit {code}")[-500:]
    result.update(step=step, error=log.redact(error), error_code=error_code, line=wants_line_of(path, step))
    gap = for_host(entry.get("known_gap"), context)
    result["status"] = "known gap" if gap else "fail"
    if gap:
        result["known_gap"] = gap
    if entry.get("precondition"):
        result["note"] = "; ".join(filter(None, [note, f"precondition: {entry['precondition']}"]))
    shot = folder / "failure.png"
    for extra in (["--inline"], []):
        shot_code, _, shot_err = agent.run("screenshot", "--out", shot, *extra, timeout=60)
        if shot_code == 0 and shot.exists():
            result["screenshot"] = str(shot)
            break
    else:
        result["screenshot_error"] = log.redact(shot_err.strip())[-200:]
    try:
        (folder / "failure-tree.json").write_text(json.dumps(agent.snapshot(), indent=1), encoding="utf-8")
        result["tree"] = str(folder / "failure-tree.json")
    except SuiteError:
        pass
    log.line("recipe", entry["name"], f"{result['status']} at {step}: {error}", result["ms"])
    return result


# --- The app ------------------------------------------------------------------------------------------------------

def port_answers(addr):
    host, _, port = addr.rpartition(":")
    try:
        with socket.create_connection((host, int(port)), timeout=0.5):
            return True
    except OSError:
        return False


def data_plane(addr):
    host, _, port = addr.rpartition(":")
    return f"{host}:{int(port) + 1}"


def android_adb(env):
    sdk = env.get("ANDROID_HOME") or env.get("ANDROID_SDK_ROOT")
    if sdk:
        candidate = Path(sdk) / "platform-tools" / ("adb.exe" if os.name == "nt" else "adb")
        if candidate.exists():
            return str(candidate)
    return shutil.which("adb")


def android_serial(adb):
    listed = subprocess.run([adb, "devices"], capture_output=True, text=True, timeout=30).stdout.splitlines()[1:]
    online = [line.split("\t")[0] for line in listed if line.endswith("\tdevice")]
    return online[0] if online else None


class Recorder:
    """The platform's own screen recorder for one recipe: `xcrun simctl io booted recordVideo` on the Simulator,
    `adb shell screenrecord` on the emulator (at most 180 s a file). Desktops record with the machine's recorder."""

    def __init__(self, target, folder, env, log):
        self.target, self.folder, self.log, self.env = target, Path(folder), log, env
        self.file = self.folder / "recording.mp4"
        self.proc = None
        if target == "ios":
            command = ["xcrun", "simctl", "io", "booted", "recordVideo", "--force", str(self.file)]
        elif target == "android":
            self.adb = android_adb(env)
            command = [self.adb, "shell", "screenrecord", "/sdcard/e2e-recording.mp4"] if self.adb else None
        else:
            command = None
        if command:
            try:
                self.proc = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                time.sleep(1.0)
            except OSError as error:
                log.line("record", target, f"not started: {error}", 0)

    def stop(self):
        if not self.proc:
            return None
        self.proc.send_signal(signal.SIGINT)
        try:
            self.proc.wait(timeout=20)
        except subprocess.TimeoutExpired:
            self.proc.kill()
        if self.target == "android":
            time.sleep(1.0)
            subprocess.run([self.adb, "pull", "/sdcard/e2e-recording.mp4", str(self.file)], capture_output=True)
            subprocess.run([self.adb, "shell", "rm", "-f", "/sdcard/e2e-recording.mp4"], capture_output=True)
        return str(self.file) if self.file.exists() else None


def launch(target, db, env, out, log, just):
    """`just nsh-run-agent <target> <db>` in its own process group, its output in OUT/app.log."""
    app_log = (out / "app.log").open("w", encoding="utf-8")
    kwargs = {"cwd": ROOT, "env": env, "stdout": app_log, "stderr": subprocess.STDOUT}
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        kwargs["start_new_session"] = True
    command = [just, "nsh-run-agent", target] + ([str(db)] if db else [])
    log.line("launch", " ".join(command[1:]), "started", 0)
    return subprocess.Popen(command, **kwargs)


def stop(proc, log):
    if proc is None or proc.poll() is not None:
        return
    t0 = time.monotonic()
    try:
        if os.name == "nt":
            proc.send_signal(signal.CTRL_BREAK_EVENT)
        else:
            os.killpg(proc.pid, signal.SIGINT)
        proc.wait(timeout=30)
    except (OSError, subprocess.TimeoutExpired):
        try:
            if os.name == "nt":
                proc.kill()
            else:
                os.killpg(proc.pid, signal.SIGKILL)
        except OSError:
            pass
    log.line("stop", "app", "stopped", (time.monotonic() - t0) * 1000)


def wait_for(agent, proc, timeout_s, log, what):
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout_s:
        if proc is not None and proc.poll() is not None:
            log.line("wait", what, f"the app ended (exit {proc.returncode}): see app.log", (time.monotonic() - t0) * 1000)
            return None
        hello = agent.hello()
        if hello and hello.get("ready", True):
            log.line("wait", what, "ready", (time.monotonic() - t0) * 1000)
            return hello
        time.sleep(2)
    log.line("wait", what, f"no answer in {timeout_s} s", (time.monotonic() - t0) * 1000)
    return None


# --- Reports ------------------------------------------------------------------------------------------------------

def write_report(out, header, results):
    (out / "report.json").write_text(json.dumps({**header, "recipes": results}, indent=1), encoding="utf-8")
    lines = [f"# e2e {header['suite']} on {header['target']}", "",
             f"App {header.get('app_commit') or '?'}, gpui-agent-cli {header.get('cli_version') or '?'}, "
             f"host {header.get('platform') or '?'} ({header.get('form_factor') or '?'}), features: "
             f"{', '.join(header.get('features') or []) or 'none'}.", "",
             "| # | recipe | result | ms | failing step | error / note |", "|---|---|---|---|---|---|"]
    for index, r in enumerate(results, 1):
        step = r.get("step") or ""
        if r.get("line"):
            step = f"{step} `{r['line']}`"
        detail = r.get("error") or r.get("note") or ""
        if r.get("known_gap"):
            detail = f"{detail} (known gap: {r['known_gap']})"
        if r.get("screenshot"):
            detail += f" [screenshot]({Path(r['screenshot']).relative_to(out).as_posix()})"
        lines.append(f"| {index} | {r['name']} | {r['status']} | {r.get('ms', 0)} | {step} | "
                     f"{' '.join(str(detail).split()).replace('|', '/')} |")
    (out / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def summary(results):
    counts = {}
    for r in results:
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    return ", ".join(f"{n} {status}" for status, n in sorted(counts.items()))


# --- Commands -----------------------------------------------------------------------------------------------------

def check(suite_name, cli):
    """Every recipe of the suite checked without an app: the suite's own rules, then `gpui-agent recipe validate` for
    each file the installed client can read. Exit 0 when nothing is wrong."""
    suite = load_suite(suite_name)
    problems = check_suite(suite)
    cli_version = installed_cli_version()
    validated = 0
    if shutil.which(cli) or Path(cli).exists():
        for entry in suite["recipes"]:
            for file in entry_files(entry):
                path = (suite["_dir"] / file).resolve()
                if not path.is_file():
                    continue
                paced = any(step.get("key_delay_ms") is not None for step in recipe_steps(path))
                if paced and not version_at_least(cli_version, entry.get("needs_cli") or "999.0.0"):
                    print(f"skip  {file}: paced typing needs gpui-agent-cli {entry.get('needs_cli')}")
                    continue
                done = subprocess.run([cli, "recipe", "--schema", str(SCHEMA), "validate", str(path)],
                                      capture_output=True, text=True)
                validated += 1
                if done.returncode:
                    problems.append(f"{entry['name']}: {file}: {(done.stderr or done.stdout).strip()[-300:]}")
    else:
        print(f"no {cli}: recipe files checked by this script only")
    for problem in problems:
        print("FAIL  " + problem)
    print(f"{suite['_path'].name}: {len(suite['recipes'])} recipes, {validated} files validated by gpui-agent-cli "
          f"{cli_version or '?'}, {len(problems)} problem(s)")
    return 1 if problems else 0


def parse_args(argv):
    parser = argparse.ArgumentParser(prog="e2e", description=__doc__.split("\n\n")[0])
    parser.add_argument("target", choices=TARGETS + ("check",))
    parser.add_argument("suite", nargs="?", default="smoke")
    parser.add_argument("--db", help="database for the app (default: a new one under ~/BuwizNsh/agents)")
    parser.add_argument("--out", help="report folder (default: ~/BuwizNsh/evidence/e2e/<target>-<time>)")
    parser.add_argument("--addr", help=f"window plane address (default GPUI_AGENT_ADDR, else {DEFAULT_ADDR})")
    parser.add_argument("--no-launch", action="store_true", help="drive an app already started with nsh-run-agent")
    parser.add_argument("--keep-data", action="store_true", help="Android: keep the emulator's test database")
    parser.add_argument("--allow-send", action="store_true",
                        help="run recipes marked send (Queue): also needs EBIR_TEST_ENV=1 and BIR_SFTP_DRY_RUN=1")
    parser.add_argument("--only", action="append", default=[], help="run only this recipe (repeatable)")
    parser.add_argument("--record", action="store_true", help="iOS / Android: a recording per recipe")
    parser.add_argument("--form-factor", choices=("phone", "desktop"), help="override what the host reports")
    parser.add_argument("--launch-timeout", type=int, default=3600, help="seconds for the build and first window")
    parser.add_argument("--reply-timeout-ms", type=int, help="gpui-agent's wait for one reply (slow emulators)")
    parser.add_argument("--cli", default=os.environ.get("GPUI_AGENT_CLI", "gpui-agent"))
    parser.add_argument("--just", default=os.environ.get("JUST", "just"))
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.target == "check":
            return check(args.suite, args.cli)
        return run(args)
    except SuiteError as error:
        print(f"e2e: {error}", file=sys.stderr)
        return 2


def run(args):
    suite = load_suite(args.suite)
    problems = check_suite(suite)
    if problems:
        raise SuiteError("the suite cannot run:\n  " + "\n  ".join(problems))
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = Path(args.out) if args.out else Path.home() / "BuwizNsh" / "evidence" / "e2e" / f"{args.target}-{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    token = env.get("GPUI_AGENT_TOKEN") or secrets.token_hex(16)
    env["GPUI_AGENT_TOKEN"] = token
    addr = args.addr or env.get("GPUI_AGENT_ADDR") or DEFAULT_ADDR
    env["GPUI_AGENT_ADDR"] = addr
    # What the app is launched with: a dry-run test build (nsh-run-agent sets the same; this is the runner's word).
    env.update({"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1", "BUWIZ_NO_KEYCHAIN": "1", "BIR_AUTH_BYPASS": "1"})
    env.pop("BIR_SFTP_LIVE", None)
    log = RunLog(out / "run.log", secrets_=[token])
    header = {"suite": suite["_path"].stem, "target": args.target, "started": stamp, "out": str(out),
              "cli_version": installed_cli_version(), "app_commit": git_head()}
    proc = None
    results = []
    try:
        if not args.no_launch:
            if port_answers(addr) or port_answers(data_plane(addr)):
                raise SuiteError(f"something already listens on {addr} or {data_plane(addr)}: stop it, or pass "
                                 "--no-launch to drive it")
            db = Path(args.db) if args.db else Path.home() / "BuwizNsh" / "agents" / f"e2e-{args.target}-{stamp}.db"
            db.parent.mkdir(parents=True, exist_ok=True)
            if args.target == "android" and not args.keep_data:
                fresh_android(env, log)
            proc = launch(args.target, db, env, out, log, args.just)
        window = Agent(args.cli, addr, env, log, args.reply_timeout_ms)
        hello = wait_for(window, proc, args.launch_timeout if proc else 30, log, "window plane")
        if not hello:
            raise SuiteError(f"the window's control plane on {addr} never answered (see {out / 'app.log'})")
        features = list(hello.get("features") or [])
        form_factor = args.form_factor or ("phone" if hello.get("platform") == "mobile" else "desktop")
        header.update(platform=hello.get("platform"), os=hello.get("os"), features=features, form_factor=form_factor)
        data = Agent(args.cli, data_plane(addr), env, log, args.reply_timeout_ms)
        header["data_plane"] = bool(wait_for(data, None, 60, log, "data plane"))
        context = {"target": args.target, "form_factor": form_factor, "features": features, "env": os.environ,
                   "allow_send": args.allow_send, "cli_version": header["cli_version"],
                   "recorder": (lambda folder: Recorder(args.target, folder, env, log)) if args.record else None}
        stopped = None
        for index, entry in enumerate(suite["recipes"], 1):
            if args.only and entry["name"] not in args.only:
                continue
            if stopped:
                results.append({"name": entry["name"], "status": "not run", "ms": 0,
                                "note": f"{stopped} failed and the suite needs it"})
                continue
            agent = data if entry.get("plane") == "data" else window
            result = run_entry(index, entry, suite, context, agent, log, out)
            results.append(result)
            if entry.get("required") and result["status"] not in ("pass", "skipped"):
                stopped = entry["name"]
    finally:
        stop(proc, log)
        write_report(out, header, results)
    print(f"e2e {header['suite']} on {args.target}: {summary(results)}. Report: {out / 'report.md'}")
    return 0 if all(r["status"] in ("pass", "skipped") for r in results) else 1


def fresh_android(env, log):
    """A fresh first run on the emulator: the development build's guarded test database is removed (never real data:
    the test-data folder exists only under EBIR_TEST_ENV)."""
    adb = android_adb(env)
    if not adb:
        raise SuiteError("no adb: set ANDROID_HOME")
    serial = env.get("ANDROID_SERIAL") or android_serial(adb)
    if not serial:
        raise SuiteError("no Android emulator is online (adb devices)")
    t0 = time.monotonic()
    done = subprocess.run([adb, "-s", serial, "shell", "run-as", ANDROID_HOST, "rm", "-f", ANDROID_TEST_DB,
                           ANDROID_TEST_DB + "-wal", ANDROID_TEST_DB + "-shm"], capture_output=True, text=True)
    log.line("fresh", f"{serial}:{ANDROID_TEST_DB}", "removed" if done.returncode == 0 else
             f"not removed ({done.stderr.strip() or 'host not installed yet'})", (time.monotonic() - t0) * 1000)


def git_head():
    try:
        return subprocess.run(["git", "-C", str(ROOT), "rev-parse", "--short=8", "HEAD"], capture_output=True,
                              text=True, timeout=10).stdout.strip() or None
    except (OSError, subprocess.SubprocessError):
        return None
