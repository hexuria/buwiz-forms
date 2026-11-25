# Gesture pass — iPhone Simulator, signed in, same run as the sign-in check (PR #345 @ b51fabe4)

Method, plainly: simctl injects no touches or drags. Every gesture below was performed as a REAL drag on the Simulator's screen (mouse down, move, up over the Simulator window) or the simulator's HID/accessibility bridge; navigation between tests used accessibility presses. Colors measured numerically (pixel RGB).

## (a) Back by edge swipe — DOES NOT WORK
Real left-edge drags (fast and slow) on Settings → Account and on the tax profile page: the page neither moves nor pops. Navigation is custom-rendered (NSH), not a UIKit navigation controller — no interactive pop gesture. Tested on those two pushed screens; not tested on a form/filing (opening one creates a draft return, forbidden by the task) nor the code page. One navigation system — expect the same everywhere.

## (b) Sheets and dialogs — WORKS, with a nuance
Tax profile sheet: a drag from the sheet's middle scrolled content; a downward drag from the sheet's top DISMISSED it. Dialogs: none reachable without creating data.

## (c) Lists — momentum present, status-bar tap WORKS, bounce unconfirmed
Fast drag on Home flung well past the drag distance (momentum). Status-bar tap jumped back toward the top. End-of-list bounce not verifiable in stills; see the video. Form library / long form not covered (no forms set up; drafts forbidden).

## (d) Safe areas
iPhone portrait: nothing under the Dynamic Island; home-indicator area clear. iPhone landscape: app switches to a desktop-style sidebar layout; notch at the left edge, sidebar content begins to its right (d-landscape-safe-area.png). iPad Pro 13-inch: no notch; pages centered and clear.

## (e) Keyboard — DOES NOT MEET the requirement on the sign-in pages
Email page: keyboard fully covers "Continue with email" (../ios-sign-in/b-keyboard-email.png). Code page: pad half-covers Resend, fully covers "Change email" (../ios-sign-in/b-keyboard-code.png). The layout does not lift or scroll to keep the submit control visible.

## (f) Two back controls
- Settings → Account: TWO — small "‹ Settings" in the top bar AND a second "‹ Settings" line above the large "Account" title (f-two-back-settings-account.png).
- Settings root: ONE. Tax profile: ONE.
Not covered: General/Device/Import-Export/Jobs pages, New taxpayer, forms/filings.

## (g) System bar strips — top mismatch CONFIRMED (measured)
- iPhone Home light portrait: strip RGB(244,244,246) vs header RGB(255,255,255) — MISMATCH. Bottom: tab bar and strip both RGB(244,244,246) — uniform but grey, not white.
- iPhone Home dark portrait: strip RGB(14,14,16) vs header RGB(22,22,25) — MISMATCH. Bottom uniform RGB(14,14,16).
- iPhone dark landscape: same pattern (edges RGB(14,14,16), top bar RGB(22,22,25)).
- iPad light portrait: sign-in page uniform grey; signed-in iPad strips NOT covered.
Files: g-strips-iphone-light-portrait.png, g-home-light-portrait-raw.png, g-strips-iphone-dark-portrait.png, g-strips-iphone-dark-landscape.png, g-strips-ipad-light-portrait.png, g-strips-ipad-dark-portrait.png, d-landscape-safe-area.png.

## Recording
gesture-pass.mp4 — one unbroken take of the pass; it runs LONGER than two minutes (the pass was exploratory, not a trimmed demo).

## Not covered
Form/filing screens (drafts forbidden), code-page edge swipe, bounce verdict, iPad signed-in strips and landscape, dialogs, keyboard behavior beyond the sign-in pages.

## Late additions
g-strips-ipad-dark-landscape.png: iPad landscape on the sign-in page — uniform grey RGB(244,244,246) everywhere including all edges (this page has no header or tab bar, so no mismatch is visible on it). iPhone restored afterwards: portrait, light appearance, status-bar override re-applied. iPad signed-in strips remain uncovered: the accessibility driver can only observe the first booted simulator and the iPhone held the signed-in session.
