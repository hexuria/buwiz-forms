# PR #406 Windows UI proof

BEFORE: `44ccfd8080f3028528077cd9a7fa64733846f309` (main)
AFTER: `a9c9fd7c0a50a3639435dcb3e83c71fdb6f6bdea` (PR #406 head)
NSH pin for both: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`

## before/

- `step1-rdo.png` / `step1-rdo.mp4` — new tax profile RDO using the legacy trigger+popover control; typing and row selection behavior, including the `xyz` case as far as the old control allows.
- `step2-zip.png` / `step2-zip.mp4` — new tax profile ZIP using the legacy trigger+popover control; digit search, city-name search for `makati`, and row selection.
- `step3-form-rdo.png` / `step3-form-rdo.mp4` — 2551Q profile-backed RDO picker using the legacy trigger+popover control; row selection and Tab away.
- `step4-profile-dates.png` / `step4-profile-dates.mp4` — Individual profile birth date and Registered activity business start date as plain `YYYY-MM-DD` text boxes, including save/reopen behavior.

## after/

- `step1-rdo.png` / `step1-rdo.mp4` — new tax profile RDO combobox: typing `05` and `makati`, Down/Up highlight movement, Enter selection, chevron reopen, Escape close, and `xyz`+Tab reverting with `“xyz” is not in the list. The box kept 047 — East Makati.`
- `step2-zip.png` / `step2-zip.mp4` — new tax profile ZIP combobox: typing `12`, keyboard selection, and typing `makati` showing Makati ZIP rows.
- `step3-form-rdo.png` / `step3-form-rdo.mp4` — 2551Q profile-backed RDO combobox: typing, selection, Tab away, and reopened full RDO catalog starting at `001 — Laoag City`.
- `results.md` — concise AFTER pass/fail notes written during capture.
- `results.json` — machine-readable AFTER step status.

AFTER step 4 (desktop profile dates) was omitted because the follow-up instruction said it was unaffected on desktop and to rerun it only if time allows; steps 1–3 were prioritized after the final-head correction.

No focus loss after a pick, misplaced list, dead keys, overlapping note, or recycled-row wrong check was observed in the captured AFTER flows.
