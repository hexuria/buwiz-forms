#!/usr/bin/env python3
"""Deterministic BEFORE/AFTER form capture helper for form-review.

Seeds the same long-name Juana Santos demo profile via `profile.save`,
replays `payloads/payload-<CODE>.json` via `form.fill`, exports the print
sheet via the app's Print Preview button + headless Chrome PDF rendering,
and captures every nav section screenshot at a fixed 1372x871 client size.

Usage:
  python3 form-review/scripts/seed_and_capture.py <before|after> <CODE> [CODE...]
"""
import glob, json, os, re, shutil, subprocess, sys, time
import pypdfium2 as pdfium
from PIL import Image

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(SCRIPT_DIR, "..", ".."))
PAYLOAD_DIR = os.path.join(SCRIPT_DIR, "payloads")
CACHE_DIR = os.environ.get("BUWIZ_REVIEW_CACHE", "/home/ubuntu/.cache/buwiz-review")
TMP_ROOT = os.environ.get("TMPDIR", os.path.join(CACHE_DIR, "tmp"))
SHOT_DIR = os.path.join(TMP_ROOT, "gpui-agent-screenshots")

TOK = os.environ.get("GPUI_AGENT_TOKEN") or open(os.path.join(CACHE_DIR, ".tok")).read().strip()
ENV = os.environ.copy()
ENV["GPUI_AGENT_TOKEN"] = TOK
ENV["DISPLAY"] = os.environ.get("DISPLAY", ":0")

DEMO_PROFILE = {
    "tin": "00012345300000",
    "name": "SANTOS, JUANA MARIA DELA CRUZ-VILLANUEVA",
    "rdo_code": "008",
    "address": "UNIT 2804-B, 28TH FLOOR, ONE CORPORATE PLAZA TOWER NORTH, 845 ARNAIZ AVENUE COR. PASEO DE ROXAS, SAN LORENZO, MAKATI CITY, METRO MANILA",
    "zip_code": "1003",
    "phone": "09171234567",
    "email": "juana.santos@example.com",
    "birth_date": "1988-01-15",
}


def run_agent(session, *args, check=True):
    cmd = ["gpui-agent", "--session", session, *args]
    p = subprocess.run(cmd, env=ENV, capture_output=True, text=True)
    out = p.stdout
    if check and p.returncode != 0:
        raise RuntimeError(f"cmd failed ({p.returncode}): {cmd}\nstdout={out}\nstderr={p.stderr}")
    idx = out.find("{")
    if idx != -1:
        try:
            return json.loads(out[idx:])
        except Exception:
            pass
    return out


def invoke(_method, **kwargs):
    args = ["invoke", _method]
    for k, v in kwargs.items():
        args += ["--arg", f"{k}={json.dumps(v)}"]
    return run_agent("rev-data", *args, check=False)


def close_print_windows():
    p = subprocess.run(["wmctrl", "-l"], env=ENV, capture_output=True, text=True)
    for line in p.stdout.splitlines():
        if "BIR Form " in line:
            wid = line.split()[0]
            subprocess.run(["wmctrl", "-i", "-c", wid], env=ENV)
    time.sleep(0.4)


def dismiss_modals():
    for _ in range(3):
        snap = run_agent("rev-win", "snapshot", "--refs", check=False)
        if not isinstance(snap, str):
            return
        clicked = False
        for btn_label in (
            'button "Save to profile"',
            'button "Continue"',
            'button "Only this return"',
            'button "OK"',
            'button "Close"',
        ):
            for line in snap.splitlines():
                if btn_label in line:
                    ref = line.strip().split()[0]
                    run_agent("rev-win", "press", ref, check=False)
                    time.sleep(0.4)
                    clicked = True
                    break
            if clicked:
                break
        if not clicked:
            break


def slugify(s):
    s = re.sub(r"[^a-zA-Z0-9]+", "-", s.lower()).strip("-")
    return s[:40] or "section"


def seed_profile():
    r = invoke("profile.save", **DEMO_PROFILE)
    print("profile.save:", r)
    assert isinstance(r, dict) and r.get("ok"), r
    subprocess.run(["wmctrl", "-r", "eBIRForms", "-e", "0,100,100,1400,900"], env=ENV, check=False)
    time.sleep(0.5)


