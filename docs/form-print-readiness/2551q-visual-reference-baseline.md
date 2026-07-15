# 2551Q Visual Reference Baseline

This baseline covers only BIR Form 2551Q, January 2018 (ENCS). The reference
PNGs are development-time calibration evidence. They are not runtime page
backgrounds and must not be bundled or loaded by the HTML renderer.

## Reproducible commands

Generate the canonical Rust contracts and the two legacy Typst calibration
pages:

```sh
rtk cargo run -q -p bir-print --bin generate_render_contract -- --visual-references
```

Verify the committed inputs, dimensions, page count, PNGs, and deterministic
manifest without rewriting them:

```sh
rtk cargo run -q -p bir-print --bin generate_render_contract -- --check-visual-references
```

The check must report:

```text
verified 2551Q:2018 visual references: 2 pages, 612x936pt, 1224x1872px at 144 DPI
```

The generator stops on source or PNG hash drift. A rejected regenerated PNG is
preserved under `.scratch/visual-reference-drift/2551q-2018/` for review; it is
never accepted automatically.

## Pinned provenance

| Artifact | SHA-256 |
| --- | --- |
| Official source PDF (metadata provenance; PDF is not stored in the repository) | `1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24` |
| `formtypes/2551Qv2018/metadata.json` | `c71547d2ff917a30480c75dc76de2461ca7b51c22c3a08381cd06b316822eadb` |
| `formtypes/2551Qv2018/formtype.json` | `1e6beb14024a1335c22d8e0506af8dbb0b56084d8c877c7bf1e2e4efa35728f5` |
| `formtypes/2551Qv2018/template.typ` | `a9f30daa3e328d1b5828ace4a37e26d73f9303c7ddc18273d378348f4297864d` |
| `formtypes/2551Qv2018/pages/page1.svg` | `e62c392a3962ba4c2c31ffcb4b77a7798140473a2af99abf95173680536db599` |
| `formtypes/2551Qv2018/pages/page2.svg` | `377ec4cee07cbff674686926aa0d402ec068b9a70fe3e8dbfc9802e90902f47a` |
| `packages/form-contracts/fixtures/2551q-6-rows.json` | `f3d49ddab5cdd7c1d889a7b2cbd519babf7556c186702f0232b9f18257f7a5b7` |
| `packages/form-contracts/src/generated/2551q-atc-reference.json` | `ccc2056a6151ae8be17f4af2d2718d93c8c7065afe65c9156c9ffef75338ff3f` |
| `packages/form-renderer/references/2551q-2018-page-1.png` | `c78f0724e2f320f1b306408008e9085ed36397c4e1add66bf5e77c322a3485ea` |
| `packages/form-renderer/references/2551q-2018-page-2.png` | `d6ab5afbf6b3f4cbac7c69a01df231eaf6dcf7fde587e78c02ee20e3f2508d1a` |
| `packages/form-renderer/references/manifest.json` | `b88e39b5ee2c7cedd27a87dcd2645b08e02fb6813aa4e6f2e1c713cc8d7e8baf` |
| Discrete government-seal crop | `d532deb6eff07393f0dd2360526805bbcf2680c727baedbb4510ed63c58fb3f4` |
| Discrete page 1 barcode crop | `ddb2025f8630575db15d43b855511f50821b25bc5fa05f767b2439bd9bc45279` |
| Discrete page 2 barcode crop | `dd1e8dd49782640a51fb7a69bae6662ca595f73384c371d9344a1448e3530e77` |

Official source URL:

```text
https://bir-cdn.bir.gov.ph/local/pdf/2551Q%20Jan%202018%20ENCS%20final%20rev%203_copy.pdf
```

The source PDF hash above is the pinned extraction provenance recorded in
`metadata.json`; this command does not download the PDF. The checker directly
hashes the locally owned SVG pages, Typst overlay, form layout, fixture, and
reference PNGs.

## Discrete runtime artwork provenance

