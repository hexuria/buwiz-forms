# eBIRForms on Linux

Status: fully tested. Back to the [README](../../README.md). The same matrix
for all platforms is in [FEATURES.md](../FEATURES.md).

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Check, Save draft | ✓ | All 43 forms. |
| Submission queue (in shipped builds Queue is refused — the return stays a draft; a return queued by an older version stays Queued) | ✓ |  |
| Database storage (plain SQLite, not encrypted) | ✓ |  |
| Close button (X) and quit shortcut | ✓ | X, Alt+F4 and Ctrl+W (Close Window) ask "Hide to tray or Quit?" by default; Ctrl+Q quits for real (`closePolicy` in `nsh/shell/desktop/menu_model.ts`). |
| System tray icon | partial | Needs a StatusNotifier host. Many desktops have none; the app then runs without a tray. |
| Global show/hide hotkey | partial | Super+Shift+E suggestion. Hiding uses `xdotool`, which is X11 only. Wayland: unverified. |
| Single-instance guard | ✓ | Loopback ping only (`instance.port`). The platform hook is an empty stub. |
| Window size and position memory | ✓ | Restoring the position under Wayland: unverified. |
| Print preview | partial | In-app window: one picture per sheet drawn on the CPU by Blitz + `vello_cpu` (`nsh/src/print.rs`, `nsh/src/sheets.rs`). The PDF/print path uses WebKitGTK's print operation (GTK's "Print to File"). |
| Export profile (Settings → Transfer) and Save as PDF (print preview) | partial | Needs `xdg-desktop-portal` or `zenity`. Otherwise files go to `~/Documents/eBIRForms/exports`. |
| PIN or authenticator app lock | ✓ |  |
| In-app notifications drawer and toasts | ✓ | Notifications panel (Ctrl+Shift+N) and in-app toasts. |
| Background refresh when the database changes | ✓ | The paced watch in `nsh/shell/app.ts` re-reads. |
| Gmail connect for BIR receipt tracking | ✓ | Settings → Account → Google, or the taxpayer inbox card, in a build given a Google client (none in 1.0.0: an app password instead); the host opens the browser. |
| Keyboard shortcuts | ✓ | Ctrl based; also F11. |
| Packaging | ✓ | `.deb`, `.rpm`, x86_64 Arch package, tarball, installer, and x86_64 AUR build file. |
| Agent control (gpui-agent) | ✓ | Opt-in build feature. |
| Agent screenshots | ✗ | macOS only. |

## Install and run

### One-line installer

The installer detects Debian/Ubuntu, Fedora/RHEL, openSUSE, and Arch-based
systems, verifies the downloaded package against the release checksums, and
installs it with the distro's package manager. Unknown distributions and
`--tarball` use a per-user install under `~/.local/share/ebirforms`.

```bash
curl -fsSL https://github.com/hexuria/buwiz-forms/releases/latest/download/install-linux.sh | sh
```

To install a specific release:

```bash
curl -fsSL https://github.com/hexuria/buwiz-forms/releases/latest/download/install-linux.sh | sh -s -- --version v0.2.0
```

### Manual package install

