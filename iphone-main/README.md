driver: gpui-agent-cli 0.4.1
recipes: agent-recipes/signin-look.json (pin-error + newtp + signout-gap), agent-recipes/signin-look-lock.json (TIN+PIN lock attempt), agent-recipes/signin-look-bottom.json (newtp bottom scroll)
app: hexuria/buwiz-forms main 517374ded99efcc790dcbd70df69c55728f1f6a9, nsh 0.6.1 at pin 8c78cfd6, iPhone 17 + iPhone SE (3rd gen) simulators, iOS 27.0, light + dark.

## Per picture

1. Email page, field focused, keyboard up — NOT CAPTURED on any device/mode: known_gap hexuria/nsh-gpui-agent#13. The email and code pages live behind the account gate, which is reached only by signing out; sign-out is a press inside a sheet (the navbar avatar menu), and presses inside sheets answer ok but nothing happens (evidence: signout-gap recipe, report-A.json per combo: `wait_until timed out: node locked-screen not found` after `press nb-menu-logout` answered ok). No fallback was used, per the rules.
2. Code page just opened, number pad up — NOT CAPTURED on any device/mode: known_gap hexuria/nsh-gpui-agent#13, same cause as picture 1.
3. TIN + PIN with the PIN boxes focused — NOT CAPTURED on any device/mode: known_gap hexuria/nsh-gpui-agent#13 plus a platform limitation. Reaching the lock screen needs either sign-out (sheet press, gap #13) or the BIR_SHELL_START lock route, which cannot reach an iOS app process: nsh.json capabilities.process is empty, so nsh passes no SIMCTL_CHILD_* environment to the app (evidence: signin-look-lock run reports, `no node matches ^PIN` at resolve time on all four combos).
4. Setup "Choose the admin PIN" in its error state, keyboard up — CAPTURED on all four combos (002-s2.png app surface; 02-pin-error-shot-last.png full screen with the number pad).
5. "Just me" (New taxpayer) top — CAPTURED on all four combos (005-top.png). Bottom — see per-combo lines.

## Per check (picture 4, all combos)

- logo WHOLE with its words: PASS on all four combos.
- nothing cut under the Back row: PASS on all four combos (Back fully visible above the keyboard).
- no large empty band between content and keyboard: PASS on all four combos (standard keyboard spacing only).
- focused field and its button in view: PASS on all four combos (first PIN box focused with cursor, Continue in view).
- honest note: on iPhone SE the number pad's bottom row sits at the screen edge in the full-screen captures; the pad is crowded but the Back row and all content are fully visible.

## Per check (picture 5 top, all combos)

- logo WHOLE with its words: PASS on all four combos.
- nothing cut under the Back row: PASS — this page has no Back row at the top; nothing is cut.
- no large empty band: PASS.
- focused field and its button in view: not applicable — no field is focused on arrival; the page top is shown as ordered.

## Per check (picture 5 bottom)

- CAPTURED on all four combos (newtp-bottom.png app surface; newtp-bottom-screen.png full screen). Scrolled with two gpui-agent scroll ops, dy +600 each, on auth-scroll — the first recipe attempt used dy -500 (wrong direction, the page did not move) and was corrected.
- logo WHOLE: not applicable at the bottom — the logo scrolls away with the page top; it is verified whole in the top shots.
- nothing cut: PASS on all four combos (every field whole, Cancel and Create taxpayer fully visible).
- no large empty band: PASS — no keyboard is up on this page; the content ends with the buttons at the page bottom.
- focused field and its button in view: PASS — no field is focused; the Create taxpayer button is fully in view on all four combos.
- honest note: the mid-run bottom shots in 008-bottom.png (signin-look-newtp.json) show the page nearly unmoved — they used the wrong-direction scroll; the correct bottoms are newtp-bottom.png from signin-look-bottom.json.

## Run metadata

Each combo folder holds report-A.md/report-A.json (just e2e runner output for the signin-look suite) and report-B.md/report-B.json (the lock-route attempt). Videos of every recipe run are the --record output referenced inside report-A.json; the png files are gpui-agent screenshots (app surface) and ffmpeg-extracted last frames of the recordings (full screen incl. keyboard).
