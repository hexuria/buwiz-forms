# Findings ledger

One JSON file per pull-request group in `tools/form_findings_priority.json`,
named after the group's branch with `/` replaced by `__`
(`fix__findings-q3-2551q-2550q.json`). Each maps form code → finding id →
entry; finding ids are the ids in `docs/form-findings/<CODE>/findings.json`,
and `manual-<n>` for its `manual` list.

```json
{"2551Q": {"R8.0": {"status": "fixed", "pr": "PR 1", "proof": "screen_form_module::tests::new_2551q_has_no_penalty"}}}
```

Statuses: `fixed` (needs `pr` and `proof`), `not_a_bug` (needs `reason` and
the `reviewer` verdict), `deferred` (needs `approved_by_maintainer: true`,
set only by the maintainer). `python3 scripts/check_fix_order_goal.py` reads
every file here.
