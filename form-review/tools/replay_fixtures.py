#!/usr/bin/env python3
"""Deterministic fixture replay for neon-4 BEFORE/AFTER form reviews.

Opens each canonical saved draft from `before.db` (copied verbatim to
`after-deterministic.db` and exported under `fixtures/<FORM>.json`) through
the Filings UI (`nav-filings` -> `return-<id>`, NEVER `form.open` which resets
open-ended forms), asserts `length(data_json)` is unchanged after print, renders
the generated `sheet.html` via headless Chromium + pdftoppm, and writes
`print-p*.png` into `form-review/<FORM>/after/`.
"""
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

AGENTS_DIR = Path("/home/ubuntu/BuwizNsh/agents")
REPO_DIR = Path("/home/ubuntu/repos/buwiz-forms")
DB_PATH = AGENTS_DIR / "after-deterministic.db"
TOKEN = os.environ.get("GPUI_AGENT_TOKEN", "neon4-deterministic")

# (form_code, draft_id, expected_bytes, expected_pages, capture_screens)
FORMS = [
    ("1800", 1, 10405, 2, False),
    ("1801", 2, 32881, 2, False),
    ("1600WP", 6, 2488, 1, False),
    ("1602Q", 7, 2774, 2, False),
    ("2552", 5, 3351, 2, True),
]


def gpui(port: int, *args: str):
    cmd = ["gpui-agent", "--addr", f"127.0.0.1:{port}", "--token", TOKEN, *args]
    res = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if res.returncode != 0:
        raise RuntimeError(f"gpui-agent failed ({res.returncode}): {res.stderr or res.stdout}")
    out = res.stdout.strip()
    return json.loads(out) if out.startswith("{") else out


def get_container_id() -> str:
    return subprocess.check_output(
        ["docker", "ps", "-q", "--filter", "ancestor=buwiz-nsh-2404"], text=True
    ).strip().splitlines()[0]


def slugify(s: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")
    return s[:40] or "section"


def check_draft_bytes(draft_id: int, expected_bytes: int, label: str) -> None:
    con = sqlite3.connect(DB_PATH)
    row = con.execute("SELECT length(data_json) FROM form_drafts WHERE id=?", (draft_id,)).fetchone()
    con.close()
    assert row is not None and row[0] == expected_bytes, (
        f"{label}: draft id={draft_id} size mismatch (got {row}, expected {expected_bytes})"
    )


def render_form(form_code: str, draft_id: int, expected_bytes: int, expected_pages: int, capture_screens: bool) -> None:
    check_draft_bytes(draft_id, expected_bytes, f"{form_code} before open")
    gpui(17421, "viewport", "--width", "1400", "--height", "900")
    time.sleep(0.2)
    gpui(17421, "click", "nav-filings")
    time.sleep(0.5)
    gpui(17421, "click", f"return-{draft_id}")
    time.sleep(0.6)

    cid = get_container_id()
    subprocess.run(["docker", "exec", cid, "sh", "-c", "rm -rf /tmp/ebirforms-print-*"], check=True)
    gpui(17421, "click", "fp-print")

    sheet_path = ""
    for _ in range(30):
        time.sleep(0.5)
        sheet_path = subprocess.check_output(
            ["docker", "exec", cid, "sh", "-c", "ls -t /tmp/ebirforms-print-*/*/sheet.html 2>/dev/null | head -n 1"],
            text=True,
        ).strip()
        if sheet_path:
            break
    assert sheet_path, f"No sheet.html produced for {form_code}"
    check_draft_bytes(draft_id, expected_bytes, f"{form_code} after print")

    local_html = AGENTS_DIR / f"det-{form_code}-sheet.html"
    local_pdf = AGENTS_DIR / f"det-{form_code}.pdf"
    subprocess.run(["docker", "cp", f"{cid}:{sheet_path}", str(local_html)], check=True)
    subprocess.run(
        [
            "/home/ubuntu/.local/bin/google-chrome",
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            "--no-pdf-header-footer",
            f"--print-to-pdf={local_pdf}",
            f"file://{local_html}",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    out_dir = REPO_DIR / "form-review" / form_code / "after"
    out_dir.mkdir(parents=True, exist_ok=True)
    for old in out_dir.glob("print-p*.png"):
        old.unlink()
    subprocess.run(
        ["pdftoppm", "-png", "-scale-to-x", "1400", "-scale-to-y", "-1", str(local_pdf), str(out_dir / "print-p")],
        check=True,
    )
    for p in sorted(out_dir.glob("print-p-*.png")):
        n = int(p.stem.split("-")[-1])
        p.rename(out_dir / f"print-p{n}.png")
    pngs = sorted(p.name for p in out_dir.glob("print-p*.png"))
    print(f"[{form_code}] Draft id={draft_id} ({expected_bytes}B unchanged) -> {pngs}")
    assert len(pngs) == expected_pages

    if capture_screens:
        snap = gpui(17421, "snapshot")["tree"]
        nav_buttons = []
        expand_btn_name = None

        def walk(n):
            nonlocal expand_btn_name
            nid = n.get("id") or ""
            if nid == "fp-parts-toggle":
                expand_btn_name = n.get("name") or ""
            if re.fullmatch(r"fp-nav-\d+", nid):
                nav_buttons.append((int(nid.split("-")[-1]), nid, n.get("name") or nid))
            for c in n.get("children") or []:
                walk(c)

        for node in snap.get("nodes") or []:
            walk(node)
        if expand_btn_name and "Expand" in expand_btn_name:
            gpui(17421, "click", "fp-parts-toggle")
            time.sleep(0.4)
        nav_buttons.sort()
        for idx, nid, label in nav_buttons:
            gpui(17421, "click", nid)
            time.sleep(0.35)
            shot = out_dir / f"screen-{idx + 1:02d}-{slugify(label)}.png"
            gpui(17421, "screenshot", "--inline", "--out", str(shot))
            print(f"  screen -> {shot.name}")


if __name__ == "__main__":
    targets = sys.argv[1:] or [f[0] for f in FORMS]
    for form_code, draft_id, expected_bytes, pages, cap_scr in FORMS:
        if form_code in targets:
            render_form(form_code, draft_id, expected_bytes, pages, cap_scr)
