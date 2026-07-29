#!/usr/bin/env python3
"""Run the whole formgen pipeline over every source PDF and package the result.

This is the one-shot scaffolding driver. It converts each pinned BIR PDF into a
hand-maintainable HTML/CSS bundle under forms/, then gets archived: from that
point the HTML is the artefact people edit, and this script exists only as the
record of how it was derived.

Because the HTML becomes hand-maintained, three things matter more than they
would in a normal build:

  * Every bundle carries provenance.json (source files, SHA-256, page geometry,
    generator version). Re-running the generator over an edited bundle would
    destroy those edits, so the provenance is what lets anyone tell where a
    file came from without re-deriving it.
  * The CSS is split. Declarations common to every form land in the shared
    forms/base.css; only what is genuinely form-specific lands in the form's
    own form.css, and the guide document's own rules land in guide.css.
    A fix to the page scaffolding is then one edit, not 48.
  * Fonts live once at forms/fonts/, not once per form.

Two documents per bundle
------------------------

A BIR sheet carries reference material -- ATC tables, "Guidelines and
Instructions" -- that nobody fills in, either printed below the form on the same
page or shipped as a separate PDF. guides.py decides where the cut falls; this
driver runs it between lattice and emit and then emits the bundle twice:

    index.html   the form, with any inline guide region removed
    guide.html   that region, plus any standalone guide PDF for the same form

A bundle with no guide content gets no guide.html, and nothing links to one.
Standalone guide PDFs stay out of form discovery -- converting an instruction
sheet into a fillable-looking bundle is worse than skipping it -- but they are
not discarded either: guides.py maps each one to its parent form, this driver
copies it into that bundle's guides/ directory and extracts it with the same
extractor the form uses, and emit.py reflows that extraction into guide.html.
It is not a second *form* conversion -- no lattice, no font plan, no parity
score -- it is the guide's text, in the guide document, because that is the
only version of it that prints. The pinned PDF stays beside the HTML and is
linked, and remains the exact artefact.

Usage:
    python3 tools/formgen/batch.py --source-root ~/Downloads/forms --out forms
    python3 tools/formgen/batch.py --only 2551Q --dry-run
"""

from __future__ import annotations

import argparse
import collections
import dataclasses
import functools
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
from typing import Any, Iterable

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent

sys.path.insert(0, str(HERE))
import guides  # noqa: E402 - a sibling stage; used for its schema and its partition proof

# The 35-form conversion corpus. Anything outside this list is still converted,
# but lands under extra/ so the maintained set stays the committed scope.
CORPUS = {
    "0605", "0619E", "0619F", "1601C", "1701", "1701Q", "1702MX", "1702RT",
    "2550Q", "2551Q",
    "0620", "2551M", "1603Q", "1606", "2550-DS",
    "1600-PT", "1600-VT", "1601-FQ", "1601EQ", "1602Q", "2550M",
    "1604C", "1604F", "2316", "2000-DST", "2000-OT", "1621",
    "1701A", "1701MS", "1702EX", "1702Q", "1707A", "1709", "2200C", "2200M",
}

# Guides, instruction sheets and annexes are not fileable forms. Converting one
# produces a plausible-looking bundle for a document nobody files, which is
# worse than skipping it. They are not discarded: guides.py maps each one back
# to its parent form and this driver renders it into that form's guide.html.
NON_FORM_MARKERS = ("guide", "guidelines", "instruction", "annex")

# Revisions that the folder name does not carry and the sheet does not stamp in
# extractable text. Only entries confirmed from the document body belong here --
# an invented revision is worse than an honest "undated", because revision is
# part of a form's identity and everything downstream pins on it.
REVISION_OVERRIDES = {
    "0605": "1999",     # printed in the sheet body; matches the repo's existing pin
}

STAGES = ("extract", "lattice", "guides", "fonts", "emit")

# What a bundle is called from the inside. These are passed to emit.py rather
# than left to match its defaults: the two documents link to each other and the
# guide embeds PDFs by relative path, so the names have to be one fact, not two
# that happen to agree.
FORM_DOCUMENT = "index.html"
GUIDE_DOCUMENT = "guide.html"
GUIDE_PDF_DIR = "guides"

# The flags emit.py needs before this driver can split anything.
GUIDE_EMIT_FLAGS = frozenset({"--guide-plan", "--document"})

