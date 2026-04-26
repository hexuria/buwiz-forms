# eBIRForms System Architecture & Reverse Engineering Notes

## 1. Introduction
This document consolidates findings from reverse-engineering the official eBIRForms Windows application. The original application is a native Windows app written in C++, with a network transport layer developed in Delphi/Pascal using the Indy/Synapse libraries. The objective of this research is to safely bypass the legacy UI and replicate the exact submission pipeline within our modern Rust architecture.

## 2. Legacy Application Architecture
- **Desktop Application (`BIRForms.exe`)**: Manages the UI and saves the unencrypted form data.
- **Data Storage**:
  - `savefile/`: Stores plaintext data in a custom pseudo-XML format.
  - `IAF_RDO_Copy/`: Stores the ZLib-compressed, AES-encrypted representation of the pseudo-XML payload.
- **Uploader Utility (`cFTPSend.exe`)**: Intercepts the encrypted payload and transmits it over FTP to the BIR servers.
- **Reference Proof**: `/Volumes/goldcoders/reverse-engineer-ebir-forms/EBIR_FORM_SENT.md` provides proof of this workflow via CLI `curl` tests.

## 3. The Custom Pseudo-XML Payload
The BIR application does not use standard XML. It instead uses a custom key-value layout, with URL-encoded values.
**Format**: `<div>key=url_encoded_valuekey=</div>`

### Rust Implementation Reference
- **File**: `bir/crates/bir-core/src/bir_xml.rs` (Lines 48-60)
- **Implementation**: The `generate_bir_xml` function replicates this exact structure. We serialize properties using a `BTreeMap<String, String>`.
- **Note on "Duplicated Fields"**: A concern was raised regarding duplicated fields (like `txtDateIssue`) in the payload causing email failures. Upon investigation, since the Rust implementation utilizes a `BTreeMap`, keys are inherently deduplicated. The `debug.log` simply contained multiple appended submission attempts, leading to the visual confusion of duplicate fields.

## 4. Encryption & Compression
- **Methodology**: The unencrypted pseudo-XML is ZLib-compressed and then AES-128 encrypted using the DCPcrypt2 library.
- **Rust Implementation**: Handled transparently by `bir/crates/bir-core/src/crypto.rs` using `compress_and_encrypt` and `decrypt_and_decompress`.

## 5. Transport Protocol & Email Triggers
- **Network Protocol**: FTP (Port 21). The destination server runs FileZilla Pro Enterprise.
- **Target Gateway**: `103.56.5.254:21`
- **Credentials**: Hardcoded as `uploadOnly` / `12birBIR`.
- **Target Route**: The file is stored under a directory matching the form type (e.g., `/2551Qv2018`).
- **Rust Implementation**: Managed by the async `suppaftp` engine in `bir/crates/bir-core/src/transport.rs` (Lines 29-63).

### The Email Confirmation Trigger Mechanism
The BIR backend monitors these FTP directories. When a file is successfully uploaded (receiving an FTP `226` response), a background decryption service processes it. 
**Crucially, the server relies on the filename itself to determine the recipient of the confirmation email.**

- **Expected Filename Structure**: `[TIN]-[FORM_TYPE]-[PERIOD]#[EMAIL]#.xml`
- **Resolution of the Missing Email Bug**: 
  Our background daemon in `bir/crates/bir-core/src/background_cron.rs` generated filenames using the `default_submission_filename` method in `bir/crates/bir-core/src/forms/form_2551q.rs`. Previously, this generated filenames without the email suffix (e.g., `010558054000-2551Qv2018-122026Q1.xml`). As a result, the BIR server successfully ingested the form but had no email address to send the receipt to. This has now been corrected to format the filename properly as `{tin}-2551Qv2018-{period_code}#{email}#.xml`.

## 6. Common Issues & Vulnerabilities in the Legacy App
When analyzing the legacy eBIRForms pipeline, we identified several major architectural and security flaws:

1. **Cleartext Hardcoded Credentials**: 
   The FTP authentication (`uploadOnly` / `12birBIR`) is heavily hardcoded within `cFTPSend.exe` and is transmitted in plain text over Port 21. Any local network observer or proxy could intercept the handshake.
   
2. **Cleartext Local Storage**: 
   The application saves unencrypted taxpayer data locally in `savefile/`. If the user's machine is compromised, sensitive financial and tax data is trivially exposed.
   
3. **Reliance on Filename for Routing**: 
   The backend relies solely on the filename string to route the receipt email (`#email#.xml`). This is brittle and theoretically allows an observer to spoof or hijack the confirmation email destination.
   
