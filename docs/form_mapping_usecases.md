# eBIRForms Form Mapping Evidence

The coordinate-overlay layout editor and `formtype.json` workflow described by
older revisions of this document have been retired. New printable forms use
human-reviewed semantic HTML/CSS and a typed Rust render contract.

## Evidence sources

Use each source for the facts it can actually prove:

1. The exact official PDF proves revision identity, page geometry, labels,
   visual structure, comb capacity, and pagination.
2. Saved eBIRForms XML and eFPS manifests provide field-name and payload-shape
   evidence. One sample does not prove a formula or optional-field behavior.
3. Existing Rust models, validation, persistence, queue behavior, and tests
   show what the app currently implements; they are not automatically official
   tax evidence.
4. Reviewed COR/profile facts determine suggestions and prefills through the
   effective-dated profile resolver. React never infers them.

## Conversion workflow

Follow `.codex/skills/ebirforms-convert-form-to-html/SKILL.md`:

1. lock the exact form identity and official source hash;
2. audit formulas, XML, validation, persistence, and lifecycle behavior;
3. add a Rust `RenderEnvelopeV1` provider and fixture matrix;
4. build semantic exact-revision React markup and scoped CSS;
5. extract exact embedded seal/logo objects and prove each PDF417/QR payload,
   zero-difference module matrix, vector rendering, and live bundled-font
   caption—or record an audited explicit no-symbol result without fabrication;
6. calibrate against reference-only official-PDF rasters;
7. prove capacity, no truncation, pagination, native preview/print/PDF, and
   packaged-offline operation;
8. promote only after the migration capability gates pass.

OCR or AI may help inventory labels and detect candidate regions, but its
output is non-authoritative. It must never generate tax formulas, submission
semantics, or runtime full-page backgrounds.
