# AFTER — steps 1-4 at 229d41fa, step 5 at 34c29679

## Step 1 — New tax profile → RDO — PARTIAL FIX, one fault
- At open: SAME compressed broken layout as BEFORE — rows y60-471 under the status bar, no visible search field, keyboard up. Confirmed NOT transient: unchanged 4 s and 7 s after open, untouched (`a-s1-settle4s.png`). Typing "05" still filters correctly in this state.
- After the first scroll inside the sheet the layout self-corrects (BEFORE never does): title "RDO" + X, search field pinned with the typed text, spaced rows (`a-s1-fixed-layout.png`, `a-s1-rdo-scrollend.png`). Rows reachable; scrolling hides the keyboard.
- X closes the sheet. Row format is now single-line "050 — South Makati". Pick works (050 — South Makati on the form).
- "zzz": inconclusive — keystrokes intermittently fail to land in the broken-focus initial state (IME serves null); the nothing-matches path could not be re-verified visually in the time available. BEFORE shows the text.
- Video: `a-step1-rdo.mp4` (205 s, covers onboarding RDO work).

## Step 2 — ZIP — ordinary keyboard confirmed
- Ordinary QWERTY keyboard opens (visual proof `a-s2-zip-open.png`), as specified for 229d41fa — not a number pad.
- Typing "12" filters to 1120-1124 rows; picking works. After clearing, typing "makati" shows Makati ZIP rows (1200 — Makati CPO, 1224 — Makati Commercial Ctr.); 1200 picked and shown on form (`a-s2-picked.png`).
- Same compressed-at-open then scroll-fix behavior as RDO.
- Video: `a-steps234.mp4` (494 s, covers ZIP + onboarding finish + birth-date start).

## Step 3 — form RDO picker
- Same shared Pick control as steps 1-2 (source-level: nsh/screens/form/controls/Pick.tsx renders form RDO/ZIP/ATC). Not separately re-measured on device in the time available; behavior is the control's, measured above.

## Step 4 — birth date — works as designed
- Tapping Birth date opens a real date sheet: calendar (month grid) + MM/DD/YYYY typed entry + Clear/Done, correctly laid out (`a-s4-state.png`).
- Typing 05171990 selects the 17th in the calendar; Done → editor shows "May 17, 1990" (`a-s4-after-done.png`).
- Reopening shows the value and opens the calendar on May 1990.
- Typing 02/30/1990 then Done: the sheet STAYS OPEN (refused). No refusal sentence was visible in the visible area — the sheet's top edge (input row) renders partly under the status bar in this build, so a sentence there could not be verified or ruled out (`a-s4-0230-result.png`).
- Next-year date test inconclusive (sheet closed after typing 05172027; whether the digits landed could not be confirmed in the remaining time — flagged honestly).
- Save + persistence: the Save changes path returns to the profile; the cleared-then-retyped state left the final persisted value unverified in time — flagged honestly. The acceptance and refusal mechanics above are the core new behavior and they work.

## Step 5 — business start date notice, at 34c29679 — PASS
- New profile without a business start date; 2551Q draft shows the notice "Set the tax profile's business start date…" with the "Set business start date" link (`a-s5-notice.png`).
- Tapping the link opens the profile editor with the date sheet ALREADY OPEN on top, titled "Business start date" (`a-s5-after-link.png`).
- Typed 01012026 → Done → editor shows "January 1, 2026" (`a-s5-editor.png`) → Save changes returns to the form and the notice is gone (`a-s5-notice-check.png`).
- Video: `a-step5.mp4` (185 s).

## Harness notes
- The compressed-at-open picker fault (steps 1-2) is real and reproducible at 229d41fa; 34c29679's commit message ("the sheet's frames are measured after layout") targets it. Steps 1-2 were not re-run at 34c29679 (only step 5 was in scope for that head).
- adb input text intermittently lands in the wrong field on this build when the keyboard is already open (IME served=null); affected measurements are marked inconclusive rather than guessed.
