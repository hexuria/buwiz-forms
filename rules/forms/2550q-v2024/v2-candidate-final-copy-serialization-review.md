# 2550Q v2024 candidate Final Copy serialization review

## Scope and pinned authority

This review identifies the package-specific encrypted Final Copy artifact and
records the locally established plaintext staging shape. It does not approve
serialization nodes, encryption, submission, or filing.

The authoritative form source is
`C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form2550Qv2024.hta`,
SHA-256
`3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70`.
The reviewed dummy encrypted copy is
`C:\Mac\Home\Downloads\forms\2550Qv2024\2550Q-final-copy-#email-redacted#.xml`,
ciphertext SHA-256
`57ccf9d8132c490d54bceaf5c55fc2b4bec01b780951a63600402c61a595cdbe`
and decrypted SHA-256
`6dce2b9614d583cd682de6d301dd4b52078938be2d840ff39dcaeb67cad9ee98`.
Only its ordered, value-free field audit is retained at
`forms/2550q-v2024/fixtures/encrypted-field-audit-v796.json`.

The external helper is `C:\eBIRForms\Encrypt.exe`, 489,452 bytes, SHA-256
`429337f44f84b93cd1095df48c8f3265e5ede7c646d1b48d9b80f4f92de74d2c`.
Its identity is pinned, but its container, compression, and encryption
algorithms remain opaque.

## Artifact identity decision

The v2 candidate records one artifact identity:

- artifact ID: `official-encrypted-final-copy`;
- target: `encrypted-final-copy`; and
- variant ID: `p7.9.6.0-dom-order`.

The variant is intentionally package-specific. The source iterates the live
`frmMain.elements` collection, so a different package DOM may change the
occurrence order even when the printed revision label is unchanged.

This identity is an inventory landmark only. Its official branch is
`documented_only`, its filing-safe branch is `unresolved`, and neither branch
contains nodes.

## Established plaintext staging shape

`saveEncryptedProfile(isFromSubmit)` at HTA lines 5194-5267 first requires
`saveXML(true)` to succeed. It then creates an `IAF_RDO_Copy` plaintext staging
file and concatenates, in this order:

1. the exact source literal `<?xml version='1.0'?>`;
2. the runtime `xmlFormat.innerHTML` separator;
3. one pseudo-div for every live `frmMain.elements` entry whose type is not
   `button`, `hidden`, or `undefined`, preserving DOM order;
4. the runtime `xmlFormat.innerHTML` separator, followed by
   `All Rights Reserved BIR 2012.0`; and
5. standalone `<dateFiled>YYYY/MM/DD</dateFiled>` metadata followed by `\n`.

Text and `select-one` controls use the direct source concatenation
`<div>ID=VALUEID=</div>\n`. Radio and checkbox controls use the same framing
with the JavaScript boolean `checked` value. The source does not perform XML
escaping in this loop.

After closing the staging file, the source invokes `EncryptFile(xmlFileName)`.
The plaintext staging representation and the external encrypted file are
therefore distinct layers. The current contract identifies the resulting
encrypted Final Copy target; it does not claim that the plaintext staging
bytes are a user-facing or filing-safe artifact.

## Ordered occurrence evidence

The value-free decrypted audit contains exactly 159 occurrences and 159 unique
keys. `field_inventory_sha256` is
`245a84b2ff73b8b00ebb72f65b33be4fc5f15051cd562d9c9e0a363388ec33f1`.
Its order-sensitive `ordered_field_inventory_sha256` is
`b0c81408ca4e6afd61ada8d72ad61ca9833db7de958f2e772496e3c20405fd95`.
Its `keys` array preserves the observed occurrence order.

The first occurrence is `frm2550qv2024:calendarNo1`; the last is
`driveSelectTPExport`. The sequence includes unprefixed schedule controls and
runtime metadata controls. It excludes `dateFiled`, which the source appends
as a standalone metadata element after the final marker.

The reviewed sample happens to contain no duplicate pseudo-div key. That fact
does not authorize key-based sorting or deduplication: the governing source
rule is occurrence-preserving DOM order.

The separate plaintext Save review proves that these 159 occurrences are the
exact prefix of the pinned plaintext finalized save's ordered 160-occurrence
sequence; the sole plaintext suffix is pseudo-div `dateFiled`. This
cross-artifact relation is evidence for the reviewed samples, not permission
to collapse their different marker and metadata envelopes.

## Date and marker boundary

The marker content is source-established as
`All Rights Reserved BIR 2012.0`: `xmlClose.innerHTML` supplies
`All Rights Reserved BIR 2012.` and the function appends `0`.

`dateFiled` is generated from the local runtime clock as a zero-padded
`YYYY/MM/DD` string. It is not one of the 159 pseudo-div occurrences and must
not be projected from the lexicographically sorted v1 field inventory.

The exact bytes contributed by `xmlFormat.innerHTML`, the text-file encoding,
newline conversion, and any platform-specific character conversion are not
yet independently reviewed. Consequently the marker and date placement are
documented structurally, not encoded as executable literal or metadata nodes.

## Filename and encryption boundary

The source derives the staging filename from `existingXMLFileName` or
`fileName`, strips a directory prefix when present, and writes beneath
`IAF_RDO_Copy/`. When `txtFinalFlag == "3"`, it inserts
`#<globalEmail>#` before `.xml`.

The exact upstream filename construction, sanitization, overwrite policy,
path confinement, package-version behavior, helper input/output contract,
encryption envelope, error handling, and ciphertext determinism are not
reviewed. The source also ignores the `EncryptFile` exit code. No production
implementation may infer success merely because the helper was invoked or a
path was returned.

## Documented-only artifact decision

The artifact is present in the candidate contract so callers can distinguish
“official artifact observed but unavailable” from “artifact not inventoried.”
It has no serialization nodes and cannot be selected by materialization.

Executable translation is blocked until all of the following are bound:

- all 159 occurrence identities to lossless canonical inputs;
- exact value projections for text, select, radio, checkbox, schedule, and
  runtime metadata controls;
- raw-value escaping and delimiter behavior;
- exact separator, newline, and text encoding bytes;
- the final marker and standalone `dateFiled` nodes;
- a reviewed clock source and timezone policy;
- filename, overwrite, and path-confinement rules; and
- an independently reviewed encryption provider and failure contract.

The test-only candidate currently models 32 validation fields, not the full
159-control serialization surface. Partial node emission would silently drop
official state and is therefore forbidden.

## Filing-safe unresolved

No filing-safe decision approves the plaintext staging format, raw unescaped
values, generated filing date, filename construction, overwrite behavior,
external encryption, encrypted container, or artifact custody policy. The
filing-safe branch remains `unresolved`.

The reviewed registry remains empty. This review does not activate Save, Final
Copy, Upload, Submit, queueing, transport, release status, or any production
capability.
