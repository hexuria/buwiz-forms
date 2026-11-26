# PR #406 confirming Windows owner-case proof

Commit: `b4b0fc1156842e69041e721f6990288db0c0b1bf`
NSH pin: `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`
Build: fresh isolated `target-b4b0fc11`, fresh scratch database, window `900x700`.

## confirm-b4b0fc11/

- `rdo.png` / `rdo.mp4` — new tax profile RDO combobox: click into the field, type `0`, scroll the filtered list to its end, pick with Enter, reopen, scroll the full list to its end, and click the final row (`93B — Zamboanga Sibugay`).
- `zip.png` / `zip.mp4` — new tax profile ZIP combobox: click into the field, type `1`, scroll the filtered list to its end, pick with Enter, reopen, scroll the full catalog to its end, and click the final row (`0560 — Far Eastern Broadcasting`). The mp4 is dead-time-trimmed because physical wheel scrolling through the long catalog took minutes; interaction frames are genuine and unaltered.
- `results.md` — detailed confirming-run notes.
- `results.json` — machine-readable confirming-run result.

Result: `b4b0fc11 owner case Windows: RDO P, ZIP P`.

No focus loss after picking, misplaced list, dead keys, note overlap, or recycled-row wrong-check behavior was observed.
