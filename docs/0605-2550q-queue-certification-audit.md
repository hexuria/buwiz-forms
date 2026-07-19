# 0605:1999 and 2550Q:2024 queue certification audit

## Verdict

Queue/submission remains disabled for both exact revisions. The locked sources prove official form identity, editable-save field contracts, semantic XML replay, and decryption of application-produced companions. They do **not** prove a currently accepted electronic submission route, an exact application-writer payload, a durable pre-network claim path, or an authoritative BIR confirmation contract.

The production status therefore remains `manual/external` for filing. Local draft editing, saving, XML import/export, and HTML certification may continue without implying submission support.

## Locked source evidence

| Form | Reviewed source | SHA-256 | Proven boundary |
| --- | --- | --- | --- |
| `0605:1999` | `0605version1999_09.02.2022_copy.pdf` | `de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f` | Official two-page 612 x 936 point form and instructions |
| `0605:1999` | `00000000000000-0605-01312026102841.xml` | `01992fcdaef50493e756b89728af8d107ec1a0cafa94e677edbac1e2f08dc499` | Exact 235-field editable-save map |
| `0605:1999` | encrypted companion for the preceding save | `09cd3626efd6a7490b5922c9dbb6fad98b0b066ffb5de87c3ea6a6677210620f` | Decryptable 235-field semantic companion with `txtFinalFlag=0` |
| `0605:1999` | `00000000000000-0605-12312025143024.xml` | `f8659d2011d2914073725ccef1fc4f2e74d4f315bf333d5ec3084a1fdff524f7` | Second exact 235-field editable-save map |
| `0605:1999` | encrypted companion for the preceding save | `c53a196dcfe1fb585fefc7b48c2a4f2abe9ec9114d55541e44a40e4399c39928` | Second decryptable 235-field semantic companion with `txtFinalFlag=0` |
| `2550Q:2024` | `2550Q  April 2024 ENCS_Final.pdf` | `18eb16925010fdda820cef958221ba2c0d073066efa93a898113e39b31135a25` | Official two-page 612 x 1008 point return |
| `2550Q:2024` | `2550Q guidelines April 2024_final.pdf` | `b6ee4f090cb48963a44b1ef58fd6cdb4b5865ba4674963c3661c7f164895b120` | Official one-page filing, rate, penalty, and attachment guidance |
| `2550Q:2024` | `00000000000000-2550Qv2024-122025Q1.xml` | `43577fdd70b8959b16dbada9ff7d8418a1fdc5d18e61302c8cbfc8e9bbab4520` | Exact 160-field editable-save map |
| `2550Q:2024` | encrypted companion for the preceding save | `57ccf9d8132c490d54bceaf5c55fc2b4bec01b780951a63600402c61a595cdbe` | Decryptable 160-field semantic companion with `txtFinalFlag=0` and standalone `dateFiled` |

The samples under `/Users/uriah/Downloads/forms` are retained outside the repository. Their hashes are locked in the Rust external-source tests; the 2550Q renderer provenance also records its PDF, guideline, and XML sources. The observed filename stems are evidence of those saves only. They do not establish a general filename grammar or an accepted remote folder.

## What the implementation proves

### 0605

- `Form0605Draft` recomputes Item 20D as 20A + 20B + 20C and Item 21 as 19 + 20D, matching the official form.
- The checked importer requires the complete 235-key source shape, rejects missing or contradictory selections, recomputes source totals, and round-trips both reviewed editable saves exactly at the field-map level.
- The encrypted companions decrypt with the reviewed BIR IAF passphrase and replay through the same semantic contract.
- Only four code/index pairs are source-proven: `FP010`/`AtcCode1`, `II011`/`AtcCode24`, `DO`/`TaxTypeCode4`, and `IT`/`TaxTypeCode9`. Other imported pairs remain review-only.
- Item 22 signatures and Part III payment details exist on the official PDF and persist in the local app draft, but neither reviewed 235-field XML source contains keys for them.
- The desktop editor saves a local Draft and explicitly refuses Submitted, Confirmed, Paid, and retry-state advancement.

### 2550Q

