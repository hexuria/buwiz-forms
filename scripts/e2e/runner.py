"""End-to-end suites for the app, run through gpui-agent recipes, the same on every platform.

    python3 scripts/e2e <target> [suite] [options]      (or: just e2e <target> [suite] [options])
    python3 scripts/e2e check [suite]                   (no app: every recipe of the suite is checked)

<target> is macos, linux, windows, ios (the Simulator) or android (the emulator). A run:

1. checks the suite (every recipe file parses, its params are supplied, the safety rules below hold);
2. starts the app with `just nsh-run-agent <target> <scratch db>` (a debug build, dry run, no keychain), a token it
   makes itself unless GPUI_AGENT_TOKEN is set, never printed;
3. waits for the window's control plane (GPUI_AGENT_ADDR, default 127.0.0.1:17421) and the data plane (the next port);
4. runs each recipe of the suite with `gpui-agent recipe run`, the variant for the form factor the app reports
   (phone or desktop; the target's own variant, e.g. linux or android, where one exists), its generated ids read
   from a snapshot first;
5. writes OUT/report.json, OUT/report.md and OUT/run.log: per recipe pass / fail, time, the failing step and its
   error, a screenshot and the tree on failure. OUT is ~/BuwizNsh/evidence/e2e/<target>-<time> unless --out.

Everything goes through gpui-agent: no OS input, no fixed coordinates. Typing at a person's pace is the recipe's own
`key_delay_ms` (gpui-agent-cli 0.4.1); the software keyboard is the `system-keyboard` node a host with feature
`software_keyboard` puts in the tree. A recipe that needs either runs its `fallback` on an older client or host, or
is reported as "needs ...", never silently passed.

Safety, enforced here (on every word of a step, camelCase split, and again on ids resolved from the tree):
- a recipe whose steps press Queue / Send / Submit / File (or invoke a send) must be marked "send" in its suite, and
  runs only with --allow-send, in a dry-run test environment (EBIR_TEST_ENV=1, BIR_SFTP_DRY_RUN=1, no BIR_SFTP_LIVE),
  against an app whose app.info says "database": "test"; with --no-launch no recipe runs against any other app;
- no step types, sets, pastes or keys into a password, PIN, passcode, secret, token, OTP / TOTP or code box, or
  presses the software keyboard's keys;
- Continue / Resend on a sign-in screen only in a recipe marked "sign_in", in an app this run launched, against an
  account server on this machine (a debug build's default is the local mock);
- the app's environment has none of the live SFTP or relay variables (BIR_SFTP_HOST, _PASSWORD, ... BIR_RELAY_URL);
- the token is masked in run.log, the receipts, the failure trees and app.log;
- afterwards the app is stopped (on the Simulator or emulator too) and the scratch database removed.
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
# The NSH development host's id on Android and on the iOS Simulator.
ANDROID_HOST = "com.longbridge.nsh.host"
# The Android development build's guarded test database (EBIR_TEST_ENV): a fresh run removes it.
ANDROID_TEST_DB = "files/nsh/dev.goldcoders.bir/test-data/bir_data.db"

SEND_WORDS = {"queue", "send", "submit", "file", "filing"}
SECRET_WORDS = {"password", "passcode", "pin", "secret", "token", "otp", "totp", "recovery"}
# A secret word with a prefix in one word: newpin, confirmpin, oldpasscode, adminotp, newcode.
SECRET_WORD = re.compile(r"(new|old|confirm|current|repeat|admin|setup)?(pins?|passcodes?|passwords?|otp|totp)"
                         r"|(new|old|confirm|current|repeat|admin|setup)codes?")
# Screens that talk to the account server: every action there is a sign-in step.
SIGN_IN_PREFIXES = {"gate", "auth"}
# A code box is a secret too (the email sign-in code, an authenticator's): "code" then one of these words.
CODE_BOX_WORDS = {"box", "boxes", "cell", "cells"}
# ... or one of these, then "code" (a node's name: "Authenticator code", "6-digit code", "Verification code").
CODE_KIND_WORDS = {"authenticator", "verification", "security", "confirmation", "login", "email", "sign", "time",
                   "digit", "access", "one"}
SIGN_IN_WORDS = {"gate", "signin", "login", "auth"}
# Ops that put text or keys into a box.
TYPING_OPS = {"type", "set_value", "paste", "key"}
# Step fields that are what is typed or expected, not what is acted on.
VALUE_FIELDS = {"text", "value", "id", "needs", "screenshot", "key_delay_ms", "timeout_ms", "op", "line", "words"}
# The variables that point the app at a real SFTP server or relay: never in a run's environment. The app stays in dry
# run under the runner anyway (its own guard), but the live host and its password must not reach the process.
LIVE_SEND_VARS = ("BIR_SFTP_LIVE", "BIR_SFTP_HOST", "BIR_SFTP_PORT", "BIR_SFTP_USERNAME", "BIR_SFTP_PASSWORD",
                  "BIR_SFTP_ACCEPT_ANY_HOST_KEY", "BIR_RELAY_URL", "BUWIZ_RELAY_SEND")
# The account server a debug build signs in to: unset, it is the local mock (http://localhost:3008).
ACCOUNT_VARS = ("BUWIZ_SERVER_URL", "BUWIZ_ACCOUNT_URL")
# `--accounts mock` uses the mock's own fixed port, as the Android development scripts do. It has no health or
# version route, so a listener already there cannot be identified as another mock and is never shared.
MOCK_ACCOUNT_PORT = 3008
MOCK_ACCOUNT_ADDR = f"127.0.0.1:{MOCK_ACCOUNT_PORT}"
MOCK_ACCOUNT_URL = f"http://{MOCK_ACCOUNT_ADDR}"
MOCK_ACCOUNT_FIRST_CODE = 246810
MOCK_ACCOUNT_CODE_COUNT = 8
ACCOUNTS_ENV = "E2E_ACCOUNTS"
MOCK_ACCOUNT_URL_ENV = "E2E_MOCK_ACCOUNT_URL"
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


def installed_cli_version(cli="gpui-agent", env=None, run=subprocess.run):
    """The version of the gpui-agent binary the runner will invoke: E2E_GPUI_AGENT_CLI_VERSION first (tests), then
    `<cli> --version`. cargo's install list is not evidence: a prebuilt install replaces the binary without it."""
    env = os.environ if env is None else env
    override = env.get("E2E_GPUI_AGENT_CLI_VERSION")
    if parse_version(override):
        return override.strip().lstrip("v")
    try:
        done = run([cli, "--version"], capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.SubprocessError):
        return "unknown"
    if getattr(done, "returncode", 0) != 0:
        return "unknown"
    lines = (getattr(done, "stdout", "") or "").strip().splitlines()
    match = re.fullmatch(r"gpui-agent(?:-cli)?\s+v?(\d+\.\d+\.\d+)", lines[0] if lines else "")
    return match.group(1) if match else "unknown"


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
        # Every word after the op, flag values included, but the text a typing op types.
        typed_index = None
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
            if op in TYPING_OPS and op != "key" and len(rest) > 1:
                step["text"] = rest[1]
                typed_index = tokens.index(rest[1], tokens.index(rest[0], 1) + 1)
        step["words"] = [token for index, token in enumerate(tokens[1:], 1) if index != typed_index]
        if op == "invoke" and rest:
            step["name"] = rest[0]
        steps.append(step)
    if not steps:
        raise SuiteError(f"{path}: no steps")
    return steps


