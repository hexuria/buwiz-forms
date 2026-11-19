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
| `4xx` | `{"status":"rejected","error":"…"}` | refused, nothing kept (`TransportError::RelayRefused`): the claim is let go and the return goes back to **Draft** with the reason |
| `503` | `{"status":"failed","error":"…"}` | unavailable, nothing kept (`TransportError::RelayUnavailable`): the claim is let go, the return stays **Queued** and is retried after 1, 2, 4, 8 minutes (the fifth failure returns it to Draft) |
| other `5xx` (`502` = the mock BIR hand-off failed) | `{"status":"failed","error":"…"}` | outcome unknown (`TransportError::Http`): the file may have gone on, so the return keeps its claim and is never sent again without a person |

`202` means the relay holds the file. Confirmation from the BIR (the email
receipt) stays outside this contract, as on the desktop.

### `GET /v1/health`

`200 {"ok":true,"mock":true}`.

The app asks it before it claims a queued return: a relay that does not answer
there leaves the return Queued, unclaimed, and retried after the back-off.

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

## On a physical iPhone

Not run on a device yet (none attached to the build machine). `nsh run --target ios`
only boots a simulator, so a device needs a signed package:

1. Apple side: an Apple Developer team (a free personal team works, but its
   profiles expire after 7 days). Get these from the team, or let Xcode create
   them (Xcode > Settings > Accounts, then any project with automatic signing):
   - an **Apple Development** certificate in the login keychain. Check with
     `security find-identity -v -p codesigning`, which prints
     `Apple Development: Name (TEAMID)`;
   - an App ID for `dev.goldcoders.bir.nsh`, the `id` in `nsh/nsh.json`;
   - the iPhone registered by its UDID (`xcrun devicectl list devices`);
   - a **development provisioning profile** for that App ID, certificate and
     device (`.mobileprovision`).
2. On the iPhone: Settings > Privacy & Security > Developer Mode on (iOS 16+),
   and trust the Mac when you connect it.
3. Build and install (from `nsh/`, with `PATH` and `CARGO_TARGET_DIR` as the
   `nsh-*` recipes set them):

   ```sh
   nsh package --target ios --sign "Apple Development: Name (TEAMID)" \
     --profile ~/Downloads/buwiz_dev.mobileprovision        # writes .nsh/out/<name>.ipa
   xcrun devicectl device install app --device <UDID> .nsh/out/<name>.ipa
   xcrun devicectl device process launch --device <UDID> \
     -e '{"EBIR_TEST_ENV":"1","BIR_AUTH_BYPASS":"1","BIR_SFTP_DRY_RUN":"1"}' dev.goldcoders.bir.nsh
   ```

   Without `--sign` and `--profile` you get an unsigned `.app` that a device
   refuses. `-e` (or `DEVICECTL_CHILD_<name>` variables) is the device's
   version of the simulator's `SIMCTL_CHILD_<name>`. The Rust side reads these
   variables from the process environment.
4. A device build is a release build, so `BUWIZ_NO_KEYCHAIN` is not forced:
   the app uses the iOS keychain unless you pass `BUWIZ_NO_KEYCHAIN=1`.

## Mock relay vs a real relay

| | Mock (today) | Real relay |
| --- | --- | --- |
| URL | `BIR_RELAY_URL=http://127.0.0.1:8787` | e.g. `https://relay.example.com` |
| Allowed by the app | yes | **refused**: the worker does not start |
| From a device | no: `127.0.0.1` is the phone itself, not the Mac | needs the changes below |

What refuses it today, on purpose:

- `bir_core::transport::loopback_relay_url` (`crates/bir-core/src/transport.rs`)
  accepts only an `http://` origin on `127.0.0.1`, `::1` or `localhost`, with
  no user, path, query or fragment. Anything else is a `Config` error, and the
  upload never starts.
- `relay_target` in `nsh/src/guards.rs` then requires that loopback port to
  answer. Otherwise the send mode is `Refused`.

To test a real relay (a deliberate, reviewed change), you need:

1. Relay origin allowlist: replace the loopback-only rule with an explicit
   list. For example, accept `https://<host>` only when it equals an origin
   compiled into the build (`option_env!("BUWIZ_RELAY_ORIGIN")`), and keep
   refusing every other origin. Keep `http://` loopback for the mock.
2. In `relay_target`, the reachability probe dials the URL's host and port.
   For `https://` without a port, use 443, not 80.
3. Authentication: the v1 contract has none. Add a header, e.g.
   `authorization: Bearer …`, read from the keychain and never from a file
   in the repository.
4. App Transport Security: iOS blocks plain `http://` to anything but
   loopback. Use `https://`. A LAN `http://` mock needs
   `NSAllowsLocalNetworking` and `NSLocalNetworkUsageDescription` under
   `ios.infoPlist` in `nsh/nsh.json`.
5. The relay decides where returns go. Point a test relay at a mock or test
   BIR endpoint: the app cannot tell what the relay forwards to.

Until those changes land, the closest device test is the dry run (no relay
URL), or the mock relay from the simulator.