# The flag that turns a standalone guide PDF into printable text rather than an
# embedded viewer. Optional in the same way the naming flags are: a checkout
# whose emit.py predates it still builds every bundle, with the guide PDFs
# linked instead of converted.
GUIDE_SOURCE_FLAG = "--guide-source"


@dataclasses.dataclass
class Source:
    pdf: pathlib.Path
    code: str
    revision: str
    variant: str            # "" for the main form, else "attachment" / "conso"
    in_corpus: bool

    @property
    def slug(self) -> str:
        base = f"{self.code}-{self.revision}"
        return f"{base}-{self.variant}" if self.variant else base

    @property
    def key(self) -> str:
        return self.slug.lower()


def classify(pdf: pathlib.Path) -> Source | None:
    """Derive form identity from the folder and file name, or skip."""
    name = pdf.name.lower()
    if any(marker in name for marker in NON_FORM_MARKERS):
        return None

    folder = pdf.parent.name
    match = re.match(r"^(?P<code>\d{4}[A-Za-z-]*?)(?:v(?P<rev>\d{4}[A-Za-z]?))?$", folder)
    if not match:
        return None
    code = match.group("code").upper().rstrip("-")
    revision = (match.group("rev") or "").upper()

    if not revision:
        revision = REVISION_OVERRIDES.get(code, "")
    if not revision:
        # Folders like "1601-FQ" omit the revision; take it from the file name's
        # year. No leading word boundary: "0605version1999" has none.
        year = re.search(r"(?:19|20)\d{2}", pdf.name)
        revision = year.group(0) if year else "undated"

    variant = ""
    if "attachment" in name:
        variant = "attachment"
    elif "conso" in name:
        variant = "conso"

    return Source(pdf=pdf, code=code, revision=revision, variant=variant,
                  in_corpus=code in CORPUS)


def discover(root: pathlib.Path) -> list[Source]:
    seen: dict[str, Source] = {}
    skipped: list[pathlib.Path] = []
    for pdf in sorted(root.rglob("*.pdf")):
        source = classify(pdf)
        if source is None:
            skipped.append(pdf)
            continue
        # A duplicated folder (1600PT vs 1600-PTv2018) can offer the same form
        # twice; keep the one whose folder carried an explicit revision.
        prior = seen.get(source.key)
        if prior is None or (len(pdf.parent.name) > len(prior.pdf.parent.name)):
            seen[source.key] = source
    return sorted(seen.values(), key=lambda s: (not s.in_corpus, s.code, s.revision, s.variant))


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def extract_images(pdf: pathlib.Path, assets: pathlib.Path) -> int:
    """Write every embedded XObject to <sha256>.<ext> beside the bundles.

    emit.py resolves images by content hash, so this has to run before it or the
    form gets placeholders. Naming by hash also means the seal shared by several
    forms is stored once and cannot be silently swapped for different bytes.
    """
    import fitz  # local import: only the driver needs it

    assets.mkdir(parents=True, exist_ok=True)
    written = 0
    doc = fitz.open(pdf)
    for page in doc:
        for info in page.get_images(full=True):
            try:
                payload = doc.extract_image(info[0])
            except Exception:  # noqa: BLE001 - a broken XObject must not stop the form
                continue
            data = payload["image"]
            target = assets / f"{hashlib.sha256(data).hexdigest()}.{payload.get('ext', 'png')}"
            if not target.exists():
                target.write_bytes(data)
                written += 1
    return written


def run_stage(args: list[str]) -> tuple[bool, str]:
    result = subprocess.run([sys.executable, *args], capture_output=True, text=True, cwd=REPO)
    if result.returncode != 0:
        tail = (result.stderr or result.stdout).strip().splitlines()
        return False, "\n".join(tail[-4:]) if tail else f"exit {result.returncode}"
    return True, ""


@functools.lru_cache(maxsize=1)
def emit_options() -> frozenset[str]:
    """The long options emit.py actually accepts, read from its own --help.

    The emitter is developed alongside this driver, so a checkout can have the
    guide plan without the emitter that consumes it. Asking emit.py what it
    supports -- once -- means such a checkout still builds all 51 forms exactly
    as it does today instead of failing 51 times on an unknown flag, and the run
    says plainly that nothing was split.
    """
    result = subprocess.run([sys.executable, str(HERE / "emit.py"), "--help"],
                            capture_output=True, text=True, cwd=REPO)
    return frozenset(re.findall(r"--[a-z][a-z0-9-]+", result.stdout))


