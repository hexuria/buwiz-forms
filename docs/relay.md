# The Buwiz relay and the local mock relay

A release build sends a filed return through the Buwiz relay (protocol v2,
`crates/bir-submit`) on every platform: macOS, Windows, Linux, iPhone, iPad
and Android alike. See [Sending: the Buwiz relay](#sending-the-buwiz-relay) at
the end. Most of this page is about the **local mock relay**, a development
tool with its own small contract (v1, below) that takes the file on this
computer. Nothing the mock relay does can reach the BIR or any other live
endpoint.

## Contract (v1)

One origin, `BIR_RELAY_URL`. The app accepts only `http://` on `127.0.0.1`,
`::1` or `localhost`, with no path, query, credentials or fragment
(`bir_core::transport::loopback_relay_url`). Anything else is refused before a
byte is sent, and the app's send guard does not start the submission worker.
Relay use requires a debug build or the explicit `mock-relay` feature; a release
build without that feature refuses `BIR_RELAY_URL` before connecting.

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
| `4xx` except `408`/`429` | `{"status":"rejected","error":"…"}` | refused, nothing kept (`TransportError::RelayRefused`): the claim is let go and the return goes back to **Draft** with the reason |
| `408`, `429`, `503` | `{"status":"failed","error":"…"}` | unavailable, nothing kept (`TransportError::RelayUnavailable`): the claim is let go, the return stays **Queued** and is retried after 1, 2, 4, 8 minutes (the fifth failure returns it to Draft) |
| other `5xx` (`502` = the mock BIR hand-off failed) | `{"status":"failed","error":"…"}` | outcome unknown (`TransportError::Http`): the file may have gone on, so the return keeps its claim and is never sent again without a person |

`202` means the mock relay holds the file, not that it reached the BIR. The app
uses dry-run notices and alerts and does not poll for a BIR email receipt.
Client logs record only the form code and byte length; the mock's console omits
the receipt body, whose IAF filename contains the taxpayer's TIN and email.

### `GET /v1/health`

`200 {"service":"buwiz-mock-relay","version":1,"ok":true,"mock":true}`.
Both startup and worker probes require HTTP 200 and all four identity fields.
For `localhost`, startup probes IPv4 and IPv6 loopback, then replaces the configured
URL with the numeric address that passed. Worker health checks and uploads use
that same address, never a fresh resolution of `localhost`.

**Upgrading:** a relay still returning `{"ok":true,"mock":true}` is out of date
and is refused. Stop it and use the restart command in the refusal, which preserves
the checked bind address and port. For `BIR_RELAY_URL=http://[::1]:9000`, run
`just mock-relay --bind ::1 --port 9000` to rebuild and restart it, then restart the
app so its startup guard can enable the submission worker. The recipe forwards
these arguments to the relay binary; for `localhost`, the advice uses the numeric
loopback address that answered the health check.

The app asks it before it claims a queued return: a relay that does not answer
there leaves the return Queued, unclaimed, and retried after the back-off.

### Android development APK: private dev.env

**Debug builds or compile-time opt-in only**: before the door and its guards
run, the Android app reads `<engine data_dir>/dev.env` once per process when
`debug_assertions` is enabled or it was compiled with `BUWIZ_ANDROID_DEV=1`.
`nsh package` builds release APKs; opt in from `nsh/` with:

```sh
BUWIZ_ANDROID_DEV=1 nsh package --target android --abi x86_64
```

Packaged APKs are not debuggable, so `adb shell run-as` cannot write their
private configuration. Bake synthetic-test defaults into the opted-in APK instead:

```sh
CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true BUWIZ_ANDROID_DEV=1 BUWIZ_ANDROID_DEV_ENV='EBIR_TEST_ENV=1;BIR_AUTH_BYPASS=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1;BIR_RELAY_URL=http://127.0.0.1:8787' nsh package --target android --abi x86_64
```

The sign-in bypass (`BIR_AUTH_BYPASS`) only exists in builds with
`debug_assertions`, hence `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true`; without it
the app shows the real Buwiz sign-in.

`BUWIZ_ANDROID_DEV_ENV` accepts the same allowlisted `KEY=VALUE` entries,
separated by semicolons or newlines, with the same validation and 16 KiB limit.
Duplicate keys within either source are rejected. If a private `dev.env` exists,
its entries override baked defaults per key; omitted keys retain their baked
values. Both sources must validate before any configuration is applied.
A missing file is fine: the baked defaults still apply.

Neither source accepts a secret. `GPUI_AGENT_TOKEN` in particular is refused in
`BUWIZ_ANDROID_DEV_ENV` and in `dev.env` (startup fails, as for any unknown
key): a baked value can be read out of the built library by anyone given the
build. `just nsh-run-agent android` bakes only the switches
(`GPUI_AGENT=1`, `GPUI_AGENT_ADDR`, the dry-run settings) and hands the token
over at run time: once the development host is installed it writes
`files/nsh/dev.goldcoders.bir/agent-token` with `adb shell run-as`, the
token on standard input (not in any process's arguments, not an intent extra).
The app's data plane waits up to 10 s for that file when `GPUI_AGENT=1`, reads
it once and deletes it; without it the listener is refused.

Android installs these values in bir-core's process-wide configuration store
before the door starts its database or workers; it does not modify the process
environment. Core, account-gate and transport readers consult that store before
the launch environment. The Android send guard also clears the live-send
acknowledgement and, unless a mock is selected, forces dry-run mode through the
store, not OS environment writes.

Both build-time values use `option_env!`; Cargo tracks their changes
automatically (the app has no `build.rs`). Setting them only at app runtime
does not enable or change the loader. Production release builds without the
opt-in neither embed the baked configuration nor read the file, even when
`run-as` is available. Do not distribute opted-in APKs as production builds
or put secrets in baked configuration.
With the current NSH host this is normally
`files/nsh/dev.goldcoders.bir/dev.env` relative to `run-as`'s working
directory (NSH uses `getFilesDir()/nsh/<app id>`). The file must exist before
the first native Door call. Restart the app after changing it.

### Android emulator access commands

An opted-in Android dev build with the exact configured value `EBIR_TEST_ENV=1`
also polls `<engine data_dir>/dev-command` every 500 ms, even with the ordinary
shell polls disabled or Android PrintActivity in front. Commands are `lock`
(the existing Ctrl+L handler) and `logout` (local Settings Sign out); the file is
deleted before dispatch. Unknown commands are consumed, rejected and logged.
No mailbox timer starts on desktop or when either gate is off.

The current host supplies `getFilesDir()/nsh` as `EngineConfig.support_dir`, with
no explicit `data_dir`; the engine appends the app ID. Thus the mailbox is
`/data/data/dev.goldcoders.bir/files/nsh/dev.goldcoders.bir/dev-command`
(`/data/user/0/...` is Android's equivalent path for user 0). On the rooted test
emulator, atomically publish a complete command with:

```sh
adb shell "su 0 sh -c 'cd /data/data/dev.goldcoders.bir/files/nsh/dev.goldcoders.bir && printf lock > dev-command.next && mv dev-command.next dev-command'"
adb shell "su 0 sh -c 'cd /data/data/dev.goldcoders.bir/files/nsh/dev.goldcoders.bir && printf logout > dev-command.next && mv dev-command.next dev-command'"
```

The app must already have started so its data directory exists. Receipt logs
include a Unix-millisecond timestamp through tracing. The mailbox does not
launch or activate an Activity; the existing print-revocation handler still
dismisses an unsubmitted preview as documented in the migration lifecycle model.

### Accepted dev.env keys

Only these keys are accepted:
`EBIR_TEST_ENV`, `BIR_AUTH_BYPASS`, `BUWIZ_NO_KEYCHAIN`, `BIR_SFTP_DRY_RUN`,
`BIR_RELAY_URL`, and the agent data plane's `GPUI_AGENT` and `GPUI_AGENT_ADDR`
(never its token, see above). The format is literal `KEY=VALUE`, with surrounding whitespace
trimmed; blank lines and full-line `#` comments are allowed. No `export`,
quoting, variable expansion or inline comments. Unknown/duplicate keys,
malformed lines, invalid UTF-8 or files over 16 KiB refuse startup. A missing
file is fine. No partial configuration is applied on parse failure.

Create a local file named `dev.env` containing:

```dotenv
EBIR_TEST_ENV=1
BIR_AUTH_BYPASS=1
BUWIZ_NO_KEYCHAIN=1
BIR_SFTP_DRY_RUN=1
BIR_RELAY_URL=http://127.0.0.1:8787
```

For a debuggable development APK (not a normal `nsh package` APK), the optional
private-file override can be installed as follows:

```sh
adb shell am force-stop dev.goldcoders.bir
adb shell run-as dev.goldcoders.bir mkdir -p files/nsh/dev.goldcoders.bir
adb shell run-as dev.goldcoders.bir sh -c '"cat > files/nsh/dev.goldcoders.bir/dev.env"' < dev.env
adb reverse tcp:8787 tcp:8787
# Start both mocks, then launch the app normally.
```

For a packaged APK with baked defaults, skip the `run-as` commands and use
`adb reverse tcp:8787 tcp:8787` before launching with both mocks running.

The `run-as` path is app-private, not shared external storage. Verify the actual
engine data path if using a different host/app ID. `EBIR_TEST_ENV=1` gives
each engine a fresh scratch database: this configuration is for synthetic
phone tests, not persistent real taxpayer work. The normal Final Copy and
queue-admission checks still apply.

## Where the app decides

- `bir-core/src/transport.rs`: `resolve_sftp_endpoint` returns
  `ResolvedSftpTarget::Relay` when `BIR_RELAY_URL` is set (before the dry run;
  setting it is the ask to use the relay), and the upload goes to the relay
  instead of SFTP. The source logs as `relay`.
- `nsh/src/guards.rs`: the send guard answers `SendMode::Relay` (`doorStatus().sending
  === "relay"`) only when the relay URL is loopback, its port answers, and
  `GET /v1/health` identifies `buwiz-mock-relay` version 1 with `ok` and `mock`
  both true. For `localhost`, it tries both loopback families and pins the
  configured upload URL to the one that passed. A set
  but bad or silent relay is `Refused`: the worker does not start. `BIR_SFTP_LIVE`
  is still removed and the Dev Mode test server still refuses.
- The iOS app gets `BIR_RELAY_URL` (and the dev variables) from the launch
  environment; Android uses the baked/private configuration described above.
  `nsh/nsh.json` declares no `capabilities` on purpose: any
  declaration denies everything it does not list, and the desktop print
  preview's web view shows a `file:` page that no grant covers. In the iOS
  simulator, `xcrun simctl launch` hands the app every `SIMCTL_CHILD_<name>`
  variable of `nsh run`'s environment as `<name>`, so export those (below).

## The local mock relay

```sh
just mock-bir                 # optional: the mock BIR SFTP server, port 2222
just mock-relay               # the relay, http://127.0.0.1:8787
just mock-account             # separate terminal: local sign-in, http://localhost:3008
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
just mock-account & just mock-bir & just mock-relay &
SIMCTL_CHILD_BIR_RELAY_URL=http://127.0.0.1:8787 SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1 \
  SIMCTL_CHILD_EBIR_TEST_ENV=1 SIMCTL_CHILD_BIR_AUTH_BYPASS=1 SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1 \
  nsh run --target ios   # from nsh/, with PATH and CARGO_TARGET_DIR as the nsh-* recipes set them
```

Queue a validated demo return (Juana Santos): the relay prints the file, the
mock BIR prints the upload, and Filings shows the return as Submitted.

## Submission safety and mock limitations

The worker persists PUT-start before posting. A 202 accepted response marks the
return Submitted, including inbox-only mode (`forwarded: "none"`); it does not
mean BIR confirmation. A failed, rejected, disconnected, or timed-out operation
after PUT-start leaves the claim pending for manual reconciliation, without
automatic retry or SFTP fallback. Pre-upload failures retain the existing retry
behavior. The client timeout is 60 seconds.

This contract has no idempotency key or status endpoint. Repeating a POST writes
the inbox file again and can forward it again, even though its hash-derived
receipt ID is unchanged. Receipt-file writes are best-effort.

Authentication is out of scope for this local mock. Use synthetic taxpayer data
only. A real relay needs TLS, authenticated clients, taxpayer authorization,
durable idempotency and custody records, audited access, rate limits, protected
storage, retention/deletion policy, and reconciliation of ambiguous hand-offs.

## On a physical iPhone

Not run on a device yet (none attached to the build machine). `nsh run --target ios`
only boots a simulator, so a device needs a signed package:

1. Apple side: an Apple Developer team (a free personal team works, but its
   profiles expire after 7 days). Get these from the team, or let Xcode create
   them (Xcode > Settings > Accounts, then any project with automatic signing):
   - an **Apple Development** certificate in the login keychain. Check with
     `security find-identity -v -p codesigning`, which prints
     `Apple Development: Name (TEAMID)`;
   - an App ID for `dev.goldcoders.bir`, the `id` in `nsh/nsh.json`;
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
     -e '{"EBIR_TEST_ENV":"1","BIR_AUTH_BYPASS":"1","BIR_SFTP_DRY_RUN":"1"}' dev.goldcoders.bir
   ```

   Without `--sign` and `--profile` you get an unsigned `.app` that a device
   refuses. `-e` (or `DEVICECTL_CHILD_<name>` variables) is the device's
   version of the simulator's `SIMCTL_CHILD_<name>`. The Rust side reads these
   variables from the process environment.
4. A device build is a release build, so `BUWIZ_NO_KEYCHAIN` is not forced:
   the app uses the iOS keychain unless you pass `BUWIZ_NO_KEYCHAIN=1`.
   For the same reason, launched without `BIR_SFTP_DRY_RUN=1` (or
   `EBIR_TEST_ENV=1`) and signed in to a real account, it files through the
   production Buwiz relay, as the App Store build does.

## Sending: the Buwiz relay

The local mock relay (`BIR_RELAY_URL`) stays what it is: a development tool,
honoured only in a debug build or one with the `mock-relay` feature, and only
on loopback. It is not how a build that ships sends. Protocol v2 replaced the
production design this page once sketched (a v1 relay origin with a bearer
header); there is no "real relay" URL to point `BIR_RELAY_URL` at.

### Every platform, one way

Real sending, on a desktop and on a phone alike, goes through the Buwiz relay
(`crates/bir-submit`, used by `crates/bir-core/src/relay.rs`):

- the app signs each request with a **device key**, made on first use and kept
  in the OS credential store (the keychain on macOS and iOS; a file in the
  app's private data folder on Android or where there is no store);
- the return is **sealed** on the device for the sending service, so Buwiz's
  server sees it only to send it;
- the **app opens the connections** to the BIR's servers itself, only to the
  hosts named on Buwiz's **signed trust list** (trusted through a public key
  compiled into the build), and pipes the sending service's session through
  them;
- the app keeps Buwiz's signed receipt. It needs the person's Buwiz sign-in
  and a TIN approved on that account.

`bir_core::transport` decides the route: in a release build with no dry-run
flag, the default is the Buwiz relay (`SendMode::Buwiz`, `doorStatus().sending
=== "buwiz"`), whatever the platform. A practice account only simulates and
never calls the relay; `BIR_SFTP_DRY_RUN=1` and a test environment are dry
runs, as before.

### A development build

A development build never sends through production. It has nowhere to send
by default, and sends through the Buwiz relay only when asked to and only to a
Buwiz server on this computer whose trust list a test key signed:

| Variable | Value |
| --- | --- |
| `BUWIZ_SERVER_URL` | a loopback origin, e.g. `http://localhost:3008` |
| `BUWIZ_TRUST_KEYS` | the test key(s) that signed that server's trust list; production's key is refused |
| `BUWIZ_RELAY_SEND` | `1`: the ask to send through it |

The send guard also requires that server's port to answer; otherwise the
worker does not start. Whether anything reaches the BIR is that server's mode.

From a phone: the iOS Simulator shares the Mac's loopback, so
`http://localhost:3008` works as is (pass the variables as
`SIMCTL_CHILD_<name>`, as above). An Android emulator reaches it with
`adb reverse tcp:3008 tcp:3008`. That is only the network half on Android:
the three variables are not among the [accepted dev.env keys](#accepted-devenv-keys)
yet, and `BUWIZ_TRUST_KEYS` is read from the process environment, which an
Android app process does not have.