Download the matching package from the
[Releases](https://github.com/hexuria/buwiz-forms/releases) page, then use your
distro's package manager:

```bash
# Debian 12+ and Ubuntu 22.04+
sudo apt install ./eBIRForms-Linux-x64-<version>.deb

# Fedora 36+
sudo dnf install ./eBIRForms-Linux-x64-<version>.rpm

# openSUSE Tumbleweed (Leap 15 is unsupported)
sudo zypper --no-gpg-checks install --allow-unsigned-rpm ./eBIRForms-Linux-x64-<version>.rpm

# Arch Linux, x86_64 only
sudo pacman -U ./eBIRForms-Linux-x64-<version>.pkg.tar.zst
```

The `.deb` and `.rpm` install `/usr/bin/ebirforms`, the app's `nsh/` bundle and
`print-overrides/` under `/usr/share/ebirforms/`, a desktop entry, and five
hicolor icon sizes. The package metadata declares runtime libraries; RPM
dependencies are detected from the binary's linked shared libraries.

### Tarball

Use `--tarball` with the installer or download the `.tar.gz` release asset. To
install manually, extract it and run `./install-desktop-entry.sh`; launch with
`./bir`. The tarball contains `bir`, `nsh/`, `print-overrides/`, `assets/`,
and the desktop-entry installer at its top level; the app resolves `nsh/`
and `print-overrides/` beside the binary.

### Runtime requirements

Release binaries target glibc 2.35 or newer: Ubuntu 22.04+, Debian 12+,
Fedora 36+, current Arch Linux, and openSUSE Tumbleweed. openSUSE Leap 15 is
unsupported. Install Noto Sans (`fonts-noto-core` on Debian/Ubuntu,
`google-noto-sans-fonts` on Fedora, `noto-fonts` on Arch) to avoid a blank
window when the system has no UI font.

## Build from source

Run `scripts/setup-linux.sh`. It detects apt, dnf, pacman or zypper, installs
the libraries, Rust and `just`, and checks the result. Other flags:
`--check`, `--extras`, `--no-agent`, `--compositor`. Then `just run`.

For the NSH app and its Android build, run `scripts/setup-linux-nsh.sh` after
that (needs Docker; `--no-android`, `--check`). It builds NSH in an Ubuntu
24.04 image and writes `~/BuwizNsh/nsh2404.sh` to run NSH commands with the
host display. Android setup requires an x86_64 host; use `--no-android` on ARM.

On other distributions, install the equivalent of these Debian and Ubuntu
packages:

```bash
sudo apt-get install -y \
  build-essential pkg-config perl python3 \
  libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libwayland-client0 libasound2-dev libudev-dev libvulkan-dev \
  libfontconfig1-dev libfreetype-dev libssl-dev libpolkit-gobject-1-dev \
  libgtk-3-dev libwebkit2gtk-4.1-dev libxdo-dev \
  mesa-vulkan-drivers libappindicator3-dev librsvg2-dev patchelf xvfb \
  fonts-noto-core fonts-noto-color-emoji
```

- OpenSSL compiles from vendored source, so a C toolchain and `perl` are
  needed even when `libssl-dev` is installed.
- Headless machines can run the GTK print tests under a virtual framebuffer:
  `xvfb-run -a just test`.
- Blank window on a VM or software GPU: llvmpipe's Vulkan path can present
  stale frames. Force the GL backend with `VK_ICD_FILENAMES=/dev/null just run`.
- Without a compositing manager (KWin or Xfwm with compositing off, Openbox,
  most VMs), the app uses the window manager's title bar and frame instead of
  its own; a transparent shadow would otherwise draw as a thick black border.
  Resize handles depend on the window manager's frame theme. If the theme
  draws no resize borders, use the window manager's Resize command or pick a
  theme with visible borders.
  `scripts/setup-linux.sh --compositor` installs and autostarts `xcompmgr` to
  keep the app's own frame.

## Data

- Database: `~/.local/share/dev.goldcoders.bir/bir_data.db` (the
  `directories` crate with app id `dev.goldcoders.bir`). Override with
  `BIR_DATABASE_PATH`.
- The database is plain SQLite; nothing is encrypted and there is no key
  to store. The Buwiz account refresh token uses `keyring` (Secret
  Service) plus the fallback file `buwiz-cloud-account.json` beside the
  database (see [cloud-account.md](../cloud-account.md)).
- Window size and position: `window-geometry` in the data folder.
- `instance.port`: in the local data folder chosen by the `directories` crate
  (exact path unverified).

## Close, quit and the tray

- The window's X button, Alt+F4 and Ctrl+W (Close Window) ask "Hide to tray
  or Quit?" by default ("Don't ask again" remembers the choice; Settings >
  Keyboard changes it). Ctrl+Q quits for real. Any quit while returns are
  still sending warns first: Quit anyway / Wait / Hide to tray
  (`closePolicy` and the guarded quit path in `nsh/shell/desktop/`).
- Why: many Linux desktops (GNOME, fluxbox) have no StatusNotifier tray host,
  and there is no dependable way to hide a window. `xdotool` is optional and
  X11-only.
- The tray icon needs a StatusNotifier host and GTK. The app calls `gtk::init`
  at startup and, if the tray cannot be created, logs a warning and carries on
  without it. If you have no tray, the app has no "run in background" mode.

## Single instance

- `platform::enforce_single_instance` is an empty stub on Linux. The guard is
  the loopback ping in `crates/bir-core/src/ipc.rs`. The first copy writes
  its UDP port to `instance.port`. A second copy sends `SHOW`, and if it gets an
  answer, it exits. A stale file is harmless: the ping times out after 100 ms.
- Raising the first window uses `xdotool`. Whether this works under
  Wayland is unverified.

## Global hotkey

- Registered only after you record a hotkey in Settings. The suggested default
  for a bare key is Super+Shift plus the key. Toggling visibility uses
  `xdotool`. Behavior under Wayland, and when no `xdotool` is installed, is
  unverified.

## Print and export

- Print preview opens in the app's own window: each sheet is laid out by
  Blitz (stylo, taffy, parley) and painted by `vello_cpu` into a picture
  (`nsh/src/print.rs`, `nsh/src/sheets.rs`); no browser is involved.
- The PDF and the printout go through WebKitGTK's print operation (GTK's
  "Print to File" printer and the GTK print dialog, `nsh/src/print.rs`).
- Export dialogs use `rfd`. A dialog exists when the
  `org.freedesktop.portal.Desktop` name is on the session bus or `zenity` is on
  `PATH`. Otherwise the export goes to `~/Documents/eBIRForms/exports/<name>`
  (or `<data folder>/exports/<name>` if `~/Documents` does not exist), and the
  app shows "Saved to <path>" with an Open folder button. If you cancel a real
  dialog, nothing is written.

## Security

- The PIN and authenticator-app locks are in Settings → Account → Security.
- No screen offers calendar export (.ics), Google Calendar, unlock with
  Touch ID/Windows Hello, or printing a receipt as PDF; that code exists
  in the core crates and `nsh/src/auth.rs` but has no control in
  `nsh/screens` or `nsh/shell`.

## Notifications

- Notifications are in-app (the drawer and toasts). The headless `bir`
  uses `notify-rust`.

## Keyboard shortcuts

Ctrl+K (command palette), Ctrl+B (sidebar), Ctrl+Shift+B (tax profile
panel), Ctrl+Shift+N (notifications panel), Ctrl+T (theme), Ctrl+N (new
return), Ctrl+, (Settings), Ctrl+L (lock), Ctrl+[ and Ctrl+] (back and
forward), F11, Alt+F4, Ctrl+W.

## Unverified

- Wayland behavior of the hotkey, window raise and window position memory.
- The exact `instance.port` path.