def guide_emission_supported() -> bool:
    return GUIDE_EMIT_FLAGS <= emit_options()


def with_supported(argv: list[str], flags: Iterable[tuple[str, str]]) -> list[str]:
    """Append the flags emit.py accepts, and quietly drop the ones it does not.

    Only the naming flags go through here. They are refinements -- emit.py's own
    defaults already spell them the same way -- so a checkout without them still
    emits a correct bundle, whereas a missing --guide-plan means no split at all
    and is reported instead of ignored.
    """
    supported = emit_options()
    for flag, value in flags:
        if flag in supported:
            argv += [flag, value]
    return argv


def stage_fonts(plan_path: pathlib.Path, fonts_dir: pathlib.Path) -> None:
    """Put every WOFF2 the plan resolved beside the HTML the emitter is about to write.

    Done here, between the fonts and emit stages, because emit.py checks that the
    files its @font-face rules point at are actually there and the check is only
    worth anything if nothing had to be staged by hand. A newly mapped family
    would otherwise render from a platform fallback while the plan went on
    reporting advances for the font we meant to ship.
    """
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    root = pathlib.Path(plan["generator"]["fonts_root"])
    fonts_dir.mkdir(parents=True, exist_ok=True)
    for face in plan["faces"]:
        relative = face.get("font_file")
        if not relative:
            continue
        source = root / relative
        target = fonts_dir / pathlib.PurePosixPath(relative).name
        if source.is_file() and not target.is_file():
            shutil.copy2(source, target)


# ---------------------------------------------------------------------------
# guide documents
# ---------------------------------------------------------------------------


def stage_guide_pdfs(pdfs: Iterable[pathlib.Path], staging: pathlib.Path) -> None:
    """Put the standalone guide PDFs where the emitted guide document expects them.

    emit.py links each one as guides/<name> and checks the file is actually
    beside its output, so they have to be staged before the emit rather than at
    packaging time -- otherwise every guide document is emitted with a warning
    about a file this driver was always going to copy.
    """
    staging.mkdir(parents=True, exist_ok=True)
    for pdf in pdfs:
        target = staging / pdf.name
        if not target.is_file():
            shutil.copy2(pdf, target)


def stage_guide_sources(pdfs: Iterable[pathlib.Path], slug: str, code: str,
                        revision: str, work: pathlib.Path
                        ) -> tuple[list[pathlib.Path], str]:
    """Extract each standalone guide PDF with the extractor the form itself uses.

    A guide shipped as its own PDF used to be handed to the browser as a linked
    <object>, which does not print with the document around it: twelve bundles
    had a guide that could be read on screen and not put on paper. Running the
    guide PDF through extract.py makes its text available to the same reflow the
    inline guides go through, so all 29 guides become one kind of document.

    These IRs are a *reading* artefact and deliberately go no further down the
    pipeline -- no lattice, no font plan, no round-trip score. A guide has no
    fields to model and no parity to measure; what it has is text, and text is
    all the reflow reads. They land in a subdirectory so audit.py's `*.ir.json`
    sweep, which scores forms, cannot mistake one for a form.

    Returns the IR paths, or an empty list and the first error.
    """
    out: list[pathlib.Path] = []
    for index, pdf in enumerate(pdfs, 1):
        target = work / "ir" / "guides" / f"{slug}.guide-{index}.ir.json"
        target.parent.mkdir(parents=True, exist_ok=True)
        ok, error = run_stage([str(HERE / "extract.py"), "--pdf", str(pdf),
                               "--form-code", code, "--revision", revision,
                               "--expected-sha256", sha256_file(pdf),
                               "--out", str(target)])
        if not ok:
            return [], f"{pdf.name}: {error}"
        out.append(target)
    return out, ""


def region_summary(entry: dict[str, Any], destination: str) -> dict[str, Any]:
    """What one inline region took off the form page, for provenance.json."""
    return {
        "page": entry["page"],
        "cut_y_pt": entry["cut_y_pt"],
        "reclaimed_pt": entry["reclaimed_pt"],
        "reclaimed_pct": entry["reclaimed_pct"],
        "marker": entry["marker"],
        "marker_pattern": entry["marker_pattern"],
        "moved": {
            "rules": len(entry["rule_ids"]),
            "cells": len(entry["cell_ids"]),
            "text_runs": len(entry["text_run_indices"]),
            "area_fills": len(entry["area_fill_indices"]),
            "images": len(entry["image_indices"]),
        },
        "straddlers_kept_by_form": len(entry["straddlers"]),
        "moved_to": destination,
    }


