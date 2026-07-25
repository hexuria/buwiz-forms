# 2550Q v2024 candidate repeated-group projection review

## Scope

This review binds the 28 unbounded dynamic-family descriptors to the same seven
semantic group identities already persisted and captured by the GPUI/core
2550Q adapter. It approves raw-string candidate field behavior for the
official profile only. It does not approve serialization nodes, calculations,
filing-safe behavior, Final Copy, or submission.

## Identity decision

The exact group identities are:

| Source family | Semantic group ID | Members |
| --- | --- | ---: |
| Schedule 1 capital goods | `schedule-1-capital-good-row` | 9 |
| Schedule 3 creditable VAT | `schedule-3-creditable-vat-row` | 5 |
| Schedule 4 advance VAT | `schedule-4-advance-vat-row` | 6 |
| Item 19 additional credits | `item-19-additional-row` | 2 |
| Item 42 additional input tax | `item-42-additional-row` | 2 |
| Item 47 additional input tax | `item-47-additional-row` | 2 |
| Item 56 additional deductions | `item-56-additional-row` | 2 |

These IDs match `crates/bir-core/src/form_rules/form_2550q.rs`. The evidence
inventory must not introduce a second naming system.

The adapter assigns fixed-width monotonic `StableInstanceId` values, persists
them with each row, and retains them across reorder. They are neither UUIDs nor
official package row IDs. The v2 group contract therefore uses the explicit
`assigned-stable-id` identity kind.

## Raw-string behavior decision

All 28 family members are live HTML `input type="text"` controls when their
row exists. The official save loops read `.value` directly. For candidate
evaluation, each member therefore:

- has v2 `value_type: string`;
- uses `control_kind: text`;
- is conditional on a materialized group instance;
- performs no normalization;
- uses string coercion with `on_empty: empty-string`; and
- preserves malformed numeric/date-looking text as text.

This decision is deliberately lexical. It does not claim that a date, money,
integer, or computed-looking control has passed the package's separate
calculation or validation behavior. Turning these members into decimal/date
types at capture time would destroy the raw input needed for official
bug-compatible validation and exact later serialization.

## Cardinality and ordering

All seven groups have `min_occurs: 0` and `max_occurs: null`. The package has no
maximum-row guard.

The member order is the source DOM child order recorded by
`serialization-binding-inventory-v796.json`. Runtime group instances use
stable instance identity order; a later executable artifact must separately
review how that stable order is related to official DOM/display order.

## Candidate-only decision

The official field branches may execute as lossless raw-string capture.
Every filing-safe branch remains `unresolved`. The serialization artifacts
remain `documented_only` with no nodes, and the reviewed registry remains
empty.

The four additional-item families currently have no live GPUI editor controls.
Their groups and fields are still declared so missing capture remains an
explicit gap instead of silently deleting official package state.
