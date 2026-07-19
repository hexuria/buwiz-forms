# Windows candidate certification collector/verifier foundation

Status: operator-only, untrusted foundation. It does not register a producer,
write form release evidence, change a route/readiness matrix, set
`release_ready`, or permit a tagged release.

This foundation consumes the exact three files uploaded by the manually
dispatched `html-candidate-certification.yml` Windows job:

- `candidate-manifest.json`;
- `eBIRForms-Windows-x64-<source-sha>.zip`; and
- `form-renderer-build-identity.json`.

The archive is the workflow's portable, non-development candidate: root
`bir.exe` plus `assets/`. The inspector rejects hash/source/renderer identity
mismatches, unsafe, case-colliding, Unicode-normalization-colliding, or reserved
Windows device paths, symlinks, encrypted members, oversized expansion, and
any unexpected MSI, MSIX, or Setup installer inside the portable archive.

## Inspect the exact candidate

Use a new output directory:

```powershell
python scripts/windows_candidate_certification.py inspect `
  --candidate-manifest C:\candidate\candidate-manifest.json `
  --candidate-archive C:\candidate\eBIRForms-Windows-x64-SHA.zip `
  --renderer-identity C:\candidate\form-renderer-build-identity.json `
  --output-dir target\windows-candidate-inspection
```

The resulting binding is explicitly non-promotional, untrusted, and
incomplete. It hashes the actual extracted package, `bir.exe`, renderer tree,
and packaged renderer identity.

## Probe the real non-development executable offline

Run the probe from an elevated Windows terminal. It requires all Windows
Defender Firewall profiles to be enabled, creates unique outbound block rules
for the exact extracted `bir.exe` and selected `msedgewebview2.exe`, launches
without `DEVELOPER_MODE` or native evidence environment variables, verifies
that the process remains alive, and always removes and re-checks both temporary
rules. Blocking only `bir.exe` is insufficient because WebView2 networking runs
in child runtime processes:

```powershell
python scripts/windows_candidate_certification.py probe `
  --candidate-manifest C:\candidate\candidate-manifest.json `
  --candidate-archive C:\candidate\eBIRForms-Windows-x64-SHA.zip `
  --renderer-identity C:\candidate\form-renderer-build-identity.json `
  --output-dir target\windows-candidate-probe `
  --webview2-executable "C:\Program Files (x86)\Microsoft\EdgeWebView\Application\VERSION\msedgewebview2.exe" `
  --timeout 5
```

Startup under an outbound block is not proof of preview, print, PDF export, or
rollback behavior. The probe report retains those gaps.

## Collect one real 2551Q Windows candidate exercise

`scripts/windows_candidate_collector.py` is the operator-only collector for the
closed attestation below. It is intentionally **not** a CI simulator: it must run
in an elevated, interactive x86-64 Windows session against the exact portable
candidate. The output directory must be new or empty; the collector replaces
its ACL with one current-user Full Control rule before retaining taxpayer-facing
screenshots or logs.

Before running it, prepare all of these independently reviewable inputs:

- an exact manifest/archive/renderer-identity triplet whose manifest-bound
  `bir.exe` already has a valid SHA-256 Authenticode signature and signed
  timestamp;
- the Rust-owned `verify_certification_pdf.exe` built from that same source
  revision;
- the Windows SDK x86-64 `signtool.exe`;
- the exact x86-64 `msedgewebview2.exe` used by the candidate;
- a reviewed external runtime witness executable capable of consuming the
  collector's fresh request and writing the closed observation described below;
- a separate exact-candidate rollback bundle containing every required case;
  and
- an online named printer selected as the Windows default, with PrintService
  Operational logging enabled and permission to complete a real two-page job.

Then run:

```powershell
python scripts/windows_candidate_collector.py `
  --candidate-manifest C:\candidate\candidate-manifest.json `
  --candidate-archive C:\candidate\eBIRForms-Windows-x64-SHA.zip `
  --renderer-identity C:\candidate\form-renderer-build-identity.json `
  --pdf-verifier C:\candidate-tools\verify_certification_pdf.exe `
  --signtool "C:\Program Files (x86)\Windows Kits\10\bin\VERSION\x64\signtool.exe" `
  --webview2-executable "C:\Program Files (x86)\Microsoft\EdgeWebView\Application\VERSION\msedgewebview2.exe" `
  --runtime-witness C:\candidate-tools\reviewed-runtime-witness.exe `
  --rollback-bundle C:\rollback\windows-rollback-bundle.json `
  --output-dir C:\evidence\windows-2551q-candidate `
  --printer "eBIRForms Certification Printer" `
  --automation-identity "DOMAIN\operator" `
  --allow-live-print
```

