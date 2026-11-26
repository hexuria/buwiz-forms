<p align="center">
  <img src="assets/brand/buwiz-forms.svg" alt="Buwiz Forms logo" width="240">
</p>

# E-BIRForms

[![CI](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml)
[![Release](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml)
[![Release (macOS)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml)

E-BIRForms is an offline-first desktop app to prepare, validate, print and
submit Philippine BIR returns. It is a native app: a Rust back end and screens
built on NSH (Native Shell). It covers 43 eBIRForms 7.9.6.2.1 forms.
In this version Queue is refused — pressing it says the return cannot be
sent to the BIR yet and keeps it a draft; a return queued by an older
version stays queued (see "Sending is switched off" below). Profiles, TINs and
drafts stay in a SQLite database on your own machine. The app does not encrypt
that file; see [PRIVACY_POLICY.md](PRIVACY_POLICY.md).

A new install asks you to sign in to a Buwiz account before it can be
used (the account server is [buwiz-com](https://github.com/buwiz-com/buwiz-com),
which serves `account.buwiz.com` and `api.buwiz.com`). Sign-in is
passwordless: the app opens your browser to `account.buwiz.com`, you
sign in with a six-digit code mailed to your address, and approving the
device signs the app in. Release builds call `https://api.buwiz.com`.
Debug builds use `http://localhost:3008`. A build can bake in
`BUWIZ_SERVER_URL` and `BUWIZ_ACCOUNT_URL`. A running process can still
override those with the same variable names. The app does not ask for
the address. Once signed in, it still prepares and files returns when
that server is unreachable. Details: [docs/cloud-account.md](docs/cloud-account.md).

## Platforms

| Platform | Status | Setup script | Database | Details |
| --- | --- | --- | --- | --- |
| macOS | Fully tested | `scripts/setup-macos.sh` | `~/Library/Application Support/dev.goldcoders.bir/bir_data.db` | [macos.md](docs/platforms/macos.md) |
| Linux | Fully tested | `scripts/setup-linux.sh` | `~/.local/share/dev.goldcoders.bir/bir_data.db` | [linux.md](docs/platforms/linux.md) |
| Windows | Built and tested in CI on every pull request | `scripts\setup-windows.ps1` | `%LOCALAPPDATA%\dev.goldcoders.bir\bir_data.db` | [windows.md](docs/platforms/windows.md) |

Not every feature works the same on every platform. The full table is in
[docs/FEATURES.md](docs/FEATURES.md). Each platform page lists the packages,
runtime requirements, key storage, close and quit behaviour, and known limits.

## Install

Download from the [Releases](https://github.com/hexuria/buwiz-forms/releases)
page:

- **macOS:** open the `.dmg` and drag `eBIRForms.app` to Applications.
- **Linux:** `sudo apt install ./eBIRForms-Linux-x64-<version>.deb`. Other
  package formats are in [linux.md](docs/platforms/linux.md).
- **Windows:** run `eBIRForms-Windows-x64-<version>-Setup.exe`, or use the
  `.msi`.

## Build from source

Run the setup script for your platform from the table above (add `--check` on
macOS and Linux to verify an existing install), then:

```bash
just run                  # run the app in a development window (use a scratch database)
```

Packaging is described on the platform pages: `just _package-mac` (and
`just app` for the Mac App Store package) on macOS,
`packaging/linux/build-packages.sh` on Linux, and on Windows
`scripts/package_nsh_windows.ps1` (the release workflows build these on every tag).

## Headless command line (`bir`)

The headless agent binary is `bir`, built from `crates/bir-cli` in the root
workspace. From a checkout:

```bash
cargo install --path crates/bir-cli --locked
```

The headless release workflow (`bir-v*` tags) archives the same executable.

`cargo install` makes a release build, and a release build starts automation
only with `GPUI_AGENT_ALLOW_RELEASE=1`:

```bash
GPUI_AGENT=1 GPUI_AGENT_ALLOW_RELEASE=1 GPUI_AGENT_TOKEN=<token> bir serve --detach
bir status
bir shutdown
```

Environment and the full command list are in
[docs/agent.md](docs/agent.md).

This `bir` is a command line tool, not the desktop app. The desktop app comes
from the dmg, deb or exe on the Releases page. Publish flow and tags are in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#publishing-the-bir-cli).

## Using the app

**Set up a profile.** Create a taxpayer profile with the TIN and
registered details. You can keep many profiles in one workspace. A saved
return keeps a snapshot of the profile as it was at save time.

**Browse the tax form library.** The sidebar picks **Home**, **Form
library**, **Calendar**, **Filings**, **Taxpayers** and **Settings**.
The Calendar shows your deadlines. The library has a card for each
form. Pick a form and a period to open its editor.

**Fill a return.** Type into the form. Validation errors show on the form, and
totals are computed for you.

**Validate, print, queue.** The editor header has
`[Print preview] [Check] [Queue]`. Check validates the return. In this
version Queue refuses — it says the return cannot be sent to the BIR yet
and keeps it a draft. After an amendment
the header shows the amendment state instead.

**Statuses.** A return moves through Draft, Queued, Submitted, Confirmed and
Paid. In this version Queue refuses, so returns stay Draft; a return an
older version queued keeps its Queued status. When BIR receipt emails
arrive in the
inbox you configured, the status moves to Confirmed and then to Paid. A
submitted return is read-only.

**Sending goes through Buwiz.** When you queue a return, the app sends it to
the BIR through Buwiz's server: the return is sealed for that server on your
computer, the BIR's own servers are reached from your computer, and the app
keeps Buwiz's signed receipt. It needs your Buwiz sign-in and a TIN that is
approved on your Buwiz account; a return for any other TIN comes back to Draft
with the reason. A return queued by an earlier version is not sent: it goes
back to Draft once, to be checked and queued again. To simulate a submission,
start the app with `BIR_SFTP_DRY_RUN=1`; nothing is sent. Debug builds are dry
by default, and never send through Buwiz's production server. The code that
files with the BIR itself is compiled only with bir-core's `direct-bir` cargo
feature, which no build that ships enables.

**Revert to Draft as an amendment.** Queued, Submitted and Confirmed returns
can be reverted to Draft. Paid returns cannot. Reverting a Submitted or
Confirmed return starts an amendment and keeps a read-only copy of what BIR
received.

**Print preview.** Print preview fills the frozen printed sheet of the
exact BIR revision with your data. It never changes the return's
status. It opens in its own window — a web view on macOS, sheet images
drawn in-app by Blitz and `vello_cpu` on Windows and Linux
(`nsh/src/print.rs`). Export what you want to keep.

**Notifications drawer.** The bell in the top bar opens a drawer with alerts
and receipts. It closes when you change page.

**Theme.** Switch between light and dark with the theme button in the top bar,
or Cmd/Ctrl+T.

**Keyboard shortcuts** (Cmd on macOS, Ctrl elsewhere): K opens the
command palette, B toggles the sidebar, Shift+B the tax profile panel,
Shift+N the notifications panel, T switches theme, N starts a new
return, comma opens Settings, L locks the app, [ and ] go back and
forward.

## More documentation

- [docs/FORMS.md](docs/FORMS.md): the 43 forms, queue support, parity proof
  and printed sheets.
- [docs/FEATURES.md](docs/FEATURES.md): the feature matrix for all three platforms.
- Platform pages: [macOS](docs/platforms/macos.md),
  [Windows](docs/platforms/windows.md), [Linux](docs/platforms/linux.md).
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md): commands, gpui-agent, versioning,
  CI/CD, architecture, reference tooling and development notes.
- [Download the official forms](https://www.bir.gov.ph/bir-forms) from BIR.
