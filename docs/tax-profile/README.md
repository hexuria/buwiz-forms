# TIN and branch-code knowledge base

This directory is the shared product and regulatory knowledge base for taxpayer identity, BIR registration units, branch codes, facilities, filing scope, and multi-branch returns.

## What belongs here

- [`CONTEXT.md`](CONTEXT.md) defines the vocabulary used by the guide and implementation plan.
- [`TIN_BRANCH_PROFILE_AND_FILING_GUIDE_2026-08-07.md`](TIN_BRANCH_PROFILE_AND_FILING_GUIDE_2026-08-07.md) records the research cutoff, source-backed conclusions, evidence gaps, and the form-family filing-scope catalogue.
- [`TIN_BRANCH_IMPLEMENTATION_PLAN_2026-08-07.md`](TIN_BRANCH_IMPLEMENTATION_PLAN_2026-08-07.md) turns those conclusions into a dependency-ordered migration and resolver plan. It is documentation only; it does not authorize filing or change the current data model.

The guide treats the nine-digit TIN as the taxpayer identity (`000-000-000`). A head office is a registration unit with branch code `00000`; other units retain the BIR-confirmed five-digit code. The displayed filing identifier is the composition `000-000-000-00000`, not a second taxpayer identity. Facility Codes remain separate from branch codes.

## Relationship to this repository

The per-year Forms Set remains the user-owned source of enabled forms. Its precedence and persistence behavior are documented in the checkout's existing `docs/forms-set-refactor-plan.md` working document. That older documentation is intentionally outside this PR because the repository's broad `docs/` ignore rule keeps the existing local documentation set untracked. Tax-type registration evidence and the resolved filing obligation are upstream inputs; they must not silently rewrite a user's manual Forms Set decisions.

When this knowledge is implemented in the Rust/GPUI application, remap the original-app audit references in the implementation plan to this repository's ownership boundaries:

- `crates/bir-core/src/profile.rs` — effective taxpayer/profile ledger;
- `crates/bir-core/src/forms/forms_set.rs` — per-year Forms Set;
- `crates/bir-core/src/forms/registry.rs` — canonical form identity;
- `crates/bir-core/src/integration/validation.rs` — form and obligation resolution;
- `crates/bir-core/src/calendar_rules.rs` — recurring calendar rules;
- `docs/forms-set-refactor-plan.md` — existing Forms Set precedence and behavior.

The copied implementation plan contains source paths from the original Native/Zig application as historical audit evidence. Those paths are not claims about this repository and must be remapped before any code or schema work begins.

## Reading and change boundary

These documents are product and research guidance, not legal or tax advice. A current COR/eCOR, ORUS record, BIR issuance, and the exact form revision and filing period control a filing decision. If registration evidence or an effective policy cannot establish one safe filing unit and return coverage, the application should surface **Review Required** rather than infer a branch or consolidated return.

This PR adds documentation only. No application code, database schema, generated form asset, or filing/submission path is changed.
