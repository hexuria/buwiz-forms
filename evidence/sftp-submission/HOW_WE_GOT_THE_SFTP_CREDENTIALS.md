# How we got the SFTP credentials

Study note for later. This is the recovery path, not a password dump.
Passwords are git-ignored. Do not copy them into this file, a PR, or a commit.

Date: 2026-09-11
Branch: codex/sftp-on-pr39 (PR 41)
Authority: official Offline eBIRForms 7.9.6.x

Companion docs:

1. STUDY_WALKTHROUGH.md - full restore process
2. EBIRFORMS_SFTP_SUBMISSION_FINDINGS_2026-09-11.md - hashes, IL, protocol tables
3. README.md - short index

## What the credentials actually are

They are not a taxpayer password, not a Gmail app password, and not something
we invented.

Official eBIRForms no longer hardcodes the SFTP login. On Final Copy / Submit
it asks BIR for a short-lived connection blob, unwraps it, then hands host /
user / pass / port to ebfSFTP.exe / WinSCP.

Buwiz now does the same:

1. HTTP GET tinDispatcherSFTP.php
2. Unwrap the JSON fields
3. SSH password-auth to that host on port 22
4. SFTP PUT the already-encrypted IAF file into /{formType}/{filename}

The live host we recovered is ebf2.bir.gov.ph:22. Username length 15, password
length 18. Those lengths match the dummy PUT that later got a BIR confirmation
email.

## Why they were not in .env at first

Production default is still the dispatcher. .env.example only had blank
commented keys so git never received a password.

The local ignored .env later got a filled copy so this machine can point
straight at the same live endpoint without asking the dispatcher every time.
That file is git-ignored:

- C:\Users\uriah\Code\buwiz-forms-wt-sftp-on-pr39\.env
- C:\Users\uriah\Code\buwiz-forms\.env

If BIR_SFTP_HOST is set, dispatcher is skipped. Leave it blank to go back to
fetching fresh credentials at runtime.

## Recovery process

### 1. Watch the official client, do not guess

Old Buwiz FTP (103.56.5.254:21, uploadOnly) is dead. Official 7.9.6.x ships
ebfSFTP.exe + WinSCP instead of cFTPSend.exe.

Install used: C:\eBIRForms. Helpers are virtualized, so copy bytes with
Python Path.read_bytes() rather than trusting Explorer listings.

### 2. Read the extracted HTA/JS/VBS

BIRForms.exe unpacks under %TEMP%\{GUID}\. The submit path is:

- form HTA sendEmail() / Final Copy
- js/environment.js - PROD dispatcher bases and form folder names
- js/eBIRTools.vbs - launches ebfSFTP.exe with argv from conConfig
- ebfSFTP.exe - unwraps dispatcher fields, then WinSCP SFTP

BIRForms.exe itself does not construct SFTP user/pass. It receives conConfig
from the dispatcher.

### 3. Fetch the dispatcher with dummy data

GET http://birgovph.com/tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0

HTTP 200, JSON with single quotes. Same body for dummy TINs 000000000 /
123456789 and forms 1601Cv2018 / 2551Qv2018. Shared SFTP endpoint, not per-TIN.

Backup URL in the official client: http://ws2.birgovph.com/tinDispatcherSFTP.php

Returned fields (ciphertext except mode/port):

- mode
- server
- SSLPort (legacy, ignored)
- port
- username
- password

### 4. Unwrap the ciphertext

Official ebfSFTP.exe decrypts each field with:

- Base64
- 32-byte salt + 16-byte IV + AES-256-CBC PKCS7
- PBKDF2-HMAC-SHA1
- 100000 rounds (easy IL misread is 1000; 1000 fails live ciphertext)
- UTF-8 BOM stripped after decrypt

The unwrap passphrase Carlo*TSSD2!018 is a protocol constant in every 7.9.6
install. It is not the SFTP login password.

Buwiz code: crates/bir-core/src/transport.rs
(unwrap_dispatcher_field, fetch_sftp_endpoint).

Unwrapped non-secret result:

- host = ebf2.bir.gov.ph
- port = 22
- DEV override host in official JS = ftp2.birgovph.com (not used in PROD)

Username/password were unwrapped only to prove login. They stay in the local
ignored .env, never in git.

### 5. Prove SSH/SFTP with dummy TIN only

cargo run -p bir-core --bin sftp_harness -- --live-connect
cargo run -p bir-core --bin sftp_harness -- --live-put

Live-connect printed source=dispatcher, host/port/folder, username_len=15,
password_len=18, auth=ok, sftp=ok, cwd=/.

Live-put reused existing dummy ciphertext:

C:\eBIRForms\IAF_RDO_Copy\00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml

BIR later emailed a confirmation for that dummy 1601C. Transport success is
PUT ok; filing evidence is the email.

Headless later queued the next month with the same dummy profile and .env
override:

00000000000000-1601Cv2018-102026#codeitlikemiley@gmail.com#.xml

Local status became Submitted. Wait for email before treating that as a
second BIR receipt.

## How to refresh credentials later

Do not commit the result.

1. Keep .env git-ignored.
2. Either leave BIR_SFTP_HOST blank so runtime fetches tinDispatcherSFTP.php
   again, or fetch the dispatcher, unwrap, and rewrite only the local .env.
3. Log host / port / username_len / password_len only.
4. Use dummy TIN 00000000000000 unless you intend to file a real return.

Blank template is in repo-root .env.example.

## Agent queue vs cron PUT

The agent host used to stop at confirmation and refuse to queue. That made
headless filing useless. On this branch:

- filing.submit still only shows confirmation
- filing.queue / form.queue / form.submit with confirm=true queues 1601C / 2551Q
- in-process cron then opens SFTP (dispatcher or BIR_SFTP_*) and PUTs

Clicking form-1601c-submit-confirm directly is still refused so the confirm
argument cannot be skipped.

## What not to do

- Do not put BIR_SFTP_PASSWORD in git, PR text, or this folder.
- Do not copy OSS port 23; live BIR is port 22.
- Do not treat dry-run (BIR_SFTP_DRY_RUN=1) as a BIR filing.
- Do not PUT a real taxpayer TIN unless you mean to file.

## Three ways to queue (for Grok Bot later)

1. Grok Bot / gpui-agent against bir-headless. GUI does not need to be open.
   filing.start and filing.queue accept tin, code, year, period. Queue with
   confirm=true. Recipe: crates/bir-desktop/recipes/form-1601c-queue.json
2. Painted bir confirm button. Cron still needs painted bir or bir-headless
   running to PUT.
3. Cron on a row that is already Queued. Cron does not create the queue.

Grok Bot uses the same GPUI_AGENT_TOKEN as the host that owns 127.0.0.1:17421.
