# eBIRForms SFTP submission — investigation log

- **Date:** 2026-09-11
- **Analyst branch / worktree:** `worktree-ebirforms-sftp-investigation` at
  `C:\Users\uriah\Code\buwiz-forms\.claude\worktrees\ebirforms-sftp-investigation`
- **Authority (ground truth):** installed Offline eBIRForms at `C:\eBIRForms`
  (product **7.9.6.1**, `BIRForms.exe` sha256
  `a43a4599f95158e6ba0e7a1c4b88c4e2cf215ac86e53c24259cc69d1b664829c`),
  its extracted HTA/JS/VBS tree, and the `ebfSFTP.exe` .NET assembly.
- **Prior art cross-referenced:** `codex/ebirforms-sftp-transport`
  (`C:\Users\uriah\Code\buwiz-forms-wt-sftp`) — an independent, largely-correct
  SFTP implementation. Two defects found and documented below.
- **Constraint honored:** no real taxpayer data submitted; no PROD credentials
  recovered or recorded. The only value decrypted here is the client's own
  hard-coded **DEV/UAT** host, used solely to validate the crypto.

Format per entry: **Tested → Observed → Proves → Unknown/Next.**

---

## 1. Located the installation and the transport binaries

- **Tested:** searched the drive for `ebfSFTP.exe` / `BIRForms.exe`.
- **Observed:** two identical installs, `C:\eBIRForms` and
  `C:\eBIRForms-reinstall`. `ebfSFTP.exe` and `proxy.ini` are byte-identical
  between them. The install still contains the **old** transport (`cFTPSend.exe`),
  the payload encryptor (`Encrypt.exe`), `WinSCP.exe`, `WinSCPnet.dll`
  (v1.16.0.16453), `ebfSFTP.exe` (9,216 bytes), `ebfSFTP.exe.config`,
  `ebfSFTP.pdb`, and `proxy.ini`.
- **Proves:** the SFTP shim is a single, tiny, stable artifact worth full static
  analysis; the old FTP tooling was retained but superseded.
- **Next:** decompile `ebfSFTP.exe`.

## 2. `ebfSFTP.exe.config` and `proxy.ini`

- **Observed:** config targets **.NET Framework 4.7.2**. `proxy.ini`
  `[ProxySettings]` has empty `ProxyAddress`, `ProxyPort=8080`, plus FTP-era keys
  (`proxyMode=9`, `ControlPortProxy`, `UsePassive`, …).
- **Proves:** it is a managed assembly loadable by reflection on this machine;
  proxy is **off by default** (empty address).
- **Next:** confirm which proxy.ini keys the SFTP shim actually reads.

## 3. Decompiled `ebfSFTP.exe` by reflection + IL (not `strings`)

- **Tested:** `scripts/ildump.ps1` / `scripts/ildump_sftp.ps1` —
  `Assembly.Load` + `MethodBody.GetILAsByteArray()` + `Module.Resolve*`, with an
  assembly-resolver so `WinSCPnet` types resolve. Reflection does **not** execute
  the assembly.
- **Observed (`ebfSFTP.Program`):**
  - `Main(string[] args)` decrypts `args[0] (host)`, `args[2] (user)`,
    `args[3] (pass)`; `Int32.Parse(args[1]) (port)`; wraps `args[4]` as
    `"/"+arg+"/"` (folder); takes `args[5]` as the local file; calls
    `SFTPSend(host, port, user, pass, file, folder)`; prints the result (`"0"`).
  - `Encrypt/Decrypt`: passphrase literal **`Carlo*TSSD2!018`**; `Aes.Create()`
    with `KeySize=256`, `BlockSize=128`, `Mode=CBC(1)`, `Padding=PKCS7(2)`;
    32-byte random salt via RNG; `Rfc2898DeriveBytes(pass, salt, 100000)`
    (PBKDF2-**HMAC-SHA1**, .NET default) → `GetBytes(KeySize/8=32)`;
    wire layout `base64(salt[32] ‖ iv[16] ‖ ciphertext)`.
  - `SFTPSend`: `SessionOptions.Protocol = Sftp(0)`,
    **`SshHostKeyPolicy = GiveUpSecurityAndAcceptAny(1)`**, sets host/port/user/pass;
    calls `getProxySettings()`; only if `ProxyAddress.Trim() != ""` adds raw
    WinSCP settings `ProxyMethod="3"`(HTTP), `ProxyHost/Port/Username/Password`;
    `Session.Open`; `TransferOptions.TransferMode = Binary(0)`;
    `Session.PutFiles(localFile, "/folder/", remove=false, opts)`; `result.Check()`.
  - `getProxySettings`: parses `proxy.ini`, reading only `ProxyAddress`,
    `ProxyPort`, `ProxyUser`, `ProxyPassword` (ignores `proxyMode`, `UsePassive`, etc.).
  - PDB path: `D:\code_repository\github_repo\EBIRFormsSFTP\ebfSFTP\ebfSFTP\obj\Debug\ebfSFTP.pdb` (Debug build).
