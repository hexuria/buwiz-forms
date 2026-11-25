# Proof for PR #359 (fix/print-failure-log) at e25bca84 — macOS UI run

App commit: e25bca84fd74e6027cb96b81f7a0959b8923c6fd (detached). Kit: nsh 0.6.1 at NSH pinned ref 8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf (sibling checkout at the same commit, CLI rebuilt from it). macOS 26.6.2. Dev run: `just nsh-run-agent macos` with a fresh scratch database, GPUI_AGENT_TOKEN set, BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1, BIR_LOG_DIR pointing at the scratch folder. Demo taxpayer only: Juana Santos, TIN 000-123-453, branch 00000, Individual, RDO 039, Quezon City, ZIP 1100. Nothing queued, nothing sent, no account signed in. Input: real input only — cliclick (HID) for everything, plus System Events key codes for Page Up / Cmd-Shift-G / Return, and one accessibility press through System Events for step c) as detailed there. No agent presses (an agent press does not reach the second window, issue #358).

## What each file shows

- `step2-print-save-cancel.mp4` — one screen recording of all of Step 2, sped up 6x (144 s) to fit the three-minute limit: taxpayer creation, 2551Q Q4 2026, taxable 100,000.00 at PT010, Print preview, then steps b) c) d) as below.
- `01-b-save-panel-up-toolbar-preparing-with-cancel.png` — step b): the save panel is up and the preview's toolbar reads "Preparing the PDF…" with a Cancel beside it; Save as PDF and Print disabled.
- `02-b-panel-cancelled-toolbar-normal-no-note.png` — step b) after cancelling the save panel itself: toolbar back to normal (just "A4 · 2 pages · Folio sheet at 88%"), no note, both buttons enabled.
- `03-c-toolbar-cancel-note-shown-panel-stayed-up.png` — step c): after pressing the toolbar's Cancel (see below how), the note reads "Stopped waiting. Press the button again to try once more." and both buttons are enabled again — while the save panel is STILL UP. That is the answer to the task's question: the panel stayed up; it did not close. It was dismissed afterwards by its own Cancel.
- `04-c-note-persists-after-panel-dismissed.png` — after dismissing the panel: the note persists with both buttons enabled.
- `05-d-saved-note.png` — step d): third press of Save as PDF, saved into the scratch folder; the note reads "Saved BIR Form 2551Q - Juana Santos.pdf".
- `saved-bir-form-2551q-juana-santos.pdf` — the file saved in step d): 350,069 bytes, 2 pages.
- `step2e-log-lines.txt` — every line the app logged from the moment of b) to d): exactly one line, `WARN nsh_app_ebirforms::print: Save as PDF: the person stopped waiting for it to finish` at 16:49:34Z, written by the toolbar Cancel in step c). Nothing was logged for b) (panel cancelled) or d) (save completed).

## What the check does not cover, and deviations

- Step c) input method, read this first: on macOS the save panel is a window-modal sheet of the preview window itself. While it is up, NO real HID click reaches the preview window — proven on the recording: two correctly-placed cliclick clicks on the toolbar's Cancel (and one on the 100% zoom control) did nothing; the system swallows them for the modal panel. The Cancel was therefore pressed through the app's accessibility tree with System Events (`perform action "AXPress"` of the toolbar Cancel button) — the second input path the task sanctions. It worked at once: the note appeared and the buttons re-enabled while the panel stayed up.
- Consequence for the feature as designed: during the panel phase of the wait, a person using mouse or touch cannot press the toolbar Cancel at all on macOS (the modal panel eats the clicks); the Cancel is only reachable once the panel is gone but the wait still hangs (a panel that never answers after being dismissed, or a slow PDF write). Keyboard/VoiceOver users can reach it through accessibility, as this proof did.
- Print and export/import failure logging are not exercised here; only Save as PDF, per the task's steps.
- The fifth screenshot (04) is additive evidence, not a required one; the required three are 01 (b), 03 (c), 05 (d).
- One anomaly seen but not explained: after the modal phase ended, the zoom control showed "100%" selected in step d) though it read "Fit width" in step b); possibly one of the swallowed clicks was delivered late. Visible in the recording; no effect on the findings.
- Build-folder deviation (report-only, no code changed): the justfile recipes hard-pin CARGO_TARGET_DIR=~/BuwizNsh/target-nsh, so this run (and the just-invoked CI steps) built in target-nsh, not target-nsh-pr359. Direct-cargo steps used target-nsh-pr359 as ordered.
