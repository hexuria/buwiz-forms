# Priority HTML Form Readiness

The machine-readable authority is
`packages/form-specs/form-migration-status.json`, cross-checked against the Rust
capability registry. This page records source-pack constraints; it does not
promote a form by itself.

| Exact target | Reviewed local source | Current HTML status | Blocking evidence |
| --- | --- | --- | --- |
| `2551Qv2018` | PDF + XML | HTML provider and visual reference | Native packaged-output evidence before final release promotion |
| `1601Cv2018` | PDF + plain/encrypted XML | Missing renderer | Complete exact-revision contract and HTML/native gates |
| `0619Ev2018` | PDF + plain/encrypted XML | Missing renderer | Formula evidence and full typed behavior |
| `0619Fv2018` | PDF + plain/encrypted XML | Missing renderer | Formula evidence and full typed behavior |
| `0605v1999` | PDF + XML | Missing renderer | Use revision `1999`; the former `2018` target was incorrect |
| `1701Qv2018` | Locked 2018 PDF only | Missing renderer | No saved XML; the discovered eFPS template ends at legacy Items 26–41 and is not evidence for the locked 2018 Items 36–68 layout; current field surface remains a conservative scaffold |
| `2550Qv2024` | PDF + guide + XML | Missing renderer | Complete contract; official paper is 612 by 1008 points |
| `1701v2018` | Main PDF + conditional attachments + XML | Missing renderer | Prove conditional page families and large field surface |
| `1702RTv2018C` | PDF + XML | Missing renderer | Formula and schedule evidence |
| `1702MXv2018C` | Main/attachment PDFs + XML | Missing renderer | Encrypted payload exposes 378 fields absent from the plain sample |

Forms remain `ScaffoldOnly` or manual/external until typed behavior, formulas,
XML, persistence, queue behavior, HTML parity, native output, and packaged
offline gates are all proven. The Forms Set may recommend an unconverted form,
but the app must not pretend it can render or file it.
