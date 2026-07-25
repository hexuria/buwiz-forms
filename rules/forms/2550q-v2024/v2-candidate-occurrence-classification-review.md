# 2550Q v2024 candidate occurrence classification review

## Scope

This review closes the classification of all 160 observed serialization
occurrences by recording an explicit `classification` on each one, so that the
executable and documented-only populations stop being derivable only by
re-running a set difference against the rule set's field surface.

It does not make any serialization artifact executable, does not add an artifact
node, does not resolve a calculation, does not select a production clock, does
not approve a timezone, does not authorize Final Copy or Submit, and does not
open the reviewed registry.

The classification is emitted by
`crates/bir-rules-codegen` (`build-2550q-bindings`) into
`rules/forms/2550q-v2024/fixtures/serialization-binding-inventory-v796.json`,
together with a generated partition reconciliation at
`rules/forms/2550q-v2024/fixtures/static-occurrence-reconciliation-v796.json`.
The builder refuses to run when the pinned input hashes do not match, and now
also refuses when the classification counts drift from the published
decomposition.

## The split was previously unrepresented in the artifact that enumerates it

Every static occurrence carries a `candidate_v2_field_id`, including the nine
workflow, credential and UI-state controls. The inventory therefore could not
distinguish an executable value projection from a documented-only one on its
own: the distinction existed only by joining the inventory against
`rules/ir/v2/2550q-v2024-p7.9.6.0/rule-set.json` and subtracting.

Nothing pinned that join. The inventory pinned five v1 evidence inputs and read
the rule set without pinning it, so either side could move and silently change
which controls counted as "the 53".

## Classification decision

Each of the 160 occurrences now carries exactly one classification:

| classification | count |
| --- | ---: |
| `executable-singleton` | 66 |
| `executable-group-field` | 40 |
| `documented-only-derived-or-alias` | 44 |
| `documented-only-workflow-or-ui` | 5 |
| `documented-only-credential` | 4 |
| `generated-context-metadata` | 1 |

The three documented-only classes total 53, matching the published split of 44
derived/alias controls plus nine workflow, credential and UI-state controls.

The nine are separated into credential and workflow/UI classes because the
constraint on them differs in kind: a credential must never enter the tax draft
at all, whereas navigation and finality state must simply not be promoted into
filing data or read as authority.

Evidence:

- `candidate-static-surface-projection-review#workflow-credential-and-ui-state-controls`
  enumerates the nine controls verbatim and states that the candidate must not
  capture credentials into the tax draft, promote navigation state into filing
  data, or infer finality from `txtFinalFlag`.
- `candidate-serialization-binding-inventory-review#complete-observed-occurrence-binding`
  establishes the 119 static, 40 materialized repeated-family and one generated
  occurrence populations.

## The rule-set join is pinned by field surface, not by file hash

Pinning `sha256(rule-set.json)` into the inventory is not possible. The
inventory is itself a declared source of that rule set, so the pin would change
the inventory, which would change the hash the rule set must declare for it,
which would change the rule set, which would invalidate the pin. The loop does
not settle.

The inventory therefore pins `input_sha256.rule_set_field_ids`: a digest over
the sorted executable field-id surface. That is the part of the rule set the
classification actually depends on, it changes exactly when that surface drifts,
and it is stable across `source_set_sha256` rolls because the source-set digest
is computed over parsed canonical JSON with the pin fields nulled.

## Non-executable decision

The following remain unchanged and unresolved by this review:

- all three serialization artifacts stay `documented_only`, node-less and
  unmaterializable;
- the 44 derived and alias projections stay documented-only; none of their
  calculations, scopes, instance selectors, rounding policies or fixtures have
  been resolved, and naming the classification does not supply that evidence;
- the nine workflow, credential and UI-state controls stay outside the candidate
  field surface;
- the relation between app-owned stable repeated-group order and the official
  live DOM and display serialization order stays unresolved;
- the production clock, timezone and custody provider for `local-current-date`
  stays unresolved.

The inventory remains value-free: `values_emitted` is `false`, no artifact node
list exists, the filing-safe branch remains `unresolved`, and the reviewed
runtime registry remains empty. This review confers no authority to Save, Final
Copy, Submit, queue, transport or release.
