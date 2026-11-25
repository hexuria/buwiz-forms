# iPhone Simulator check — email + 6-digit-code sign-in (PR #345, commit b51fabe44876b0197ba6ab93286d5df51e0812dd)

Device: iPhone 17 Pro Simulator (UDID 405F45A9-B471-47B7-A5D9-E545EA624713), iOS 26.5, Light appearance, hardware keyboard off, status bar overridden (9:41, charged, 100%, 4 cellular bars, 3 wifi bars).
Installed bundle id: com.longbridge.nsh.host (the NSH dev host; not dev.goldcoders.bir).
Server: `just mock-account --port 3008` (every code 246810); restarted once with `--resend-wait 15` for check e.
Launch variables (each as SIMCTL_CHILD_<name>, and no others): EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1, BUWIZ_SERVER_URL=http://localhost:3008, BUWIZ_TRUST_KEYS=dev-test-key-not-production, BIR_DATABASE_PATH=<container>/Documents/buwiz-check/bir_data.db. BIR_AUTH_BYPASS deliberately NOT set. GOOGLE_CLIENT_ID/SECRET unset. Nothing was sent anywhere; no return created; Queue never pressed.
Input method: simctl cannot inject taps or keys. The app was driven through the simulator accessibility/HID bridge (native press/set_text/press_key) plus the on-screen number pad. Everything a person does was possible; two exceptions: (1) tapping the home-screen icon launches WITHOUT the SIMCTL_CHILD variables, so for the recording the app was opened with `xcrun simctl launch` while the recording ran (the sanctioned fallback); (2) typing "@" through the hardware-keyboard channel once produced "2", so the address was entered with the accessibility text setter — visually identical.

## Files
- 01-email-page.png — first launch: email page with the email keyboard up (@ and . keys visible). NOTE: the Continue button is behind the keyboard.
- 02-code-page.png — code page: six boxes, number pad up, "Resend in 4:30" counting down. No iOS one-time-code suggestion appeared above the keyboard.
- 03-wrong-code.png — after typing 111111: red boxes and the message "That code is not right." — DEVIATION: no "4 tries left." suffix.
- 04-reopened-on-code-page.png — after terminate + relaunch: straight on the code page; countdown continued (4:11 before closing → 3:41 after reopening, not reset to 5:00).
- 05-home.png — Home ("Good morning", deadline cards, tab bar). DEVIATION: Home was NOT immediate after the sixth digit on first sign-in — a forced first-run gate appears (see report).
- 06-relaunch-signed-in.png — terminate + launch with the same variables: straight to Home, still signed in.
- 07-reinstall-email-page.png — after uninstall + reinstall (fresh container, fresh database path): signed out, on the email page.
- a-wording.png — the first-run gate, the only screen naming the device: "Who will use eBIRForms on this device?" — "this device", never "this computer".
- b-keyboard-code.png — code page with the number pad up: the Resend button is half-covered by the pad, "Change email" is fully hidden.
- c-paste-246810.png — screen right after pasting 246810: the paste filled all six boxes and signed in at once.
- c-paste-246-810.png — screen right after pasting "246 810" (with space): same result — signed in at once.
- e-resend.png — the Resend state unlocked: the button reads "Resend" and is pressable. While locked it reads "Resend in M:SS", greyed, disabled, not pressable. The mock's `--resend-wait 15` shortened the five minutes; after pressing, it reads "We sent a new code to demo@example.com." and locks again.
- flow-icon-to-home.mp4 — home screen with the NshDev icon, app opened (via simctl launch — see input method), email page, typing, Continue, code page, 246810 on the number pad, Home. One take.
- flow-wrong-code-and-resend.mp4 — code page, wrong code 111111 and its message, Resend unlocking at 0:15, pressed, new code sent.

## What this check does not cover
- A second wrong code and the tries counter (only one wrong attempt recorded; the message never names the tries left).
- The one-time-code AutoFill suggestion (never offered by iOS; the pasteboard menu does offer Paste and AutoFill).
- Real email delivery (mock server only), account deletion, and anything past Home (no return created).
- iPad and landscape (covered separately in proof/ios-gestures/).
