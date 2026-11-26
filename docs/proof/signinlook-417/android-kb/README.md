# Job 42b — #417 sign-in logo re-check, keyboard up (Android)

- Build: hexuria/buwiz-forms `1cf908a651db63ed204cb5485cb269b2c05f43c6` (feat/relay-filing-signinlook), debug APK, kit pin 8c78cfd6, docs/relay.md recipe, x86_64, own build folder.
- APK sha256: `0d9b851849d0a41c3a1e5da9c5fb082ab59b8442004c67182290e1455876f8f7`
- Replaces linux-5's wrong shots in this folder (commit 87caaa5, which pictured the welcome page).

## Guards (held throughout)

- Local mock account server only, reached through `adb reverse tcp:3008`. Nothing non-local in its log.
- `BIR_AUTH_BYPASS` absent from the packaged dev env.
- `EBIR_TEST_ENV=1`, `BUWIZ_NO_KEYCHAIN=1`, `BIR_SFTP_DRY_RUN=1`. Never BIR_SFTP_LIVE.
- Network cut before the first sign-in (`ping 8.8.8.8` → Network is unreachable).
- Every navigation verified by uiautomator text BEFORE the picture was taken.

## Tap-overlap regression check (done before any picture)

uiautomator dump of the first page: `Button` `gate-start-continue` ("Continue with email") at bounds `[63,1550][1017,1676]`. The only overlapping views are its own ancestors (`auth-scroll`, `ScrollView` containers) — nothing sits over the button. One tap at its centre (540, 1613) navigated to the email page on the first try. **No regression at 1cf908a6.** Evidence: `dump1.xml` (first page), `dump2.xml` (email page after the tap).

## Pictures

Light and dark, each taken after the keyboard had settled (no layout change for seconds, verified by text dump).

| picture | logo whole, words, nothing cut under Back row | no large empty gap between Continue and keyboard while logo clipped | field and button in view |
|---|---|---|---|
| light-01-emailpage-kbup-empty.png | pass | pass | pass |
| light-02-emailpage-kbup-badaddr-error.png | pass | pass | pass |
| light-03-codepage-keypad.png | pass | pass (n/a — digit pad) | pass |
| light-04-tinpin-pinpad.png | pass | pass (n/a) | pass |
| dark-01-emailpage-kbup-empty.png | pass | pass | pass |
| dark-02-emailpage-kbup-badaddr-error.png | pass | pass | pass |
| dark-03-codepage-keypad.png | pass | pass (n/a) | pass |
| dark-04-tinpin-pinpad.png | pass | pass (n/a) | pass |

No picture fails a check. The TIN+PIN page was reached for real: taxpayer created through the UI (invented values), PIN set in Settings → Account → Security, signed out; the lock page shows TIN + PIN boxes with the PIN number pad up.

## Logo size

- Email page with keyboard up (light-01/02, dark-01/02): the logo renders at what looks like the **largest step (150)** — it fills its box fully, with the words, and nothing is cut under the Back row. Measured on screen ≈ 386 px tall on a 2400 px-tall display, consistent with the ~150 pt step fitting in the ~474 pt of room above the keyboard.
- Code page with the number pad up (light-03, dark-03): same large step, whole.
- TIN+PIN page with the PIN keypad up (light-04, dark-04): the logo steps **down** to a smaller size (two digit rows + Sign in + links take the room), but stays whole with its words — the step-down is the intended behavior, not clipping.
