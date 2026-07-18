# 0619E/F January 2018 queue certification audit

## Verdict

`0619Ev2018` and `0619Fv2018` remain manual/external for electronic submission. Their reviewed encrypted files are hash-locked outbound candidates, but the Rust writer does not reproduce those ciphertexts byte-for-byte, and the accepted transport, dynamic credentials, BIR validation response, and crash-safe persistence contracts are not certified. Both capability records and model constants therefore keep `queue_submission=false`.

## Locked evidence

The audit used eBIRForms package `7.9.5.0` (`BIRForms.exe` SHA-256 `3d087545564531de1fbe8fb28f086ce6398e18608c54a0ea33353042665917eb`) and the reviewed sources in `/Users/uriah/Downloads/forms/0619E` and `/Users/uriah/Downloads/forms/0619F`.

| Evidence | 0619E | 0619F |
| --- | --- | --- |
| Editable XML | 58 fields, `a6f21e372a1ce6d707ede13f2447290683ab302d859c3b684a06c55788cbfade` | 59 fields, `f7a1f2481104b8c23b22f92aef263ae02f768227ec6961cb10e4daf0817f8a18` |
| Encrypted candidate | `1c49950df1197906bb73ddbb5d0f5f5e1c3f488f376e05b6d53febc1b32016ab` | `d561ce34a44a732e52047552c6d4c0b975b3c45042dc0aba4907abfda89b53fb` |
| Decrypted candidate | 59 fields, `6a1911a359efedae7e35fa21c2def9af62c7cc1194e768d4bca5f3193c33fef4` | 60 fields, `087116111a5222233d65b9f63bda8bcde4203f072bbb0925ae57c1cadc29c067` |
| Embedded HTA | resource 141, decoded SHA-256 `a0ef4d8958b28e63c511e7bc961e67e8ec254a6cb5f4e9f93f6d92fc9fe56f47` | resource 142, decoded SHA-256 `f9e23eafae2bf8b04e0996d0b4bdb902ae898e583db934941257088bce9a0f62` |

Rust decrypts and semantically replays both encrypted candidates. Current Rust re-encryption instead produces SHA-256 `46903e58ce8b09500dc87fc63823209b8ab119990d6aee0d8073c5968a32f3a6` for 0619E and `d720ec50eea6ebbdf2c211df2b1b8eac0ae0ba275112fc26e6671450b3ac4306` for 0619F. Those hashes differ from the locked candidate hashes, so exact outbound generation remains unproven.

## Native outbound path

The hash-locked HTAs use this path:

`sendEmail -> saveEncryptedProfile(true) -> saveXML(true) -> EncryptFile -> RenameAndSendFile`

The package evidence establishes the following behavior:

- `saveEncryptedProfile(true)` serializes the complete form element set and writes `txtFinalFlag=0` in the reviewed candidates.
- `eBIRTools.vbs` invokes external `Encrypt.exe` and `cFTPSend.exe` processes. Neither executable appears in the reviewed package manifest.
- Production folder names are `0619E` and `0619F`, but the connection is not static. `string-util.js` synchronously calls `http://birgovph.com/tinDispatcher.php?t={tin}&f={form}&v=7.9.5.0`, with `http://ws2.birgovph.com/` as fallback, and consumes runtime `mode`, `server`, `SSLPort`, `port`, `username`, and `password` values.
- The reviewed package contains no response from that dispatcher and no hash-locked helper binaries, so it cannot establish the current endpoint, protocol mode, ports, service credentials, helper exit semantics, or server-side acceptance rules.
- The HTA treats `cFTPSend.exe` exit code `0` as upload success, immediately labels the result subject to BIR validation, and tells the filer to retain a later email as evidence. It does not parse an authoritative BIR acceptance response.

The hard-coded FTP host and credentials in the current Rust transport are not present in the reviewed package resources. They cannot be extended to 0619E/F merely because 1601C and 2551Q already use that transport.

## Credential and persistence boundary

The reviewed encrypted candidates contain blank `ebirOnlineConfirmUsername`, `ebirOnlineUsername`, and `ebirOnlineSecret` fields and `txtEnroll=Y`. The native registered-user flow can copy the entered password into `ebirOnlineSecret` before encryption. Rust therefore rejects nonblank credential fields or unreviewed enrollment/export-device values during 0619E/F import and excludes those controls from persisted unmodeled XML state.

The existing 1601C and 2551Q queue implementations freeze a reviewed field map behind a fingerprint, atomically establish a durable pre-network claim, reject stale replacements, and preserve an unknown network outcome for manual reconciliation. No equivalent 0619E/F database path exists, and the background worker dispatches only the two certified forms. The 0619E/F lifecycle methods now refuse Queued, Submitted, Confirmed, Paid, and retry-state advancement even when a caller injects a later status.

## Evidence required before promotion

Queue support must remain false until all of the following are reviewed together:

1. Exact Rust byte-for-byte reproduction of the reviewed encrypted candidates, plus hash-locked `Encrypt.exe` and `cFTPSend.exe` binaries matching the reviewed package generation, including their argument, encryption, protocol, and exit-code behavior.
2. A current, authorized dispatcher response proving the accepted endpoint, transfer mode, ports, folder identity, and service-credential handling for both exact form revisions.
3. A captured successful 0619E and 0619F submission with authoritative BIR confirmation evidence and a deterministic parser that distinguishes upload completion from BIR validation.
4. Dedicated immutable queue persistence for each form: reviewed fingerprint, current-profile revalidation, atomic pre-network claim, cancellation rules, and unknown-outcome reconciliation.
5. Tests proving that the exact queued snapshot produces the reviewed outbound payload without persisting taxpayer login secrets, and that every missing or changed evidence item fails before network I/O.
