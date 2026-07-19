# CLAUDE.md — bir-print-parity (HTML-only eBIRForms migration)

Authoritative checkout: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity` on branch `codex/print-preview-parity`. Do not work in the sibling `bir/` checkout. Prefix shell commands with the `rtk` wrapper (`rtk git status`, `rtk cargo test --locked -p bir-print`, `rtk npm run test:forms`).

## The visual parity gate (read before touching any form renderer)

- The release gate is a **complete-page pixel difference ≤ 1%** per official page at 144 DPI (pixelmatch threshold 0.1, `FORM_VISUAL_MAX_CHANGED_PERCENT=1`). It is enforced by `packages/form-renderer/visual/form-parity.spec.ts` and re-verified independently by `scripts/audit_html_form_migration.py`.
- For chromium-equipped forms (currently `2551Q:2018`) the gate compares against the **same-rasterizer chromium reference** (`references/*-chromium.png`), which removes the ~3.6% Poppler-vs-Chromium rasterizer noise floor that made the old raw comparison unreachable. The raw Poppler-raster diff and the pinned per-page noise floor are **mandatory, clearly-labeled diagnostics**: always report them alongside — never instead of — the gate number.
- **Never report a masked or structure-only percentage as visual parity.** Structure-only line diagnostics (~0.07%) are geometry probes only. Full parity is the complete-page number, nothing else.
- Never weaken the numeric 1% gate, no matter how close a form gets.

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

- Process playbooks (follow them): `.codex/skills/ebirforms-convert-form-to-html/` and `.codex/skills/ebirforms-print-preview/`.
- Honest per-form readiness numbers: `docs/form-print-readiness/priority-forms-readiness.md`.
- `AGENT.md` covers only the legacy GPUI native-UI layer, not this migration.
