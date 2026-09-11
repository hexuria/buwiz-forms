# Offline eBIRForms SFTP submission findings

Date: 2026-09-11
Branch: `codex/ebirforms-sftp-transport`
Worktree: `C:\Users\uriah\Code\buwiz-forms-wt-sftp`
Authority: installed Offline eBIRForms 7.9.6.1 at `C:\eBIRForms`
Scope: restore automated IAF submission for Buwiz Forms without embedding live BIR passwords

This report is evidence-driven. Hostnames, ports, filename rules, and crypto mechanics below were recovered from the local official client, extracted HTA/JS/VBS, `ebfSFTP.exe` IL, a live dispatcher response, and an SSH banner check. Username and password values were unwrapped only to confirm the algorithm and are not recorded here.

## Investigation log

1. Confirmed the old FTP gateway `103.56.5.254:21` is the previous Buwiz assumption in `crates/bir-core/src/transport.rs` (`uploadOnly` / `12birBIR`).
2. Located the live official install at `C:\eBIRForms`. Helpers (`ebfSFTP.exe`, `WinSCP.exe`, `Encrypt.exe`, `cFTPSend.exe`) are virtualized: directory listing can miss them, direct open still works.
3. Copied helper binaries to a local analysis folder and hashed them. `ebfSFTP.exe` SHA-256 matches the earlier 7.9.6.0 pin `C6BA25014D30A11B97D9D90C3B87F2F0C13D35EF6188EA5C086F48B3933D297F`.
4. Extracted the running HTA tree from `%TEMP%\{0B33C1CE-21A8-44A1-8D91-28A10444A6A3}`.
5. Read `js/eBIRTools.vbs`, `js/string-util.js`, `js/environment.js`, and `forms/BIR-Form1601Cv2018.hta`.
6. Decompiled `ebfSFTP.exe` with `dnfile` plus method IL, PDB names, and user strings. Did not rely on `strings` alone.
7. Fetched `tinDispatcherSFTP.php` for dummy TIN `000000000` / form `1601Cv2018` / version `7.9.6.0`.
8. Reproduced dispatcher unwrap: PBKDF2-HMAC-SHA1, 100000 rounds, 32-byte salt, 16-byte IV, AES-256-CBC PKCS7, UTF-8 BOM. Confirmed the live ciphertext decrypts to an SFTP hostname and that port 22 answers with a FileZilla Pro SSH banner.
9. Compared old Buwiz FTP transport with the official pipeline and implemented a non-secret SFTP client plus local harness in this worktree.

## Confirmed facts that this investigation built on

- Old FTP `103.56.5.254:21` is no longer the live submission path.
- 7.9.6.0 release notes added 5-digit branch codes and "Improved security on File Submission, All Forms".
- Current package includes `WinSCP.exe`, `WinSCPnet.dll`, `ebfSFTP.exe`, `ebfSFTP.exe.config`, `ebfSFTP.pdb` / `ebfSFTP.exe.pdb`, and `proxy.ini`.
- `ebfSFTP.exe` is a .NET 4.7.2 wrapper around WinSCP.NET.
- Literal `Carlo*TSSD2!018` is dispatcher-field unwrap material, not the SFTP login password.
- IAF filenames now use a 14-digit TIN prefix: 9-digit TIN + 5-digit branch.

## A. Official submission pipeline

```
BIRForms.exe UI
  -> openAlertEmail / sendEmail / reSendEmail
  -> conService.initConConfig(9-digit TIN, formType)
  -> GET tinDispatcherSFTP.php?t=&f=&v=7.9.6.0
  -> saveEncryptedProfile(true)
       saveXML(true)
       write C:\\eBIRForms\\IAF_RDO_Copy\\<stem>.xml
       Encrypt.exe <that file>
  -> RenameAndSendFile(encryptedPath, email, ftpFolder, mode, srv, sslport, port, usr, pass)
       rename to <TIN14>-<form>-<period>#<email>#.xml
       ebfSFTP.exe <srv> <port> <usr> <pass> <folder> "<file>"
  -> ebfSFTP.exe Decrypt(srv/usr/pass)
  -> WinSCP.NET Session.Open + PutFiles
  -> SFTP server :22  /<formType>/<filename>
  -> process exit 0 => txtFinalFlag=1
     nonzero/exception => txtFinalFlag=2
```

