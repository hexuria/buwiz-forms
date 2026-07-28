---
name: ebirforms-validation-rules
description: Extract, audit, compile, or extend the official Offline eBIRForms validation rules corpus under rules/, the bir-rules-codegen compiler, the bir-rules static runtime, or the bir-core trusted filing boundary. Use for rule evidence, JSON IR v2 snapshots, source-set digests, serialization bindings, or filing-safe profile work. Do not use for HTML print preview, visual parity, or renderer work.
---

# Work the eBIRForms validation-rules corpus

This is a **fail-closed evidence system**, not ordinary application code. Nothing
here is authorized to file a tax return, and every gate exists to keep it that
way. Read [boundaries](references/boundaries.md) before your first edit.

## Non-negotiable boundaries

Only form 2550Q has a v2 rule set, it is `review_status: candidate`, and it is
**test-only**. It must never be promoted. Five guards keep production closed,
each one edit away from opening a filing path:

- `crates/bir-core/src/form_rules/form_2550q.rs:42-44` — returns `None`
- `crates/bir-rules/src/generated/mod.rs:4` — `#[cfg(test)]` candidate module
- `crates/bir-core/src/form_rules/form_2550q.rs:930` — `#[cfg(test)]` evaluator
- `crates/bir-rules/src/generated/registry.rs:18-19` — empty reviewed registry
- `crates/bir-core/src/form_rules/payload.rs:228-234` — always returns `Err`

Also fixed: filing-safe stays `unresolved`; all three serialization artifacts
stay `documented_only`, node-less and `values_emitted: false`; official defects
are preserved in the official profile rather than silently fixed; never use the
official submission path for discovery; never use real taxpayer data; never
touch the live encrypted database.

**Never weaken a validator, threshold or assertion to make a gate pass.**

## Triage by ownership

| Change | Owner |
| --- | --- |
| Rule evidence, fixtures, review documents | `rules/` |
| Audit, compile, digest, drift | `crates/bir-rules-codegen/` |
| Typed evaluator, workflow, serialization primitives | `crates/bir-rules/` |
| Shadow/trusted dispatch, Final Copy proof | `crates/bir-core/src/form_rules/` |
| Stale-safe report controller | `crates/bir-desktop/src/components/form_validation/` |

Evidence flows one way: `rules/` → codegen → runtime → core → GPUI. The packaged
app never reads `rules/`.

## Standard workflow

1. **Check status first.** `status` is the machine-checkable condition and
   separates **boundary** criteria (a production authority must stay closed)
   from **slice** criteria (current work). A boundary failure is far more serious
   than an open slice.
2. **Reproduce before changing.** Copy inputs to a scratch root and run tools
   with `--repo-root <scratch>`. Never let an exploratory run write into `rules/`.
3. **Compare semantically.** `jq -S -c` equality decides whether evidence
   changed. A byte difference with `jq -S -c` equality is formatting, not a
   regression. Confusing the two wastes sessions.
4. **Roll digests with the tool, never by hand.** A source-set roll is a
   123-file, 246-site atomic transaction; a partial roll must fail.
5. **Regenerate and confirm scope.** After touching any generator source, the
   manifest diff must be confined to `generator.source_sha256`.

## Helper commands

```sh
rtk cargo run -q --locked -p bir-rules-codegen -- status
rtk cargo run --locked -p bir-rules-codegen -- validate-v1
rtk cargo run --locked -p bir-rules-codegen -- audit
rtk cargo run --locked -p bir-rules-codegen -- roll-pin --rule-set-id <id> --dry-run
rtk cargo run --locked -p bir-rules-codegen -- build-2550q-bindings
rtk npm run rules:check
rtk cargo test --locked -p bir-rules -p bir-rules-codegen
rtk cargo test --locked -p bir-core form_2550q
```

`validate-v1` must always report **43 forms, 659 JSON (139 v2), 9,592 fields,
2,007 validations, 623 calculations, 1,354 negative fixtures, 216 schema
documents**. If any count moves, the change is wrong.

## Traps that have already cost sessions

- **`rules/tools/*.ps1` are mostly provenance records, not tools.** Their inputs
  are not tracked, so they cannot run from a clone on any OS. Read
  `rules/tools/README.md` first.
- **Windows PowerShell 5.1 and pwsh 7 format `ConvertTo-Json` differently.** The
  tracked evidence was written by 5.1 and no modern tool reproduces its layout.
- **Line endings are load-bearing for declared sources.** A CRLF source file
  pins a hash that cannot survive `eol=lf` checkout, which silently breaks the
  audit on every clone.
- **You cannot pin `sha256(rule-set.json)` into the binding inventory.** The
  inventory is itself a declared source of the rule set, so the pin is circular.
  Pin the field-id surface digest instead.

## Stop conditions

Stop and report rather than proceeding if: a boundary criterion fails; a
declared-source hash mismatches and you do not know why; a digest roll partially
applies; or a change would require editing an assertion to pass. The corpus is
evidence — a partial objective with intact evidence beats a complete one with
evidence you cannot defend.
