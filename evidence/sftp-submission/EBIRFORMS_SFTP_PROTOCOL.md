# eBIRForms 7.9.6.x submission protocol — analysis & Buwiz integration plan

Evidence-driven reconstruction of how the current Offline eBIRForms client
submits returns, why Buwiz's old FTP path stopped working, and how to restore
submission with a maintainable, secret-free Rust transport.

All facts below are traced to the installed client at `C:\eBIRForms`
(product 7.9.6.1), its extracted HTA/JS/VBS, the `ebfSFTP.exe` IL, and one
non-production crypto validation. No PROD credentials are recorded.

---

## 1. Architecture of the current submission flow

```
┌────────────────────────────────────────────────────────────────────────────┐
│ BIRForms.exe  (self-extracts HTA/JS/VBS to %TEMP%\{GUID}\ ; runs via mshta)  │
└───────────────┬──────────────────────────────────────────────────────────────┘
                │  user fills form, clicks "Submit / Final Copy"
                ▼
   string-util.js  sendEmail()/reSendEmail()
                │
     ┌──────────┴───────────┐
     │ 1. conService.initConConfig(TIN9, formType)                             │
     │      GET http://birgovph.com/tinDispatcherSFTP.php?t=TIN9&f=form&v=7.9.6.0│
     │      (backup: http://ws1.birgovph.com/tinDispatcherSFTP.php?…)          │
     │      → JSON { mode, server, SSLPort, port, username, password }         │
     │        server/username/password are AES-wrapped (base64); port plaintext │
     ├──────────────────────┤
     │ 2. saveEncryptedProfile(true)                                            │
     │      write plaintext savefile → then Encrypt.exe encrypts the IAF        │
     │      → C:\eBIRForms\IAF_RDO_Copy\<TIN14>-<form>-<period>#<email>#.xml     │
     │        (opaque ciphertext payload)                                       │
     ├──────────────────────┤
     │ 3. eBIRTools.vbs RenameAndSendFile(file,email,folder,mode,srv,ssl,port,usr,pass)
     │      objShell.Run:                                                        │
     │      "…\ebfSFTP.exe" <srvEnc> <port> <usrEnc> <passEnc> <folder> "<file>" │
     └──────────┬───────────┘
                ▼
   ebfSFTP.exe (.NET 4.7.2, WinSCP.NET 1.16)
     Decrypt(args[0]=host), Int32.Parse(args[1]=port),
     Decrypt(args[2]=user), Decrypt(args[3]=pass),
     folder = "/" + args[4] + "/", file = args[5]
        │  AES-256-CBC/PKCS7, PBKDF2-HMAC-SHA1(100000, salt32), key="Carlo*TSSD2!018"
        ▼
     WinSCP.NET Session
        Protocol = Sftp
        SshHostKeyPolicy = GiveUpSecurityAndAcceptAny   ← NO host-key check
        (optional HTTP proxy from proxy.ini if ProxyAddress set)
        Session.Open  →  PutFiles(file, "/folder/", remove=false, Binary)
        result.Check()  →  print "0"; exit 0 on success, throw/nonzero on failure
        │
        ▼
   BIR SFTP server  (SSH/2, port 22; DEV/UAT host = ftp2.birgovph.com)
        stores /<formType>/<TIN14>-<form>-<period>#<email>#.xml
        → later emails an e-filing confirmation to <email>
```

Key architectural shift: the endpoint is no longer a fixed host. A **dispatcher
web service chooses the SFTP endpoint per TIN + form at submit time** and hands
back encrypted connection fields.

---

## 2. Component / responsibility / evidence

