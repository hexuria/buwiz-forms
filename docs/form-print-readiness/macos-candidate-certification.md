# macOS candidate certification collector/verifier foundation

Status: operator-only, untrusted foundation. It does not register a producer,
write release evidence, change a readiness matrix, set `release_ready`, or
permit a tagged release.

This foundation consumes the exact three files uploaded by the manually
dispatched `html-candidate-certification.yml` macOS job:

- `candidate-manifest.json`;
- `eBIRForms-macOS-universal-WORKFLOW_SHA.zip`; and
- `form-renderer-build-identity.json`.

It rejects any archive, source revision, renderer tree, or identity that does
not match those immutable manifest hashes. ZIP extraction rejects traversal,
symlinks, duplicate or case-colliding paths, oversized archives, and more than
one application bundle.

The workflow requires the same five Apple signing/notarization secrets as the
public release workflow. It imports exactly one Developer ID Application
identity for the configured team, signs with hardened runtime and production
entitlements, submits a temporary archive for notarization, staples and
validates the application, and only then creates the manifest-bound archive.
The job fails closed if any credential or verification step is unavailable.

## Inspect the exact candidate

Use a new output directory:

```sh
python3 scripts/macos_candidate_certification.py inspect \
  --candidate-manifest /path/to/candidate-manifest.json \
  --candidate-archive /path/to/eBIRForms-macOS-universal-WORKFLOW_SHA.zip \
  --renderer-identity /path/to/form-renderer-build-identity.json \
  --output-dir target/macos-candidate-inspection
```

The resulting binding is deliberately marked:

```json
{
  "promotion_eligible": false,
  "trusted_producer": false,
  "certification_complete": false
}
```

It is a candidate identity check, not platform evidence.

## Probe the actual non-development executable

On macOS, a second command launches the extracted release binary without the
development flag or development evidence environment variables. The process
runs under `sandbox-exec` with networking denied, must remain alive for the
requested interval, and must leave the package tree unchanged:

```sh
python3 scripts/macos_candidate_certification.py probe \
  --candidate-manifest /path/to/candidate-manifest.json \
  --candidate-archive /path/to/eBIRForms-macOS-universal-WORKFLOW_SHA.zip \
  --renderer-identity /path/to/form-renderer-build-identity.json \
  --output-dir target/macos-candidate-probe \
  --timeout 5
```

This probe proves only candidate startup under a local network-denial policy.
It does not prove that preview, Export PDF, the native save chooser, printing,
or rollback worked.

## Immutable external attestation

The external operator/collector format is closed by:

- `packages/form-specs/schema/macos-candidate-certification-attestation-v1.schema.json`
- `packages/form-specs/schema/macos-candidate-certification-report-v1.schema.json`

The attestation must retain immutable file records and observations for all of
the following in one candidate-bound exercise:

1. Accessibility permission and the automation identity.
2. A real non-development launch with networking denied.
3. The 2551Q HTML preview, immutable envelope hash, document run identity,
   one-use nonce, and two identical 612 x 936 point geometry measurements.
4. The visible toolbar **Export PDF** control and the native save chooser.
5. A completed native system-print job, including printer name and job ID.
6. A two-page, non-empty 612 x 936 point PDF carrying the expected Rust-owned
   form and envelope evidence.
7. Developer ID signing, Gatekeeper notarization acceptance, and a valid
   stapled ticket.
8. An unchanged application tree, preserved pre-existing destination,
   unchanged draft snapshot, and no sibling temporary-file leak.
9. Every rollback case: `release_ready_false`, `kill_switch`, `missing_assets`,
   `renderer_error`, `late_renderer_error`, `readiness_timeout`,
   `invalid_geometry`, `rejected_pdf`, `destination_preserved`,
   `no_temp_leaks`, and `draft_unchanged`.

The attestation must retain the exact deterministic output of the owned PDF
verifier. Build that verifier separately:

```sh
cargo build --locked --release -p bir-print \
  --features native-output-evidence \
  --bin verify_certification_pdf
```

Then perform strict verification on macOS:

```sh
python3 scripts/macos_candidate_certification.py verify-attestation \
  --candidate-manifest /path/to/candidate-manifest.json \
  --candidate-archive /path/to/eBIRForms-macOS-universal-WORKFLOW_SHA.zip \
  --renderer-identity /path/to/form-renderer-build-identity.json \
  --attestation /path/to/macos-attestation.json \
  --pdf-verifier target/release/verify_certification_pdf \
  --report target/macos-candidate-certification-report.json
```

The verifier re-extracts and re-hashes the workflow archive, re-runs the
specified `bir-print` PDF verifier against the exact exported PDF, and requires
its fresh stdout to match the retained verifier artifact byte-for-byte. It also
queries live Accessibility and printer state, re-checks the application
signature, runs Gatekeeper assessment, validates stapling, and rejects any
failure or unavailable prerequisite.

