# PR #425 Windows click proof

Build under test: `c42626973461a21a2e8ab58caca2405ff0891252` (`feat/searchpick-followups`)  
Sibling nsh: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (verified in the isolated checkout)  
Target: `C:\Users\Administrator\pr425-click-proof\target-c4262697` (fresh)  
Scratch DB: `C:\Users\Administrator\pr425-click-proof\scratch-c4262697.db` (fresh)  
Window: 900x700. Safety env used: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`; `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, and `BIR_SFTP_LIVE` unset. No suites, Queue, push, commit, or live submission.

## Counted results

- **Row pick: 0/5**
  - Run 1: typed `makati`, physically clicked `047 — East Makati`; box became empty, not `047 — East Makati`.
  - Run 2: typed `makati`, physically clicked `048 — West Makati`; box became empty.
  - Run 3: typed `makati`, physically clicked `049 — North Makati`; box became empty.
  - Run 4: typed `makati`, physically clicked `050 — South Makati`; box became empty and showed `“makati” fits 4 choices. Pick one from the list. The box stays empty.`
  - Run 5: typed `makati`, physically clicked `047 — East Makati`; box became empty.
  - In every run the clicked row was not committed. Because no valid row click ever established a value, `047 — East Makati` was selected once with keyboard Enter only as setup for the following chevron/click-away tests.

- **Chevron: 0/3**
  - In all 3 runs, after typing `makati` with the filtered list open, the first physical chevron click did not close the list. Instead it restored/selected the text of the last pick (`047 — East Makati`) and opened the unfiltered/full list.
  - The second physical chevron click did not close the list; the list remained open in all 3 runs.

- **Click-away: 0/3 strict passes**
  - In all 3 runs, after typing `makati`, physically clicking the Phone field closed the list, moved focus to Phone, and restored the RDO box to `047 — East Makati`.
  - However the required note saying the typed text was not taken did **not** appear in any of the 3 runs, so none met the full requirement.

- **Alt+Tab observation**
  - With `makati` typed and the filtered list open, Alt+Tab switched to another Windows window (the system's Shutdown Event Tracker dialog, due window ordering). Alt+Tab back returned to eBIRForms with the RDO box still showing `makati` and the filtered list still open.

## Defects observed

1. Real mouse clicks on option rows do not commit the clicked row; the combobox behaves like a blur/outside press and reverts instead.
2. Real mouse clicks on the chevron do not toggle close/open as required; the first click opens/commits to the full-list state and subsequent clicks leave it open.
3. Clicking into the next field closes and reverts correctly, but the required “text wasn't taken” note is missing.

## Artifacts

- Primary video: `C:\Users\Administrator\pr425-proof-artifacts\pr425-clicks.mp4`
- Alt+Tab return still: `C:\Users\Administrator\pr425-proof-artifacts\alt-tab-back.png`
- Row click failure note still: `C:\Users\Administrator\pr425-proof-artifacts\row-click-failure-note.png`
- Click-away missing-note still: `C:\Users\Administrator\pr425-proof-artifacts\click-away-no-note.png`

The primary video is the genuine capture produced by the recording tool. It was copied unchanged to the required filename; no manual trimming or editing was performed.
