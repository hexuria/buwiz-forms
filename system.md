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
