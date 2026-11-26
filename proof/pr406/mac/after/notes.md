macOS desktop at a9c9fd7c — PASS. Production `App.tsx`, fresh scratch `mac.db`, safety env, real mouse/keyboard. Window requested 1000x800 and was wider than 600pt.

- 10.2 PASS RDO typed `05`, Down moved highlight, Up returned, Enter picked `005 — Alaminos City`.
- 13.0 PASS chevron reopened full list with current `005` checked/in view.
- 17.9 PASS Escape closed; typing `xyz` then Tab reverted to `005 — Alaminos City` with sentence `"xyz" is not in the list. The box kept 005 — Alaminos City.` (`rdo-xyz-revert.png`).
- 21.4 PASS ZIP typing `12` opened filtered combobox list.
- 75.7 PASS real scrolling reached end; final row `9712 — Taraka` visible (`zip-end.png`).
- 78.5 PASS picked final row; ZIP box showed `9712 — Taraka` (`zip-picked.png`).