def without_step_node_refs(text, ids):
    """`text` without gpui-agent 0.5.0's `$<step>.node` / `${<step>}.node` references to the recipe's own steps (the
    node a scroll_until step matched): they are no params, and gpui-agent binds them itself."""
    for step_id in ids:
        text = text.replace(f"${step_id}.node", "").replace(f"${{{step_id}}}.node", "")
    return text


def step_node_refs(text, ids):
    """The recipe's own step ids `text` refers to as `$<step>.node`."""
    return {step_id for step_id in ids if f"${step_id}.node" in text or f"${{{step_id}}}.node" in text}


def placeholders_in_text(text, ids=()):
    """The $params in `text` (`$$` is a literal dollar; `$<step>.node` of a step in `ids` is no param)."""
    return {a or b for a, b in PARAM.findall(without_step_node_refs(text.replace("$$", ""), ids))}


def placeholders(path):
    """The $params a recipe file uses (comments excluded)."""
    text = Path(path).read_text(encoding="utf-8")
    if not str(path).endswith(".json"):
        text = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    return placeholders_in_text(text, step_ids(path))


def is_json_recipe(path):
    text = Path(path).read_text(encoding="utf-8")
    return str(path).endswith(".json") or text.lstrip().startswith("{")


def recipe_step_params(path):
    """The $params each step uses, in step order. For `.wants` the whole source line counts, typed text included."""
    ids = step_ids(path)
    if is_json_recipe(path):
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        return [placeholders_in_text(json.dumps(step), ids) for step in data.get("steps") or []]
    lines = [line for line in Path(path).read_text(encoding="utf-8").splitlines()
             if line.strip() and not line.lstrip().startswith("#")]
    return [placeholders_in_text(line, ids) for line in lines]


def step_ids(path):
    """The ids gpui-agent reports for a recipe's steps: JSON steps carry their own; `.wants` steps are s1, s2, ..."""
    steps = recipe_steps(path)
    if is_json_recipe(path):
        return [str(step.get("id") or f"s{index}") for index, step in enumerate(steps, 1)]
    return [f"s{index}" for index, _ in enumerate(steps, 1)]


def needs_edges_cut(steps, ids, cut):
    """The `(needed, dependent)` JSON step edges a cut after position `cut` would separate: `needs`, and a
    `$<step>.node` reference, which depends on that step as gpui-agent reads it."""
    positions = {step_id: position for position, step_id in enumerate(ids, 1)}
    edges = []
    for position, step in enumerate(steps, 1):
        needs = step.get("needs") or []
        if isinstance(needs, str):
            needs = [needs]
        needs = list(needs) + sorted(step_node_refs(json.dumps(step), ids) - {str(need) for need in needs})
        for need in needs:
            need_position = positions.get(str(need))
            if need_position is not None and (need_position <= cut) != (position <= cut):
                edges.append((str(need), ids[position - 1]))
    return edges


def bind(text, params):
    """`text` with $name / ${name} replaced from `params` (unknown names stay as written)."""
    if not isinstance(text, str):
        return text
    return PARAM.sub(lambda m: str(params.get(m.group(1) or m.group(2), m.group(0))), text.replace("$$", "\0")).replace(
        "\0", "$")


def word_list(text):
    """The words of an id or a name, in order: split at anything but a letter or digit and where a lowercase letter or
    a digit meets an uppercase one (`fpQueue` is fp, queue)."""
    text = re.sub(r"(?<=[a-z0-9])(?=[A-Z])", " ", str(text or ""))
    return [word for word in re.split(r"[^a-z0-9]+", text.lower()) if word]


def words(text):
    return set(word_list(text))