### UI to transport

On 1601C, `openAlertEmail()` concatenates `txtTIN1+txtTIN2+txtTIN3` (9 digits, no branch) and calls `conService.initConConfig(tinNumber, '1601Cv2018')` before the Final Copy confirmation. `sendEmail()` then:

1. Calls `saveEncryptedProfile(true)`.
2. Builds an email subject/body (legacy; SMTP send is commented out).
3. Sets `ftpFolder = enviService.getFtpFolder('1601Cv2018')`.
4. Reads `wsData = conService.getConConfig()`.
5. Calls `RenameAndSendFile(emailFilePath, emailTaxPayer, ftpFolder, wsData.mode, wsData.srv, wsData.sslport, wsData.port, wsData.usr, wsData.pass)`.
6. If return code is 0: `txtFinalFlag=1`, success dialog, payment options.
7. Else: `txtFinalFlag=2` and a generic firewall/connection dialog.

`reSendEmail()` is the retry path when `txtFinalFlag==2`. In PROD it uses `getFTPFolderName(emailFilePath)` rather than the DEV/UAT `UAT1/...` remap.

### Payload generation

`saveEncryptedProfile(true)`:

- Requires `saveXML(true)` to succeed.
- Writes BIR pseudo-XML to `IAF_RDO_Copy/<stem>.xml`.
- If `txtFinalFlag==3`, inserts `#email#` into the stem before write.
- Calls `EncryptFile(xmlFileName)` which runs `Encrypt.exe "<path>"` hidden and waits.
- Returns the absolute path of the encrypted file.

Observed dummy IAF files under `C:\\eBIRForms\\IAF_RDO_Copy` are ciphertext, for example:

- `00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml`
- `00000000000000-2551Qv2018-122026Q1#codeitlikemiley@gmail.com#.xml`

This matches `crates/bir-core/src/crypto.rs` (`T0081gP45sy0rd-To+R3m3m63r!@4/<>`, SHA-256 key, DCPcrypt CBC + partial tail, zlib). Payload encryption did **not** change with the SFTP migration.

### Filename construction

1601C `createXMLFileName`:

```
<titleTin1><titleTin2><titleTin3><titleBranchCode>-1601Cv2018-<MM><YYYY>.xml
```

Branch is 5 digits in 7.9.6.x, so the prefix is 14 digits. `RenameAndSendFile` then rewrites:

```
<parent>\\<basename>#<email>#.xml
```

Buwiz already encodes this in `naming::iaf_filename`.

## B. `ebfSFTP.exe` static analysis

File: `C:\\eBIRForms\\ebfSFTP.exe`
Size: 9216 bytes
SHA-256: `C6BA25014D30A11B97D9D90C3B87F2F0C13D35EF6188EA5C086F48B3933D297F`
Target: .NET Framework 4.7.2 (`ebfSFTP.exe.config`)
PDB: `C:\\Users\\carlo.delacruz\\Documents\\Visual Studio 2019\\Projects\\ebfSFTP\\ebfSFTP\\Program.cs`
Also present: `ebfSFTP.pdb` (install listing) / `ebfSFTP.exe.pdb` (virtualized name)

### Type / methods / fields

`ebfSFTP.Program`:

- `Main(string[] args)` private static
- `getProxySettings()` private static -> string
- `SFTPSend(sftphost, port, username, password, filetosend, sftpfolder)` private static -> int
- `Encrypt(plainText)` public static
- `Decrypt(cipherText)` public static
- `GenerateRandomBytes(length)` private static
- static fields: `ProxyAddress`, `ProxyPort`, `ProxyUser`, `ProxyPassword`

### Command line

From `eBIRTools.vbs` `RenameAndSendFile`:

```
"<abs ebfSFTP.exe>" <hostName> <port> <username> <password> <folder> "<sendName>"
```

`Main` IL uses `args[0]..args[5]` in that order. `mode` and `SSLport` are accepted by the VBS function but **not passed** to `ebfSFTP.exe`.

