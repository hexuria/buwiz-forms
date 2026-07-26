# Deterministic evidence packet-set factory

The Phase 1B factory projects the tracked v1 corpus into reviewed, value-free
portable packets. It does not observe the official application, execute an
official binary, create a v2 rule set, change a review status, or make anything
filing-capable. Real packet output is intentionally not tracked until the
review workflow described in the execution plan is complete.

The implementation is
`crates/bir-rules-codegen/src/evidence_set.rs`. Its closed supporting schemas
are:

- [`evidence-review-ledger-v1.schema.json`](evidence-review-ledger-v1.schema.json);
- [`evidence-review-scaffold-request-v1.schema.json`](evidence-review-scaffold-request-v1.schema.json);
- [`evidence-vault-source-map-v1.schema.json`](evidence-vault-source-map-v1.schema.json);
- [`evidence-vault-capture-metadata-v1.schema.json`](evidence-vault-capture-metadata-v1.schema.json);
- [`evidence-vault-catalog-v1.schema.json`](evidence-vault-catalog-v1.schema.json);
- [`evidence-derived-summary-v1.schema.json`](evidence-derived-summary-v1.schema.json);
- [`evidence-packet-set-v1.schema.json`](evidence-packet-set-v1.schema.json).

All ledger and catalog files must already be exact `bir-json-c14n-v1` bytes.
The builder does not supply dates, usernames, reviewer identities,
attestations, capture arguments, or review decisions. Those facts come only
from the reviewed ledger.

## Candidate review, digest planning, and reviewed construction

Review is a three-step state transition. First, a candidate ledger has
candidate/null packet review metadata, candidate per-derived-file statuses,
explicit collector attestations, and a null expected digest. Produce it from
the exact external request rather than hand-authoring derived identities or
censuses:

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  scaffold-evidence-review-ledger \
  --input /review/scaffold-request.json \
  --vault-catalog /review/vault-catalog.json \
  --output /review/candidate-ledger.json --dry-run
```

The request contains exactly 43 entries in `rules/index.json` order and
supplies capture provenance, timestamps, attestations, excerpts, and gaps
explicitly. The scaffold recomputes form/rule-set identities, v1 source
digests, censuses, and vault bindings. It has no reviewer, review-status, or
expected-packet-digest input, so it cannot approve its own output. After
inspecting the dry-run digest, rerun to a fresh external output without
`--dry-run`.

The candidate staging command then writes a fresh external packet that can be
inspected:

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  stage-evidence-packet-review --form-id 1701q-v2018 \
  --review-ledger /review/candidate-ledger.json \
  --vault-catalog /review/vault-catalog.json \
  --output-root tmp/1701q-review
```

This packet is non-authoritative. `import-evidence` rejects it, and
`check-evidence-packet-set` rejects any set containing it. After inspecting the
derived files, the reviewer changes the packet and every derived-file status
to `reviewed`, supplies the reviewer and decision timestamp, and leaves
`expected_packet_digest_sha256` null for one no-write planning pass:

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  build-evidence-packet --form-id 1701q-v2018 \
  --review-ledger /review/ledger.json \
  --vault-catalog /review/vault-catalog.json \
  --output-root tmp/packet-plan --dry-run
```

The dry run prints the exact final reviewed packet digest but creates no
directory. The reviewer binds that digest in the reviewed ledger and reruns
without `--dry-run`. A normal build rejects candidate status and any null,
fake, or stale digest. At no point does the tool change a review state,
attestation, timestamp, or ledger.

The 43-packet command follows the same protocol:

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  build-evidence-packet-set \
  --review-ledger /review/ledger.json \
  --vault-catalog /review/vault-catalog.json \
  --output-root evidence/validation-rules/packets/v1
```

The target must not exist. Construction reserves that exact fresh directory,
writes every file with create-only semantics, and verifies the complete result
through the packet checker. It never replaces or removes an existing path. If
construction fails, the partial fresh target remains for explicit inspection
instead of being deleted by pathname. Output beneath `rules/`, overwrite, path
escape, hard links, symlinks, and Windows reparse points are rejected.

## Source-set digest

`tracked_v1_source_set_sha256` is a real length-framed digest with domain
`bir-tracked-v1-source-set-v1`. Its entries are the form directory's tracked
v1 evidence files in portable path order:

- every JSON input is duplicate-key checked and hashed as canonical
  `bir-json-c14n-v1`;
- Markdown evidence is UTF-8 with BOM removed and line endings normalized to
  LF; and
- `README.md`, `HANDOFF.md`, and `v2-*` review documents are excluded because
  they are not v1 evidence inputs.

The required core is `manifest.json`, `fields.json`, `validations.json`,
`calculations.json`, `workflow.json`, `gaps.md`, and
`fixtures/negative-cases.json`. The ledger must bind the computed digest.

For a form with no v2 snapshot, the ledger supplies the planned rule-set ID and
the packet records `{"status":"planned","source_set_sha256":null}`. If an
exact v2 snapshot exists, the factory accepts only its real tracked rule-set ID
and pinned source-set digest. It never derives or invents a runtime digest.

## Vault catalog and upstream identity

The factory never reads `official_assets.path` from a v1 manifest. The shared
closed disposition policy divides every declared asset into `acquirable`,
`zero-size-provenance`, or `metadata-only-taxpayer-payload`. Only acquirable
official/runtime assets are matched to the external catalog by
`(sha256, size_bytes)` and deduplicated by that tuple. A catalog path must be
exactly:

```text
upstream/sha256/<first-two-hex>/<full-sha256>
```

Every catalog entry carries the exact source-map digest and verifier-result
digest accepted by acquisition. The catalog must use one capture session and
one exact map/verifier/provenance binding. Each reviewed ledger entry must
attribute its selected upstream entries to that same session, both digests,
and canonical provenance; the packet builder checks this again rather than
trusting the scaffold. Both digests are first-class packet-manifest fields.
The derived summary repeats the session ID, both verifier digests, and the
canonical provenance digest so summary-only review retains the attribution.
Provenance must name the exact portable source-map verifier invocation. Drive
paths, UNC paths, `file:` locators, and macOS user/volume paths are rejected.
Zero-size provenance records and declared dummy save/final-copy/taxpayer-shaped
assets are never read or admitted to the vault. Their asset ID, kind, hash,
size, disposition, and exact manifest locator remain bound in the value-free
summary and an explicit generated gap.

## Value-free projection

The summary carries only stable identifiers, one-based ordinals, JSON
pointers, source references, hashes, sizes, counts, and explicit gap reasons.
It never copies raw/default/example values, taxpayer identity, fixture payload,
credentials, transport requests, or official prose.

The mandatory sections are:

- capture sessions, reviewed source-excerpt locators, and capture gaps;
- a field/control ID projection for DOM review;
- XML/serialization evidence with `values_emitted: false`;
- runtime-observed validation IDs separated from source-derived order;
- workflow state/action identifiers for save/finalize/reopen review; and
- field, validation, calculation, workflow, serialization, fixture, and
  explicit-gap censuses.

An explicit `serialization-binding-inventory-v796.json` produces observed,
ordered key occurrences. Without that fixture, field keys are labeled
`field-key-projection`, every occurrence is unobserved, the manifest-declared
serializable count is retained, and any count delta is an explicit gap. For
example, 1701Q's 172 field keys are not silently equated with its 173 declared
runtime-serializable elements.

## Aggregate layout and check

The set root contains only:

```text
packet-set.json
<form-id>/evidence-packet.json
<form-id>/derived/...
```

`packet-set.json` preserves `rules/index.json` order and binds its canonical
hash, ordered identity digest, every packet digest, and every packet-manifest
hash. The aggregate digest uses domain
`bir-evidence-packet-set-digest-v1`.

```text
rtk cargo run --locked -p bir-rules-codegen -- \
  check-evidence-packet-set \
  --packet-root evidence/validation-rules/packets/v1

rtk cargo run --locked -p bir-rules-codegen -- \
  check-evidence-packet-set \
  --packet-root evidence/validation-rules/packets/v1 \
  --vault /approved/vault --json
```

The check is read-only. It rejects extra, missing, duplicate, reordered, or
drifted packets and invokes the packet verifier for every member. Without
`--vault`, successful output always reports
`full_upstream_verified: false`. With `--vault`, every content-addressed byte
count, full-file hash, and reviewed excerpt hash must verify.
