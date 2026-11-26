# Job 43b — sheet re-check at 34c29679 (the installed build)

## Steps 1-2 at open: the compressed layout is NOT fixed at 34c29679
- RDO sheet at open, keyboard UP: rows at y60-471 compressed into the top strip under the status bar, NO search field, NO title/X — unchanged at 1 s (`a34-rdo-open-1s.png`) and at 3 s (`a34-rdo-open-3s.png`). The list does open scrolled to the current choice (rows around 050 / 1200).
- ZIP sheet identical: compressed, no search visible at 1 s and 3 s (`a34-zip-open-1s.png`, `a34-zip-open-3s.png`).
- Measured in the profile-editor context (same shared Pick control as onboarding). The 34c29679 "frames measured after layout" change does not repair the keyboard-up open state on this emulator. Note (per ios-submission's reading): the "self-correct after a scroll" seen at 229d41fa is really the scroll DISMISSING the keyboard — the sheet only lays out correctly once the keyboard is down. Keyboard-up vs keyboard-down internals (windowSize/keyboardFrame/safeArea/softInputMode/view bounds) are measured in the follow-up deferred after job 45.

## Step 4 refusal sentences — obstructed by the same fault, honestly reported
- In the taxpayer editor the date sheet renders shifted up: the month header half-clips under the status bar and the MM/DD/YYYY input row — and any sentence under it — is OFF-SCREEN with the keypad up (`a34-0230-top.png`, crop). The input cannot be seen, so a refusal sentence can neither be confirmed nor ruled out visually.
- Keypad-tap entry of 02/30/1990 (taps at the on-screen keys' positions) followed by Done: the sheet STAYS OPEN (`a34-0230-done.png`). No calendar day becomes selected, so whether the digits even landed is unverifiable in this state — the input row is invisible. The next-year test is inconclusive for the same reason.
- What is established: the impossible date is not committed (the sheet does not close; earlier at 229d41fa typed input via `adb input text` did reach the field — 05171990 selected the 17th — so the entry path itself works; the keypad-tap path is what's unverified).

## Step 3 — form RDO picker
- No separate device run. Source level: `txtRDOCode` in both 2551Q and 1701Q is a profile-backed form box, and form boxes with long option lists render through the SAME `SearchPick` control measured in steps 1-2 (`nsh/screens/form/controls/FieldControl.tsx` → `./Pick`). Its sheet behavior in a form is the control's behavior measured above.
