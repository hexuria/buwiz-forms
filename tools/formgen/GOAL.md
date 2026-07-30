# Goal: every eBIRForms sheet correct, fillable, printable and proven

Bring all 51 generated forms to a state where a taxpayer can fill them, print
them, and get a sheet indistinguishable from the official form — and where that
claim is checked by a command rather than asserted by a person.

Scope is `tools/formgen/` and `forms/` on branch `gol/pdf-native-form-extraction`.
Nothing outside this worktree changes. `main` stays clean.

## Done when

```sh
python3 tools/formgen/gate.py
```

exits 0. **`gate.py` does not exist yet; building it is the first increment.**
It must check all of the following and print a line per check:

1. Every module with `--self-test` passes (`extract`, `lattice`, `fonts`,
   `guides`, `emit`, `verify`, `index_page`, `gate` itself).
2. A clean regenerate converts **51/51** with no stage failure.
3. `rules 100% on 51/51`, `rules_missing 0`, `rules_extra 0`,
   `rules_thickness_violations 0`.
4. `paper exact 51/51`.
5. `artwork complete 51/51`, `images_placement_violations 0`.
6. `text == 100% on 51/51`, `text_missing 0`, `text_extra 0` corpus-wide.
7. The eight assertions in **New permanent assertions** below.
8. `review-findings.json`: every finding of severity `blocker` or `major` has
   `status` of `fixed` or `not-a-defect` **and** a non-empty `resolution`.
   `minor` and `cosmetic` may remain `open`; they are not gating.
9. Determinism: two consecutive regenerates produce a byte-identical `forms/`.
10. `git status --short forms/` shows no deletion of a tracked file.

Any check that cannot be evaluated is a **failure**, never a pass. A missing
check is the failure mode this project has already been burned by: the audit
scored 137 real defects as clean because it only compared what it knew to
compare.

## Why this goal exists

The numeric audit reported `rules 100% on 51/51` while 137 real defects were
present, including a solid black rectangle over a form's header, a seal printed
upside-down, statutory tax brackets a taxpayer could type over, hidden
white-on-white text published in black, and money grids with no input fields at
all. The audit was not wrong; it was answering a narrower question than anyone
reading it assumed. Closing that gap is as much the goal as fixing the forms.

## State at the time of writing

Committed at `838cfd8`, plus uncommitted Round 1 work in `extract.py`,
`lattice.py` and `guides.py`.

| | |
| --- | --- |
| forms | 51 (38 corpus bundles incl. 3 attachment/continuation sheets, 13 `extra/`) |
| official eBIRForms package | 51 forms, of which we have **42**; 9 have no source PDF |
| rules / paper / artwork | 100% 51/51 / exact 51/51 / **regressed to 32/51** |
| text | 100% on 48/51 before Round 1; 46/51 after, by design (see debt) |
| review findings | 138 in `tools/formgen/review-findings.json`, 137 audit-blind |

## Method

Read `tools/formgen/BLOCKER-PLAN.md` for the nine root causes and their verified
mechanisms. Read `tools/formgen/README.md` for why the pipeline is vector-only.
Then work in increments, smallest verifiable unit first, and after each one run
`gate.py` and revert anything that regresses.

**One agent per file, always.** Two agents editing `emit.py` concurrently cost a
day of work and produced a misattributed regression. When fanning out, assign
file ownership explicitly and tell each agent to stop and report rather than edit
a file it does not own.

**A change to `extract.py` usually needs a caller.** Round 1 added
`asset_for_xref()` with no caller because `batch.py` was assigned to nobody; that
alone regressed artwork from 51/51 to 32/51. Whenever the IR gains a field, the
same increment must name who consumes it.

## Work remaining

### Round 1 debt — measured, expected, must clear first

Round 1 made the IR correct and left the emitted HTML stale. All three are the
exact size of the handoff, not regressions in the source:

1. **`rules_extra` 0 → 399** on 0605, 1600WP, 2551M, 2553, 2550M. The IR
   correctly stopped claiming 408 phantom hairlines that were an artefact of
   forcing diagonal `l` ops axis-aligned with an invented 0.24pt thickness. The
   HTML still draws them. Clears when `emit.py` renders `page["paths"]`.
2. **`images_missing` 0 → 37** across 19 forms — exactly the 37 `/SMask`
   placements. `extract.py` composites the mask and hashes the composited
   pixels; the asset on disk is still the base image. Clears when `batch.py`
   calls `asset_for_xref()`.
3. **text 100% on 48 → 46** (2550M, 2553). The IR now carries U+FFFD for 7
   glyphs that `rawdict` mis-reported as `§`; the HTML still prints `§`. Clears
   when `emit.py` re-emits and `fonts.py` substitutes glyph 131.

### Round 2 — emission

| Owner | File | Work |
| --- | --- | --- |
| A | `emit.py` | render `paths` (944 across the corpus, 257 filled — the ► markers and decimal points); apply `image["transform"]` (5 placements are not positive-diagonal scales); **C4** make a comb-bearing cell fillable whatever text it also holds; **C6 part 2** never make a cell editable when pre-printed text fills its geometry; **C8** carry each run's colour so white text stays white; **C9** reflow by lattice row, not by text-run y; **S3** drop a page whose content was wholly relocated; **S5** take the run origin after leading spaces; **S7** the 3 genuine render defects |
| B | `batch.py` | call `asset_for_xref()` so composited assets reach `forms/assets/`; keep the tracked-file guard honest |
| C | `fonts.py` | substitute glyph 131; **S6** stop smearing word-spacing across all glyphs as letter-spacing on justified text (2553) |
| D | `audit.py` | the eight new assertions, and make `gate.py` the single entry point |

