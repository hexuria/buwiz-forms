# e-BIRForms render-check navigation (fresh DB start)
Order on an empty DB: 1 profile, 2 include forms for the year, 3 open a period, 4 go back, repeat.
Coordinates: xdotool display (1920x1080) for the 1280x860 window moved to 0,0 (xdo then reports 18,45). Approximate.
Screenshot (1440x900) point to display: X = x/0.75, Y = (y-45)/0.75. A banner pushes content down about 70 display px.
Helpers: /tmp/claude-1000/-home-agent-workspace/75a3a4d5-f426-4ca2-af38-3fdce8d073d7/scratchpad/vnc/xdo and .../vnc/shot. "xdo" below means the first.

## 1. Create a taxpayer profile
- Empty DB opens on "Taxpayer Profile Setup". Later, "+" beside TAXPAYER PROFILES (252,173).
- Errors seen: TIN must be 3-3-3-5 digits. Required errors for name, registered address, ZIP, phone.
  Not tested: email, start date, status.
- Values that worked (dummy_profile): TIN 123 / 456 / 788 / 00000 (segments auto-advance after 3 digits; check digit passes).
  RDO: type 039, click "039 - South Quezon City" (combobox; Enter not tested).
  Taxpayer Type Corporation (default Individual). Name: Sample Dummy Taxpayer.
  Line of business: Sample Consulting Services. Address: 123 Sample Street, Barangay Example, Quezon City.
  ZIP: type 1100, click "1100 - Central". Phone: 09170000000. Email: sample.taxpayer@example.com (keep lowercase).
  Start date: 01/15/2020 (typed without slashes it stays raw). Status: Active. VAT unchecked (Non-VAT).
- Individual shows Tax Classification, EOPT Tier and Birth Date. Corporation hides the first two and shows
  Business Start Date. 2550Q on Corporation keeps "Draft needs review: taxpayer_classification", so try Individual
  for 2550Q (not verified end to end).
- Save Profile is at the bottom of the Tax Profile tab (scroll down). The profile then shows in the sidebar.
- Gotchas: hover before clicking; typed digits go to whatever field has focus (mine landed in the RDO box).

## 2. Include forms for a year: Profile Settings > COR > Forms & Elections
- Profile actions button (1249,61) > Profile Settings (1117,111) > COR (483,224) > Forms & Elections (515,369).
- Year field (1197,528): click, ctrl+a, type 2025, click the "2025" suggestion (1181,576).
- Filter the "Manually include an official form" input (513,745): type 1601C, click the suggestion (505,793),
  then Add (1211,745). Banner: Save Changes (1168,213). Check the table afterwards.
- MISTAKE TO AVOID: I set Year to 2025, but the heading still read "2026 Forms Set" and the 1601C include
  saved into 2026. Before Add or Save, confirm the heading shows the target year; re-enter the tab if not.
- Set used for review: 2025 = 1601C, 2550Q, 2551Q. On the 2026 set, 1702Q, 1702RT and 2551Q appeared
  without a manual include ("Migration review"). Untested: Withholding "Compensation" box as another route to 1601C.

## 3. Open a draft for a period (Tax Form Library)
- Tax Year button (1207,175) > 2025 at (1213,275).
- 1601C (monthly): "Dec New" at (715,643), library at top. Opens the Dec 2025 draft.
- 2550Q: scroll the library 3 notches from the top, then Q4 "New" near (1192,695).
  2551Q: scroll 6 notches, then Q4 near (715,732). Re-screenshot first; positions move.
- Annual forms (1702RT, 0605) show "Open certification draft". A card appears only if its form is in the year's set.

## 4. Return to the library between forms
- "← Back" at (367,55) on any form page (1280 width). The library reopens at the top, same tax year.
- Or click the profile's sidebar entry: first about (137,327), second about (137,403).
- Do not click Submit, Generate XML, Manual / External Filing or Revert to Draft during review: they change state.

## 5. xdotool tips
- Click: `xdo mousemove X Y; sleep 0.6; xdo click 1`. A bare move-then-click sometimes did not register.
- Wait 1-3 s after each action; the app renders slowly. Re-shoot if a view looks stale.
- Crop check: `shot out.png x,y,w,h` (1440x900 coordinates) to confirm focus and typed text.
- Scroll inside the window only: `xdo mousemove 1000 560; xdo click 5` (one notch is small, about 60 screenshot px).
- Wheel over the desktop switches fluxbox workspaces. Restore Workspace 1 with `xdo set_desktop 0` (not tested by me).
  Ctrl+F1 did not work; the toolbar arrow at (11,1068) steps back one workspace.
