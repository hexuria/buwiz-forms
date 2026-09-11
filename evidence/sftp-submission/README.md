# eBIRForms SFTP submission — investigation (worktree: ebirforms-sftp-investigation)

Evidence-driven reconstruction of the current Offline eBIRForms 7.9.6.x
submission protocol, and a secret-free path to restore submission in Buwiz.

| File | Contents |
|---|---|
| [`PROCESS_JOURNAL.md`](./PROCESS_JOURNAL.md) | Start-to-finish chronological record of *how* the investigation was done (commands, observations, decisions, dead-ends), for review. |
| [`INVESTIGATION_LOG.md`](./INVESTIGATION_LOG.md) | Running log: what was tested, observed, proven, and what remains — including independent Rust-crypto and live-dispatcher verification. |
| [`EBIRFORMS_SFTP_PROTOCOL.md`](./EBIRFORMS_SFTP_PROTOCOL.md) | Deliverables 1–8: architecture diagram, component/evidence table, why old FTP broke, FTP↔SFTP diff, verification status, safe test plan, Rust recommendations, `SubmissionTransport` design. |
| [`submission_transport.reference.rs`](./submission_transport.reference.rs) | Secret-free reference module: `SubmissionTransport` trait + dispatcher/crypto/mock. Crypto unit-verified. |
| [`scripts/ildump.ps1`](./scripts/ildump.ps1), [`scripts/ildump_sftp.ps1`](./scripts/ildump_sftp.ps1) | Reflection + IL disassembler for `ebfSFTP.exe` (no external tools). |
| [`scripts/decrypt_devuat_srv.ps1`](./scripts/decrypt_devuat_srv.ps1) | Validates the credential-wrap crypto against a non-production vector → `ftp2.birgovph.com`. |
| [`scripts/crypto-verify/`](./scripts/crypto-verify/) | Standalone Rust crate that reproduces the .NET decrypt byte-for-byte (`cargo run`). Verified PASS here. |
| [`scripts/sftp-loopback/`](./scripts/sftp-loopback/) | In-process russh SFTP server + client loopback upload proof. **Not run on the investigation VM** (needs `clang` for `ring`, disk, no local SSH server); runs on a machine with the workspace toolchain. |

## Headline findings

- Transport changed **FTP/21 → SFTP/22** on the **same host** (`ftp2.birgovph.com`
  = `103.56.5.254`, the old gateway IP). Old FTP confirmed dead.
- Endpoint is resolved **per-TIN/form at submit time** via
  `tinDispatcherSFTP.php` (primary `birgovph.com`, backup **`ws1`**), which
  returns `mode/server/SSLPort/port/username/password`; `server/username/password`
  are AES-wrapped. Live dummy-TIN probe: `mode=2`, `port=22`, `SSLPort=990`.
- Wrap crypto: **AES-256-CBC/PKCS7**, **PBKDF2-HMAC-SHA1** (100000, 32-byte salt),
  `base64(salt‖iv‖ct)`, **UTF-8 no BOM**, passphrase `Carlo*TSSD2!018`. Verified
  in .NET and Rust.
- `ebfSFTP.exe` sets WinSCP **`SshHostKeyPolicy = GiveUpSecurityAndAcceptAny`** →
  **no host-key verification**. Binary `PutFiles` to `/<formType>/<file>`.
- Payload is encrypted by `Encrypt.exe` **before** transport (unchanged old→new).
- Filenames now use a **14-digit** TIN prefix (9 + 5). Buwiz `naming.rs` /
  `receipt.rs` already handle this.

## Corrections to the parallel `codex/ebirforms-sftp-transport` branch

1. `wrap_dispatcher_field` must **not** prepend a UTF-8 BOM (official helper uses
   `UTF8NoBOM`; verified 17-byte plaintext).
2. `DISPATCHER_BACKUP` should be **`ws1.birgovph.com`** (connection-WS backup),
   not `ws2` (that is the version-check backup).
