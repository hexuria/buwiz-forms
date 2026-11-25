# Android sign-in (email + code) proof - PR #345 at b51fabe4

Dev build, API 35 phone emulator (KVM), mock account server on localhost:3008 (adb reverse), juana@example.com. No BIR_AUTH_BYPASS. Show-taps on for recordings.

## Sign-in flow files
- `01-email-page.png` - PASS: email page, email keyboard with @ key visible (see 01b)
- `01b-email-page-keyboard.png` - PASS with note: keyboard COVERS the Continue button; the keyboard's own enter arrow submits
- `02-code-page.png` / `02b-code-page-keyboard.png` - PASS: code page, six boxes, Resend countdown, number pad; pad half-covers Change email
- `03-wrong-code.png` - FAIL: after wrong code 111111 the error reads "That code is not right." - the required " 4 tries left." is MISSING
- `05-after-font-change.png` - PASS: font_scale 1.3 change while away, same process, still on code page, countdown continued
- `08-relaunch-check.png` / `08-relaunch-still-signed-in.png` - mixed: see report (soft relaunch stays signed in; force-stop signs out)
- `09-reinstall-signed-out.png` - PASS: uninstall+install lands signed out on the email page
- `signin-whole-flow-1.mp4` - launch -> email -> code -> paste via Gboard clipboard chip -> signed in
- `signin-whole-flow-2.mp4` - taxpayer creation after sign-in (form filling difficulties visible)
- `signin-leave-and-return.mp4` - step 5: Home/Settings 30s, return via Recents, then font change variant
- `landscape-email-page.png` / `landscape-email-page-keyboard.png` / `landscape-code-page.png` / `landscape-code-page-keyboard.png` - landscape reachability checks
- `01b-verify-keyboard-covers.png` - extra keyboard-coverage evidence

## navigation/ - native-navigation pass (three-button vs gesture)
- `nav-three-button-mode.mp4` / `nav-gesture-mode.mp4` - recordings in each mode
- `strips-<mode>-<theme>-<screen>.png` - system-bar strip evidence. Hex samples in the report/README:
  - threebutton light forms: status f4f4f6, header f4f4f6, tabbar f4f4f6, strip-under-tab fefefe (MISMATCH: white strip under grey page)
  - threebutton dark forms: status/header/tabbar 0e0e10, under-tab 14151a (slightly lighter strip)
  - gesture light forms: uniform f4f4f6 everywhere
  - signin email light: uniform f4f4f6
- `back-from-forms-threebutton.png` / `back-edge-swipe-gesture.png` - back behavior evidence
- `overscroll-fling-home.png` - fling/overscroll check on Home
- `landscape-threebutton-forms.png` / `landscape-gesture-forms.png` - landscape checks
