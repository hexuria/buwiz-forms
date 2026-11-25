# bir-submit

The client of the Buwiz relay server (protocol v2): the device key, tickets,
HPKE sealing, the signed trust list, the WebSocket relay pipe, receipts and
the account's TIN list. A leaf crate: it depends on no `bir-*` crate. The
host app owns the one Buwiz sign-in and hands it in as a `TokenSource`.

Ported from [eBIRForms/eBIRForms](https://github.com/eBIRForms/eBIRForms)
`crates/bir-submit` at commit `14a0f63`. Every file but `src/wire.rs` keeps
the original's name; `src/transport.rs` (the sister's queue transport) is
not ported.

## Deviations from the source

1. `src/wire.rs` holds the pieces of the sister's `bir-core` the client used (`TinKey`, `TinRefusal`, `FailureClass`, `Blame`, the hold/limit/final codes, `capped_wait_until`, `parse_time`); every `bir_core::` path is `crate::wire::`.
2. The seventh TIN refusal, `tin_test_only`: `TinRefusal::TestOnly`, and `TinStatus::TestOnly` (`test_only`) in `tins.rs`. An eighth, `account_practice` (`TinRefusal::AccountPractice`): a practice account never sends.
3. No BIR "sign" observation: no `SignCell`, `last_sign`, `buwiz-bir` header reading; a `bir` object in a body or relay result is ignored.
4. No sign-in: the host's `TokenSource` supplies access tokens (`force_refresh` after 401 `invalid_token`), the login, and hears `session_ended`; no `Submission` wrapper.
5. The device id is its own secret (`device_id`, next to `device_key`); `forget_device()`; `403 device_revoked` forgets the device and ends the host's session; `401 unknown_device` registers a new key once and retries once.
6. `secrets.rs`: service `com.buwiz.bir`, keychain entries `device-key`/`device-id`, `platform_store(data_dir, use_keychain)`, and a keychain write read back before it is trusted.
7. `SubmitConfig::new(server_url)` takes the host's URL and refuses one that is not https or http to this machine; device name `eBIRForms on <os>`.
8. Names sent: User-Agent `buwiz-forms/<version> (<os>)`, `buwiz-client` default `buwiz-forms/<version>`.
9. `tins.rs`: a debug build lets `BUWIZ_ACCOUNT_URL` (https or http to this machine) replace `https://account.buwiz.com`.
10. `trust.rs` is unchanged, pinned key and `lab` feature included.
