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

Even a complete foundation report remains `promotion_eligible: false`,
`trusted_producer: false`, and `promotion_satisfied: false`. Never copy it into
`form-release-evidence.json` or use it to change a route/readiness flag.