def guide_block(plan: dict[str, Any], pdfs: list[pathlib.Path],
                converted: bool) -> dict[str, Any]:
    """The bundle's guide record: what left the form pages, and where it went."""
    regions = [region_summary(entry, GUIDE_DOCUMENT) for entry in plan["inline"]]
    pcts = [r["reclaimed_pct"] for r in regions]
    origins = (["inline-region"] if regions else []) + (["standalone-pdf"] if pdfs else [])
    return {
        "document": GUIDE_DOCUMENT,
        "origins": origins,
        "moved_from_form": regions,
        "standalone_pdfs": [{"file": pdf.name, "sha256": sha256_file(pdf),
                             "linked_as": f"{GUIDE_PDF_DIR}/{pdf.name}",
                             "reflowed_into": GUIDE_DOCUMENT if converted else None}
                            for pdf in pdfs],
        "reclaimed_pt_total": round(sum(r["reclaimed_pt"] for r in regions), 2),
        "reclaimed_pct_mean": round(sum(pcts) / len(pcts)) if pcts else 0,
        "reclaimed_pct_min": min(pcts) if pcts else 0,
        "reclaimed_pct_max": max(pcts) if pcts else 0,
        "note": ("Reference material printed on the source sheet, or shipped as a separate "
                 "guide PDF. It is not part of the filed form: index.html no longer carries "
                 "the regions listed under moved_from_form. Each PDF under standalone_pdfs "
                 "was re-extracted and reflowed into guide.html so that it prints with the "
                 "document; the pinned PDF is kept beside it and linked, and remains the "
                 "exact artefact."),
    }


