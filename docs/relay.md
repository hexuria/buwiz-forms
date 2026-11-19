# Relay (phone sending) and the local mock relay

A phone cannot reach the BIR's SFTP gateway the way the desktop app does, so
the NSH app on iOS and Android sends a filed return through the Buwiz relay.
The production relay is not built. This page defines the minimal contract the
app speaks and the **local mock relay** that implements it for development.
Nothing here can reach the BIR or any other live endpoint.

## Contract (v1)

One origin, `BIR_RELAY_URL`. The app accepts only `http://` on `127.0.0.1`,
`::1` or `localhost`, with no path, query, credentials or fragment
(`bir_core::transport::loopback_relay_url`). Anything else is refused before a
byte is sent, and the app's send guard does not start the submission worker.

### `POST /v1/submissions`

The body is the encrypted IAF file, byte for byte.

| Header | Value |
| --- | --- |
| `content-type` | `application/octet-stream` |
| `x-relay-form` | the form folder, e.g. `1701Qv2018` (letters, digits, `.`, `_`, `-`) |
| `x-relay-filename` | the IAF file name, no path |
| `x-relay-sha256` | lowercase hex SHA-256 of the body |

Answers (JSON):

| Status | Body | The app |
| --- | --- | --- |
| `202` | `{"status":"accepted","receipt_id":"relay-…","sha256":"…","forwarded":"mock-bir"\|"none",…}` | the return is **Submitted** |
| `4xx` | `{"status":"rejected","error":"…"}` | rejected (`TransportError::Rejected`), not retried as sent |
| `5xx` | `{"status":"failed","error":"…"}` | transport error (`TransportError::Http`), the queue retries |

`202` means the relay holds the file. Confirmation from the BIR (the email
receipt) stays outside this contract, as on the desktop.

### `GET /v1/health`

`200 {"ok":true,"mock":true}`.

## Where the app decides

- `bir-core/src/transport.rs`: `resolve_sftp_endpoint` returns
  `ResolvedSftpTarget::Relay` when `BIR_RELAY_URL` is set (before the dry run;
  setting it is the ask to use the relay), and the upload goes to the relay
  instead of SFTP. The source logs as `relay`.
- `nsh/src/guards.rs`: the send guard answers `SendMode::Relay` (`doorStatus().sending
  === "relay"`) only when the relay URL is loopback and its port answers. A set
  but bad or silent relay is `Refused`: the worker does not start. `BIR_SFTP_LIVE`
  is still removed and the Dev Mode test server still refuses.
- The phone app gets `BIR_RELAY_URL` (and the dev variables) from the launch
  environment. `nsh/nsh.json` declares no `capabilities` on purpose: any
  declaration denies everything it does not list, and the desktop print
  preview's web view shows a `file:` page that no grant covers. In the iOS
  simulator, `xcrun simctl launch` hands the app every `SIMCTL_CHILD_<name>`
  variable of `nsh run`'s environment as `<name>`, so export those (below).

## The local mock relay

```sh
just mock-bir                 # optional: the mock BIR SFTP server, port 2222
just mock-relay               # the relay, http://127.0.0.1:8787
```

`just mock-relay` (`crates/bir-core/src/bin/mock_relay.rs`, feature
`mock-relay`, logic in `bir_core::mock_relay`) listens on loopback only.
Each accepted file is kept under `target/mock-relay/inbox/{form}/{file}` with a
receipt in `target/mock-relay/receipts/`. When `target/mock-bir/app.env` exists
(a running `just mock-bir` writes it) the relay hands the file on to that mock
over SFTP, pinned to its host key; a non-loopback host in that file is refused.

| Option | Does |
| --- | --- |
| `--port N`, `--bind IP` | where it listens (8787, `127.0.0.1`; only loopback) |
| `--dir PATH` | inbox and receipts (default `target/mock-relay`) |
| `--mock-bir FILE`, `--no-forward` | hand-off target, or none |
| `--fail none\|reject\|unavailable` | answer 202, 422 or 503 |

The iOS simulator and the Android emulator reach the Mac's loopback
differently: the iOS simulator shares it (`http://127.0.0.1:8787`); an Android
emulator needs `adb reverse tcp:8787 tcp:8787` so the same URL works.

### Phone run against the mock

```sh
just mock-bir & just mock-relay &
SIMCTL_CHILD_BIR_RELAY_URL=http://127.0.0.1:8787 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 \
  SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
  nsh run --target ios   # from nsh/, with PATH and CARGO_TARGET_DIR as the nsh-* recipes set them
```

Queue a validated demo return (Juana Santos): the relay prints the file, the
mock BIR prints the upload, and Filings shows the return as Submitted.