def selectors(step, params=None):
    """Every string a step acts on, bound: its target and every other word or field, but the text it types and the
    values it expects."""
    params = params or {}
    found = []
    if "words" in step:
        found += step["words"]

    def walk(value):
        if isinstance(value, str):
            found.append(value)
        elif isinstance(value, dict):
            for item in value.values():
                walk(item)
        elif isinstance(value, list):
            for item in value:
                walk(item)

    # Only a step's own text and expected values are skipped: a nested {"target": {"id": ...}} is read.
    for key, value in step.items():
        if key != "words" and key not in VALUE_FIELDS:
            walk(value)
    return [bind(item, params) for item in found]


def is_secret_text(text):
    """An id, name or role that says password, PIN, passcode, secret, token, OTP or TOTP, or a code box."""
    listed = word_list(text)
    if set(listed) & SECRET_WORDS or any(SECRET_WORD.fullmatch(word) for word in listed):
        return True
    pairs = list(zip(listed, listed[1:]))
    return any(word == "code" and following in CODE_BOX_WORDS for word, following in pairs) or any(
        following == "code" and word in CODE_KIND_WORDS for word, following in pairs)


def is_send_step(step, params=None):
    """A step that would queue, send, submit or file a return: a press on a control so named, or such an invoke."""
    op = step.get("op", "")
    if op in PRESS_OPS or op == "invoke":
        return any(words(item) & SEND_WORDS for item in selectors(step, params))
    return False


def is_secret_entry(step, params=None, names=None):
    """A step that would type or key into a password, PIN, passcode, secret, token or code box, or press a key of the
    software keyboard. `names` maps a resolved id to the name and role its node has."""
    op = step.get("op", "")
    found = selectors(step, params)
    if op in PRESS_OPS | TYPING_OPS and any(item.startswith("system-keyboard") for item in found):
        return True
    if op not in TYPING_OPS:
        return False
    names = names or {}
    return any(is_secret_text(item) or any(is_secret_text(described) for resolved, described in names.items()
                                           if resolved and resolved in item)
               for item in found)


def is_sign_in_step(step, params=None):
    """An action on a sign-in screen (a press, key or typing on gate-* or auth-*: Enter in the email box sends the
    code), or Continue / Verify / Resend on one."""
    op = step.get("op")
    if op not in PRESS_OPS | TYPING_OPS:
        return False
    for item in selectors(step, params):
        listed = word_list(item)
        if "resend" in listed or (listed and listed[0] in SIGN_IN_PREFIXES):
            return True
        joined = set(listed) | ({"signin"} if any(a == "sign" and b == "in" for a, b in zip(listed, listed[1:]))
                                else set())
        if joined & SIGN_IN_WORDS and listed and listed[-1] in ("continue", "verify"):
            return True
    return False


def step_problems(steps, params, file, entry, names=None):
    """What a recipe's steps must not do, as sentences, with `params` bound."""
    problems = []
    for step in steps:
        what = step.get("target") or step.get("name") or step.get("op")
        if is_send_step(step, params) and not entry.get("send"):
            problems.append(f"{file} presses {what!r}, a send step, but the suite does not mark the recipe "
                            "\"send\": true")
        if is_secret_entry(step, params, names):
            problems.append(f"{file} types into or keys {what!r}: never a password, PIN, passcode, secret, token, "
                            "code box or the software keyboard")
        if is_sign_in_step(step, params) and not entry.get("sign_in"):
            problems.append(f"{file} presses {what!r} on a sign-in screen: only a recipe marked \"sign_in\": true, "
                            "against the local mock account server")
    return problems


def is_loopback_url(url):
    return bool(re.match(r"^https?://(localhost|127\.0\.0\.1|\[::1\])(:\d+)?(/|$)", url or ""))


def send_env_ok(env):
    """The environment of a dry-run test build: EBIR_TEST_ENV=1, BIR_SFTP_DRY_RUN=1, no BIR_SFTP_LIVE."""
    return env.get("EBIR_TEST_ENV") == "1" and env.get("BIR_SFTP_DRY_RUN") == "1" and not env.get("BIR_SFTP_LIVE")


def mock_account_params(first=MOCK_ACCOUNT_FIRST_CODE, count=MOCK_ACCOUNT_CODE_COUNT):
    """The invented sign-in fixtures a mock run exposes to recipes: two example.com accounts and the mock's first
    deterministic codes (`code_1` is the first request's code, counting up exactly as the mock does)."""
    params = {"account_a": "account-a@example.com", "account_b": "account-b@example.com"}
    for number in range(1, count + 1):
        params[f"code_{number}"] = f"{first + number - 1:06d}"
    return params


def launch_env(environ, token, addr, accounts="bypass"):
    """The environment the app is launched with: the dry-run test settings, the agent's address and token, none of
    the live SFTP or relay variables, and no account server that is not on this machine. `mock` leaves the account
    gate on and pins both account URLs to this run's loopback mock."""
    if accounts not in ("bypass", "mock"):
        raise SuiteError(f"unknown accounts mode {accounts!r}")
    if accounts == "mock":
        if environ.get("BIR_AUTH_BYPASS") == "1":
            raise SuiteError("--accounts mock refuses BIR_AUTH_BYPASS=1 in the environment: the app would open "
                             "without the account gate")
        for name in ACCOUNT_VARS:
            value = environ.get(name)
            if value and not is_loopback_url(value):
                raise SuiteError(f"--accounts mock refuses {name}={value}: the account server must be loopback "
                                 "(127.0.0.1, [::1] or localhost)")
    env = dict(environ)
    env.update({"EBIR_TEST_ENV": "1", "BIR_SFTP_DRY_RUN": "1", "BUWIZ_NO_KEYCHAIN": "1",
                "GPUI_AGENT_TOKEN": token, "GPUI_AGENT_ADDR": addr, ACCOUNTS_ENV: accounts})
    for name in LIVE_SEND_VARS:
        env.pop(name, None)
    if accounts == "mock":
        env.pop("BIR_AUTH_BYPASS", None)
        env[MOCK_ACCOUNT_URL_ENV] = MOCK_ACCOUNT_URL
        for name in ACCOUNT_VARS:
            env[name] = MOCK_ACCOUNT_URL
    else:
        env.pop(MOCK_ACCOUNT_URL_ENV, None)
        env["BIR_AUTH_BYPASS"] = "1"
        for name in ACCOUNT_VARS:
            if not is_loopback_url(env.get(name)):
                env.pop(name, None)
    return env


