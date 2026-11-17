# 2550Q v2024 candidate review - Validate orders 19-21

## Scope and pinned authority

This decision covers only the singleton Item 47 block in official
`validate()` at HTA lines 8563-8584.

The authoritative source is `BIR-Form2550Qv2024.hta` from Offline
eBIRForms package `7.9.6.0`, pinned by SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
No typed Rust field, formatted display value, v1 logical-type label, or
browser reconstruction overrides the exact DOM reads and branches below.

## Official source transcription

The official function reads the exact DOM description string and parses the
exact amount string with JavaScript `parseFloat`:

```javascript
var addSpecifyNo47 =
    d.getElementById('frm2550qv2024:addSpecifyNo47').value;
var otherSpecify47 =
    parseFloat(d.getElementById('frm2550qv2024:otherSpecify47').value);
```

It then executes these branches in this exact order:

1. `isNaN(otherSpecify47)` alerts
   ` (Item 47) is a required field.` and returns.
2. `addSpecifyNo47 === "" && otherSpecify47 > 0` alerts
   `Specify field (Item 47) is required field.` and returns.
3. `addSpecifyNo47 !== "" && otherSpecify47 === 0` alerts
   ` (Item 47) value is required when Specify field is provided.` and
   returns.

The leading space in orders 19 and 21, the unusual `is required field`
grammar in order 20, and every terminal period are official bytes. They are
retained in `official_message` and must not be normalized as editorial
cleanup.

These are v1 validation indices 18-20 and negative-case indices 18-20.

## JavaScript parseFloat compatibility contract

The amount predicate consumes the raw DOM string. ECMAScript `parseFloat`
removes leading ECMAScript whitespace and parses the longest leading decimal
or signed-Infinity prefix. Empty or malformed prefixes produce NaN. The
shared closed `javascript-parse-float` predicate must be reused unchanged.

Therefore:

| Raw Item 47 amount | JavaScript result | Official consequence with blank description |
| --- | ---: | --- |
| `""`, whitespace, `"NaN"`, `"+"` | NaN | order 19 |
| `"0"`, `"-0"`, underflow to signed zero | zero | passes orders 19-21 |
| `"1abc"`, `"1,000.00"`, or `"1e"` | positive one | order 20 |
| `"Infinity"` or numeric overflow | positive infinity | order 20 |
| `"-Infinity"` or any negative number | negative | passes orders 19-21 |

Whitespace-only descriptions are nonempty under the exact `=== ""` tests.
With a nonempty description, positive or negative zero triggers order 21.
NaN never reaches orders 20-21 because order 19 returns first.

## Raw-control decision

The official predicates read:

- `frm2550qv2024:addSpecifyNo47` - runtime static-control index 78, v1 field
  index 9; and
- `frm2550qv2024:otherSpecify47` - runtime static-control index 80, v1 field
  index 45.

The v1 field record incorrectly labels `addSpecifyNo47` as logical boolean
despite the official text input and string comparisons. Both v2 fields are
strings because validation acts on their exact lexical buffers. The amount
field is `required` for Validate because every NaN result stops at order 19;
the description is conditionally required.

The official static `otherSpecify47` control is disabled and named
`frm2550qv2024:txtVatableSales47A`; its value participates in the official
Item 47/additional-row and Item 47B calculation workflow. The validation
branch still reads the control directly. This decision models only that read
and does not authorize or reconstruct the unresolved disabled-control,
additional-row, or 12-percent calculation workflow.

Both controls already have live singleton GPUI `InputState`s and raw-capture
bindings. Checked XML must now require both exact pre-parse authorities.
`otherSpecify47` must be captured before typed money parsing; malformed or
prefix-bearing text cannot be synthesized from the typed draft.

## Official-profile executable scope

The official executable candidate adds exactly:

- order 19 `2550q-validate-47-nan`;
- order 20 `2550q-validate-47-description`; and
- order 21 `2550q-validate-47-value`.

Fixtures bind blank, whitespace, malformed, longest-prefix, comma,
incomplete-exponent, leading-dot, Infinity, overflow, negative, signed-zero,
underflow, whitespace-description, positive, and cross-block first-error
behavior. Every Validate fixture supplies both Item 47 raw authorities and
expects all three rules after Item 42 in source order.

## Official evaluation policy

Evaluation remains fail-fast in ascending phase-local rule order. NaN stops
at order 19. A blank description with a positive amount stops at order 20.
When Item 42 order 18 and Item 47 order 19 both fail, only Item 42 is emitted.

## Filing-safe unresolved

No filing-safe Item 47 lexical policy, disabled-control/additional-row/
calculation workflow, JavaScript-prefix compatibility decision, amount
normalization, or issue wording has been independently reviewed. All new
filing-safe field and rule branches remain `unresolved`.