- `Form2550QDraft` implements the official 12% VAT arithmetic, Part IV-to-Part II carry-over, schedule totals, penalties, and the reviewed Item 26 rule where negative excess credit does not offset cash penalties.
- The checked importer requires the complete 160-field source shape, validates exact reviewed transport fields and formulas, and semantically replays both the plain save and decrypted companion.
- Schedule 1, Schedule 3, and Schedule 4 currently require exactly two rows because that is the only reviewed editable-save capacity. This is a source-bound limit, not evidence that another electronic writer or service accepts a different capacity.
- The desktop editor persists a local quarterly Draft and explicitly refuses electronic submission.

## Blocking gaps

### Payload and filename contract

The reviewed plain files are editable saves (`txtFinalFlag=1`). The encrypted companions use `txtFinalFlag=0`; 2550Q also moves `dateFiled` to a standalone element. The tests prove decryption and semantic replay, but they do not prove that Rust reproduces the exact native writer bytes or that any generated payload is accepted by a BIR submission service. Blank `ebirOnline*` fields and `txtEnroll=Y` are observed values, not a certified credential or enrollment protocol.

### Transport route

The capability registry authorizes queue transport only for `2551Qv2018` and `1601Cv2018`. The background worker dispatches only those forms. Although the generic FTP helper accepts an arbitrary form-type string, no reviewed evidence establishes an accepted `/0605v1999`, `/0605`, or `/2550Qv2024` remote folder, current endpoint, transfer mode, service credentials, helper exit semantics, or server-side validation behavior for these revisions.

The generated `docs/efps_manifests/0605.json` and `docs/efps_manifests/2550Q.json` describe eFPS browser-form fields. They are useful formula and field-discovery evidence, but they are not evidence for the desktop eBIRForms encrypted-file queue.

### Durable submission state

Both forms can persist editable Draft JSON. Neither has the submission fingerprint, atomic pre-network claim token, stale-generation cancellation, immutable queued snapshot, or unknown-network-outcome reconciliation path implemented for the two queue-certified forms. Adding a background dispatch branch without those protections would permit duplicate or stale submissions.

### Confirmation and payment

No reviewed response artifact or deterministic parser proves that an upload for either form was accepted by BIR. FTP completion alone cannot be treated as Submitted or Confirmed. There is also no certified receipt/payment reconciliation path for these revisions. Consequently, injected later statuses must not advance the filing lifecycle.

## Evidence required before promotion

1. A hash-locked, authorized source package or captured native application workflow proving the exact submission payload, final-flag/date behavior, filename grammar, and remote form identifier for each revision.
2. Authorized end-to-end evidence showing the accepted endpoint, transfer mode, credentials/enrollment handling, and server response for `0605:1999` and `2550Q:2024`. A transport-success exit code is insufficient.
3. Authoritative BIR confirmation artifacts and a deterministic parser that distinguishes transmission completion, validation rejection, acceptance, and payment/receipt state.
4. Per-form immutable queue snapshots, field-map fingerprints, atomic claims established before network I/O, stale/cancel handling, unknown-outcome preservation, and support-assisted reconciliation tests.
5. Exact native-writer byte comparison or another reviewed proof that Rust output—not merely decrypted source semantics—is accepted for both revisions.
6. Only after all preceding evidence is green may the registry set `queue_submission=true` and expose the form type to the background worker.

## Reproducible checks

The audit passed these focused checks against the locked source folders:

```text
cargo test --locked -p bir-core form_0605 -- --nocapture
# 26 passed, 1 ignored

cargo test --locked -p bir-core form_2550q -- --nocapture
# 14 passed, 1 ignored

EBIRFORMS_0605_SOURCE_DIR=/Users/uriah/Downloads/forms/0605 \
  cargo test --locked -p bir-core \
  locked_external_pdf_plain_and_encrypted_sources_match_and_semantically_replay \
  -- --ignored --nocapture
# 1 passed

EBIRFORMS_2550Q_SOURCE_DIR=/Users/uriah/Downloads/forms/2550Qv2024 \
  cargo test --locked -p bir-core \
  locked_external_sources_match_hashes_and_roundtrip_all_160_fields \
  -- --ignored --nocapture
# 1 passed
```