`--allow-live-print` only acknowledges that a physical/named-printer job may be
completed. Immediately before invoking **Print**, the collector asks the
operator to type the exact printer name again. A cancel, spool-only observation,
wrong/default-printer substitution, event without exactly two pages, or missing
Event 307 fails closed. If the certification printer produces a retained file,
pass it with `--printer-output`; otherwise the attestation retains a null output
hash while still requiring the real completed job.

The collector launches the manifest-bound `bir.exe` itself, removes development
and native-evidence environment variables, binds two exact-program outbound
Firewall rules, and verifies their cleanup. Windows UI Automation is restricted
to the launched PID. It must find the real `2551Q HTML Form Preview`, invoke the
exact **Export PDF** control, observe the new same-process native Save chooser,
replace a pre-existing challenged destination through the native confirmation,
and then invoke the exact **Print** control. Screenshots are local sensitive
evidence; do not upload them to a public artifact or issue.

### Fresh runtime-witness contract

After launch, the collector writes `runtime-observation-request.json` and pauses
so the reviewed witness can attach. The request contains a fresh raw challenge,
its SHA-256 hash, exact candidate hashes, PID, output path and destination-before
hash, printer, WebView2 executable hash, witness executable hash, and the only
permitted output path. The witness must write
`external-runtime-observation.json` during that same run with the closed scope:

```json
{
  "schema_version": 1,
  "scope": "external_windows_candidate_runtime_observation",
  "promotion_eligible": false,
  "trusted_producer": false,
  "collector_challenge_sha256": "<sha256 from request>",
  "witness_name": "<reviewed name>",
  "witness_version": "<reviewed version>",
  "witness_executable_sha256": "<exact supplied witness hash>",
  "candidate": "<exact candidate object from request>",
  "pid": 1234,
  "form": {"code": "2551Q", "revision": "2018"},
  "non_dev_build": true,
  "dev_tools_enabled": false,
  "started_at_utc": "<RFC3339 UTC>",
  "completed_at_utc": "<later RFC3339 UTC>",
  "document_run_id": "<immutable preview run>",
  "envelope_sha256": "<render envelope hash>",
  "preview_nonce": 17,
  "print_nonce": 18,
  "geometry_measurements": "<two identical complete 2-page measurements>",
  "export": "<same preview nonce, S_OK, success=true, challenged output hash/size>",
  "print": "<print nonce, S_OK, Succeeded, exact printer>",
  "webview2": "<exact executable/version/channel/scope and _7/_16 interfaces>",
  "dependencies": {"msvc_runtime_loaded": true, "webview2_loader_bound": true},
  "strict_verifier_gaps": [
    "runtime witness producer is not registered as trusted",
    "external UI Automation and printer evidence are required",
    "external exact-candidate binding is required"
  ]
}
```

The string placeholders above are explanatory only; the actual observation
uses the exact object/array schemas enforced by the collector. Missing fields,
extra fields, non-finite geometry, duplicate nonces, another PID/candidate,
another witness/runtime, or output bytes that changed after observation all fail
closed. The external witness remains untrusted and non-promotional.

### Separate rollback input

The rollback bundle scope is
`external_windows_candidate_rollback_bundle`. It must bind the exact six-field
candidate object, retain distinct equal-hash before/after snapshots for the
destination and draft, retain a `{"remaining": []}` temporary-file manifest,
and contain every one of the 24 cases enumerated by the attestation schema,
exactly once, with `passed: true` and a verified artifact record. It must exist
before collection; the collector never invents rollback results from the happy
path. Its `promotion_eligible` and `trusted_producer` values must both be false.

On success the collector writes the closed attestation, invokes
`validate_attestation`, and immediately invokes the strict verifier against the
original manifest/archive/identity, owned PDF verifier, and `signtool`. The
report still records `promotion_satisfied: false`; this command never updates
release evidence or readiness.

## Closed external attestation

The operator/collector formats are closed by:

- `packages/form-specs/schema/windows-candidate-certification-attestation-v1.schema.json`
- `packages/form-specs/schema/windows-candidate-certification-report-v1.schema.json`

One candidate-bound exercise must retain immutable records for:

1. Windows edition/build, OS and process architecture, session, elevation and
   process integrity level; x86-64 Windows UI Automation and the external
   automation identity.
2. The exact WebView2 runtime version/channel/architecture/scope, availability
   of `ICoreWebView2_7` and `ICoreWebView2_16`, loaded MSVC runtime, and bound
   WebView2 loader.
3. The actual non-development executable launched with exact-program Windows
   Defender Firewall outbound blocks for both `bir.exe` and the attested
   `msedgewebview2.exe`, followed by verified cleanup of both rules.
