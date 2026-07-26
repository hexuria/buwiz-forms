# Portable validation-rule evidence packets

Portable evidence packets let an offline observation workstation return
derived, reviewable facts without copying the official package, credentials,
taxpayer values, or anything capable of online submission into the repository.
They are an evidence transport format, not a promotion mechanism and not a
source of runtime authority.

The machine contract is
[`evidence-packet-v1.schema.json`](evidence-packet-v1.schema.json). The Rust
implementation is in `crates/bir-rules-codegen/src/evidence.rs`.
Deterministic construction of the reviewed 43-packet set is a separate
fail-closed layer documented in
[`evidence-packet-set-factory.md`](evidence-packet-set-factory.md).

## Boundary

A packet contains exactly:

- canonical `evidence-packet.json`;
- one or more canonical JSON files beneath `derived/`; and
- metadata for one or more upstream files beneath an optional, separately
  controlled vault.

Upstream bytes are never packet members. Derived documents use the fixed
classification `non-taxpayer-derived` and media type `application/json`.
Manifest and derived JSON must be exact `bir-json-c14n-v1` bytes: UTF-8 without
a BOM, sorted object keys, no insignificant whitespace, and no trailing
newline. Every internal path is normalized, relative, and uses `/`.

The manifest binds the exact candidate and official identities separately:
`rule_set_id`, `form_code`, `form_revision`, `official_package_version`, and
`official_package_evidence_id` are all required. The package evidence ID must
resolve to an entry in `upstream_evidence`; its hash and size therefore bind
the version claim to external vault bytes without putting those bytes in the
packet. `form_id` remains the portable corpus directory identity and is not a
substitute for any of those exact fields.

`tracked_v1_source_set_sha256` is always a real 64-hex digest of the tracked v1
evidence source set. It is distinct from `rule_set_source_state`. A rule set
that has not been derived is represented as
`{"status":"planned","source_set_sha256":null}`; only an actually pinned rule
set uses `{"status":"pinned","source_set_sha256":"<64-hex>"}`. In particular,
the planned state gives pre-v2 forms a machine-readable identity without
fabricating a runtime source-set digest.

`capture_provenance` is also mandatory. It records the full capture-tool commit,
capture-tool version, exact argument vector, Windows version, official
application version, and start/finish UTC timestamps. `command_argv` preserves
argument boundaries; it is not a shell-requoted command string. Packet v1
accepts only the exact four-argument portable
`bir-rules-codegen verify-evidence-vault-source-map --source-map ../...json`
invocation. `source_map_sha256` and `source_verification_sha256` bind its exact
input and independently computed result directly in the manifest. The
operating system is closed to `windows`, the finish must not precede the start,
and provenance strings and arguments are subject to the same non-sensitive
boundary as the rest of the manifest.

Every derived entry has one closed `kind`:

- `source-excerpt`;
- `structured-dom-inventory`;
- `xml-inventory`;
- `runtime-exact-errors`, `runtime-validation-order`, or
  `runtime-save-reopen`;
- `record-census`; or
- `gap-report`.

Its tagged `observation` is exactly one of `observed`, `not-observed`, or
`gap`. The latter two require a non-empty reason and carry no fabricated source
locator. An observed `source-excerpt` must provide `source_excerpt`, which
binds a non-empty byte range and excerpt SHA-256 to the path, size, and SHA-256
of one declared upstream file. Its `full_file_path` is the same portable
`upstream/...` vault-relative path declared by that entry; drive letters, UNC
paths, and other machine-local locators are rejected. Other kinds must leave
`source_excerpt` null.
This makes unavailable evidence explicit without turning an absence into an
official-behavior claim.

The verifier rejects:

- duplicate or unknown JSON keys, noncanonical bytes, and unknown enum values;
- absolute paths, `\`, empty/`.`/`..` components, path escape, and symlinks;
- undeclared packet files, duplicate identities, unsorted inventories, and
  references to an unknown upstream evidence ID;
- a missing exact form/rule-set/package/verifier identity, a package evidence
  ID that does not resolve, incomplete capture provenance, non-verifier or
  unsafe argv, an impossible Gregorian UTC date, or reversed capture
  timestamps;
- a non-digest tracked v1 source set, an unknown rule-set source state, a
  non-null planned digest, or a non-digest pinned source set;
- an unknown derived kind, a reasonless `not-observed`/`gap`, or a source
  excerpt whose full-file locator differs from its upstream declaration;
- a size, SHA-256, or packet-digest mismatch;
- missing or false `derived-only`, `no-taxpayer-values`, `no-credentials`, or
  `no-online-submission` attestations; and
- derived JSON shaped like raw/field values, taxpayer identity values,
  credentials, request/payload/transport data, or an online submission URL.

`packet_digest_sha256` is SHA-256 using the existing length-framed digest
construction with domain `bir-evidence-packet-digest-v1`. Its entries are the
canonical manifest with `packet_digest_sha256` replaced by the empty string,
followed by every derived file in path order. The normalized manifest binds all
upstream hashes and sizes, review state, and attestations without creating a
self-referential hash.

## Commands

From the repository root:

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  verify-evidence --packet tmp/packet

rtk cargo run --locked -p bir-rules-codegen -- \
  verify-evidence --packet tmp/packet --vault tmp/upstream-vault

rtk cargo run --locked -p bir-rules-codegen -- \
  import-evidence --packet tmp/packet --staging-root tmp/evidence-import

rtk cargo run --locked -p bir-rules-codegen -- \
  stage-form --form-id 2550q-v2024 --staging-root tmp/form-work
```

CLI paths are ordinary OS paths so a removable or network volume can be used.
Only paths stored inside the packet must use the portable relative syntax.
`--repo-root` is accepted by the two staging commands when repository discovery
is not appropriate.

`verify-evidence` is read-only. Without `--vault`, successful output always
reports `full upstream verified: false`; absence of controlled upstream bytes
must never be mistaken for complete verification. With `--vault`, every
declared upstream byte count and hash must match or the command fails.

`import-evidence` accepts only a `reviewed` packet whose every derived file is
also `reviewed`. It copies only declared derived files, creates the staging root
itself, refuses any existing target, and rejects a target beneath canonical
`rules/`. It never writes to the corpus.

`stage-form` mirrors `rules/forms/<form-id>/` beneath a fresh staging root. This
is the portable starting point for a builder or observation workflow. It
refuses an existing destination and any destination under canonical `rules/`.
Moving reviewed output from staging into the corpus remains a separate human
review and source-pin transaction.

## Review states

`candidate` is verifiable but cannot be imported. Candidate review metadata is
null. `reviewed` and `rejected` require a reviewer and exact UTC decision
timestamp. A rejected packet remains verifiable for audit history but cannot be
imported. Nothing in this format changes a v2 rule set's candidate status,
populates the reviewed registry, supplies artifact nodes, or authorizes Final
Copy or filing.