def capture_form(code, mode):
    print(f"\n========== CAPTURING {code} ({mode}) ==========")
    out_dir = os.path.join(REPO, "form-review", code, mode)
    if os.path.exists(out_dir):
        shutil.rmtree(out_dir)
    os.makedirs(out_dir, exist_ok=True)
    payload_path = os.path.join(PAYLOAD_DIR, f"payload-{code}.json")
    assert os.path.exists(payload_path), f"missing {payload_path}"

    close_print_windows()
    for d in glob.glob(os.path.join(TMP_ROOT, "ebirforms-print-*")):
        shutil.rmtree(d, ignore_errors=True)
    invoke("form.dismiss", discard=True)
    time.sleep(0.3)
    saved_raw = json.load(open(payload_path))
    if isinstance(saved_raw, dict):
        open_args = dict(saved_raw.get("open_args") or {})
        saved = saved_raw.get("passes") or []
        ui_rows = saved_raw.get("ui_rows") or []
    else:
        open_args = {}
        saved = saved_raw
        ui_rows = []
    open_args.setdefault("code", code)
    open_args.setdefault("year", 2025)
    open_args.setdefault("tin", DEMO_PROFILE["tin"])
    r = invoke("form.open", **open_args)
    assert isinstance(r, dict) and r.get("ok"), (code, r)
    time.sleep(0.8)
    dismiss_modals()

    for i, pass_fields in enumerate(saved, 1):
        if pass_fields:
            res = invoke("form.fill", fields=pass_fields)
            if isinstance(res, dict) and not res.get("ok") and "approval required" in str(res.get("error", "")):
                time.sleep(0.4)
                dismiss_modals()
                time.sleep(0.5)
                print(f"  pass {i}: sent={len(pass_fields)} approved via modal Continue")
                continue
            assert isinstance(res, dict) and res.get("ok"), (code, i, res)
            applied = res.get("result", {}).get("applied", [])
            refused = res.get("result", {}).get("refused") or {}
            print(f"  pass {i}: sent={len(pass_fields)} applied={len(applied)} refused={len(refused)}")
            time.sleep(0.3)
            if refused:
                retry_fields = {k: pass_fields[k] for k in refused if k in pass_fields}
                res2 = invoke("form.fill", fields=retry_fields)
                assert isinstance(res2, dict) and res2.get("ok"), (code, i, res2)
                applied2 = res2.get("result", {}).get("applied", [])
                refused2 = res2.get("result", {}).get("refused") or {}
                print(f"  pass {i} retry: sent={len(retry_fields)} applied={len(applied2)} refused={len(refused2)}")
                time.sleep(0.3)

    for block in ui_rows:
        sec_prefix = block["section"]
        rows_list = block["rows"]
        snap = run_agent("rev-win", "snapshot", "--refs")
        # Find the nav sidebar button (listed under `nav`) rather than collapsed section card
        sec_ref = None
        in_nav = False
        for line in snap.splitlines():
            if " nav" in line:
                in_nav = True
            if in_nav and f'button "{sec_prefix}' in line:
                sec_ref = line.strip().split()[0]
                break
        if not sec_ref:
            for line in snap.splitlines():
                if f'button "{sec_prefix}' in line:
                    sec_ref = line.strip().split()[0]
                    break
        if sec_ref:
            run_agent("rev-win", "press", sec_ref, check=False)
            time.sleep(0.5)
        added = 0
        for r_idx, row_vals in enumerate(rows_list, 1):
            snap = run_agent("rev-win", "snapshot", "--refs")
            add_ref = None
            for line in snap.splitlines():
                if 'button "Add row"' in line:
                    add_ref = line.strip().split()[0]
                    break
            if not add_ref:
                print(f"  ui_rows {sec_prefix} row {r_idx}: no Add row button")
                break
            run_agent("rev-win", "press", add_ref, check=False)
            time.sleep(0.35)
            dsnap = run_agent("rev-win", "snapshot", "--refs")
            in_dlg = False
            tbs = []
            done_ref = None
            for line in dsnap.splitlines():
                if " dialog" in line:
                    in_dlg = True
                if in_dlg:
                    m = re.match(r'\s*(@e\d+)\s+textbox\s+"([^"]+)"', line)
                    if m:
                        tbs.append((m.group(1), m.group(2)))
                    if 'button "Done"' in line:
                        done_ref = line.strip().split()[0]
                        break
            for (tb_ref, tb_label), val in zip(tbs, row_vals):
                run_agent("rev-win", "fill", tb_ref, str(val), check=False)
                time.sleep(0.1)
            if done_ref:
                run_agent("rev-win", "press", done_ref, check=False)
                time.sleep(0.35)
                added += 1
        print(f"  ui_rows {sec_prefix}: added {added}/{len(rows_list)} rows")

    invoke("form.save_draft")
    time.sleep(0.8)
    dismiss_modals()

    subprocess.run(["wmctrl", "-r", "eBIRForms", "-e", "0,100,100,1400,900"], env=ENV, check=False)
    time.sleep(0.4)

    run_agent("rev-win", "scroll", "main-scroll", "--dy", "5000", check=False)
    time.sleep(0.3)
    res_p = run_agent("rev-win", "click", "fp-print", check=False)
    print("  click fp-print:", str(res_p)[:120])
    sheet = None
    for _ in range(20):
        matches = glob.glob(os.path.join(TMP_ROOT, "ebirforms-print-*", code, "sheet.html"))
        if matches:
            sheet = matches[0]
            break
        time.sleep(0.4)
    if not sheet:
        raise RuntimeError(f"sheet.html not found for {code}")
    print(f"  print sheet: {sheet}")
    os.makedirs(CACHE_DIR, exist_ok=True)
    shutil.copy2(sheet, os.path.join(CACHE_DIR, f"{mode}-{code}-sheet.html"))
    pdf_out = os.path.join(CACHE_DIR, f"{mode}-{code}.pdf")
    subprocess.run(
        [
            "google-chrome",
            "--headless=new",
            "--no-sandbox",
            "--disable-gpu",
            f"--print-to-pdf={pdf_out}",
            "--no-pdf-header-footer",
            f"file://{sheet}",
        ],
        env=ENV,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=True,
    )
    doc = pdfium.PdfDocument(pdf_out)
    print(f"  rendered {len(doc)} print pages")
    for pi, page in enumerate(doc, 1):
        w, h = page.get_size()
        img = page.render(scale=1400.0 / w).to_pil()
        p_dst = os.path.join(out_dir, f"print-p{pi}.png")
        img.save(p_dst)
        print(f"  saved print-p{pi}.png ({img.size})")

    close_print_windows()

    snap = run_agent("rev-win", "snapshot", "--refs")
    lines = snap.splitlines()
    nav_buttons = []
    in_nav = False
    nav_indent = 0
    for line in lines:
        if 'scroll "fp-nav-list"' in line:
            in_nav = True
            nav_indent = len(line) - len(line.lstrip())
            continue
        if in_nav:
            ind = len(line) - len(line.lstrip())
            if ind <= nav_indent:
                in_nav = False
                continue
            m = re.match(r'\s*(@e\d+)\s+button\s+"([^"]+)"', line)
            if m:
                nav_buttons.append((m.group(1), m.group(2)))

    print(f"  found {len(nav_buttons)} nav sections: {[b[1] for b in nav_buttons]}")
    for i, (ref, title) in enumerate(nav_buttons, 1):
        cur_snap = run_agent("rev-win", "snapshot", "--refs")
        cur_ref = None
        for line in cur_snap.splitlines():
            if f'button "{title}"' in line:
                cur_ref = line.strip().split()[0]
                break
        if cur_ref:
            run_agent("rev-win", "press", cur_ref, check=False)
            time.sleep(0.5)
            dismiss_modals()
        slug = slugify(title)
        fname = f"screen-{i:02d}-{slug}.png"
        shot_name = f"{code}-{mode}-{fname}"
        run_agent("rev-win", "screenshot", "--out", shot_name)
        src = os.path.join(SHOT_DIR, shot_name)
        dst = os.path.join(out_dir, fname)
        shutil.copy2(src, dst)
        im = Image.open(dst)
        print(f"  saved {fname} ({im.size})")

    close_print_windows()
    invoke("form.dismiss", discard=True)
    time.sleep(0.5)


if __name__ == "__main__":
    if len(sys.argv) < 3 or sys.argv[1] not in ("before", "after"):
        sys.exit("Usage: seed_and_capture.py <before|after> <CODE> [CODE...]")
    mode = sys.argv[1]
    codes = sys.argv[2:]
    seed_profile()
    for c in codes:
        capture_form(c, mode)
