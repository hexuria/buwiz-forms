# 1702 Corporate Form Readiness

Last verified: **July 16, 2026**.

This document covers `1702RT:2018C` and `1702MX:2018C`. The authoritative
capabilities and routes are in
`packages/form-specs/form-migration-status.json`; platform evidence is in
`packages/form-specs/form-release-evidence.json`.

Both exact revisions currently have:

- typed Rust drafts, formula evidence, XML round-trip coverage, persistence,
  and desktop editors;
- Rust `RenderEnvelopeV1` providers and canonical fixtures;
- semantic React/CSS components, exact-revision specifications, and
  deterministic four-page pagination;
- pinned official four-page references in
  `packages/form-renderer/references/manifest.json`.

Both remain `ScaffoldOnly`, use the `experimental` renderer route, have queue
submission disabled, and are not release-ready. A calibration-viewer fixture is
development evidence only.

## Official page facts

| Exact revision | Official PDF SHA-256 | Pages | Geometry |
| --- | --- | ---: | --- |
| `1702RT:2018C` | `d9a6a8a13e0114934261151c4eb269a1573042e7ce670eaf12b15f169d308d2d` | 4 | 612 x 936 pt |
| `1702MX:2018C` | `81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2` | 4 | 612 x 936 pt |

The calibration viewer renders these as four physical pages in one continuous
scroll area. A PDF application may display pages 2 and 3 side by side, but
spread presentation does not combine them into one page and must not change
the renderer page count.

## Current visual truth

Neither form passes the strict complete-page visual gate. The latest local
`1702RT:2018C` diagnostic differs from the official raster by approximately:

| Page | Full-page difference |
| ---: | ---: |
| 1 | 13.30% |
| 2 | 10.68% |
| 3 | 14.01% |
| 4 | 21.94% |

These values are far above the 1% release threshold. Sparse ruled-line or
masked-region scores must not be reported as whole-page parity. The current
1702RT document is an HTML scaffold that needs semantic layout calibration; it
is not an identical reconstruction of the official form.

The 1702MX complete-page comparisons also remain above 1%, so its
`visual_parity` capability stays false.

## Calibration requirements

For both revisions:

1. Compare each physical HTML page with the corresponding pinned official
   raster from the reference manifest.
2. Reconstruct the official header, identity fields, totals, signatures,
   schedules, continuation pages, and attachment sections semantically.
3. Test normal, long-value, validation-edge, and maximum-schedule fixtures.
4. Preserve all valid values; never shorten text to fit a comb.
5. Verify exact page rectangles, no clipping or overflow, and stable geometry
   after fonts load.
6. Keep CSS form-scoped unless a geometry primitive is demonstrably shared.

Start the calibration viewer:

```sh
npm ci
npm run dev:calibration
```

Select a committed 1702 fixture, then use HTML, Overlay, Difference, and
opacity controls. Previous, Next, and page-number controls are scroll
shortcuts; normal scrolling remains available across all four pages.

Run focused and repository checks after calibration:

```sh
npm run typecheck:forms
npm run test:forms
npm run test:forms:visual
npm run audit:forms:migration
```

## Promotion boundary

Do not enable queue submission until the Rust filing capability is independently
reviewed. Do not change `visual_parity` while any official page exceeds the
complete-page threshold or a critical region is wrong. Do not set
`release_ready` until macOS, Windows, and Linux preview, print, direct PDF
export, and packaged-offline evidence is recorded.

Rendering failure must remain visible and retryable. It must not route to a
different document implementation or mutate the draft.
