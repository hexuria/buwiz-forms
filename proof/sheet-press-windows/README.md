driver: gpui-agent-cli 0.4.1

# Sheet press Windows proof

Recipes named by this proof: `agent-recipes/e2e/sheet-press.json` and `agent-recipes/e2e/delete-row.wants`, in the `agent-recipes/e2e/smoke.json` suite. Supporting recipe updates are in `open-2551q-windows.wants`, `money-boxes-paced.json`, and `queue-windows.wants` so the filled 2551Q remains valid for Queue.

App commit: `43defa623a38122aaeca68a8fea4c988ed0ddfe5`. The app binary was built on Windows with only the local, uncommitted `nsh-gpui-agent-windows` dependency redirected to plug branch `fix/sheet-node-geometry` (`247fdd87c3ef3682db73daff486894bec685fb0c`; the later plug commit `9232764b3bd7c58cb3f2de0d887064f37f1db280` adds only a test).

- `before/` is the reproduction with the app-pinned old plug: the Schedule 1 drawer host is `730x0`; `fp-txtATCAmt1` has painted bounds `411x30` but `visible=false`, and pressing it is refused as offscreen.
- `after-targeted/` is the first targeted fixed-plug run: `sheet-press` and `delete-row` pass.
- `report.json`, `report.md`, `run.log`, and the numbered recipe folders are the final full Windows smoke run with the fixed plug.

Final full-suite result: 10 pass, 0 fail, 1 known gap (`sign-out`). `sheet-press` types `100000` in the Schedule 1 drawer and presses Done; `delete-row` types `1000` in row 2 and presses Delete row. The larger first-row amount keeps the return positive after the money-box recipe's credits so Queue can validate; `money-boxes` clears Item 16 after proving it because the return ends as not amended; the income election is graduated because an 8% election forbids a PT010 row.