# --- Suites -------------------------------------------------------------------------------------------------------

VARIANT_KEYS = ("phone", "desktop") + TARGETS + ("file",)


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
    """The recipe file of `entry` for this host: the target's own (`windows`, `linux`, `android`, ...) when given, then
    `phone` / `desktop` by the form factor, then `file` (the same everywhere). None when the entry has no variant for
    it. Linux needs its own: its window opens 420 px wide, so the app shows the narrow layout on a desktop host."""
    if target in TARGETS and entry.get(target):
        return entry[target]
    if entry.get(form_factor):
        return entry[form_factor]
    return entry.get("file")


def entry_files(entry):
    names = [entry[key] for key in VARIANT_KEYS if entry.get(key)]
    if entry.get("fallback"):
        names += [value for value in entry["fallback"].values()] if isinstance(entry["fallback"], dict) else [
            entry["fallback"]]
    return names


def check_suite(suite, extra_params=None):
    """Every problem of `suite` that would stop it before a host sees it, as sentences (empty: it can run).
    `extra_params` are supplied by the runner itself (the mock sign-in fixtures), not by the suite."""
    problems = []
    base = suite["_dir"]
    runner_params = dict(extra_params or {})
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
            problems.append(f"{where}: names no recipe file (file, phone, desktop or a target)")
        supplied = set(params) | set(entry.get("set") or {}) | set(entry.get("resolve") or {}) | set(runner_params)
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
            bound = {**params, **(entry.get("set") or {}), **runner_params}
            for step in steps:
                if step.get("key_delay_ms") is not None and step.get("op") != "type":
                    problems.append(f"{where}: {file}: key_delay_ms on {step.get('op')} (only type is paced)")
            ids = step_ids(path)
            used_by_step = recipe_step_params(path)
            for param, spec in (entry.get("resolve") or {}).items():
                after = spec.get("after") if isinstance(spec, dict) else None
                if after is None:
                    continue
                if after not in ids:
                    problems.append(f"{where}: {file}: ${param} resolves after {after!r}, which is not a step id")
                    continue
                cut = ids.index(after) + 1
                for index in range(cut):
                    if param in used_by_step[index]:
                        problems.append(f"{where}: {file} uses ${param} at {ids[index]}, at or before its resolve's "
                                        f"after step {after}")
                        break
                if is_json_recipe(path):
                    for needed, dependent in needs_edges_cut(steps, ids, cut):
                        problems.append(f"{where}: {file}: ${param} resolves after {after!r}, cutting the needs edge "
                                        f"{needed} -> {dependent}")
            problems += [f"{where}: {problem}" for problem in step_problems(steps, bound, file, entry)]
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
    return resolve_node(spec, snapshot)[0]


def resolve_node(spec, snapshot):
    """(value, node) of [`resolve`]; (None, None) when no node matches."""
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
            return match.group(spec["group"]), node
        return node.get("id"), node
    return None, None


def redact_file(path, log):
    """`path` rewritten with every secret of `log` masked (a file another program wrote: the receipt, the app's log)."""
    try:
        text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return
    masked = log.redact(text)
    if masked != text:
        Path(path).write_text(masked, encoding="utf-8")


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


def resolve_param(param, spec, agent, log):
    """Resolve one suite parameter from a fresh snapshot, logging exactly what happened."""
    t0 = time.monotonic()
    node = None
    try:
        value, node = resolve_node(spec, agent.snapshot())
    except SuiteError as error:
        value, why = None, str(error)
    else:
        why = "no node matches " + json.dumps(spec)
    log.line("resolve", f"${param}", value or why, (time.monotonic() - t0) * 1000)
    return value, node, why


def write_recipe_slices(path, points, folder):
    """Write the slices of `path` cut after the given step positions into `folder`, with id maps back to the original."""
    ids = step_ids(path)
    endpoints = sorted(set(points))
    if not endpoints or endpoints[-1] < len(ids):
        endpoints.append(len(ids))
    json_recipe = is_json_recipe(path)
    if json_recipe:
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        source = data["steps"]
    else:
        source = [line.strip() for line in Path(path).read_text(encoding="utf-8").splitlines()
                  if line.strip() and not line.lstrip().startswith("#")]
    parts = []
    start = 0
    for number, end in enumerate(endpoints, 1):
        if end <= start:
            continue
        part_path = Path(folder) / f"slice-{number}{'.json' if json_recipe else '.wants'}"
        original_ids = ids[start:end]
        if json_recipe:
            part_steps = source[start:end]
            part = dict(data)
            part["steps"] = part_steps
            if isinstance(part.get("params"), list):
                used = set()
                for step in part_steps:
                    used |= placeholders_in_text(json.dumps(step), ids)
                part["params"] = [name for name in part["params"] if name in used]
            part_path.write_text(json.dumps(part, indent=1) + "\n", encoding="utf-8")
            local_ids = original_ids
        else:
            part_path.write_text("\n".join(source[start:end]) + "\n", encoding="utf-8")
            local_ids = [f"s{index}" for index in range(1, end - start + 1)]
        parts.append({"path": part_path, "start": start, "end": end, "number": number,
                      "ids": original_ids, "id_map": dict(zip(local_ids, original_ids))})
        start = end
    return parts


