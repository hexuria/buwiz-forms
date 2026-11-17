# 2550Q v2024 candidate review - Validate orders 22-24

## Scope and pinned authority

This decision covers the singleton Item 56 validation block at HTA lines
8587-8610 and records, without executing, the successful Validate transition
at lines 8615-8618.

The authoritative source is `BIR-Form2550Qv2024.hta` from Offline
eBIRForms package `7.9.6.0`, pinned by SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
No typed Rust field, formatted display value, v1 logical-type label, or
browser reconstruction overrides the exact DOM reads and branches below.

## Official source transcription

Unlike Items 19, 42, and 47, the official function preserves both the exact
raw amount string and its parsed number:

```javascript
var addSpecifyNo56 =
    d.getElementById('frm2550qv2024:addSpecifyNo56').value;
var otherSpecify56 =
    d.getElementById('frm2550qv2024:otherSpecify56').value;
var otherSpecify56Value = parseFloat(otherSpecify56);
```

It then executes these branches in this exact order:

1. `otherSpecify56 !== "" && isNaN(otherSpecify56Value)` alerts
   ` (Item 56) must be a valid number.` and returns.
2. `addSpecifyNo56 === "" && otherSpecify56Value > 0` alerts
   `Specify field (Item 56) is required field.` and returns.
3. `addSpecifyNo56 !== "" &&
   (otherSpecify56 === "" || otherSpecify56Value === 0)` alerts
   `(Item 56) value is required when Specify field is provided.` and
   returns.

The leading space in order 22, the lack of a leading space in order 24, the
unusual `is required field` grammar in order 23, and all terminal periods are
official bytes. They remain exact in `official_message`.

These are v1 validation indices 21-23 and negative-case indices 21-23.

## Raw-string and JavaScript parseFloat contract

Order 22 checks the original string before checking the parsed result.
Consequently, exact blank is exempt from the NaN error, while whitespace is
nonblank and fails because `parseFloat` returns NaN. Order 24 also checks the
original string so a nonblank description plus exact blank amount fails even
though order 22 passed.

| Description | Raw amount | Parsed result | Official consequence |
| --- | --- | ---: | --- |
| blank | `""` | NaN | passes orders 22-24 |
| nonblank | `""` | NaN | order 24 |
| any | whitespace, `"NaN"`, or `"+"` | NaN | order 22 |
| blank | `"1abc"`, `"1,000.00"`, or `"1e"` | positive one | order 23 |
| blank | `"Infinity"` or overflow | positive infinity | order 23 |
| blank | negative or `"-Infinity"` | negative | passes |
| nonblank | `"0"`, `"-0"`, underflow, or `"0x10"` | zero | order 24 |

Whitespace-only descriptions are nonempty under the exact `=== ""` tests.
The shared closed `javascript-parse-float` predicate is sufficient, but the
Item 56 predicates must combine it with exact `is-empty`/`not is-empty`
checks on the original strings. Decimal coercion or early blank-to-null
normalization would change observable official behavior.

## Raw-control decision

The official predicates read:

- `frm2550qv2024:addSpecifyNo56` - runtime static-control index 91, v1 field
  index 10; and
- `frm2550qv2024:otherSpecify56` - runtime static-control index 93, v1 field
  index 46.

The v1 field record incorrectly labels `addSpecifyNo56` as logical boolean.
Both v2 fields are strings because validation uses exact lexical state. Both
are conditionally required: exact blank amount and blank description pass
all three official branches.

The official `otherSpecify56` control is disabled and populated through the
Item 56/additional-row workflow. The candidate models only its exact
validation read. The disabled-control and additional-row workflow remains
unresolved.

Both controls already have live singleton GPUI `InputState`s and raw-capture
bindings. Checked XML must now require both exact pre-parse authorities even
though official Validate permits an exact blank pair; required raw authority
means the lexical state must be captured, not that the value must be
nonblank.

## Official-profile executable scope

The candidate adds exactly:

- order 22 `2550q-validate-56-nan`;
- order 23 `2550q-validate-56-description`; and
- order 24 `2550q-validate-56-value`.

Fixtures distinguish exact blank from whitespace, malformed input,
longest-prefix parsing, comma, incomplete exponent, leading dot, Infinity,
overflow, negatives, signed zero, underflow, hexadecimal prefix,
whitespace-only descriptions, valid pairs, and cross-block first-error
behavior.

## Successful Validate transition remains unresolved

After all checks pass, official `validate()` calls
`AllControlDisabled(true)`, alerts
`Validation successful. Click on Edit if you wish to modify your entries.`,
and returns.

The current result contract deliberately cannot carry a workflow-state
mutation, and the candidate has no reviewed success-alert or edit-lock
policy. These effects remain documented-only and cannot be inferred from an
empty violation report. Completing orders 1-24 therefore does not authorize
the GPUI Validate action, Final Copy, serialization, queueing, or submission.

## Filing-safe unresolved

No filing-safe Item 56 lexical policy, disabled-control/additional-row
workflow, success transition, amount normalization, or issue wording has been
independently reviewed. All new filing-safe field and rule branches remain
`unresolved`.
