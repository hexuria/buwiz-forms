# E-BIRForms

[![CI](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml)
[![Release](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml)
[![Release (macOS)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml)

E-BIRForms is an offline-first desktop app to prepare, validate, print and
submit Philippine BIR returns. It is a native app: a Rust back end and screens
built on NSH (Native Shell). It covers 43 eBIRForms 7.9.6.2.1 forms. A
submission queue sends returns to BIR in the background, and a dry-run mode
lets you test the whole flow without sending anything. Profiles, TINs and
drafts stay in a SQLite database on your own machine. The app does not encrypt
that file; see [PRIVACY_POLICY.md](PRIVACY_POLICY.md).

A new install asks you to sign in to a Buwiz account
([buwiz-server](https://github.com/hexuria/buwiz-server)) before it can be used.
Sign-in uses the device flow, in the browser. Release builds use
`https://api.buwiz.com` and open account pages on `https://account.buwiz.com`.
Debug builds use `http://localhost:3008`. A build can bake in
`BUWIZ_SERVER_URL` and `BUWIZ_ACCOUNT_URL`. A running process can still
override those with the same variable names. The app does not ask for the
address. Once signed in, it still prepares and files returns when that server
is unreachable. Details: [docs/cloud-account.md](docs/cloud-account.md).

## Platforms

| Platform | Status | Setup script | Database | Details |
| --- | --- | --- | --- | --- |
| macOS | Fully tested | `scripts/setup-macos.sh` | `~/Library/Application Support/dev.goldcoders.bir/bir_data.db` | [macos.md](docs/platforms/macos.md) |
| Linux | Fully tested | `scripts/setup-linux.sh` | `~/.local/share/dev.goldcoders.bir/bir_data.db` | [linux.md](docs/platforms/linux.md) |
| Windows | Partly verified, on one Windows Server 2022 VM | `scripts\setup-windows.ps1` | `%LOCALAPPDATA%\dev.goldcoders.bir\bir_data.db` | [windows.md](docs/platforms/windows.md) |

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
just run                  # development build with diagnostics
```

Packaging is described on the platform pages: `just _package-mac` and `just app`
on macOS, `packaging/linux/build-packages.sh` on Linux, and on Windows
`nsh package --target windows` and then `scripts/package_nsh_windows.ps1`.

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

**Set up a profile.** Create a taxpayer profile (Cmd/Ctrl+N) with the TIN and
registered details. You can keep many profiles in one workspace. The profile
page has tabs for the tax profile, email settings (used to read BIR
receipts), the calendar, security (an optional PIN or authenticator lock) and
export. A saved return keeps a
snapshot of the profile as it was at save time.

**Browse the tax form library.** The dashboard has two tabs, Calendar and Tax
Form Library. The Calendar shows your deadlines. The library has a card for
each form. Pick a form and a period to open its editor.

**Fill a return.** Type into the form. Validation errors show on the form, and
totals are computed for you.

**Validate, Submit, more.** The editor header has three controls:
`[Validate] [Submit] [⋯]`. Validate checks the return. With the default
"Separate Validate button" setting, Submit waits until you have validated.
The ⋯ menu has Save draft, Print preview, Revert to Draft and, after an
amendment, View what was sent. Submit is hidden for the four 2200 excise forms
that cannot be queued.

**Statuses.** A return moves through Draft, Queued, Submitted, Confirmed and
Paid. Submit queues the return. The background engine uploads it over SFTP.
When BIR receipt emails arrive in the inbox you configured, the
status moves to Confirmed and then to Paid. A submitted return
is read-only.

**Sending is switched off.** This version cannot send a return to the BIR.
Submit says so and the return stays a draft; a return queued by an earlier
version stays queued and is not sent. Sending comes back in a later version.
To simulate a submission, start the app with `BIR_SFTP_DRY_RUN=1`; nothing is
sent. Debug builds are dry by default; their Dev Mode panel defaults to dry
run. The code that files with the BIR itself is compiled only with bir-core's
`direct-bir` cargo feature, which no build that ships enables.

**Revert to Draft as an amendment.** Queued, Submitted and Confirmed returns
can be reverted to Draft. Paid returns cannot. Reverting a Submitted or
Confirmed return starts an amendment and keeps a read-only copy of what BIR
received.

**Print preview.** Print preview fills the frozen printed sheet of the exact
BIR revision with your data. It never changes the return's status. On macOS
and Windows it opens in its own window with a Print button. On Linux it opens
in your default browser. Export what you want to keep.

**Notifications drawer.** The bell in the top bar opens a drawer with alerts
and receipts. It closes when you change page.

**Theme.** Switch between light and dark with the theme button in the top bar,
or Cmd/Ctrl+T.

**Dev Mode (debug builds only).** Debug builds have a Dev Mode panel with
switches for submission mode, test SFTP server, showing uncertified forms and a
separate dev database. A release build ignores every one of them.

**Keyboard shortcuts** (Cmd on macOS, Ctrl elsewhere): Enter submits the
current form, N creates a profile, B toggles the sidebar, F focuses search, K
opens the command palette, Shift+X opens cron tasks, comma opens Settings.

## More documentation

- [docs/FORMS.md](docs/FORMS.md): the 43 forms, queue support, parity proof
  and printed sheets.
- [docs/FEATURES.md](docs/FEATURES.md): the feature matrix for all three platforms.
- Platform pages: [macOS](docs/platforms/macos.md),
  [Windows](docs/platforms/windows.md), [Linux](docs/platforms/linux.md).
- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md): commands, gpui-agent, versioning,
  CI/CD, architecture, reference tooling and development notes.
- [Download the official forms](https://www.bir.gov.ph/bir-forms) from BIR.
