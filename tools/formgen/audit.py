#!/usr/bin/env python3
"""Round-trip every generated form and score it, so the index reports fact.

The index page previously coloured rows by whether a form was in the 35-form
corpus, which reads like a quality badge but measures membership. This produces
the real signal: print each generated bundle to PDF with Chromium, re-extract it
with the same extractor, and diff against the source IR.

Scoring is deliberately blunt -- rules and text runs recovered, as percentages.
A form that renders every rule and every string at tolerance is "clean"; one
that drops structure is not, and the number says how badly.
"""

from __future__ import annotations

import argparse
import copy
import json
import pathlib
import sys
import traceback

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import extract  # noqa: E402
import verify  # noqa: E402


def form_side(reference: dict, plan: dict | None) -> tuple[dict, dict]:
    """Drop everything the guide plan moved out, from the reference IR.

    The form document no longer contains the guide's rules and strings, so
    scoring it against the whole source IR counts correctly-relocated content as
    missing. That is how a corpus at 100% rules came to read 42/51: nothing had
    moved on the sheet, the denominator was simply the wrong one.

    Indices are removed high-to-low so earlier removals cannot shift later ones.
    """
    if not plan or not plan.get("inline"):
        return reference, {"rules": 0, "text_runs": 0, "images": 0}

    filtered = copy.deepcopy(reference)
    removed = {"rules": 0, "text_runs": 0, "images": 0}
    by_page = {region["page"]: region for region in plan["inline"]}

    for page in filtered["pages"]:
        region = by_page.get(page["index"])
        if region is None:
            continue

        claimed_rules = set(region.get("rule_ids") or ())
        if claimed_rules:
            before = len(page["rules"])
            page["rules"] = [r for r in page["rules"] if r["id"] not in claimed_rules]
            removed["rules"] += before - len(page["rules"])

        for index in sorted(region.get("text_run_indices") or (), reverse=True):
            if 0 <= index < len(page["text_runs"]):
                del page["text_runs"][index]
                removed["text_runs"] += 1

        for index in sorted(region.get("image_indices") or (), reverse=True):
            if 0 <= index < len(page["images"]):
                del page["images"][index]
                removed["images"] += 1

        # stats are what the rule denominator is read from, so they must follow.
        page["stats"]["rules_structural"] = sum(
            1 for r in page["rules"] if r["role"] == "structural")

    return filtered, removed


def score(slug: str, ir_path: pathlib.Path, html_path: pathlib.Path,
          work: pathlib.Path, guide_dir: pathlib.Path | None = None) -> dict:
    record: dict = {"slug": slug, "status": "error", "error": None}
    try:
        reference = json.loads(ir_path.read_text())

        plan = None
        if guide_dir is not None:
            plan_path = guide_dir / f"{slug}.guide.json"
            if plan_path.is_file():
                plan = json.loads(plan_path.read_text())
        reference, relocated = form_side(reference, plan)
        record["guide_relocated"] = relocated

        pdf = work / f"{slug}.audit.pdf"
        pdf.parent.mkdir(parents=True, exist_ok=True)

        paper = reference["paper"]
        verify.html_to_pdf(html_path, pdf, paper["width_pt"], paper["height_pt"])

        candidate = extract.extract(pdf, reference["form"]["code"],
                                    reference["form"]["revision"], None)
        report = verify.diff_ir(reference, candidate, verify.Tolerances(), roles=["structural"])
        totals = report.get("totals", {})

        # Denominators come from the source IR, so a percentage always answers
        # "of what the official form contains, how much did we reproduce".
        rules_ref = sum(p["stats"]["rules_structural"] for p in reference["pages"])
        text_ref = sum(len(p["text_runs"]) for p in reference["pages"])
        rules_missing = totals.get("rules_missing", 0)
        text_missing = totals.get("text_missing", 0)

        record.update({
            "status": "ok",
            "paper_ok": bool(report.get("paper", {}).get("ok", True)),
            "rules_ref": rules_ref,
            "rules_missing": rules_missing,
            "rules_extra": totals.get("rules_extra", 0),
            "rules_thickness_violations": totals.get("rules_thickness_violations", 0),
            "rules_pct": round(100.0 * (rules_ref - rules_missing) / rules_ref, 2) if rules_ref else None,
            "text_ref": text_ref,
            "text_missing": text_missing,
            "text_extra": totals.get("text_extra", 0),
            "text_pct": round(100.0 * (text_ref - text_missing) / text_ref, 2) if text_ref else None,
            "images_missing": totals.get("images_missing", 0),
            "images_placement_violations": totals.get("images_placement_violations", 0),
        })
    except Exception as exc:  # noqa: BLE001 - one bad form must not stop the sweep
        record["error"] = f"{type(exc).__name__}: {exc}"
        record["trace"] = traceback.format_exc(limit=3)
    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ir-dir", type=pathlib.Path, default=pathlib.Path("build/ir"))
    parser.add_argument("--html-dir", type=pathlib.Path, default=pathlib.Path("build/html"))
    parser.add_argument("--work", type=pathlib.Path, default=pathlib.Path("build/audit"))
    parser.add_argument("--guide-dir", type=pathlib.Path, default=pathlib.Path("build/guides"),
                        help="Guide plans; content moved to guide.html leaves the form denominator.")
    parser.add_argument("--out", type=pathlib.Path, default=pathlib.Path("build/audit.json"))
    parser.add_argument("--only", action="append", default=None)
    args = parser.parse_args()

    slugs = sorted(p.name[: -len(".ir.json")] for p in args.ir_dir.glob("*.ir.json"))
    if args.only:
        wanted = {s.lower() for s in args.only}
        slugs = [s for s in slugs if any(w in s for w in wanted)]

    records = []
    for i, slug in enumerate(slugs, 1):
        html = args.html_dir / f"{slug}.html"
        if not html.is_file():
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} no html", file=sys.stderr)
            continue
        record = score(slug, args.ir_dir / f"{slug}.ir.json", html, args.work,
                       args.guide_dir if args.guide_dir.is_dir() else None)
        records.append(record)
        if record["status"] == "ok":
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} "
                  f"rules {record['rules_pct']:>6}%  text {record['text_pct']:>6}%",
                  file=sys.stderr)
        else:
            print(f"[{i:>2}/{len(slugs)}] {slug:<26} ERROR {record['error'][:60]}",
                  file=sys.stderr)
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
