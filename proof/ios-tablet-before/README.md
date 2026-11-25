# proof/ios-tablet-before — forced phone shell on iPad, "before" pictures

Branch `try/ios-tablet-phone-shell-forced` at exactly **9b962644** (main plus the experiment line:
on iOS the app always takes the narrow phone shell). Device: **iPad Pro 13-inch (M5) simulator,
iOS 27.0** (2064x2752 px portrait, 2752x2064 landscape, scale 2 = 1032x1376 pt / 1376x1032 pt).
Light appearance, status bar overridden to 9:41 / full wifi / full battery.
Demo taxpayer Juana Santos only, TIN 000-123-453; one draft 2551Q Q4 2026 (Schedule 1 row 1,
PT010, 250,000.00 at 3.0 = 7,500.00), never queued. Scratch database, all dry-run switches.

**Multitasking & Gestures**: could not be confirmed — iPadOS 27.0 Settings search finds no
"Multitasking" entry at all (the pane may be renamed or absent in this beta runtime). The
pictures show no window title bar or resize handle inside the sim, so the app is effectively
in the full-screen mode regardless.

**Driven with**: gpui-agent (window plane taps/presses by id and by sim-point coordinates,
`paste`/`set-value` for plain fields) plus real mouse clicks and keystrokes through the
Simulator window for the fields the agent's hit-test cannot reach (the row-editor drawer's
amount field — typed digit by digit; a fast `type` dropped digits, that is an input-method
artifact, not an app fault). Screenshots are `xcrun simctl io <udid> screenshot` at full
resolution. Rotation via the Simulator Device menu.

## One line per picture

Portrait:
- `forced-01-home-portrait` — Home tiles stretch the full 1032 pt width; one tile per row with
  vast empty right-hand space; the strip of days runs edge to edge. Looks wrong: rows are
  ~1000 pt wide where a phone shows ~350.
- `forced-02-forms-portrait` — form library list, full-width rows, long text lines, lots of
  empty space right of the row text.
- `forced-03-filings-portrait` — filings list, same full-width stretching.
- `forced-04-calendar-portrait` — month grid spread across the whole width (day cells ~140 pt
  wide each), deadlines list underneath edge to edge; looks sparse but readable.
- `forced-05-account-menu-portrait` — the round-initials menu: a small anchored menu in the
  top-right, actually one of the less broken pieces at this width.
- `forced-06-profile-sheet-portrait` — the tax profile sheet covers the RIGHT HALF of the
  screen (~half of 1032 pt) where a phone shows a bottom sheet; the left half shows the dimmed
  page. Oddly placed and far too wide, with the sheet's rows stretched inside it.
- `forced-07-notifications-sheet-portrait` — same right-half sheet for notifications, nearly
  empty ("No notifications yet"); a huge blank panel for one line of text.
- `forced-08-settings-list-portrait` — settings nav rows full width, long lines.
- `forced-09-settings-account-portrait` — Settings > Account: rows and cards full width,
  sign-in card stretched ~1000 pt wide.
- `forced-10-tax-profile-portrait` — tax profile page: the profile cards span the full width;
  two-column field layout inside cards is extremely spread out (label far left, value far right).
- `forced-11-form-portrait` — the 2551Q open: the parts nav is a LEFT SIDEBAR column plus the
  form column next to it, so the form itself is squeezed into the right ~60% even though the
  shell is the phone one; the printed-form rows within are cut off at the drawer side. (The
  sidebar+content split survives from the wide layout while the tab bar is the phone's.)
- `forced-12-form-row-editor-portrait` — the row editor is a RIGHT-HALF DRAWER covering half
  the screen where a phone shows a bottom sheet; inside it the ATC line truncates and the
  amount/rate/due rows stretch to the drawer width.
- `forced-13-form-totals-portrait` — totals area with the floating to-do pill: the pill is a
  small phone-sized lozenge floating over a very wide canvas, centered oddly with huge margins.
- `forced-15-search-portrait` — WRONG CONTENT, kept for honesty: this shows the account sheet
  (right-half panel), not the search palette — the search menu item could not be reliably
  opened in the time available. It still documents the sheet-vs-screen fault.

Landscape (same notes amplified — everything wider still at 1376 pt):
- `forced-01-home-landscape` … `forced-13-form-totals-landscape` — same screens as the portrait
  set; the stretching is worse (rows/cards ~1300 pt wide), the sheets/drawers still take the
  right half but the half is now wider than a phone screen; the bottom tab bar spans the full
  width with large empty gaps between its four items.
- `forced-15-search-landscape` — same wrong-content caveat as the portrait one.

## Missing
- `forced-14-new-taxpayer-zip-picker` (both orientations) — not taken: the new-taxpayer flow is
  only reachable through Log out on an installed app and there was not enough time before this
  machine was reassigned; no picture exists rather than a wrong one.
- The whole `main-` set (desktop frame at 6bb90ad1) — skipped entirely per ios-submission's
  follow-up instruction.

## What the pictures do not show
- No version number anywhere on screen was checked for explicitly; none of the screens in this
  set is known to display one.
- Pixel-exact measurements are eyeballed from the 1032x1376 pt canvas, not instrumented.
