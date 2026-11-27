# PR #425 fef911cf Windows click proof

Build under test: `fef911cf8baf979e5cb399cf0bf93a4593e214e5` (`feat/searchpick-followups`)  
Sibling nsh: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (verified in the isolated checkout)  
Target: `C:\Users\Administrator\pr425-fef-click-proof\target-fef911cf` (fresh)  
Scratch DB: `C:\Users\Administrator\pr425-fef-click-proof\scratch-fef911cf.db` (fresh)  
Window: 900x700. Safety env used: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, and `BIR_SFTP_LIVE` unset. No suites, Queue, push, commit, or live submission.

## Counted results

- **Row pick: 5/5**
  - Run 1: clicked `047 — East Makati`; box showed `047 — East Makati`, no note.
  - Run 2: clicked `048 — West Makati`; box showed `048 — West Makati`, no note.
  - Run 3: clicked `049 — North Makati`; box showed `049 — North Makati`, no note.
  - Run 4: clicked `050 — South Makati`; box showed `050 — South Makati`, no note.
  - Run 5: clicked `047 — East Makati`; box showed `047 — East Makati`, no note.
  - Caveat: the filtered `makati` list contains only four RDO rows (`047` through `050`), so five mutually distinct rows were impossible; the fifth run reused `047` after all four distinct rows had been clicked.

- **Chevron: 3/3**
  - In each verified run, the first real chevron click closed the list and restored `047 — East Makati` with the refusal note.
  - The second real chevron click opened the full list.

- **Click-away: 3/3, note Y**
  - In all 3 runs, typing `makati` and clicking into Phone closed the list, restored `047 — East Makati`, and left the note visible after the click.
  - Note text shown: `“makati” fits 4 choices. Pick one from the list. The box kept 047 — East Makati.`

- **Alt+Tab: PASS**
  - With `makati` typed and the filtered list open, Alt+Tab switched to Docker Desktop (a benign second window; the prior Shutdown Event Tracker was closed before the run).
  - Alt+Tab back showed the list closed, the box restored to `047 — East Makati`, and the note visible: `“makati” fits 4 choices. Pick one from the list. The box kept 047 — East Makati.`

- **ZIP regression: PASS**
  - Typed `12`, clicked the first visible row, and the box picked `1012 — Tondo South` with no note.

## Defects observed

None in the required flows. One extra chevron close/open cycle was performed during capture because one batched run did not have a separately inspected intermediate screenshot; the counted three chevron runs were visually verified.

## Artifacts

- Primary video: `C:\Users\Administrator\pr425-fef-proof-artifacts\pr425-fef911cf-clicks.mp4`
- Click-away note still: `C:\Users\Administrator\pr425-fef-proof-artifacts\click-away-note.png`
- Alt+Tab return still: `C:\Users\Administrator\pr425-fef-proof-artifacts\alt-tab-back-note.png`
- Chevron reopened list still: `C:\Users\Administrator\pr425-fef-proof-artifacts\chevron-reopen.png`
- ZIP pass still: `C:\Users\Administrator\pr425-fef-proof-artifacts\zip-click-pass.png`

The primary video is the genuine capture produced by the recording tool. It was copied unchanged to the required filename; no manual trimming or editing was performed.
