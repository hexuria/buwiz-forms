# Release-Evidence Runbook (2551Q:2018 first)

This is the single end-to-end procedure for taking one exact form revision
from "renderer exists" to "release-ready with audited evidence". It reflects
the audit contract in `scripts/audit_html_form_migration.py`: every gate needs
a hashed, independently recomputable report from a **registered trusted
producer**, bound to one clean curated source revision. The producer
registries ship empty on purpose; nothing in this runbook weakens that.

## Ground rules

- Evidence lands in a dedicated **evidence-only commit** (reports, artifacts,
  pointers, capability flips) so the curated source revision it binds is the
  parent commit and stays clean.
- A failing gate writes no evidence. There is no partial credit and no
  "diagnostic promoted later".
- Producer registration (adding an id to `TRUSTED_VISUAL_EVIDENCE_PRODUCERS`,
  `TRUSTED_PLATFORM_EVIDENCE_PRODUCERS`, or
  `TRUSTED_ROLLBACK_EVIDENCE_PRODUCERS`) happens only after the user reviews
  that producer's code and one full run of its output. Registration is its own
  reviewed source commit, never bundled into feature work.

## 0. Preconditions

```sh
rtk git status --short           # nothing dirty under curated paths
rtk npm run audit:forms:migration -- --require-clean-source
rtk npm run audit:no-legacy
```

Untracked diagnostics under `tmp/` and `test-results/` are outside the
curated paths and do not block; anything under `packages/`, `crates/`,
`scripts/`, or the workflow/build manifests does.

## 1. Visual parity evidence

1. Close the gate first: `rtk npm run test:forms:visual` must pass at the
   strict ≤1% complete-page threshold against the pinned chromium raster.
   Until it does, iterate with the diagnostics:
   - `rtk npm run report:visual:regions` — browserless region re-rank of the
     last captured run (worst regions first).
   - `rtk npm run diagnose:fonts` — font-stack attribution against both
     rasters.
2. On a clean tree, run the producer:

   ```sh
   node scripts/run_python.mjs scripts/produce_visual_evidence.py --form 2551Q:2018
   ```

   It reruns the suite, requires `passed: true`, copies hashed artifacts into
   `evidence/visual/2551q-2018/`, self-verifies the report through the
   audit's validator, and prints the `visual_parity` pointer. The expected
   final state today is the explicit refusal
   `no trusted attested visual evidence producer is registered`.
3. Review `scripts/produce_visual_evidence.py`, the parity spec, and one full
   report. If accepted, register `playwright-form-parity-v1` in
   `TRUSTED_VISUAL_EVIDENCE_PRODUCERS` in a reviewed source commit.
4. Land the report + artifacts + pointer in the evidence-only commit.

## 2. Native print/export and packaged-offline evidence (per platform)

Current state, stated honestly:

- Candidate builds come from `.github/workflows/html-candidate-certification.yml`
  (macOS signed+notarized; **Windows unsigned — a signed candidate source is
  still an open gap**; Linux portable tarball).
- The operator collectors (`scripts/{macos,windows,linux}_candidate_collector.py`
  plus their `*_candidate_certification.py` verifiers) exercise real preview,
  system print, PDF export, and network-denied launch on native hardware, but
  they emit the *certification* schema which is schema-locked to
  `promotion_eligible: false`. **A promotion-grade platform producer that
  emits the audit's `native_print_export` / `packaged_offline` report shapes
  does not exist yet** — building it (likely as a thin, reviewable wrapper
  over the collector output plus the audit's field contract) is the next
  infrastructure task after visual parity closes.
- The same applies to the 11-case rollback drill (`rollback_drill`): the
  bundle format is documented in `macos-candidate-certification.md`, but no
  promotion-grade rollback producer exists.

Per platform, once a producer exists and is reviewed/registered:

1. Dispatch the candidate workflow at the pinned revision; download the
   artifact.
2. Operator runs the collector on native hardware (real printer; macOS needs
   `--allow-live-print`), then the certification verifier.
3. The platform producer converts the verified run into the audit's report
   shape, self-verifies, and prints pointers for
   `native_print_export.<platform>` and `packaged_offline.<platform>`.

## 3. Promotion

Only after all pointers exist, in the evidence-only commit:

1. Fill the slots in `packages/form-specs/form-release-evidence.json`.
2. Flip the earned capabilities and `release_ready` in
   `packages/form-specs/form-migration-status.json` **and** the generated
   Rust capability manifest via its generator.
3. Verify:

   ```sh
   rtk npm run audit:forms:migration -- --require-release-ready 2551Q:2018
   ```

   This recomputes every visual number from the hashed artifacts, revalidates
   every platform/rollback report, and only then accepts `release_ready`.
4. `release.yml` runs the same command in preflight; a tag build stays blocked
   until this passes.

## Sequence for the remaining forms

Repeat per form in the reviewed order (1601C → 0619E → 0619F → 0605 → 1701Q →
2550Q → 1701 → 1702RT → 1702MX), adding the chromium reference + noise floor
for each via `scripts/prepare_chromium_reference.mjs` and the Rust pins before
starting its visual work. Queue/fileability evidence stays Rust-owned and
independent of this visual chain.
