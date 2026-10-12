#!/usr/bin/env python3
"""Report-only Windows runtime smoke for the painted desktop agent."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import struct
import subprocess
import sys
import time
from typing import Any

SCRIPTS = Path(__file__).resolve().parent
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from file_tax_smoke import AgentClient, SmokeError

PHASES = ("preview-1601c", "preview-2551q", "tray", "submit")
SMOKE_TIN = "12345678800000"
PANIC_MARKERS = ("panicked at", "overflowed its stack")
TRAY_NOTE = (
    "Showing the window again and graceful exit while hidden are not "
    "agent-reachable because the agent mailbox drains in AppState::render; "
    "cover those as manual VM steps (#98)."
)


def read_stack_reserve(path: str | Path) -> int:
    """Read SizeOfStackReserve from a PE32+ optional header."""
    data = Path(path).read_bytes()
    if data[:2] != b"MZ":
        raise ValueError("not a PE file: missing MZ")
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_offset : pe_offset + 4] != b"PE\0\0":
        raise ValueError("not a PE file: missing PE signature")
    optional_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
    optional = pe_offset + 24
    if optional + optional_size > len(data):
        raise ValueError("truncated PE optional header")
    if struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise ValueError("only PE32+ binaries are supported")
    return struct.unpack_from("<Q", data, optional + 72)[0]


def scan_logs(stdout: str, stderr: str) -> str | None:
    combined = f"{stdout}\n{stderr}"
    for marker in PANIC_MARKERS:
        if marker in combined:
            return f"runtime marker: {marker}"
    return None


def classify_exit(code: int | None) -> str | None:
    if code is None:
        return None
    unsigned = code & 0xFFFFFFFF
    if unsigned == 0:
        return None
    if unsigned == 0xC00000FD:
        return "stack overflow (0xC00000FD)"
    if unsigned == 0xC0000005:
        return "access violation (0xC0000005)"
    return f"non-zero exit (0x{unsigned:08X})"


def smoke_env(parent: dict[str, str], phase_dir: Path, addr: str, token: str, connect_id: str) -> dict[str, str]:
    env = {k: v for k, v in parent.items() if not k.startswith("BIR_SFTP_")}
    env.update(
        {
            "BIR_SFTP_DRY_RUN": "1",
            "BIR_DATABASE_PATH": str(phase_dir / "smoke.db"),
            "LOCALAPPDATA": str(phase_dir / "localappdata"),
            "EBIR_TEST_ENV": "1",
            "GPUI_AGENT": "1",
            "GPUI_AGENT_ALLOW_RELEASE": "1",
            "GPUI_AGENT_ADDR": addr,
            "GPUI_AGENT_TOKEN": token,
            "GPUI_AGENT_CONNECT_ID": connect_id,
            "GPUI_AGENT_RUNTIME_DIR": str(phase_dir / "gpui-agent-runtime"),
            "RUST_BACKTRACE": "1",
        }
    )
    return env


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def existing_bir() -> bool:
    if os.name != "nt":
        return False
    result = subprocess.run(
        ["tasklist.exe", "/FI", "IMAGENAME eq bir.exe", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
        check=False,
    )
    return any(line.strip().lower().startswith('"bir.exe"') for line in result.stdout.splitlines())


def invoke(client: AgentClient, name: str, args: dict[str, Any] | None = None) -> dict:
    reply = client.request({"op": "invoke", "name": name, "args": args or {}})
    if not reply.get("ok"):
        raise SmokeError(f"{name} failed: {reply}")
    return reply


def set_value(client: AgentClient, target: str, value: str) -> None:
    reply = client.request({"op": "set_value", "target": target, "value": value})
    if not reply.get("ok"):
        raise SmokeError(f"set_value {target} failed: {reply}")


def seed_profile(client: AgentClient, year: int) -> None:
    invoke(client, "profile.create")
    for target, value in (
        ("profile-tin", SMOKE_TIN),
        ("profile-name", "Windows Smoke Taxpayer"),
        ("profile-rdo", "018"),
        ("profile-lob", "Retail"),
        ("profile-address", "Manila"),
        ("profile-zip", "1000"),
        ("profile-phone", "09170000000"),
        ("profile-email", "windows-smoke@example.com"),
    ):
        set_value(client, target, value)
    invoke(client, "profile.save")
    invoke(
        client,
        "profile.forms_set.set",
        {"tin": SMOKE_TIN, "year": year, "codes": ["1601C", "2551Q"], "confirm": True},
    )
    # Reload the saved profile/editor state before filing.start. The desktop
    # refuses to leave a dirty profile editor even after forms_set is saved.
    invoke(client, "profile.edit", {"tin": SMOKE_TIN})


def response_tree(reply: dict) -> dict:
    tree = reply.get("tree")
    if isinstance(tree, dict):
        return tree
    result = reply.get("result")
    if isinstance(result, dict) and isinstance(result.get("tree"), dict):
        return result["tree"]
    raise SmokeError(f"snapshot returned no tree: {reply}")


def find_node(value: Any, node_id: str) -> dict | None:
    if isinstance(value, dict):
        if value.get("id") == node_id:
            return value
        for child in value.get("nodes", []) + value.get("children", []):
            found = find_node(child, node_id)
            if found:
                return found
    elif isinstance(value, list):
        for child in value:
            found = find_node(child, node_id)
            if found:
                return found
    return None


def node_has_area(node: dict) -> bool:
    bounds = node.get("bounds")
    return isinstance(bounds, dict) and float(bounds.get("w", 0)) > 0 and float(bounds.get("h", 0)) > 0


def node_value(client: AgentClient, node_id: str) -> dict | None:
    return find_node(response_tree(client.request({"op": "snapshot"})), node_id)


def wait_preview(client: AgentClient, proc: subprocess.Popen, node_id: str, timeout: float = 30) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise SmokeError(f"process exited while waiting for {node_id}: {classify_exit(proc.returncode)}")
        try:
            node = node_value(client, node_id)
            if node and node_has_area(node):
                return
        except (OSError, SmokeError, ValueError, json.JSONDecodeError):
            pass
        time.sleep(0.5)
    raise SmokeError(f"{node_id} never appeared with non-empty bounds within {timeout}s")


def wait_status(client: AgentClient, proc: subprocess.Popen, timeout: float = 90) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise SmokeError(f"process exited while waiting for Submitted: {classify_exit(proc.returncode)}")
        node = node_value(client, "form-1601c-status")
        if node and str(node.get("value", "")).lower() == "submitted":
            return
        time.sleep(1)
    raise SmokeError("form-1601c-status did not reach Submitted within 90s")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""


def run_phase(bin_path: Path, work: Path, phase: str) -> dict[str, Any]:
    phase_dir = work / phase
    phase_dir.mkdir(parents=True, exist_ok=True)
    stdout_path, stderr_path = phase_dir / "stdout.log", phase_dir / "stderr.log"
    result: dict[str, Any] = {
        "phase": phase,
        "status": "fail",
        "step_reached": "before launch",
        "reason": None,
        "exit_code": None,
        "stdout": str(stdout_path),
        "stderr": str(stderr_path),
    }
    if phase == "tray":
        result["note"] = TRAY_NOTE
    if existing_bir():
        result["reason"] = "close the running app first (single-instance)"
        return result
    token = secrets.token_hex(32)
    connect_id = f"windows-smoke-{phase}-{secrets.token_hex(8)}"
    addr = f"127.0.0.1:{free_port()}"
    env = smoke_env(dict(os.environ), phase_dir, addr, token, connect_id)
    (phase_dir / "gpui-agent-runtime").mkdir(exist_ok=True)
    proc: subprocess.Popen | None = None
    client: AgentClient | None = None
    out = open(stdout_path, "wb")
    err = open(stderr_path, "wb")
    try:
        proc = subprocess.Popen([str(bin_path)], env=env, stdout=out, stderr=err)
        result["step_reached"] = "launched"
        deadline = time.monotonic() + 60
        last_error = ""
        while time.monotonic() < deadline:
            if proc.poll() is not None:
                if proc.returncode == 0:
                    raise SmokeError("close the running app first (single-instance)")
                raise SmokeError(f"process exited before hello: {classify_exit(proc.returncode)}")
            try:
                client = AgentClient(addr, token)
                hello = client.request({"op": "hello"})
                if hello.get("ok"):
                    break
                client.close()
                client = None
                last_error = str(hello)
            except (OSError, SmokeError, ValueError, json.JSONDecodeError) as exc:
                last_error = str(exc)
            time.sleep(0.25)
        else:
            raise SmokeError(f"hello did not answer within 60s: {last_error}")
        result["step_reached"] = "hello"
        year = time.localtime().tm_year
        seed_profile(client, year)
        result["step_reached"] = "seeded profile"
        if phase.startswith("preview-"):
            code = "1601C" if phase.endswith("1601c") else "2551Q"
            period = 12 if code == "1601C" else 4
            invoke(client, "filing.start", {"code": code, "year": year, "period": period, "tin": SMOKE_TIN})
            if code == "1601C":
                invoke(client, "form.fill", {"any_taxes_withheld": "Yes", "category_of_agent": "P"})
                set_value(client, "form-1601c-tax-14", "1000.00")
                set_value(client, "form-1601c-tax-25", "100.00")
            invoke(client, "form.print")
            result["step_reached"] = f"requested {code} preview"
            wait_preview(client, proc, "print-preview-scroll")
            result["step_reached"] = "preview painted"
            time.sleep(5)
            hello = client.request({"op": "hello"})
            if not hello.get("ok"):
                raise SmokeError(f"hello after preview failed: {hello}")
            result["step_reached"] = "preview held and hello answered"
        elif phase == "tray":
            reply = client.request({"op": "keybinding", "binding": "app.toggle_visibility", "scope": "global"})
            if not reply.get("ok"):
                raise SmokeError(f"app.toggle_visibility failed: {reply}")
            result["step_reached"] = "visibility hidden"
            time.sleep(3)
            if proc.poll() is not None:
                raise SmokeError(f"process died while hidden: {classify_exit(proc.returncode)}")
            marker = scan_logs(read_text(stdout_path), read_text(stderr_path))
            if marker:
                raise SmokeError(marker)
            result["step_reached"] = "visibility hidden; process alive after 3s"
            client.close()
            client = None
            proc.kill()
            proc.wait(timeout=5)
            result["exit_code"] = proc.returncode
            result["status"] = "pass"
            return result
        elif phase == "submit":
            invoke(client, "filing.start", {"code": "1601C", "year": year, "period": 1, "tin": SMOKE_TIN})
            invoke(client, "form.fill", {"any_taxes_withheld": "Yes", "category_of_agent": "P"})
            set_value(client, "form-1601c-tax-14", "1000.00")
            set_value(client, "form-1601c-tax-25", "100.00")
            invoke(client, "filing.validate")
            invoke(client, "filing.submit")
            invoke(client, "filing.queue", {"confirm": True, "tin": SMOKE_TIN, "year": year, "period": 1, "code": "1601C"})
            result["step_reached"] = "queued filing"
            wait_status(client, proc)
            result["step_reached"] = "Submitted"
            deadline = time.monotonic() + 90
            while time.monotonic() < deadline and "SFTP dry-run PUT (no network)" not in read_text(stdout_path):
                time.sleep(1)
            if "SFTP dry-run PUT (no network)" not in read_text(stdout_path):
                raise SmokeError("stdout did not contain SFTP dry-run PUT (no network)")
            result["step_reached"] = "Submitted and dry-run PUT logged"
        reply = client.request({"op": "shutdown"})
        if not reply.get("ok"):
            raise SmokeError(f"shutdown failed: {reply}")
        client.close()
        client = None
        try:
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
            raise SmokeError("process did not exit within 15s after shutdown")
        result["exit_code"] = proc.returncode
        if classify_exit(proc.returncode):
            raise SmokeError(classify_exit(proc.returncode) or "non-zero exit")
        result["status"] = "pass"
    except Exception as exc:
        result["reason"] = str(exc)
    finally:
        if client is not None:
            try:
                if phase != "tray":
                    client.request({"op": "shutdown"})
            except Exception:
                pass
            client.close()
        if proc is not None and proc.poll() is None:
            proc.kill()
            proc.wait(timeout=5)
        out.close()
        err.close()
        marker = scan_logs(read_text(stdout_path), read_text(stderr_path))
        app_logs = sorted(str(path) for path in phase_dir.rglob("ebirforms.*.log"))
        result["logs"] = [str(stdout_path), str(stderr_path), *app_logs]
        result["exit_code"] = proc.returncode if proc is not None and proc.poll() is not None else result["exit_code"]
        if marker:
            result["status"] = "fail"
            result["reason"] = marker
        elif (
            result["status"] != "pass"
            and not result["reason"]
            and proc is not None
            and proc.returncode is not None
        ):
            exit_reason = classify_exit(proc.returncode)
            if exit_reason:
                result["reason"] = exit_reason
    return result


def write_summary(results: list[dict[str, Any]]) -> None:
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary:
        return
    with open(summary, "a", encoding="utf-8") as handle:
        handle.write("\n### Windows smoke\n\n| Phase | Status | Step | Reason |\n|---|---|---|---|\n")
        for item in results:
            handle.write(
                f"| {item['phase']} | {item['status']} | {item['step_reached']} | "
                f"{item.get('reason') or ''} |\n"
            )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", required=True)
    parser.add_argument("--work", required=True)
    parser.add_argument("--phase", choices=("all", *PHASES), default="all")
    parser.add_argument("--json-out")
    args = parser.parse_args()
    bin_path = Path(args.bin).resolve()
    work = Path(args.work).resolve()
    phases = PHASES if args.phase == "all" else (args.phase,)
    results = [{"phase": "metadata", "status": "report", "stack_reserve": read_stack_reserve(bin_path)}]
    results.extend(run_phase(bin_path, work, phase) for phase in phases)
    output = {"binary": str(bin_path), "stack_reserve": results[0]["stack_reserve"], "phases": results[1:]}
    json_path = Path(args.json_out) if args.json_out else work / "results.json"
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(output, indent=2) + "\n", encoding="utf-8")
    print(f"stack_reserve: {output['stack_reserve']} (report-only)")
    print(json.dumps(output, indent=2))
    write_summary(results[1:])
    return 0 if all(item["status"] == "pass" for item in results[1:]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
