<!--
Conversion Strategy v2, adopted 2026-07-20. Produced from three parallel
investigations (playbook audit, extraction spike on 2316v2021 + 1602Qv2019,
dynamic-behaviour catalog) grounded in the adversarially-verified session
findings recorded in priority-forms-readiness.md and
official-fidelity-criterion-v1.md.

Supersedes the calibration ORDERING and visual-gate TARGETS of the v1 flow in
.codex/skills/ebirforms-convert-form-to-html/. It does NOT supersede that
skill's fail-closed rules: identity pinning, Rust-first tax behaviour,
artwork provenance, adaptive-guide safety, and empty trusted-producer
registries all remain in force. The 1% complete-page number remains a
mandatory, never-hidden diagnostic; it no longer directs calibration effort.
-->

# eBIRForms HTML Conversion Strategy v2

**Scope:** scale from 10 converted forms to all 35 form directories in `/Users/uriah/Downloads/forms` (42 PDFs, 87 measured pages including guides/attachments) while preserving real-form dynamic behaviour.
**Repo:** `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity`, branch `codex/print-preview-parity`.
**Basis:** every claim below traces to the adversarially-verified session findings, the playbook audit, the extraction spike (2316v2021 + 1602Qv2019), or the dynamic-behaviour catalog. Estimates not backed by measurement are labeled as such.

---

## 1. DOCTRINE — what changed and why

1. **Text pixels are unwinnable, corpus-wide. Stop paying for them.** `pdffonts` shows `emb=no` for the dominant fonts on every one of the 35 forms; the pinned references therefore encode Poppler's *substituted* glyph outlines, not BIR's. For 2551Q, glyph outline shape is ~57% of the text residual and text is ~56–59% of total pixel error. Every rendering-side fix was adversarially refuted: real Arial scores *worse*, path-filling our own text through the reference pipeline scores worse, 19 weight variants and 26 size variants and all CSS knobs failed. (Nuance from the spike: `ABCDEE+` subsets such as Arial Narrow/Calibri *are* embedded, but the dominant Arial/Times WinAnsi fonts still substitute, so the conclusion stands.) The historical 1% complete-page gate is unreachable and any workflow step aimed at it is waste.
2. **Structure pixels are winnable. Pay for them.** With text removed from both sides by construction, 2551Q scores 3.0556% (p1) / 2.3837% (p2) against a rasterization floor of ~0.55–0.69%. The residual is pure displacement — ink quantity matches within 0.1%: ~1px rule registration drift, ATC row-pitch drift (66% of p2 structural residual), comb-tick geometry. All CSS-fixable.
3. **The release criterion splits by evidence type** (per `docs/form-print-readiness/official-fidelity-criterion-v1.md`):
   - **Structure → pixels**: cell-scoped edge F1 (radius 1) + structural-ink displacement + ink budget, computed on text-neutralized rasters.
   - **Text → content assertions**: an exhaustive static-text manifest (every printed string, page-scoped, typed), not pixels.
   - **Artwork → payload/hash**: decoded/re-encoded PDF417/QR/seal provenance (encoded-artwork-integrity-v1).
   - It is a **non-regression gate pinned to a reviewed baseline**, never a parity claim. The 1% complete-page number remains a mandatory, never-hidden **diagnostic** (alongside the raw Poppler diff and pinned noise floor). Never report masked or structure-only numbers as parity.
4. **Ground-truth extraction replaces hand-measurement.** PyMuPDF span extraction matched DOM runs 93/93 on the 2551Q ATC table with exact bboxes, sizes, and bold flags. The spike extracted full geometry contracts for two very different drawing styles (Word-clean 2316, Excel-fragmented 1602Q) in 0.045 s and 0.37 s respectively; corpus extraction is under 2 minutes of machine time. The 40% calibration cost center and the hand-measured-coordinate spec authoring both collapse into "generate, then review."
5. **Semantic flow layout and dynamic behaviour are non-negotiable.** These are forms, not pictures: growable schedules with continuation pages, adaptive wrap fields with reviewed second lines, comb fields that become plain boxes rather than truncate, checkboxes, provenance-verified artwork. Per-glyph absolute positioning broke the static-text assertion and destroys adaptive behaviour — it is banned for value-bearing regions.
6. **Everything fail-closed stays fail-closed.** Identity pinning, Rust-first tax behaviour, empty trusted-producer frozensets, evidence-only promotion commits, and the geometry gate's refusal to certify `pending`/`unresolved` fits are all KEEP. Generators produce *candidates*; humans accept them; audits verify the accepted artifacts.

---

## 2. THE PIPELINE — per-form conversion flow

