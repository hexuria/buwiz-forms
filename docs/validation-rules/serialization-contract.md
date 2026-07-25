# Reviewed serialization contract

Status: the closed schema, fail-closed audit, canonical subtree digest,
generated borrowed Rust statics, sealed contract-owned materialization trace,
and independently rendered and parsed checked plaintext artifact are
implemented. Container stages and filing authorization are not implemented.
The generated registry remains empty because no reviewed form has a non-empty
contract. No form is authorized by this document, and no adapter may infer
executable behavior from the v1 inventory.

## Why a field map is insufficient

The 43-form v1 corpus contains 9,592 field records:

- 8,358 records have a concrete serialized key;
- 1,234 have no concrete key;
- 793 of the unkeyed records belong to 1601EQ and 1701Q, whose extraction has
  no occurrence mapping;
- the remaining 441 unkeyed records are explicit unbounded runtime-family
  descriptors across 14 forms.

The v2 compatibility key `legacy_v1.declared_counts.field_groups` preserves
that descriptor count. Each legacy descriptor becomes one unique member field
inside an unbounded v2 field group; the count does not measure the number of
logical group objects. For example, 2550Q has 28 descriptors organized as
seven logical row groups, so its declared count remains 28.

Five forms contain real repeated serialized keys. There are nine duplicate-key
groups and 25 total records:

- 1702Q: `frm1702q:txtTelNum` twice;
- 1707A: `frm1707Av2021:txtI11Email` twice;
- 2000: `frm2000:modLabel` four times;
- 2200A: five identity keys three times each;
- 2551Q: `txtEmail` twice.

Therefore a `BTreeMap<String, String>` is not a lossless representation.
Checked serialization must preserve global source order and the 1-based
occurrence number for each repeated key.

Document inventories also differ. Examples include 1702MX (210 editable
occurrences versus 588 final-copy occurrences), 1701 (837 versus 838), and
2550Q (160 editable pseudo-div occurrences versus 159 encrypted-final-copy
pseudo-div occurrences). For 2550Q, `dateFiled` is absent from the final-copy
pseudo-div inventory but is appended as a standalone metadata element. A
parser that verifies only `<div>` fields while treating every other element as
envelope noise cannot prove complete serialization.

## No universal codec

Direct source inspection finds selective JavaScript `escape(...)` branches in
all 43 exact-revision HTAs. The conditions name 170 form/key pairs, at least
136 of which map directly to current concrete keys. Raw writes coexist with
escaped writes in the same serializers.

Examples:

- ordinary 2550Q save/submit code applies JavaScript escape only to taxpayer
  name and address, while its encrypted `IAF_RDO_Copy` loop writes those
  values raw;
- 1702Q has a read-only branch whose `id !== A || id !== B` condition is
  always true and consequently escapes every text/select value;
- 1701MS source escapes only taxpayer name, contradicting the blanket
  "URL-encoded" notes attached to many extracted field records.

The term "URL-encoded" in prose is not enough to select UTF-8 percent encoding,
JavaScript `escape`, form encoding, or raw output. Codec behavior is executable
only when its exact artifact path and source reference have been reviewed.

## Required closed IR

Serialization belongs to the reviewed generated ruleset, not to a GPUI view or
an adapter-owned manifest. Each exact rule-set revision must expose profiled,
artifact-specific branches:

```text
SerializationContract
  contract_version
  artifacts[]
    artifact_id
    target
    variant_id
    official
    filing_safe
    source_refs
```

`target` is a closed enum. At minimum it distinguishes:

- editable save;
- Final Copy;
- submission payload;
- historical/import compatibility.

`variant_id` distinguishes source paths that have different behavior even when
they serve the same broad target. A serializer must never combine observations
from `saveXML`, `saveXMLsubmit`, an encrypted copy loop, or a legacy import path
without an explicit reviewed decision.

An executable artifact branch contains an ordered list of nodes:

```text
ArtifactNode
  PseudoXmlField
    ordinal
    key_projection
    occurrence_projection
    value_projection
    semantic_format
    body_codec
    presence
  MetadataElement
    ordinal
    exact_tag
    value_projection
    semantic_format
    body_codec
    presence
  ReviewedLiteral
    ordinal
    exact_bytes
  DynamicGroup
    ordinal
    group_id
    instance_order
    min_occurs
    max_occurs
    nodes[]
```

Every node is closed (`additionalProperties: false`) and source-bound.
`ordinal` is artifact-global. Duplicate ordinals, duplicate projected
key/occurrence pairs, gaps in per-key occurrence numbering, unresolved
branches, and overlapping dynamic projections are compile errors.

### Key and occurrence projection

`key_projection` is one of:

- an exact literal key;
- a reviewed group-indexed template with explicit index base, step, padding,
  prefix, and suffix.

