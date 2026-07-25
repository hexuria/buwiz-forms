# Why this corpus is fail-closed

The rules under `rules/` describe how the official Offline eBIRForms package
validates a taxpayer's return. Getting one of them wrong does not produce a bug
report — it produces a wrong tax filing. Every design choice here follows from
that, and several of them look excessive until you remember it.

## The two meanings of "complete"

`manifest.json` may say `"status": "complete"`. That means **the v1 evidence
inventory is complete or its gaps are explicit**. It never means a form is
executable, filing-safe, release-ready, or authorized for Final Copy or
submission. Those are separate, independently evidenced decisions.

## Official behavior and safe behavior are different branches

Every rule carries two profiles. `official` reproduces the package as it
actually behaves, **including its defects** — thirteen confirmed official bugs
are recorded for 2550Q alone. `filing_safe` is where a safer behavior may live,
and only with independent evidence.

Never "fix" the official branch. A reproduction of an official defect is
evidence; a silent correction is a divergence nobody reviewed, and it will be
discovered by a taxpayer rather than by a test.

`filing_safe` for 2550Q is `unresolved`, and a non-executable branch can never
fall back to the other profile.

## Identity is five parts, never one

A rule set is selected by form code **and** printed revision **and** official
package version **and** rule-set ID **and** source-set digest. Selecting by form
code alone would let a draft prepared under one official package be validated by
another — which is exactly the failure a versioned corpus exists to prevent.

## Why the digest roll is atomic

`source_set_sha256` binds a rule set to its 121 fixtures. It appears at 246
sites across 123 files. A partial roll leaves a corpus that passes some checks
and fails others, with no single place to see which — so a partial roll must
fail outright rather than be patched around. Use the `roll-pin` command.

Note that the digest is computed over *parsed canonical JSON* with the pin
fields nulled, so it does not depend on the current pin values and is immune to
formatting and line endings. Declared **source** hashes are the opposite: they
hash raw file bytes, so line endings matter enormously there.

## What "documented_only" means

A recorded, source-pinned, machine-readable identity that the compiler and
dispatcher must **refuse to execute**. It is not the same as `unresolved`, which
is the absence of a reviewed answer. Both are non-executable; only one claims to
know what the official behavior is.

An artifact whose branch is `documented_only` carries no node list at all — the
plan is absent at the type level, not merely empty — so nothing can be
materialized from it.

## The registry is empty on purpose

`crates/bir-rules/src/generated/registry.rs` declares no reviewed rule sets. That
single fact is what makes the submission preflight unable to pass, which is what
makes the whole subsystem safe to develop against. Populating it is a reviewed
promotion step with its own evidence commit — never a side effect of making a
test green.
