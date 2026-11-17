# shell/desktop: the desktop host (D4)

What makes the app a desktop app: the application menu, the tray and hide-to-tray, the global show/hide hotkey, the updater and the one-copy
guard's wake-up. `App.tsx` mounts `<DesktopHost />` beside the frame; it draws only the About dialog. Off the desktops it does nothing.
Rust half: `src/desktop.rs` (the hooks' mailbox, the one-copy guard, the hotkey combo mapping, module `Desktop`).

| File | What |
|---|---|
| `menu_model.ts` | Pure: the menus as data for macOS/Windows/Linux, signed in or locked, with or without a taxpayer (`buildMenu`, `buildTrayMenu`, `flatten`, `printMenu`, `CLOSE_POLICY`). Port of crates/bir-shell/src/app_menu.rs. |
| `actions.ts` | What an item does through the frame's API (`app.go`, `toggleSidebar`, `togglePanel`, `cycleTheme`, `openPalette`, `startNewReturn`); `showMain`, `hideToTray`, `toggleMain`, `quitApp` (= `app.quit`: beforeLeave hooks, then the door's `shutdown()`, then `Application.quit()`), `closePressed`, `setSignOutHandler` (the sign-in screens take over Sign Out; the default is the old fallback: `cloud_account_sign_out`, `session_closed`, `app.setCloudLocked(true)`). |
| `Desktop.tsx` | `DesktopHost`: close guard (`closeOnPress: false` + `closePress`), the menu effect, `Tray.set`, the hotkey effect, a 250 ms tick of `Desktop.poll()`. Props `updater` and `shortcuts` swap nsh/updater and GlobalShortcut for a test. `installedMenu()`, `installedTray()` for fixtures. |
| `hotkey.ts` | Pure: follows the stored combo, replaces a chord the system has not answered yet, gives late answers back. |
| `hotkey_state.ts` | A leaf (imports nothing of the app): `hotkeyStatus()` (written by DesktopHost) and `holdHotkey()`, which Settings' shortcut recorder uses (F17): while a hold is on, DesktopHost's effect reads the combo as empty, so the chord goes back to the system until the hold ends. Screens reach both through `shell/index.ts`. |
| `updater.ts`, `updates.ts` | Pure state machine (idle, checking, downloading, ready, failed, up_to_date, available, installing) and its wiring to nsh/updater and to Rust (`publishUpdate`). |
| `banner.ts` | Pure: what the banner says (`shell/UpdateSlot.tsx` draws it; the frame mounts it, the locked frames do not yet). |
| `native.ts` | The only import of `Desktop` from `ebirforms/native`. |

## Behaviour (and where it differs from the old app)
- Close button, Cmd+W, Cmd+H hide the window (`win.hide()`); the Dock icon stays (NSH has no per-window activation switch: `macos.activationPolicy: accessory` would remove it for good), and a Dock click shows the window again by itself (nsh-platform-macos `applicationShouldHandleReopen`). Per docs/platforms: macOS X and Cmd+Q hide, quit is in the tray; Windows X and Alt+F4 hide, no Ctrl+Q; Linux X and Ctrl+Q quit. The old app_menu.rs bound Ctrl/Cmd+Q to a real quit everywhere: `CLOSE_POLICY` is the one place to flip it.
- Tray: primary click shows the window, secondary click opens Show / Hide / Quit (an `onClick` stops the primary click from opening the menu). Quit = `quitApp()`.
- Dropped from the old menu: macOS Services, Edit > Delete (no NSH role). Items with no NSH role are ours (About, Hide, Close Window, Sign Out, Quit, Bring All to Front); the rest are roles. The Shell's own `keymap` chords (mod+b, mod+k ...) stay; a keymap chord wins over the same menu shortcut, so a key never runs twice.
- Window size and place: nsh.json `window.restoreBounds: true` (NSH keeps the placement itself).
- Hotkey: read from `store.systemInfo().global_hotkey` (the page may read only `theme_preference` with `get_setting`), only while the filing chrome is up; mapped by Rust (`Desktop.hotkeyShortcut`, bir-app-api's `hotkey_combo` rules; a combo with no Control, Alt or Command is refused).
- One copy: `desktop::prepare_process` runs when the native module registers. `<database>.instance` is an exclusive file lock, `<database>.instance.port` the loopback port of the running copy; a second launch sends "show" and exits 0 before any window opens; the first copy's `Desktop.poll().activate` shows its window. It is off in a test environment (`EBIR_TEST_ENV`) unless `BIR_SINGLE_INSTANCE=1`; `=0` turns it off. It never writes beside the owner's folders.
- Updates are OFF in this build (no `"updater"` in nsh.json): `bir.update_status()` is idle with "Automatic updates are not configured in this build, so nothing is checked.", the banner never shows, nothing touches the network. The Rust mailbox is per engine thread (each test `open()` has its own).

## To turn updates on (owner, parked: no keys, feeds or signing in the repo)
1. `nsh keygen` (keys stay with the owner; the public key goes in nsh.json).
2. nsh.json: `"updater": { "github": "hexuria/buwiz-forms", "publicKey": "<keys/update.pub>" }` (or `"url"` of your own host, which needs `publicKey`). `version` must be semver and `build` must grow. The old `BIR_UPDATE_FEED_URL` has no equivalent: the source is fixed when the app is packaged.
3. `nsh package` per platform, then `nsh update-manifest` for `latest.json` and `latest.json.sig`; publish both with the archives. macOS needs a signed and notarized `.app` (the swap runs `codesign --verify`).
4. No code changes: `shellInfo().version` becomes non-null, the driver is "configured", checks run 2 s after sign-in and every 4 hours; Settings' "Download updates automatically" (`auto_update_download`, on when never set) and Help > Check for Updates… work as before. Rehearse against a local server first (docs/dev/updater.md in the NSH checkout).

## Testing
- `menu_model.test.ts`, `updater.test.ts`, `hotkey.test.ts`, `actions.test.ts`: plain. `desktop.test.tsx` (fixture `nsh/DesktopDemo.tsx`, params in its header) and `nsh/shots/desktop.test.tsx` (`just nsh-shots desktop`, real windows, PNGs under ~/BuwizNsh/evidence/desktop).
- No test API reads or clicks the system's menu bar or tray (`app.menu()` sees only `Menu.popup`). The fixture's strip of 8-point buttons runs the installed menu's and tray's `onSelect` (keep it off the window's rounded corner: a click there misses the window); `keyboard.press("mod+h")` goes through the real application menu where the frame has no keymap chord.
- A hidden window shows nothing a test can press or read (`app.texts()` is empty): read `app.window("main").state().visible` and choose a tray item with `app.host.send("run", { id: "tray:Show eBIRForms" })`. Headless, a window shown again comes back at the default viewport (390x844); a real window keeps its bounds (shots test).
- `hotkeyRecorder.test.tsx` (DesktopDemo `solo`, `start: "section=computer"`, `system: "fake"`, `refuse`): the shortcut recorder in Settings against this host: the chord is given back while the box listens, the new one (not the old) is taken when it ends, a refused chord is said under the box.
- Headless, the real `GlobalShortcut.register` never answers (status "pending"); `shortcuts` swaps in a fake. A real macOS window answers at once (registered, or refused when another app holds the chord).