- **Proves:** exact argument contract, crypto, and SFTP session config. **The
  official client performs NO host-key verification.** Payload encryption is NOT
  done here.
- **Enum values resolved from `WinSCPnet.dll`:** `Protocol{Sftp=0,…}`,
  `SshHostKeyPolicy{Check=0,GiveUpSecurityAndAcceptAny=1,AcceptNew=2}`,
  `TransferMode{Binary=0,…}`; `PutFiles(localPath, remotePath, remove, options)`.
- **Unknown at this point:** where the encrypted host/user/pass come from.

## 4. Traced the caller: extracted HTA/JS/VBS

- **Tested:** grepped the running extraction dir
  `%TEMP%\{0B33C1CE-21A8-44A1-8D91-28A10444A6A3}`.
- **Observed:**
  - `js/eBIRTools.vbs` → active `RenameAndSendFile(...)` (authored
    `carlodlc 2026-feb-04`) builds:
    `"…\ebfSFTP.exe" <hostName> <port> <username> <password> <folder> "<sendName>"`
    and runs it hidden, waiting; return code `0` = success. The **old**
    `RenameAndSendFile_original` used `cFTPSend.exe folder file mode host sslport port user pass`.
  - `js/string-util.js:1130` calls
    `RenameAndSendFile(emailFilePath, email, ftpFolder, wsData.mode, wsData.srv, wsData.sslport, wsData.port, wsData.usr, wsData.pass)`
    where `wsData = conService.getConConfig()`.
  - `conService._conWebService` (string-util.js) GETs
    **`tinDispatcherSFTP.php?t=<9-digit TIN>&f=<formType>&v=<version>`** on
    `birgovph.com` (primary) then the **backup connection WS**; parses JSON
    `{mode, server, SSLPort, port, username, password}`; in PROD `srv = server`
    (encrypted), and in DEV/UAT a hard-coded encrypted `srv` blob is used.
  - `js/environment.js`: `currEnvi='PROD'`, `currVer='7.9.6.0'`;
    `userTypeWS.PROD = { primary:'http://birgovph.com/', backup:'http://ws1.birgovph.com/' }`.
- **Proves:** credentials are **not embedded** anywhere. They are fetched
  **per-TIN, per-form at submit time** from a BIR dispatcher and arrive
  **already AES-encrypted**; the shim decrypts them with the embedded passphrase.
  **Backup dispatcher host is `ws1.birgovph.com`** (connection WS backup),
  not `ws2`.
- **Unknown/Next:** confirm the crypto reproduces a real ciphertext.

## 5. Validated the crypto end-to-end (non-production vector)

- **Tested:** `scripts/decrypt_devuat_srv.ps1` — reproduced `Program.Decrypt`
  against the client's own DEV/UAT `srv` blob
  `25s+rBZx/AO+YuDjzPzIBx81hOVx4fhdnOHNysmXar3RpmRnduhtuxoasmUEANldVjKUeaebvHyefVvj5aJQ/+hCjBdF+xwd7GFdWWSbqL8=`.
- **Observed:** decrypts cleanly to **`ftp2.birgovph.com`**; plaintext is exactly
  **17 bytes, no UTF-8 BOM** (`salt=32, iv=16, ct=32`).
- **Proves:** (a) crypto reconstruction is byte-exact; (b) the DEV/UAT SFTP host
  is `ftp2.birgovph.com`; (c) **the official `Encrypt`/`Decrypt` writes UTF-8
  WITHOUT a BOM** — `new StreamWriter(cryptoStream)` uses `UTF8NoBOM`.
