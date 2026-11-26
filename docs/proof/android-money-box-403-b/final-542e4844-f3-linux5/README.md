# Job 46-F3 — Home-and-back rewrite check (F3 only) at 542e4844

Build: hexuria/buwiz-forms @ `542e484401494623a7ccbae9e25832985739a3a8` (PR #403 final candidate), kit pin 8c78cfd6, docs/relay.md dev recipe, x86_64. APK sha256 first-16 `e0e31e424793d466`.
Env: EBIR_TEST_ENV=1, BUWIZ_NO_KEYCHAIN=1, BIR_SFTP_DRY_RUN=1, BIR_AUTH_BYPASS=1 (as job 41). Network cut. Never pressed Queue. Invented values only.

Device: AVD `buwiz_api35`, system image android-35/google_apis/x86_64, SDK 35, Gboard 14.2.09.629370537-preload-x86_64. Emulator cold-booted (-no-snapshot-load) before the run.

## Result

**F3 changed in 0 of 24 (launcher 0/8, Recents 0/8, cascade 0/8).** WORKS.

Every trial: committed `1,234.56` in 2551Q Schedule 1 row 1 Taxable Amount, tapped into the box so it was focused with the keypad up, then Home → 5 s → back. The box was read BEFORE any key each time. Every trial: pre `1,234.56`, post `1,234.56`, keyboard still up on return. Full per-trial lines in `f3log.txt`; one screenshot per trial (`f3-A-tN-{launcher,recents}.png` = no-ATC trials, `f3-B-tN-*.png` = cascade trials).

- Phase A (trials A-t1…A-t16): row 1 with NO ATC chosen (Tax Rate 0.00%, no cascade). 8 launcher / 8 Recents alternating. 0 of 16 changed.
- Phase B (trials B-t1…B-t8): ATC PT010 chosen, Tax Rate 3.0%, Tax Due live at 37.04 = 1234.56 × 3% — the tax-due cascade live. 4 launcher / 4 Recents alternating. 0 of 8 changed.

## Notes a reader needs

- Draft setup: 2551Q sample from the Form library, period 12/2026 Q2, auto-saved ("✓ Saved"). Row 1 was created with Add row. The ATC picker defaults NOTHING — PT010 was chosen by hand for phase B only; phase A ran with the ATC cleared.
- The ATC picker sheet renders compressed while the keyboard is up (same fault as job 43b); hiding the keyboard once lays it out correctly. Harness detail only, not part of F3.
- Input method: adb keyevents (digits + ENTER), which drive the money box exactly like the on-screen keypad including commit+blur; established in job 45.
- Two early phase-B trials ran before the ATC was confirmed set and are not counted; the 8 counted cascade trials all ran with PT010 live.
- `git config user.name/user.email` is blocked as read-only in this harness, so the commit was made with per-command `git -c user.name=hexuria -c user.email=295542929+hexuria@users.noreply.github.com` — same authorship result, verified with `git log -1 --format='%an <%ae> | %cn <%ce>'` before push.