`occurrence_projection` is explicit. It may be a fixed positive integer or a
reviewed group-instance projection. Runtime-generated keys may not be supplied
as arbitrary adapter strings.

### Value projection

The value source is selected by the contract:

- canonical field plus an exact singleton/group instance selector;
- derived output plus calculation/output and a required singleton,
  current-group, or stable-instance selector;
- versioned context value;
- reviewed constant;
- reviewed default.

The adapter supplies raw inputs and stable group instances only. It does not
declare which semantic value fed an occurrence.

### Presence

Presence is one of:

- `always`;
- `when`, with an executable typed predicate;
- `omitted`.

Generated values are represented by their value projection, not by a vague
presence label. An unresolved condition is not treated as false. The
materializer must account for every reviewed node, including explicit
omissions, and reject every uncontracted output node.

### Semantic formatting

Formatting typed values into serializer text is separate from body encoding.
The closed formatting layer must cover only reviewed modes, including:

- text and explicit blank/absent policy;
- boolean with exact true/false strings;
- base-10 integer;
- exact decimal with explicit scale, rounding, grouping, decimal separator,
  and negative representation;
- date with an exact closed pattern.

Type/format mismatch is a compile error. Formatting may not use locale,
floating point, or current machine settings.

### Body codec

Body encoding is independently selected per node and artifact variant:

- exact raw text;
- legacy JavaScript `escape` over UTF-16 code units;
- UTF-8 percent encoding with an explicit allowed-character set;
- another future closed codec only after its algorithm and evidence are added.

Ambiguous prose such as "URL-encoded" remains `documented_only` or
`unresolved`. It does not default to any executable codec.

Outer compression, encryption, signing, and transport framing are separate
versioned stages. Their proofs must bind the exact plaintext artifact digest;
they must not change field semantics.

## Runtime boundary

The packaged `CompiledRuleSet` exposes the selected static serialization
contract. The trusted flow is:

1. resolve the complete `FormRevisionKey`;
2. evaluate the exact raw/context snapshot under `FilingSafe`;
3. select the reviewed artifact variant;
4. materialize every ordered node from the trusted evaluation and stable group
   instances;
5. apply semantic formatting and the reviewed body codec;
6. generate the artifact bytes inside `bir-core`;
7. parse the result independently into the same ordered node representation;
8. require exact contract/materialization/parser agreement;
9. bind contract digest, variant, profile, request, context, bytes, and all
   occurrence records into the checked proof.

Steps 1 through 9 are implemented generically by the sealed `bir-rules`
materializer and `bir-core::form_rules::CheckedSerializationArtifact`.
`bir-core` does not accept the materializer's trace as self-authenticating: it
independently selects and walks the exact generated plan. A sealed
request/result-bound `SerializationInspector` re-evaluates conditional
presence for the exact group instance and resolves the complete value-source
identity, including both group ID and stable instance ID. Core compares those
facts with the materializer trace before it recomputes formatting and codecs,
renders the plaintext, and parses every byte back. The inspector caches one
validated canonical/derived snapshot for the bound request; it does not trust
adapter claims or repeatedly re-evaluate per record. The accounting manifest
also preserves contract omissions and zero-byte group records so deleting an
unrendered node cannot disappear from the proof. This stricter boundary is
domain-separated as `checked-serialization-artifact-v3`.

`CheckedFinalCopyPayload` must not accept adapter-supplied semantic and encoded
values as independent facts. It may retain an occurrence manifest as proof
output, but that manifest is produced by the trusted materializer.

Queue authorization re-resolves the exact registered ruleset and repeats the
trusted evaluation/materialization transaction. A stored hash is integrity
evidence, not authorization.

## Mandatory regressions

The implementation gate includes:

- omitted required occurrence;
- extra occurrence;
- duplicate occurrence;
- occurrence 1/2 of the same key;
- deletion or insertion in the middle;
- reordered distinct keys;
- reordered repeated keys;
- raw versus JavaScript-escaped divergence;
- literal percent versus percent escape;
- Unicode BMP and surrogate-pair JavaScript escape cases;
- semantic/encoded mismatch;
- wrong semantic formatter or value type;
- conditional presence true/false/unresolved;
- a forged `PresenceFalse` trace when the independent predicate is true;
- the same stable instance ID presented under the wrong repeated-group ID;
- zero, one, and multiple dynamic group instances;
- wrong group ordering or generated index;
- unexpected metadata element or envelope bytes;
- wrong artifact variant or behavior profile;
- stale input/context/ruleset/contract digest;
- outer-container plaintext digest mismatch.

Until a non-empty contract is emitted for a reviewed snapshot and its checked
plaintext artifact is bound through the reviewed Final Copy/container, queue,
and transport stages, those operations remain closed.
