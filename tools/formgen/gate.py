#!/usr/bin/env python3
"""The done-condition for tools/formgen/GOAL.md. Exit 0 means finished.

One command, so nobody has to decide whether the work is done by reading a
summary. Every check prints a line and a verdict.

The rule that matters most here: **a check that cannot be evaluated is a
failure, never a pass.** This project has already been burned by the opposite.
The numeric audit reported `rules 100% on 51/51` while 137 real defects were
present -- a black rectangle over a header, a seal printed upside-down, tax
brackets a taxpayer could type over -- because it only compared what it knew to
compare. An unimplemented assertion that silently passes is that same failure
wearing a green tick, so `UNEVALUABLE` is counted with the failures and named in
the summary.

Usage:
    python3 tools/formgen/gate.py                 # the real gate
    python3 tools/formgen/gate.py --only rules    # one check, while iterating
    python3 tools/formgen/gate.py --list          # what the checks are
    python3 tools/formgen/gate.py --self-test     # the gate checks itself
"""

from __future__ import annotations

import argparse
import dataclasses
import enum
import hashlib
import json
import pathlib
import subprocess
import sys
from typing import Callable, Iterable

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

FORMS = REPO / "forms"
BUILD = REPO / "build"
AUDIT_JSON = BUILD / "audit.json"
BATCH_REPORT = BUILD / "batch-report.json"
FINDINGS = HERE / "review-findings.json"

EXPECTED_FORMS = 51

# Modules that expose --self-test. lattice and fonts need an --ir argument, so
# they are invoked with one rather than being excused from the check.
SELF_TEST_MODULES = ("extract", "lattice", "fonts", "guides", "emit", "verify",
                     "index_page", "gate")

# The eight assertions from GOAL.md. gate.py does not implement them: audit.py
# owns them, and the gate's job is to demand them. Each maps to the key audit.py
# must publish per form in its record.
REQUIRED_ASSERTIONS = {
    "inputs_over_printed_text": "No <input> overlaps a pre-printed text run's bbox",
    "comb_slots_match_printed": "Every comb's slot count equals its printed compartment count",
    "money_boxes_have_inputs": "Every printed money box on a form page has an input",
    "rules_below_guide_cut": "No form-side rule extends below that page's guide cut",
    "run_colour_matches_ir": "No emitted run's colour differs from the IR's",
    "reflow_rate_without_description": "No relocated table row has an empty description and a rate",
    "image_transform_applied": "Every non-positive-diagonal image transform is emitted",
    "no_invented_codepoints": "No IR run holds a character the source did not state",
}


class Verdict(enum.Enum):
    PASS = "PASS"
    FAIL = "FAIL"
    UNEVALUABLE = "UNEVALUABLE"   # counted as a failure; see the module docstring

    @property
    def ok(self) -> bool:
        return self is Verdict.PASS


@dataclasses.dataclass
class Result:
    name: str
    verdict: Verdict
    detail: str


def run(args: list[str], timeout: int = 5400) -> tuple[int, str]:
    proc = subprocess.run([sys.executable, *args], cwd=REPO, timeout=timeout,
                          capture_output=True, text=True)
    return proc.returncode, (proc.stdout + proc.stderr)


def tree_digest(root: pathlib.Path) -> str:
    """Hash a directory's paths and contents, for the determinism check."""
    digest = hashlib.sha256()
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        digest.update(str(path.relative_to(root)).encode())
        digest.update(path.read_bytes())
    return digest.hexdigest()


def load(path: pathlib.Path) -> object | None:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:  # noqa: BLE001 - absent or unreadable is "cannot evaluate"
        return None


# ---------------------------------------------------------------------------
# checks
# ---------------------------------------------------------------------------


def check_self_tests() -> Result:
    failures, missing = [], []
    for module in SELF_TEST_MODULES:
        path = HERE / f"{module}.py"
        if not path.is_file():
            missing.append(module)
            continue
        args = [str(path), "--self-test"]
        # Two modules key their self-test off a form's IR rather than shipping a
        # fixture, so the gate supplies one instead of skipping them.
        if module in ("lattice", "fonts"):
            ir = BUILD / "ir" / "2551q-2018.ir.json"
            if not ir.is_file():
                missing.append(f"{module} (no IR to test against)")
                continue
            args += ["--ir", str(ir)]
        code, _ = run(args, timeout=900)
        if code != 0:
            failures.append(module)
    if missing:
        return Result("self-tests", Verdict.UNEVALUABLE,
                      f"cannot run: {', '.join(missing)}")
    if failures:
        return Result("self-tests", Verdict.FAIL, f"failing: {', '.join(failures)}")
    return Result("self-tests", Verdict.PASS,
                  f"{len(SELF_TEST_MODULES)} modules pass")


