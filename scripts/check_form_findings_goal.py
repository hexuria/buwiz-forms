#!/usr/bin/env python3
"""Done-condition of the per-form findings goal (.claude/GOAL.md).

Exit 0 only when every screen form has a reviewed findings record covering
every approved rule, a rendered findings page with its screenshots, and a
published artifact URL in the manifest. Prints what is missing otherwise.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FINDINGS = ROOT / "docs" / "form-findings"
SECTIONS = ["R0", "R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "preview", "validation"]
STATUSES = {"ok", "findings", "n/a"}


def check_form(code):
    problems = []
    d = FINDINGS / code
    f = d / "findings.json"
    if not f.exists():
        return [f"{code}: no findings.json"]
    try:
        data = json.loads(f.read_text())
    except ValueError as e:
        return [f"{code}: findings.json is not JSON ({e})"]
    if data.get("form") != code:
        problems.append(f"{code}: findings.json names form {data.get('form')!r}")
    rules = data.get("rules", {})
    for s in SECTIONS:
        sec = rules.get(s)
        if not isinstance(sec, dict):
            problems.append(f"{code}: section {s} missing")
            continue
        if sec.get("status") not in STATUSES:
            problems.append(f"{code}: section {s} status {sec.get('status')!r}")
        if sec.get("status") == "findings":
            items = sec.get("findings") or []
            if not items:
                problems.append(f"{code}: section {s} says findings but lists none")
            for i, it in enumerate(items):
                if not str(it.get("evidence", "")).strip():
                    problems.append(f"{code}: section {s} finding {i} has no evidence")
    if data.get("reviewed") is not True or not str(data.get("review_notes", "")).strip():
        problems.append(f"{code}: not reviewed by hand")
    page = d / "page.html"
    if not page.exists():
        problems.append(f"{code}: no page.html")
    for shot in data.get("screenshots", []):
        if not (d / shot).exists():
            problems.append(f"{code}: screenshot {shot} missing")
    if not data.get("screenshots"):
        problems.append(f"{code}: no screenshots named")
    return problems


def main():
    codes = [f["c"] for f in json.loads((ROOT / "app/forms/forms-schema.json").read_text())]
    problems = []
    for code in codes:
        problems += check_form(code)
    manifest_path = FINDINGS / "manifest.json"
    manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    for code in codes:
        url = (manifest.get("forms", {}).get(code) or {}).get("url", "")
        if not url.startswith("https://claude.ai/"):
            problems.append(f"{code}: no artifact URL in manifest")
    if not str(manifest.get("index_url", "")).startswith("https://claude.ai/"):
        problems.append("index: no artifact URL in manifest")
    done = sum(1 for c in codes if not any(p.startswith(c + ":") for p in problems))
    print(f"forms complete: {done}/{len(codes)}")
    for p in problems[:60]:
        print("  " + p)
    if len(problems) > 60:
        print(f"  ... {len(problems) - 60} more")
    return 0 if not problems else 1


if __name__ == "__main__":
    sys.exit(main())
