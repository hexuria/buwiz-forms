# 2550Q v2024 candidate plaintext Save serialization review

## Scope and pinned authority

This review identifies the package-specific plaintext editable-save and
finalized-save artifact variants and establishes their occurrence-order rule.
It does not approve serialization nodes, file creation, Final Copy, or filing.

The authoritative form source is
`C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form2550Qv2024.hta`,
SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The value-free ordered evidence is
`forms/2550q-v2024/fixtures/plaintext-field-audit-v796.json`, derived from the
single pinned dummy plaintext sample with SHA-256
`43577fdd70b8959b16dbada9ff7d8418a1fdc5d18e61302c8cbfc8e9bbab4520`.
The email-bearing source filename remains redacted.

## Corrected sample classification

The pinned plaintext sample is not evidence of a non-final editable-save tail.
It ends in the exact string `All Rights Reserved BIR 2012.0`.
`isItAFinalCopy(xmlFile)` at HTA lines 5308-5318 uses that exact substring as
its finality classifier. The sample is therefore a plaintext finalized save.

Its pseudo-div `txtFinalFlag` value is independently `1`. That control value
does not override the marker classifier. Treating `txtFinalFlag == "1"` as
proof that the artifact is editable would contradict the loaded source.

## Artifact identity decision

The candidate records two package-specific plaintext artifact identities:

- `official-editable-save`, target `editable-save`, variant
  `p7.9.6.0-dom-order`; and
- `official-finalized-save`, target `finalized-save`, variant
  `p7.9.6.0-dom-order`.

Both official branches are `documented_only`, both filing-safe branches are
`unresolved`, and neither artifact contains nodes.

The identities are distinct because `saveXML(false)` and `saveXML(true)` have
different final-marker semantics even though they share the same control loop.
The finalized variant is observed in the pinned sample. The editable variant's
occurrence rule is source-established, but no separately pinned plaintext
sample proves its complete non-final byte tail.

## Shared occurrence-order rule

The normal `saveXML(isFinalCopy)` path at HTA lines 5516-5628:

1. starts with `<?xml version='1.0'?>` and the runtime
   `xmlFormat.innerHTML` separator;
2. iterates the live `frmMain.elements` collection in DOM order;
3. excludes controls whose type is `button`, `hidden`, or `undefined`;
4. emits text and `select-one` controls as pseudo-div occurrences;
5. emits radio and checkbox controls as pseudo-div occurrences using their
   JavaScript boolean `checked` value;
6. appends generated pseudo-div `dateFiled` after the live-control loop; and
7. appends the marker selected by `isFinalCopy`.

The boolean affects only the final marker after the shared loop:

- `saveXML(true)` appends `All Rights Reserved BIR 2012.0` and sets
  `gIsReadOnly = true`;
- `saveXML(false)` appends `All Rights Reserved BIR 2012.` without the final
  zero.

Consequently the editable and finalized plaintext paths share one
occurrence-order rule for a given live DOM. They must still remain separate
artifact identities because their tails and workflow effects differ.

## Ordered sample evidence

The value-free plaintext audit contains 160 occurrences and 160 unique keys.
Its sorted-set `field_inventory_sha256` is
`8191f685cb07c4d233cc3de32066fd7b83248160df780578b960cb57d9ac5f29`.
Its order-sensitive `ordered_field_inventory_sha256` is
`64154a96231f59c04ce83840955713f8a668984759e6c50e44ebd7bb010fc1d3`.

The first occurrence is `frm2550qv2024:calendarNo1`; the last is
`dateFiled`. The first 159 occurrences exactly equal the encrypted Final Copy
audit's full ordered sequence, whose order-sensitive digest is
`b0c81408ca4e6afd61ada8d72ad61ca9833db7de958f2e772496e3c20405fd95`.
The sole plaintext suffix is pseudo-div `dateFiled`.

This is an observed-instance baseline, not a fixed-row schema. Schedule and
additional-item controls can be added or removed at runtime, so executable
serialization must use reviewed dynamic groups and occurrence identities
rather than hard-code this sample's row count.

## Value-codec boundary

Within the shared plaintext loop:

- `taxpayerName` and `taxpayerAddress` pass through legacy JavaScript
  `escape()`;
- other text and `select-one` values are concatenated directly;
- radio and checkbox values use JavaScript `true` or `false`; and
- generated `dateFiled` uses local-clock `YYYY/MM/DD`.

The pinned sample is 10,910 bytes, has no UTF-8 BOM, contains only ASCII bytes,
uses 160 bare LF separators and no CRLF separators, begins with the exact XML
declaration, and ends with the finalized `.0` marker. Those observations do
not prove how `Scripting.FileSystemObject.CreateTextFile` handles non-ASCII
unescaped values on every supported Windows locale.

No executable codec may be selected until each occurrence is bound to its
exact semantic source, raw-value policy, presence policy, and artifact-specific
body codec.

## Documented-only artifact decision

The two artifact identities distinguish the known official paths without
granting materialization authority. Executable translation remains blocked by:

- the unmodeled majority of the 160-control serialization surface;
- dynamic row-group ordering and occurrence projection;
- exact runtime separator, encoding, and non-ASCII behavior;
- complete editable versus finalized marker bytes;
- reviewed local-clock and timezone policy for `dateFiled`;
- filename, version, overwrite, and path-confinement behavior;
- exact file lifecycle and failure handling; and
- an independent filing-safe policy.

Partial node emission would silently drop official control state and is
forbidden.

## Filing-safe unresolved

No filing-safe decision approves either plaintext artifact, its raw unescaped
values, legacy escape behavior, generated filing date, final marker, filename,
overwrite behavior, storage location, or custody policy. Both filing-safe
branches remain `unresolved`.

The reviewed registry remains empty. This review does not activate Save, Final
Copy, Upload, Submit, queueing, transport, release status, or any production
capability.
