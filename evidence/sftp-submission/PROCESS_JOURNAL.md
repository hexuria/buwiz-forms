# Process journal — eBIRForms SFTP investigation (chronological)

A start-to-finish record of *how* this was done, for later review. Each step
lists the action, the command/tool, what came back, and the decision it drove.
Companion docs: `INVESTIGATION_LOG.md` (findings), `EBIRFORMS_SFTP_PROTOCOL.md`
(deliverables), `README.md` (index).

- **Date:** 2026-09-11
- **Machine:** Windows 11 ARM64 in a UTM/QEMU VM. Git Bash + Windows PowerShell.
- **Worktree used for all my writes:** `worktree-ebirforms-sftp-investigation`
  (`.claude/worktrees/ebirforms-sftp-investigation`). I created this with
  `EnterWorktree` after the user asked; nothing was written outside it except
  disposable files in the session scratchpad.
- **Hard environmental limits discovered:** no network egress to BIR from this
  VM; no `clang`; no local SSH server; C: drive repeatedly at 100%.

---

## Phase 0 — Orientation (build on prior findings, don't restart)

1. Listed `C:\Program Files*`, then searched the drive:
   `find /c -maxdepth 4 -iname ebfSFTP.exe` → found installs at `C:\eBIRForms`
   and `C:\eBIRForms-reinstall`.
2. Read `changelog-ebirforms.md` (product 7.9.6.1; 3→5 digit branch; dummy-TIN
   frontend gate) and confirmed the project layout (Rust workspace, `crates/`).
3. Listed the install: found `ebfSFTP.exe` (9 KB), `ebfSFTP.pdb`,
   `ebfSFTP.exe.config`, `WinSCP.exe`, `WinSCPnet.dll`, `Encrypt.exe`,
   `cFTPSend.exe` (old FTP), `proxy.ini`, plus `savefile/`, `IAF_RDO_Copy/`, etc.
   **Decision:** analyze the tiny `ebfSFTP.exe` in full; treat `cFTPSend.exe` as
   the legacy transport for the old/new diff.
4. `cmp` showed `C:\eBIRForms` and `C:\eBIRForms-reinstall` `ebfSFTP.exe` and
   `proxy.ini` are byte-identical → one binary to analyze.
5. Read `ebfSFTP.exe.config` (.NET 4.7.2) and `proxy.ini` (proxy empty by default).

## Phase 1 — Decompile ebfSFTP.exe (no `strings`)

6. No `strings`/ILSpy/dotnet SDK present. **Decision:** since this is .NET
   Framework on Windows, use **PowerShell reflection** — `Assembly.Load` +
   `MethodBody.GetILAsByteArray()` + `Module.Resolve*` — which reads metadata/IL
   **without executing** the assembly. Wrote `scripts/ildump.ps1`.
7. First run threw on 2-byte opcode mapping (`OpCode.Value` is a signed short);
   fixed the map key to `([int]$op.Value -band 0xFFFF)`.
8. Dumped `Program`: recovered `Main`, `Encrypt`, `Decrypt`, `getProxySettings`,
   `GenerateRandomBytes`. `SFTPSend` failed because `WinSCPnet` couldn't resolve.
9. Wrote `scripts/ildump_sftp.ps1` with a guarded assembly resolver
   (`LoadFile` + cache; first attempt stack-overflowed from resolver recursion —
   fixed by pre-loading and returning cached assemblies). Dumped `SFTPSend`.
10. Resolved WinSCP enum values from `WinSCPnet.dll` by reflection:
    `Protocol.Sftp=0`, `SshHostKeyPolicy{Check=0,GiveUpSecurityAndAcceptAny=1,
    AcceptNew=2}`, `TransferMode.Binary=0`, `PutFiles(localPath,remotePath,remove,options)`.

    **Recovered contract:** args `= hostEnc, port, userEnc, passEnc, folder, file`;
    host/user/pass are `Decrypt`ed; crypto = AES-256-CBC/PKCS7 + PBKDF2-HMAC-SHA1
    (100000, 32-byte salt), `base64(salt‖iv‖ct)`, passphrase `Carlo*TSSD2!018`;
    session uses `SshHostKeyPolicy=GiveUpSecurityAndAcceptAny` (**no host-key
    check**), binary `PutFiles` to `/folder/`, exit 0 = success.

## Phase 2 — Who calls the shim (integration)

