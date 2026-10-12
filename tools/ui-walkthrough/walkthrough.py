#!/usr/bin/env python3
"""Repeatable UI walkthrough for the bir desktop app on Linux.

Starts the app on its own Xvfb display with a private HOME / XDG_DATA_HOME,
drives it through the app's gpui-agent automation interface (nav.go,
profile.set, form.open, dashboard.tab, clicks on tab ids), screenshots every
top-level page with ImageMagick `import`, and writes an index.html contact
sheet. Scrolling (Settings, Dev Mode) uses xdotool mouse-wheel events, since
the agent has no scroll op for those pages on Linux.

Exit codes: 0 all pages captured, 1 a page could not be reached or captured,
2 the app crashed or never came up, 3 bad arguments / missing tools.

See README.md for usage and the environment variables.
"""

import argparse
import hashlib
import hmac
import html
import json
import os
import secrets
import shlex
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PROTOCOL_VERSION = 2
# Same values the bir-headless tests use (valid TIN check digit).
DEMO_PROFILE = [
    ("profile-tin", "12345678800002"),
    ("profile-name", "Walkthrough Demo Taxpayer"),
    ("profile-rdo", "018"),
    ("profile-lob", "Retail"),
    ("profile-address", "Manila"),
    ("profile-zip", "1000"),
    ("profile-phone", "09170000000"),
    ("profile-email", "walkthrough@example.com"),
]
REPO = Path(__file__).resolve().parents[2]


def env(name, default=""):
    value = os.environ.get(name)
    return value if value not in (None, "") else default


def log(msg):
    print(f"[walkthrough] {msg}", flush=True)


class Crash(Exception):
    pass


class Unreachable(Exception):
    pass


# --------------------------------------------------------------------------
# gpui-agent protocol v2 client (NDJSON over loopback TCP, HMAC challenge)
# --------------------------------------------------------------------------
class AgentClient:
    def __init__(self, addr, token, timeout=20.0):
        host, port = addr.rsplit(":", 1)
        self.sock = socket.create_connection((host, int(port)), timeout=timeout)
        self.file = self.sock.makefile("rwb")
        self.auth = None
        self.seq = 0
        if token:
            line = self.file.readline()
            if not line:
                raise ConnectionError("connection closed before challenge")
            challenge = json.loads(line)
            if challenge.get("op") != "challenge":
                raise ConnectionError(f"expected challenge, got {challenge!r}")
            nonce = bytes.fromhex(challenge["nonce"])
            self.auth = hmac.new(token.encode(), nonce, hashlib.sha256).hexdigest()

    def request(self, op, **fields):
        self.seq += 1
        req = {"v": PROTOCOL_VERSION, "id": str(self.seq), "op": op}
        if self.auth:
            req["auth"] = self.auth
        req.update(fields)
        self.file.write((json.dumps(req) + "\n").encode())
        self.file.flush()
        line = self.file.readline()
        if not line:
            raise ConnectionError(f"connection closed during {op}")
        return json.loads(line)

    def invoke(self, name, **args):
        resp = self.request("invoke", name=name, args=args)
        if not resp.get("ok"):
            raise Unreachable(f"{name} {args}: {resp.get('error')}")
        return resp.get("result")

    def click(self, target):
        resp = self.request("click", target=target)
        if not resp.get("ok"):
            raise Unreachable(f"click {target}: {resp.get('error')}")

    def snapshot(self):
        resp = self.request("snapshot")
        if not resp.get("ok"):
            raise Unreachable(f"snapshot: {resp.get('error')}")
        return resp.get("tree") or {}

    def close(self):
        try:
            self.sock.close()
        except OSError:
            pass


def walk_nodes(nodes):
    for node in nodes:
        yield node
        yield from walk_nodes(node.get("children", []))


def find_node(tree, node_id):
    for node in walk_nodes(tree.get("nodes", [])):
        if node.get("id") == node_id:
            return node
    return None


# --------------------------------------------------------------------------
# Process / display helpers
# --------------------------------------------------------------------------
def free_display(start=110):
    for n in range(start, start + 400):
        if not Path(f"/tmp/.X{n}-lock").exists() and not Path(f"/tmp/.X11-unix/X{n}").exists():
            return n
    raise RuntimeError("no free X display number")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def tool_env(extra_ld):
    e = dict(os.environ)
    if extra_ld:
        e["LD_LIBRARY_PATH"] = extra_ld + (":" + e["LD_LIBRARY_PATH"] if e.get("LD_LIBRARY_PATH") else "")
    return e