- **Consequence for prior art:** the codex `wrap_dispatcher_field` prepends a BOM
  and the codex doc claims "UTF-8 BOM". Harmless for a local round-trip (unwrap
  strips a leading BOM) but **not byte-accurate** to the official helper.

## 6. Payload format

- **Tested:** inspected `savefile/*.xml` vs the actually-uploaded
  `IAF_RDO_Copy/…#email#.xml`.
- **Observed:** `savefile` is plaintext XML; the `#email#` IAF copy is
  high-entropy ciphertext (no XML header).
- **Proves:** `Encrypt.exe` encrypts the payload **before** transport. Payload
  encryption is a **separate, pre-transport step, unchanged old→new**; the
  transport uploads opaque bytes.
- **Unknown:** whether the `Encrypt.exe` payload format itself changed in 7.9.6.x
  (out of transport scope; Buwiz already produces an encrypted IAF for the FTP path).

## 7. Independent Rust crypto verification (offline)

- **Tested:** `scripts/crypto-verify/` — a standalone Rust crate porting
  `unwrap_dispatcher_field` (aes/cbc/pbkdf2/sha1/data-encoding), run against the
  DEV/UAT `srv` vector.
- **Observed:** compiles and prints `unwrapped = "ftp2.birgovph.com" (len 17)` →
  `PASS`.
- **Proves:** the Rust port reproduces the .NET decrypt **byte-for-byte**; the
  crypto in the reference module is verified, not assumed.

## 8. Independent live verification (dummy TIN, read-only, no secrets recorded)

- **Tested (my own probes, not inherited):**
  - `ssh-keyscan` / TCP banner to `ftp2.birgovph.com:22` and `103.56.5.254:21`.
  - `GET http://birgovph.com/tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0`
    (dummy TIN; no auth; no submission).
- **Observed:**
  - **`ftp2.birgovph.com` resolves to `103.56.5.254`** — the *same IP* as the
    retired FTP gateway `103.56.5.254:21`.
  - From this VM, **both 21 and 22 on `103.56.5.254` are unreachable** (egress
    blocked here), so old FTP is confirmed dead and SSH algo capture is not
    possible *from this machine*.
  - Dispatcher returns **HTTP 200, 373 bytes**; keys
    `[SSLPort, mode, password, port, server, username]`; **`mode=2`
    (eBIRForms SFTP), `port=22`, `SSLPort=990`**; `server/username/password`
    are AES-wrapped base64 (lengths 88/88/108). **Login secrets were not
    decrypted or recorded.**
- **Proves (independently of the codex branch):** the dispatcher is live and
  routes 1601C to SFTP on port 22; the field shape and wrap format match the IL;
  the "new provider" framing is wrong — it is the **same host** moving FTP→SFTP.
- **Corroboration:** the codex findings independently saw a FileZilla-Pro SSH
  banner on 22 for the dispatched host. Consistent with `mode=2/port=22` here.
- **Still unproven from this VM:** the SSH KEX/host-key/cipher list (blocked by
  local egress, not by the protocol). `russh` defaults are standard and expected
  to interoperate; confirm on a network with egress or a local server (Layer C).

---

## Current unknowns (need runtime, dummy data only)

1. Exact PROD `mode`/`port` matrix and whether every form routes to SFTP
   (`mode=2`) vs eFPS (`mode=3/4`) — observable via the dummy-TIN dispatcher.
2. Whether the dispatcher pins a fixed port (22) or returns per-RDO ports —
   read the dummy-TIN `port` field.
3. SSH auth negotiation details (KEX/cipher/host-key type) accepted by the server
   — needed to confirm `russh` defaults interoperate. (SSH banner seen; full
   algo list not yet captured.)
4. Whether `Encrypt.exe`'s payload format changed in 7.9.6.x.

## Next tests (smallest experiments)

- Re-fetch `tinDispatcherSFTP.php` for a **dummy** TIN and record only
  `mode`/`port`/field-shape (not decrypted secrets). **Outward-facing → confirm
  before running.**
- Point the Rust harness at a **local** OpenSSH container using `TEST_SFTP_*`
  placeholders and verify filename + `/folder/` path + binary upload.
- Diff `Encrypt.exe` payload of a dummy savefile vs Buwiz's own IAF encryptor
  output to confirm payload parity.
