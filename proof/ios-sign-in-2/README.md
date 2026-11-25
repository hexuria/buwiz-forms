# proof/ios-sign-in-2 — Simulator proof of the new sign-in, PR #345 at fe1a5f46

One line per file. Commit fe1a5f46ad2e4a918bf794024769ac9329c4163f (feat/relay-filing-signin). Devices: iPhone 17 Pro, iOS 26.5 (shots 1206x2622 portrait, 2622x1206 landscape); iPad Pro 13-inch (M5), iOS 26.5 (2064x2752 portrait, 2752x2064 landscape). Every launch: SIMCTL_CHILD_EBIR_TEST_ENV=1, SIMCTL_CHILD_BUWIZ_NO_KEYCHAIN=1, SIMCTL_CHILD_BIR_SFTP_DRY_RUN=1, SIMCTL_CHILD_BUWIZ_SERVER_URL=http://localhost:3008, SIMCTL_CHILD_BUWIZ_TRUST_KEYS=dev-test-key-not-production, SIMCTL_CHILD_BIR_DATABASE_PATH under the app container's Documents; no BIR_AUTH_BYPASS, never BIR_SFTP_LIVE. Server: `just mock-account --port 3008` with defaults (every code 246810). Nothing was queued; no return created. Recordings open on the home screen and the app is launched with `xcrun simctl launch` while recording, so the SIMCTL_CHILD variables apply.

## Screenshots
- 01-wrong-code-4-left.png — iPhone; wrong code 111111 once: "That code is not right. 4 tries left."
- 02-wrong-code-3-left.png — iPhone; wrong code 222222 next: "That code is not right. 3 tries left."
- 03-change-email-same-address.png — iPhone; code page after Change email with the SAME address retyped: back on the code page, countdown continued (4:22 before, 3:41 after — not reset), exactly one code line for the address in the mock's output.
- 04-reopened-on-code-page.png — iPhone; app terminated from the switcher with a code pending and relaunched: the code page returns, countdown continued (4:57 at send, 4:38 after relaunch).
- 05-settings-buwiz-account.png — iPhone; Settings > Account signed in: ONE "Buwiz account" group with the address and "Sign out of Buwiz".
- 06-sign-out-confirm.png — iPhone; the single confirmation the one "Sign out of Buwiz" press asks.
- 07-after-sign-out.png — iPhone; after confirming: the email sign-in page.
- 08-pin-lock-keyboard.png — iPhone; the PIN lock screen ("Enter your TIN and your PIN.") with the number pad up: PIN boxes and the Sign in button visible.
- 09-first-run-keyboard.png — iPhone; first-run help-desk step "Choose the admin PIN" with the number pad up: PIN boxes and Continue visible.
- kb-iphone-email-portrait.png — iPhone portrait; email page, keyboard up: field and "Continue with email" both visible.
- kb-iphone-email-landscape.png — iPhone landscape; email page, keyboard up: field visible, "Continue with email" hidden behind the keyboard (see the report for the exact way to continue).
- kb-iphone-code-portrait.png — iPhone portrait; code page, number pad up: boxes, Resend and Change email all visible.
- kb-iphone-code-landscape.png — iPhone landscape; code page, number pad up: digit boxes visible, Resend / Change email hidden behind the keyboard.
- kb-ipad-email-portrait.png / kb-ipad-email-landscape.png — iPad; email page, keyboard up: field and "Continue with email" both visible in both orientations.
- kb-ipad-code-portrait.png / kb-ipad-code-landscape.png — iPad; code page, keyboard up: boxes, Resend and Change email all visible in both orientations.

## Recordings (iPhone, h264)
- flow-returning-icon-to-home.mp4 — 38.0 s. Home screen, launch, email typed key by key, "Continue with email" pressed WITH THE KEYBOARD UP (it pressed, no arrow key needed in portrait), code page with "Resend in 5:00", 2-4-6-8-1-0 on the number pad, Home. An install that already has the demo taxpayer.
- flow-first-install-icon-to-home.mp4 — 75.3 s. Fresh install (app uninstalled and reinstalled first): sign-in, "Who will use eBIRForms on this device?", New taxpayer for Juana Santos, TIN 000-123-453 tapped digit by digit, to Home. Dead time trimmed at start/end only.
- flow-wrong-codes.mp4 — 35.2 s. The code page: 111111 ("4 tries left"), 222222 ("3 tries left"), then 246810 and Home.

## What this run does not cover
No real device, no Keychain (BUWIZ_NO_KEYCHAIN=1), no live send (mock only, dry run forced), no Android, no dark mode. The PIN lock sign-back-in itself is not recorded. iPad recordings not taken (screenshots only, per the job's recording spec being iPhone).
