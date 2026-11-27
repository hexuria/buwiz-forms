PR425 macOS re-proof at app `fef911cf8baf979e5cb399cf0bf93a4593e214e5`, production `App.tsx`, scratch `mac.db`, real mouse/keyboard input, initial window wider than 600pt.

- RDO row click: PASS 5/5. Each run typed `makati`, clicked `050 — South Makati`; the box showed code+name with no not-in-list/not-taken note.
- RDO chevron: PASS 3/3. Each run typed `makati`; chevron click closed the list and second chevron click reopened it.
- RDO click away: PASS 3/3, note YES 3/3. Clicking Phone closed the list, reverted to `050 — South Makati`, and showed the not-taken note.
- Cmd+Tab: PASS. With `makati` typed and the list open, Cmd+Tab away closed the list and settled the box back to `050 — South Makati` with the note; returning preserved that state.
- Narrow linked date sheet: PASS. Under 600pt the 2551Q “Set business start date” link opened Edit profile → Registered activity and the date sheet once. Close → widen >600 → narrow again remounted the edit sheet but did not reopen the date sheet.
