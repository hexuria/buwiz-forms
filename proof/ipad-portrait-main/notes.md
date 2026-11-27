iPad Pro 13-inch (M5), production `App.tsx` at main `4512a45a2fabf6ed98c24602b6fd3ee4b3194a74`, scratch `ipad.db`, real Simulator taps only. Connect Hardware Keyboard was off (unchecked in Simulator; the on-screen keyboard appeared). The device started erased/fresh.

## ZIP `makati` exact-field reproduction

- PORTRAIT, numeric first: opened ZIP, typed `12`, picked `1200 — Makati CPO + Buendia Ave`; reopened ZIP, switched to letters, typed `makati`. Zoomed field text is exactly lowercase `makati` — not `12makati`, not `Makati`, no leading/trailing space or punctuation/autocorrect. Rows appeared: `1200 — Makati CPO + Buendia Ave` and `1224 — Makati Commercial Ctr.`
- PORTRAIT, fresh letters-first: fresh new taxpayer ZIP, switched to letters first, typed `makati`. Zoomed field text is exactly lowercase `makati`. Same rows appeared.
- LANDSCAPE, numeric first: opened ZIP, typed `12`, picked `1012 — Pilar`; reopened ZIP, switched to letters, typed `makati`. Zoomed field text is exactly lowercase `makati`. Same rows appeared.
- LANDSCAPE, fresh letters-first: fresh new taxpayer ZIP, switched to letters first, typed `makati`. Zoomed field text is exactly lowercase `makati`. Same rows appeared.

Result: the `makati` matching failure was NOT reproduced in any of the four cases; rows appeared in all four.

## RDO opening clipping, portrait

Five openings, no typing. In every opening the search field was already visible at about 0.5s, remained visible at about 1s and 3s, and did not need typing to settle. Clipping count: 0/5. Persistent off-screen search count: 0/5. Result: opening clipping was NOT reproduced.

Main video: `rdo-five-openings.mp4`.