class Walkthrough:
    def __init__(self, args):
        self.args = args
        self.out = Path(args.out).resolve()
        self.out.mkdir(parents=True, exist_ok=True)
        self.shots = []  # (file, title, note)
        self.failures = []
        self.layout = []
        self.xvfb = None
        self.app = None
        self.client = None
        self.display = None
        self.xdotool = shlex.split(env("WALK_XDOTOOL", "xdotool"))
        self.xdotool_env = tool_env(env("WALK_XDOTOOL_LD_LIBRARY_PATH"))
        self.import_cmd = shlex.split(env("WALK_IMPORT", "import"))
        self.width, self.height = args.size.split("x")

    # ---- setup ---------------------------------------------------------
    def start_xvfb(self):
        n = self.args.display or free_display()
        self.display = f":{n}"
        cmd = [env("WALK_XVFB", "Xvfb"), self.display, "-screen", "0",
               f"{self.width}x{self.height}x24", "-nolisten", "tcp", "-ac"]
        log(f"Xvfb on {self.display} ({self.args.size})")
        self.xvfb = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            if Path(f"/tmp/.X11-unix/X{n}").exists():
                return
            if self.xvfb.poll() is not None:
                raise Crash(f"Xvfb exited with {self.xvfb.returncode}")
            time.sleep(0.1)
        raise Crash("Xvfb socket never appeared")

    def prepare_home(self):
        root = Path(tempfile.mkdtemp(prefix="bir-walk-", dir=env("WALK_TMPDIR") or None))
        self.root = root
        self.home = root / "home"
        self.xdg = root / "xdg-data"
        for d in (self.home, self.xdg, root / "xdg-config", root / "xdg-cache"):
            d.mkdir(parents=True)
        if self.args.data_from:
            src = Path(self.args.data_from)
            dst = self.home / ".taxman-ebir"
            dst.mkdir()
            copied = []
            for name in ("bir_data.db", "bir_data.db-wal", "bir_data.db-shm", "bir_key.txt"):
                if (src / name).is_file():
                    shutil.copy2(src / name, dst / name)
                    copied.append(name)
            if "bir_data.db" not in copied:
                raise SystemExit(f"--data-from {src}: no bir_data.db there")
            log(f"seeded {dst} with {', '.join(copied)}")
        log(f"private HOME {self.home}, XDG_DATA_HOME {self.xdg}")

    def start_app(self):
        binary = self.args.bin
        if not Path(binary).is_file():
            raise SystemExit(f"app binary not found: {binary} (build with --features agent, or set WALK_BIN)")
        self.addr = f"127.0.0.1:{free_port()}"
        self.token = secrets.token_hex(16)
        e = dict(os.environ)
        for k in ("WAYLAND_DISPLAY", "BIR_DATABASE_PATH"):
            e.pop(k, None)
        e.update({
            "DISPLAY": self.display,
            "HOME": str(self.home),
            "XDG_DATA_HOME": str(self.xdg),
            "XDG_CONFIG_HOME": str(self.root / "xdg-config"),
            "XDG_CACHE_HOME": str(self.root / "xdg-cache"),
            "GPUI_AGENT": "1",
            "GPUI_AGENT_ADDR": self.addr,
            "GPUI_AGENT_TOKEN": self.token,
            # Links and folders the app tries to open land in a log, not a browser.
            "BROWSER": "true",
        })
        if env("WALK_APP_LD_LIBRARY_PATH"):
            e["LD_LIBRARY_PATH"] = env("WALK_APP_LD_LIBRARY_PATH")
        if env("WALK_APP_LD_PRELOAD"):
            e["LD_PRELOAD"] = env("WALK_APP_LD_PRELOAD")
        if env("WALK_VK_ICD_FILENAMES"):
            e["VK_ICD_FILENAMES"] = env("WALK_VK_ICD_FILENAMES")
        if env("WALK_EMULATED_GPU", "1") == "1":
            e["ZED_ALLOW_EMULATED_GPU"] = "1"
            e["MESA_SHADER_CACHE_DISABLE"] = "true"
        if env("WALK_APP_PATH_PREPEND"):
            e["PATH"] = env("WALK_APP_PATH_PREPEND") + ":" + e["PATH"]
        self.app_log = self.out / "app.log"
        log(f"starting {binary} (agent on {self.addr}; log {self.app_log})")
        self.app = subprocess.Popen([binary], env=e, stdin=subprocess.DEVNULL,
                                    stdout=open(self.app_log, "wb"), stderr=subprocess.STDOUT,
                                    start_new_session=True)
        deadline = time.time() + self.args.startup_timeout
        last = None
        while time.time() < deadline:
            self.check_alive()
            try:
                self.client = AgentClient(self.addr, self.token)
                hello = self.client.request("hello")
                if hello.get("ok"):
                    log("agent answered hello")
                    return
                last = hello.get("error")
            except (OSError, ConnectionError, ValueError) as err:
                last = err
                if self.client:
                    self.client.close()
                self.client = None
            time.sleep(0.5)
        raise Crash(f"agent never answered on {self.addr} ({last}); is the binary built with --features agent?")

    def check_alive(self):
        if self.app is not None and self.app.poll() is not None:
            raise Crash(f"app exited with code {self.app.returncode} (see {self.app_log})")

    def fit_window(self):
        """Move the app window to 0,0 and size it to the screen (no WM on Xvfb)."""
        try:
            wids = subprocess.run(self.xdotool + ["search", "--onlyvisible", "--pid", str(self.app.pid)],
                                  env=self.xdo_env(), capture_output=True, text=True, timeout=10).stdout.split()
            for wid in wids:
                subprocess.run(self.xdotool + ["windowmove", wid, "0", "0",
                                               "windowsize", wid, self.width, self.height],
                               env=self.xdo_env(), capture_output=True, timeout=10)
            if wids:
                log(f"window {wids[0]} fitted to {self.args.size}")
                time.sleep(1.0)
        except (OSError, subprocess.TimeoutExpired) as err:
            log(f"window fit skipped: {err}")

    def xdo_env(self):
        e = dict(self.xdotool_env)
        e["DISPLAY"] = self.display
        return e

    # ---- primitives ----------------------------------------------------
    def wait_for(self, node_id, timeout=None):
        deadline = time.time() + (timeout or self.args.page_timeout)
        tree = {}
        while time.time() < deadline:
            self.check_alive()
            tree = self.client.snapshot()
            if find_node(tree, "overlay-admin-auth"):
                raise Unreachable("admin PIN/TOTP overlay is up; use data without an app lock")
            if find_node(tree, "overlay-profile-auth"):
                raise Unreachable("profile PIN overlay is up; use a profile without a PIN")
            node = find_node(tree, node_id)
            if node and node.get("visible", True):
                return tree
            time.sleep(0.3)
        raise Unreachable(f"`{node_id}` never appeared")

    def scroll(self, tree, notches):
        bounds = None
        for node in walk_nodes(tree.get("nodes", [])):
            if node.get("id", "").startswith("page-") and node.get("bounds", {}).get("w"):
                bounds = node["bounds"]
        if bounds:
            x = int(bounds["x"] + bounds["w"] / 2)
            y = int(bounds["y"] + bounds["h"] / 2)
        else:
            x, y = int(self.width) // 2 + 120, int(self.height) // 2
        cmd = self.xdotool + ["mousemove", str(x), str(y)] + ["click", "--repeat", str(notches), "--delay", "40", "5"]
        subprocess.run(cmd, env=self.xdo_env(), capture_output=True, timeout=60, check=True)
        time.sleep(1.0)

    def screenshot(self, title, note=""):
        self.check_alive()
        time.sleep(self.args.settle)
        n = len(self.shots) + 1
        slug = "".join(c if c.isalnum() else "_" for c in title.lower()).strip("_")
        name = f"{n:02d}_{slug}.png"
        path = self.out / name
        subprocess.run(self.import_cmd + ["-display", self.display, "-window", "root", str(path)],
                       env=self.xdo_env(), capture_output=True, timeout=60, check=True)
        self.shots.append((name, title, note))
        log(f"  -> {name}")

    def record_layout(self, title, tree):
        window = (find_node(tree, "bir-window") or {}).get("bounds")
        entry = {"page": title, "window": window, "issues": []}
        if window and window.get("w"):
            for node in walk_nodes(tree.get("nodes", [])):
                b = node.get("bounds") or {}
                if not b.get("w") or node.get("id") == "bir-window" or not node.get("visible", True):
                    continue
                if b["x"] < -0.5 or b["y"] < -0.5 or b["x"] + b["w"] > window["w"] + 0.5 or b["y"] + b["h"] > window["h"] + 0.5:
                    entry["issues"].append({"id": node.get("id"), "bounds": b})
        self.layout.append(entry)

    # ---- the walk ------------------------------------------------------
    def step(self, title, fn):
        log(title)
        try:
            fn()
        except Unreachable as err:
            log(f"  FAILED: {err}")
            self.failures.append(f"{title}: {err}")
            try:
                self.screenshot(f"{title} (failed)", str(err))
            except Exception:  # noqa: BLE001 - best-effort evidence
                pass

    def nav(self, slug, page_id, title, note=""):
        self.client.invoke("nav.go", page=slug)
        tree = self.wait_for(page_id)
        self.record_layout(title, tree)
        self.screenshot(title, note)
        return tree

    def notifications_drawer(self):
        """Notifications is a drawer over the current page, not a page.

        The agent tree has no node for the overlay, so open it through
        navigation, give it a moment to slide in, screenshot it, then close it
        by toggling the bell so later pages aren't covered.
        """
        self.client.invoke("nav.go", page="notifications")
        self.check_alive()
        time.sleep(1.5)
        tree = self.client.snapshot()
        if not find_node(tree, "notifications_bell_btn"):
            raise Unreachable("`notifications_bell_btn` (top strip) never appeared")
        self.screenshot("Notifications", "drawer over the current page")
        # Close it by toggling the bell through the agent (a key press can miss
        # the window on a bare Xvfb display with no window manager to focus it).
        self.client.click("notifications_bell_btn")
        time.sleep(0.5)
        return tree

    def check_seed(self):
        """Fail when the app could not read the seeded DB and quarantined it."""
        if not self.args.data_from:
            return
        quarantined = sorted((self.home / ".taxman-ebir").glob("bir_data.db.corrupt-*.bak"))
        if quarantined:
            msg = (f"the seeded database could not be decrypted and was quarantined as "
                   f"{quarantined[-1].name}; the app is running on an empty DB "
                   f"(no persistent keyring for the SQLCipher key, see README)")
            log(f"SEED FAILED: {msg}")
            self.failures.append(f"--data-from: {msg}")

    def list_profiles(self):
        result = self.client.invoke("profile.list")
        if isinstance(result, dict):
            result = result.get("profiles", [])
        return [p for p in (result or []) if not p.get("archived") and not p.get("is_archived")]

    def create_demo_profile(self):
        c = self.client
        c.invoke("profile.create")
        for target, value in DEMO_PROFILE:
            resp = c.request("set_value", target=target, value=value)
            if not resp.get("ok"):
                raise Unreachable(f"demo profile {target}: {resp.get('error')}")
        c.invoke("profile.save")
        log(f"created demo profile {DEMO_PROFILE[0][1]}")
        # profile.save reloads the saved row into the editor, so no
        # unsaved-changes banner is left to block navigation.
        resp = c.request("invoke", name="nav.go", args={"page": "global-dashboard"})
        if not resp.get("ok"):
            raise Unreachable(f"demo profile saved but navigation is blocked: {resp.get('error')}")

    def pick_profile(self):
        if self.args.tin:
            return self.args.tin
        profiles = self.list_profiles()
        if not profiles and self.args.demo_profile:
            self.step("Create demo profile", self.create_demo_profile)
            profiles = self.list_profiles()
        return profiles[0].get("tin") if profiles else None

    def run(self):
        c = self.client
        self.step("Global Dashboard", lambda: self.nav("global-dashboard", "page-global-dashboard", "Global Dashboard"))

        tin = self.pick_profile()
        if tin:
            log(f"profile {tin}")

            def dashboard(tab, title):
                c.invoke("profile.set", tin=tin, view="dashboard")
                self.wait_for("page-dashboard")
                c.invoke("dashboard.tab", tab=tab)
                time.sleep(0.5)
                tree = self.wait_for("page-dashboard")
                self.record_layout(title, tree)
                self.screenshot(title, f"TIN {tin}")

            self.step("Dashboard - Calendar", lambda: dashboard("calendar", "Dashboard - Calendar"))
            self.step("Dashboard - Tax Form Library", lambda: dashboard("forms", "Dashboard - Tax Form Library"))
        else:
            self.failures.append("profile dashboard: the database has no active profile")
            log("no profile: skipping profile dashboard and forms")

        self.step("Import Data", lambda: self.nav("import-export", "page-import-export", "Import Data"))
        self.step("Notifications", self.notifications_drawer)

        def cron(tab_id, title):
            c.invoke("nav.go", page="cron-tasks")
            self.wait_for("page-cron-tasks")
            c.click(tab_id)
            tree = self.wait_for("page-cron-tasks")
            self.record_layout(title, tree)
            self.screenshot(title)

        self.step("Background Tasks - Jobs", lambda: cron("jobs_tab", "Background Tasks - Jobs"))
        self.step("Background Tasks - Logs", lambda: cron("logs_tab", "Background Tasks - Logs"))
        self.step("Tax Calendars", lambda: self.nav("admin-calendar", "page-admin-calendar", "Tax Calendars"))

        def scrolled(slug, page_id, title):
            tree = self.nav(slug, page_id, title)
            self.scroll(tree, self.args.scroll_notches)
            self.screenshot(f"{title} (end)", f"{self.args.scroll_notches} wheel notches via xdotool")

        self.step("Settings", lambda: scrolled("settings", "page-settings", "Settings"))
        if not self.args.no_dev_mode:
            self.step("Dev Mode", lambda: scrolled("dev-mode", "page-dev-mode", "Dev Mode"))

        if tin:
            for code, page_id, period in self.args.forms:
                def open_form(code=code, page_id=page_id, period=period):
                    args = {"code": code, "tin": tin, "period": period}
                    if self.args.year:
                        args["year"] = self.args.year
                    resp = c.request("invoke", name="form.open", args=args)
                    if not resp.get("ok"):
                        if "unknown form" not in str(resp.get("error")):
                            raise Unreachable(f"form.open {args}: {resp.get('error')}")
                        # Event-kit (queueable) forms are not in form.open's
                        # table; nav.go on their slug opens them for the
                        # selected taxpayer (current year, period 1).
                        c.invoke("profile.set", tin=tin, view="dashboard")
                        self.wait_for("page-dashboard")
                        c.invoke("nav.go", page=code.lower())
                    tree = self.wait_for(page_id)
                    self.record_layout(f"Form {code}", tree)
                    self.screenshot(f"Form {code}", f"TIN {tin}, period {period}")
                self.step(f"Form {code}", open_form)
            # Leave the last form so the next run does not start inside it.
            try:
                c.invoke("nav.go", page="global-dashboard")
            except Unreachable:
                pass

    # ---- output --------------------------------------------------------
    def write_index(self):
        cards = []
        for name, title, note in self.shots:
            cards.append(
                f'<figure><a href="{html.escape(name)}"><img src="{html.escape(name)}" loading="lazy"></a>'
                f"<figcaption><b>{html.escape(name)}</b> {html.escape(title)}"
                f"{'<br><small>' + html.escape(note) + '</small>' if note else ''}</figcaption></figure>")
        fails = "".join(f"<li>{html.escape(f)}</li>" for f in self.failures) or "<li>none</li>"
        page = f"""<!doctype html><meta charset="utf-8"><title>bir UI walkthrough</title>
<style>body{{font:14px sans-serif;margin:16px;background:#f4f4f5}}
.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(420px,1fr));gap:12px}}
figure{{margin:0;background:#fff;border:1px solid #ddd;padding:6px}}img{{width:100%;display:block}}</style>
<h1>bir UI walkthrough</h1>
<p>{time.strftime('%Y-%m-%d %H:%M:%S')} &middot; binary <code>{html.escape(self.args.bin)}</code> &middot; display {self.args.size}</p>
<h2>Failures</h2><ul>{fails}</ul>
<div class="grid">{''.join(cards)}</div>
"""
        (self.out / "index.html").write_text(page)
        (self.out / "layout.json").write_text(json.dumps(self.layout, indent=2))
        (self.out / "pages.txt").write_text("".join(f"{n}\t{t}\n" for n, t, _ in self.shots))

    def shutdown(self):
        if self.client:
            try:
                self.client.request("shutdown")
            except Exception:  # noqa: BLE001
                pass
            self.client.close()
        if self.app and self.app.poll() is None:
            try:
                self.app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(self.app.pid, 15)
                try:
                    self.app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(self.app.pid, 9)
        if self.xvfb and self.xvfb.poll() is None:
            self.xvfb.terminate()
            self.xvfb.wait(timeout=5)
        if not self.args.keep_home and getattr(self, "root", None):
            shutil.rmtree(self.root, ignore_errors=True)


