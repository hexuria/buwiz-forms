#!/usr/bin/env python3
"""Decide which parts of a BIR sheet are reference material rather than form.

A BIR sheet carries two documents printed on the same paper. One is the form:
the boxes a taxpayer fills in, whose geometry is the whole point of this
pipeline. The other is reference material -- ATC code tables, "Guidelines and
Instructions", penalty schedules -- which nobody fills in and which the BIR does
not require on the filed sheet.

Splitting them is worth doing twice over:

  * It frees space. The guide region occupies a mean 67% of the page it sits on
    (min 38%, max 93%), and that is exactly the room a growable band needs
    before it has to spill onto a continuation page.
  * It separates content whose parity does not matter from content whose parity
    is the entire objective. A mis-set line in an ATC table costs nothing; a
    mis-set line in a money comb is the bug we are here to prevent.

How the cut is found
--------------------

Lexically, by three STRICT markers. Strictness is the load-bearing part. A
loose /penalt/ pattern was tried and is wrong: it matches "Add: Penalties",
which is a *form* line on 1601C, 1603Q, 1600-PT and 2551Q, and cutting there
would throw away fillable fields.

Structurally, by what lies below the candidate. The lexical hit only proposes a
cut; the region below it has to look like reference material to be accepted --
substantial prose (>= 25 text runs) and essentially nothing fillable
(<= max(4, 5% of the page's field cells)). That second test is what actually
decides, and it is what rejects the ATC-code *column headers* that appear on
the fillable face of 1700, 1701, 1701Q, 1702EX and friends: those headings sit
above a hundred field cells, so no cut is taken.

Membership is geometric. An element belongs to the guide only if it lies wholly
below the cut. Anything crossing the cut is a straddler, and **the form always
wins a straddler** -- losing a rule off the form is a defect, carrying a stray
guide rule on the form is cosmetic. Straddlers are reported individually
because they are where a split can go wrong.

Standalone guides
-----------------

Some forms ship their guide as a separate PDF instead of, or as well as, an
inline region. batch.py deliberately skips those files -- converting an
instruction sheet into a fillable-looking bundle is worse than skipping it --
but they *are* the guide for their parent form, so this module enumerates them
and maps each back to a form code and revision using the same folder-name logic
batch.py's classify() applies. batch.py is not modified; the mapping is
exported from here.

Nothing here rasterises anything. Every decision is a coordinate or a string.

Usage:
    python3 tools/formgen/guides.py --ir build/ir/1603q-2018.ir.json \\
        --layout build/layout/1603q-2018.layout.json \\
        --out build/guides/1603q-2018.guide.json --summary
    python3 tools/formgen/guides.py --corpus
    python3 tools/formgen/guides.py --self-test
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import pathlib
import re
import sys
from typing import Any, Iterable, Sequence

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

# Strict, and deliberately so. Each pattern was checked against the whole corpus
# for false positives on form content before it was admitted.
#   - the en dash in the "table" pattern is what BIR actually prints; the hyphen
#     and em dash are there because three sheets differ.
#   - "^\s*table \d+" is anchored: "see Table 2 below" is prose inside a form.
MARKERS: tuple[tuple[str, re.Pattern[str]], ...] = (
    ("guidelines-and-instructions", re.compile(r"guidelines?\s+and\s+instructions?\s+for", re.I)),
    ("table-n", re.compile(r"^\s*table\s+\d+\s*[-–—]", re.I)),
    ("alphanumeric-tax-code", re.compile(r"alphanumeric tax code", re.I)),
)

# A guide region is prose. Fewer runs than this and the "marker" is a stray
# heading with nothing under it -- 1701MS page 2 ends with a two-row rate table
# 14 runs long, which is form content, not a guide.
MIN_GUIDE_TEXT_RUNS = 25

# ...and a guide region has essentially nothing fillable in it. The floor of 4
# absorbs the one or two spurious slivers the lattice finds at a table's edge
# (2000-OT page 2 has two, 1600-PT four); the 5% term keeps the allowance
# proportionate on dense sheets rather than fixed.
GUIDE_FIELD_CELL_FLOOR = 4
GUIDE_FIELD_CELL_FRACTION = 0.05

# batch.py skips these; for this module they are the target, not the noise.
GUIDE_NAME_MARKERS = ("guide", "guidelines", "instruction", "annex")

# Owned here and imported by batch.py, which already depends on this module.
# It used to be duplicated in both files with a self-test asserting the copies
# agreed; the test did its job and caught the drift, but a constant that can
# drift at all is the defect. One definition cannot disagree with itself.
REVISION_OVERRIDES = {
    "0605": "1999",     # printed in the sheet body; matches the repo's existing pin
    # Neither folder ("2550M", "2551M") nor file name ("bir2550m.pdf",
    # "2551m.pdf") carries a year, so discovery fell back to "undated". Both
    # sheets do stamp the revision, in the masthead where every other BIR form
    # stamps it -- the year simply never reached classify(), which only reads
    # names. The PDF metadata is *not* the evidence for either (2550M's says
    # created 2007-04-15 by "Acrobat PDFWriter 5.0", 2551M's 2003-02-14 by
    # "Acrobat PDFWriter 4.05"): that dates the file, not the form.
    "2550M": "2007",    # p1 masthead "February 2007 (ENCS)" at (468.96, 69.39) pt,
                        # repeated in the p2/p3 running head:
                        # "BIR  Form 2550M  -  February 2007 (ENCS)      Page 2"
    "2551M": "2002",    # p1 masthead "April  2002  (ENCS)" at (476.4, 99.36) pt
}

SCHEMA_VERSION = 1
DEFAULT_SOURCE_ROOT = pathlib.Path("~/Downloads/forms").expanduser()


# --------------------------------------------------------------------------
# detection
# --------------------------------------------------------------------------

@dataclasses.dataclass(frozen=True)
class Candidate:
    """A lexical hit that proposes a cut, before the structural test runs."""
    cut_y: float
    marker: str
    pattern: str


@dataclasses.dataclass(frozen=True)
class Straddler:
    """An element crossing the cut. Awarded to the form, reported regardless."""
    kind: str                # rule | area_fill | image | text_run | cell
    ref: str                 # rule/cell id, or "#<index>" for the index-keyed lists
    x0: float
    y0: float
    x1: float
    y1: float
    detail: str              # why it is interesting: cell kind, fill tone, ...

    def as_dict(self) -> dict[str, Any]:
        return {"kind": self.kind, "ref": self.ref, "x0": self.x0, "y0": self.y0,
                "x1": self.x1, "y1": self.y1, "detail": self.detail}


def marker_hit(text: str) -> tuple[str, str] | None:
    """Return (matched substring, pattern name) for the first marker that fires."""
    for name, pattern in MARKERS:
        match = pattern.search(text)
        if match:
            return match.group(0), name
    return None


def page_candidates(text_runs: Sequence[dict[str, Any]]) -> list[Candidate]:
    """Every lexical hit on the page, topmost first.

    Keyed on the run's top edge, not its baseline: the cut has to fall above the
    heading's own glyphs or the heading stays behind on the form.
    """
    found: dict[float, Candidate] = {}
    for run in text_runs:
        hit = marker_hit(run["text"])
        if hit is None:
            continue
        cut_y = run["y0"]
        # Two markers can fire on one line ("Table 1 - Alphanumeric Tax Code").
        # First wins, so the reported marker is stable under pattern reordering
        # only if MARKERS is; that ordering is fixed above.
        if cut_y not in found:
            found[cut_y] = Candidate(cut_y=cut_y, marker=run["text"].strip(), pattern=hit[1])
    return [found[y] for y in sorted(found)]


def field_cell_allowance(field_cell_count: int) -> float:
    return max(float(GUIDE_FIELD_CELL_FLOOR), GUIDE_FIELD_CELL_FRACTION * field_cell_count)


def below(items: Iterable[dict[str, Any]], cut_y: float) -> list[dict[str, Any]]:
    """Elements lying wholly below the cut. Touching the cut counts as below."""
    return [item for item in items if item["y0"] >= cut_y]


def straddling(items: Iterable[dict[str, Any]], cut_y: float) -> list[dict[str, Any]]:
    return [item for item in items if item["y0"] < cut_y < item["y1"]]


def structurally_valid(cut_y: float, text_runs: Sequence[dict[str, Any]],
                       field_cells: Sequence[dict[str, Any]]) -> tuple[bool, int, int]:
    """The test that actually decides. Returns (valid, runs below, fields below)."""
    runs_below = len(below(text_runs, cut_y))
    fields_below = len(below(field_cells, cut_y))
    valid = (runs_below >= MIN_GUIDE_TEXT_RUNS
             and fields_below <= field_cell_allowance(len(field_cells)))
    return valid, runs_below, fields_below


def collect_straddlers(cut_y: float, ir_page: dict[str, Any],
                       layout_page: dict[str, Any]) -> list[Straddler]:
    out: list[Straddler] = []
    for rule in straddling(ir_page["rules"], cut_y):
        out.append(Straddler("rule", rule["id"], rule["x0"], rule["y0"], rule["x1"], rule["y1"],
                             f"{rule['axis']} {rule['thickness_pt']}pt gray={rule['gray']}"))
    for index, fill in enumerate(ir_page["area_fills"]):
        if fill["y0"] < cut_y < fill["y1"]:
            out.append(Straddler("area_fill", f"#{index}", fill["x0"], fill["y0"],
                                 fill["x1"], fill["y1"], f"gray={fill['gray']}"))
    for index, image in enumerate(ir_page["images"]):
        if image["y0"] < cut_y < image["y1"]:
            out.append(Straddler("image", f"#{index}", image["x0"], image["y0"],
                                 image["x1"], image["y1"], image["sha256"][:12]))
    for index, run in enumerate(ir_page["text_runs"]):
        if run["y0"] < cut_y < run["y1"]:
            out.append(Straddler("text_run", f"#{index}", run["x0"], run["y0"],
                                 run["x1"], run["y1"], repr(run["text"][:40])))

    # A straddling cell is the worst case: it stays with the form, but the runs
    # it owns may sit below the cut and therefore leave with the guide. emit.py
    # has to render that cell without them, so the count is reported here rather
    # than left to be discovered at emit time.
    run_top = {f"p{ir_page['index']}t{i}": r["y0"]
               for i, r in enumerate(ir_page["text_runs"])}
    for cell in straddling(layout_page["cells"], cut_y):
        orphaned = sum(1 for t in cell["text_run_ids"] if run_top.get(t, 0.0) >= cut_y)
        detail = f"kind={cell['kind']}"
        if orphaned:
            detail += f" loses {orphaned}/{len(cell['text_run_ids'])} runs to the guide"
        out.append(Straddler("cell", cell["id"], cell["x0"], cell["y0"],
                             cell["x1"], cell["y1"], detail))
    return out


def detect_page(ir_page: dict[str, Any], layout_page: dict[str, Any]) -> dict[str, Any] | None:
    """Find the guide region on one page, or None.

    Topmost valid candidate wins: a page with both a table heading and a
    guidelines heading is one guide region starting at the first of them.
    """
    text_runs = ir_page["text_runs"]
    field_cells = [c for c in layout_page["cells"] if c["kind"] == "field"]

    chosen: Candidate | None = None
    runs_below = fields_below = 0
    for candidate in page_candidates(text_runs):
        valid, runs_below, fields_below = structurally_valid(
            candidate.cut_y, text_runs, field_cells)
        if valid:
            chosen = candidate
            break
    if chosen is None:
        return None

    cut_y = chosen.cut_y
    height = ir_page["height_pt"]
    reclaimed = height - cut_y

    guide_runs = [i for i, r in enumerate(text_runs) if r["y0"] >= cut_y]
    guide_cells = [c["id"] for c in layout_page["cells"] if c["y0"] >= cut_y]
    guide_rules = [r["id"] for r in ir_page["rules"] if r["y0"] >= cut_y]
    guide_fills = [i for i, f in enumerate(ir_page["area_fills"]) if f["y0"] >= cut_y]
    guide_images = [i for i, m in enumerate(ir_page["images"]) if m["y0"] >= cut_y]
    straddlers = collect_straddlers(cut_y, ir_page, layout_page)

    return {
        "page": ir_page["index"],
        "cut_y_pt": round(cut_y, 2),
        "reclaimed_pt": round(reclaimed, 2),
        "reclaimed_pct": round(reclaimed / height * 100),
        "text_run_indices": guide_runs,
        "cell_ids": guide_cells,
        "rule_ids": guide_rules,
        "area_fill_indices": guide_fills,
        "image_indices": guide_images,
        "marker": chosen.marker,
        "marker_pattern": chosen.pattern,
        "text_runs_below": runs_below,
        "field_cells_below": fields_below,
        "field_cells_on_page": len(field_cells),
        "field_cell_allowance": round(field_cell_allowance(len(field_cells)), 2),
        "straddlers": [s.as_dict() for s in straddlers],
    }


# --------------------------------------------------------------------------
# standalone guide PDFs
# --------------------------------------------------------------------------

def is_guide_name(pdf: pathlib.Path) -> bool:
    name = pdf.name.lower()
    return any(marker in name for marker in GUIDE_NAME_MARKERS)


def parent_form_identity(pdf: pathlib.Path) -> tuple[str, str] | None:
    """Map any source PDF to (code, revision) from its folder and file name.

    This is batch.classify()'s identity logic with the non-form skip removed --
    a guide's *folder* is its parent form's folder, so the same derivation names
    the form the guide belongs to. self_test() asserts the two stay in step.
    """
    match = re.match(r"^(?P<code>\d{4}[A-Za-z-]*?)(?:v(?P<rev>\d{4}[A-Za-z]?))?$", pdf.parent.name)
    if not match:
        return None
    code = match.group("code").upper().rstrip("-")
    revision = (match.group("rev") or "").upper()
    if not revision:
        revision = REVISION_OVERRIDES.get(code, "")
    if not revision:
        year = re.search(r"(?:19|20)\d{2}", pdf.name)
        revision = year.group(0) if year else "undated"
    return code, revision


def form_key(code: str, revision: str) -> str:
    return f"{code}-{revision}".upper()


def standalone_guide_pdfs(source_root: pathlib.Path = DEFAULT_SOURCE_ROOT
                          ) -> dict[str, list[pathlib.Path]]:
    """Every PDF batch.py skips as a non-form, keyed by its parent form.

    A folder can hold more than one (2200AN ships an Annex A alongside its
    guidelines), so the value is a sorted list, not a single path. Sorted by
    path throughout, because the caller pins on this and determinism is the
    property being protected.
    """
    mapping: dict[str, list[pathlib.Path]] = {}
    if not source_root.is_dir():
        return mapping
    for pdf in sorted(source_root.rglob("*.pdf")):
        if not is_guide_name(pdf):
            continue
        identity = parent_form_identity(pdf)
        if identity is None:
            continue
        mapping.setdefault(form_key(*identity), []).append(pdf.resolve())
    return {key: sorted(paths) for key, paths in sorted(mapping.items())}


# --------------------------------------------------------------------------
# plan
# --------------------------------------------------------------------------

def build_plan(ir: dict[str, Any], layout: dict[str, Any],
               standalone: dict[str, list[pathlib.Path]] | None = None) -> dict[str, Any]:
    if ir["form"] != layout["form"]:
        raise ValueError(f"IR is {ir['form']} but layout is {layout['form']}")
    if len(ir["pages"]) != len(layout["pages"]):
        raise ValueError("IR and layout disagree on page count")

    inline = [entry for entry in
              (detect_page(ip, lp) for ip, lp in zip(ir["pages"], layout["pages"]))
              if entry is not None]

    key = form_key(ir["form"]["code"], ir["form"]["revision"])
    guides = (standalone or {}).get(key, [])
    pcts = [e["reclaimed_pct"] for e in inline]

    return {
        "schema_version": SCHEMA_VERSION,
        "form": dict(ir["form"]),
        "inline": inline,
        "standalone_pdf": str(guides[0]) if guides else None,
        "standalone_pdfs": [str(p) for p in guides],
        "stats": {
            "pages": len(ir["pages"]),
            "pages_with_guide": len(inline),
            "reclaimed_pct_mean": round(sum(pcts) / len(pcts)) if pcts else 0,
            "reclaimed_pct_min": min(pcts) if pcts else 0,
            "reclaimed_pct_max": max(pcts) if pcts else 0,
            "guide_text_runs": sum(len(e["text_run_indices"]) for e in inline),
            "guide_cells": sum(len(e["cell_ids"]) for e in inline),
            "guide_rules": sum(len(e["rule_ids"]) for e in inline),
            "guide_area_fills": sum(len(e["area_fill_indices"]) for e in inline),
            "guide_images": sum(len(e["image_indices"]) for e in inline),
            "straddlers": sum(len(e["straddlers"]) for e in inline),
        },
    }


def check_partition(plan: dict[str, Any], ir: dict[str, Any],
                    layout: dict[str, Any]) -> list[str]:
    """Prove the split is exhaustive and disjoint on every page it touches.

    emit.py will ask "does this element go in the form or the guide?" exactly
    once per element. If the answer is ever "both" or "neither", the sheet
    silently loses or duplicates geometry, so the invariant is checked here
    rather than trusted.
    """
    problems: list[str] = []
    by_page = {e["page"]: e for e in plan["inline"]}
    for ir_page, layout_page in zip(ir["pages"], layout["pages"]):
        entry = by_page.get(ir_page["index"])
        if entry is None:
            continue
        cut_y = entry["cut_y_pt"]
        buckets = (
            ("text_run", [r["y0"] for r in ir_page["text_runs"]], entry["text_run_indices"],
             list(range(len(ir_page["text_runs"])))),
            ("area_fill", [f["y0"] for f in ir_page["area_fills"]], entry["area_fill_indices"],
             list(range(len(ir_page["area_fills"])))),
            ("image", [m["y0"] for m in ir_page["images"]], entry["image_indices"],
             list(range(len(ir_page["images"])))),
            ("rule", [r["y0"] for r in ir_page["rules"]], entry["rule_ids"],
             [r["id"] for r in ir_page["rules"]]),
            ("cell", [c["y0"] for c in layout_page["cells"]], entry["cell_ids"],
             [c["id"] for c in layout_page["cells"]]),
        )
        for kind, tops, guide_side, all_refs in buckets:
            guide_set = set(guide_side)
            if len(guide_set) != len(guide_side):
                problems.append(f"page {ir_page['index']} {kind}: duplicate entries")
            unknown = guide_set - set(all_refs)
            if unknown:
                problems.append(f"page {ir_page['index']} {kind}: unknown refs {sorted(unknown)[:3]}")
            for ref, top in zip(all_refs, tops):
                # cut_y_pt is rounded for the report; compare with the same slack.
                should_be_guide = top >= cut_y - 0.005
                if should_be_guide != (ref in guide_set):
                    problems.append(
                        f"page {ir_page['index']} {kind} {ref}: top {top} vs cut {cut_y} "
                        f"but {'in' if ref in guide_set else 'not in'} the guide")
    return problems


# --------------------------------------------------------------------------
# corpus sweep
# --------------------------------------------------------------------------

def load_pair(ir_dir: pathlib.Path, layout_dir: pathlib.Path,
              slug: str) -> tuple[dict[str, Any], dict[str, Any]]:
    ir = json.loads((ir_dir / f"{slug}.ir.json").read_text(encoding="utf-8"))
    layout = json.loads((layout_dir / f"{slug}.layout.json").read_text(encoding="utf-8"))
    return ir, layout


def sweep(ir_dir: pathlib.Path, layout_dir: pathlib.Path,
          source_root: pathlib.Path) -> list[tuple[str, dict[str, Any]]]:
    standalone = standalone_guide_pdfs(source_root)
    out: list[tuple[str, dict[str, Any]]] = []
    for path in sorted(ir_dir.glob("*.ir.json")):
        slug = path.name[: -len(".ir.json")]
        if not (layout_dir / f"{slug}.layout.json").is_file():
            continue
        ir, layout = load_pair(ir_dir, layout_dir, slug)
        out.append((slug, build_plan(ir, layout, standalone)))
    return out


def print_corpus_table(rows: Sequence[tuple[str, dict[str, Any]]],
                       standalone: dict[str, list[pathlib.Path]],
                       stream: Any = sys.stdout) -> None:
    def w(line: str = "") -> None:
        print(line, file=stream)

    w(f"{'form':<26} {'pg':>3} {'guide page':>10} {'cut pt':>8} {'reclaim':>8} "
      f"{'%':>4} {'runs':>5} {'fld':>4} {'std':>4}  marker")
    w("-" * 118)
    with_inline = with_standalone = neither = 0
    pcts: list[int] = []
    for slug, plan in rows:
        has_std = bool(plan["standalone_pdf"])
        if not plan["inline"]:
            if has_std:
                with_standalone += 1
            else:
                neither += 1
            w(f"{slug:<26} {plan['stats']['pages']:>3} {'-':>10} {'-':>8} {'-':>8} "
              f"{'-':>4} {'-':>5} {'-':>4} {'yes' if has_std else '-':>4}")
            continue
        with_inline += 1
        if has_std:
            with_standalone += 1
        for n, entry in enumerate(plan["inline"]):
            pcts.append(entry["reclaimed_pct"])
            w(f"{slug if n == 0 else '':<26} {plan['stats']['pages'] if n == 0 else '':>3} "
              f"{entry['page']:>10} {entry['cut_y_pt']:>8.2f} {entry['reclaimed_pt']:>8.2f} "
              f"{entry['reclaimed_pct']:>4} {entry['text_runs_below']:>5} "
              f"{entry['field_cells_below']:>4} "
              f"{('yes' if has_std else '-') if n == 0 else '':>4}  {entry['marker'][:48]}")
    w("-" * 118)
    pages = sum(p["stats"]["pages"] for _, p in rows)
    w(f"{len(rows)} forms, {pages} pages: {len(pcts)} guide pages on {with_inline} forms; "
      f"{with_standalone} forms have a standalone guide PDF; {neither} have neither.")
    if pcts:
        w(f"reclaimed: mean {round(sum(pcts) / len(pcts))}%  min {min(pcts)}%  max {max(pcts)}%")

    w()
    w("straddling elements (form wins every one):")
    total = 0
    for slug, plan in rows:
        for entry in plan["inline"]:
            for s in entry["straddlers"]:
                total += 1
                w(f"  {slug:<24} p{entry['page']} {s['kind']:<9} {s['ref']:<7} "
                  f"y {s['y0']:.2f}->{s['y1']:.2f}  {s['detail']}")
    w(f"  {total} straddler(s)")

    w()
    w("standalone guide PDFs:")
    built = {form_key(p["form"]["code"], p["form"]["revision"]) for _, p in rows}
    for key, paths in standalone.items():
        for path in paths:
            mark = "" if key in built else "   (no built form for this key)"
            w(f"  {key:<16} {path}{mark}")


# --------------------------------------------------------------------------
# self-test
# --------------------------------------------------------------------------

def self_test(ir_dir: pathlib.Path, layout_dir: pathlib.Path,
              source_root: pathlib.Path) -> int:
    """Assert against the real corpus, not a synthetic fixture."""
    failures: list[str] = []

    def check(condition: bool, message: str) -> None:
        if not condition:
            failures.append(message)

    # The loose pattern that was rejected must stay rejected: these four strings
    # are form content and no marker may fire on them.
    for text in ("Add: Penalties", "Penalties", "Surcharge, Interest and Compromise Penalty",
                 "Total Penalties", "see Table 2 below", "Table of contents"):
        check(marker_hit(text) is None, f"a marker fired on form content {text!r}")
    for text in ("Guidelines and Instructions for BIR Form No. 1603Q",
                 "Table 1 – Alphanumeric Tax Code (ATC)",
                 "ALPHANUMERIC TAX CODES (ATC)"):
        check(marker_hit(text) is not None, f"no marker fired on guide heading {text!r}")

    rows = sweep(ir_dir, layout_dir, source_root)
    check(len(rows) == 51, f"expected 51 forms in the corpus, got {len(rows)}")
    plans = dict(rows)

    pages = sum(p["stats"]["pages"] for _, p in rows)
    check(pages == 110, f"expected 110 pages in the corpus, got {pages}")

    guide_pages = [(slug, e) for slug, p in rows for e in p["inline"]]
    forms_with = [slug for slug, p in rows if p["inline"]]
    check(len(guide_pages) == 17, f"expected 17 guide pages, got {len(guide_pages)}")
    check(len(forms_with) == 17, f"expected 17 forms with a guide, got {len(forms_with)}")

    pcts = [e["reclaimed_pct"] for _, e in guide_pages]
    check(round(sum(pcts) / len(pcts)) == 67, f"expected mean reclaim 67%, got {pcts}")
    check(min(pcts) == 38, f"expected min reclaim 38%, got {min(pcts)}")
    check(max(pcts) == 93, f"expected max reclaim 93%, got {max(pcts)}")

    # The four cuts measured by the validated prototype, to the hundredth.
    expected = {
        ("1603q-2018", 2): (284.54, 0, 171, 70),
        ("1600-pt-2018", 2): (286.02, 4, 126, 69),
        ("2551q-2018", 2): (295.51, 3, 93, 68),
        ("2550m-undated", 3): (72.52, 4, 124, 93),
    }
    for (slug, page), (cut, fields, runs, pct) in expected.items():
        entry = next((e for e in plans[slug]["inline"] if e["page"] == page), None)
        if entry is None:
            failures.append(f"{slug} p{page}: no guide detected")
            continue
        check(entry["cut_y_pt"] == cut, f"{slug} p{page}: cut {entry['cut_y_pt']} != {cut}")
        check(entry["field_cells_below"] == fields,
              f"{slug} p{page}: {entry['field_cells_below']} fields below, expected {fields}")
        check(entry["text_runs_below"] == runs,
              f"{slug} p{page}: {entry['text_runs_below']} runs below, expected {runs}")
        check(entry["reclaimed_pct"] == pct,
              f"{slug} p{page}: reclaimed {entry['reclaimed_pct']}%, expected {pct}%")

    # The false positives the structural test exists to reject. Every one of
    # these pages carries an ATC or rate-table heading over fillable form.
    for slug, page in (("1701-2018", 1), ("1701q-2018", 1), ("1702ex-2018", 1),
                       ("1700-2018", 2), ("1601eq-2019", 2), ("2552-2018", 2),
                       ("1701ms-2024", 2)):
        entry = next((e for e in plans[slug]["inline"] if e["page"] == page), None)
        check(entry is None, f"{slug} p{page}: cut a guide out of fillable form")

    # Exhaustive and disjoint, on every form, not just the interesting ones.
    for slug, plan in rows:
        ir, layout = load_pair(ir_dir, layout_dir, slug)
        for problem in check_partition(plan, ir, layout):
            failures.append(f"{slug}: {problem}")

    # No text run may straddle: a split glyph run cannot be rendered twice.
    for slug, entry in guide_pages:
        split_runs = [s for s in entry["straddlers"] if s["kind"] == "text_run"]
        check(not split_runs, f"{slug} p{entry['page']}: text run straddles the cut {split_runs}")

    # Determinism: the plan is a pure function of its inputs.
    ir, layout = load_pair(ir_dir, layout_dir, "1603q-2018")
    standalone = standalone_guide_pdfs(source_root)
    first = json.dumps(build_plan(ir, layout, standalone), sort_keys=True)
    second = json.dumps(build_plan(ir, layout, standalone), sort_keys=True)
    check(first == second, "build_plan is not deterministic")

    # The standalone mapping, and the claim that it uses batch.py's logic.
    check(len(standalone) >= 1 or not source_root.is_dir(),
          f"no standalone guide PDFs found under {source_root}")
    if source_root.is_dir():
        for key in ("1601EQ-2019", "1701Q-2018", "2550Q-2024", "2552-2018", "1600WP-2010"):
            check(key in standalone, f"standalone guide for {key} not found")
        check(len(standalone.get("2200AN-2018", [])) == 2,
              "2200AN should map to two non-form PDFs (guidelines + Annex A)")
        failures.extend(batch_agreement_failures(source_root))

    for message in failures:
        print(f"FAIL {message}", file=sys.stderr)
    print(f"guides self-test: {len(failures)} failure(s)", file=sys.stderr)
    return 1 if failures else 0


def batch_agreement_failures(source_root: pathlib.Path) -> list[str]:
    """parent_form_identity() must agree with batch.classify() where both speak.

    The two derivations are written out separately -- batch.py is owned
    elsewhere and importing its classify() would not work here anyway, since it
    returns None for exactly the files this module is about. Asserting the
    agreement is what keeps them from drifting apart silently.
    """
    sys.path.insert(0, str(HERE))
    try:
        import batch  # noqa: PLC0415 - optional, and only for this cross-check
    except Exception as error:  # noqa: BLE001 - a broken batch.py is not our failure
        return [f"(skipped batch.py cross-check: {error})"]
    finally:
        sys.path.pop(0)

    out: list[str] = []
    if getattr(batch, "REVISION_OVERRIDES", None) != REVISION_OVERRIDES:
        out.append(f"REVISION_OVERRIDES drifted from batch.py: {batch.REVISION_OVERRIDES}")
    if tuple(getattr(batch, "NON_FORM_MARKERS", ())) != GUIDE_NAME_MARKERS:
        out.append(f"NON_FORM_MARKERS drifted from batch.py: {batch.NON_FORM_MARKERS}")
    for pdf in sorted(source_root.rglob("*.pdf")):
        source = batch.classify(pdf)
        if source is None:
            continue
        mine = parent_form_identity(pdf)
        if mine != (source.code, source.revision):
            out.append(f"identity drift on {pdf.name}: {mine} vs batch {(source.code, source.revision)}")
    return out


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def print_summary(slug: str, plan: dict[str, Any], stream: Any = sys.stderr) -> None:
    stats = plan["stats"]
    print(f"{slug}: {stats['pages']} page(s), {stats['pages_with_guide']} with a guide region",
          file=stream)
    for entry in plan["inline"]:
        print(f"  page {entry['page']}: cut y={entry['cut_y_pt']}pt  "
              f"reclaims {entry['reclaimed_pt']}pt ({entry['reclaimed_pct']}%)  "
              f"{entry['text_runs_below']} runs / {entry['field_cells_below']} field cells below "
              f"(allowance {entry['field_cell_allowance']})", file=stream)
        print(f"           marker [{entry['marker_pattern']}] {entry['marker']!r}", file=stream)
        print(f"           guide takes {len(entry['rule_ids'])} rules, "
              f"{len(entry['area_fill_indices'])} fills, {len(entry['image_indices'])} images, "
              f"{len(entry['cell_ids'])} cells, {len(entry['text_run_indices'])} runs",
              file=stream)
        for s in entry["straddlers"]:
            print(f"           STRADDLES {s['kind']} {s['ref']} "
                  f"y {s['y0']:.2f}->{s['y1']:.2f} ({s['detail']}) -- kept by the form",
                  file=stream)
    print(f"  standalone guide PDF: {plan['standalone_pdf'] or 'none'}", file=stream)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--ir", type=pathlib.Path, help="one form's .ir.json")
    parser.add_argument("--layout", type=pathlib.Path, help="the matching .layout.json")
    parser.add_argument("--out", type=pathlib.Path, help="write the guide plan here")
    parser.add_argument("--summary", action="store_true")
    parser.add_argument("--corpus", action="store_true",
                        help="sweep every build/ir/*.ir.json and print the detection table")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--ir-dir", type=pathlib.Path, default=pathlib.Path("build/ir"))
    parser.add_argument("--layout-dir", type=pathlib.Path, default=pathlib.Path("build/layout"))
    parser.add_argument("--out-dir", type=pathlib.Path,
                        help="--corpus: write one guide plan per form here")
    parser.add_argument("--source-root", type=pathlib.Path, default=DEFAULT_SOURCE_ROOT)
    args = parser.parse_args()

    if args.self_test:
        return self_test(args.ir_dir, args.layout_dir, args.source_root)

    if args.corpus:
        rows = sweep(args.ir_dir, args.layout_dir, args.source_root)
        print_corpus_table(rows, standalone_guide_pdfs(args.source_root))
        if args.out_dir:
            args.out_dir.mkdir(parents=True, exist_ok=True)
            for slug, plan in rows:
                (args.out_dir / f"{slug}.guide.json").write_text(
                    json.dumps(plan, indent=2) + "\n", encoding="utf-8")
        return 0

    if not (args.ir and args.layout):
        parser.error("--ir and --layout are required unless --corpus or --self-test")

    ir = json.loads(args.ir.read_text(encoding="utf-8"))
    layout = json.loads(args.layout.read_text(encoding="utf-8"))
    plan = build_plan(ir, layout, standalone_guide_pdfs(args.source_root))

    problems = check_partition(plan, ir, layout)
    if problems:
        for problem in problems:
            print(f"PARTITION {problem}", file=sys.stderr)
        return 1

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(plan, indent=2) + "\n", encoding="utf-8")
    if args.summary or not args.out:
        print_summary(args.ir.name[: -len(".ir.json")], plan)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