def check_conversion() -> Result:
    report = load(BATCH_REPORT)
    if not isinstance(report, list):
        return Result("conversion", Verdict.UNEVALUABLE, "no batch report")
    failed = [r for r in report if r.get("stage_failed")]
    if failed:
        return Result("conversion", Verdict.FAIL,
                      f"{len(failed)} failed: " +
                      ", ".join(f"{r['slug']}@{r['stage_failed']}" for r in failed[:5]))
    if len(report) != EXPECTED_FORMS:
        return Result("conversion", Verdict.FAIL,
                      f"{len(report)} forms, expected {EXPECTED_FORMS}")
    return Result("conversion", Verdict.PASS, f"{len(report)}/{EXPECTED_FORMS} converted")


def audit_records() -> list[dict] | None:
    data = load(AUDIT_JSON)
    if not isinstance(data, list):
        return None
    return [r for r in data if r.get("status") == "ok"]


def _tally(name: str, keys: Iterable[str], pct_key: str | None = None) -> Result:
    records = audit_records()
    if records is None:
        return Result(name, Verdict.UNEVALUABLE, "no audit report")
    if len(records) != EXPECTED_FORMS:
        return Result(name, Verdict.FAIL,
                      f"audit covers {len(records)}/{EXPECTED_FORMS} forms")
    bad: list[str] = []
    for key in keys:
        offenders = [r for r in records if r.get(key)]
        if offenders:
            total = sum(r.get(key, 0) for r in offenders)
            bad.append(f"{key}={total} on {len(offenders)} form(s) "
                       f"(e.g. {offenders[0]['slug']})")
    if pct_key:
        short = [r for r in records if r.get(pct_key) != 100.0]
        if short:
            worst = min(short, key=lambda r: r.get(pct_key) or 0)
            bad.append(f"{pct_key} below 100 on {len(short)} form(s), "
                       f"worst {worst['slug']} {worst.get(pct_key)}%")
    if bad:
        return Result(name, Verdict.FAIL, "; ".join(bad))
    return Result(name, Verdict.PASS, f"clean on {len(records)}/{EXPECTED_FORMS}")


def check_rules() -> Result:
    return _tally("rules", ("rules_missing", "rules_extra",
                            "rules_thickness_violations"), "rules_pct")


def check_paper() -> Result:
    records = audit_records()
    if records is None:
        return Result("paper", Verdict.UNEVALUABLE, "no audit report")
    bad = [r["slug"] for r in records if not r.get("paper_ok")]
    if bad:
        return Result("paper", Verdict.FAIL, f"{len(bad)} form(s): {', '.join(bad[:5])}")
    return Result("paper", Verdict.PASS, f"exact on {len(records)}/{EXPECTED_FORMS}")


def check_artwork() -> Result:
    return _tally("artwork", ("images_missing", "images_placement_violations"))


def check_text() -> Result:
    return _tally("text", ("text_missing", "text_extra"), "text_pct")


def check_assertions() -> Result:
    """The eight assertions that would have caught the 137 audit-blind defects.

    audit.py owns them. Until each publishes a boolean per form, this reports
    UNEVALUABLE, which the gate counts as a failure -- the whole point of the
    exercise is that an unchecked claim must not read as a satisfied one.
    """
    records = audit_records()
    if records is None:
        return Result("assertions", Verdict.UNEVALUABLE, "no audit report")
    absent = [k for k in REQUIRED_ASSERTIONS if not any(k in r for r in records)]
    if absent:
        return Result("assertions", Verdict.UNEVALUABLE,
                      f"{len(absent)}/{len(REQUIRED_ASSERTIONS)} not implemented in "
                      f"audit.py: {', '.join(sorted(absent))}")
    violations: list[str] = []
    for key, description in REQUIRED_ASSERTIONS.items():
        offenders = [r["slug"] for r in records if r.get(key) is not True]
        if offenders:
            violations.append(f"{key} fails on {len(offenders)} form(s) "
                              f"({description})")
    if violations:
        return Result("assertions", Verdict.FAIL, "; ".join(violations))
    return Result("assertions", Verdict.PASS,
                  f"all {len(REQUIRED_ASSERTIONS)} hold on {len(records)} forms")


def check_findings() -> Result:
    data = load(FINDINGS)
    if not isinstance(data, dict) or "findings" not in data:
        return Result("findings", Verdict.UNEVALUABLE, "no findings ledger")
    gating = [f for f in data["findings"] if f.get("severity") in ("blocker", "major")]
    unresolved = [f for f in gating
                  if f.get("status") not in ("fixed", "not-a-defect")
                  or not (f.get("resolution") or "").strip()]
    if unresolved:
        by_form: dict[str, int] = {}
        for f in unresolved:
            by_form[f["form"]] = by_form.get(f["form"], 0) + 1
        worst = sorted(by_form.items(), key=lambda kv: -kv[1])[:4]
        return Result("findings", Verdict.FAIL,
                      f"{len(unresolved)}/{len(gating)} blocker+major unresolved "
                      f"(worst: {', '.join(f'{k} {v}' for k, v in worst)})")
    return Result("findings", Verdict.PASS,
                  f"all {len(gating)} blocker+major resolved")