def run_recipe_file(agent, path, params, entry, folder, log, receipt_name="receipt.json", screenshot_dir=None):
    """Run one recipe file through gpui-agent and return its process result, parsed receipt and receipt path."""
    used = placeholders(path)
    receipt_path = Path(folder) / receipt_name
    args = ["recipe", "--schema", SCHEMA, "run", path]
    for key in sorted(used):
        args += ["--set", f"{key}={params[key]}"]
    args += ["--receipt-out", receipt_path, "--screenshot-dir", screenshot_dir or folder, "--screenshot-flagged"]
    if entry.get("yes"):
        args.append("--yes")
    code, stdout, stderr = agent.run(*args, timeout=entry.get("timeout_s", 900))
    redact_file(receipt_path, log)
    return code, stdout, stderr, first_json(stdout), receipt_path


def write_merged_receipt(path, parts, runs, folder):
    """One receipt for a split recipe, with every `.wants` step id mapped back to its original sN."""
    merged = {}
    fingerprints = []
    elapsed_ms = 0
    for run in runs:
        receipt = run["receipt"]
        if isinstance(receipt, dict):
            for key, value in receipt.items():
                if key not in ("steps", "ok", "recipe"):
                    merged[key] = value
            if receipt.get("fingerprint") is not None:
                fingerprints.append(receipt["fingerprint"])
            try:
                elapsed_ms += int(receipt.get("elapsed_ms") or 0)
            except (TypeError, ValueError):
                pass
    steps = []
    for run in runs:
        receipt = run["receipt"]
        if not isinstance(receipt, dict):
            continue
        for step in receipt.get("steps") or []:
            item = dict(step)
            item["id"] = run["part"]["id_map"].get(item.get("id"), item.get("id"))
            steps.append(item)
    merged.update(v=1, recipe=Path(path).name, steps=steps, fingerprints=fingerprints, elapsed_ms=elapsed_ms,
                  split=True,
                  ok=len(runs) == len(parts) and all(
                      run["code"] == 0 and isinstance(run["receipt"], dict) and run["receipt"].get("ok")
                      for run in runs))
    receipt_path = Path(folder) / "receipt.json"
    receipt_path.write_text(json.dumps(merged, indent=1), encoding="utf-8")
    return merged, receipt_path


def map_slice_stderr(path, part, stderr):
    """Map a dead slice's stderr back to the original recipe: `(text, original step id or None)`.

    gpui-agent reports `.wants` parse errors by line. A written slice has exactly one step on each non-comment line,
    so that local line number is the local step number; `part["id_map"]` takes either form back to the original id.
    """
    text = stderr or ""
    if not text:
        return text, None
    candidates = []
    for match in re.finditer(r"step\s+`([^`]+)`", text):
        candidates.append((match.start(), match.group(1), None))
    for match in re.finditer(r"\bstep\s+([A-Za-z0-9][\w:.-]*)\b", text):
        candidates.append((match.start(), match.group(1), None))
    if not str(part["path"]).endswith(".json"):
        for match in re.finditer(r"\bline\s+(\d+)\b", text):
            candidates.append((match.start(), f"s{int(match.group(1))}", match.group(1)))
    local = line = original_step = None
    for _, candidate, candidate_line in sorted(candidates, key=lambda item: item[0]):
        if candidate in part["id_map"]:
            local, line = candidate, candidate_line
            original_step = part["id_map"][candidate]
            break
    original_path = Path(path)
    text = text.replace(str(part["path"]), str(original_path))
    if part["path"].name != original_path.name:
        text = text.replace(part["path"].name, original_path.name)
    if original_step:
        if line is not None:
            text = re.sub(rf"\bline\s+{re.escape(line)}\b", f"step {original_step}", text)
        else:
            text = re.sub(rf"step\s+`{re.escape(local)}`", f"step {original_step}", text)
            text = re.sub(rf"\bstep\s+{re.escape(local)}\b", f"step {original_step}", text)
    return text, original_step


def capture_failure_evidence(result, folder, agent, log):
    """The failure screenshot and tree the report links to, when the host can give them."""
    shot = Path(folder) / "failure.png"
    for extra in (["--inline"], []):
        shot_code, _, shot_err = agent.run("screenshot", "--out", shot, *extra, timeout=60)
        if shot_code == 0 and shot.exists():
            result["screenshot"] = str(shot)
            break
    else:
        result["screenshot_error"] = log.redact(shot_err.strip())[-200:]
    try:
        (Path(folder) / "failure-tree.json").write_text(log.redact(json.dumps(agent.snapshot(), indent=1)),
                                                        encoding="utf-8")
        result["tree"] = str(Path(folder) / "failure-tree.json")
    except SuiteError:
        pass


