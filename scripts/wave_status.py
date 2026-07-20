#!/usr/bin/env python3
"""Report, per form, which Wave 0 exit criteria are met — and which are not.

WHY THIS EXISTS. "Is Wave 0 done?" was being answered from memory, which is how
a wave stays open indefinitely: without a checkable definition of done, every
answer is a judgement call and the work never converges. This computes the
answer from the repository instead, so progress is a fact rather than a claim
and anyone (including the next agent) can see exactly what remains.

It is a STATUS REPORT, never a gate and never promotion evidence. It reports
what exists; it does not certify that what exists is correct. A form with every
box ticked here has satisfied Wave 0's *retrofit* checklist — it has not been
released, and `release_ready` stays false until the separate native/offline/
rollback evidence chain exists.

Exit criteria per form, each independently checkable:

  chromium_reference   a same-rasterizer chromium reference is pinned in Rust
  spec_retargeted      the parity spec measures against that reference
  geometry_contract    a contract has been extracted from the pinned PDF
  capacity_reviewed    comb capacities were checked against official dividers
  weight_reviewed      stroke weights were checked, with the grey test applied
  static_text          an exhaustive static-text manifest exists
  region_table         a critical-region table exists for region-ranked diffs
  structure_diff       the text-neutralized structural artifact is produced

Usage:
    rtk python3 scripts/wave_status.py            # table + what remains
    rtk python3 scripts/wave_status.py --json     # machine-readable
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# (code, revision, rust module stem, spec stem, reference stem)
FORMS = [
    ("0605", "1999", "form_0605", "form-0605", "0605-1999"),
    ("0619E", "2018", "form_0619e", "form-0619e", "0619e-2018"),
    ("0619F", "2018", "form_0619f", "form-0619f", "0619f-2018"),
    ("1601C", "2018", "form_1601c", "form-1601c", "1601c-2018"),
    ("1701", "2018", "form_1701", "form-1701", "1701-2018"),
    ("1701Q", "2018", "form_1701q", "form-1701q", "1701q-2018"),
    ("1702MX", "2018C", "form_1702mx", "form-1702mx", "1702mx-2018c"),
    ("1702RT", "2018C", "form_1702rt", "form-1702rt", "1702rt-2018c"),
    ("2550Q", "2024", "form_2550q", "form-2550q", "2550q-2024"),
    ("2551Q", "2018", "form_2551q", "form", "2551q-2018"),
]

CRITERIA = [
    "chromium_reference",
    "spec_retargeted",
    "geometry_contract",
    "capacity_reviewed",
    "weight_reviewed",
    "static_text",
    "region_table",
    "structure_diff",
]

# Wave 0 recorded these two reviews as commits rather than as tracked artifacts,
# because the output is a decision (apply / refute), not a file. Reading the log
# keeps this honest: the criterion is met only if the commit actually landed.
#
# These are EXACT prefixes, deliberately. The first draft used a loose regex
# (`weight defect|weight fixes|...`) and it immediately reported weight review
# complete on all ten forms by matching "Teach the localizer to tell weight
# defects from displacement" — a tooling commit from days earlier. A status
# report that says done when nothing was done is worse than no report, so
# substring-matching a specific subject line is the only acceptable form here.
CAPACITY_COMMIT = "Correct 23 charbox capacities across seven forms"
# Subjects that count as a committed weight/stroke correction. 2551Q's landed
# before this wave had a naming convention, under its own subject; listing it
# explicitly is honest, whereas loosening the match to catch it would reopen
# the false-positive hole described above.
WEIGHT_COMMITS = (
    "Apply the verified stroke-weight corrections",
    "Fix 2551Q page-one structural displacement",
)


def git_log_subjects() -> list[str]:
    import subprocess

    try:
        out = subprocess.run(
            ["git", "log", "--format=%s", "-80"],
            cwd=REPO, capture_output=True, text=True, timeout=30,
        )
        return out.stdout.splitlines() if out.returncode == 0 else []
    except Exception:
        return []


def weight_reviewed_forms() -> set[str]:
    """Which forms actually received a committed weight correction.

    PER FORM, deliberately. A whole-repo subject match reported weight review
    complete on all ten the moment one commit landed covering three of them —
    while the other six had just been REVERTED for causing clipping. Since this
    report is the stop condition for the Wave 0 goal loop, a false green ends
    the work early with six forms unfixed. So the question asked here is the
    narrow, checkable one: did a weight commit touch THIS form's stylesheet?
    """
    import subprocess

    reviewed: set[str] = set()
    try:
        out = subprocess.run(
            ["git", "log", "--format=%H %s", "-80"],
            cwd=REPO, capture_output=True, text=True, timeout=30,
        )
        if out.returncode != 0:
            return reviewed
        for line in out.stdout.splitlines():
            sha, _, subject = line.partition(" ")
            if not any(w in subject for w in WEIGHT_COMMITS):
                continue
            files = subprocess.run(
                ["git", "show", "--name-only", "--format=", sha],
                cwd=REPO, capture_output=True, text=True, timeout=30,
            )
            for path in files.stdout.splitlines():
                match = re.search(r"src/forms/Form([0-9A-Za-z]+)\.(css|tsx)$", path)
                if match:
                    reviewed.add(match.group(1).upper())
                # 2551Q is the exception: it has no Form2551Q.css because it was
                # the first form migrated and its rules live in the shared
                # print.css. Matching only src/forms/ reported it unreviewed
                # despite its structural corrections being committed and
                # verified against both rasterizers.
                elif path.endswith("src/print.css"):
                    reviewed.add("2551Q")
    except Exception:
        return reviewed
    return reviewed


def evaluate(subjects: list[str]) -> list[dict]:
    rows = []
    capacity_done = any(CAPACITY_COMMIT in s for s in subjects)
    weight_forms = weight_reviewed_forms()

    for code, revision, rust_stem, spec_stem, ref_stem in FORMS:
        rust = REPO / f"crates/bir-print/src/html_forms/{rust_stem}.rs"
        spec = REPO / f"packages/form-renderer/visual/{spec_stem}-parity.spec.ts"
        rust_text = rust.read_text(encoding="utf-8") if rust.is_file() else ""
        spec_text = spec.read_text(encoding="utf-8") if spec.is_file() else ""

        refs = REPO / "packages/form-renderer/references"
        chromium = any(refs.glob(f"{ref_stem}-page-*-chromium.png"))
        # A pin in Rust is what makes a reference load-bearing; a loose PNG is not.
        pinned = "CHROMIUM_REFERENCES" in rust_text and "chromium_references: None" not in rust_text
        static_text = (
            any(refs.glob(f"{ref_stem}-static-text-inventory.json"))
            or any(refs.glob(f"{ref_stem}-official-static-text.json"))
        )
        region_table = (
            REPO / f"packages/form-renderer/visual/regions/{code.lower()}.ts"
        ).is_file()
        # Must be a TRACKED artifact. The first draft accepted a contract in
        # /tmp, which is both ephemeral and untracked — exactly what CLAUDE.md
        # forbids as evidence. A contract nobody can review after the scratch
        # directory is cleared has not been reviewed.
        contract = (
            REPO / f"packages/form-specs/geometry-contracts/{ref_stem}.json"
        ).is_file()

        rows.append({
            "form": f"{code}:{revision}",
            "chromium_reference": chromium and pinned,
            "spec_retargeted": "-chromium.png" in spec_text,
            "geometry_contract": contract,
            "capacity_reviewed": capacity_done,
            "weight_reviewed": code.upper() in weight_forms,
            "static_text": static_text,
            "region_table": region_table,
            "structure_diff": "structure-diff" in spec_text or "structural_diff" in spec_text,
        })
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    args = parser.parse_args()

    rows = evaluate(git_log_subjects())

    if args.json:
        print(json.dumps({"criteria": CRITERIA, "forms": rows}, indent=2))
        return 0

    width = max(len(r["form"]) for r in rows) + 2
    header = "form".ljust(width) + " ".join(c[:9].rjust(9) for c in CRITERIA)
    print(header)
    print("-" * len(header))
    for row in rows:
        line = row["form"].ljust(width)
        line += " ".join(("yes" if row[c] else "NO").rjust(9) for c in CRITERIA)
        print(line)

    remaining: dict[str, list[str]] = {}
    for row in rows:
        missing = [c for c in CRITERIA if not row[c]]
        if missing:
            remaining[row["form"]] = missing

    print()
    if not remaining:
        print("Wave 0 retrofit checklist COMPLETE for all forms.")
        print()
        print("This is not a release claim. Baselines remain unpinned while the")
        print("criterion's blocking preconditions stand (see")
        print("docs/form-print-readiness/official-fidelity-criterion-v1.md section 8),")
        print("and release_ready stays false for every form.")
        return 0

    print(f"{len(remaining)} form(s) with open criteria:")
    for form, missing in remaining.items():
        print(f"  {form}: {', '.join(missing)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
