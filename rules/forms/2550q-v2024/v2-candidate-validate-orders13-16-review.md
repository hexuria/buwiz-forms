# 2550Q v2024 candidate review — Validate orders 13–16

## Scope and pinned authority

This decision covers only the singleton Item 19 block in official
`validate()`:

- alerting orders 13–15 at HTA lines 8506–8526; and
- the silent order-16 return at HTA lines 8528–8531.

The authoritative source is
`BIR-Form2550Qv2024.hta` from Offline eBIRForms package `7.9.6.0`, pinned by
SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
No browser reconstruction, typed Rust field, formatted display value, or
v1 logical-type label overrides these statements.

## Official source transcription

The official function first reads the exact DOM strings:

```javascript
var addSpecifyNo19 =
    d.getElementById('frm2550qv2024:addSpecifyNo19').value;
var otherCreditsNo19 =
    parseFloat(d.getElementById('frm2550qv2024:otherCreditsNo19').value);
```

It then executes these branches in this exact order:

1. `isNaN(otherCreditsNo19)` alerts
   `Other Credits (Item 19) is a required field` and returns.
2. `addSpecifyNo19 === "" && otherCreditsNo19 > 0` alerts
   `Item 19 specify is a required field.` and returns.
3. `addSpecifyNo19 !== "" && otherCreditsNo19 === 0` alerts
   `Item 19 value is required when specify is provided.` and returns.
4. `addSpecifyNo19 === "" && otherCreditsNo19 > 1000` returns without an
   alert.

The first three branches are v1 validation indices 12–14 and negative-case
indices 12–14. The silent branch was appended in v1 at validation and
negative-case index 24 with source order 16.

## JavaScript parseFloat compatibility contract

The amount predicate consumes the raw DOM string, not the typed money field.
ECMAScript `parseFloat` removes leading ECMAScript whitespace and parses the
longest leading `StrDecimalLiteral` prefix. This includes signed decimal and
exponent forms plus signed `Infinity`; malformed or empty prefixes produce
NaN. Therefore:

| Raw Item 19 amount | JavaScript result | Official consequence with blank description |
| --- | ---: | --- |
| `""`, whitespace, `"NaN"`, `"+"` | NaN | order 13 |
| `"0"`, `"-0"`, underflow to signed zero | zero | passes orders 13–15 |
| `"1abc"` | `1` | order 14 |
| `"1,000.00"` | `1` | order 14 |
| `"1e"` | `1` | order 14 |
| `"Infinity"` or numeric overflow | positive infinity | order 14 |
| `"-Infinity"` or any negative number | negative | passes orders 13–15 |

Whitespace-only descriptions are nonempty under the exact `=== ""` tests.
When a description is nonempty, `"-0"` and positive zero match order 15.
NaN never reaches orders 14–16 because order 13 returns first.

The shared v2 runtime must use a closed JavaScript-compatibility predicate,
not decimal coercion. Decimal coercion would reject valid longest-prefix and
Infinity inputs and would erase signed-zero/IEEE-754 behavior.

## Order 16 reachability decision

Order 16 is provably unreachable:

- its predicate requires a blank description and amount greater than 1000;
- every number greater than 1000 is also greater than zero; and
- order 14 has the same blank-description condition with amount greater than
  zero and returns first.

Order 16 also has no alert or state mutation. It is preserved here as
confirmed incorrect official behavior and remains outside the executable
candidate rule inventory. Omitting it from execution does not change any
reachable official result. It must not be relabeled as a passing validation
or silently deleted from historical evidence.

## Raw-control decision

The official predicates read:

- `frm2550qv2024:addSpecifyNo19` — runtime static control index 41, v1 field
  index 7; and
- `frm2550qv2024:otherCreditsNo19` — runtime static control index 43, v1 field
  index 43.

The v1 field record incorrectly labels `addSpecifyNo19` as logical boolean
despite the official text input and string comparisons. The v2 field type is
therefore `string`.

Both controls already have live GPUI `InputState`s and singleton raw-capture
bindings. Checked XML must now require both exact pre-parse authorities.
`otherCreditsNo19` must be captured before typed money parsing; a malformed
raw value cannot be synthesized from the typed draft or converted to zero.

## Official-profile executable scope

The official executable candidate adds exactly:

- order 13 `2550q-validate-19-nan`;
- order 14 `2550q-validate-19-description`; and
- order 15 `2550q-validate-19-value`.

Fixtures bind empty, whitespace, malformed, prefix, comma, exponent,
Infinity, negative, signed-zero, positive, and first-error behavior. Every
Validate fixture supplies both Item 19 raw authorities and expects all three
rules in source order.

## Official evaluation policy

Evaluation remains fail-fast in ascending phase-local rule order. When Item
13, Item 14, and any Item 19 predicate match together, only the earliest
issue is emitted. Within Item 19, NaN stops at order 13, and a blank
description with positive amount stops at order 14.

## Filing-safe unresolved

No filing-safe Item 19 lexical policy, JavaScript-prefix compatibility
decision, amount normalization, issue wording, or silent-unreachable-branch
policy has been independently reviewed. All new filing-safe field and rule
branches remain `unresolved`.