def finish_recipe(result, entry, path, code, stdout, stderr, receipt, note, started, context, agent, log, folder,
                  step_hint=None):
    """Fill `result` from a recipe's terminal receipt, exactly as an unsplit run reports it."""
    result["ms"] = round((time.monotonic() - started) * 1000)
    if code == 0 and isinstance(receipt, dict) and receipt.get("ok"):
        result["status"] = "pass"
        log.line("recipe", entry["name"], "pass" + (f" ({note})" if note else ""), result["ms"])
        return result
    step, error, error_code = failing_step(receipt)
    if step is None:
        step = step_hint
    if not error:
        error = log.redact(stderr.strip() or stdout.strip() or f"exit {code}")[-500:]
    result.update(step=step, error=log.redact(error), error_code=error_code, line=wants_line_of(path, step))
    gap = for_host(entry.get("known_gap"), context)
    result["status"] = "known gap" if gap else "fail"
    if gap:
        result["known_gap"] = gap
    if entry.get("precondition"):
        result["note"] = "; ".join(filter(None, [note, f"precondition: {entry['precondition']}"]))
    capture_failure_evidence(result, folder, agent, log)
    log.line("recipe", entry["name"], f"{result['status']} at {step}: {error}", result["ms"])
    return result


# --- One recipe -----------------------------------------------------------------------------------------------------

def recipe_params(suite, entry, context):
    """The params one recipe runs with. Runner-supplied fixtures come last: a suite may not replace the invented
    example.com accounts or the mock's code sequence with a real address or another code."""
    return {**(suite.get("params") or {}), **(entry.get("set") or {}), **(context.get("params") or {})}


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
        if context.get("database") != "test":
            return None, "presses a send control: the app does not answer app.info with \"database\": \"test\"", \
                "refused"
    if entry.get("sign_in"):
        if not context.get("launched"):
            return None, "signs in: only in an app this run launched, against the local mock account server", \
                "refused"
        servers = [context["env"].get(name) for name in ACCOUNT_VARS]
        if any(server and not is_loopback_url(server) for server in servers):
            return None, "signs in: the account server is not on this machine", "refused"
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
    params = recipe_params(suite, entry, context)
    names = {}
    steps = recipe_steps(path)
    ids = step_ids(path)
    early = []
    late = {}
    for param, spec in (entry.get("resolve") or {}).items():
        after = spec.get("after") if isinstance(spec, dict) else None
        if after is None:
            early.append((param, spec))
            continue
        if after not in ids:
            raise SuiteError(f"{path}: ${param} resolves after {after!r}, which is not a step id")
        late.setdefault(ids.index(after) + 1, []).append((param, spec, after))
    for param, spec in early:
        value, node, why = resolve_param(param, spec, agent, log)
        if node:
            names[value] = f"{node.get('id', '')} {node.get('name', '')} {node.get('role', '')}"
        if not value:
            result.update(status="setup", step=None, error=f"${param}: {why}")
            result["ms"] = round((time.monotonic() - started) * 1000)
            return result
        params[param] = value
    # The suite was checked before the run; what a resolved id names is known only now.
    problems = step_problems(steps, params, file, entry, names)
    if problems:
        result.update(status="refused", error="; ".join(problems))
        log.line("recipe", entry["name"], f"refused: {result['error']}", 0)
        return result
    recorder = context["recorder"](folder) if context.get("recorder") else None
    if not late:
        code, stdout, stderr, receipt, _ = run_recipe_file(agent, path, params, entry, folder, log)
        if recorder:
            result["video"] = recorder.stop()
        return finish_recipe(result, entry, path, code, stdout, stderr, receipt, note, started, context, agent, log,
                             folder)

    parts = write_recipe_slices(path, sorted(late), folder)
    runs = []
    terminal = None
    for part in parts:
        number = part["number"]
        code, stdout, stderr, receipt, receipt_path = run_recipe_file(
            agent, part["path"], params, entry, folder, log, receipt_name=f"slice-{number}-receipt.json",
            screenshot_dir=folder / f"slice-{number}-shots")
        run = {"part": part, "code": code, "stdout": stdout, "stderr": stderr, "receipt": receipt,
               "receipt_path": receipt_path}
        runs.append(run)
        if not (code == 0 and isinstance(receipt, dict) and receipt.get("ok")):
            terminal = run
            break
        for param, spec, after in late.get(part["end"], []):
            value, node, why = resolve_param(param, spec, agent, log)
            if node:
                names[value] = f"{node.get('id', '')} {node.get('name', '')} {node.get('role', '')}"
            if not value:
                if recorder:
                    result["video"] = recorder.stop()
                merged, merged_path = write_merged_receipt(path, parts, runs, folder)
                redact_file(merged_path, log)
                result.update(status="setup", step=after, error=f"${param}: after {after}: {why}",
                              line=wants_line_of(path, after))
                result["ms"] = round((time.monotonic() - started) * 1000)
                capture_failure_evidence(result, folder, agent, log)
                log.line("recipe", entry["name"], f"setup at {after}: {result['error']}", result["ms"])
                return result
            params[param] = value
            problems = step_problems(steps, params, file, entry, names)
            if problems:
                if recorder:
                    result["video"] = recorder.stop()
                result.update(status="refused", error="; ".join(problems))
                result["ms"] = round((time.monotonic() - started) * 1000)
                log.line("recipe", entry["name"], f"refused: {result['error']}", 0)
                return result
    if terminal is None:
        terminal = runs[-1]
    step_hint = None
    if terminal["receipt"] is None:
        mapped_stderr, step_hint = map_slice_stderr(path, terminal["part"], terminal["stderr"])
        terminal = {**terminal, "stderr": mapped_stderr}
    if recorder:
        result["video"] = recorder.stop()
    merged, merged_path = write_merged_receipt(path, parts, runs, folder)
    redact_file(merged_path, log)
    return finish_recipe(result, entry, path, terminal["code"], terminal["stdout"], terminal["stderr"], merged, note,
                         started, context, agent, log, folder, step_hint=step_hint)


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


