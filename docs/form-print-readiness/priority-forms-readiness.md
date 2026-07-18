# Priority HTML Form Readiness

Last verified: **July 18, 2026**.

The machine-readable authority is
`packages/form-specs/form-migration-status.json`, cross-checked against
`crates/bir-core/src/forms/support_level.rs`. Release evidence lives in
`packages/form-specs/form-release-evidence.json`. This page summarizes those
records and source-pack constraints; it does not promote a form.

Every exact revision below has a Rust render provider, committed fixtures,
semantic React/CSS, a paper specification, deterministic pagination, and
official calibration references. Every form is still `ScaffoldOnly`, every
`visual_parity` capability is false, and no form is `release_ready`.

## Current status

| Exact target | Queue authority | Renderer route | Reviewed source | Primary blockers |
| --- | --- | --- | --- | --- |
| `2551Q:2018` | Proven | `html_only` | PDF + XML | Full-page visual gate and signed native/package evidence |
| `1601C:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, visual, native, and packaged-offline evidence |
| `0619E:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0619F:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0605:1999` | Blocked | `experimental` | PDF + XML | Queue, calibrated parity, native, and packaged-offline evidence; exact revision is 1999 |
| `1701Q:2018` | Blocked | `experimental` | Locked 2018 PDF + hash-locked 7.9.5.0 HTA editable XML source | Runtime XML replay, reviewed outbound helpers/credential handling, queue, parity, native, and packaged-offline evidence |
| `2550Q:2024` | Blocked | `experimental` | PDF + guide + XML | Queue, calibrated parity, native, and packaged-offline evidence; official paper is 612 by 1008 points |
| `1701:2018` | Blocked | `experimental` | Main PDF + conditional attachments + XML | Queue, conditional-page parity, native, and packaged-offline evidence |
| `1702RT:2018C` | Blocked | `experimental` | Four-page PDF + XML | Queue and full-page parity; native and packaged-offline evidence |
| `1702MX:2018C` | Blocked | `experimental` | Main/attachment PDFs + XML | Queue and full-page parity; native and packaged-offline evidence |

The current complete-page comparisons are all above the strict 1% release
threshold. Structural line-only diagnostics are not a substitute for that
gate. The current honest raw 2551Q results are approximately 7.20% and 5.32%;
its page-indexed static-copy checks pass and its non-promoting two-pixel edge
F1 scores are approximately 0.965 and 0.990. `0619E:2018` and `0619F:2018`
remain blocked at approximately 9.37% and 10.35%, respectively. These values
must not be replaced by the much smaller structure-only percentages.
`0605:1999` remains blocked at approximately 7.79% on page 1 and 11.04% on
page 2 even though its structure-only diagnostics are approximately 0.26% and
0.20%. `1701Q:2018` remains blocked at approximately 15.79% on page 1 and
10.04% on page 2; its structure-only diagnostics are approximately 0.35% and
0.12%. The latter two forms pass their page geometry, overflow, capacity, and
reviewed-copy checks, but those narrower checks do not establish visual parity.

A July 18 cross-rasterizer diagnostic also rendered the first official 2551Q
page itself through Poppler and through Chromium from Poppler's vector SVG;
those two official-source rasters differed by approximately 3.61% under the
same pixel comparison. That diagnostic is not promotion evidence and does not
relax the 1% gate. It records why font/rasterizer work must be evaluated with
the raw result, exact static copy, critical geometry, and edge evidence shown
separately rather than reporting one masked percentage as overall parity.

## Status meanings

- `ScaffoldOnly` means some in-app implementation exists but the exact revision
  has not satisfied the production capability gate.
- `experimental` permits development calibration and testing only.
- `html_only` means the exact revision uses the semantic HTML document path and
  has no alternate renderer. It is not a release claim.
- Queue authority is controlled independently by Rust-owned filing evidence.
- `release_ready` requires queue, visual, native output, and packaged-offline
  capabilities plus reviewed evidence.

The calibration viewer intentionally lists committed fixtures for both
HTML-routed and scaffold-only forms. Its labels expose those statuses rather
than implying that every visible form can be filed or shipped.

## Promotion gates

Before changing queue authority, verify the typed model, exact XML round trip,
formula and validation evidence, persistence, carry-over, amended-return
behavior, queue adapter, and desktop editor.

Before changing `visual_parity`, verify full-page comparisons against the
pinned exact-revision official PDF at the manifest DPI, exact page geometry,
critical regions, long values, maximum schedules, and zero clipping.

Before setting `release_ready`, record successful preview, system print, direct
PDF export, and packaged-offline operation on macOS, Windows, and Linux using
the same immutable semantic document.

Run the audit after every capability or route change:

```sh
npm run audit:forms:migration
```