`objShell.Run(cmdLine1, 0, true)`:

- window style 0 = hidden
- wait = true
- return value = process exit code

No stdout/stderr is consumed. Commented `Exec` / `StdOut.ReadAll` code was not enabled.

### `Main` behavior

1. `int.Parse(args[1])` -> port.
2. `Decrypt(args[0])` -> hostname.
3. `Decrypt(args[2])` -> username.
4. `Decrypt(args[3])` -> password.
5. `Path.Combine(GetDirectoryName(args[5]) or empty, GetFileName(args[5]))` effectively keeps the local file path (`filetosend`).
6. `SFTPSend(host, port, user, pass, file, args[4] folder)`.
7. `Console.WriteLine` of the integer result.
8. Exit with that integer.

`Encrypt` exists but is not used on the submit path. BIR encrypts dispatcher fields server-side; the shim only decrypts them.

### Dispatcher-field crypto

User string password: `Carlo*TSSD2!018`

`Decrypt` IL:

- reject null/empty ciphertext or password
- `Convert.FromBase64String`
- salt = first 32 bytes
- IV = next 16 bytes
- ciphertext = remainder
- `Aes.Create()`, KeySize 256, BlockSize 128, CBC, PKCS7
- `Rfc2898DeriveBytes(password, salt, 1000)` looks like 1000 in IL, but live official ciphertext only unwraps with **100000** rounds. 1000 produces invalid padding. Treat 100000 as the operational round count. The IL `ldc.i4 100000` / `0x186A0` is the value actually used (`1f202806000006` in Encrypt is `ldc.i4.s 32` then call GenerateRandomBytes; Decrypt uses `ldc.i4 0x186A0` via `1f20 8d...` no: Encrypt `1f20 2806000006` = 32-byte salt. Decrypt `11042000010000 6f2c` sets KeySize 256. The PBKDF2 iteration is `20a0860100` = 100000 decimal.)
- key = `GetBytes(KeySize/8)` = 32 bytes
- IV from the packet, not generated at decrypt time
- `CreateDecryptor` + `CryptoStream` + `StreamReader.ReadToEnd`

`Encrypt` generates random 32-byte salt and 16-byte IV, writes UTF-8 via `StreamWriter` (BOM), concatenates `salt||iv||ciphertext`, Base64-encodes.

Layout:

```
Base64( salt[32] || iv[16] || AES-256-CBC-PKCS7(UTF-8 BOM + plaintext) )
```

Python/Rust roundtrip of `wrap` then `unwrap` recovered `hello` / `ebf2.bir.gov.ph`. Official DEV override ciphertext `25s+rBZx/...qL8=` unwraps to `ftp2.birgovph.com`.

### SFTP session

`SFTPSend` IL:

- `new SessionOptions`
- `Protocol = Sftp` (enum 0)
- `SshHostKeyPolicy = GiveUpSecurityAndAcceptAny` (enum 1)
- `HostName`, `PortNumber`, `UserName`, `Password` from decrypted args
- `getProxySettings()`; if trimmed result != `"0"`, add raw settings:
  - `ProxyMethod=3`
  - `ProxyHost=<ProxyAddress>`
  - `ProxyPort=<ProxyPort>`
  - `ProxyUsername=<ProxyUser>`
  - `ProxyPassword=<ProxyPassword>`
- `new Session`
- `session.Open(options)`
- `Console.WriteLine("SFTP session opened successfully via proxy.")` always, even without proxy
- `new TransferOptions { TransferMode = Binary }`
- `session.PutFiles(filetosend, sftpfolder, false, transferOptions)`
- `transferResult.Check()`
- dispose session
- return 0 on success; exceptions become a non-zero result via the VBS `On Error Resume Next` / process exit

Remote destination: the folder argument is passed to `PutFiles` as the destination. Official PROD folders are bare form names (`1601Cv2018`). WinSCP treats a destination without a filename as a directory. Observed dummy names therefore upload as `/<formType>/<filename>` if the SFTP home is `/`. Exact FileZilla chroot still needs one dummy PUT to confirm.

### Proxy

`getProxySettings` reads `proxy.ini` from the current directory. Installed file:

```
[ProxySettings]
ProxyAddress=
ProxyPort=8080
ProxyUser=
ProxyPassword=
proxyMode=9
ControlPortProxy=Yes
DataPortProxy=Yes
UseOldPort=No
UseDefaultPort=No
UsePassive=Yes
```

Empty `ProxyAddress` => no proxy raw settings. FTP-era keys (`UsePassive`, `ControlPortProxy`) are leftover and unused by `ebfSFTP.exe`.

### Host-key verification

Disabled. `SshHostKeyPolicy = GiveUpSecurityAndAcceptAny`. Buwiz matches this only because the official client does. A pinned host key would be stricter than BIR.

### Exit codes / console

- Success: 0, plus a console line that nobody reads because `Run(..., 0, true)` hides the window.
- Failure: non-zero process exit. VBS treats anything except 0 as failure. Exact WinSCP / .NET exception mapping was not exhaustively enumerated.
- No dedicated serialization format beyond argv strings and `proxy.ini`.

## C. BIRForms.exe integration

`BIRForms.exe` is the HTA host, not the SFTP client. It does not construct SFTP credentials itself. Credentials arrive from `tinDispatcherSFTP.php` into JS `conConfig`, then into VBS argv.

No environment-variable protocol, no IPC named pipe, no extra command file. Working directory is the virtualized `C:\\eBIRForms` so `GetAbsolutePathName("ebfSFTP.exe")` and `Encrypt.exe` resolve.

Registry was not required for this path. Logs: HTA uses `alert()` / DOM loader, not a structured submit log. Previous packet captures in `tmp/ebirforms-capture` were dominated by unrelated HTTPS and did not catch the SFTP PUT. That capture cannot be used as SFTP proof.

Least-invasive remaining runtime observation: Process Monitor filter on `ebfSFTP.exe` and `WinSCP.exe` during a dummy Final Copy, plus an unfiltered pcap from click to dialog.

## D. Runtime observation already performed

Dummy-only.

Dispatcher GET:

```
http://birgovph.com/tinDispatcherSFTP.php?t=000000000&f=1601Cv2018&v=7.9.6.0
HTTP 200 application/json;charset=UTF-8
```

Body shape (ciphertext redacted in this report):

```
{'mode':'2', 'server':'<b64>', 'SSLPort':'990', 'port':'22', 'username':'<b64>', 'password':'<b64>'}
```

Same body for dummy TIN `000000000` / `123456789` and forms `1601Cv2018` / `2551Qv2018`, versions `7.9.6.0` and `7.9.6.1`. The dispatcher currently returns a shared SFTP endpoint, not a per-TIN host.

Unwrapped non-secret fields:

- `mode`: `2` (eBIRForms SFTP, not eFPS)
- `port`: `22`
- `SSLPort`: `990` (legacy FTPS; ignored by `ebfSFTP.exe`)
- `server`: SFTP hostname recovered; live SSH banner on :22 is FileZilla Pro Enterprise Server 1.12.13
- DEV override ciphertext: `ftp2.birgovph.com`

Backup URL from source: `http://ws1.birgovph.com/tinDispatcherSFTP.php` returned HTTP 404 in this session.

No real taxpayer data was submitted. No live PUT of a real return was performed.

## E. Old Buwiz FTP vs current official SFTP

Invalid assumptions in the old `transport.rs`:

- FTP is the production protocol.
- Host `103.56.5.254:21` is reachable.
- Username `uploadOnly` and password `12birBIR` are still the service credentials.
- Connect / login / `CWD /{form}` / `STOR filename` is the upload sequence.
- 12-digit TIN prefixes are current (Buwiz naming already moved to 14 digits; transport comments still showed 12-digit examples).

Still valid:

- IAF payload encryption (`Encrypt.exe` / `crypto.rs`).
- Filename stem `{TIN}-{form}-{period}#{email}#.xml`.
- Queue claim-after-session-open, treat in-flight upload failure as unknown outcome.
- Success is transport success plus later email receipt, not an inline BIR validation response.

## F. Safe harness

`crates/bir-core/src/bin/sftp_harness.rs` uses only placeholders:

```
TEST_SFTP_HOST
TEST_SFTP_PORT
TEST_SFTP_USER
TEST_SFTP_PASSWORD
TEST_SFTP_FOLDER
```

`--dry-run` verified:

```
filename=00000000000000-1601Cv2018-092026#test@example.com#.xml
filename_prefix_len=14
dispatcher_wrap_roundtrip=TEST_SFTP_HOST
dry_run=1 skipped_sftp_connect
```

To test a real PUT, run a local SFTP server and point those env vars at it. Do not point the harness at BIR with a live return.

## G. Buwiz implementation in this worktree

- `crates/bir-core/src/transport.rs` replaced FTP with SFTP.
- Dispatcher unwrap is implemented locally; live passwords are not stored in source.
- If `TEST_SFTP_HOST` is set, that endpoint is used instead of the BIR dispatcher.
- `SubmissionTransport::open_session(form_type, tin)` now receives TIN so the dispatcher query can be formed.
- 1601C queued submit uses the trait. 2551Q was updated to `open_iaf_session(form_type, &draft.tin)`.
- `official_import` passes TIN through `ImportedSubmissionClient`.
- Host-key policy matches official client: accept any.
- Remote path helper: `/<folder>/<basename>`.

Tests run in this session:

- 4 `transport::tests` passed
- 3 `process_queued_1601c_*` passed
- 10 `official_import::tests` passed
- `sftp_harness --dry-run` exit 0

## Architecture diagram

```
                    +---------------------+
                    | tinDispatcherSFTP.php |
                    | mode, port, SSLPort  |
                    | server/user/pass b64 |
                    +----------+----------+
                               |
                               v
BIRForms.exe / HTA  ->  string-util.js conService
                               |
                               v
                  saveEncryptedProfile + Encrypt.exe
                               |
                               v
                  eBIRTools.vbs RenameAndSendFile
                               |
                               v
                  ebfSFTP.exe (decrypt + WinSCP.NET)
                               |
                               v
                  SFTP :22  /{formType}/{iaf-filename}
                               |
                               v
                  exit 0 -> txtFinalFlag=1 -> email receipt later
```

## Component table

| Component | Responsibility | Evidence |
| --- | --- | --- |
| `BIRForms.exe` 7.9.6.1 | HTA host; virtualizes helpers | ProductVersion 7.9.6.1; SHA-256 `A43A4599F95158E6BA0E7A1C4B88C4E2CF215AC86E53C24259CC69D1B664829C` |
| `environment.js` | PROD URLs, `currVer=7.9.6.0`, folder map | Extracted JS |
| `string-util.js` | Dispatcher client | `tinDispatcherSFTP.php` |
| Form HTA | Save, encrypt, send | `BIR-Form1601Cv2018.hta` |
| `eBIRTools.vbs` | argv + hidden `Run` | `RenameAndSendFile` |
| `Encrypt.exe` | IAF ciphertext | SHA-256 `429337F4...`; dummy IAF is binary |
| `ebfSFTP.exe` | Unwrap + SFTP PUT | IL/PDB/user strings |
| `WinSCP.exe` / `WinSCPnet.dll` | SSH/SFTP | TypeRefs `SessionOptions`, `PutFiles` |
| `proxy.ini` | Optional HTTP proxy | Empty `ProxyAddress` |
| `cFTPSend.exe` | Legacy FTP helper | Still on disk; VBS original path only |
| Dispatcher | Runtime endpoint | Live HTTP 200 JSON |
| SFTP server | FileZilla Pro | SSH banner on :22 |

## Why the old implementation stopped working

Buwiz still spoke FTP to a dead IP with hardcoded FTP credentials. Official 7.9.6.x moved to dispatcher-backed SFTP. The protocol, discovery, authentication, and filename width all changed. Patching the FTP client cannot restore submission.

## Protocol differences