def check_determinism(regenerate: bool) -> Result:
    if not FORMS.is_dir():
        return Result("determinism", Verdict.UNEVALUABLE, "no forms/ tree")
    if not regenerate:
        return Result("determinism", Verdict.UNEVALUABLE,
                      "needs two regenerates; re-run without --only")
    first = tree_digest(FORMS)
    code, out = run([str(HERE / "batch.py"), "--report", str(BATCH_REPORT)])
    if code != 0:
        return Result("determinism", Verdict.FAIL,
                      f"second regenerate failed: {out.strip().splitlines()[-1:]}")
    second = tree_digest(FORMS)
    if first != second:
        return Result("determinism", Verdict.FAIL,
                      f"forms/ differs between runs ({first[:12]} vs {second[:12]})")
    return Result("determinism", Verdict.PASS, f"byte-identical ({first[:12]})")


def check_no_tracked_deletions() -> Result:
    proc = subprocess.run(["git", "status", "--porcelain", "--", "forms/"],
                          cwd=REPO, capture_output=True, text=True)
    if proc.returncode != 0:
        return Result("tracked-files", Verdict.UNEVALUABLE, "git status failed")
    deleted = [line[3:] for line in proc.stdout.splitlines()
               if line[:2].strip() in ("D", "AD") or line.startswith(" D")]
    if deleted:
        return Result("tracked-files", Verdict.FAIL,
                      f"{len(deleted)} tracked file(s) deleted: {', '.join(deleted[:3])}")
    return Result("tracked-files", Verdict.PASS, "no tracked deletion")


CHECKS: dict[str, Callable[[], Result]] = {
    "self-tests": check_self_tests,
    "conversion": check_conversion,
    "rules": check_rules,
    "paper": check_paper,
    "artwork": check_artwork,
    "text": check_text,
    "assertions": check_assertions,
    "findings": check_findings,
    "tracked-files": check_no_tracked_deletions,
}


def self_test() -> int:
    """Prove the gate can fail, and that it treats absence as failure.

    A gate that cannot fail is worthless, and one that passes on a missing check
    is worse than none at all -- so both properties are asserted rather than
    assumed.
    """
    failures = []
    if Verdict.UNEVALUABLE.ok:
        failures.append("UNEVALUABLE must not count as ok")
    if not Verdict.PASS.ok:
        failures.append("PASS must count as ok")

    missing = check_assertions()
    if missing.verdict is Verdict.PASS:
        failures.append("assertions reported PASS while audit.py does not "
                        "implement them")

    probe = Result("probe", Verdict.UNEVALUABLE, "x")
    if summarise([probe]) == 0:
        failures.append("an UNEVALUABLE check must make the gate exit non-zero")
    if summarise([Result("probe", Verdict.PASS, "x")]) != 0:
        failures.append("an all-PASS run must exit 0")

    if len(REQUIRED_ASSERTIONS) != 8:
        failures.append(f"GOAL.md names 8 assertions, gate has "
                        f"{len(REQUIRED_ASSERTIONS)}")

    for name in failures:
        print(f"FAIL {name}", file=sys.stderr)
    print(f"gate self-test: {len(failures)} failure(s)", file=sys.stderr)
    return 1 if failures else 0


def summarise(results: list[Result], echo: bool = False) -> int:
    width = max((len(r.name) for r in results), default=0)
    for r in results:
        if echo:
            print(f"  {r.verdict.value:<11} {r.name:<{width}}  {r.detail}")
    failed = [r for r in results if not r.verdict.ok]
    if echo:
        print()
        if failed:
            print(f"GATE FAILS -- {len(failed)} of {len(results)} checks not satisfied")
            for r in failed:
                print(f"  - {r.name}: {r.detail}")
        else:
            print(f"GATE PASSES -- all {len(results)} checks satisfied")
    return 1 if failed else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--only", action="append", choices=sorted(CHECKS) + ["determinism"],
                        help="Run one check while iterating. Not the done-condition.")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--skip-regenerate", action="store_true",
                        help="Score the forms/ tree as it stands. Not the done-condition.")
    parser.add_argument("--json", type=pathlib.Path, default=None)
    args = parser.parse_args(argv)

    if args.list:
        for name in list(CHECKS) + ["determinism"]:
            print(name)
        return 0
    if args.self_test:
        return self_test()

    full = not args.only
    if full and not args.skip_regenerate:
        print("regenerating and auditing (this takes a while)...", file=sys.stderr)
        code, out = run([str(HERE / "batch.py"), "--report", str(BATCH_REPORT)])
        if code != 0:
            print(f"  batch.py failed:\n{out[-2000:]}", file=sys.stderr)
        code, out = run([str(HERE / "audit.py"), "--out", str(AUDIT_JSON)])
        if code != 0:
            print(f"  audit.py failed:\n{out[-2000:]}", file=sys.stderr)

    wanted = args.only or list(CHECKS)
    results = [CHECKS[name]() for name in wanted if name in CHECKS]
    if "determinism" in wanted or full:
        results.append(check_determinism(regenerate=full and not args.skip_regenerate))

    print(f"\nformgen gate -- {len(results)} checks\n")
    exit_code = summarise(results, echo=True)

    if args.json:
        args.json.write_text(json.dumps(
            [{"name": r.name, "verdict": r.verdict.value, "detail": r.detail}
             for r in results], indent=2) + "\n", encoding="utf-8")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
