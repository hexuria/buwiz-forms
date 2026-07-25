# Evidence â€” 2550Q April 2024 ENCS

The exact April 2024 runtime HTA, its shared `string-util.js` dependency, loaded
`eBIRTools.vbs`, external encryption and transport helpers, installed help,
official form PDF, official guidelines PDF, dummy plaintext finalized save,
dummy encrypted final copy, and package executable are pinned in
`manifest.json`. The executable-helper hashes establish package identity only;
they do not prove internal algorithms or authorize execution.

The plaintext finalized save contains 160 unique pseudo-div keys; the
decrypted encrypted final copy contains 159. Their pseudo-div difference is
generated `dateFiled`. The plaintext sample ends in the exact
`All Rights Reserved BIR 2012.0` marker used by `isItAFinalCopy()` and is
therefore not evidence of the non-final `saveXML(false)` tail, even though its
serialized `txtFinalFlag` value is independently `1`.
The Final Copy source instead appends `dateFiled` as a standalone metadata
element after the final marker, so it is absent from the 159-key pseudo-div
inventory but not from the complete decrypted artifact.

The concrete union therefore has 160 keys. Source inspection also proves 28 unbounded indexed families: nine Schedule 1 fields, five Schedule 3 fields, six Schedule 4 fields, and description/amount pairs for Items 19, 42, 47, and 56. The HTA has Add/Delete behavior and no maximum-row guard, so `fields.json` records family descriptors instead of inventing a finite row count.

All representative values are excluded. Email-bearing filenames are redacted as `#email-redacted#`. No online submission was performed.

## V2 role reconciliation

Direct inspection of the pinned HTA resolves several v1 metadata and alias
conflicts for an official-profile candidate:

- `lessOutputVat`, `addOutputVat`, and every
  `txtTotalTaxWithHeld3{N}` control are enabled source inputs even though their
  v1 field records say `computed: true`.
- `txtAllowedInputTax1{N}` and `txtBalanceInputTax1{N}` are disabled Schedule 1
  outputs derived by `computeSched1`.
- `domesticInputTax`, `serviceInputTax`, and `importInputTax` are disabled
  outputs derived at 12% by `compute44AB` through `compute46AB`.
- unprefixed `otherSpecify47B` is the exact disabled concrete key derived at
  12% by `compute47AB`; the prefixed calculation token is an alias and does
  not identify a second field.

The pinned shared script defines `NumWithComma` with JavaScript `parseFloat`
and defines `formatCurrency`/`round` using binary-number arithmetic followed by
`Math.floor(value * 100 + 0.50000000001)`. Exact-decimal v2 arithmetic must not
claim bit-for-bit official compatibility at rounding boundaries until
executable fixtures establish the intended emulation or record a reviewed
profile difference.

The exact HTA locators and candidate treatments are recorded in
`docs/validation-rules/2550q-adapter-map.md`. These observations resolve field
roles only. They do not approve filing-safe behavior, artifact serialization,
Final Copy, queueing, or submission.

The source-pinned combined Final Copy/Submit call graph is recorded separately
in `v2-candidate-final-copy-submit-workflow-review.md`. The pinned shared
runtime's `checkNetConnection()` returns `true` immediately, so its
no-connection local-copy branch is unreachable. No online submission was
performed.

The package-specific Final Copy artifact identity and value-free 159-key
occurrence order are reviewed in
`v2-candidate-final-copy-serialization-review.md`. That review records only a
node-less `documented_only` artifact. It does not approve materialization,
encryption, custody, or filing-safe behavior.

The corrected plaintext sample classification, shared editable/finalized Save
order rule, exact value-free 160-key sequence, and separate plaintext artifact
identities are reviewed in
`v2-candidate-plaintext-save-serialization-review.md`. The encrypted 159-key
sequence is its exact prefix and pseudo-div `dateFiled` is the sole suffix.
Both plaintext artifacts remain node-less and non-executable.

The reproducible value-free projection plan is retained in
`fixtures/serialization-binding-inventory-v796.json` and reviewed in
`v2-candidate-serialization-binding-inventory-review.md`. It binds all 160
observed plaintext occurrences, the 159-occurrence encrypted prefix, the
artifact-specific taxpayer name/address codec difference, and all 28 unbounded
families partitioned into seven `assigned-stable-id` dynamic groups. All 159
live serialized-control occurrences have candidate identities: 119 static
controls plus 40 materialized repeated-family occurrences. The executable
candidate surface contains 66 singleton fields and 28 repeated-family
descriptors. Forty-four derived/alias controls and nine
workflow/credential/UI-state controls remain identity-only documented
bindings. Generated `dateFiled` has a reviewed value projection from the
required immutable `local-current-date` context snapshot; the production clock
and timezone provider remains unresolved.
Stable-instance order is not yet bound to official live DOM/display order.
The core/GPUI raw adapter declares and exposes all 66 executable singleton
identities. Amended-return Yes/No and short-period Yes/No use four independent
raw-backed choices whose exact mutually exclusive values are captured only on
explicit clicks or restored from reviewed import. TIN segments,
branch, RDO, taxpayer name, address, ZIP, contact, and email use raw-only
controls that remain absent for profile-derived drafts until exact
reviewed-import text is restored or the user explicitly edits the control;
typed profile values are never fabricated into raw authority.
No taxpayer or credential values are retained.
