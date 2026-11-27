# PR #425 fef911cf Windows real-mouse click proof

Commit under test: `fef911cf8baf979e5cb399cf0bf93a4593e214e5` (`feat/searchpick-followups`)
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (verified in the isolated checkout)
Build: fresh isolated `target-fef911cf`, fresh scratch database, window `900x700`.

Result: `#425 Windows at fef911cf: row 5/5, chevron 3/3, click-away 3/3 note Y, Alt+Tab P, ZIP P`.

## Files

- `pr425-fef911cf-clicks.mp4` — one unedited video of the whole job: five row clicks, three chevron close/open cycles, three click-away checks, Alt+Tab away/back, and the ZIP regression.
- `click-away-note.png` — click-away restored `047 — East Makati` with the note visible.
- `alt-tab-back-note.png` — after Alt+Tab back, the list was closed, the box showed `047 — East Makati`, and the note was visible.
- `chevron-reopen.png` — chevron reopened the full list after closing it.
- `zip-click-pass.png` — ZIP row click picked `1012 — Tondo South`.
- `results.md` — detailed run-by-run observations.
- `results.json` — machine-readable results.

## Summary

- Row pick: 5/5. Clicked rows `047`, `048`, `049`, `050`, then `047` again; every click committed the row with no note. The filtered `makati` list has only four RDO rows, so five mutually distinct rows were impossible.
- Chevron: 3/3. First click closed the list; second click reopened the full list.
- Click-away: 3/3, note Y. Every run closed the list, restored `047 — East Makati`, and left `“makati” fits 4 choices. Pick one from the list. The box kept 047 — East Makati.` visible.
- Alt+Tab: PASS. After switching to Docker Desktop and back, the list was closed, the box showed `047 — East Makati`, and the note was visible.
- ZIP regression: PASS. Typing `12` and clicking a row picked `1012 — Tondo South` with no note.

No defects were observed in the required flows. One extra chevron cycle was performed because one batched attempt lacked a separately inspected intermediate screenshot; the counted three cycles were visually verified. The video was copied unchanged; no manual trimming or editing was performed.
