# Audit â€” 2550M February 2007 ENCS

- Revision pinned to February 2007 ENCS through runtime header/application identity and the official PDF.
- HTA, help, PDF, plaintext saves, ATC catalog, package executable, and three later BIR circulars are hashed.
- Inventory: 142 observed save keys; 250 bounded concrete keys; 42 unbounded indexed families; 292 unique entries total.
- Runtime controls: 186 static controls, 182 with IDs, four without IDs.
- ATC catalog: 36 revision-applicable records; hard-coded 12% output-tax computation recorded separately.
- Main Validate, Save preflight, all eight schedule modals, calculations, final-copy, and transport code inspected.
- First-error ordering is recorded for main Validate and row validators.
- Dummy-only negative cases bind to rule IDs; no real taxpayer data is stored.
- Confirmed defects include the commented future-period validation, nonblank-only TIN validation, February date bypasses, no-op Schedule 1/4/5 validation, disabled Schedule 7/8 over-application checks, stale Item 23/24 labels, obsolete Schedule 4 behavior, and wrong Income Tax transport labels.
- Later legal behavior is not merged into the legacy revision; compatibility and recommended current behavior are both retained.

Run tk powershell -NoProfile -ExecutionPolicy Bypass -File '\\mac\goldcoders\reverse-engineer-ebir-forms\bir-print-parity\rules\validate.ps1' -RequireJsonSchema after updating the index.