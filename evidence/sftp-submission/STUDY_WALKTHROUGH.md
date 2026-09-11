# How we restored eBIRForms submission

Study notes for later. This is the process, not just the result.

Date: 2026-09-11
Worktree: C:\Users\uriah\Code\buwiz-forms-wt-sftp
Branch: codex/ebirforms-sftp-transport
Authority: Offline eBIRForms 7.9.6.1 at C:\eBIRForms

Companion technical report: EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md

## What working means now

1. Buwiz can log into BIR SFTP.
2. Buwiz can PUT an encrypted IAF file.
3. BIR sent a confirmation email for the dummy 1601C upload.

Email confirmation was not required to prove the transport. It arrived afterward and closes the loop: the file was accepted far enough for BIR to generate a receipt.

We still do not bake BIR passwords into source. Credentials come from tinDispatcherSFTP.php at runtime.

## Why the old code died

Buwiz crates/bir-core/src/transport.rs used to do this:

- TCP connect 103.56.5.254:21
- FTP login uploadOnly / 12birBIR
- CWD /{formType}
- STOR {filename}

That was a faithful copy of old cFTPSend.exe. Official 7.9.6.0 changed the story:

- Release notes: Improved security on File Submission, All Forms
- Branch code 3 digits -> 5 digits, so IAF prefix became 14 digits
- New binaries: ebfSFTP.exe, WinSCP.exe, WinSCPnet.dll
- Extracted VBS RenameAndSendFile now launches ebfSFTP.exe, not cFTPSend.exe

FTP to a dead IP cannot be patched into SFTP. We had to watch the current official client.

## Process, in the order we actually did it

### 1. Do not restart from scratch

The handover already knew:

- FTP :21 was dead
- ebfSFTP.exe wraps WinSCP
- Literal Carlo*TSSD2!018 is crypto material
- Host/user/pass are not hardcoded in BIRForms.exe
- Filename prefix is 14 digits

First job was to inspect the live install and the extracted HTA tree, not guess a new endpoint.

### 2. Inventory the official install

Path: C:\eBIRForms

Helpers are virtualized. Get-ChildItem can miss ebfSFTP.exe / WinSCP.exe. Direct open still works. We copied bytes with Python Path.read_bytes() into a local analysis folder and hashed them.

Useful files:

- BIRForms.exe 7.9.6.1
- ebfSFTP.exe + .config + PDB
- WinSCP.exe, WinSCPnet.dll
- Encrypt.exe (IAF payload crypto, unchanged)
- cFTPSend.exe (legacy, still on disk)
- proxy.ini
- IAF_RDO_Copy dummy ciphertext XML files
- Release Notes.txt for 7.9.6.0 / 7.9.6.1

### 3. Read the extracted HTA/JS/VBS, not the packed EXE

BIRForms.exe unpacks to %TEMP%\{GUID}\. Live tree used here:

C:\Users\uriah\AppData\Local\Temp\{0B33C1CE-21A8-44A1-8D91-28A10444A6A3}

Key files:

- js/environment.js — currEnvi=PROD, currVer=7.9.6.0, dispatcher bases, form folder map
- js/string-util.js — conService.initConConfig GETs tinDispatcherSFTP.php
- js/eBIRTools.vbs — RenameAndSendFile builds the ebfSFTP.exe command line
- forms/BIR-Form1601Cv2018.hta — Final Copy / Submit path

This is where we learned credentials are fetched, not stored.

Dispatcher URL:

    http://birgovph.com/tinDispatcherSFTP.php?t={9-digit TIN}&f={formType}&v=7.9.6.0

Backup in source: http://ws1.birgovph.com/... (404 in this session).
Version-check backup is ws2, which is a different PHP file.

JS stores:

- mode
- srv from det.server (ciphertext)
- sslport from det.SSLPort (990, unused by the SFTP shim)
- port from det.port (22)
- usr / pass (ciphertext)

DEV/UAT override the host ciphertext to a hardcoded blob that unwraps to ftp2.birgovph.com. PROD uses the dispatcher host as-is.

### 4. Watch how the file is named and encrypted

1601C createXMLFileName:

    {tin1}{tin2}{tin3}{branch}-1601Cv2018-{MM}{YYYY}.xml

Branch is 5 digits, so prefix is 14.

saveEncryptedProfile(true):

1. saveXML(true)
2. Write BIR XML into IAF_RDO_Copy/
3. Encrypt.exe "path" — this is the IAF payload crypto
4. Return absolute path of ciphertext

RenameAndSendFile then renames to:

    {stem}#{email}#.xml

Encrypt.exe did not change. Buwiz crypto.rs still matches it (T0081gP45sy0rd-To+R3m3m63r!@4/<>, SHA-256, DCPcrypt CBC + tail, zlib). The migration was transport-only.

### 5. Decompile ebfSFTP.exe properly

Do not stop at strings. We used dnfile for metadata, user-string heap, method RVAs, and IL.