def start_mock_account(just, env, out, log, timeout_s, popen=None, answers=None, sleep=None):
    """Start the repository's own in-memory account mock on its fixed loopback port and wait for it.

    The command is the same one the Android development scripts use (`just mock-account --auto-approve`), with the
    loopback bind and port made explicit so `.env` cannot move it. The mock offers no health or version route, so a
    port that is already taken is refused outright instead of trusting an unidentified listener.
    """
    popen = subprocess.Popen if popen is None else popen
    answers = port_answers if answers is None else answers
    sleep = time.sleep if sleep is None else sleep
    if answers(MOCK_ACCOUNT_ADDR):
        raise SuiteError(f"--accounts mock: {MOCK_ACCOUNT_ADDR} already listens, and mock_account has no health or "
                         "version route by which the listener can be identified; stop it and rerun")
    command = [just, "mock-account", "--auto-approve", "--bind", "127.0.0.1", "--port", str(MOCK_ACCOUNT_PORT)]
    kwargs = {"cwd": ROOT, "env": env, "stderr": subprocess.STDOUT}
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        kwargs["start_new_session"] = True
    out = Path(out)
    try:
        with (out / "mock-account.log").open("w", encoding="utf-8") as mock_log:
            kwargs["stdout"] = mock_log
            proc = popen(command, **kwargs)
    except OSError as error:
        raise SuiteError(f"--accounts mock: cannot start {' '.join(command)}: {error}") from None
    log.line("mock-account", " ".join(command[1:]), "started", 0)
    t0 = time.monotonic()
    try:
        while not answers(MOCK_ACCOUNT_ADDR):
            if proc.poll() is not None:
                raise SuiteError(f"--accounts mock: mock_account ended (exit {proc.returncode}) before listening "
                                 f"on {MOCK_ACCOUNT_ADDR}; see {out / 'mock-account.log'}")
            if time.monotonic() - t0 >= timeout_s:
                raise SuiteError(f"--accounts mock: mock_account did not listen on {MOCK_ACCOUNT_ADDR} in "
                                 f"{timeout_s} s; see {out / 'mock-account.log'}")
            sleep(1)
    except SuiteError:
        stop_mock_account(proc, log)
        raise
    log.line("mock-account", MOCK_ACCOUNT_ADDR, "listening", (time.monotonic() - t0) * 1000)
    return proc


def android_mock_reverse(env, log):
    """Point the emulator's loopback mock port at this machine's, the way scripts/run-android-windows.ps1 does."""
    adb = android_adb(env)
    if not adb:
        raise SuiteError("--accounts mock on android: no adb: set ANDROID_HOME")
    serial = env.get("ANDROID_SERIAL") or android_serial(adb)
    if not serial:
        raise SuiteError("--accounts mock on android: no Android emulator is online (adb devices)")
    t0 = time.monotonic()
    port = f"tcp:{MOCK_ACCOUNT_PORT}"
    done = subprocess.run([adb, "-s", serial, "reverse", port, port], capture_output=True, text=True, timeout=30)
    if done.returncode:
        raise SuiteError(f"--accounts mock on android: adb reverse {port} failed: "
                         f"{(done.stderr or done.stdout).strip() or 'no error text'}")
    log.line("mock-account", f"{serial} reverse {port}", "added", (time.monotonic() - t0) * 1000)
    return adb, serial


def remove_android_mock_reverse(reverse, log):
    """Remove only the reverse entry this run added."""
    adb, serial = reverse
    t0 = time.monotonic()
    port = f"tcp:{MOCK_ACCOUNT_PORT}"
    try:
        subprocess.run([adb, "-s", serial, "reverse", "--remove", port], capture_output=True, timeout=30)
        log.line("mock-account", f"{serial} reverse {port}", "removed", (time.monotonic() - t0) * 1000)
    except (OSError, subprocess.SubprocessError) as error:
        log.line("mock-account", f"{serial} reverse {port}", f"not removed: {error}", 0)


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


def stop(proc, log, what="app"):
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
    log.line("stop", what, "stopped", (time.monotonic() - t0) * 1000)


def stop_mock_account(proc, log):
    stop(proc, log, "mock-account")


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

def check(suite_name, cli, accounts="bypass"):
    """Every recipe of the suite checked without an app: the suite's own rules, then `gpui-agent recipe validate` for
    each file the installed client can read. Exit 0 when nothing is wrong."""
    suite = load_suite(suite_name)
    problems = check_suite(suite, extra_params=mock_account_params() if accounts == "mock" else None)
    cli_version = installed_cli_version(cli)
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
    parser.add_argument("--accounts", choices=("bypass", "mock"), default="bypass",
                        help="bypass the account gate (default), or run signed out against the loopback mock")
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
            return check(args.suite, args.cli, args.accounts)
        return run(args)
    except SuiteError as error:
        print(f"e2e: {error}", file=sys.stderr)
        return 2


