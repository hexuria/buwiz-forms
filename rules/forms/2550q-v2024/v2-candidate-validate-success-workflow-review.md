# 2550Q v2024 candidate review - successful Validate workflow

## Scope and pinned authority

This decision covers the successful tail of `validate()` at HTA lines
8615-8618 and the control-state implementation in `AllControlDisabled(state)`
at lines 8688-8758.

The authoritative source is `BIR-Form2550Qv2024.hta` from Offline
eBIRForms package `7.9.6.0`, pinned by SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The legacy workflow inventory is separately pinned as source
`v1-workflow`, transition index 1.

## Exact official transition

After every alerting branch at Validate orders 1-24 has continued without
returning, the official function executes:

```javascript
AllControlDisabled(true);
alert("Validation successful. Click on Edit if you wish to modify your entries.");
return;
```

`AllControlDisabled(true)` disables the editable form controls, schedule-add
buttons, and Validate button. Its button tail enables Edit, Print, Final Copy,
and Upload by assigning each corresponding `disabled` property to `!state`.
The exact success alert is 72 characters and ends with a period.

This establishes one official transition:

- transition ID: `validate-success`;
- current state: `edit`;
- action and required evaluation phase: `validate`;
- next state: `validated`;
- state effect: `set-workflow-state(validated)`; and
- notification effect: an `alert` carrying the exact official message.

The candidate does not translate the later Edit, Final Copy, or transport
transitions. Their guards, artifacts, and side effects remain unresolved.

## Request-bound execution decision

The workflow transition is not inferred from an empty violation list.
Callers must provide:

1. the exact `EvaluationRequest`;
2. the matching validated `EvaluationResult`;
3. current state `edit`; and
4. action `validate`.

The rules provider rejects a mismatched rule-set identity, input revision,
context fingerprint, canonical raw snapshot, action/phase pair, invalid
evaluation, missing or ambiguous transition, unavailable profile branch,
false guard, unsupported effect, or state-effect/target mismatch.

This preserves a deliberate separation:

- rule evaluation proves the complete ordered validation report;
- workflow selection proves an explicit transition against that same report;
- GPUI may apply control locking and display the returned alert only after
  both operations succeed.

The state output is semantic. GPUI owns presentation and maps `validated` to
its revision-specific control policy; the rules engine does not depend on
GPUI controls. Final Copy, serialization, queueing, upload, and submission
remain unavailable because their transitions and artifacts are unresolved.

## Fixture decision

The existing positive official Validate fixture is extended with a workflow
invocation and exact expected result. Generator audit requires every
executable workflow transition/profile branch to have a zero-violation
fixture, exact action/phase binding, exact transition identity, matching
request identity/revision/fingerprint, and byte-exact notification payload.

Negative runtime tests separately prove that an invalid evaluation, wrong
current state, wrong action phase, and mismatched request/result cannot
activate the transition.

## Filing-safe unresolved

No filing-safe validation-success policy, edit locking policy, Final Copy
authorization, or user-notification wording has been independently reviewed.
The filing-safe transition branch remains `unresolved`; it cannot borrow the
official branch.