Existing scripts are used as-is unless marked NEW. All commands prefixed `rtk`. Stage state for the orchestrator lives under `tmp/convert/<CODE>-<REV>/` (never committed, never evidence).

### Stage 1 — Identity and source lock (existing)
- **Command:** `rtk python3 .codex/skills/ebirforms-convert-form-to-html/scripts/inventory_form.py --source-dir /Users/uriah/Downloads/forms/<DIR>`
- **Generated:** source-pack hash inventory. **Hand:** nothing. **Reviewed:** identity fields.
- **Exit gate:** pinned PDF sha256 recorded; fail-closed on ambiguity.
- **Time:** 10–15 min.

### Stage 2 — One-command reference preparation (NEW wrapper: `scripts/prepare_form_references.mjs`)
- **Command:** `rtk node scripts/prepare_form_references.mjs --repo . --form-code <CODE> --revision <REV> --pdf <pinned pdf> --expected-sha256 <pin>`
- Wraps the existing `prepare_official_reference.py` (Poppler 144 DPI, primary provenance) **and** `scripts/prepare_chromium_reference.mjs` (pdftocairo SVG → same-rasterizer Chromium PNG + per-page noise floor), then emits `references/pins/<CODE>-<REV>.json` (hashes + noise floors) for the **automated Rust pin round-trip**: a codegen step consumes the JSON into `crates/bir-print/src/html_forms/form_*.rs` pins instead of hand-copying; `npm run references:generate` + `rtk cargo test --locked -p bir-print` byte-lock as today.
- **Exit gate:** both rasters pinned, noise floor recorded, `cargo test -p bir-print` green.
- **Time:** ~10 min machine per form; the chromium pipeline must be extended beyond 2551Q (currently 2551Q-only) — one-time engineering, est. 0.5–1 day.

