driver: gpui-agent-cli 0.4.1 + nsh-gpui-agent b559e40dc593bd8e8cd78060ddb5a3507a237767

# android-410: a bottom sheet with a focused field sits on the keyboard, not one keyboard-height above it

Issue hexuria/buwiz-forms#410. Driven with gpui-agent only on the Android emulator (api 35, Gboard); adb only
installed, read logcat, screen-recorded and changed the navigation mode (the overlay packages, since the secure
setting does not apply on this image). The app is the development build with the safety variables
(BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1) against a scratch database.

The case: the sign-up form's RDO picker, a bottom sheet with a search field. The search is focused, the keyboard is
up.

- before.png — the app at main (92b3e201) with the kit at its pin (8c78cfd6): the sheet floats a keyboard-height
  above the keyboard, the dimmed form showing through the gap.
- after-gesture-nav.png — the app at hexuria/buwiz-forms#515's head with the kit built from hexuria/nsh#57's branch
  (d4e5eeee), gesture navigation: the sheet sits on the keyboard.
- after-3button-nav.png — the same build, 3-button navigation: the sheet sits on the keyboard, the navigation bar
  below it.
- after-3button-nav.mp4 — the after run in 3-button navigation: open the picker, focus the search, the keyboard
  arrives and the sheet rests on it.