def convert(source: Source, work: pathlib.Path, backend: str, fonts_dir: pathlib.Path,
            source_root: pathlib.Path, guide_layout: str | None) -> dict[str, Any]:
    """Run the pipeline for one form. Returns a record, never raises."""
    slug = source.key
    ir = work / "ir" / f"{slug}.ir.json"
    layout = work / "layout" / f"{slug}.layout.json"
    plan = work / "fonts" / f"{slug}.fontplan.json"
    guide_plan = work / "guides" / f"{slug}.guide.json"
    html = work / "html" / f"{slug}.html"
    for path in (ir, layout, plan, guide_plan, html):
        path.parent.mkdir(parents=True, exist_ok=True)

    record: dict[str, Any] = {
        "slug": slug, "code": source.code, "revision": source.revision,
        "variant": source.variant, "in_corpus": source.in_corpus,
        "source_file": source.pdf.name, "sha256": sha256_file(source.pdf),
        "stage_failed": None, "error": None,
    }

    assets = work / "html" / "assets"
    record["images_extracted"] = extract_images(source.pdf, assets)

    steps = [
        ("extract", [str(HERE / "extract.py"), "--pdf", str(source.pdf),
                     "--form-code", source.code, "--revision", source.revision,
                     "--expected-sha256", record["sha256"], "--out", str(ir)]),
        ("lattice", [str(HERE / "lattice.py"), "--ir", str(ir), "--out", str(layout)]),
        ("guides", [str(HERE / "guides.py"), "--ir", str(ir), "--layout", str(layout),
                    "--source-root", str(source_root), "--out", str(guide_plan)]),
        ("fonts", [str(HERE / "fonts.py"), "--ir", str(ir), "--out", str(plan)]),
    ]
    for name, argv in steps:
        ok, error = run_stage(argv)
        if not ok:
            record["stage_failed"] = name
            record["error"] = error
            return record
        if name == "fonts":
            stage_fonts(plan, fonts_dir)

    guide = json.loads(guide_plan.read_text(encoding="utf-8"))
    standalone = [pathlib.Path(p) for p in guide["standalone_pdfs"]]
    has_guide = bool(guide["inline"] or standalone)
    splitting = has_guide and guide_emission_supported()
    guide_html = work / "html" / f"{slug}.{GUIDE_DOCUMENT}"
    guide_source_irs: list[pathlib.Path] = []
    if splitting and standalone:
        stage_guide_pdfs(standalone, guide_html.parent / GUIDE_PDF_DIR)
        if GUIDE_SOURCE_FLAG in emit_options():
            guide_source_irs, error = stage_guide_sources(
                standalone, slug, source.code, source.revision, work)
            if error:
                record["stage_failed"] = "extract-guide"
                record["error"] = error
                return record

    # The form. A bundle with nothing to split is emitted exactly as it was
    # before guides existed -- same argv, same bytes -- so the 22 forms with no
    # guide content cannot regress on a run that only touches the other 29.
    emit_argv = [str(HERE / "emit.py"), "--ir", str(ir), "--layout", str(layout),
                 "--font-plan", str(plan), "--rule-backend", backend, "--out", str(html)]
    if splitting:
        emit_argv += ["--guide-plan", str(guide_plan), "--document", "form"]
        with_supported(emit_argv, [("--guide-href", GUIDE_DOCUMENT)])
    ok, error = run_stage(emit_argv)
    if not ok:
        record["stage_failed"] = "emit"
        record["error"] = error
        return record

    # One guide document per bundle, whatever the guide is made of: emit.py
    # renders the inline regions and embeds the standalone PDFs in the same
    # document, so a form with both still gets exactly one guide.html.
    if splitting:
        guide_argv = [str(HERE / "emit.py"), "--ir", str(ir), "--layout", str(layout),
                      "--font-plan", str(plan), "--rule-backend", backend,
                      "--guide-plan", str(guide_plan), "--document", "guide",
                      "--out", str(guide_html)]
        with_supported(guide_argv, [("--form-href", FORM_DOCUMENT),
                                    ("--guide-pdf-dir", GUIDE_PDF_DIR)])
        with_supported(guide_argv, [(GUIDE_SOURCE_FLAG, str(p)) for p in guide_source_irs])
        if guide_layout:
            with_supported(guide_argv, [("--guide-layout", guide_layout)])
        ok, error = run_stage(guide_argv)
        if not ok:
            record["stage_failed"] = "emit-guide"
            record["error"] = error
            return record

    ir_data = json.loads(ir.read_text())
    layout_data = json.loads(layout.read_text())
    record.update({
        "pages": len(ir_data["pages"]),
        "paper": f"{ir_data['paper']['width_pt']}x{ir_data['paper']['height_pt']}",
        "uniform_paper": ir_data["paper"]["uniform"],
        "fonts": sorted({f["family"] for f in ir_data["fonts"].values()}),
        "rules": sum(p["stats"]["rules_structural"] for p in ir_data["pages"]),
        "text_runs": sum(len(p["text_runs"]) for p in ir_data["pages"]),
        "images": sum(len(p["images"]) for p in ir_data["pages"]),
        "cells": sum(len(p["cells"]) for p in layout_data["pages"]),
        "comb_cells": sum(1 for p in layout_data["pages"] for c in p["cells"] if c.get("comb")),
        "growables": [
            {"page": p["index"], "rows": g["row_count"], "capacity": g["capacity"],
             "pitch_pt": g["row_pitch_pt"]}
            for p in layout_data["pages"] for g in p.get("growable", [])
        ],
        "sources": [{"role": "form", "file": source.pdf.name, "sha256": record["sha256"]}]
                   + [{"role": "guide", "file": pdf.name, "sha256": sha256_file(pdf)}
                      for pdf in (standalone if splitting else [])],
        "guide": (guide_block(guide, standalone, bool(guide_source_irs))
                  if splitting else None),
        "guide_detected": {"inline_pages": [e["page"] for e in guide["inline"]],
                           "standalone_pdfs": [p.name for p in standalone]},
        "html_bytes": html.stat().st_size,
        "html": str(html),
        "guide_build": {"plan": str(guide_plan),
                        "html": str(guide_html) if splitting else None,
                        "pdfs": [str(p) for p in (standalone if splitting else [])]},
        "guide_source_irs": [str(p) for p in guide_source_irs],
        "font_plans": [str(plan)],
    })
    return record


STYLE_RE = re.compile(r"<style[^>]*>(.*?)</style>", re.DOTALL)
COMMENT_RE = re.compile(r"/\*.*?\*/", re.DOTALL)

# Keys that name the build tree, not the source. They are machine-dependent and
# stay out of the committed bundle.
BUILD_ONLY_KEYS = ("html", "font_plans", "guide_build", "guide_source_irs")


