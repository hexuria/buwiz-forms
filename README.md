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

## Platform status

| Platform | Status | Details |
| --- | --- | --- |
| macOS | Fully tested | [docs/platforms/macos.md](docs/platforms/macos.md) |
| Linux | Fully tested | [docs/platforms/linux.md](docs/platforms/linux.md) |
| Windows | Not yet verified. Tracked in [#90](https://github.com/hexuria/buwiz-forms/issues/90) | [docs/platforms/windows.md](docs/platforms/windows.md) |

Not every feature works the same on every platform. The full table is in
[docs/FEATURES.md](docs/FEATURES.md).

## macOS

**Prerequisites (to build from source):** Xcode command line tools, Homebrew,
and a Rust toolchain with both Apple targets. One idempotent script installs
them all:

```bash
scripts/setup-macos.sh           # install everything
scripts/setup-macos.sh --check   # verify an existing install
```

OpenSSL compiles from vendored source, so the system perl and clang are
enough. The app uses the system WebView for print preview. You need no other
document renderer.

**Install:** download the `.dmg` from the
[Releases](https://github.com/hexuria/buwiz-forms/releases) page and drag
`eBIRForms.app` to Applications. The macOS release workflow signs and
notarizes the DMG, and stops if the Apple secrets are missing. Maintainers
provision those secrets once with `scripts/setup_macos_release_secrets.sh`.

**Build and run from source:**

```bash
just run                  # development build with diagnostics
just _package-mac         # universal .app and .dmg in target/release-artifacts
```

**Where data lives:** `~/Library/Group Containers/group.dev.goldcoders.bir/bir_data.db`.
The database key is in the macOS Keychain.

**Notes:**
- The window's close button and Cmd+Q hide the app to the menu bar. Use the
  tray menu's Quit to quit.
- The global show/hide hotkey is off in Mac App Store builds.

More in [docs/platforms/macos.md](docs/platforms/macos.md).

## Windows

Windows is not yet verified. Treat it as untested until
[#90](https://github.com/hexuria/buwiz-forms/issues/90) is closed.

**Prerequisites (to build from source):** Visual Studio 2022 Build Tools with
the C++ workload and Windows 11 SDK, Rust, OpenSSL, Strawberry Perl, NASM, Inno
Setup and Python.
One script installs them and sets the `OPENSSL_*` variables:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
```

The script asks for administrator rights through UAC. Strawberry Perl must come
before Git Bash's perl on `PATH`.

**Install:** the release workflow builds `eBIRForms-Windows-x64-<version>-Setup.exe`
and an `.msi`. Run the Setup program and start eBIRForms from the Start menu.

**Build and run from source:**

```powershell
just run                  # development build with diagnostics
just exe                  # Inno Setup installer
just msix                 # MSIX package
```

**Where data lives:** `%LOCALAPPDATA%\Taxman\eBIRForms\bir_data.db`. The
database key is meant to be kept in Windows Credential Manager (untested).

**Notes:**
- The close button and Alt+F4 hide the app to the tray. Use the tray menu's
  Quit to quit.
- Windows Hello unlock is built in but untested.

More in [docs/platforms/windows.md](docs/platforms/windows.md).

## Linux

**Install the `.deb`** (Debian, Ubuntu and relatives). Download it from the
[Releases](https://github.com/hexuria/buwiz-forms/releases) page and install:

```bash
sudo apt install ./eBIRForms-Linux-x64-<version>.deb
```

The package installs `/usr/bin/ebirforms`, the assets in
`/usr/share/ebirforms/assets`, a desktop entry
(`/usr/share/applications/dev.goldcoders.bir.desktop`) and hicolor icons
(48, 64, 128, 256 and 512 px, `/usr/share/icons/hicolor/<size>/apps/dev.goldcoders.bir.png`). After install,
eBIRForms appears in your application menu under Office. You can also run
`ebirforms` from a terminal. It needs GTK 3, WebKitGTK 4.1, Vulkan,
xkbcommon, Wayland client, fontconfig and freetype. `apt` pulls them in. Noto
Sans must be installed, or the window shows as a blank frame.

**Build from source:**

```bash
scripts/setup-linux.sh           # installs system libraries, Rust and just
scripts/setup-linux.sh --check   # verify an existing install
just run                         # development build with diagnostics
just _package-linux              # .deb (or a tarball without cargo-deb)
```

The script supports apt, dnf, pacman and zypper. The full package list is in
[docs/platforms/linux.md](docs/platforms/linux.md).

**Where data lives:** `~/.taxman-ebir/bir_data.db`.

**Notes:**
- The window's X button, Ctrl+Q and Alt+F4 quit the app.
- The tray icon needs a StatusNotifier host, which many desktops lack. The app
  starts without a tray when none is found.
- Print preview opens in your default browser.
- Exports need `xdg-desktop-portal` or `zenity`. Without them files go to
  `~/Documents/eBIRForms/exports`.
- Blank window on a VM or software GPU: start with `VK_ICD_FILENAMES=/dev/null`.

More in [docs/platforms/linux.md](docs/platforms/linux.md).

## Headless command line (`bir`)

Without the desktop app, the headless agent installs straight from this
repository. There is no crates.io release and no clone is needed:

```bash
cargo install --git https://github.com/hexuria/buwiz-forms bir --locked
```

That builds `main`. Add `--tag <tag>` or `--rev <commit>` to pin a version,
and run the same command again to update. The build needs the same system
libraries as the desktop app (see your platform's page under `docs/platforms/`).

`cargo install` makes a release build, and a release build starts automation
only with `GPUI_AGENT_ALLOW_RELEASE=1`:

```bash
GPUI_AGENT=1 GPUI_AGENT_ALLOW_RELEASE=1 GPUI_AGENT_TOKEN=<token> bir serve --detach
bir status      # same env; prints the hello line
bir shutdown
```

Environment and the full command list are in
[crates/bir-desktop/docs/AGENT.md](crates/bir-desktop/docs/AGENT.md).

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
or Cmd/Ctrl+Shift+T.

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