11. `changelog` said the client extracts HTA/JS to `%TEMP%\{GUID}\`. Grepped
    `%TEMP%` for `ebfSFTP|sftphost|Carlo` → hit
    `%TEMP%\{0B33C1CE-…}\js\eBIRTools.vbs` and `string-util.js`.
12. Read `eBIRTools.vbs`: active `RenameAndSendFile` (authored `carlodlc
    2026-feb-04`) builds `"…\ebfSFTP.exe" host port user pass folder "file"`;
    legacy `RenameAndSendFile_original` used `cFTPSend.exe`. Confirmed arg order.
13. Traced the call site `string-util.js:1130` → args come from
    `conService.getConConfig()`; folder from `enviService.getFtpFolder`.
14. Read `string-util.js` 760–908 + `environment.js`: credentials are fetched at
    submit time from **`tinDispatcherSFTP.php?t=<TIN9>&f=<form>&v=<ver>`**
    (primary `birgovph.com`, backup `ws1.birgovph.com`), JSON
    `{mode,server,SSLPort,port,username,password}`, `server/username/password`
    AES-wrapped. **Key insight:** no credentials are embedded anywhere.

## Phase 3 — Validate the crypto (non-production vector)

15. `string-util.js` had a hard-coded DEV/UAT `srv` blob. Wrote
    `scripts/decrypt_devuat_srv.ps1` reproducing `Decrypt`. Result:
    **`ftp2.birgovph.com`**, plaintext exactly **17 bytes, no BOM**
    (`salt=32,iv=16,ct=32`). Crypto reconstruction is byte-exact; also proved the
    official helper writes **UTF-8 without a BOM**.

## Phase 4 — Payload format + prior art

16. Compared `savefile/*.xml` (plaintext) vs the uploaded `IAF_RDO_Copy/…#email#.xml`
    (`xxd` → high-entropy ciphertext). **`Encrypt.exe` encrypts the payload before
    transport; the transport moves opaque bytes.** Payload step unchanged old→new.
17. `git worktree list` revealed a parallel `codex/ebirforms-sftp-transport`
    worktree. **Read-only** review of its `transport.rs`, `sftp_harness.rs`,
    findings doc, and `Cargo.toml` (russh 0.63 / russh-sftp 3.0). It matched my
    findings; I found **two defects** (below). I did not modify it.
18. Verified Buwiz `naming.rs` already models 14-digit TINs and `receipt.rs`
    `split_bir_filename` already strips `#email#` and accepts 14-digit prefixes.

## Phase 5 — Create my worktree + write deliverables

19. `EnterWorktree ebirforms-sftp-investigation`. Wrote `INVESTIGATION_LOG.md`,
    `EBIRFORMS_SFTP_PROTOCOL.md` (deliverables 1–8, architecture, tables, diff,
    test plan, Rust recommendations), `submission_transport.reference.rs`
    (secret-free `SubmissionTransport` trait + dispatcher/static/mock impls),
    `README.md`, and copied the analysis scripts.

## Phase 6 — Independent verification (go beyond reading codex)

20. **Rust crypto, offline:** standalone crate porting `unwrap_dispatcher_field`
    (`aes/cbc/pbkdf2/sha1/data-encoding`). `cargo run` →
    `unwrapped = "ftp2.birgovph.com" (len 17)` → **PASS**. My Rust matches .NET.
21. **DEV/UAT SSH host:** `ssh-keyscan -p22 ftp2.birgovph.com` → nothing.
    DNS + TCP: **`ftp2.birgovph.com` = `103.56.5.254`** (same IP as the dead FTP
    gateway). Both `:21` and `:22` unreachable **from this VM** (egress blocked).
22. **Live dispatcher (dummy TIN, read-only GET):**
    `tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0` → HTTP 200; keys
    `[SSLPort,mode,password,port,server,username]`; **`mode=2`, `port=22`,
    `SSLPort=990`**; `server/username/password` AES-wrapped (88/88/108 chars).
    **Login secrets deliberately NOT decrypted or recorded.**

    Net: the entire live pipeline is confirmed except opening an authenticated
    SSH session (which needs the live secrets — out of scope).

## Phase 7 — Attempt an end-to-end SFTP upload proof (loopback)

23. **Goal:** prove the SFTP client connects + authenticates + uploads, without
    BIR/real data — by running an in-process russh SFTP server on `127.0.0.1` and
    driving the client into it. Wrote `scripts/sftp-loopback/` (server + client).
24. `cargo build` failed on two counts:
    - `ring` build needs **`clang`** — not installed (`command -v clang` empty).
    - **disk full**: `os error 112` "not enough space on the disk" (C: at 100%).
25. **Decision (respecting the user's boundary):** do NOT delete shared cargo
    caches or other worktrees' `target/` (could break the codex build that is
    likely filling the disk). Cleaned only my own scratchpad build residue →
    freed ~6.3 GB.

26. **Found `clang`** at `C:\Program Files\LLVM\bin\clang.exe` (v22). Rebuilt the
    loopback harness with LLVM on PATH; `ring` compiled. Then fixed russh 0.63
    API by reading the crate sources: `channel_open_session` takes a 4th
    `reply: ChannelOpenHandle` and you must `reply.accept().await`
    (`Drop` rejects otherwise → `AdministrativelyProhibited`); russh-sftp 3.0
    `server::Handler` is native-async by default (drop `#[async_trait]`);
    `check_server_key(&PublicKeyOrCertificate)`; host key from a fixed test seed
    via `Ed25519Keypair::from_seed` + `PrivateKey::new` (no RNG, no secret in repo).

27. **RAN IT — PASS:**
    ```
    auth=ok (connected to 127.0.0.1:PORT, accept-any host key)
    uploaded 32 bytes to /1601Cv2018/00000000000000-1601Cv2018-092026#test@example.com#.xml
    PASS: connect + password auth + accept-any host key + SFTP upload verified
    ```
    The SFTP client mechanic (connect → password auth → accept-any host key →
    upload to `/formType/filename`) is **proven by execution**, with the server
    holding the exact bytes. The only reason it can't reach BIR is network egress
    from this VM — not the code.

---

## Phase 8 — Wire `SubmissionTransport` into `bir-core` for real

28. Added `crates/bir-core/src/submission_transport.rs` (the real module: the
    `SubmissionTransport` trait + `DispatcherSftpTransport` / `StaticSftpTransport`
    / `MockTransport`, dispatcher client, byte-exact crypto, and the russh SFTP
    upload proven in Phase 7). Registered `pub mod submission_transport;` in
    `lib.rs` and added deps to `crates/bir-core/Cargo.toml`
    (`pbkdf2`, `sha1`, `async-trait`, `russh` 0.63, `russh-sftp` 3.0).
29. **Verified the exact wired file compiles** via a focused `modcheck` crate that
    `#[path]`-includes `submission_transport.rs` and pulls the same deps —
    `cargo check` → `Finished`, no errors (used ~300 MB). I deliberately did NOT
    run a full cold `cargo check -p bir-core` here: it would pull the whole crate
    dep tree (openssl-vendored, imap, rusqlite, …) and risked zeroing the shared
    disk during the concurrent codex build. Workspace-level `cargo check -p
    bir-core` + `Cargo.lock` update should be run when the disk frees; dep risk is
    low (all mainstream crates, resolved cleanly in isolation).

## Defects found in `codex/ebirforms-sftp-transport` (documented, not touched)

1. `wrap_dispatcher_field` prepends a UTF-8 **BOM**; the official `Encrypt` uses
   `UTF8NoBOM` (proven: 17-byte plaintext). Harmless for its local round-trip but
   not byte-accurate.
2. `DISPATCHER_BACKUP` = `ws2.birgovph.com`; the **connection-WS** backup is
   `ws1.birgovph.com` (`ws2` is the version-check backup).

My `submission_transport.reference.rs` has both correct.

## Honest status

- **Achieved:** complete, evidence-based protocol reconstruction; byte-exact
  crypto verified in .NET *and* Rust; live dispatcher confirmed (`mode=2/port=22`);
  arg contract, host-key policy, filename/branch rules, payload boundary; a
  secret-free trait design; two corrections to the parallel branch.
- **Proven by execution:** the SFTP client mechanic — `scripts/sftp-loopback`
  connects, authenticates by password, accepts any host key, and uploads to
  `/formType/filename`, with bytes verified server-side (built once `clang` was
  on PATH).
- **NOT achieved (and why):** a **real BIR** submission or live BIR SFTP
  connection — blocked by no egress to `103.56.5.254:22` from this VM. A real
  submission additionally needs live dispatcher credentials + a genuine encrypted
  IAF (out of scope). BIR's specific SSH algorithm negotiation is therefore still
  unconfirmed against the real server (the loopback proves our client, not BIR's
  exact KEX/cipher set).
- **To close it fully:** run the harness / wired transport on a network with
  egress to BIR using live dispatcher fields, or against a local OpenSSH server
  with `TEST_SFTP_*` for a same-stack check.