4. The 2551Q HTML preview, immutable envelope hash, document run identity,
   one-use nonce, and two identical two-page 612 x 936 point measurements with
   no clipping or overflow.
5. The visible **Export PDF** toolbar control, native save chooser, and
   WebView2 `PrintToPdf` `S_OK` plus success result for the same nonce.
6. WebView2 native `Print` `S_OK` plus `Succeeded`, and a completed two-page
   print job bound to a Windows PrintService Operational event 307, printer,
   job/document identifier, submission/completion time, EventRecordID, and
   output hash when the selected printer produces a retained file.
7. A two-page, non-empty 612 x 936 point PDF revalidated by
   `bir-print::html_output::validate_pdf_file`.
8. A valid timestamped Authenticode signature on the exact manifest-bound
   `bir.exe`, including signer/issuer/serial/thumbprint, code-signing EKU,
   SHA-256 file digest, RFC 3161 timestamp identity/time/digest, trusted chain,
   `Get-AuthenticodeSignature`, and `signtool verify /pa /all /v`.
9. An unchanged package tree, preserved pre-existing destination, unchanged
   draft snapshot, and no sibling partial-file leak.
10. Every rollback case required by the schema, covering the release-ready
    false and kill-switch paths; missing renderer assets; early/late renderer
    failures; readiness timeout; invalid geometry/PDF; destination, temporary
    file, draft, and package-tree rollback; missing/old WebView2 and missing
    `_7`/`_16` interfaces; WebView2 Print/PrintToPdf HRESULT/result failures;
    unavailable printer; invalid/missing-timestamp Authenticode; and Firewall
    cleanup.

Build the owned PDF verifier from the same source revision:

```powershell
cargo build --locked --release -p bir-print `
  --features native-output-evidence `
  --bin verify_certification_pdf
```

Then run strict verification on Windows, passing the Windows SDK `signtool.exe`:

```powershell
python scripts/windows_candidate_certification.py verify-attestation `
  --candidate-manifest C:\candidate\candidate-manifest.json `
  --candidate-archive C:\candidate\eBIRForms-Windows-x64-SHA.zip `
  --renderer-identity C:\candidate\form-renderer-build-identity.json `
  --attestation C:\evidence\windows-attestation.json `
  --pdf-verifier target\release\verify_certification_pdf.exe `
  --signtool "C:\Program Files (x86)\Windows Kits\10\bin\VERSION\x64\signtool.exe" `
  --report target\windows-candidate-certification-report.json
```

The verifier re-extracts the original archive, re-hashes its binary and
renderer, reruns the Rust-owned PDF verifier with the explicit `windows`
platform scope, queries live signature/printer/print-event state, and rejects
missing or changed artifacts.

## Windows distribution policy and current blockers

The candidate workflow currently creates an **unsigned portable ZIP**. The
strict Authenticode gate must fail for that exact archive. Signing its extracted
`bir.exe` afterward mutates the manifest-bound package and is rejected. That
unsigned portable ZIP cannot certify a later signed EXE/MSI release, and its
extracted tree cannot certify the installer-produced installed tree.

The tagged public release workflow is separate and currently requires:

- a timestamped, Authenticode-signed `bir.exe` payload;
- one signed Inno Setup EXE installer;
- one signed MSI installer; and
- no MSIX artifact in the public GitHub release.

MSIX is Store-only and remains blocked by its separate artwork/runtime/store
certification policy. This portable attestation must record
`msix_certification_claimed: false`,
`public_installer_certification_claimed: false`, and
`installed_payload_certification_claimed: false`; it cannot certify the Store
package, later signed public installers, or their installed payloads.

To make the strict candidate verifier pass in a future slice, the
non-publishing workflow must sign the exact portable `bir.exe` before hashing
and uploading it. Public installer evidence still needs a distinct final
installer-bound collector because installer creation and signing produce
different artifacts.

The operator collector is implemented, but no live Windows run is committed or
claimed. It cannot run successfully against today's unsigned workflow archive,
and it cannot substitute a mocked runtime observation, spool-only print record,
or afterward-signed extracted executable. The remaining Windows work is
external: construct the exact signed/timestamped portable candidate, provide the
reviewed runtime witness and separate rollback bundle, execute the collector on
real x86-64 Windows with WebView2 and the named printer, and review its strict
non-promotional report. Public EXE/MSI and installed-tree certification remain
separate.

Even a complete foundation report remains `promotion_eligible: false`,
`trusted_producer: false`, and `promotion_satisfied: false`. Never copy it into
`form-release-evidence.json` or use it to change a route/readiness flag.
