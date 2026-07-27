# AGENTS.md — bir (HTML-only eBIRForms migration)

Authoritative checkout: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir` on branch `main`. This is the only worktree and the only branch. The former `bir-print-parity` worktree and the 49 auxiliary branches were removed on 2026-07-28; every branch was first pushed to `codeitlikemiley/ebirforms` at its exact commit and tagged `rescue/<branch>`, and that remote was then detached from this clone (re-add it to restore anything). `public` -> `hexuria/buwiz-forms` is the working remote and the only CI that runs. On 2026-07-28 `main` was rewritten to strip `Co-Authored-By:` trailers, changing the SHA of the 265 commits from 2026-07-18 onward; any commit hash in an older note must be translated through `docs/commit-sha-remap-20260728.md`, and the pre-rewrite history is kept locally at `refs/backup/pre-trailer-strip-20260728` (never push it). Prefix shell commands with the `rtk` wrapper (`rtk git status`, `rtk cargo test --locked -p bir-print`, `rtk npm run test:forms`).

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
- `npm run audit:forms:migration` needs `-- --require-clean-source` before evidence runs.

## Do not

- Reintroduce Typst, `formtypes/`, legacy viewers, full-page runtime backgrounds, or a packaged Node runtime (`npm run audit:no-legacy` enforces this).
- Run broad `git clean`, `git reset`, or `git checkout --` sweeps. Only one
  worktree and one branch remain, so the old "auxiliary worktrees may hold
  unique commits" hazard is gone, but untracked evidence and scratch trees in
  this checkout are still not disposable — inspect before removing anything.
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

## Validation rules — a separate objective in the same checkout

`rules/`, `crates/bir-rules/`, `crates/bir-rules-codegen/`,
`crates/bir-core/src/form_rules/` and
`crates/bir-desktop/src/components/form_validation/` are the official-eBIRForms
**validation-rules** subsystem. It is unrelated to the print migration above and
has its own objective, plan and gates. Do not apply print-parity reasoning to it.

- The live objective is **library-first**: portable evidence packets followed
  by strict v2 candidates for all 43 forms in the exact order in
  `docs/validation-rules/execution-plan.md`. Objective:
  `.claude/GOAL.md`. Historical promotion analysis:
  `docs/validation-rules/action-plan.md`. Prior session: `handoff.md`.
- Rebaseline at local/origin `0cde6a1`: v1 **43 forms / 9,592 fields / 2,007
  validations / 623 calculations**; v2 **1 candidate / 27 executable
  validations / 1 executable calculation**. The 2550Q 53-projection
  classification is complete, but that is not a form-completion or promotion
  claim.
- The single machine-checkable condition is
  `rtk cargo run -q --locked -p bir-rules-codegen -- status`. It separates
  **Boundary** criteria (a production filing path must stay closed),
  **ActiveLibrary** criteria (the default library completion gate), and
  **DeferredPromotion** criteria (reported but required only by
  `status --require-promotion`). Default success does not prove promotion
  readiness. A boundary failure is far more serious than unfinished library
  work.
- Corpus audit: `rtk cargo run --locked -p bir-rules-codegen -- validate-v1`.
  Baseline: 43 forms, 659 JSON (139 v2), 9,592 fields, 2,007 validations, 623
  calculations, 1,354 negative fixtures, 216 schema documents. If any count
  moves, the change is wrong — never adjust the validator to match.
- Library coverage is reported by `coverage --json`, `operator-census --json`,
  and `reconciliation --json`. The 43-candidate gate requires zero
  unclassified and zero unresolved **legacy records**: every record is
  represented by v2 or source-backed and classified intentionally non-runtime
  under the closed reason enum. Filing-safe profile branches may remain
  unresolved as deferred promotion work; these are different states.
- Until the 43-form candidate library baseline exists, freeze `bir-core`,
  `bir-desktop`, GPUI, persistence, Final Copy, filing, queue/transport,
  registry population, and promotion work. Create no auxiliary worktrees and
  do not move validation work into an existing one. The worktree cleanup this
  bullet anticipated was explicitly authorized and completed on macOS on
  2026-07-28, ahead of the library baseline: one worktree (`bir`) and one branch
  (`main`) now remain, with every removed branch backed up and tagged first.

**Only 2550Q has a v2 rule set, it is `candidate`, and it is test-only. It must
never be promoted.** Five guards keep production closed, each one edit away from
opening a filing path:

- `crates/bir-core/src/form_rules/form_2550q.rs:42-44` — returns `None`
- `crates/bir-rules/src/generated/mod.rs:4` — `#[cfg(test)]` candidate module
- `crates/bir-core/src/form_rules/form_2550q.rs:930` — `#[cfg(test)]` evaluator
- `crates/bir-rules/src/generated/registry.rs:18-19` — empty reviewed registry
- `crates/bir-core/src/form_rules/payload.rs:228-234` — always returns `Err`

Also non-negotiable: filing-safe stays `unresolved`; all three serialization
artifacts stay `documented_only`, node-less and `values_emitted: false`;
preserve official defects in the official profile rather than silently fixing
them; a source-set digest roll is a 123-file atomic transaction that must fail
rather than be partially applied; never use the official submission path for
discovery and never use real taxpayer data.

`rules/tools/*.ps1` are mostly **provenance records, not tools** — their inputs
(extracted installer, `.hta`, `atcCodes.xml`, savefile XML) are not tracked, so
they cannot run from a clone on any OS. Read `rules/tools/README.md` before
invoking any of them.

## Deep guidance

- **Conversion strategy (start here for any new form): `docs/form-print-readiness/conversion-strategy-v2.md`** — structure-first calibration, generated geometry contracts, text via content assertions (never text-pixel chasing: all 35 source PDFs have substituted fonts, so text pixels are unwinnable by proof), dynamic-behaviour pattern library, and the 35-form wave plan.
- **Recurring styling patterns (read before building OR reviewing any form): `docs/form-print-readiness/custom_form_styling.md`** — every defect found by eye during review, tagged by provenance, so a new form starts correct: uniform-1px borders (`border-thickness-decision.md`), the `> span` → money-grid-collapse specificity trap, comb `:not(:last-child)` dividers, empty-cell guides, grey-band vs white-knockout, pre-printed constants, and a per-form checklist.
- Release-criterion design and the six-component table: `docs/form-print-readiness/official-fidelity-criterion-v1.md` — implemented and running in reporting mode, not gating, baselines not pinned (see the status statement above).
- Process playbooks (follow them, as amended by the strategy above): `.codex/skills/ebirforms-convert-form-to-html/` and `.codex/skills/ebirforms-print-preview/`.
- Honest per-form readiness numbers: `docs/form-print-readiness/priority-forms-readiness.md`.
- `AGENT.md` covers only the legacy GPUI native-UI layer, not this migration.
