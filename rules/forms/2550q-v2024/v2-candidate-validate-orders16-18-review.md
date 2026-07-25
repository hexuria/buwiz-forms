# 2550Q v2024 candidate review - Validate orders 16-18

## Scope and pinned authority

This decision covers only the singleton Item 42 block in official
`validate()` at HTA lines 8537-8559.

The authoritative source is `BIR-Form2550Qv2024.hta` from Offline
eBIRForms package `7.9.6.0`, pinned by SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
No typed Rust field, formatted display value, v1 logical-type label, or
browser reconstruction overrides the exact DOM reads and branches below.

## Official source transcription

The official function reads the exact DOM description string and parses the
exact amount string with JavaScript `parseFloat`:

```javascript
var addSpecifyNo42 =
    d.getElementById('frm2550qv2024:addSpecifyNo42').value;
var otherSpecify42 =
    parseFloat(d.getElementById('frm2550qv2024:otherSpecify42').value);
```

It then executes these branches in this exact order:

1. `isNaN(otherSpecify42)` alerts
   ` (Item 42) is a required field` and returns.
2. `addSpecifyNo42 === "" && otherSpecify42 > 0` alerts
   `Specify field (Item 42) is required ` and returns.
3. `addSpecifyNo42 !== "" && otherSpecify42 === 0` alerts
   ` (Item 42) value is required when Specify field is provided.` and
   returns.

The leading space in orders 16 and 18, the trailing space in order 17, the
missing terminal period in orders 16 and 17, and the capitalized `Specify` in
order 18 are official bytes. They are retained in `official_message`; they
must not be normalized as editorial cleanup.

These are v1 validation indices 15-17 and negative-case indices 15-17.

## JavaScript parseFloat compatibility contract

The amount predicate consumes the raw DOM string. ECMAScript `parseFloat`
removes leading ECMAScript whitespace and parses the longest leading decimal
or signed-Infinity prefix. Empty or malformed prefixes produce NaN. The
shared closed `javascript-parse-float` predicate already implements this
contract and must be reused unchanged for Item 42.

Therefore:

| Raw Item 42 amount | JavaScript result | Official consequence with blank description |
| --- | ---: | --- |
| `""`, whitespace, `"NaN"`, `"+"` | NaN | order 16 |
| `"0"`, `"-0"`, underflow to signed zero | zero | passes orders 16-18 |
| `"1abc"`, `"1,000.00"`, or `"1e"` | positive one | order 17 |
| `"Infinity"` or numeric overflow | positive infinity | order 17 |
| `"-Infinity"` or any negative number | negative | passes orders 16-18 |

Whitespace-only descriptions are nonempty under the exact `=== ""` tests.
With a nonempty description, positive or negative zero triggers order 18.
NaN never reaches orders 17-18 because order 16 returns first.

## Raw-control decision

The official predicates read:

- `frm2550qv2024:addSpecifyNo42` - runtime static-control index 68, v1 field
  index 8; and
- `frm2550qv2024:otherSpecify42` - runtime static-control index 70, v1 field
  index 44.

The v1 field record incorrectly labels `addSpecifyNo42` as logical boolean
despite the official text input and string comparisons. Both v2 fields are
strings because validation acts on their exact lexical buffers.

The official static `otherSpecify42` control is disabled and named
`frm2550qv2024:totalTaxPayableNo42`; its value is populated by the official
Item 42/additional-row workflow. The validation branch still reads the
control directly. This decision models only that read and does not authorize
or reconstruct the unresolved modal/additional-row workflow.

Both controls already have live singleton GPUI `InputState`s and raw-capture
bindings. Checked XML must now require both exact pre-parse authorities.
`otherSpecify42` must be captured before typed money parsing; malformed or
prefix-bearing text cannot be synthesized from the typed draft.

## Official-profile executable scope

The official executable candidate adds exactly:

- order 16 `2550q-validate-42-nan`;
- order 17 `2550q-validate-42-description`; and
- order 18 `2550q-validate-42-value`.

Fixtures bind blank, whitespace, malformed, longest-prefix, comma,
incomplete-exponent, leading-dot, Infinity, overflow, negative, signed-zero,
underflow, whitespace-description, positive, and cross-block first-error
behavior. Every Validate fixture supplies both Item 42 raw authorities and
expects all three rules after Item 19 in source order.

## Official evaluation policy

Evaluation remains fail-fast in ascending phase-local rule order. NaN stops
at order 16. A blank description with a positive amount stops at order 17.
When Item 19 order 15 and Item 42 order 16 both fail, only Item 19 is emitted.

## Filing-safe unresolved

No filing-safe Item 42 lexical policy, disabled-control/additional-row
workflow, JavaScript-prefix compatibility decision, amount normalization, or
issue wording has been independently reviewed. All new filing-safe field and
rule branches remain `unresolved`.
