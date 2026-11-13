# Mock BIR server

A local stand-in for the BIR's SFTP endpoint, so a whole submission
(Queue, then Submitted) can be tried without sending anything to the BIR.
The app talks to it with its real transport and its real queue worker. The
only thing that changes is where the transport sends the file.

**Never expose the mock.** It listens on 127.0.0.1 (or `::1`) and refuses any
other address. Do not forward its port, tunnel it, or run it on a shared
machine. Its username and password are deliberate fakes.

## Start it

```sh
just mock-bir                                  # port 2222, folder target/mock-bir
just mock-bir --port 2300 --dir ~/MockBIR      # another port or folder
just mock-bir --fail fail-after-store          # a failure mode (below)
just mock-bir --delay 12 --delay-at close      # be slow
```

`just mock-bir` builds and runs the `mock_bir` binary in `bir-core` (behind
the non-default `mock-bir` feature, so none of it is linked into the app).
At start it prints the folder paths, the host key fingerprint, and the
environment values the app needs. It also writes those values to
`target/mock-bir/app.env`, which `just mock-bir-app` reads. Ctrl-C stops it
and removes `app.env`.

It prints one line per event: connection, login (accepted or refused),
SFTP start, open, file received (name, size, SHA-256, where it was saved),
confirmation written, injected failures, errors, connection closed.

What it writes, under the folder (`target/mock-bir` by default):

| Path | Contents |
| --- | --- |
| `uploads/{form folder}/{IAF file name}` | each upload, byte for byte as received |
| `receipts/{BIR file name}.eml` | a confirmation message shaped like BIR's, one per stored upload |
| `host_key.seed` | the mock's SSH host key, kept so the fingerprint stays the same |
| `app.env` | the `BIR_SFTP_*` values, while the mock runs |
| `mock-bir.control` | optional, written by you (below) |

`open target/mock-bir` shows it in Finder. Delete the folder to start over
(a new host key and fingerprint are made).

## Failure modes

Pick one at start (`--fail`, `--delay`, `--delay-at`) or change it while the
mock and the app keep running: write `target/mock-bir/mock-bir.control`. The
file is read at every new connection and overrides the start-up flags; a
broken file is reported and ignored. Delete it to go back to the flags.

```
# target/mock-bir/mock-bir.control
fail=fail-after-store     # none | refuse-login | fail-before-store | fail-after-store
delay=12                  # seconds, 0 for none
delay_at=close            # login | open | close
```

What each one does, and what the app's rules leave the return as (the
end-to-end tests in `crates/bir-core/src/mock_bir_e2e.rs` check each row):

| Mode | The mock | The return afterwards |
| --- | --- | --- |
| none | logs in, stores the file, answers OK | **Submitted**. The file is in `uploads/`, its confirmation in `receipts/`. |
| `refuse-login` | rejects the password | **Queued, not claimed**: "session failed before upload (no PUT attempted)". It retries after 1, 2, 4 and 8 minutes; after the fifth failure it goes back to Draft. Nothing stored. |
| `fail-before-store` | logs in, refuses the `OPEN`; no byte is stored | **Queued and claimed, "Submission outcome pending"**. The app cannot tell that nothing was stored, so it never retries by itself. |
| `fail-after-store` | stores the whole file and writes its confirmation, then answers the `CLOSE` with a failure | **Queued and claimed, "Submission outcome pending"**: the ambiguous case. The file *is* in `uploads/`. |
| `--delay N --delay-at login` | waits N s before answering the password | **Submitted** once it answers. The client has no login timeout, so a very long delay keeps the job waiting (the return stays Queued, not claimed). |
| `--delay N --delay-at open` | waits N s before answering the `OPEN` | under 10 s: **Submitted**. 10 s or more: the client's SFTP request timeout fires after the claim, so **claimed, outcome pending**, nothing stored. |
| `--delay N --delay-at close` | stores the file, waits N s, then answers the `CLOSE` | under 10 s: **Submitted**. 10 s or more: **claimed, outcome pending**, and the file *is* stored. |

The 10 seconds is the SFTP client library's default per-request timeout
(`russh-sftp`), not a value the app sets.

A claimed return is meant for support-assisted reconciliation. On a test
copy of the database, throw the copy away and make a new one.

## Point the app at it

### Make a copy of the database

Quit the app first (tray menu, Quit), so the database is not being written.
Then copy the database files to a folder outside the app's data folder:

```sh
mkdir -p ~/BuwizMock
cp "$HOME/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db"* ~/BuwizMock/
```

The `*` copies `bir_data.db` and, if present, its `-wal` and `-shm` files.
The copy opens with the same keychain key as the original; no keychain
change is needed.

Before the first run, know what the copy holds:

- **Queued returns in the copy are sent to the mock the moment the app
  starts.** That is the point, but only if the mock values are in place
  before launch. `just mock-bir-app` makes sure of that.
- Confirmation-email tracking and Google Calendar in the copy use your real
  accounts. The inbox poll only reads, but it is your real mailbox. Turn
  tracking off in the copy if you do not want that (it cannot confirm a mock
  filing anyway, see below).
- Do not set `EBIR_TEST_ENV`: it forces a dry run (nothing reaches the mock)
  and swaps the database key, so the copy will not open.

### Installed app (recommended)

With the mock running in one terminal:

```sh
just mock-bir-app ~/BuwizMock/bir_data.db
# another install location:
just mock-bir-app ~/BuwizMock/bir_data.db /Applications/eBIRForms.app
```

The recipe refuses a database inside the app's data folder, refuses a mock
that is not listening, refuses a non-loopback host, clears any other
`BIR_SFTP_*` or dry-run value (including ones from `.env`), then starts the
bundle's executable with `BIR_DATABASE_PATH` and the values from
`target/mock-bir/app.env`. It is the same as:

```sh
BIR_DATABASE_PATH=~/BuwizMock/bir_data.db \
BIR_SFTP_HOST=127.0.0.1 BIR_SFTP_PORT=2222 \
BIR_SFTP_USERNAME=mock-filer BIR_SFTP_PASSWORD=mock-password-not-a-bir-secret \
BIR_SFTP_HOST_KEY_SHA256=<fingerprint the mock printed> \
/Applications/eBIRForms.app/Contents/MacOS/bir
```

Then queue a return as usual. The mock prints the connection, the login and
the file; the app shows the return as Submitted. Quit the app to finish.
Going back to normal needs nothing: open the app from Finder or the Dock as
usual and it uses the real database and the real BIR path again.

### Dev run

```sh
set -a; . target/mock-bir/app.env; set +a
BIR_DATABASE_PATH=~/BuwizMock/bir_data.db just shell-run
```

A debug build normally simulates every submission. A set `BIR_SFTP_HOST`
makes it talk to that host for real, and the Dev Mode panel's submission
switch steps aside while it is set. Open a new terminal (or `unset` the
`BIR_SFTP_*` values) to go back.

### Installed app opened from Finder (`LSEnvironment`)

Possible, but riskier: if Launch Services does not apply the values, a
release build with no `BIR_SFTP_HOST` sends queued returns to the **real
BIR**, and the worker starts at launch, before you can check. Prefer
`just mock-bir-app`. If you still want it, with the app quit:

```sh
PLIST=/Applications/eBIRForms.app/Contents/Info.plist
/usr/libexec/PlistBuddy -c "Add :LSEnvironment dict" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_DATABASE_PATH string $HOME/BuwizMock/bir_data.db" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_SFTP_HOST string 127.0.0.1" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_SFTP_PORT string 2222" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_SFTP_USERNAME string mock-filer" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_SFTP_PASSWORD string mock-password-not-a-bir-secret" "$PLIST"
/usr/libexec/PlistBuddy -c "Add :LSEnvironment:BIR_SFTP_HOST_KEY_SHA256 string <fingerprint>" "$PLIST"
# Editing Info.plist breaks the signature: sign again, as `just _package-mac` does.
codesign --force --options runtime --entitlements entitlements.dev.plist --sign - /Applications/eBIRForms.app
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f /Applications/eBIRForms.app
```

The new signature can make macOS ask once whether the app may use its
keychain item; allow it. After launching, check the values reached the
process before queueing anything:

```sh
ps -Eww -p "$(pgrep -x bir)" | tr ' ' '\n' | grep -E '^(BIR_SFTP_HOST|BIR_DATABASE_PATH)='
```

To go back: quit the app, `/usr/libexec/PlistBuddy -c "Delete :LSEnvironment" "$PLIST"`,
sign and `lsregister -f` again as above (or reinstall the app).

## The confirmation step

In a release build, only the confirmation-inbox poll moves a return from
Submitted to Confirmed:

1. After a real (not dry-run) submission, the worker schedules a
   `bir_poll_email <inbox>` job, if the taxpayer's email tracking is on.
2. The job connects with IMAP over TLS on port 993 to the profile's IMAP
   host (app password) or Gmail (Google sign-in), selects `INBOX` and
   searches for messages from BIR's sender address in the last 30 days (the
   address is a literal in `crates/bir-core/src/email/fetcher.rs`).
3. Each message's text goes through `receipt::parse_bir_receipt_email`,
   which needs the `File name: ….xml`, `Date received by BIR: …` and
   `Time received by BIR: …` lines.
4. The receipt is saved (one per file name) and
   `Database::confirm_submission_from_receipt` confirms the return when the
   TIN, form and period in the file name (the IAF name without `#email#`)
   match a return that is **Submitted**, and the BIR time, read as Manila
   time, is no earlier than the queue authorization (2 minutes' grace) or,
   without one, the submission (15 minutes' grace).

The mock does not run an IMAP server, because the real fetcher cannot be
pointed at one: the port (993), TLS with the system's certificate trust,
and the sender filter are fixed in `email/fetcher.rs`. Reaching a local
server would need a change to that path or a locally made certificate
trusted in your login keychain, and this setup does neither.

Instead the mock writes `receipts/{BIR file name}.eml` for every stored
upload: the lines BIR's message carries, stamped with the Manila time the
mock stored the file, from a placeholder sender. The end-to-end tests feed
it through the same parse, save and confirm steps the fetcher runs, so the
receipt matching rules are exercised for real. What that leaves out: the
IMAP connection and login, the sender search, HTML-only messages, and the
delay before BIR's email arrives.

In the installed app, a return sent to the mock **stays Submitted**. A real
BIR email for the same period cannot confirm it either: it was received
before this queue authorization, so the timing rule ignores it.

## What this does not prove about the real BIR

- **The dispatcher.** With `BIR_SFTP_HOST` set the app skips the dispatcher
  request, its HTTPS-then-HTTP fallback and the unwrapping of the connection
  fields. A release build with no flags goes through all of that; the mock
  does not.
- **Host-key handling.** The mock run pins the mock's key. The official path
  accepts any host key.
- **The real server.** Its SSH implementation and algorithms, which SFTP
  extensions it offers (the mock offers none, so the client sends no
  `fsync`), whether the form folders exist and are writable, packet and
  size limits, how it treats an existing file, server-side checks that
  reject a file, rate limits and concurrent sessions.
- **The network.** DNS, firewalls, proxies, captive portals and latency on
  the way to the BIR.
- **The email.** Its real wording and format, its sender, and how long it
  takes to arrive (minutes to hours, sometimes never).
- **The official client.** The mock follows this app's transport, not
  the official eBIRForms client.