- Avoid the sidebar theme button (133,575) and the banner Discard (1057,213): both change state.
- Shared display: another operator moved the pointer during my session. Check `xdo getmouselocation`
  twice and read logs/app.log before acting.

## Setup done (partial, 2026-10-10)
- Profile A "Sample Individual" (TIN 123/456/788/00000, Individual, RDO 039) is saved. Tax Classification is "Self-Employed / Professional" (no "Micro" option), EOPT Tier Micro.
- A's 2025 Forms Set is saved with 34 forms: 0605 0619E 0619F 1600PT 1600VT 1600WP 1601C 1601EQ 1601FQ 1602Q 1603Q 1604E 1701MS 1701Q 1702EX 1702Q 1706 1707A 1800 1801 2000 2000OT 2200A 2200AN 2200C 2200M 2200P 2200S 2200T 2550M 2550Q 2551Q 2552 2553.
- Not in A's set: 1604C, 1604F, 1606, 1707 (no registry match; the registry has 1604CF and 1707A instead). 1700, 1701, 1701A, 1702MX, 1702RT: Add was refused with no message.
- Profile B "Sample Corporation" is NOT saved. 123/456/789 fails the check digit. 000/000/000 is rejected by validation (official_tin_check_code). Save fails silently because the TIN error has no rendered field.
- The unsaved B draft blocks switching profiles, so the library check was not done. Library notes: NOT verified. Before the adds, the 2025 library showed "No forms configured for 2025". Card labels and scroll notches for a monthly, a quarterly and an annual form are still needed.
- Traps: xdo takes display coords, display = (x/0.75, (y-45)/0.75). Screenshot coords land in the sidebar. Check focus before typing: keys go to the page when focus is missed.
- Do not click the banner Discard without the user's OK. It is the only unblock besides a valid TIN or EBIR_RELAXED_VALIDATION=1 on a debug build.

## Display :98 setup (2026-10-11; supersedes the 2026-10-10 version)
Start: app on :98 (bir pid 10353, started 00:21 on 2026-10-11, HOME /home/agent/.cache/bir-review-home) with an empty DB. Data now persists across restarts. Coordinates are :98 screenshot = xdotool pixels, 1440x900, window at 0,0.

Profiles
- A "Sample Individual": TIN 123/456/788/00000 (sidebar 12345678800000), Individual, RDO 039 - South Quezon City, Tax Classification "Self-Employed / Professional" (no "Micro" option), EOPT Tier Micro, birth Jan 15 1980, start Jan 15 2020, Active, Non-VAT, line of business "Sample Consulting Services", address "123 Sample Street, Barangay Example, Quezon City", ZIP 1100 - Central, phone 09170000000, email sample.taxpayer@example.com.
- B "Sample Corporation": TIN 234/567/890/00000 (sidebar 23456789000000), Corporation, RDO 039 - South Quezon City, same line of business, address, ZIP, phone and start date as A, Active, Non-VAT, email sample.corporation@example.com.

Forms by year (state after the 2026-10-11 setup)
Legend: manual = added by hand. auto = Migration review row, left checked. excl = unchecked, shown as Manual override.
- A 2025 (36 codes, the full brief list): manual 0605 0619E 0619F 1600PT 1600VT 1600WP 1601C 1601EQ 1601FQ 1602Q 1603Q 1604C 1604E 1604F 1606 1701 1706 1707 1707A 1800 1801 2000 2000OT 2200A 2200AN 2200C 2200M 2200P 2200S 2200T 2550M 2550Q 2552 2553. auto 1701Q, 2551Q. excl 1701MS.
- A 2024: manual 1700. auto 1701Q, 2551Q. excl 1701MS.
- A 2023: manual 1701A. auto 1701Q, 2551Q. excl 1701.
- A 2026 (not in brief): manual 1701MS. auto 1701Q, 2551Q.
- B 2025: manual 1702RT, 1702Q. auto 2551Q.
- B 2024: manual 1702EX. auto 1702Q, 2551Q. excl 1702RT.
- B 2023: manual 1702MX. auto 1702Q, 2551Q. excl 1702RT.
- B 2026 (not in brief, untouched): auto 1702Q, 1702RT, 2551Q.
- Nothing from the brief was refused. 1604C, 1604F, 1606 and 1707 are in the picker as their own codes.
- Individual COR page has an "Annual Income Tax Election (not extracted from COR)" block (Year 2026, Election empty). Nothing was applied. Corporation has no such block. Do not press Apply or Save Profile with an election entered.