### Stage 3 — Geometry-contract extraction (`scripts/extract_geometry_contract.py` + `scripts/review_geometry_contract.py`, promoted from the spike scripts)
- **Extract:** `rtk node scripts/run_python.mjs scripts/extract_geometry_contract.py --repo . --form-code <CODE> --revision <REV> --pdf <pinned pdf> --expected-sha256 <pin> --output references/geometry-contracts/<CODE>-<REV>/`
- **Review (separate step, mandatory):** `rtk node scripts/run_python.mjs scripts/review_geometry_contract.py --repo . --contract references/geometry-contracts/<CODE>-<REV>/contract.json --pdf <pinned pdf> --output <review dir>`
- Extraction fails closed on a SHA-256 mismatch and writes nothing. `--check-only` re-derives the contract and exits non-zero if the committed copy is stale. Output is deterministic: two runs over the same bytes are byte-identical (no timings or absolute paths enter the document), so it is safe to diff in review.
- Emits `contract.json` (schema `bir-geometry-contract/draft-0`): coalesced rules in pt and px **carrying per-rule thickness, `device_px`, `subpixel` and `predicted_min_tone`**; fill regions; **knockout regions** (white rects painted over shaded bands); full text runs with text/bbox/size/font/bold/italic; comb candidates from both detectors (tick-run + container-aware with smallest-container preference); checkbox candidates from both explicit rects and the rule lattice; image bboxes + xrefs; source sha256. The review script emits per-page 144-DPI overlay PNGs plus a `REVIEW.md` accept/reject checklist.
- **Rule weight is not uniform and must not be averaged away.** Coalescing merges on position *and* thickness. 2551Q draws Part boundaries at 1.44pt (2.88px → 3 device px) directly against 0.48pt interior rules (1 device px); merging on position alone invents a single fictional weight and erases the distinction the renderer has to reproduce. Measured page-1 weight histogram: 552 rules at 1 device px, 47 at 2, 21 at 3, 4 at 4.
- **Sub-pixel guides explain the "grey ink" puzzle.** Comb dividers on 2551Q are 0.24pt = 0.48 device px — pure black in the PDF, but they can never ink a whole pixel, so they cannot raster darker than ~133/255. Raster-side detectors keyed on near-black tone miss them; the vector contract states them exactly. 667 of 778 vertical rules across both pages are sub-pixel.
- Coalescing tolerance is auto-selected by a plateau sweep (0.5→3.0pt). Plateaus are detected by *relative* stability (1%), not exact equality: real staircases never repeat a count exactly, and an exact-match selector silently falls back to the first tolerance in the sweep. Corpus-wide the selector picks 2.0pt on 23 of 42 PDFs, 0.75 on 12, 0.5 on 6, 1.0 on 1.
- **Generated:** everything above. **Reviewed (mandatory):** comb/checkbox candidate accept/reject via overlays. **Hand:** semantic naming (TIN vs ZIP) — always human, never proposed by either script.
- **Two comb passes are needed, not one.** A band pass (a run of dividers sharing a y-extent) carries 2551Q's 12-to-40-cell combs; a container-anchored pass (ticks inside an explicit rect) carries the 3-to-5-cell combs that dominate Word-clean forms, which the band pass cannot see because a 3-cell comb has only 2 interior ticks. With the band pass alone 2316 page 1 reports 4 combs; with both it reports 29.
- **Known limitations, measured not assumed:** (a) the detector finds *geometric* comb runs, not render fields — where several fields share one uninterrupted uniform-pitch tick run with no heavier divider between them, they read as one comb (2551Q: 55 geometric combs vs 96 declared `.comb-value` fields; item 8 Taxpayer's Name is one 40-cell run spanning the full page width with only the page borders at each end). Splitting those is a review decision, not a geometric one. (b) Merged/spanned table cells remain unproven — see the cell-graph builder open decision. (c) The 4 zero-comb PDFs in the corpus are all prose guides (1601-FQ, 1601-EQ, 1701-MS, 2550Q guidelines), correctly yielding nothing. (d) Older-generation forms (2550M, 2551M) are sparse by design — 1 and 4 comb candidates respectively — because they use ruled boxes rather than character combs; a low count there is the form, not a miss.
- **Exit gate:** contract committed with accepted-candidate annotations; overlays archived as review evidence.
- **Time:** measured over the full 42-PDF / 87-page corpus — 8.9 s of extraction, 13.7 s wall including SHA-256 hashing and per-form process spawn, 42/42 succeeding; slowest single form 0.87 s (1701, 4 pages). Overlay review is minutes per page, ~15–45 min per form.

### Stage 4 — Generated manifests and scaffolds (three NEW generators, all contract-consumers)
1. `scripts/generate_static_text_manifest.py --contract <path> --out packages/form-renderer/visual/official-<code>-static-text.gen.ts` — first draft of the page-scoped typed string manifest (the text gate). Replaces 16KB hand-written files; 8 existing hand inventories become validation oracles for the generator.
2. `scripts/generate_region_tables.py --contract <path> --out packages/form-renderer/visual/<code>-regions.gen.ts` — the 4–17 hand-measured float coordinates per parity spec, generated.
3. `scripts/generate_layout_scaffold.py --contract <path> --out tmp/convert/<CODE>-<REV>/scaffold/` — measured grid geometry (row pitch, column widths, section boxes, comb capacities, checkbox geometry) as a starting CSS skeleton + a component-spec JSON (comb cells, money-comb shape, checkbox positions). *Higher risk:* the cell-graph builder that turns rule lattices into table skeletons is the one unproven link (merged/spanned cells on 1602Q p2, per-band column systems on 2316) — run it as the next spike before trusting it corpus-wide.
- **Generated:** all drafts. **Reviewed:** every manifest before commit (the text gate is only as good as its manifest). **Hand:** nothing at this stage.
- **Exit gate:** reviewed manifest + region tables committed; scaffold available.
- **Time:** seconds machine; manifest review 30–60 min/form; expected honest coverage: ~70–80% of hand-measured geometry on dense field pages, ~40–50% on prose/guideline pages.

### Stage 5 — Tax behaviour: Rust provider + fixture matrix (existing flow, unchanged)
- Rust-first provider in `crates/bir-print/src/html_forms/`, fixture matrix (empty/short/exact/capacity-plus-one combs, plain-field proofs), `rtk cargo test --locked -p bir-print`, `rtk npm run contracts:check`.
- Only 8/35 dirs have XML samples (`0605`, `0619E`, `0619F`, `1601Cv2018`, `1701v2018`, `1702MXv2018c`, `1702RTv2018c`, `2550Qv2024`); forms without samples stay `ScaffoldOnly` per existing policy — conversion of the render layer still proceeds.
- **Exit gate:** provider tests + fixtures green, fail-closed on undeclared schedules.
- **Time:** 0.5–1.5 days/form (real tax logic; generators do not help here and should not).

### Stage 6 — Semantic HTML build from scaffold (hand-authored, generator-fed)
- Author TSX/CSS in `packages/form-renderer/src/forms/` starting from the Stage-4 scaffold and the shared primitive library (Section 3) instead of from nothing. Flow layout only; no absolute positioning of value-bearing regions.
- **Exit gate:** `rtk npm run typecheck:forms && rtk npm run test:forms`; geometry gate (`measureRenderedPages`) reports no overflow.
- **Time:** 0.5–1 day for family siblings, 1–2 days for novel layouts (vs. historical 3–4 calendar days with 6–15 TSX + 9–14 CSS calibration commits each).

### Stage 7 — Artwork verification (existing + NEW batch tool)
- Existing per-form flow (`prepare_monochrome_bir_seal.py`, decoded/re-encoded PDF417 module-matrix SVG with payload provenance). NEW: `scripts/extract_machine_artwork.py --contract <path> --pdf <pinned pdf> --out packages/form-renderer/src/forms/official<Code>Assets.ts` batching the extract/decode/re-encode step — every page of every sampled PDF carries a top-right PDF417, so this runs ~87 times corpus-wide.
- **Exit gate:** encoded-artwork-integrity-v1 (payload/hash match) per artwork; `Absent` recorded with hashed object inventory where genuinely absent (0605 precedent).
- **Time:** ~1 h/form including review, mostly machine once the batch tool exists.

### Stage 8 — Structural calibration (NEW scored artifact: `scripts/compare_structural.mjs`)
- **Command:** `rtk node scripts/compare_structural.mjs --form-code <CODE> --revision <REV>` — produces the text-neutralized structural diff (text removed from both sides by construction: reference text masked from the contract's span bboxes, DOM text hidden at screenshot time), scored against the pinned per-page noise floor (~0.55–0.69%), plus `rtk npm run report:visual:regions` for displacement targeting. Promotes what currently lives as ad-hoc `tmp/` trial dirs into a first-class, reviewable artifact.
- Iterate **CSS displacement fixes only** (rule registration, row pitch, comb ticks). Full loop details in Section 4.
- **Exit gate:** structural components of official-fidelity-v1 pass against the reviewed baseline; complete-page 1% number and raw Poppler diff reported as diagnostics.
- **Time:** hours per form, not days (displacement is enumerable from the region report).

### Stage 9 — Native print/export + audit (existing, one modification)
- Existing platform certification drivers, `verify_form_conversion.py`, `produce_visual_evidence.py`, `rtk npm run audit:forms:migration -- --require-clean-source`, `rtk npm run audit:no-legacy`.
- **Required change (open tasks #12/#13):** implement the official-fidelity-v1 components in spec + audit; retarget the `visual_parity` capability/gate naming; **drop the `report.passed is not True` abort at `scripts/produce_visual_evidence.py` (~line 177)** — it hard-requires the unreachable 1% gate and can currently never emit evidence — while keeping every other fail-closed behaviour. Re-point every 1% citation (full list in the playbook audit §D, including `CLAUDE.md` itself, both SKILL.md files, `visual-calibration.md`, `docs/adding-a-new-form.md` §7, `docs/form-tooling-guide.md`, `release-visual-threshold.ts`, `audit_html_form_migration.py`).
- **Exit gate:** audits green with the composite criterion; evidence produced by user-reviewed registered producers only.
- **Time:** amortized machinery; ~0.5 day/form for runs + review.

### Stage 10 — Reviewed-baseline snapshot + promotion (NEW workflow)
- Snapshot the accepted structural rasters + manifests as the per-form non-regression baseline (the anchor official-fidelity-v1 pins to), with replacement-version audit trail for any later change. Promotion evidence lands in a dedicated evidence-only commit binding a clean curated source revision, exactly as today.
- **Exit gate:** baseline committed and referenced by the gate config.
- **Time:** <1 h/form once the workflow exists (one-time build est. 1–2 days).

### Orchestrator (NEW: `scripts/convert_form.py`)
- **Command:** `rtk python3 scripts/convert_form.py --form-code <CODE> --revision <REV> --source-dir /Users/uriah/Downloads/forms/<DIR> [--resume] [--until <stage>]`
- Chains Stages 1–4 and 7–8 mechanically, pauses at every human-review checkpoint (comb acceptance, manifest review, baseline acceptance), records stage state in `tmp/convert/<CODE>-<REV>/state.json`, and emits one consolidated report. Shared paper-geometry constants for the corpus classes (612×792, 612×936, 612×1008, plus the single 936×612 landscape attachment) live in one module consumed by generators and renderer alike — fixing the current three-way naming disagreement (renderer `paper-legal`=1008pt vs Rust `LEGAL`=936pt vs specs `BIR_FOLIO`=936/`BIR_LEGAL`=1008).

---

## 3. DYNAMIC BEHAVIOUR — pattern library, spec vs code, review rules

### Existing patterns (KEEP, generalize)
| Pattern | Mechanism today | Becomes declarative spec | Stays code |
|---|---|---|---|
| Growable schedules | `RenderSchedulePolicy` (Rust, fail-closed pagination) + hand-mirrored TS `SchedulePolicy` + `paginateSchedule`; only 2551Q actually grows, with ~500 lines bespoke continuation CSS | Rows-per-page/slot counts, column definitions (comb cells, alignment, integer/fraction split), masthead variant strings, identity-strip field list, totals/subtotal/"continued" labels, suppress-on-continuation regions. **Collapse the Rust/TS duplication into one generated source** via the existing `cargo run -p bir-print --bin generate_render_contract` | The splitter state machine, the fail-closed throws on undeclared/over-capacity schedules |
| Adaptive wrap (2551Q Item 17) | `AdaptivePlainValue` 0.5px ladder 12→10.5px, MutationObserver flips `specify-line-extended`, fixed reviewed second row (36pt / section 271.5pt) | `{fieldId, maxPx, minPx, extensionRows, extendedRowHeightPt, sectionHeightPt}` consumed by a shared `ExtendableLine` component; per-field reviewed px table stays a reviewed artifact | Fit ladder, fit certification against client rects, WKWebView refit loop protection |
| Charbox → plain box | `CombValue` (exact, throws over capacity for Rust-validated fields) + `AdaptiveCombValue` (switch to plain box in same footprint, measured fit) | `{integerCells, fractionCells, leadingCells, groupSeparators, align, centavosPolicy}` per money field; **capacities come from the geometry contract** (guide counts verified by overlay review), replacing `OFFICIAL_2551Q_COMB_CAPACITIES`-style hand counts and inline `cells={N}` | The fitting/certification engine; the legacy crude-formula path is retired as forms migrate to measured mode |
| Checkboxes | Shared `CheckChoice` + per-form CSS translateY nudges | Geometry from contract (spike probe was exact on both test forms) | Component |
| Artwork | Per-form `official*Assets.ts`, lossless seal + PDF417 module-matrix SVG; `OfficialPdf417` copy-pasted into 9 files | Placement bboxes + xref provenance from contract; assets generated by `extract_machine_artwork.py` | **One** shared `OfficialPdf417` component (de-duplicate the 9 copies) |
| Repeated furniture | `PageTwoIdentity`/`ContinuationIdentity1701`/`PageIdentity1702MX`/`PageTwoIdentity2550Q`… all hand-rolled per form; payment-details grid duplicated in 9/10 forms; per-form `OfficialDeclaration` | Field lists + capacities per form | Shared `PageIdentityStrip`, `PaymentDetailsGrid`, `Declaration` components parameterized by the spec |

### New primitives required by the unconverted corpus (from the catalog, Part B)
1. **Open-box field** (1604C, 2316, 1621): borderless/rounded-rect field with no comb guides — must join comb as a first-class primitive with the same over-capacity → fit-ladder → fail-closed behaviour.
2. **Pre-printed comb constants** (1702Q TIN branch "0 0 0 0", ATC "IC 055"; 1621 "W I 1 6 5", "6 . 0 %"): combs with immutable pre-filled cells; spec marks cells as constants, renderer refuses values into them.
3. **Label-keyed fixed row tables** (1604C/1604F January…December + TOTAL) including intra-page column-continuation ("Continuation of Part II" splits columns, not rows).
4. **Multi-schedule continuation sheets** (2000-DST: four independent growable schedules on one page): Rust policy already supports N schedules; the React continuation-sheet design for one-of-N/combined schedules does not exist and needs a reviewed design decision (see Open Decisions).
5. **Whole-form repetition** (2316: one certificate per employee — the dynamic axis is N copies of the entire form for batch print, not rows).
6. **Dual-column amount schedules and specify-rows** (1702Q: EXEMPT/SPECIAL pairs; Item-17 pattern generalized to schedule rows) — spec-level extensions of existing primitives.
7. **Instructions/prose pages** (1602Q p3, 1702Q p3, 1621 p2, guide PDFs): re-flowed semantic prose (never absolute-positioned spans), structure gate on section geometry, text gate on manifest; needs a metric-compatible serif (e.g. Tinos) added to the bundled font set (Arimo + Roboto Condensed today).
8. **Tiered/nested ATC tables** (2000-DST DS 106/109 five-line rate brackets, row-spanning cells) — beyond the 2551Q ATC table; depends on the cell-graph spike.

### Review rules that keep dynamic behaviour safe (unchanged in spirit, now enforced by spec)
- Fitting is fail-closed: `measureRenderedPages` refuses to certify while any fit is `pending` and counts `unresolved` as overflow. No form ships with a pending fit.
- **No truncation, ever.** Over-capacity always converts to a measured plain box.
- Extended lines are **fixed, reviewed geometry** (explicit second-row heights), never free reflow.
- Guide counts and capacities come from the contract **only after** overlay accept/reject review; Rust-validated fields (TIN) keep exact-capacity throws.
- Semantic field naming and input-vs-display classification are always human.
- Growth policies live in Rust and fail closed; TS consumes generated policy, never hand-mirrors it.

---

## 4. CALIBRATION — the new loop

**The primary iteration metric is the text-excluded structural diff** (`scripts/compare_structural.mjs`): fast, honest, and every point of it is fixable. The loop:

1. Run the structural diff; compare against the pinned per-page noise floor (~0.55–0.69%).
2. Run `rtk npm run report:visual:regions` on the structural rasters to rank displacement regions.
3. Fix **only displacement**: rule registration (~1px class), row-pitch drift (the ATC pitch was 66% of 2551Q p2's structural residual), comb-tick geometry, fill-region offsets. All CSS.
4. Re-run. Converge toward the floor. Current 2551Q standing: 3.0556% (p1) / 2.3837% (p2) structure-only, ink quantity within 0.1% — displacement, not missing/extra ink.
5. Report, always and clearly labeled: the complete-page pixel number (vs the chromium reference), the raw Poppler diff, and the pinned noise floor — **as diagnostics, never as gates, never hidden, and never with a masked/structure-only number presented as parity**.
6. Exit: the official-fidelity-v1 components (structural edge F1 radius 1, structural-ink, ink budget; static-text manifest; artwork payload/hash) pass against the reviewed baseline, once tasks #12/#13 land. Until then the structural diff + diagnostics are recorded but nothing is promoted (fail-closed, as today).

**Nobody pixel-chases text ever again.** The adversarial record: 19 weight variants, 26 size variants, real Arial, path-filled text through the reference pipeline — all refuted; glyph outline shape is unfixable renderer-side because the official PDFs never embedded the fonts. `diagnose:fonts` is re-labeled attribution-only evidence, never a gap-closing step. The person-hours that went into the ~119 calibration commits (40% of the 297-commit migration burst) are redirected to:
- **static-text manifest correctness** — exhaustive, page-scoped, reviewed content assertions (the text gate), generated first-draft from the contract and human-verified;
- structural displacement fixes (bounded, enumerable);
- dynamic-behaviour fixtures and review.

---

## 5. SCALING TO 35 — corpus inventory and wave plan

### Full corpus (measured with PyMuPDF 1.27.2.3; 42 PDFs, 87 pages total — the earlier ~74 estimate undercounted guides/attachments)

| Dir | Pages (per PDF) | Geometry (pt) | XML | Status |
|---|---|---|---|---|
| 0605 | 2 | 612×936 | yes (4) | **converted** |
| 0619E | 1 | 612×792 | yes | **converted** |
| 0619F | 1 | 612×792 | yes | **converted** |
| 0620v2019 | 1 | 612×792 | no | wave 1 |
| 1600-PTv2018 | 2 | 612×936 | no | wave 2 |
| 1600-VTv2018 | 2 | 612×936 | no | wave 2 |
| 1601-FQ | 2 + guide 1 | 612×936 | no | wave 2 |
| 1601Cv2018 | 2 | 612×936 | yes | **converted** |
| 1601EQv2019 | 2 + guide 1 | 612×936 | no | wave 2 |
| 1602Qv2019 | 3 | 612×936 | no | wave 2 (spike-proven contract) |
| 1603Qv2018 | 2 | 612×936 | no | wave 1 |
| 1604Cv2018 | 1 | 612×936 | no | wave 3 (label-keyed tables) |
| 1604Fv2018 | 2 | 612×936 | no | wave 3 |
| 1606v2018 | 2 | 612×936 | no | wave 1 |
| 1621v2019 | 2 | 612×936 | no | wave 3 (schedule+prose page) |
| 1701Av2018 | 2 | 612×936 | no | wave 4 |
| 1701MSv2024 | 2 + guide 2 | 612×936 | no | wave 4 |
| 1701Qv2018 | 2 | 612×936 | no | **converted** |
| 1701v2018 | 4 + attach 2 (612×792) + conso 2 (**936×612 landscape**) | 612×936 | yes | **converted** (attach/conso scope: open decision) |
| 1702EXv2018 | 3 | 612×936 | no | wave 4 |
| 1702MXv2018c | 4 + attach 2 | 612×936 | yes | **converted** |
| 1702Qv2018 | 3 | 612×936 | no | wave 4 (pre-printed comb constants, dual-column schedules, instructions p3) |
| 1702RTv2018c | 4 | 612×936 | yes | **converted** |
| 1707Av2021 | 2 | 612×936 | no | wave 4 |
| 1709v2020 | 3 | 612×936 | no | wave 4 |
| 2000-DSTv2018 | 2 | 612×936 | no | wave 3 (4 growable schedules, tiered ATC) |
| 2000-OTv2018 | 2 | 612×936 | no | wave 3 |
| 2200Cv2018 | 2 | 612×936 | no | wave 4 |
| 2200Mv2018 | 2 | 612×936 | no | wave 4 |
| 2316v2021 | 1 | 612×936 | no | wave 3 (whole-form repetition, open boxes; spike-proven contract) |
| 2550-DSv2025 | 1 | 612×936 | no | wave 1 |
| 2550M | 4 | 612×1008 | no | wave 2 |
| 2550Qv2024 | 2 + guide 1 | 612×1008 | yes | **converted** |
| 2551M | 2 | 612×1008 | no | wave 1 |
| 2551Qv2018 | 2 | 612×936 | no | **converted** (gold standard) |

### Waves

- **Wave 0 — retrofit the 10 converted forms** (0605, 0619E, 0619F, 1601C, 1701, 1701Q, 1702MX, 1702RT, 2550Q, 2551Q): extend the chromium reference pipeline to all of them, run the extractor to generate their geometry contracts, validate the static-text-manifest generator against the 8 existing hand inventories, snapshot reviewed baselines, switch their specs/audits to official-fidelity-v1, de-duplicate `OfficialPdf417`/identity strips/payment grids into shared primitives. No new form work here — this is where the composite criterion and generators get proven against known-good ground truth. Est. 1.5–2.5 weeks including the criterion implementation (tasks #12/#13).
- **Wave 1 — generator pilot on close siblings** (0620v2019, 2551M, 1603Q, 1606, 2550-DS): forms with converted near-relatives, low novelty, one exercising the 612×1008 class and one the 612×792 class. Purpose: measure the real per-form marginal cost with generators before committing to the schedule. Est. 1–1.5 days/form.
- **Wave 2 — remittance/quarterly family** (1600-PT, 1600-VT, 1601-FQ, 1601EQ, 1602Q, 2550M): standard dense field pages, patterns fully covered by the existing library; 1602Q's contract already exists from the spike. Est. 1.5–2.5 days/form (1602Q p2 prose page at the higher end).
- **Wave 3 — new-primitive forms** (1604C, 1604F, 2316, 2000-DST, 2000-OT, 1621): each introduces a cataloged missing pattern (Section 3). Budget primitive-building separately: est. 1–2 days per new primitive (open-box, pre-printed constants, label-keyed tables, multi-schedule continuation, whole-form repetition), then 2–4 days/form. The cell-graph spike must land before 2000-DST's tiered ATC table.
- **Wave 4 — long/heavy forms** (1701A, 1701MS, 1702EX, 1702Q, 1707A, 1709, 2200C, 2200M): 2–4 pages each, income-tax and excise complexity, instruction pages. Est. 2–4 days/form, mostly Rust provider + review time.

### Parallelization and marginal cost
- **Fully parallel now, corpus-wide:** Stages 1–4 and 7 (identity, references, extraction, manifests, artwork) are machine + review with zero inter-form dependencies. Total machine cost for extraction across all 42 PDFs is **under 2 minutes**; run it for the whole corpus in Wave 0 so every later wave starts from a reviewed contract.
- **Parallel across forms:** Rust providers and HTML builds (independent per form, including across agents — with compact per-form prompts, never full history).
- **Serial per form:** structural calibration and baseline review.
- **Marginal cost target once generators exist** (honest, pilot-verified in Wave 1): family sibling ~1–1.5 days; standard 2-pager ~2 days; novel-pattern form 2–4 days — versus the historical 3–4 calendar days with 40% of commits spent on calibration churn that findings 1–2 prove was partly unwinnable. This is roughly a 2× throughput gain plus elimination of the unbounded pixel-chase risk, not a 10× miracle. Remaining 25 forms: est. 10–14 working weeks of one person's effort end-to-end, less with parallel agents on Stages 5–6.
- XML-less forms (23 of the remaining 25) stay `ScaffoldOnly` with `release_ready: false` until their evidence chains complete; render conversion proceeds regardless since queue/fileability is Rust-owned and independent.

---

## 6. WHAT WE STOP DOING — stop-list with evidence

1. **Stop chasing the 1% complete-page gate.** Evidence: `emb=no` dominant fonts on all 35 PDFs; glyph outline shape ~57% of text residual; text ~56–59% of total error; all rendering-side fixes adversarially refuted. The number survives only as a mandatory diagnostic.
2. **Stop all font-knob gap-closing.** Evidence: 19 weight variants, 26 size variants, real Arial (scored worse), path-filled text (scored worse) — every knob refuted. `diagnose:fonts` becomes attribution-only.
3. **Stop authoring HTML by pixel-diff convergence from nothing.** Evidence: calibration was 40% of 297 commits (119 commits over ~5 days); per-form components averaged 6–15 TSX + 9–14 CSS commits over 3–4 days; 80% of form-component commits were calibration-flavored. Extraction-first authoring replaces it.
4. **Stop hand-measuring coordinates.** Evidence: PyMuPDF 93/93 span match with exact bboxes; spike contracts delivered exact rule positions, fills, comb geometry in <0.4 s/form. The 22–36KB specs' 4–17 hand-measured floats each become generated region tables.
5. **Stop hand-authoring static-text inventories from scratch.** Evidence: 8 hand files + a 16KB hand-written TS manifest exist; the extractor emits reviewable drafts with bboxes as verification anchors.
6. **Stop hand-copying Rust reference pins.** Evidence: `prepare_chromium_reference.mjs` currently prints pins for manual transcription — automate the JSON round-trip; `cargo test` still byte-locks.
7. **Stop hand-mirroring schedule policies between Rust and TS.** Evidence: 2551Q 6/6/12 and 1601C 3/3/3 are duplicated by hand today; the contracts generator (`generate_render_contract`) already exists.
8. **Stop copy-pasting shared components.** Evidence: `OfficialPdf417` duplicated in 9 files; page-two identity strips hand-rolled in 6 forms; payment grid duplicated in 9/10.
9. **Stop absolute-positioning experiments on value-bearing regions.** Evidence: per-glyph placement broke the static-text assertion and destroys adaptive behaviour (session finding 5).
10. **Stop reporting masked/structure-only numbers as parity, and never weaken the numeric gates.** Unchanged honesty rule — structure-only figures are structure figures, clearly labeled; the composite criterion is a non-regression gate, never a parity claim.
11. **Stop ad-hoc `tmp/` trial pipelines as the home of structural comparison.** Evidence: 0619-weight-trial, 0619f-row-trial etc. were unreviewable one-offs; `compare_structural.mjs` makes the artifact first-class.

---

## 7. OPEN DECISIONS for the user

1. **Approve the composite criterion as the release gate** (implement tasks #12/#13) and the coordinated wording change across every §D location — including `CLAUDE.md` itself and the `produce_visual_evidence.py` 1%-PASS abort — in one change, so agents stop being instructed to chase text.
2. **Trusted-producer registration** for each new generator (`extract_geometry_contract.py`, manifest/region/scaffold generators, `compare_structural.mjs`): the audit frozensets are intentionally empty and only you register producers after review. Which, and in what order?
3. **Where geometry contracts live and how they're pinned:** proposed `references/geometry-contracts/<CODE>-<REV>/` tracked in git and hashed into `references/manifest.json` via `npm run references:generate`. Confirm or relocate.
4. **Continuation-sheet design for multi-schedule forms** (2000-DST's four schedules; 1621's schedule-sharing-a-page-with-prose): no official layout exists for these continuations — the masthead/marking/one-of-N layout needs a product decision like 2551Q's "CONTINUATION ATTACHMENT" got.
5. **2316 batch printing:** whole-form-per-employee repetition is a new dynamic axis — is N-certificates-per-print-job in scope for this migration, and where does the employee list come from (Rust envelope shape)?
6. **Scope of guide/attachment PDFs:** 4 guide PDFs (1601-FQ, 1601EQ, 1701MS, 2550Q) and the 1701/1702-MX attachments plus the 936×612 landscape 1701 Conso — convert, defer, or exclude from the 35-form goal?
7. **Font bundle addition:** a metric-compatible serif (e.g. Tinos) for instructions pages — bundling and licensing sign-off.
8. **Paper naming unification:** pick canonical names for 612×792 / 612×936 / 612×1008 across renderer CSS, Rust, and specs (currently three conflicting vocabularies).
9. **Cell-graph builder spike:** approve it as the next spike (merged/spanned cells, per-band column systems) before Wave 3 commits to generated table skeletons — it is the one unproven link in the extraction pipeline.
10. **Wave ordering by business priority:** the waves above are ordered by engineering risk; if specific forms matter to users sooner (e.g. 2551M/2550M monthly filers), reorder Waves 1–2 accordingly.
11. **XML-less forms policy:** confirm that render-converting the 23 sample-less forms ahead of tax-behaviour evidence is acceptable (they remain `ScaffoldOnly`, `release_ready: false` throughout), or whether sample acquisition should gate their waves.
