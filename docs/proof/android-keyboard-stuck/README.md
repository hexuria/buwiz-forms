# Job 15 — the stuck padding: minimal steps and cause

**App commit:** `fe1a5f46` (plus proof-docs commits on this branch — the app code is unchanged).
**NSH CLI:** `nsh 0.6.1 (8c78cfd)`, pin `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`.
**Android runtime classes.dex sha256:** `4c1f44452f0f26ec3d1aa55592de01063afab5506faef919e7122e902c1a58aa` — pin build plus six uncommitted `NSHKBD` log lines (never committed or pushed): the three from job 11 (`Mounter.inputFocused`, `Mounter.keyboard`, `ScrollBox.setContentPaddingBottom`) plus three new ones in `NshView.java`: (d) `insets ime=<ime> cached=<keyboard> pans=<systemPans()> h=<getHeight()>` logged on every `publishEnvironment` pass before the change check; (e) `windowFocus <hasFocus>` at the top of the existing `onWindowFocusChanged` override; (f) `attachedToWindow` / `detachedFromWindow` in the existing overrides. Verified in the packaged APK (`strings classes.dex | grep -c NSHKBD` = 1, sha re-matched after packaging).

Setup: signed in (mock account server), made-up taxpayer, year 2026, form 2551Q chosen; Choose forms open, search field focused, "2551q" typed, keyboard up, result row at y1720–1771 with the keyboard top at 1517. Phone API 35, gesture mode, portrait.

## Results per variant (one try each — honest count, not five)

| try | steps | stuck? | key lines |
|---|---|---|---|
| A | Home, open kit app, tap its field, Recents back | **no** — padding re-applied | leave: `windowFocus false` 13:54:31.018 → `insets ime=0.0 cached=883.0` → `setContentPaddingBottom scroll=1904 px=0`. return: `windowFocus true` 13:55:06.235 **then** `insets ime=883.0 cached=0.0` 06.382 → `keyboard screenTop=1517` → `px=668` |
| B | same but return by launcher icon | **no** — identical pattern | `windowFocus true` 13:57:14.082, `insets ime=883` 14.226, `px=668` |
| C | Recents, straight back, no other app | **no** — nothing fired at all | only `windowFocus false` then `windowFocus true` — the keyboard never hid, no insets pass, padding never lost |
| D | notification shade down and up | **no** — only focus events | `windowFocus false`/`true`; no insets passes |
| E | screen off/on (keyevent 26 twice), unlock swipe | **no** — ime never even cleared | `insets ime=883.0 cached=883.0` throughout; `windowFocus false`/`true`; the ime never counts as hidden across screen-off |

**The padding-loss stuck state did not reproduce in any of the five variants.** The invariant the logs establish: `windowFocus true` always comes BEFORE any re-fired insets pass, and whenever the keyboard genuinely hid and re-showed, the insets pass DOES fire after return and the padding is re-applied (`px=668`). Job 11's one observed stuck state (padding 0 with the keyboard up after keyboard-using-app switching) is therefore intermittent/rare or requires a trigger outside A–E — honest answer: not pinned down here.

## The more important correction this evidence forces

While capturing these variants a second fact became unavoidable: **with the padding correctly applied (px=668, after a healthy return), the 2551Q row still did not move under repeated swipes starting at y1300** — byte-identical y1720–1771 — in the SAME build where job 11 had once recorded it moving to y508. Then in variant D, a long shade-up swipe passing through the list area DID move the row (to y1052–1103). So the chooser's immovability is not (only) a padding-loss problem: the padding lands on a scroll that the results list either is not inside or cannot use (nested-scroll suspicion — the panel's outer scroll gets the padding, the results list does not scroll further for it), and whether a swipe moves anything depends on where it lands. This is exactly the gap the app's own spacer in #351 closes (job 12: same swipes move the row to y438 in AFTER), and it re-reads job 11's "intermittent stuck" as mostly this layout fault rather than padding loss. Both facts are reported as measured; the owner writing the fix should weight the per-variant logs (healthy event ordering everywhere) against the one unreproduced job-11 observation.

## Logs
`variant-A.logcat` … `variant-E.logcat` — the NSHKBD lines per try (variant A additionally includes the swipe-test rows appended at the end).
