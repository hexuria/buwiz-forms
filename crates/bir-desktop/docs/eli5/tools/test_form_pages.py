#!/usr/bin/env python3
"""Discover rules/forms ↔ html-frozen pairs, run form_page.py into a temp dir,
exit non-zero on any drawn/field_count MISMATCH.
"""
from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "rules" / "forms").is_dir())
TOOL = Path(__file__).resolve().parent / "form_page.py"

# Typed-model first-wave order (then everything else alphabetically).
TYPED_ORDER = [
    "2551q-v2018",
    "1601c-v2018",
    "0619e-v2018",
    "0619f-v2018",
    "0605-v2003",
    "1701q-v2018",
    "2550q-v2024",
    "1701-v2018",
    "1702rt-v2018c",
    "1702mx-v2018c",
]

# Explicit frozen overrides when stem matching is ambiguous.
FROZEN_OVERRIDE = {
    "2000-v2018": "2000-dst-2018",
}


def code_from_rules_id(rules_id: str) -> str:
    stem = rules_id.split("-v")[0]
    # 1601c → 1601C; 1702rt → 1702RT; 0619e → 0619E; 1701ms → 1701MS; 1600pt → 1600PT
    m = re.match(r"^(\d+)([a-z]*)$", stem)
    if not m:
        return stem.upper()
    digits, letters = m.group(1), m.group(2)
    return digits + letters.upper()


def match_frozen(rules_id: str, frozen_names: list[str]) -> str | None:
    if rules_id in FROZEN_OVERRIDE:
        return FROZEN_OVERRIDE[rules_id]
    stem = rules_id.split("-v")[0]
    year_m = re.search(r"-v(\d{4})", rules_id)
    year = year_m.group(1) if year_m else None
    letter_m = re.search(r"-v\d{4}([a-z]?)$", rules_id)
    letter = letter_m.group(1) if letter_m else ""

    cands = []
    for b in frozen_names:
        bm = re.match(r"^(.+)-(\d{4})([a-z]?)$", b)
        if not bm:
            continue
        bstem, byear, bletter = bm.group(1), bm.group(2), bm.group(3)
        if bstem.replace("-", "") != stem.replace("-", ""):
            continue
        cands.append((b, byear, bletter))

    if not cands:
        return None

    # Prefer year match, then revision letter, then exact shortest name
    scored = []
    for b, byear, bletter in cands:
        score = 0
        if year and byear == year:
            score += 10
        if letter and bletter == letter:
            score += 5
        if not letter and not bletter:
            score += 1
        scored.append((score, -len(b), b))
    scored.sort(reverse=True)
    return scored[0][2]


def discover() -> tuple[list[tuple[str, str, str]], list[str]]:
    rules = sorted(p.name for p in (ROOT / "rules/forms").iterdir() if p.is_dir())
    frozen = sorted(p.name for p in (ROOT / "html-frozen").iterdir() if p.is_dir())
    pairs = []
    blocked = []
    for rid in rules:
        b = match_frozen(rid, frozen)
        if not b:
            blocked.append(rid)
            continue
        pairs.append((rid, b, code_from_rules_id(rid)))

    def sort_key(t):
        rid = t[0]
        if rid in TYPED_ORDER:
            return (0, TYPED_ORDER.index(rid))
        return (1, rid)

    pairs.sort(key=sort_key)
    return pairs, blocked


def main() -> int:
    pairs, blocked = discover()
    print(f"discovered {len(pairs)} pairs; blocked {len(blocked)}")
    for rid in blocked:
        print(f"BLOCKED (no frozen HTML): {rid}")

    failures = []
    with tempfile.TemporaryDirectory(prefix="eli5-forms-") as tmp:
        tmp_path = Path(tmp)
        for rid, bundle, code in pairs:
            out = tmp_path / f"{code}.html"
            proc = subprocess.run(
                [sys.executable, str(TOOL), rid, bundle, str(out)],
                cwd=str(ROOT),
                capture_output=True,
                text=True,
            )
            lines = [ln for ln in (proc.stdout or "").splitlines() if ln.strip()]
            # form_page prints "path: drawn N / M OK|MISMATCH" then optional "? …" questions
            status = next((ln for ln in reversed(lines) if " drawn " in ln and (" OK" in ln or "MISMATCH" in ln)), "")
            for ln in lines:
                print(ln)
            if proc.returncode != 0:
                failures.append((rid, f"exit {proc.returncode}", (proc.stderr or "")[-500:]))
                continue
            if not status or "MISMATCH" in status or not status.rstrip().endswith("OK"):
                failures.append((rid, status or "(no status line)", (proc.stderr or "")[-500:]))

    if failures:
        print(f"\nFAIL {len(failures)} form(s):")
        for rid, line, err in failures:
            print(f"  {rid}: {line}")
            if err:
                print(f"    stderr: {err}")
        return 1
    print("\nALL OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
