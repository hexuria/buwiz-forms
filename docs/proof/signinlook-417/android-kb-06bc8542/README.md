# Job 42b rerun — #417 sign-in keyboard proof at 06bc8542 (Android)

- Build: hexuria/buwiz-forms `06bc85426a9aaa0aeaf82a78613f19fa0257244e` (feat/relay-filing-signinlook), debug APK, kit pin 8c78cfd6, docs/relay.md recipe, x86_64, own build folder.
- APK sha256: `b3f64054cc76c4ecced90c6fd78fff3b0e85f9abe5a9856fe953bcb39007b6b0`

## Guards (held throughout)

- Local mock account server only, reached through `adb reverse tcp:3008`; nothing non-local in its log.
- `BIR_AUTH_BYPASS` absent from the packaged dev env (not baked; gate passed with the mock's real code).
- `EBIR_TEST_ENV=1`, `BUWIZ_NO_KEYCHAIN=1`, `BIR_SFTP_DRY_RUN=1`. Never BIR_SFTP_LIVE.
- Network cut before the first sign-in (`ping 8.8.8.8` → Network is unreachable).
- Navigation verified before every picture (see note below).

## Tap regression check (before any picture)

uiautomator dump of the first page: "Continue with email" text bounds `[385,1558][759,1605]` inside its container `[63,1519][1017,1645]`; the only overlapping views are its own ancestors — no measuring view sits over the button. One tap at the centre navigated to the email page on the first try. **No regression at 06bc8542.** Evidence: `dump1.xml`, `dump2.xml`.

## Verification note

uiautomator verifies the email page and the TIN+PIN page by text. The **code page is not exposed in the a11y tree at all** (known since job 44 — the dump keeps showing the previous page), so its navigation is verified visually in the shots themselves; the email→code transition was additionally confirmed by the mock issuing a fresh code at the moment of arrival.

## Pictures

Light and dark, keyboard settled before each shot.

| picture | logo whole, words, nothing cut under Back row | no large empty gap while logo clipped | field and button in view |
|---|---|---|---|
| light-01-emailpage-kbup-empty.png | pass | pass | pass |
| light-02-emailpage-kbup-badaddr-error.png | pass | pass | pass |
| light-03-codepage-keypad.png | pass | pass (n/a — digit pad) | pass |
| light-04-tinpin-pinpad.png | pass | pass (n/a) | pass |
| dark-01-emailpage-kbup-empty.png | pass | pass | pass |
| dark-02-emailpage-kbup-badaddr-error.png | pass | pass | pass |
| dark-03-codepage-keypad.png | pass | pass (n/a) | pass |
| dark-04-tinpin-pinpad.png | pass | pass (n/a) | pass |

No picture fails a check. The TIN+PIN page was reached for real (fresh taxpayer, PIN set in Settings → Account → Security, sign-out).

## Logo size

Email and code pages with the keyboard up show the largest step (150), whole with its words — the fix holds at 06bc8542. The TIN+PIN page steps the logo down (two digit rows + Sign in + links take the room) but it stays whole — intended behavior.

## Back checks (new at this head)

`input keyevent 4`, then `dumpsys activity activities`:

- **First page** (fresh install, gate's first page): single Back → `topResumedActivity=...com.google.android.apps.nexuslauncher/.NexusLauncherActivity`. The app leaves. PASS — `back-firstpage-dumpsys.txt`.
- **TIN + PIN**: with the PIN keypad up, the first Back dismisses the pad; with the pad down, a single Back leaves to the launcher (`nexuslauncher` resumed). PASS — `back-tinpin-dumpsys.txt`.

## Nav-bar check (new at this head)

`wm size 720x1280`, `wm density 320`, `cmd overlay enable com.android.internal.systemui.navbar.threebutton`, on TIN + PIN:

- "Forgot PIN?" bounds `[293,1088][427,1127]`; the three-button bar's top edge is at ≈ y1190 (bar region y1190–1280 measured on screen). The control sits **above the bar** with ~63 px to spare. PASS — `navbar-tinpin.png`.
- Size, density and the overlay were reset afterwards.