4. **Non-Standard Parsing**: 
   The use of a proprietary pseudo-XML structure (`<div>key=valuekey=</div>`) is highly susceptible to formatting errors, unlike standard XML or JSON, which have strict, predictable parsing standards.

## 7. Bugs Found in Our Rust Implementation

### Bug 1: Missing Email in `default_submission_filename` (FIXED)
- **File**: `bir/crates/bir-core/src/forms/form_2551q.rs` (Line 270-272)
- **Severity**: CRITICAL — This is the primary reason submissions were not generating confirmation emails.
- **Root Cause**: The `default_submission_filename()` method generated filenames without the `#email#` suffix. The BIR backend uses the filename to determine the confirmation email recipient.
- **Before**: `{tin}-2551Qv2018-{period_code}.xml`
- **After**: `{tin}-2551Qv2018-{period_code}#{email}#.xml`

### Bug 2: Extra `#` Delimiter in `official_import.rs` (FIXED)
- **File**: `bir/crates/bir-core/src/official_import.rs` (Line 70)
- **Severity**: HIGH — The "Import Official Savefile" pathway generated malformed filenames.
- **Root Cause**: The format string used `#` as a delimiter between form_type and period_code instead of `-`.
- **Before**: `{tin}-{form_type}#{period_code}#{email}#.xml` → e.g., `010558054000-2551Qv2018#122026Q1#email@.xml`
- **After**: `{tin}-{form_type}-{period_code}#{email}#.xml` → e.g., `010558054000-2551Qv2018-122026Q1#email@.xml`

### Bug 3: Missing Legacy Fields in App-Generated Payloads (INVESTIGATION)
- **File**: `bir/crates/bir-core/src/forms/form_2551q_xml.rs` (Lines 1-138)
- **Severity**: MEDIUM — Under investigation via bruteforce tests.
- **Root Cause**: The original eBIRForms application includes several trailing fields that our Rust implementation omits:
  - `txtFinalFlag=0` (or `1`)
  - `txtEnroll=Y`
  - `txtDateExpiry=` (empty)
  - `txtTaxAgentNo=` (empty)
  - `ebirOnlineConfirmUsername=` / `ebirOnlineUsername=` / `ebirOnlineSecret=`
  - `driveSelectTPExport=0`
- **Reference**: Compare `bir/fields.txt` (original decrypted payload) vs `form_2551q_xml.rs` output.

### Bug 4: `debug.log` Not Used in Production Code (INFO)
- **Severity**: LOW
- **Finding**: The `debug.log` file at the project root is NOT generated by any Rust production code. It was likely written by a manually-run test binary or debugging session. There is no `debug.log` writer in any of the bir-core source files. The production app uses `tracing` for structured logging instead.

## 8. Resubmission Behavior (Hypothesis Under Test)
The BIR backend appears to **silently drop duplicate filings** for the same TIN+Period combination. Evidence:
- The first submission of Q1 2026 via the `verify_submission.sh` script successfully triggered a confirmation email.
- All subsequent resubmissions of Q1 2026 (even with fresh `txtDateIssue` timestamps) upload successfully (FTP 226) but produce no email.

### Active Hypotheses:
1. **Filename collision**: The BIR backend may track processed filenames and skip duplicates.
2. **Period deduplication**: The backend may parse the payload and reject originals for already-filed periods.
3. **Amended flag required**: Resubmissions must explicitly set `amendedRtn_1=true` to be accepted as corrections.
4. **Missing legacy fields**: The backend may silently reject payloads missing `txtFinalFlag`, `txtEnroll`, etc.

### Testing Infrastructure
- **Location**: `bir/bruteforce/`
- **Script**: `bruteforce/bruteforce.py` — Autonomous test harness with SQLite DB, FTP retry, and IMAP email polling.
- **Database**: `bruteforce/bruteforce.db` — Tracks all experiment results.
- **Logs**: `bruteforce/logs/` — Stores per-run payload JSON, encrypted XML, and curl verbose output.

## 9. Cross-Reference: Proven Working Submission
- **Reference File**: `/Volumes/goldcoders/reverse-engineer-ebir-forms/EBIR_FORM_SENT.md`
- **TIN Used**: `010558054000` (corporate TIN)
- **Filename**: `010558054000-2551Qv2018-122026Q1#codeitlikemiley@gmail.com#.xml`
- **Base Payload**: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir-analyze/modified.json` — The JSON representation of the decrypted, proven-working payload from the original eBIRForms app.
- **Original Fields**: `/Volumes/goldcoders/reverse-engineer-ebir-forms/bir/fields.txt` — The raw decrypted pseudo-XML from the official savefile.