def run(args):
    suite = load_suite(args.suite)
    account_params = mock_account_params() if args.accounts == "mock" else {}
    problems = check_suite(suite, extra_params=account_params)
    if problems:
        raise SuiteError("the suite cannot run:\n  " + "\n  ".join(problems))
    stamp = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    out = Path(args.out) if args.out else Path.home() / "BuwizNsh" / "evidence" / "e2e" / f"{args.target}-{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    token = os.environ.get("GPUI_AGENT_TOKEN") or secrets.token_hex(16)
    addr = args.addr or os.environ.get("GPUI_AGENT_ADDR") or DEFAULT_ADDR
    # What the app (and every gpui-agent call) gets: a dry-run test build, no live SFTP or relay settings, no remote
    # account server. nsh-run-agent sets and unsets the same again after `just` reads .env.
    env = launch_env(os.environ, token, addr, accounts=args.accounts)
    secret_values = [token] + [os.environ[name] for name in ("BIR_SFTP_PASSWORD", "BIR_SFTP_USERNAME")
                               if os.environ.get(name)]
    log = RunLog(out / "run.log", secrets_=secret_values)
    header = {"suite": suite["_path"].stem, "target": args.target, "started": stamp, "out": str(out),
              "cli_version": installed_cli_version(args.cli), "app_commit": git_head()}
    proc = None
    mock_proc = None
    mock_reverse = None
    results = []
    made_db = None
    try:
        if not args.no_launch and (port_answers(addr) or port_answers(data_plane(addr))):
            raise SuiteError(f"something already listens on {addr} or {data_plane(addr)}: stop it, or pass "
                             "--no-launch to drive it")
        if args.accounts == "mock":
            mock_proc = start_mock_account(args.just, env, out, log, args.launch_timeout)
            if args.target == "android":
                mock_reverse = android_mock_reverse(env, log)
        if not args.no_launch:
            db = Path(args.db) if args.db else Path.home() / "BuwizNsh" / "agents" / f"e2e-{args.target}-{stamp}.db"
            made_db = None if args.db else db
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
        header["database"] = app_database(data, log) if header["data_plane"] else None
        if args.no_launch and header["database"] != "test":
            raise SuiteError(f"the app on {addr} does not answer app.info with \"database\": \"test\" "
                             f"({header['database'] or 'no answer'}): drive only a test build on a scratch database")
        context = {"target": args.target, "form_factor": form_factor, "features": features,
                   "env": os.environ if args.no_launch else env, "launched": not args.no_launch,
                   "database": header["database"], "params": account_params,
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
        if proc is not None:
            close_device(args.target, addr, env, log)
        if mock_reverse:
            remove_android_mock_reverse(mock_reverse, log)
        stop_mock_account(mock_proc, log)
        redact_file(out / "app.log", log)
        redact_file(out / "mock-account.log", log)
        write_report(out, header, results)
        if made_db:
            remove_scratch(made_db, log)
    print(f"e2e {header['suite']} on {args.target}: {summary(results)}. Report: {out / 'report.md'}")
    return 0 if all(r["status"] in ("pass", "skipped") for r in results) else 1


def app_database(data, log):
    """What the app's data plane says its database is (`app.info` -> `database`: `test` or `real`), or None."""
    t0 = time.monotonic()
    code, reply, err = data.json("invoke", "app.info", timeout=30)

    def find(value):
        if isinstance(value, dict):
            if isinstance(value.get("database"), str):
                return value["database"]
            return next((found for found in map(find, value.values()) if found), None)
        if isinstance(value, list):
            return next((found for found in map(find, value) if found), None)
        return None

    kind = find(reply) if code == 0 else None
    log.line("app.info", "database", kind or f"no answer: {err[-200:]}", (time.monotonic() - t0) * 1000)
    return kind


def close_device(target, addr, env, log):
    """The app stopped on the Simulator or the emulator too, and the emulator's forwarded ports removed: the next run
    must not find this one's control plane."""
    t0 = time.monotonic()
    commands = []
    if target == "ios":
        commands.append(["xcrun", "simctl", "terminate", booted_simulator() or "booted", ANDROID_HOST])
    elif target == "android":
        adb = android_adb(env)
        if adb:
            port = int(addr.rpartition(":")[2])
            commands += [[adb, "shell", "am", "force-stop", ANDROID_HOST],
                         [adb, "forward", "--remove", f"tcp:{port}"], [adb, "forward", "--remove", f"tcp:{port + 1}"]]
    for command in commands:
        try:
            subprocess.run(command, capture_output=True, timeout=30)
        except (OSError, subprocess.SubprocessError):
            pass
    if commands:
        log.line("close", target, "the app stopped on the device", (time.monotonic() - t0) * 1000)


def booted_simulator():
    """The UDID of the one booted Simulator, or None (none, several, or no simctl)."""
    try:
        listed = json.loads(subprocess.run(["xcrun", "simctl", "list", "devices", "booted", "-j"],
                                           capture_output=True, text=True, timeout=30).stdout or "{}")
    except (OSError, subprocess.SubprocessError, ValueError):
        return None
    booted = [device.get("udid") for devices in (listed.get("devices") or {}).values() for device in devices
              if device.get("state") == "Booted"]
    return booted[0] if len(booted) == 1 else None


def remove_scratch(db, log):
    """The run's scratch database and its SQLite side files; the evidence stays. On Windows the app's grandchildren
    can still hold the files for a moment after the group break (there is no process-tree kill): retry for a few
    seconds before reporting a file as not removed."""
    removed = True
    for path in (db, Path(f"{db}-wal"), Path(f"{db}-shm"), Path(f"{db}-journal")):
        deadline = time.monotonic() + 20
        while True:
            try:
                path.unlink()
                break
            except FileNotFoundError:
                break
            except OSError as error:
                if os.name != "nt" or time.monotonic() >= deadline:
                    removed = False
                    log.line("clean", path, f"not removed: {error}", 0)
                    break
                time.sleep(0.5)
    if removed:
        log.line("clean", db, "scratch database removed", 0)


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
