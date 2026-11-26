# Windows app icon — before/after proof

BEFORE: `ee3e649d` (main).
AFTER: `f9c604e3` (PR #369 head) = main `a1a86ded` (same tree `8edc678b`).
Both with `../nsh` at `8c78cfd6` (the commits' NSH_PINNED_REF), `nsh 0.6.1 (8c78cfd)`.

Machine: Microsoft Windows Server 2022 Standard 10.0.20348, 1920x1080, `EnableLUA=0` (UAC disabled).
Theme at the start: shell dark (`SystemUsesLightTheme=0`), apps light (`AppsUseLightTheme=1`) — restored after the pass.

## Steps done / skipped (wrap-up scope)

Done: Step 0 facts; Step 1 both builds + AFTER MSIX analysis; Step 2 all
pictures a–g for BOTH sides (taskbar dark+light with 4x crops, Start all-apps
and pinned in dark and light, title bar with crop, Alt-Tab, Settings > Apps,
Explorer large icons); Step 3 dev runs for both commits with the demo taxpayer
(menu row, tray dark+light, 4x crops); Step 4 WACK; Step 5 cleanup. The screen
video covers the shell pass and the dev-run window but is weak on the
tray-overflow and Alt-Tab moments — the numbered PNGs are the proof for those.
Skipped: nothing was skipped for lack of time except polish on the video (the
wrap-up order arrived after every step had already completed).

## How the pictures were taken

Every picture is a full-screen PNG captured with real input only (mouse/keyboard
through pyautogui/SendInput). Crops marked `-4x` are nearest-neighbour
enlargements of the matching full-screen PNG.

**Important host limitation:** on this machine UAC is disabled (`EnableLUA=0`), and
packaged MSIX activation fails silently — Start-menu launch of the installed
package produces no window, no error and no process (the build's own smoke test
says the same: "Packaged MSIX activation not verifiable on this host: UAC is
disabled (EnableLUA=0); registration and layout launch were verified"). So:

- Everything the shell itself draws — Start (all apps, pinned tile), Settings >
  Apps, the Explorer icons — is the **real installed package** (a test-signed
  copy of the MSIX, certificate in `LocalMachine\TrustedPeople`, removed after
  the pass).
- Everything the running app draws — taskbar button, title bar, Alt-Tab — is the
  **same `eBIRForms.exe` byte-for-byte, launched from the `makeappx unpack`
  layout** with a scratch `LOCALAPPDATA`, left at the sign-in screen. The
  packaged exe is what the MSIX installs; only the launch mechanism differs.

The tile background in the AFTER manifest is `#FFFFFF` and Start renders the
plated assets.

## The pairs

For each file `before-NN-*.png` there is an `after-NN-*.png` of the same scene.

- **01 / 06 taskbar dark/light (+ 4x crops).** The running app's taskbar button.
  Before: the old blue circle "e". After: the Buwiz Forms logo. Does not show:
  the MSIX activation path (broken on this host — see above).
- **02 / 03 Start, dark.** 02: the app in the all-apps list (and "Recently
  added"). 03: the tile pinned to Start. Before: blue "e" glyph and tile.
  After: the full Buwiz Forms logo with its name on the white plated tile.
  Does not show: launching the app (MSIX activation, see above).
- **07 Start, light.** Same list in light mode.
- **04 title bar (+ 4x crop).** Top left of the running window. Before: small
  blue "e". After: the logo mark. The crop is a 2x/4x enlargement of the same
  full-screen pixels.
- **05 Alt-Tab.** The app in the Alt-Tab grid with its icon.
- **08 Settings > Apps.** The app's row in "Apps & features" (Server 2022's
  name for "Installed apps"): icon, name, publisher "GoldCoders Corp".
- **09 (after only) Explorer large icons.** `eBIRForms.exe` in the unpacked
  layout in Large icons view; the exe carries the new logo `.ico` (the kit made
  it from `nsh.json` `icon.windows = ../assets/icon-window.png`).
- **10 menu row (step 3, dev run).** The in-app menu row at the top left of the
  window in a `just nsh-run-agent` dev run, demo taxpayer Juana Santos on Home.
  Before: the old blue "e". After: `nsh/shell/app-mark.png`.
- **11 / 12 tray (step 3, dev run), light and dark.** The notification-area
  overflow with the app's tray icon. The overflow also shows the other side's
  still-running instance where both were up at once; the pairing is by color:
  blue "e" = before, logo = after.

## Files

- `before-01..08`, `before-10..12` (+`-4x` crops), `after-01..09` (+`-4x` crops)
  as described above.
- `after-AppxManifest.packaged.xml` — the AppxManifest.xml inside the AFTER
  MSIX. Its `Properties/Logo` is `assets\icon-store.png`; `VisualElements` uses
  `assets\icon-tile.png` (Square150x150), `assets\icon-app.png` (Square44x44)
  and `assets\icon-wide.png` (Wide310x150). `resources.pri` is inside the
  package, and `assets\icon-*` carries 63 files: plated scale variants of
  tile/wide/store/app, target sizes 16–256 for the app icon, and unplated +
  lightunplated transparent variants of every target size.
- `windows-icon-pass.mp4` — one screen recording (2:22, uncompressed-length
  real time, no speedup needed) of the AFTER pass: the running packaged-layout
  exe at the sign-in screen in dark and in light, the Start list and pinned
  tile in light and in dark, Settings > Apps, the Explorer large-icons view of
  the unpacked layout, and the dev-run window on Home (menu row) in light and
  in dark. What it does not show well: the tray-overflow steps — the pass
  clicks the overflow chevron, but the flyout is small and the capture is
  compressed; the tray icons are proven by the sharper PNG pairs
  `*-11-tray-light*` / `*-12-tray-dark*` instead. Alt-Tab appears only briefly;
  `*-05-alttab.png` is the proof for it.
- `wack-report.xml` — Windows App Certification Kit report on a self-signed
  copy of the AFTER MSIX (the artifact itself was never signed or modified).

Nothing here is a signed or unsigned binary: no `.msix`, `.msi` or installer
is committed.