Main argv:

    ebfSFTP.exe <host-cipher> <port> <user-cipher> <pass-cipher> <folder> "<local-file>"

VBS mode and SSLport are not passed.

Decrypt:

- Base64
- salt = 32 bytes
- IV = 16 bytes
- rest = AES-256-CBC PKCS7
- PBKDF2-HMAC-SHA1(password=Carlo*TSSD2!018, salt, 100000 rounds, 32-byte key)
- UTF-8 BOM stripped

Trap: the IL is easy to misread as 1000 rounds. 1000 produces invalid padding on live dispatcher ciphertext. 100000 works.

SFTPSend:

- WinSCP Protocol = Sftp
- SshHostKeyPolicy = GiveUpSecurityAndAcceptAny
- PutFiles(local, folder, false, Binary)
- optional HTTP proxy from proxy.ini if ProxyAddress is set (ours is empty)

Exit 0 = success. HTA treats nonzero as txtFinalFlag=2.

### 6. Hit the live dispatcher with dummy data

    GET http://birgovph.com/tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0

HTTP 200, JSON with single quotes. Same body for dummy TINs and for 1601C / 2551Q. Shared SFTP endpoint, not per-TIN.

Unwrapped non-secret fields:

- mode=2
- port=22
- SSLPort=990 (legacy, ignored)
- host = ebf2.bir.gov.ph
- DEV override host = ftp2.birgovph.com

Username/password were unwrapped only to prove the algorithm and log in. They are not in git.

SSH banner on :22:

    SSH-2.0-fzssh_1.1.5_FileZillaProEnterpriseServer_1.12.13

### 7. Replace Buwiz transport, keep queue semantics

Old queue rule is still correct:

- Open session before claim
- Claim immediately before irreversible upload
- Pre-upload failure stays unclaimed / retryable
- In-flight upload error is unknown-outcome; do not auto-retry (double-file risk)

We changed the session, not the claim machine:

- SubmissionTransport::open_session(form_type, tin)
- Dispatcher fetch + unwrap, or TEST_SFTP_* for local tests
- SSH password auth via russh
- SFTP PUT via russh-sftp to /{formType}/{basename}
- Host key: accept any, matching official client

Harness:

    cargo run -p bir-core --bin sftp_harness -- --dry-run
    cargo run -p bir-core --bin sftp_harness -- --live-connect
    cargo run -p bir-core --bin sftp_harness -- --live-put

Windows ARM64 build needs the usual VS + OpenSSL + clang path from this machine. That is environment, not protocol.

### 8. Prove it against BIR, dummy only

--live-connect:

    host=ebf2.bir.gov.ph
    port=22
    username_len=15
    password_len=18
    folder=1601Cv2018
    auth=ok
    sftp=ok
    cwd=/

--live-put of existing dummy ciphertext:

    payload_len=986
    upload=ok
    filename=00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml

Later: BIR confirmation email arrived. That is the official receipt path the HTA already waited for. Transport success is exit 0 / PUT ok; filing evidence is the email.

## How to re-run the study

1. Open official eBIRForms so the HTA extracts under %TEMP%\{GUID}\.
2. Read environment.js, string-util.js, eBIRTools.vbs, one form HTA sendEmail.
3. Copy ebfSFTP.exe by raw bytes; dump IL / user strings.
4. GET tinDispatcherSFTP.php with dummy 9-digit TIN.
5. Unwrap server / username / password with the PBKDF2+AES layout above. Print host/port only.
6. SSH to host:22, password-auth, request sftp, canonicalize(".").
7. PUT dummy IAF into /{form}/.
8. Wait for email if you want receipt proof.

Do not commit live passwords. Do not PUT a real TIN unless you intend to file.

## Code map

| File | Why it exists |
| --- | --- |
| crates/bir-core/src/transport.rs | Dispatcher, unwrap, SSH, SFTP PUT |
| crates/bir-core/src/background_cron.rs | Queue still claims after session open |
| crates/bir-core/src/official_import.rs | Import submit threads TIN through |
| crates/bir-core/src/crypto.rs | Unchanged IAF payload crypto |
| crates/bir-core/src/naming.rs | 14-digit TIN + #email# filename |
| crates/bir-core/src/bin/sftp_harness.rs | Dry-run / live-connect / live-put |
| evidence/sftp-submission/EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md | Evidence tables, hashes, IL notes |

## Commits on this branch

- 1eac0f70 Replace BIR FTP upload with dispatcher-backed SFTP
- cd8e5dc0 Prove live BIR SFTP login and dummy IAF PUT

## What we still do not know

- Exact FileZilla ACL / overwrite policy on retry of the same dummy name
- Whether backup dispatcher ws2.birgovph.com/tinDispatcherSFTP.php is live
- Host-key pin if we ever want to stop accepting any key
- Whether every form folder name in environment.js PROD map matches the SFTP chroot

Those are hardening, not blockers. Submission works.
