# Priority HTML Form Readiness

Last verified: **July 16, 2026**.

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
| `1701Q:2018` | Blocked | `experimental` | Locked 2018 PDF; no reviewed saved XML | Exact-revision XML proof, queue, parity, native, and packaged-offline evidence |
| `2550Q:2024` | Blocked | `experimental` | PDF + guide + XML | Queue, calibrated parity, native, and packaged-offline evidence; official paper is 612 by 1008 points |
| `1701:2018` | Blocked | `experimental` | Main PDF + conditional attachments + XML | Queue, conditional-page parity, native, and packaged-offline evidence |
| `1702RT:2018C` | Blocked | `experimental` | Four-page PDF + XML | Queue and full-page parity; native and packaged-offline evidence |
| `1702MX:2018C` | Blocked | `experimental` | Main/attachment PDFs + XML | Queue and full-page parity; native and packaged-offline evidence |

The current complete-page comparisons are all above the strict 1% release
threshold. Structural line-only diagnostics are not a substitute for that
gate. In particular, `1702RT:2018C` currently differs by approximately 13.30%,
10.68%, 14.01%, and 21.94% across its four physical pages.

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