| Component | Responsibility | Evidence |
|---|---|---|
| `BIRForms.exe` (58 MB, 7.9.6.1) | Host shell; extracts HTA/JS/VBS to `%TEMP%\{GUID}\` and runs the UI. Submission logic lives in the **extracted scripts**, not the packed exe (why `strings` on the exe failed). | Extraction dir `%TEMP%\{0B33C1CE-…}`; `changelog-ebirforms.md`; scripts reference `../` helpers. |
| `tinDispatcherSFTP.php` (birgovph.com; backup ws1) | Returns per-TIN/form `{mode, server, SSLPort, port, username, password}`; `server/username/password` AES-wrapped. Chooses transport mode. | `string-util.js` `initConConfig`/`_conWebService`; live dummy-TIN fetch (codex findings). |
| `environment.js` | Env + version constants; WS host table. `currEnvi=PROD`, `currVer=7.9.6.0`, backup=`ws1.birgovph.com`. | `js/environment.js` lines 1–60, 310–318. |
| `Encrypt.exe` (489 KB) | Encrypts the plaintext savefile into the opaque IAF payload **before** transport. | `EncryptFile()` in `eBIRTools.vbs`; `savefile/*.xml` plaintext vs `IAF_RDO_Copy/*.xml` ciphertext. |
| `eBIRTools.vbs` `RenameAndSendFile` | Renames to `#email#` IAF name and invokes `ebfSFTP.exe` with the 6 positional args; return `0`=success. | `js/eBIRTools.vbs` lines 3–56. |
| `ebfSFTP.exe` (9 KB, .NET 4.7.2) | Decrypts host/user/pass, opens WinSCP.NET SFTP (accept-any host key), uploads to `/folder/`, exit 0 on success. | IL dump of `Program.Main/SFTPSend/Encrypt/Decrypt/getProxySettings`. |
| `WinSCP.exe` + `WinSCPnet.dll` (1.16.0.16453) | Actual SSH/SFTP engine behind the shim. | `SFTPSend` IL references `WinSCP.Session/SessionOptions/TransferOptions`. |
| `proxy.ini` | Optional HTTP proxy. Shim reads only `ProxyAddress/ProxyPort/ProxyUser/ProxyPassword`; empty by default → proxy off. | `getProxySettings` IL; `proxy.ini` contents. |
| `Carlo*TSSD2!018` | PBKDF2 passphrase that unwraps the **connection fields** (not the SFTP login secret, and not the payload key). | `Encrypt`/`Decrypt` IL; validated decrypt of DEV/UAT `srv` → `ftp2.birgovph.com`. |
| `cFTPSend.exe` (335 KB) | **Legacy** FTP sender (superseded). | `RenameAndSendFile_original` in `eBIRTools.vbs`. |

### ebfSFTP.exe argument contract (exact)

| # | Arg | Encoding | Consumed as |
|---|---|---|---|
| 0 | host | AES-wrapped base64 | `Decrypt` → `SessionOptions.HostName` |
| 1 | port | plaintext integer | `Int32.Parse` → `PortNumber` |
| 2 | username | AES-wrapped base64 | `Decrypt` → `UserName` |
| 3 | password | AES-wrapped base64 | `Decrypt` → `Password` |
| 4 | folder | plaintext | wrapped `"/"+arg+"/"` → remote dir |
| 5 | file | plaintext path (quoted) | local file → `PutFiles` |

Success = process exit `0` and stdout `"0"`. Failure = unhandled exception →
nonzero exit + stack trace on stderr (no retry inside the shim).

### Credential-wrap crypto (validated byte-exact)

- Algorithm: **AES-256-CBC, PKCS7**.
- Key: **PBKDF2-HMAC-SHA1**, passphrase `Carlo*TSSD2!018`, **32-byte random salt**,
  **100000** iterations, 32-byte derived key.
- Wire format: `base64( salt[32] ‖ iv[16] ‖ ciphertext )`.
- Text encoding: **UTF-8, no BOM** (verified: DEV/UAT `srv` decrypts to a 17-byte
  string `ftp2.birgovph.com`).

---

## 3. Why the old Buwiz implementation stopped working

`crates/bir-core/src/transport.rs` (old) hard-codes a single FTP endpoint:

```rust
const BIR_FTP_HOST: &str = "103.56.5.254:21";
const BIR_FTP_USER: &str = "uploadOnly";
const BIR_FTP_PASS: &str = "12birBIR";
// connect → login → CWD /{form_type} → STOR filename
```

Every one of these assumptions is now invalid:

1. **Protocol changed FTP → SFTP.** The plaintext FTP control channel and the
   `103.56.5.254:21` gateway are gone ("Improved security on File Submission").
2. **Endpoint is no longer fixed.** It is resolved per-TIN/form via
   `tinDispatcherSFTP.php`; there is no single host/user/pass to hard-code.
3. **Credentials are dynamic and wrapped.** The client never stores them; it
   receives AES-wrapped fields at submit time.
4. **Branch width 3 → 5 digits.** Filenames now carry a **14-digit** TIN prefix
   (9 + 5), not 12.

So the old path fails at the very first step (TCP/21 unreachable), and even a
patched FTP client could not authenticate — the auth model itself changed.

---

## 4. OLD FTP vs NEW SFTP — protocol differences

| Aspect | OLD (FTP) | NEW (SFTP) |
|---|---|---|
| Transport | FTP (suppaftp), TCP/21 | SFTP over SSH/2, TCP/**22** |
| Endpoint | fixed `103.56.5.254:21` | per-TIN/form via `tinDispatcherSFTP.php` |
| Auth source | hard-coded `uploadOnly`/`12birBIR` | dispatcher returns AES-wrapped `server/username/password` |
| Credential secrecy | plaintext constants in binary | wrapped with PBKDF2/AES key `Carlo*TSSD2!018` |
| Host-key verification | n/a (FTP) | **none** — WinSCP `GiveUpSecurityAndAcceptAny` |
| Proxy | FTP proxy fields in `proxy.ini` (`proxyMode`, passive…) | HTTP proxy only (`ProxyMethod=3`), off unless `ProxyAddress` set |
| Payload encryption | `Encrypt.exe` (pre-transport) | `Encrypt.exe` (pre-transport) — **unchanged** |
| Filename prefix | 12-digit `TIN9+branch3` | **14-digit** `TIN9+branch5` |
| Remote layout | `CWD /{formType}` then `STOR file` | `PutFiles(file, "/{formType}/")` |
| Transfer mode | binary | binary |
| Dispatcher key (`t=`) | n/a | **first 9 digits** of TIN only |
| Success signal | FTP 226 | shim exit `0` / stdout `"0"` |

**Unchanged:** IAF payload is opaque ciphertext produced before transport; remote
directory is the form-type folder; the human confirmation still arrives by email.

---

## 5. Verification status of runtime facts (dummy data only)

Independently verified in this investigation (see INVESTIGATION_LOG §7–8):

- ✅ **Dispatcher live**; `mode=2` (SFTP), **`port=22`**, `SSLPort=990` for
  `t=000000000&f=1601Cv2018&v=7.9.6.0` (HTTP 200; login secrets not recorded).
- ✅ **Rust crypto port reproduces the .NET decrypt byte-for-byte**
  (DEV/UAT `srv` → `ftp2.birgovph.com`, 17 bytes, no BOM).
- ✅ **Old FTP dead**: `103.56.5.254:21` unreachable.
- ✅ **Same host, new protocol**: `ftp2.birgovph.com` → `103.56.5.254`, the
  *same IP* as the retired FTP gateway. The migration is a protocol/port change
  on the same infrastructure, fronted by the dispatcher — not a new provider.

Still needing runtime verification:

1. **SSH algorithm negotiation** (KEX, host-key type, cipher, MAC) the server
   accepts — to confirm `russh` defaults interoperate. Not capturable from this
   VM (egress to `103.56.5.254:22` is blocked here). Codex saw a FileZilla-Pro
   SSH banner on 22, consistent with `mode=2/port=22`.
2. **Full `mode`/`port` matrix across all forms** (eFPS `mode=3/4` cases).
3. **Server directory expectations** — does `/{formType}/` need to pre-exist, and
   does the server ack/rename on receipt?
4. **`Encrypt.exe` payload format parity** with Buwiz's own IAF encryptor in
   7.9.6.x (out of transport scope, but gates a real submission).
5. **Backup selection semantics** — exact retry to `ws1.birgovph.com` on
   `mode=0`, and timeout behavior.

---

## 6. Safe, reproducible test plan (no real taxpayer data)

**Layer A — crypto (offline, no network):**
- Reproduce `unwrap_dispatcher_field` against the DEV/UAT `srv` blob; assert
  `== "ftp2.birgovph.com"` and plaintext length 17 (no BOM). Scripts:
  `scripts/decrypt_devuat_srv.ps1` (validated) and the Rust unit test in the
  reference module.
- Round-trip `wrap` → `unwrap` for arbitrary strings.

**Layer B — dispatcher shape (network, dummy TIN, outward-facing → confirm first):**
- `GET tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0`; record only
  `mode`, `port`, and field presence. Never submit; never use a real TIN.

**Layer C — SFTP mechanics against a LOCAL server (no BIR):**
- Run a local OpenSSH/atmoz-sftp container. Export
  `TEST_SFTP_HOST/PORT/USER/PASSWORD/FOLDER`.
- Drive the Rust harness to upload a dummy encrypted IAF and assert:
  - filename is `…<TIN14>-<form>-<period>#<email>#.xml`,
  - remote path is `/<formType>/<filename>`,
  - transfer is binary and byte-identical,
  - accept-any host key connects without a known_hosts entry,
  - success/failure surface as clean `Ok/Err`.

**Layer D — parity (optional, dummy savefile):**
- Encrypt a dummy `000000000…` savefile with `Encrypt.exe` and with Buwiz's own
  IAF encryptor; diff to confirm payload format parity.

---

## 7. Rust integration recommendations for Buwiz Forms

1. **Keep payload generation and encryption where they are.** The transport
   changed; the IAF payload did not. Buwiz already produces an encrypted IAF for
   the FTP path — reuse it and treat the transport as an opaque-bytes sink.
2. **Add a dispatcher client** that mirrors `initConConfig`:
   - primary `http://birgovph.com/tinDispatcherSFTP.php`,
     **backup `http://ws1.birgovph.com/tinDispatcherSFTP.php`** (not ws2),
   - key with the **first 9 digits** of the TIN, `f=<formType>`, `v=<version>`,
   - parse the single-quoted JSON body, retry backup on `mode=0`.
3. **Reuse existing crypto crates** already in the lockfile — `aes`, `cbc`,
   `pbkdf2`, `sha1`, `data-encoding`/`base64`, `zeroize` — for
   `unwrap_dispatcher_field` (AES-256-CBC/PKCS7, PBKDF2-HMAC-SHA1, 100000, salt32,
   `base64(salt‖iv‖ct)`, **UTF-8 no BOM**).
4. **Use a pure-Rust SSH stack** (`russh` + `russh-sftp`) so there is no OpenSSL/
   libssh2 system dependency across macOS/Windows desktop targets. Configure a
   host-key handler that **accepts any key** to match the official client
   (`GiveUpSecurityAndAcceptAny`). Password auth. Binary upload to
   `/<formType>/<filename>`.
5. **Never embed credentials.** The endpoint always comes from the dispatcher at
   runtime (PROD) or from `TEST_SFTP_*` env (tests/harness). Hold the password in
   `Zeroizing<String>`.
6. **Preserve the queue/claim boundary.** Keep the existing "open session, then
   claim, then store" split so a connect timeout does not freeze a claim (the old
   `open_iaf_session` + `store` shape maps directly onto SFTP).
7. **Filename/branch width is already correct** in `crates/bir-core/src/naming.rs`
   (`Tin{segment1..3, branch}`, 14-digit `full()`), and `receipt.rs`
   `split_bir_filename` already strips `#email#` and accepts 14-digit prefixes.
   No change needed there.

### Corrections to apply to the `codex/ebirforms-sftp-transport` branch

- **BOM:** `wrap_dispatcher_field` must **not** prepend a UTF-8 BOM; the official
  `Encrypt` uses `UTF8NoBOM`. Keep the BOM-strip in `unwrap` as defensive.
- **Backup dispatcher host:** `DISPATCHER_BACKUP` should be
  `http://ws1.birgovph.com/tinDispatcherSFTP.php` (the connection-WS backup),
  not `ws2` (that is the version-check backup).
- Everything else on that branch (crypto params, accept-any host key, 9-digit
  dispatcher key, `/folder/file` layout, `TEST_SFTP_*` externalization) matches
  the evidence.

---

## 8. `SubmissionTransport` abstraction

A reference module is provided at
[`submission_transport.reference.rs`](./submission_transport.reference.rs). It is
**secret-free** and compiles against crates already in the workspace lockfile for
everything except the SSH leg (which reuses the codex `russh` implementation). It
defines:

```rust
#[async_trait::async_trait]
pub trait SubmissionTransport: Send + Sync {
    /// Upload one already-encrypted IAF payload under its form-type folder.
    async fn submit(
        &self,
        form_type: &str,
        filename: &str,
        payload: &[u8],
    ) -> Result<(), TransportError>;
}
```

with:

- `DispatcherSftpTransport` — resolves the endpoint via `tinDispatcherSFTP.php`,
  unwraps fields, and delegates to the SFTP client (production path).
- `StaticSftpTransport` — takes an `SftpEndpoint` supplied externally
  (`TEST_SFTP_*` or a config struct); embeds no secrets (harness/self-host path).
- `MockTransport` — records calls in-memory for unit tests, so callers and the
  filename/folder contract are testable with zero network.

The crypto (`wrap`/`unwrap_dispatcher_field`) and endpoint types live in the same
module and are exercised by unit tests, including the validated DEV/UAT vector.