def split_rules(css: str) -> list[tuple[str, str]]:
    """A stylesheet as (prelude, body) pairs, with nested at-rules kept whole.

    This used to be one regex over `sel{body}`, which cannot see nesting: it
    matched the rules *inside* `@media print{...}` and dropped the condition
    around them. A conditional rule that has lost its condition is not a
    smaller stylesheet, it is a different one. `@media print{.doc-link{display:
    none}}` became an unconditional `.doc-link{display:none}`, which hid both
    documents' navigation links on screen as well as on paper, and every
    print-only pagination rule the guide document now carries would have been
    applied to the screen the same way.

    Depth counting is enough because the generated CSS has no strings holding
    braces; comments are stripped first so one in prose cannot open a phantom
    block.
    """
    css = COMMENT_RE.sub("", css)
    out: list[tuple[str, str]] = []
    index = 0
    while True:
        brace = css.find("{", index)
        if brace < 0:
            return out
        prelude = css[index:brace].strip()
        depth = 1
        cursor = brace + 1
        while cursor < len(css) and depth:
            depth += (css[cursor] == "{") - (css[cursor] == "}")
            cursor += 1
        if prelude:
            out.append((prelude, css[brace + 1:cursor - 1].strip()))
        index = cursor


def document_rules(html_path: pathlib.Path) -> list[tuple[str, str]]:
    html = html_path.read_text(encoding="utf-8")
    return split_rules("\n".join(STYLE_RE.findall(html)))


def collect_fonts(records: list[dict[str, Any]],
                  fonts_src: pathlib.Path) -> dict[str, pathlib.Path]:
    """Every WOFF2 the font plans actually resolved, by the file name they use.

    Driven by the plans rather than by listing a directory, so a family added to
    fonts.py lands in forms/fonts/ without anyone remembering to stage it, and a
    family that stopped being used stops being copied. `fonts_src` stays as the
    fallback for a file the plan names but whose source tree is not on this
    machine, which keeps an already-populated directory working.

    A guide PDF gets its own plan, so a family that only the guide uses is
    staged too; a plan that resolved nothing contributes nothing.
    """
    found: dict[str, pathlib.Path] = {}
    for record in records:
        for plan_path in record.get("font_plans", []):
            if not pathlib.Path(plan_path).is_file():
                continue
            plan = json.loads(pathlib.Path(plan_path).read_text(encoding="utf-8"))
            root = pathlib.Path(plan["generator"]["fonts_root"])
            for face in plan["faces"]:
                relative = face.get("font_file")
                if not relative:
                    continue
                name = pathlib.PurePosixPath(relative).name
                for candidate in (root / relative, fonts_src / name):
                    if candidate.is_file():
                        found.setdefault(name, candidate)
                        break
    return found


def link_stylesheets(html: str, depth: str, own_css: str) -> str:
    """Strip the inline <style>, link base.css plus the document's own sheet."""
    links = (f'<link rel="stylesheet" href="{depth}base.css">\n'
             f'<link rel="stylesheet" href="{own_css}">')
    html = STYLE_RE.sub("", html, count=0)
    html = html.replace("</head>", f"{links}\n</head>", 1)
    html = html.replace('url("fonts/', f'url("{depth}fonts/')
    html = html.replace("url('fonts/", f"url('{depth}fonts/")
    # Artwork moved to the shared pool alongside fonts/, so every reference to
    # it has to climb out of the bundle too -- href/src in the markup and url()
    # in any inlined rule.
    for prefix in ('href="assets/', "href='assets/", 'src="assets/', "src='assets/",
                   'url("assets/', "url('assets/", "url(assets/"):
        html = html.replace(prefix, prefix.replace("assets/", f"{depth}assets/"))
    return html


def render_css(header: str, rules: Iterable[tuple[str, str]]) -> str:
    return header + "\n".join(f"{sel} {{ {body} }}" for sel, body in rules) + "\n"


