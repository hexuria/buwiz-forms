# HTML Form Migration Status

The application does not currently have ten production-ready forms. Exact form
support is derived from `crates/bir-core/src/forms/support_level.rs` and
`packages/form-specs/form-migration-status.json`; documentation must not promote
a scaffold to `ImplementedInApp`.

The current work converts ten existing in-app form views to semantic HTML in
this order: 2551Q, 1601C, 0619E, 0619F, 0605, 1701Q, 2550Q, 1701, 1702RT, and
1702MX. Forms without sufficient XML or formula evidence stay manual/external.

Use only the canonical repository skill:

`/.codex/skills/ebirforms-convert-form-to-html/SKILL.md`

The reviewed local source pack is `/Users/uriah/Downloads/forms`. Production
builds never read it; pinned identities and hashes live in
`packages/form-renderer/references/source-catalog.json`.

See [the evidence backlog](docs/form_scaffold/index.md) for exact source state
and blockers. Do not use a form-generator, formtype, coordinate-overlay,
full-page background, Typst, or generated-tax-behavior workflow.
