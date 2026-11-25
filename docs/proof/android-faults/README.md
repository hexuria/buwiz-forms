# Android fault reproduction proof — KVM-accelerated Linux emulator

Recorded on `origin/main` (51609c0b), development launcher per docs/platforms/devin-environment.md,
mock account, one emulator at a time. Phone: Pixel 7 API 35 (KVM, cold boot ~48 s). Tablet: Pixel Tablet
API 35, landscape (2560x1600). Show-taps was on for every recording. Made-up taxpayers only.

## Files

| file | what it shows | fault reproduced? |
|---|---|---|
| A-typing-new-taxpayer.mp4 | New taxpayer: typing "Maria Clara" key by key on the OSK, then 9 TIN digits, then an address; the keyboard visibly relayouts/collapses between fields and keystrokes misroute | YES (partially — see report) |
| A-typing-mid.png | frame of the same video: keyboard collapsed to a strip mid-typing | YES |
| A-typing-form-search.mp4 | Choose forms search: typing 2551Q key by key on the OSK | no typing fault in the search field itself |
| A-search-2551q-typed.png | the search field with 2551q typed and the keyboard up | — |
| B-keyboard-covers-search.mp4 | phone: result row for 2551Q hidden behind the keyboard, unreachable; hiding the keyboard reveals it | YES |
| B-search-covered-by-keyboard.png | phone: keyboard covers the whole result area | YES |
| B-search-result-after-hiding-keyboard.png | phone: same screen after hiding the keyboard — the 2551Q row is there | YES |
| B-keyboard-tablet-landscape.mp4 | tablet landscape: New taxpayer with keyboard up; scrolling reaches ZIP/RDO/Phone/Email and Create taxpayer | NO — scrolling works |
| B-tablet-scrolled-fields-visible.png | tablet landscape: all four fields fully visible above the keyboard after two scroll swipes | NO |
| C-back-five-times.mp4 | one of the five Back-out runs: form 2550Q, back x7, clean exit to launcher | NO — 5/5 runs clean, no ANR |
| C-form-before-back.png | the form screen before the Back presses | — |
| F-print-preview-phone.mp4 | phone: form 2551Q, tap Print preview, Android system print UI opens (1/2 pages) | NO — opens fine |
| F-print-preview-phone-open.png | phone: system print UI with 2551Q rendered | NO |
| F-print-preview-tablet.mp4 | tablet: form 2551Q, tap Print preview, system print UI opens with both pages | NO — opens fine |
| F-print-preview-tablet-open.png | tablet: system print UI with both 2551Q pages | NO |
| F-phone-printpreview-logcat.txt | logcat 1 s before to 5 s after the phone Print-preview tap (only unrelated gms noise) | — |
| F-tablet-printpreview-logcat.txt | same for the tablet tap | — |

## job 2b additions (docked keyboard, return-to-app, keyboard-between-fields)
NOTE: run on the same API 35 phone, but with the PR #345 dev build (b51fabe4) still installed from job 5, not origin/main; input/insets plumbing is shared.
- `B1-docked-keyboard.png` / `B1-docked-after-typing.png` / `B1-docked-after-scroll.png` - Gboard cleared first (docked, full width). REPRODUCED even docked: the 2551Q result row sits at y1720-1771, below the IME top edge (1517) - behind the keyboard.
- `B1-ime-insets.txt` - dumpsys: mImeHeight=820, ime frame=[0,1517][1080,2400] sideHint=BOTTOM, mImeInsetsConsumed=false - the system reports the inset but the app does not consume it.
- `B1-docked-keyboard-search.mp4` - typing, scroll attempts, tap attempts: row never reveals, taps hit the keyboard (0 selected).
- `G-return-from-other-activity.mp4` / `G-after-font-change.png` / `G-logcat.txt` - return-to-app cases: home+relaunch, recents, font_scale 1.3, Gboard-settings+back ALL keep the same pid and the app stays where it was (Form library). No reset to onboarding reproduced on this host.
- `A-keyboard-between-fields.mp4` + `A-shift-on-at-name-field.png` + `A-keyboard-tin-cell.png` + `A-keyboard-address.png` - keyboard between fields; Shift is ON (filled black arrow) when the name field is first focused.
