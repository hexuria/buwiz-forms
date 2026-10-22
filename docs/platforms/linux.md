# eBIRForms on Linux

Status: fully tested. Back to the [README](../../README.md). The same matrix
for all platforms is in [FEATURES.md](../FEATURES.md).

## Feature matrix

Legend: ✓ works and is covered by the platform's testing. ✗ does not exist or cannot work
on that platform. partial: works with limits that are named in the note. untested: the code is
there, but nobody has run it on that platform.

| Feature | Status | Notes |
|---|---|---|
| Profiles, tax form library, editors, Validate, Save draft | ✓ | All 43 forms. |
| Submission queue and dry run (SFTP upload, IMAP receipts) | ✓ |  |
| Encrypted database (SQLCipher) | ✓ |  |
| Database key storage | partial | Needs PR #85. Until it is merged the key is not kept between launches. After it: Secret Service, or a key file when there is none. |
| Close button (X) and quit shortcut | ✓ | X, Ctrl+Q, Alt+F4 and Ctrl+W quit the app. |
| System tray icon | partial | Needs a StatusNotifier host. Many desktops have none; the app then runs without a tray. |
| Global show/hide hotkey | partial | Super+Shift+E suggestion. Hiding uses `xdotool`, which is X11 only. Wayland: unverified. |
| Single-instance guard | ✓ | Loopback ping only (`instance.port`). The platform hook is an empty stub. |
| Window size and position memory | ✓ | Restoring the position under Wayland: unverified. |
| Print preview | partial | Opens the filled HTML in the default browser. Print from the browser. |
| Print an email receipt as PDF | ✓ | `lp` (CUPS); opens the file if `lp` is missing. |
| Export dialogs (save file, choose folder) | partial | Needs `xdg-desktop-portal` or `zenity`. Otherwise files go to `~/Documents/eBIRForms/exports`. |
| Unlock with the OS (Touch ID, Windows Hello) | ✗ | Not built on Linux. Use the PIN or authenticator app. |
| PIN or authenticator app lock | ✓ |  |
| Desktop notifications | untested | In-app toast. System banner: unverified. |
| Instant refresh when the database changes | ✓ | 1 s poll. |
| Add deadlines to a calendar (.ics) | ✓ | Opens with `xdg-open`. |
| Google Calendar credentials storage | untested | Through `keyring`. No backend in this branch (same cause as the database key). |
| Keyboard shortcuts | ✓ | Ctrl based; also F11. |
| Packaging | ✓ | `.deb`, `.rpm`, x86_64 Arch package, tarball, installer, and x86_64 AUR build file. |
| Agent control (gpui-agent) | ✓ | Opt-in build feature. |
| Agent screenshots | ✗ | macOS only. |
| Dev Mode panel (debug builds only) | ✓ |  |

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
curl -fsSL https://github.com/hexuria/buwiz-forms/releases/latest/download/install-linux.sh | sh -s -- --version v0.1.0
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

The `.deb` and `.rpm` install `/usr/bin/ebirforms`, the shell's `app/` and
`print-overrides/` under `/usr/share/ebirforms/`, a desktop entry, and five
hicolor icon sizes. The package metadata declares runtime libraries; RPM
dependencies are detected from the binary's linked shared libraries.

### Tarball

Use `--tarball` with the installer or download the `.tar.gz` release asset. To
install manually, extract it and run `./install-desktop-entry.sh`; launch with
`./bir`. The tarball contains `bir`, `app/`, `print-overrides/`, `assets/`,
and the desktop-entry installer at its top level; the shell resolves `app/`
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
`--check`, `--extras`, `--no-agent`. Then `just run`.

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

## Data

- Database: `~/.taxman-ebir/bir_data.db`. Override with `BIR_DATABASE_PATH`.
- Window size and position: `~/.taxman-ebir/window-geometry`.
- Key file (when used): `~/.taxman-ebir/bir_key.txt`, mode 0600.
- `instance.port`: in the local data folder chosen by the `directories` crate
  (exact path unverified).

## Database key

- In the current branch (`forms/integration`) the Linux build has no keyring
  backend. The `keyring` crate then uses an in-memory mock: saving the key
  "succeeds" but it is lost at exit, so each launch makes a new key.
- Branch `fix/linux-keyring` (commit `907350af`, PR #85) fixes that. It turns
  on the Secret Service backend, uses an existing `bir_key.txt` first, and
  writes the key file when there is no Secret Service (for example on a
  headless machine). Other keyring errors still refuse to make a new key.
- Until that is merged, do not rely on an existing Linux database being
  readable after a restart.

## Close, quit and the tray

- The window's X button, Ctrl+Q, Alt+F4 and Ctrl+W quit the app. They go through the
  same guarded quit path as the tray's Quit. The window stays open only when
  unsaved Profile compliance changes block quitting.
- Why: many Linux desktops (GNOME, fluxbox) have no StatusNotifier tray host,
  and there is no dependable way to hide a window. `xdotool` is optional and
  X11-only.
- The tray icon needs a StatusNotifier host and GTK. The app calls `gtk::init`
  at startup and, if the tray cannot be created, logs a warning and carries on
  without it. If you have no tray, the app has no "run in background" mode.

## Single instance

- `platform::enforce_single_instance` is an empty stub on Linux. The guard is
  the loopback ping in `crates/bir-desktop/src/ipc.rs`. The first copy writes
  its UDP port to `instance.port`. A second copy sends `SHOW`, and if it gets an
  answer, it exits. A stale file is harmless: the ping times out after 100 ms.
- Raising the first window uses `xdotool` and GPUI's activate. Whether this
  works under Wayland is unverified.

## Global hotkey

- Registered only after you record a hotkey in Settings. The suggested default
  for a bare key is Super+Shift plus the key. Toggling visibility uses
  `xdotool`. Behavior under Wayland, and when no `xdotool` is installed, is
  unverified.

## Print and export

- Print preview writes the filled HTML to a temporary folder and opens it in
  the default browser (`xdg-open`). The embedded webview cannot be used because
  `wry`'s WebKitGTK backend accepts only an Xlib window handle, and GPUI gives
  XCB or Wayland handles. Print from the browser.
- Receipt PDFs print with `lp` (CUPS). If `lp` is missing, the file is opened.
- Export dialogs use `rfd`. A dialog exists when the
  `org.freedesktop.portal.Desktop` name is on the session bus or `zenity` is on
  `PATH`. Otherwise the export goes to `~/Documents/eBIRForms/exports/<name>`
  (or `<data folder>/exports/<name>` if `~/Documents` does not exist), and the
  app shows "Saved to <path>" with an Open folder button. If you cancel a real
  dialog, nothing is written.

## Security

- Touch ID and Windows Hello code is not built on Linux. Use the PIN or an
  authenticator app.

## Notifications

- In-app toasts work. Whether the system banner appears is unverified.
  `bir-headless` uses `notify-rust`.

## Keyboard shortcuts

Ctrl+Enter, Ctrl+B, Ctrl+Shift+B, Ctrl+F, Ctrl+N, Ctrl+Shift+T (theme),
Ctrl+Shift+X (cron tasks), Ctrl+, (Settings), Ctrl+K (command palette), F1
(global dashboard), Ctrl+Q, Alt+F4, Ctrl+W, F11, Ctrl+=, Ctrl+- and Ctrl+0
(zoom).

## Unverified

- Wayland behavior of the hotkey, window raise and window position memory.
- The exact `instance.port` path.
- Whether system notification banners appear.
