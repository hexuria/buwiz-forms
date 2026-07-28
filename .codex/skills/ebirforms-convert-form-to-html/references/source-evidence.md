# Source and Payload Evidence

## Lock exact identity first

Cross-check the form code and revision in all available evidence:

- the revision printed on the official PDF;
- the official URL and downloaded-byte SHA-256;
- the Rust `fileable_form_type_id` or provider identity;
- the XML field prefix and eFPS manifest identity;
- any source-pack metadata.

A generic source name such as `form_0605.rs` does not prove a requested
revision. Stop on mismatches. In this repository the available payment form is
`0605v1999`; requesting `0605v2018` must fail closed.

## Inventory the whole source pack

For this repository, start from `/Users/uriah/Downloads/forms/<exact-form-folder>`
when it is available. The directory is operator-local calibration evidence and
must never become a runtime resource. Inventory it before selecting a PDF or
XML sample:

```sh
rtk python3 scripts/reference/inventory_form.py \
  --repo . --form-code 0619F --revision 2018 \
  --source-dir /Users/uriah/Downloads/forms/0619F --output -
```

Enumerate and hash every PDF, plain save payload, encrypted submission
companion, guideline, eFPS manifest, and screenshot. Do not select a canonical
file from its name alone. Record PDF page count, MediaBox, CropBox, rotation,
and whether auxiliary PDFs are conditional attachments rather than pages of the
main return.

When both plain and encrypted payloads exist, decrypt the companion with the
repository's established reader and compare complete key sets. Extra encrypted
keys are a model/XML coverage blocker; never discard them because a smaller
plain sample happens to parse.

## Payload parsing policy

BIR pseudo-XML payloads may contain URL-escaped values and repeated wrapper
text. Preserve the raw bytes and source hash, decode deterministically, record
duplicate-key behavior, and inventory repeatable numeric suffixes. Suggested
types and schedule groupings are review hints only.

A payload proves that a field key and sample value occurred. It does not prove:

- the complete schema;
- maximum capacity;
- whether a value is user-entered or computed;
- a tax formula or applicability rule;
- conditional attachment behavior.

## eFPS manifest policy

Use reviewed eFPS manifests for save keys, declared constraints, profile-prefill
hints, and explicit computed expressions. A `computed` or `readonly` flag alone
does not prove the formula. Prefer official instructions, eFPS formula source,
or corroborated varied samples, and cite the evidence in Rust.

If formula, field coverage, or exact revision evidence remains incomplete, keep
the form `ScaffoldOnly`/`disabled` and state the missing evidence. HTML layout
work must never turn incomplete tax behavior into a fileable claim.