def package(records: list[dict[str, Any]], out_root: pathlib.Path,
            fonts_src: pathlib.Path, generator_version: str) -> dict[str, Any]:
    """Split shared CSS out, then write one self-contained bundle per form."""
    ok = [r for r in records if r["stage_failed"] is None]
    if not ok:
        return {"shared_rules": 0, "bundles": 0, "guides": 0}

    # A declaration shared by every form belongs in base.css. Anything else is
    # form-specific. This is computed, not curated, so it cannot drift. Only the
    # *form* documents vote: a guide document is present in 29 bundles of 51, so
    # letting it vote would demote shared scaffolding out of base.css and churn
    # 51 form.css files for a document that is not the form.
    per_form = {r["slug"]: document_rules(pathlib.Path(r["html"])) for r in ok}

    counts: collections.Counter[tuple[str, str]] = collections.Counter()
    for rules in per_form.values():
        counts.update(set(rules))
    shared = {rule for rule, n in counts.items() if n == len(per_form)}

    out_root.mkdir(parents=True, exist_ok=True)
    (out_root / "base.css").write_text(
        render_css("/* Shared scaffolding for every generated form.\n"
                   " * Computed as the declarations common to all forms, so editing here\n"
                   " * changes every form at once. Form-specific rules live in each\n"
                   " * bundle's form.css, guide-specific rules in its guide.css. */\n",
                   sorted(shared, key=lambda r: r[0])),
        encoding="utf-8")

    fonts_out = out_root / "fonts"
    fonts_out.mkdir(exist_ok=True)
    for name, path in sorted(collect_fonts(ok, fonts_src).items()):
        shutil.copy2(path, fonts_out / name)

    guides_written = 0
    for record in ok:
        target = out_root / ("" if record["in_corpus"] else "extra") / record["slug"]
        target.mkdir(parents=True, exist_ok=True)
        depth = "../" * (len(target.relative_to(out_root).parts))

        own = [r for r in per_form[record["slug"]] if r not in shared]
        (target / "form.css").write_text(
            render_css(f"/* {record['code']} rev {record['revision']} -- form-specific rules.\n"
                       f" * Shared scaffolding is in {depth}base.css. */\n", own),
            encoding="utf-8")
        (target / FORM_DOCUMENT).write_text(
            link_stylesheets(pathlib.Path(record["html"]).read_text(encoding="utf-8"),
                             depth, "form.css"),
            encoding="utf-8")

        # The guide document, when the bundle has one. Its stylesheet is split
        # the same way the form's is: whatever base.css already carries is
        # dropped, the rest becomes guide.css.
        build = record.get("guide_build") or {}
        guide_html = build.get("html")
        if guide_html:
            guide_own = [r for r in document_rules(pathlib.Path(guide_html))
                         if r not in shared]
            (target / "guide.css").write_text(
                render_css(f"/* {record['code']} rev {record['revision']} -- guide-specific rules.\n"
                           f" * Shared scaffolding is in {depth}base.css; the form's own rules\n"
                           f" * are in form.css and are not loaded by the guide. */\n", guide_own),
                encoding="utf-8")
            (target / GUIDE_DOCUMENT).write_text(
                link_stylesheets(pathlib.Path(guide_html).read_text(encoding="utf-8"),
                                 depth, "guide.css"),
                encoding="utf-8")
            # The embedded PDFs are copied per bundle, not shared like assets/:
            # a guide belongs to one form, and a directory of every form's
            # guides would be 13 documents nobody asked this bundle for.
            for pdf in build.get("pdfs", []):
                destination = target / GUIDE_PDF_DIR
                destination.mkdir(exist_ok=True)
                shutil.copy2(pdf, destination / pathlib.Path(pdf).name)
            guides_written += 1

        # Artwork is shared, not copied per bundle, for the same reason base.css
        # is: the pool holds 119 unique images and copying it into all 51 made
        # 6,069 files. Beyond the wasted space, a hand-edit to a seal in one
        # bundle would silently diverge from the fifty other bundles drawing the
        # same seal. Names are content hashes, so sharing cannot collide.
        source_assets = pathlib.Path(record["html"]).parent / "assets"
        if source_assets.is_dir():
            shutil.copytree(source_assets, out_root / "assets", dirs_exist_ok=True)

        provenance = {k: v for k, v in record.items() if k not in BUILD_ONLY_KEYS}
        provenance["generator"] = generator_version
        provenance["note"] = (
            "Generated once from the pinned source PDF, then maintained by hand. "
            "Re-running the generator over this directory would discard manual edits."
        )
        (target / "provenance.json").write_text(
            json.dumps(provenance, indent=2) + "\n", encoding="utf-8")

    return {"shared_rules": len(shared), "bundles": len(ok), "guides": guides_written}


