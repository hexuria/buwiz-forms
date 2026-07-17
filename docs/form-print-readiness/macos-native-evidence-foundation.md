# macOS native-output evidence foundation

Status: development diagnostic only. This foundation does not enable an HTML
production route, create or merge a PDF, promote a capability, or make a form
release-ready.

The `bir-print` feature `native-output-evidence` exposes a pure transcript
verifier for evidence collected around the existing macOS
`PreparedHtmlPreview` and same-`WKWebView` PDF path. The feature is disabled by
default and is intended for development and evidence tooling only.

## What the verifier cross-checks

`DevelopmentNativeOutputTranscriptV1` records and the verifier independently
cross-checks:

- canonical source-revision syntax and equality to the collector-observed
  revision;
- a twice-stable package tree and the renderer at the exact macOS package path
  `Contents/Resources/assets/form-renderer`, compared with a separately supplied
  offline bundle tree hash;
- the immutable render envelope, final output, and per-page WKPDF payload hashes;
- a deserialized envelope whose provider, pagination, and paper geometry match
  the supplied native plan and PDF expectation, with the exact supported render
  contract version;
- one opaque WebView run identity across preparation, preflight, and WKPDF;
- exactly two identical renderer geometry reports;
- page count, paper geometry, page rectangles, overflow, clipping, and hidden
  value counters through the existing renderer-geometry validator;
- exactly one consumption of a non-zero output nonce, followed by a completion
  bearing that same nonce;
- one successful, non-empty WKPDF callback payload for every expected page;
- final PDF structure and evidence through the existing `lopdf` validator;
- equality between each final PDF page dictionary and its reachable normalized
  object graph and the recorded WKPDF page, including boxes, rotation,
  resources, annotations, and streams rather than only page content;
- fail-closed rejection of catalog `AcroForm`, `OCProperties`, `OutputIntents`,
  `OpenAction`, `AA`, and `ViewerPreferences`, plus streams backed by external
  file data, because those can affect rendering, printing, or actions outside
  the page-reachable graph;
- consistency of a failed-export collector record in which the pre-existing
  destination bytes are unchanged and the collector reports no remaining
  temporary file.

The verifier reads real artifacts and PDF bytes supplied by a collector. The
final output is opened once as a regular, non-symlink file; that owned byte
snapshot is used for hashing, `lopdf` validation, and WKPDF-page binding. It
does not launch a WebView, perform printing, write an export, synthesize runtime
evidence, or repair incomplete evidence. Test-only helpers create small PDFs
and exercise the real merge/finalize failure-cleanup path solely to test
rejection and acceptance behavior.

These cross-checks do not prove that the hashed bundle and envelope causally
produced the supplied WKPDF pages in the claimed WebView. The artifacts can be
constructed independently and still be internally consistent; the positive
unit fixture intentionally uses small synthetic blank PDFs to test verifier
mechanics, not renderer provenance. Causal bundle + envelope + same-WebView
output remains a blocking, non-promotional requirement until an attested
runtime collector records the complete run.

Source-revision provenance, the independently expected offline bundle hash, and
the collector's failure-observed and temporary-cleanup statements are not
independently established by this pure verifier. They remain collector/CI
responsibilities, which is one reason the result cannot be promotional.

## Deliberate trust boundary

The transcript scope is permanently `DevelopmentDiagnostic`, and verification
rejects any transcript that sets `promotion_eligible` to true. A successful
verification result is a consistency check, not native release evidence.

Do not add these transcripts to
`packages/form-specs/form-release-evidence.json`, register their producer as a
trusted platform evidence producer, or change a form capability or
`release_ready` value from this verifier alone.

## Development runtime observation slice

The native host now retains the two separately measured, byte-for-byte
identical geometry observations instead of retaining the second observation
only. Both reports are validated independently, bound to the immutable
document run ID, envelope hash, render epoch, readiness revision, and one-use
output nonce, and retained in the backend binding.

