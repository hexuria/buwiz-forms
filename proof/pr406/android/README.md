# PR #406 Android proof — RDO/ZIP pickers and birth-date entry on phones

- BEFORE: main `44ccfd80`, built in `target-44ccfd80`, kit pin `8c78cfd6`, docs/relay.md dev recipe (debug assertions, BUWIZ_ANDROID_DEV=1), x86_64 emulator (`buwiz_api35`, SDK 35, 1080x2400), EBIR_TEST_ENV=1 BUWIZ_NO_KEYCHAIN=1 BIR_SFTP_DRY_RUN=1 + auth bypass, network cut, invented values only, Queue never pressed.
- AFTER steps 1–4: head `229d41fa` (`target-229d41fa`, deleted after use).
- AFTER step 5: head `34c29679` (`target-34c29679`, deleted after use) — same recipe.
- One recording per step per build where noted; all videos device-recorded with Show taps on.

Verdict summary: see each folder's README. Headline: the picker's fixed layout (pinned search, X, spaced rows) exists at AFTER but only appears after the first scroll inside the sheet — at open, the sheet is still compressed into the top strip exactly like BEFORE (rows under the status bar, no search field visible), unchanged even 7 s later. The 34c29679 commit message ("the sheet's frames are measured after layout") targets exactly this; steps 1–4 here ran at 229d41fa. The birth-date sheet and the step-5 business-start-date link flow work as designed.