### Round 3 — resolve the comb-oracle disagreement

`C5` is **not closed and two independent oracles disagree**. `lattice.py`'s own
measurement says merged combs went 471 → 13. A separate oracle built from raw
PDF drawing ops says **204 genuine residual merges across 13 forms** remain:
2550Q 53, 1707 31, 2200S 20, 2200M 19, 2200T 19, 2550-DS 18, 2552 17, 1707A 16,
2000-OT 6, 0605 2, 1606 1, 1801 1, 2316 1.

Do not pick a side by preference. Build a third check that decides it — ideally
by counting compartments a human can see in a rendered crop — then fix whichever
side is wrong. A comb whose slot count disagrees with its printed compartment
count puts a typed digit on top of a divider bar, so this is a blocker, not
bookkeeping.

### Round 4 — re-review

Re-run the visual review over the **40 forms that had findings**, using the same
method: screenshot our page, render the same page of the official PDF at matching
size, look at both, and confirm against the IR before calling anything a defect.
Update `review-findings.json` in place. New findings are appended, not merged
into old ones.

### Deferred, and honestly so

- **The 9 missing official forms** (1600, 1601-E, 1601-F, 1602, 1603, 1604-CF,
  1704, 2000, 2200AN) have no source PDF anywhere on disk. Adding them is one
  `batch.py` run once the files exist. **Do not download them** — ask.
- **App integration.** Field ids are stable and deterministic but nothing binds
  them to the Rust `RenderEnvelope` or to `assets/form-renderer`. Out of scope
  here; it is a project, not a cleanup.
- **Type3 embedding.** Chromium embeds the bundled WOFF2 as Type3, so the differ
  cannot read back a font *family* through the round trip. Advances, positions
  and sizes are all verified. Shipping static instances would fix it; not gating.

## New permanent assertions

These turn the audit's blindness into a test. All eight go in `audit.py` and are
checked by `gate.py`:

1. No `<input>` overlaps a pre-printed text run's bbox.
2. Every comb's slot count equals its printed compartment count.
3. Every printed money box on a form page has an input.
4. No form-side rule extends below that page's guide cut.
5. No emitted run's colour differs from the IR's.
6. No relocated table row has an empty description cell and a non-empty rate
   cell — that pattern is the C9 defect signature.
7. Every image whose transform is not a positive-diagonal scale is emitted with a
   matching transform.
8. No IR text run contains a character that the source did not state: no `?`
   where the codepoint was not U+003F, and no `§` where it was not U+00A7.

## Constraints that cannot be broken

- **Never widen a `verify.py` tolerance** to make something pass. Position
  0.25pt, thickness 0.05pt, advance 0.10pt, size 0.01pt.
- **Never special-case on form code.** A fix belongs in the algorithm or in the
  form's extracted data.
- **The pipeline never rasterises or pixel-diffs.** Correctness is IR-vs-IR and
  numeric. Rasterising is permitted *only* so a human can look at something.
- **Decorative greys keep their exact grey.** Painting them black is a
  documented past failure.
- **Deterministic:** same input, byte-identical output. No timestamps, no
  randomness, no dict-order dependence.
- **`forms/` is hand-maintained.** Never regenerate over an edited bundle
  without saying so; `provenance.json` is what makes an edit traceable.
- **`main` stays clean.** Dependencies belong on this branch, declared where the
  other shipped faces are declared (`packages/form-renderer/package.json`).
- **Report a cost; never trade it.** If a fix costs geometry, say so and stop.
  A partial objective with a clean tree beats a complete one that broke
  something.

## Judgement calls already made

Recorded so they are not silently reversed:

- **SVG paints the rule layer**, not CSS boxes. Chromium snaps CSS box geometry
  to the 0.75pt device grid when printing, collapsing 0.24/0.48/1.44pt onto
  {0.75, 1.5}; SVG rects round-trip at 0.00pt. The SVG is generated from the box
  model per render, so growable bands still grow — this is not the static traced
  SVG that was rejected.
- **`shape-rendering="crispEdges"` is off.** It does not blur sub-pixel rules, it
  deletes them: at 1× DPR every 0.24pt comb divider vanished.
- **Paper is each PDF's own MediaBox**, never A4. Four sizes appear across the
  corpus, including one landscape sheet.
- **Arial Narrow resolves to Arimo at `scaleX(0.820047)`**, not to Roboto
  Condensed, which is not metric-compatible (1.358pt worst glyph vs 0.0058pt).
- **Straddling elements at a guide cut are clipped**, not awarded to the form.
  Awarding them left empty three-sided frames down two-thirds of a page.
- **2551Q's comb pins moved** (page 1 488→489, page 2 258→264). The old pins
  encoded a 14-box TIN reporting 11 slots; freezing them would need a per-form
  special case.
- **The 13 `extra/` bundles stay.** Outside the official 51 but real forms,
  quarantined, and `rm -rf forms/extra` drops them.