The semantic renderer embeds three reviewed, official-reference-derived PNGs:
the Page 1 government seal and a distinct static form barcode for each official
page. The manifest records each source page, pixel crop, image treatment, and
decoded SHA-256. The offline verifier accepts only those three embedded hashes
and rejects unknown data images and every standalone runtime raster file. The
full official pages remain calibration-only and are never used as runtime
backgrounds.

## Legacy row-coverage correction

The clean main layout defined dynamic anchors only for Schedule 1 row 1 even
though Rust exported six rows. Regenerating page 2 therefore omitted rows 2–6.
The resulting difference was 6,713 of 2,291,328 pixels (0.292974%), localized
entirely to those five rows.

The rebuild restores the donor branch's data-only row 2–6 anchors. The final
`formtype.json` is byte-identical to donor Git blob
`64a26514336fc2952a15a70424a37a62da413d37`. No SVG or Typst artwork changed.
After that correction, both generated PNGs reproduce their pinned hashes
exactly.

The ATC reconciliation then replaced invented fixture codes `PT011` through
`PT019` with the exact 22-entry January 2018 Rust registry. The contract
generator emits the ordered codes, descriptions, and rates into the generated
JSON artifact above; React imports that artifact instead of maintaining a
second tax-data table. The preserved tax-due series keeps page 1 byte-identical;
the changed Schedule 1 values intentionally produce the newly pinned page 2
hash above. The official source SVG hashes did not change.

## Local generator observation

The verified local toolchain used Typst `0.13.1` on macOS arm64. Generator wall
time is diagnostic only and is not a promotion gate; page bytes, hashes,
dimensions, and count are the deterministic gate.

## Adaptive long-text treatment

Taxpayer name, registered address, contact number, email address, and Item 12A
retain their official character combs while the value fits. A longer legal
value replaces the comb dividers with one non-truncating plain-text box sized to
the same official field rectangle; Rust retains larger defensive rendering
limits to prevent unbounded document content. TIN, RDO, ZIP, and monetary combs
remain exact fixed-capacity fields.

This adaptive treatment belongs to the semantic HTML renderer. The default
legacy snapshot renderer cannot reproduce it, so its public Rust entry point and
desktop preview preflight reject any draft that it would truncate or omit. The
guard includes the tighter repeated taxpayer-name field on Page 2 and non-empty
Item 12A/Item 17 specification text. No legacy PDF is emitted with partial form
data.

## Final integrated semantic-renderer diagnostic

The integrated owned layout was captured with Chromium on macOS arm64 at the
pinned 144 DPI dimensions. The relaxed run passed 5/5 and exists only to
collect all geometry and page metrics; the strict run uses the required 1%
ceiling and remains nonzero.

| Page | Changed pixels | Changed percent | Strict result |
| --- | ---: | ---: | --- |
| 1 | 270,714 of 2,291,328 | 11.814720546338194% | Fail |
| 2 | 220,932 of 2,291,328 | 9.642094017094017% | Fail |

Commands:

```sh
rtk env FORM_VISUAL_MAX_CHANGED_PERCENT=100 npm run test:forms:visual
rtk npm run test:forms:visual
```

The producer refuses a dirty curated source tree by default. During active
development only, add
`FORM_VISUAL_NON_PROMOTING_ALLOW_DIRTY_SOURCE=1`; that mode always records
`passed=false` and `promotion_eligible=false`, regardless of the relaxed pixel
threshold. Clean-source output is still diagnostic: the migration audit keeps
the trusted visual-producer registry empty until CI execution provenance can be
attested. Reporter-shaped JSON and re-encoded reference pixels cannot promote
`visual_parity_complete`.

The second command must remain nonzero until both page percentages are at most
1%. These diagnostic numbers do not populate `form-release-evidence.json` and
do not authorize release routing. The discrete official-source seal and
page-specific barcodes are now present. Typography, spacing, and fine geometry
calibration remain above the strict ceiling; none may be hidden with a full-page
runtime background.
