# Priority HTML Form Readiness

Last verified: **July 19, 2026**.

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
| `1601C:2018` | Proven | `experimental` | PDF + plain/encrypted XML | Full-page visual parity, native preview/print/PDF export, and packaged-offline evidence |
| `0619E:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0619F:2018` | Blocked | `experimental` | PDF + plain/encrypted XML | Queue, calibrated parity, native, and packaged-offline evidence |
| `0605:1999` | Blocked | `experimental` | PDF + XML | Queue, calibrated parity, native, and packaged-offline evidence; exact revision is 1999 |
| `1701Q:2018` | Blocked | `experimental` | Locked 2018 PDF + hash-locked 7.9.5.0 HTA-backed exact editable XML contract | Reviewed outbound helpers/credential handling, queue/submission, parity, native, and packaged-offline evidence |
| `2550Q:2024` | Blocked | `experimental` | PDF + guide + XML | Queue, calibrated parity, native, and packaged-offline evidence; official paper is 612 by 1008 points |
| `1701:2018` | Blocked | `experimental` | Main PDF + conditional attachments + XML | Queue, conditional-page parity, native, and packaged-offline evidence |
| `1702RT:2018C` | Blocked | `experimental` | Four-page PDF + XML | Queue and full-page parity; native and packaged-offline evidence |
| `1702MX:2018C` | Blocked | `experimental` | Main/attachment PDFs + XML | Queue and full-page parity; native and packaged-offline evidence |

In the authoritative migration manifest, `1601C:2018` has
`capabilities.queue_submission: true`, while `1701Q:2018` has
`capabilities.xml_round_trip: true` and `capabilities.queue_submission: false`.
Those independent gates must not be collapsed into one filing-readiness claim.

The current complete-page comparisons are all above the strict 1% release
threshold. Structural line-only diagnostics are not a substitute for that
gate. The current honest raw 2551Q results are approximately 6.76% and 4.83%;
its page-indexed static-copy checks pass and its non-promoting two-pixel edge
F1 scores are approximately 0.964 and 0.991. `0605:1999` remains blocked at
approximately 7.44% on page 1 and 10.23% on page 2 even though its
structure-only diagnostics are approximately 0.26% and 0.20%.

The latest exact complete-page and structural diagnostics for the recently
calibrated forms are:

| Exact target | Complete-page difference by page | Structural difference by page |
| --- | --- | --- |
| `0605:1999` | 7.439441%, 10.226733% | 0.257798%, 0.200626% |
| `0619E:2018` | 8.671426% | 0.175107% |
| `0619F:2018` | 8.377329% | 0.331749% |
| `1701Q:2018` | 13.479781%, 9.589548% | 0.328150%, 0.095185% |
| `1701:2018` | 12.599593%, 15.788224%, 12.253942%, 15.302349% | 0.416833%, 0.446597%, 0.243440%, 0.551471% |
| `1702RT:2018C` | 12.146711%, 8.016661%, 7.996585%, 11.390949% | 0.520877%, 0.148516%, 0.169116%, 0.875999% |
| `1702MX:2018C` | 13.217793%, 19.179227%, 11.552995%, 9.224345% | 0.385322%, 0.819307%, 0.131147%, 0.259195% |

These results retain the reviewed source corrections for 0619E checkbox
interiors, 0619F Part II row heights, 1701Q and 1701 typography, the 1702RT
Schedule IIIA rounding note, and the 1702MX Item 5 official plain
code/description boxes. Geometry, overflow, capacity, reviewed-copy, and
critical-region checks may pass while these raw comparisons remain blocked;
those narrower checks do not establish visual parity.

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
