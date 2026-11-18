# Live detection: Menu.inGlobalMenu() and Tray.available() (no stand-ins)

buwiz-forms #234 (`d97dfcc9`, with #230) on hexuria/nsh `feat/menu-tray-availability` (`95b5600c6`), Linux aarch64.
The app's code is unchanged: both values come from NSH.

| File | What it shows |
|---|---|
| `1-plasma-global-menu-one-row.png` | Plasma 6.6.7 with a Global Menu widget: the menu is in the panel, the window is one row. |
| `2-widget-removed-two-rows.png` | The widget removed while the app runs (kded drops the registrar): NSH ends the export, `Menu.inGlobalMenu()` turns false, and the window shows its own menu row (two rows). |
| `3-widget-added-one-row.png` | A widget added again: NSH exports again, the registrar maps the window to `/com/canonical/menu/nsh`, one row. |
| `4-close-with-tray.png` | Plasma (StatusNotifierWatcher present): closing the window (KWin's close, as Alt+F4) asks Quit / Hide to tray. |
| `5-close-no-tray-fluxbox.png` | A desktop with no tray host (fluxbox, its session bus has no StatusNotifierWatcher, not KDE): `Tray.available()` is false, the close question offers Cancel / Quit and says why; the narrow window shows ☰ (no Global Menu). Closed with the window manager's `_NET_CLOSE_WINDOW`. |