Coordinates (:98; existing values kept, Back and manual-include entries added)
- Profile sidebar: Sample Individual (100,315); Sample Corporation (100,380). "+" new profile (235,151).
- Profile page: "..." (1231,40) opens menu; Profile Settings (1105,88). Do not click "Add to Calendar" (~1105,130).
- Profile Settings tabs (y=200): Tax Profile (381,200), COR (466,200), Email Settings (563,200), Export (667,200).
- COR sub-tabs at page top (y=346): Uploads (375,346), Forms & Elections (497,346), OCR Settings (635,346). Uploads has "View Data", "Create correction draft", "New from Tax Profile +": do not click those.
- Profile page Tax Year button (1189,153). Popover years at y=252: 2022 (1064), 2023 (1107), 2024 (1151), 2025 (1195), 2026 (1238).
- Tax Form Library tab (515,224); Calendar tab (362,224).
- Form page "← Back": (350,28), verified 2026-10-11 on the 1601-EQ draft (the old estimate (367,10) was wrong). Form page top buttons: Print preview (887,28), Save draft (1004,28), Queue for submission (1155,28, disabled). Do not click them.
- Forms set banner at page top: Save Changes (1151,191). Discard (~1057,191): do not click.
- Save Profile: (382,644) with the Tax Profile tab scrolled to the bottom.
- Forms set Year field with the COR page at top: A (1181,692), B (1181,505). Open the list: click field, ctrl+a, BackSpace. Wait 30-45 s after the pick before reading the heading.
  - B list unscrolled: 2024 (1163,576), 2025 (1163,618), 2026 (1163,660). One notch up: 2022 (1163,336), 2023 (1163,378).
  - A list after 3 notches: 2024 (1163,763), 2025 (1163,805), 2026 (1163,845). One notch up: 2023 (1163,799).
- Manual include: the position moves with scroll and banner. A 2025 at 8 notches down, no banner: input (496,285), first suggestion (490,334), Add (1192,285). B 2025 at 5 notches: input (496,425), suggestion (490,473), Add (1192,425). B with the banner: input (496,497), suggestion (490,546), Add (1192,497). Screenshot first.

Library notes (A, 2025; "notches" = wheel clicks from the top, about 78 px each; scroll back to the top after Back)
- Annual cards (0605, 1604C, 1707, 1707A, 1800, 1801, 2000, 2000OT) show one "+ Open certification draft" button.
- Monthly and quarterly cards read "Certification preview" with tiles: 0619E, 0619F, 1600PT, 1600VT, 1600WP, 1601C, 1601EQ, 1601FQ, 1602Q, 1603Q, 2200P, 2200S, 2200T, 2550M.
- 1601EQ (right): title y~398 after 16 notches. Q4 tile (1174,557) opens the "BIR Form No. 1601-EQ" draft (Q4, 2025, step 1 Draft). Opening it may have created a draft record; nothing was saved or submitted.
- 1601FQ (left) and 1602Q (right): title y~225 after 24 notches.
- 2200P (left) and 2200S (right): title y~244 after 64 notches. 2200T (left): title y~677 after 64 notches.

Gotchas
- The forms-set heading can lag 30-45 s after a year pick. If it still shows the old year after 60 s, reopen the list, clear it and pick the year again (B 2024 needed a second pick).
- The manual-include popup can lag 5-15 s and show the previous code. Wait about 10 s, hover over the popup at (480,345)-(480,380) to repaint, and check the input text before Add. One typed "1800" did not reach the input and had to be re-entered.
- After each Add the banner shows; click Save Changes (1151,191). The input clears after the save.
- Auto-suggested annual rows block the other annual form in the same group. Uncheck the suggested row first (1701MS in 2025/2024; 1701 in 2023; 1702RT in 2024/2023), then Add.
- Auto rows that are not annual (1701Q, 2551Q, 1702Q) were left checked; the rule only excludes conflicting annual rows.
- After Save Changes on B 2024 and 2023, an "INCOME TAX / Form 1702RT" panel showed below the table although 1702RT is unchecked. It was gone on re-open and is not in the saved set.
- The "Unsaved profile changes" banner pushes the form down about 70 px.
- A click on a card's blank area in the library does nothing. A click at (1231,40) while the library was scrolled landed on the 1601EQ card's blank area.
