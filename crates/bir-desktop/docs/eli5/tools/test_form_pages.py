#!/usr/bin/env python3
"""Discover rules/forms ↔ html-frozen pairs, run form_page.py,
exit non-zero on any drawn/field_count MISMATCH.

Pass --write to regenerate crates/bir-desktop/docs/eli5/forms/<CODE>.html
and refresh forms/README.md.
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = next(p for p in Path(__file__).resolve().parents if (p / "rules" / "forms").is_dir())
TOOL = Path(__file__).resolve().parent / "form_page.py"
FORMS_DIR = ROOT / "crates/bir-desktop/docs/eli5/forms"

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
    frozen = sorted(
        p.name
        for p in (ROOT / "html-frozen").iterdir()
        if p.is_dir() and p.name not in {"assets", "fonts", "keys"}
    )
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


def section_bodies(html_text: str) -> list[tuple[str, str]]:
    found = []
    for m in re.finditer(
        r'<details[^>]*class="sec"><summary>(.*?)</summary>(.*?)</details>',
        html_text,
        re.S,
    ):
        title = re.sub(r"<[^>]+>", "", m.group(1))
        title = re.sub(r"\s+", " ", title).strip()
        found.append((title, m.group(2)))
    return found


def check_sidecar(path: Path, field_count: int, code: str) -> list[str]:
    errs = []
    if not path.is_file():
        return [f"sidecar missing: {path}"]
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as err:
        return [f"sidecar is not JSON: {err}"]
    if data.get("form_code") != code:
        errs.append(f"sidecar form_code {data.get('form_code')!r} != {code}")
    if data.get("field_count") != field_count:
        errs.append(f"sidecar field_count {data.get('field_count')!r} != {field_count}")
    seen: set[str] = set()
    n = 0
    has_print_xml = False
    for section in data.get("sections") or []:
        sid = section.get("id")
        if sid == "print_xml":
            has_print_xml = True
            if not section.get("editor_hidden"):
                errs.append("print_xml section must set editor_hidden")
        for key in section.get("fields") or []:
            n += 1
            if key in seen:
                errs.append(f"sidecar duplicates field {key}")
            seen.add(key)
    if n != field_count:
        errs.append(f"sidecar lists {n} fields, expected {field_count}")
    if code == "2551Q":
        if not has_print_xml:
            errs.append("2551Q sidecar missing print_xml section")
        else:
            print_xml = next(s for s in data["sections"] if s["id"] == "print_xml")
            blob = " ".join(print_xml.get("fields") or [])
            if "txtPg2TIN1" not in blob or "txtPg2TaxpayerName" not in blob:
                errs.append("2551Q sidecar print_xml missing page-2 TIN/name repeats")
        identity = next((s for s in data["sections"] if s.get("id") == "identity"), None)
        if identity is None:
            errs.append("2551Q sidecar missing identity section")
        else:
            blob = " ".join(identity.get("fields") or [])
            for key in ("txtTIN1", "txtRDOCode", "registeredName"):
                if key not in blob:
                    errs.append(f"2551Q sidecar identity missing {key}")
    return errs


def write_inventory_catalog(pairs: list[tuple[str, str, str]]) -> None:
    dest = ROOT / "crates/bir-core/src/forms/inventory_catalog.rs"
    lines = [
        "// @generated by crates/bir-desktop/docs/eli5/tools/test_form_pages.py --write",
        "// Do not edit by hand. Sidecars live under crates/bir-desktop/docs/eli5/forms/.",
        "",
        "/// One compiled-in rules bundle + UX section sidecar.",
        "pub struct InventoryBundle {",
        "    pub code: &'static str,",
        "    pub rules_id: &'static str,",
        "    pub frozen_bundle: &'static str,",
        "    pub fields_json: &'static str,",
        "    pub sections_json: &'static str,",
        "}",
        "",
        "pub const INVENTORY_BUNDLES: &[InventoryBundle] = &[",
    ]
    for rid, bundle, code in pairs:
        lines.append("    InventoryBundle {")
        lines.append(f'        code: "{code}",')
        lines.append(f'        rules_id: "{rid}",')
        lines.append(f'        frozen_bundle: "{bundle}",')
        lines.append(
            f'        fields_json: include_str!("../../../../rules/forms/{rid}/fields.json"),'
        )
        lines.append(
            f'        sections_json: include_str!("../../../bir-desktop/docs/eli5/forms/{code}.sections.json"),'
        )
        lines.append("    },")
    lines.append("];")
    lines.append("")
    dest.write_text("\n".join(lines) + "\n")


def check_2551q_canary(html_text: str) -> list[str]:
    """Keep 2551Q OK: identity + print/XML buckets, no dropped fields, workflow off the schedule."""
    errs = []
    if "✓ 99 of 99 fields drawn" not in html_text:
        errs.append("2551Q count line is not '✓ 99 of 99 fields drawn'")
    bodies = section_bodies(html_text)
    identity = next((b for t, b in bodies if t.startswith("Taxpayer identity")), None)
    print_xml = next((b for t, b in bodies if t.startswith("Print / XML only")), None)
    payment = next((b for t, b in bodies if t.startswith("Payment / signature")), None)
    schedule = next((b for t, b in bodies if "Schedule" in t), None)
    if identity is None:
        errs.append("2551Q missing UX section 'Taxpayer identity (profile-sourced)'")
    else:
        for key in ("txtTIN1", "txtRDOCode", "registeredName"):
            if key not in identity:
                errs.append(f"2551Q identity section missing {key}")
    if print_xml is None:
        errs.append("2551Q missing UX section 'Print / XML only (hidden in editor)'")
    else:
        if "txtPg2TIN1" not in print_xml or "txtPg2TaxpayerName" not in print_xml:
            errs.append("2551Q print/XML section missing page-2 TIN/name repeats")
    if schedule is not None and "ebirOnlineUsername" in schedule:
        errs.append("2551Q workflow field ebirOnlineUsername was drawn under Schedule")
    if payment is not None and "ebirOnlineUsername" not in payment:
        errs.append("2551Q workflow field ebirOnlineUsername is not under Payment / workflow")
    return errs


def write_readme(rows: list[dict], blocked: list[str]) -> None:
    lines = [
        "# ELI5 form field pages",
        "",
        "Generated by `../tools/form_page.py` from `rules/forms/<id>/{fields,calculations,validations,workflow}.json` and `html-frozen/<bundle>/index.html`.",
        "Sections are UX-oriented (paper Parts/Schedules are reference). Verify with:",
        "",
        "```bash",
        "python3 crates/bir-desktop/docs/eli5/tools/test_form_pages.py",
        "```",
        "",
        "| code | revision | fields drawn/field_count | calculations | validation rules | open questions |",
        "|---|---|---|---|---|---|",
    ]
    for row in rows:
        q = row["questions"] or "—"
        lines.append(
            f"| {row['code']} | {row['revision']} | {row['drawn']}/{row['count']} | "
            f"{row['calcs']} | {row['vals']} | {q} |"
        )
    lines += [
        "",
        "## BLOCKED (no frozen HTML)",
        "",
    ]
    if blocked:
        for rid in blocked:
            lines.append(f"- `{rid}`")
    else:
        lines.append("_None — every rules bundle has a matching html-frozen pair._")
    lines.append("")
    FORMS_DIR.mkdir(parents=True, exist_ok=True)
    (FORMS_DIR / "README.md").write_text("\n".join(lines))


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    write = "--write" in argv
    pairs, blocked = discover()
    print(f"discovered {len(pairs)} pairs; blocked {len(blocked)}")
    for rid in blocked:
        print(f"BLOCKED (no frozen HTML): {rid}")

    if write:
        FORMS_DIR.mkdir(parents=True, exist_ok=True)
        return _run_pairs(pairs, blocked, FORMS_DIR, write=True)

    with tempfile.TemporaryDirectory(prefix="eli5-forms-") as tmp:
        return _run_pairs(pairs, blocked, Path(tmp), write=False)


def _run_pairs(
    pairs: list[tuple[str, str, str]],
    blocked: list[str],
    dest_root: Path,
    *,
    write: bool,
) -> int:
    failures = []
    rows = []
    for rid, bundle, code in pairs:
        out = dest_root / f"{code}.html"
        proc = subprocess.run(
            [sys.executable, str(TOOL), rid, bundle, str(out)],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
        )
        lines = [ln for ln in (proc.stdout or "").splitlines() if ln.strip()]
        status = next(
            (
                ln
                for ln in reversed(lines)
                if " drawn " in ln and (" OK" in ln or "MISMATCH" in ln)
            ),
            "",
        )
        questions = [
            ln[4:].strip() if ln.startswith("  ? ") else ln.lstrip()[2:].strip()
            for ln in lines
            if ln.lstrip().startswith("? ")
        ]
        for ln in lines:
            print(ln)
        if proc.returncode != 0:
            failures.append((rid, f"exit {proc.returncode}", (proc.stderr or "")[-500:]))
            continue
        if not status or "MISMATCH" in status or not status.rstrip().endswith("OK"):
            failures.append((rid, status or "(no status line)", (proc.stderr or "")[-500:]))
            continue
        html_text = out.read_text(errors="replace")
        if code == "2551Q":
            for err in check_2551q_canary(html_text):
                failures.append((rid, err, ""))
        sidecar_path = out.with_suffix(".sections.json")
        fields = json.loads((ROOT / "rules/forms" / rid / "fields.json").read_text())
        for err in check_sidecar(sidecar_path, int(fields["field_count"]), code):
            failures.append((rid, err, ""))
        committed_sidecar = FORMS_DIR / f"{code}.sections.json"
        if not write:
            if not committed_sidecar.is_file():
                failures.append((rid, f"committed sidecar missing: {committed_sidecar}", ""))
            else:
                for err in check_sidecar(committed_sidecar, int(fields["field_count"]), code):
                    failures.append((rid, f"committed sidecar: {err}", ""))
        calcs = json.loads((ROOT / "rules/forms" / rid / "calculations.json").read_text())
        vals = json.loads((ROOT / "rules/forms" / rid / "validations.json").read_text())
        n_calcs = len(calcs.get("evaluation_order") or [])
        n_vals = len(
            vals["rules"]
            if isinstance(vals.get("rules"), list)
            else list((vals.get("rules") or {}).values())
        )
        m = re.search(r"drawn (\d+) / (\d+)", status)
        drawn, count = (m.group(1), m.group(2)) if m else ("?", "?")
        rows.append(
            {
                "code": code,
                "revision": str(fields.get("revision") or ""),
                "drawn": drawn,
                "count": count,
                "calcs": n_calcs,
                "vals": n_vals,
                "questions": " ".join(questions) if questions else "",
            }
        )

        committed = FORMS_DIR / f"{code}.html"
        if not write:
            if not committed.is_file():
                failures.append((rid, f"committed page missing: {committed}", ""))
            else:
                committed_text = committed.read_text(errors="replace")
                if f"{drawn} of {count} fields drawn" not in committed_text:
                    failures.append(
                        (
                            rid,
                            f"committed page count line does not show {drawn} of {count}",
                            "",
                        )
                    )

    if write:
        write_readme(rows, blocked)
        write_inventory_catalog(pairs)
        print(f"wrote {len(rows)} pages + README.md under {FORMS_DIR}")
        print(f"wrote inventory catalog for {len(pairs)} bundles")

    if failures:
        print(f"\nFAIL {len(failures)} check(s):")
        for rid, line, err in failures:
            print(f"  {rid}: {line}")
            if err:
                print(f"    stderr: {err}")
        return 1
    print("\nALL OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