def parse_forms(spec):
    forms = []
    for item in spec.split(","):
        item = item.strip()
        if not item:
            continue
        code, _, period = item.partition(":")
        slug = code.lower()
        forms.append((code, f"page-form-{slug}", int(period or 1)))
    return forms


def main():
    target = env("CARGO_TARGET_DIR", str(REPO / "target"))
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--out", default=env("WALK_OUT", str(REPO / "target" / "ui-walkthrough")),
                   help="output folder for PNGs, index.html, layout.json, app.log")
    p.add_argument("--bin", default=env("WALK_BIN", str(Path(target) / "debug" / "bir-desktop")),
                   help="app binary built with --features agent (default $CARGO_TARGET_DIR/debug/bir-desktop)")
    p.add_argument("--data-from", help="directory holding bir_data.db (+ -wal/-shm) and bir_key.txt to copy in")
    p.add_argument("--demo-profile", action="store_true",
                   help="if the DB has no active profile, create a demo one through the agent")
    p.add_argument("--tin", help="profile TIN for the dashboard and form pages (default: first active profile)")
    p.add_argument("--forms", default="2550Q:1,1800:1",
                   help="comma list CODE[:PERIOD] of form pages to open (default 2550Q:1,1800:1)")
    p.add_argument("--year", type=int, help="tax year for form pages (default: current year)")
    p.add_argument("--size", default="1440x900", help="Xvfb screen and window size")
    p.add_argument("--display", type=int, help="X display number (default: first free from :110)")
    p.add_argument("--scroll-notches", type=int, default=40, help="wheel notches to reach the end of a page")
    p.add_argument("--settle", type=float, default=1.5, help="seconds to wait before each screenshot")
    p.add_argument("--page-timeout", type=float, default=20.0)
    p.add_argument("--startup-timeout", type=float, default=90.0)
    p.add_argument("--no-dev-mode", action="store_true", help="skip Dev Mode (release builds)")
    p.add_argument("--no-fit", action="store_true", help="do not resize the window to the screen")
    p.add_argument("--keep-home", action="store_true", help="keep the private HOME afterwards")
    args = p.parse_args()
    args.forms = parse_forms(args.forms)

    for tool in (env("WALK_XVFB", "Xvfb"), shlex.split(env("WALK_IMPORT", "import"))[0],
                 shlex.split(env("WALK_XDOTOOL", "xdotool"))[0]):
        if not shutil.which(tool):
            print(f"missing tool: {tool}", file=sys.stderr)
            return 3

    w = Walkthrough(args)
    code = 0
    try:
        w.start_xvfb()
        w.prepare_home()
        w.start_app()
        if not args.no_fit:
            w.fit_window()
        w.check_seed()
        w.run()
        w.check_alive()
        if w.failures:
            code = 1
    except Crash as err:
        log(f"CRASH: {err}")
        w.failures.append(f"crash: {err}")
        code = 2
    except (ConnectionError, OSError) as err:
        crashed = w.app is not None and w.app.poll() is not None
        log(f"{'CRASH' if crashed else 'ERROR'}: {err}")
        w.failures.append(f"{'crash' if crashed else 'error'}: {err}")
        code = 2 if crashed else 1
    finally:
        w.write_index()
        w.shutdown()
    log(f"{len(w.shots)} screenshots, {len(w.failures)} failures -> {w.out / 'index.html'}")
    for f in w.failures:
        log(f"  - {f}")
    return code


if __name__ == "__main__":
    sys.exit(main())