## Run the external candidate collector

`scripts/macos_candidate_collector.py` is the operator-only producer for the
closed attestation above. It does not build or modify the candidate. It:

- securely extracts and independently hashes the exact manifest-bound archive;
- requires Developer ID, Gatekeeper notarization, and stapling before launch;
- generates a fresh 256-bit challenge and supplies it with a new, private 0700
  observation directory to the disabled-by-default candidate observation sink;
- launches the non-development binary through `sandbox-exec` with
  `deny network*`;
- waits for the operator to open a real 2551Q HTML preview, captures the visible
  preview/toolbar, clicks the reviewed **Export PDF** location for that exact
  PID, observes and captures the native save chooser, and binds the exported
  bytes to the candidate's causal runtime observation;
- opens the native print dialog without clicking inside it, then requires the
  operator to select the named printer and deliberately complete the job;
- requires exactly one new completed CUPS job and the candidate's successful
  AppKit print callback observation;
- reruns the owned Rust PDF verifier and retains its exact output; and
- invokes `macos_candidate_certification.py verify-attestation` before reporting
  success.

The collector never prints implicitly. `--allow-live-print` is mandatory and
is an explicit acknowledgement that the operator intends to create a physical
printer job. Tests do not pass that flag and never open or print a live job.

```sh
cargo build --locked --release -p bir-print \
  --features native-output-evidence \
  --bin verify_certification_pdf

python3 scripts/macos_candidate_collector.py \
  --candidate-manifest /path/to/candidate-manifest.json \
  --candidate-archive /path/to/eBIRForms-macOS-universal-WORKFLOW_SHA.zip \
  --renderer-identity /path/to/form-renderer-build-identity.json \
  --pdf-verifier target/release/verify_certification_pdf \
  --rollback-bundle /path/to/macos-rollback-bundle.json \
  --printer 'Exact_CUPS_Printer_Name' \
  --automation-identity 'Terminal operator / macos_candidate_collector.py' \
  --output-dir /absolute/private/path/macos-candidate-collection \
  --allow-live-print
```

The output directory must be absent or empty, canonical, owned by the current
user, and mode 0700. App-written observation files are mode 0600 and contain no
paths or taxpayer fields. The external screenshots and operator artifacts may
contain visible form data, remain local, and must never be committed.
Replace `WORKFLOW_SHA` with the 40-character checkout SHA embedded in the
downloaded workflow archive name.

### Required rollback bundle

A production candidate exposes no fixture or fault-injection route. The
collector therefore refuses to invent rollback results and requires a separate
candidate-bound bundle with exactly this closed top-level shape:

```json
{
  "schema_version": 1,
  "scope": "external_macos_candidate_rollback_bundle",
  "promotion_eligible": false,
  "trusted_producer": false,
  "candidate": {
    "candidate_manifest_sha256": "<sha256>",
    "candidate_archive_sha256": "<sha256>",
    "source_revision": "<40-hex revision>",
    "app_tree_sha256": "<sha256>",
    "renderer_bundle_sha256": "<sha256>"
  },
  "integrity": {
    "destination_before": { "path": "...", "byte_count": 0, "sha256": "..." },
    "destination_after": { "path": "...", "byte_count": 0, "sha256": "..." },
    "draft_before": { "path": "...", "byte_count": 0, "sha256": "..." },
    "draft_after": { "path": "...", "byte_count": 0, "sha256": "..." },
    "temporary_files_manifest": { "path": "...", "byte_count": 0, "sha256": "..." }
  },
  "cases": [
    { "name": "release_ready_false", "passed": true, "artifact": { "path": "...", "byte_count": 0, "sha256": "..." } }
  ],
  "strict_verifier_gaps": ["rollback producer is not registered as trusted"]
}
```

`cases` must contain each of the eleven names listed earlier exactly once.
Before/after destination and draft snapshots must be distinct retained files
with equal hashes, and `temporary_files_manifest` must be exactly
`{"remaining": []}`. These artifacts must come from the reviewed failure-drill
producer; passing unit-test logs or hand-written success files is not evidence.

## Current operator-only blockers

The workflow can now produce the exact Developer-ID-signed, notarized, and
stapled archive that the strict verifier expects. That repository-side change
does not prove that such an artifact has been dispatched, downloaded, or
exercised; those are still external evidence steps.

The repository now contains the untrusted external collector, but no real
signed candidate run or reviewed rollback bundle has been collected by this
commit. Accessibility permission, an available physical printer, an exact
signed/notarized workflow candidate, and all eleven genuine rollback artifacts
remain external prerequisites. Windows and Linux certification remain separate
incomplete milestones.

Even when all foundation checks pass, the report remains
`promotion_eligible: false`, `trusted_producer: false`, and
`promotion_satisfied: false`. Do not copy it into
`form-release-evidence.json`; producer trust and any evidence-only promotion
must be designed and reviewed as a later, separate change.