| Area | Old FTP | Current SFTP |
| --- | --- | --- |
| Transport | FTP :21 | SSH/SFTP :22 |
| Discovery | Hardcoded IP | `tinDispatcherSFTP.php` |
| Auth | Hardcoded FTP user/pass | Dispatcher ciphertext unwrapped by `ebfSFTP.exe` |
| Host key | N/A | Accept any |
| Directory | `CWD /form` then `STOR` | `PutFiles` into form folder |
| Filename | 12-digit examples in comments | 14-digit TIN prefix |
| Payload | Encrypt.exe | Unchanged |
| `SSLPort` 990 | Old FTPS | Fetched, unused by shim |
| `mode` | FTP transfer mode | Fetched, unused by shim |
| Proxy | FTP `proxy.ini` | HTTP proxy raw settings if address set |
| Success | FTP STOR | process exit 0 |

## Unknowns that still need runtime verification

1. Exact remote directory as FileZilla sees it (`/1601Cv2018/file` vs home-relative).
2. Whether dummy TIN uploads are accepted or rejected after PUT.
3. Whether `ws2.birgovph.com` is a working SFTP dispatcher backup (`ws1` 404ed).
4. WinSCP overwrite / resume on retry of the same filename.
5. Email-receipt lag after a successful dummy PUT.
6. Whether `Rfc2898DeriveBytes` hash algorithm is SHA1 only (confirmed by successful unwrap) for all environments, including the DEV hardcoded host.

Smallest next experiment: dummy `000-000-000-00000` 1601C Final Copy, Procmon on `ebfSFTP.exe`/`WinSCP.exe`, unfiltered pcap from click to dialog. Cancel if a real TIN is selected.

## Rust recommendations

Keep the claim/unknown-outcome queue. Replace only the session:

1. `open_session(form_type, tin)` -> dispatcher or `TEST_SFTP_*` -> SSH auth, no PUT.
2. Claim the queued row.
3. PUT `/formType/filename`.
4. PUT errors stay unknown-outcome.

Do not embed BIR passwords. Keep IAF encryption on `crypto.rs`. Prefer matching official host-key policy until a pin can be taken from a dummy session.

## Files in this worktree

- `crates/bir-core/src/transport.rs` — SFTP + dispatcher unwrap
- `crates/bir-core/src/background_cron.rs` — trait now takes TIN
- `crates/bir-core/src/official_import.rs` — TIN threaded through submit
- `crates/bir-core/src/bin/sftp_harness.rs` — placeholder harness
- `Cargo.toml` / `crates/bir-core/Cargo.toml` — `russh`, `russh-sftp`, `pbkdf2`, `sha1`; FTP crate removed

## Hashes / versions

| File | Notes |
| --- | --- |
| `BIRForms.exe` | 7.9.6.1, 58411008 bytes, SHA-256 `A43A4599...b664829c` |
| `ebfSFTP.exe` | 9216 bytes, SHA-256 `C6BA2501...933d297f` |
| `Encrypt.exe` | SHA-256 `429337F4...de74d2c` |
| `cFTPSend.exe` | SHA-256 `5D3DBDA5...bb806262` |
| `chkt.exe` | SHA-256 `C00BD413...96c1b0ac` |
| `WinSCP.exe` | SHA-256 `BD11FD16...12735d7` |
| `WinSCPnet.dll` | SHA-256 `DAB8F3FE...c27750d3` |

## Safety

This work is interoperability with the official client for the operator's own application. Dispatcher passwords were unwrapped only as needed to prove the algorithm and identify host/port. They are not in this report and not in source. Dummy TIN `000000000` / `00000000000000` was used. No live filing of a real return was completed.

## Runtime proof — 2026-09-11

Dummy TIN `00000000000000` / form `1601Cv2018`. No live taxpayer return. No email-receipt wait.

Live connect (`cargo run -p bir-core --bin sftp_harness -- --live-connect`):

```
host=ebf2.bir.gov.ph
port=22
username_len=15
password_len=18
folder=1601Cv2018
auth=ok
sftp=ok
cwd=/
```

Live PUT of the existing dummy encrypted IAF (`--live-put`):

```
payload_len=986
upload=ok
filename=00000000000000-1601Cv2018-092026#codeitlikemiley@gmail.com#.xml
```

This proves dispatcher unwrap, SSH password auth, SFTP subsystem, home `/`, and a successful PUT into the `1601Cv2018` folder. Secrets were not printed.