When the desktop is built with `dev-tools`, setting
`EBIR_NATIVE_OUTPUT_EVIDENCE_DIR` opts in to a local diagnostic write after a
successful direct PDF export. The host writes the exact immutable envelope and
a `DevelopmentNativeOutputObservationV1` JSON file outside the renderer bundle.
On macOS it records the actual WKPDF callback byte count and SHA-256 for every
page; on Windows it explicitly records that WebView2 exposes only the completed
PDF file. The observation also records the completed backend identity/epoch,
`lopdf` validation, final destination hash, runtime renderer-bundle hash, and
whether the sibling temporary file remains.

The collector code and its `bir-print/native-output-evidence` dependency are
compiled only through the desktop `dev-tools` feature. In a default or release
build, setting `EBIR_NATIVE_OUTPUT_EVIDENCE_DIR` alone has no effect and cannot
write an observation.

This observation shape is intentionally different from
`DevelopmentNativeOutputTranscriptV1`, requires at least one concrete strict
verifier gap, rejects `promotion_eligible: true`, and cannot be deserialized as
the strict transcript. A cargo-run build records the package hash as unavailable
rather than treating the executable as a package.

The offline verifier now writes
`assets/form-renderer-build-identity.json` beside (never inside)
`assets/form-renderer`. It contains the deterministic renderer tree hash and a
canonical curated source revision only when the migration audit proves that
source set clean. A dirty developer checkout receives an explicit unavailable
source revision; the last commit is never mislabeled as the source of dirty
renderer bytes. Package recipes run the stricter
`npm run verify:forms:offline:package`, which fails unless that clean revision
is available before any assets are copied into an installer or app bundle.

The development runtime reads this separate build-time identity, hashes the
running renderer independently, and binds the source revision only when both
bundle hashes agree. Missing, malformed, symlinked, oversized, or mismatched
identity files remain explicit observation gaps. The identity is still
non-promotional: it is package content, not a package signature or an external
collector attestation, and it cannot establish the running package hash.

Example opt-in location:

```sh
EBIR_NATIVE_OUTPUT_EVIDENCE_DIR="$PWD/target/native-output-observations" \
  cargo run --locked --bin bir --features dev-tools
```

These files contain document-envelope data and must remain local development
artifacts. They must not be committed, copied into the signed package, or added
to `form-release-evidence.json`.

## Attested runtime collector still required

A later, narrow macOS collector must gather real runtime facts without changing
the output document:

1. Independently attest the opaque run identity already assigned when the
   prepared preview's Wry/WKWebView is created and retained through preflight
   and WKPDF completion.
2. Preserve the now-retained pair of actual stable geometry measurements in an
   independently attested collector rather than trusting the app-written
   diagnostic observation.
3. Retain each WKPDF callback payload and the validated final output as separate
   verifier inputs for page-reachable object-graph comparison; diagnostic
   hashes alone are not those artifacts.
4. Exercise a real failed export against a pre-existing destination and record
   destination preservation and temporary-file cleanup.
5. Resolve the canonical source, stable package tree, independently produced
   offline bundle tree hash, and immutable envelope artifacts used by that run.
   The build identity supplies a deterministic expected renderer hash and
   source revision to compare, but the collector must still attest the signed
   package containing that identity.
6. Write the transcript outside the signed application package and invoke this
   verifier as a separate diagnostic step.
7. Attest one causal run binding the package, independently expected renderer
   hash, immutable envelope, WebView identity, nonce, geometry observations,
   WKPDF callback bytes, and finalized output.

Only after the collector itself is attested and its transcript is independently
reviewed should a separate promotion design be considered. System-print proof,
network-denied packaged operation, signed-package identity, platform coverage,
and rollback evidence remain separate, incomplete gates.

## Verification

Use a workspace-local temporary directory:

```sh
TMPDIR=/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity/target/tmp \
  cargo test --locked -p bir-print --features native-output-evidence html_output_evidence

TMPDIR=/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-print-parity/target/tmp \
  cargo check --locked -p bir-print --features native-output-evidence

npm run verify:forms:offline

# Package builds require a clean curated source revision.
npm run verify:forms:offline:package
```
