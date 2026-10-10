#!/usr/bin/env python3
"""End-to-end smoke for the /file-tax goal (called by check_file_tax_goal.sh).

1. Starts `bir-headless serve` on a temp BIR_DATABASE_PATH + temp
   GPUI_AGENT_REGISTRY (EBIR_TEST_ENV=1; never the live database).
2. Seeds the dummy test taxpayer (TIN 111111114, branch 00000) through the
   gpui-agent protocol, the same way the app's own profile editor does.
3. Starts `bir-mcp` over stdio with no GPUI_AGENT_ADDR, so it has to find
   BIR through the discovery records, and drives:
   initialize -> tools/list -> open 2551Q -> fill one box with a source ->
   needs-you non-empty -> validate returns field-level errors ->
   opening 1601C is refused -> dismiss -> opening 1601C succeeds -> shutdown.

Exits non-zero on the first broken step. Draft-only: nothing is saved,
queued, filed or paid.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import hmac
import json
import os
import re
import secrets
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

TEST_TIN = "11111111400000"  # 111111114 (official check digit) + branch 00000
FORBIDDEN_TOOL_WORDS = {"submit", "queue", "file", "pay", "paid", "payment"}
FILL_KEY = "schedule_1.0.taxable_amount"


class SmokeError(Exception):
    pass


def step(message: str) -> None:
    print(f"smoke: {message}", flush=True)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class AgentClient:
    """Minimal gpui-agent protocol v2 client (NDJSON + HMAC challenge)."""

    def __init__(self, addr: str, token: str) -> None:
        host, port = addr.rsplit(":", 1)
        self.sock = socket.create_connection((host, int(port)), timeout=15)
        self.reader = self.sock.makefile("rb")
        challenge = json.loads(self.reader.readline())
        if challenge.get("op") != "challenge":
            raise SmokeError(f"expected challenge, got {challenge}")
        nonce = bytes.fromhex(challenge["nonce"])
        self.auth = hmac.new(token.encode(), nonce, hashlib.sha256).hexdigest()
        self.version = challenge.get("v", 2)
        self.next_id = 0

    def request(self, op: dict) -> dict:
        self.next_id += 1
        body = {"v": self.version, "id": f"seed-{self.next_id}", "auth": self.auth}
        body.update(op)
        self.sock.sendall((json.dumps(body) + "\n").encode())
        line = self.reader.readline()
        if not line:
            raise SmokeError(f"agent closed the connection during {op}")
        return json.loads(line)

    def close(self) -> None:
        try:
            self.sock.close()
        except OSError:
            pass


class McpClient:
    """JSON-RPC 2.0 over newline-delimited stdio."""

    def __init__(self, argv: list[str], env: dict[str, str]) -> None:
        self.proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )
        self.next_id = 0
        self.stderr_lines: list[str] = []
        threading.Thread(target=self._drain_stderr, daemon=True).start()

    def _drain_stderr(self) -> None:
        assert self.proc.stderr is not None
        for raw in self.proc.stderr:
            self.stderr_lines.append(raw.decode(errors="replace").rstrip())

    def _send(self, message: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write((json.dumps(message) + "\n").encode())
        self.proc.stdin.flush()

    def notify(self, method: str, params: dict | None = None) -> None:
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        self._send(message)

    def call(self, method: str, params: dict | None = None, timeout: float = 30.0) -> dict:
        self.next_id += 1
        request_id = self.next_id
        message = {"jsonrpc": "2.0", "id": request_id, "method": method}
        if params is not None:
            message["params"] = params
        self._send(message)
        deadline = time.monotonic() + timeout
        assert self.proc.stdout is not None
        while time.monotonic() < deadline:
            line = self.proc.stdout.readline()
            if not line:
                raise SmokeError(
                    f"bir-mcp closed stdout during {method}; stderr:\n"
                    + "\n".join(self.stderr_lines[-30:])
                )
            line = line.strip()
            if not line:
                continue
            try:
                reply = json.loads(line)
            except json.JSONDecodeError as error:
                raise SmokeError(f"bir-mcp wrote non-JSON to stdout: {line!r}") from error
            if reply.get("id") == request_id:
                return reply
            # Server-initiated notifications / requests are ignored.
        raise SmokeError(f"bir-mcp did not answer {method} within {timeout}s")

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
        except OSError:
            pass
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=5)


def normalize(name: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def find_tool(tools: list[dict], suffixes: tuple[str, ...]) -> str:
    for tool in tools:
        normalized = normalize(tool["name"])
        if any(normalized == s or normalized.endswith("_" + s) for s in suffixes):
            return tool["name"]
    names = [tool["name"] for tool in tools]
    raise SmokeError(f"no MCP tool for {suffixes}; tools/list has {names}")


def tool_outcome(reply: dict) -> tuple[bool, object, str]:
    """(ok, payload, text) for a tools/call reply."""
    if "error" in reply:
        error = reply["error"]
        return False, error, json.dumps(error)
    result = reply.get("result") or {}
    texts = [
        item.get("text", "")
        for item in result.get("content", [])
        if isinstance(item, dict) and item.get("type") == "text"
    ]
    text = "\n".join(texts)
    payload: object = result.get("structuredContent")
    if payload is None:
        try:
            payload = json.loads(text)
        except (json.JSONDecodeError, TypeError):
            payload = text
    return not result.get("isError", False), payload, text


def call_tool(mcp: McpClient, name: str, arguments: dict) -> tuple[bool, object, str]:
    return tool_outcome(mcp.call("tools/call", {"name": name, "arguments": arguments}))


def expect_ok(label: str, outcome: tuple[bool, object, str]) -> object:
    ok, payload, text = outcome
    if not ok:
        raise SmokeError(f"{label} failed: {text}")
    if not isinstance(payload, dict):
        raise SmokeError(f"{label} returned no JSON object: {text!r}")
    return payload


def wait_ready(addr: str, token: str, proc: subprocess.Popen, log: Path) -> None:
    deadline = time.monotonic() + 60
    last_error = ""
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise SmokeError(
                f"bir-headless exited early ({proc.returncode}); log:\n"
                + (log.read_text(errors="replace") if log.exists() else "")
            )
        try:
            client = AgentClient(addr, token)
            reply = client.request({"op": "hello"})
            client.close()
            if reply.get("ok"):
                return
            last_error = str(reply)
        except (OSError, ValueError, SmokeError) as error:
            last_error = str(error)
        time.sleep(0.2)
    raise SmokeError(f"bir-headless never became ready at {addr}: {last_error}")


def seed_profile(addr: str, token: str) -> None:
    client = AgentClient(addr, token)
    try:
        reply = client.request({"op": "invoke", "name": "profile.create", "args": {}})
        if not reply.get("ok"):
            raise SmokeError(f"profile.create: {reply}")
        for target, value in [
            ("profile-tin", TEST_TIN),
            ("profile-name", "File Tax Smoke Taxpayer"),
            ("profile-rdo", "018"),
            ("profile-lob", "Retail"),
            ("profile-address", "Manila"),
            ("profile-zip", "1000"),
            ("profile-phone", "09170000000"),
            ("profile-email", "file-tax-smoke@example.com"),
        ]:
            reply = client.request({"op": "set_value", "target": target, "value": value})
            if not reply.get("ok"):
                raise SmokeError(f"set_value {target}: {reply}")
        reply = client.request({"op": "invoke", "name": "profile.save", "args": {}})
        if not reply.get("ok"):
            raise SmokeError(f"profile.save: {reply}")
    finally:
        client.close()


def run(args: argparse.Namespace) -> None:
    work = Path(args.work)
    work.mkdir(parents=True, exist_ok=True)
    registry = work / "registry"
    registry.mkdir(exist_ok=True)
    db_path = work / "bir_data.db"
    log = work / "logs" / "bir-headless.log"
    token = secrets.token_hex(16)
    addr = f"127.0.0.1:{free_port()}"

    base_env = {k: v for k, v in os.environ.items() if not k.startswith("GPUI_AGENT")}
    base_env.update(
        {
            "BIR_DATABASE_PATH": str(db_path),
            "EBIR_TEST_ENV": "1",
            "GPUI_AGENT_REGISTRY": str(registry),
            "GPUI_AGENT_TOKEN": token,
        }
    )
    headless_env = dict(base_env)
    headless_env.update(
        {
            "GPUI_AGENT": "1",
            "GPUI_AGENT_ADDR": addr,
            "BIR_HEADLESS_LOG": str(log),
            "BIR_HEADLESS_PID": str(work / "bir-headless.pid"),
        }
    )

    step(f"bir-headless serve on {addr} (db {db_path}, registry {registry})")
    headless = subprocess.Popen(
        [args.headless, "serve"],
        env=headless_env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    mcp: McpClient | None = None
    try:
        wait_ready(addr, token, headless, log)
        if not any(registry.glob("*.json")):
            raise SmokeError(f"bir-headless wrote no discovery record in {registry}")
        seed_profile(addr, token)
        step("seeded test taxpayer")

        # No GPUI_AGENT_ADDR: bir-mcp must find BIR via the discovery records.
        mcp = McpClient([args.mcp], dict(base_env))
        init = mcp.call(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "file-tax-smoke", "version": "0"},
            },
        )
        if "result" not in init:
            raise SmokeError(f"initialize failed: {init}")
        mcp.notify("notifications/initialized")
        step("initialize ok")

        listed = mcp.call("tools/list", {})
        tools = (listed.get("result") or {}).get("tools")
        if not isinstance(tools, list) or not tools:
            raise SmokeError(f"tools/list returned no tools: {listed}")
        forbidden = [
            tool["name"]
            for tool in tools
            if FORBIDDEN_TOOL_WORDS & set(normalize(tool["name"]).split("_"))
        ]
        if forbidden:
            raise SmokeError(f"tools/list exposes forbidden tools: {forbidden}")
        open_tool = find_tool(tools, ("form_open", "filing_start", "open_form"))
        fill_tool = find_tool(tools, ("form_fill", "fill_form"))
        needs_tool = find_tool(tools, ("form_needs_you", "needs_you"))
        validate_tool = find_tool(tools, ("form_validate", "validate_form"))
        dismiss_tool = find_tool(tools, ("form_dismiss", "dismiss_form"))
        step(f"tools/list ok ({len(tools)} tools, none submit/queue/file/pay)")

        # A quarter that has already started, so the period itself is valid.
        year = _dt.date.today().year - 1
        opened = expect_ok(
            "open 2551Q",
            call_tool(mcp, open_tool, {"code": "2551Q", "year": year, "period": 4, "tin": TEST_TIN}),
        )
        if opened.get("form") != "2551Q":
            raise SmokeError(f"open 2551Q returned {opened}")
        step("open 2551Q ok")

        filled = expect_ok(
            "fill",
            call_tool(mcp, fill_tool, {"fields": {FILL_KEY: "1000.00"}, "source": "document"}),
        )
        if FILL_KEY not in (filled.get("filled") or []):
            raise SmokeError(f"fill did not report {FILL_KEY} as filled: {filled}")
        if "kept_user_boxes" not in filled:
            raise SmokeError(f"fill result lacks kept_user_boxes: {filled}")
        step("fill one box with source=document ok")

        needs = expect_ok("needs_you", call_tool(mcp, needs_tool, {}))
        boxes = needs.get("boxes")
        if not isinstance(boxes, list) or not boxes:
            raise SmokeError(f"needs-you list is empty: {needs}")
        if not all(isinstance(b, dict) and b.get("field") and "label" in b for b in boxes):
            raise SmokeError(f"needs-you boxes lack field/label: {boxes}")
        step(f"needs-you lists {len(boxes)} boxes")

        validated = expect_ok("validate", call_tool(mcp, validate_tool, {}))
        field_errors = validated.get("field_errors")
        if not isinstance(field_errors, list) or not field_errors:
            raise SmokeError(f"validate returned no field_errors: {validated}")
        if not all(
            isinstance(e, dict) and e.get("field") and e.get("message") for e in field_errors
        ):
            raise SmokeError(f"field_errors are not [{{field, message}}]: {field_errors}")
        if validated.get("ok") is not False or validated.get("errors") != len(field_errors):
            raise SmokeError(f"validate ok/errors disagree with field_errors: {validated}")
        step(f"validate returned {len(field_errors)} field-level errors")

        ok, _, text = call_tool(
            mcp, open_tool, {"code": "1601C", "year": year, "period": 12, "tin": TEST_TIN}
        )
        if ok or "form already open" not in text:
            raise SmokeError(f"opening 1601C while 2551Q is open was not refused: {text}")
        step("open 1601C refused while 2551Q is open")

        dismissed = expect_ok("dismiss", call_tool(mcp, dismiss_tool, {}))
        if dismissed.get("dismissed") is not True or dismissed.get("form") != "2551Q":
            raise SmokeError(f"dismiss returned {dismissed}")
        step("dismiss 2551Q ok")

        reopened = expect_ok(
            "open 1601C after dismiss",
            call_tool(mcp, open_tool, {"code": "1601C", "year": year, "period": 12, "tin": TEST_TIN}),
        )
        if reopened.get("form") != "1601C":
            raise SmokeError(f"open 1601C returned {reopened}")
        step("open 1601C after dismiss ok")
    finally:
        if mcp is not None:
            mcp.close()
        shutdown = subprocess.run(
            [args.headless, "shutdown"],
            env=headless_env,
            capture_output=True,
            text=True,
            timeout=30,
        )
        try:
            headless.wait(timeout=15)
        except subprocess.TimeoutExpired:
            headless.kill()
            headless.wait(timeout=5)
            raise SmokeError(
                f"bir-headless did not exit after shutdown: {shutdown.stdout}{shutdown.stderr}"
            )
    if shutdown.returncode != 0:
        raise SmokeError(f"bir-headless shutdown failed: {shutdown.stdout}{shutdown.stderr}")
    step("shutdown ok")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--headless", required=True, help="path to bir-headless")
    parser.add_argument("--mcp", required=True, help="path to bir-mcp")
    parser.add_argument("--work", required=True, help="scratch directory")
    try:
        run(parser.parse_args())
    except (SmokeError, OSError, subprocess.SubprocessError) as error:
        print(f"smoke: FAILED: {error}", file=sys.stderr, flush=True)
        return 1
    print("smoke: PASSED", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
