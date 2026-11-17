# Audit

- January 2020 runtime binding: pass.
- Revision discrimination: pass; April 2014 sample and August 2022 PDF are excluded from January 2020 field/rule derivation.
- Typed inventory: 277 static controls + 1 runtime RDO = 278; no runtime families.
- Validations: 39; calculations: 9; negatives: 28; confirmed official defects: 10.
- Focused JSON Schema audit: pass (5 schema documents).
- Full strict audit (`rules/validate.ps1 -RequireJsonSchema`): pass.
- Full-audit corpus: 37 forms, 443 JSON files, 8,842 fields, 1,745 validations, 566 calculations, 1,142 negative fixtures, and 186 schema documents.
- No renderer/release/capability/commit/push changes.
