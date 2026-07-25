# AGENTS.md — bir-print-parity (HTML-only eBIRForms migration)

Authoritative checkout: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity` on branch `codex/print-preview-parity`. Do not work in the sibling `bir/` checkout. Prefix shell commands with the `rtk` wrapper (`rtk git status`, `rtk cargo test --locked -p bir-print`, `rtk npm run test:forms`).

## The visual criterion (read before touching any form renderer)

**The ≤1% complete-page gate is unreachable, and this is proven, not an excuse.**
`pdffonts` shows every one of the 35 source PDFs carries `emb=no` for its
primary faces, so the pinned references encode Poppler's *substituted* glyph
outlines rather than BIR's typography. Glyph outline shape is ~57% of the
residual. Every rendering-side lever was tested and refuted: real platform
Arial scores *worse* than bundled Arimo, path-filling our own text through the
reference pipeline scores worse, and 19 weight variants, 26 size variants and
every CSS text-rendering knob failed. Even a *perfect* ATC table leaves page 2
above 1%. See `docs/form-print-readiness/priority-forms-readiness.md`.

**Do not spend effort on text pixels. Ever.** That is where the previous 273
commits went. Text correctness is proven by content assertions instead.

The replacement criterion is `official-fidelity-v1`
(`docs/form-print-readiness/official-fidelity-criterion-v1.md`), a **composite
of six bound components**, currently **implemented and running in reporting
mode but not gating**:

| Component | Binds |
| --- | --- |
| `cell-edge-f1-v1` | displacement, per scoring cell, tolerance radius **1** |
| `structural-ink-coverage-v1` | rules, boxes, fills — font-independent by construction |
| `page-ink-budget-v1` | ink volume and exactly-white paper pixels (the tint attack) |
| `static-text-exhaustive-v1` | every printed string, ordered — **the only thing standing between us and a wrong tax rate** |
| `encoded-artwork-integrity-v1` | payload + raster crop hash + transform |
| `official-complete-page-v2` | retained, **mandatory, never hidden**, no longer gating |

Rules that are not negotiable:

- **It is a NON-REGRESSION criterion.** It certifies "no worse than a reviewed
  baseline". It can never certify "matches the official form". Reports must
  carry `proves_parity: false` and `is_non_regression_gate: true`; the audit
  rejects them otherwise.
- **The complete-page percentage stays computed and reported always**, next to
  the raw Poppler diagnostic and the pinned noise floor. Never report a masked,
  structure-only, or text-excluded number as parity.
- **Never weaken the numeric 1% threshold**, any component threshold, or any
  assertion, no matter how close a form gets.
- **A missing component is an error, not a pass.** A partial criterion promotes
  on the components it includes while the omitted one is the one that would
  have failed.
- **Baselines are not pinned yet**, so nothing gates. Reporting mode must never
  become a promotion shortcut.

### Calibrate structure, not text

Structure *is* winnable and is where effort belongs. Working loop:

```sh
rtk npx playwright test --config packages/form-renderer/playwright.structural-defects.config.ts   # where and why
rtk npx playwright test --config packages/form-renderer/playwright.comb-capacity.config.ts        # capacity vs official
rtk npx playwright test --config packages/form-renderer/playwright.fidelity-baseline.config.ts    # component values
rtk npx playwright test --config packages/form-renderer/playwright.fidelity-injection.config.ts   # the criterion's own regression test
```

**Always check both rasterizers.** A change that improves the chromium gate
while worsening the Poppler diagnostic is overfitting to one reference, not
convergence. Every real fix this session moved both the same way.

**Read tone profiles, not thresholded pixel counts.** Hard thresholds
misreported sub-pixel geometry three separate times here: they turned a correct
3-device-px stroke into an apparent 4, gave the defect localizer phantom
clusters, and would have hidden every mid-grey comb guide (official guides
measure tone 83–153 because sub-pixel black ink cannot fill a pixel).

**Distinguish weight from displacement.** An offset search reports a confident
"displaced by 2px, recovers 100%" for a stroke that is merely too thin, because
shifting it does land on ink. Acting on that moves correctly-placed rules. The
localizer now reports both stroke thicknesses; use them.

**Never change a border from raster evidence alone — check the stroke's grey
value in the geometry contract first.** The raster localizer compares ink
*presence* and cannot tell a light-grey decorative fill from a black rule. Many
official "rules" are `gray = 0.8509` (tone ~217, near-invisible on paper); where
we correctly paint nothing, the sweep reports them as missing black structure.
A contract-first re-derivation refuted seven such findings across 1701, 1601C
and 1701Q — acting on them would have painted black over grey decoration while
*improving* structural recall, because the metric only asks whether ink exists.
Width tells you how thick a stroke is; only grey tells you whether it is there.
The weight columns in `wave0-diagnostic-review.md` are contaminated this way and
are superseded by the per-form contract-derived lists.

## Reference pipeline

- Poppler raster (primary provenance): `.codex/skills/ebirforms-convert-form-to-html/scripts/prepare_official_reference.py` — pinned official PDF → 144 DPI PNG.
- Chromium raster (gate reference): `node scripts/prepare_chromium_reference.mjs --repo . --form-code <CODE> --revision <REV> --pdf <pinned pdf> --expected-sha256 <pin>` — pinned PDF → `pdftocairo` vector SVG (glyphs as paths) → rasterized by the same Chromium/Playwright environment as the parity screenshots, with the per-page noise floor recorded.
- All reference hashes are pinned in Rust (`crates/bir-print/src/html_forms/form_*.rs`). Regenerate the manifest with `npm run references:generate`; `cargo test -p bir-print` byte-locks it.
- Official page rasters are calibration-only. They must never become runtime assets or page backgrounds.

## Evidence and promotion (fail closed)

- `packages/form-specs/form-migration-status.json` and `form-release-evidence.json` are conservative truth. The trusted-producer registries in `scripts/audit_html_form_migration.py` are intentionally **empty frozensets**; a producer is registered only after the user reviews it. Hand-authored reports and untracked artifacts (`tmp/`, `test-results/`) can never promote a form.
- Never set capability flags or `release_ready` to make an audit pass. `html_only` routing is not a release claim; forms stay `ScaffoldOnly` with `release_ready: false` until the complete evidence chain exists (visual parity, native print/export per platform, packaged-offline, rollback drill).
- Promotion evidence lands in a dedicated evidence-only commit binding a clean curated source revision.

## Do not

- Reintroduce Typst, `formtypes/`, legacy viewers, full-page runtime backgrounds, or a packaged Node runtime (`npm run audit:no-legacy` enforces this).
- Run broad `git clean`, `git reset`, or worktree pruning — many auxiliary worktrees exist and may hold unique commits.
- Delete or commit `tmp/` or `test-results/` contents without inspecting them first.
- Touch the live encrypted database at `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db` — schema repairs run in-app at startup.
- Mutate confirmed COR facts directly; the replacement-version + review flow is the designed audit trail.
- Spawn subagents with full conversation history; use compact prompts with exact file paths.

## Key commands

```sh
rtk npm run audit:forms:migration          # migration/evidence audit (add -- --require-clean-source before evidence runs)
rtk npm run audit:no-legacy                # legacy-absence audit
rtk npm run contracts:check                # generated contracts match tracked files
rtk npm run typecheck:forms && rtk npm run test:forms
rtk npm run test:forms:visual              # the parity gate; artifacts land in test-results/form-renderer/
rtk npm run references:generate            # rebuild references/manifest.json from Rust pins
rtk cargo test --locked -p bir-print
```

## Deep guidance

- **Conversion strategy (start here for any new form): `docs/form-print-readiness/conversion-strategy-v2.md`** — structure-first calibration, generated geometry contracts, text via content assertions (never text-pixel chasing: all 35 source PDFs have substituted fonts, so text pixels are unwinnable by proof), dynamic-behaviour pattern library, and the 35-form wave plan.
- **Recurring styling patterns (read before building OR reviewing any form): `docs/form-print-readiness/custom_form_styling.md`** — every defect found by eye during review, tagged by provenance, so a new form starts correct: uniform-1px borders (`border-thickness-decision.md`), the `> span` → money-grid-collapse specificity trap, comb `:not(:last-child)` dividers, empty-cell guides, grey-band vs white-knockout, pre-printed constants, and a per-form checklist.
- Release-criterion design: `docs/form-print-readiness/official-fidelity-criterion-v1.md` (specified, not yet implemented or promotable).
- Process playbooks (follow them, as amended by the strategy above): `.codex/skills/ebirforms-convert-form-to-html/` and `.codex/skills/ebirforms-print-preview/`.
- Honest per-form readiness numbers: `docs/form-print-readiness/priority-forms-readiness.md`.
- `AGENT.md` covers only the legacy GPUI native-UI layer, not this migration.
