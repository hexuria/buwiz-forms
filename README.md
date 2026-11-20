# E-BIRForms

[![CI](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/ci.yml)
[![Release](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release.yml)
[![Release (macOS)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml/badge.svg)](https://github.com/hexuria/buwiz-forms/actions/workflows/release-macos.yml)

E-BIRForms is an offline-first desktop app to prepare, validate, print and
submit Philippine BIR returns. It is a native Rust app built with the
[GPUI](https://gpui.rs/) framework. It covers 43 eBIRForms 7.9.6.2.1 forms. A
submission queue sends returns to BIR in the background, and a dry-run mode
lets you test the whole flow without sending anything. Profiles, TINs and
drafts stay in an encrypted SQLite database on your own machine.

Settings → Account can sign in to [buwiz-server](https://github.com/hexuria/buwiz-server)
with the device flow (the password stays in the browser). Release builds use
`https://api.buwiz.com` and open account pages on `https://account.buwiz.com`.
Debug builds use `http://localhost:3008`. A build can bake in
`BUWIZ_SERVER_URL` and `BUWIZ_ACCOUNT_URL`. A running process can still
override those with the same variable names. The app does not ask for the
address. It still prepares and files returns when that server is
unreachable. Details: [docs/cloud-account.md](docs/cloud-account.md).

## Platforms

| Platform | Status | Setup script | Database | Details |
| --- | --- | --- | --- | --- |
| macOS | Fully tested | `scripts/setup-macos.sh` | `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db` | [macos.md](docs/platforms/macos.md) |
| Linux | Fully tested | `scripts/setup-linux.sh` | `~/.taxman-ebir/bir_data.db` | [linux.md](docs/platforms/linux.md) |
| Windows | Partly verified, on one Windows Server 2022 VM | `scripts\setup-windows.ps1` | `%LOCALAPPDATA%\Taxman\eBIRForms\bir_data.db` | [windows.md](docs/platforms/windows.md) |

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

Packaging recipes (`just _package-mac`, `just _package-linux`, `just exe`,
`just msix`) are described on the platform pages.

## Headless command line (`bir`)

The headless agent binary is `bir-headless` in the standalone shell workspace.
From a checkout:

```bash
cargo install --path crates/bir-shell --features agent --bin bir-headless --locked
```

The headless release workflow archives the same executable as `bir`.

`cargo install` makes a release build, and a release build starts automation
only with `GPUI_AGENT_ALLOW_RELEASE=1`:

```bash
GPUI_AGENT=1 GPUI_AGENT_ALLOW_RELEASE=1 GPUI_AGENT_TOKEN=<token> bir-headless serve --detach
bir-headless status
bir-headless shutdown
```

Environment and the full command list are in
[crates/bir-shell/docs/AGENT.md](crates/bir-shell/docs/AGENT.md).

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

**Dry run and live sending.** Live sending to BIR is opt-in: set
`BIR_SFTP_LIVE=1`. Set `BIR_SFTP_DRY_RUN=1` to simulate a submission; nothing
is sent. In debug builds the Dev Mode panel defaults to dry run.

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