def guide_totals(records: list[dict[str, Any]]) -> dict[str, int]:
    """How the guide documents split by origin, for the run's closing summary."""
    inline = standalone = both = 0
    for record in records:
        origins = set((record.get("guide") or {}).get("origins", []))
        inline += origins == {"inline-region"}
        standalone += origins == {"standalone-pdf"}
        both += len(origins) > 1
    return {"inline": inline, "standalone": standalone, "both": both,
            "total": inline + standalone + both}


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source-root", type=pathlib.Path,
                        default=pathlib.Path.home() / "Downloads/forms")
    parser.add_argument("--out", type=pathlib.Path, default=REPO / "forms")
    parser.add_argument("--work", type=pathlib.Path, default=REPO / "build")
    parser.add_argument("--fonts-dir", type=pathlib.Path,
                        default=REPO / "build/html/fonts")
    parser.add_argument("--rule-backend", choices=("svg", "css"), default="svg")
    parser.add_argument("--guide-layout", choices=("absolute", "reflow"), default=None,
                        help="How guide.html lays its content out (default: emit.py's).")
    parser.add_argument("--only", action="append", default=None,
                        help="Restrict to these form codes (repeatable).")
    parser.add_argument("--dry-run", action="store_true",
                        help="List what would be converted and stop.")
    parser.add_argument("--report", type=pathlib.Path, default=None)
    args = parser.parse_args(list(argv) if argv is not None else None)

    discovered = discover(args.source_root)
    sources = discovered
    if args.only:
        wanted = {code.upper() for code in args.only}
        sources = [s for s in sources if s.code in wanted]

    corpus = [s for s in sources if s.in_corpus]
    extra = [s for s in sources if not s.in_corpus]
    print(f"discovered {len(sources)} fileable PDFs "
          f"({len(corpus)} in corpus, {len(extra)} extra)", file=sys.stderr)
    missing = CORPUS - {s.code for s in corpus}
    if missing:
        print(f"corpus forms with no source PDF ({len(missing)}): "
              f"{', '.join(sorted(missing))}", file=sys.stderr)

    # Every skipped non-form maps to a parent form; one with no built form has
    # nowhere to go, and silently dropping it is how a guide goes missing. The
    # check reads the whole discovery, not the --only subset, so restricting a
    # run cannot invent orphans.
    standalone = guides.standalone_guide_pdfs(args.source_root)
    built = {f"{s.code}-{s.revision}".upper() for s in discovered}
    orphans = [(key, p) for key, paths in standalone.items() for p in paths if key not in built]
    print(f"{sum(len(v) for v in standalone.values())} standalone guide PDFs "
          f"for {len(standalone)} form keys", file=sys.stderr)
    for key, path in orphans:
        print(f"  guide with no built form: {key:<14} {path.name}", file=sys.stderr)

    if args.dry_run:
        for s in sources:
            tag = "corpus" if s.in_corpus else "extra "
            guide = standalone.get(f"{s.code}-{s.revision}".upper(), [])
            note = f"  + guide {guide[0].name}" if guide else ""
            print(f"  {tag}  {s.slug:<22} {s.pdf.parent.name}/{s.pdf.name}{note}")
        return 0

    if not guide_emission_supported():
        print("emit.py does not accept "
              f"{' and '.join(sorted(GUIDE_EMIT_FLAGS))}: guides are detected and "
              "planned, but nothing is split and no guide.html is written.",
              file=sys.stderr)

    records = []
    for i, source in enumerate(sources, 1):
        record = convert(source, args.work, args.rule_backend, args.fonts_dir,
                         args.source_root, args.guide_layout)
        records.append(record)
        status = "ok" if record["stage_failed"] is None else f"FAIL@{record['stage_failed']}"
        guide = record.get("guide") or {}
        note = f"  guide {guide['document']}" if guide else ""
        print(f"[{i:>2}/{len(sources)}] {source.slug:<24} {status}{note}", file=sys.stderr)

    generator = f"tools/formgen batch.py (rule-backend={args.rule_backend})"
    summary = package(records, args.out, args.fonts_dir, generator)

    failed = [r for r in records if r["stage_failed"]]
    totals = guide_totals(records)
    print(f"\nconverted {summary['bundles']}/{len(records)}; "
          f"{summary['shared_rules']} shared CSS rules hoisted to base.css", file=sys.stderr)
    print(f"{summary['guides']} bundles gained a guide document "
          f"({totals['inline']} from an inline region, {totals['standalone']} from a "
          f"standalone PDF, {totals['both']} from both)", file=sys.stderr)
    for r in failed:
        print(f"  FAILED {r['slug']:<24} at {r['stage_failed']}: "
              f"{(r['error'] or '')[:120]}", file=sys.stderr)

    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
