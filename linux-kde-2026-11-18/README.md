# App menu, tray and close on Linux (KDE Plasma 6.6.7, X11): evidence, 2026-11-18

What ran: buwiz-forms #234 at `d97dfcc9` (stacked on #230, `feat/app-menu` merged in), on NSH hexuria/nsh#14 at `ad9c1f10e`
(NSH 0.6.0 `buwiz` + the Global Menu export), Linux aarch64, GTK 4, KWin X11 with compositing, plasmashell with a top panel
holding the Global Menu widget (`org.kde.plasma.appmenu`), kded6's `appmenu` registrar and `statusnotifierwatcher`.
Database: a copy of the seeded scratch database (help desk admin, four demo taxpayers).

## Two local stand-ins (not committed)

The PRs pin NSH `699a0f57d`, which is not on GitHub. Against `ad9c1f10e` the app type-checks except for two calls, which the
test checkout replaced:

- `shell/chrome.ts`: `Menu.inGlobalMenu()` → `true`. The one-row window in these shots is therefore the stand-in's
  answer, not NSH's detection. The reactive switch back to two rows when the widget is removed is NOT tested here.
- `shell/desktop/actions.ts`: `Tray.available()` → `true` (correct on this Plasma: it has a tray). The no-tray close
  question is NOT tested here.

Everything else below is real: NSH exporting the menu, Plasma drawing it, items chosen from it running the app's actions,
the tray item and its menu.

## Files

| File | What it shows |
|---|---|
| `01-global-menu-view-open.png` | The app's menu in Plasma's Global Menu (top of the screen), View opened by a pointer click on the panel, shortcuts shown. The window has one row (no in-window menu). |
| `02a-registrar-GetMenuForWindow.txt` | kded's `com.canonical.AppMenu.Registrar.GetMenuForWindow(<the eBIRForms window>)` → the app's bus name and `/com/canonical/menu/nsh`. |
| `02-exported-menu.txt` | The whole exported menu (`com.canonical.dbusmenu.GetLayout`): eBIRForms (About, Settings…, Hide, Sign Out, Quit), File (New Tax Profile… above New Return…), Edit, View, Window, Help, Profiles (the taxpayer switcher, checked = current). |
| `05-tray-click-shows-juana.png` | After Profiles › Juana Santos was chosen through the exported menu (dbusmenu `Event "clicked"`, what the panel sends) the pill reads Juana Santos; Profiles' check moved from Dela Cruz to Juana (`02` lists the before state). The window was then brought back by the tray icon's primary click (`StatusNotifierItem.Activate`). |
| `05-tray-menu.txt` | The tray item: Id `dev.goldcoders.bir.nsh`, its menu Show / Hide / Quit eBIRForms. |
| `07-tray-hide-show.txt`, `07a-hidden-to-tray.png` | Tray › Hide hides the window (unmapped; Show enabled, Hide disabled); tray › Show brings it back (Show disabled, Hide enabled). |
| `08-menu-hide.txt` | Global Menu › eBIRForms › Hide eBIRForms hides the window; tray › Show brings it back. |
| `09-close-window-question.png` | Global Menu › File › Close Window → "Close eBIRForms?" with Quit / Hide to tray and "Don't ask again". The menus stay in the Plasma panel at phone width (no ☰ in the window). |

Not covered: real key presses (Ctrl+Q, Ctrl+H, Ctrl+W, F10) and Wayland.
