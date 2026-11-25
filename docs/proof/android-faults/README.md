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
