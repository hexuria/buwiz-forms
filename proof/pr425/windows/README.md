# PR #425 Windows real-mouse click proof

Commit under test: `c42626973461a21a2e8ab58caca2405ff0891252` (`feat/searchpick-followups`)
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf` (verified in the isolated checkout)
Build: fresh isolated `target-c4262697`, fresh scratch database, window `900x700`.

Result: `#425 Windows clicks: row pick 0/5, chevron 0/3, click-away 0/3, Alt+Tab: box still shows makati; filtered list still open`.

## Files

- `pr425-clicks.mp4` — one unedited video of the whole click-proof item: five row-click attempts, three chevron close/open attempts, three click-away attempts, and the Alt+Tab away/back observation.
- `row-click-failure-note.png` — still showing the failed row-click state and note `“makati” fits 4 choices. Pick one from the list. The box stays empty.`
- `click-away-no-note.png` — still showing click-away reverting to `047 — East Makati` without the required not-taken note.
- `alt-tab-back.png` — still after Alt+Tab back, with `makati` in the RDO box and the filtered list still open.
- `results.md` — detailed run-by-run observations.
- `results.json` — machine-readable counts and observations.

## Summary

- Row pick: 0/5. Physical clicks on rows did not commit the clicked row; the box became empty instead.
- Chevron: 0/3. The first physical chevron click did not close the filtered list; it restored/selected the prior pick text and opened the full list. The second click left the list open.
- Click-away: 0/3 strict passes. All three closed the list and restored `047 — East Makati`, but the required not-taken note never appeared.
- Alt+Tab: after switching away and back, the RDO box still showed `makati` and the filtered list remained open.

Keyboard Enter was used once only as clearly marked setup to establish `047 — East Makati` before chevron/click-away tests, because physical row clicks never established a valid selection.
