PR425 macOS click proof at app `c42626973461a21a2e8ab58caca2405ff0891252`, production `App.tsx`, scratch `mac.db`, real mouse/keyboard input, initial window wider than 600pt.

- RDO row click: PASS 5/5. Each run typed `makati`, clicked `050 — South Makati`, the box showed the row code/name, and no “not in the list”/not-taken note appeared.
- RDO chevron: PASS 3/3. Each run typed `makati`, clicked the chevron and the list closed; clicked it again and the list reopened.
- RDO click away: PASS 3/3. Each run typed `makati`, clicked into Phone, the list closed, the RDO box returned to `050 — South Makati`, and the note under it said the typed text fits 4 choices / was not taken.
- Narrow linked date sheet: PASS. With the window under 600pt, the 2551Q “Set business start date” link opened Edit profile → Registered activity and the date sheet auto-opened once. After closing it, widening past 600pt and narrowing again remounted the edit sheet but the date sheet did not reopen by itself.

Main video: `pr425-mac-click-proof.mp4`.
