# macOS proof: Filings status tabs follow the statuses filter (PR #431)

App commit `60a11787861650db5e4558a65d117eadf2222d4e`, nsh sibling `8c78cfd6d8ffcef260728ad2503d43c3bc9e86bf`,
CLI `nsh 0.6.1 (8c78cfd)`, cargo 1.99.0 (pinned rustup toolchain). Verification only; no product code changed.

## Files

| File | What it shows | What it does not |
|---|---|---|
| `filings-unticking.mp4` (61.75 s) | Screen recording (`screencapture -V 120 -k`, converted with ffmpeg) of a real macOS window at 390x844 running the untick/retick flow end to end: Draft tab selected, Filters sheet opened, statuses list, Draft unticked, Draft tab gone with the list falling back to All (5 remaining returns), reticked, Draft tab back. | It is not the exact headless test file: see "method" below. The Filings window overlaps an unrelated iOS Simulator window another session had open on the display — it is a real macOS window (traffic-light buttons visible at its top-left), not content inside the simulator. The recording continues past the test's end. |
| `filings-unticking.mov` | The original `screencapture` output the mp4 was converted from. | Same as above. |
| `proof-a-draft-tab-selected.png` | State (a): before filtering. All five tabs visible — All 8, **Draft 3 selected** (bold + underline), Queued 1, Submitted 2, Confirmed 2 — listing the 3 draft returns. | In-app screenshot (`app.screenshot`), the window content only, not the screen. |
| `proof-b-draft-tab-gone.png` | State (b): after unticking Draft. The Draft tab is **gone** — only All 5 (selected), Queued 1, Submitted 2, Confirmed 2 — with "5 returns" of the remaining statuses, the Filters button's badge 1 and the "× Clear filters" chip. | Same. |
| `proof-c-draft-tab-back.png` | State (c): after reticking Draft. The Draft tab is **back** — All 8, Draft 3, Queued 1, Submitted 2, Confirmed 2 — and the drafts are in the list again. | Same. |

## Method (disclosed deviation)

The order's exact command (`just nsh-test screens/filings/Filings.test.tsx --filter "unticking a status" --target macos --display 1`)
**cannot run at kit pin 8c78cfd6, structurally, at any commit**: on a real target the test file executes inside the nsh CLI's own
process, which links only `nsh/base` — and `Filings.test.tsx` imports `bir` (`ebirforms/native`) at top level, so its bundle fails to
load with `could not load module 'ebirforms/native'` before the test starts. Verified: the same failure hits every other
bir-importing test file (e.g. `screens/calendar/Calendar.test.tsx`); files without a bir import (e.g. `shots/f18.test.tsx`) pass
4/4 in a real macOS window at the same commit; the native module itself works in the host.

So the recorded run uses a scratch, working-tree-only driver `nsh/shots/filings-unticking-proof.test.tsx` (never committed,
deleted before the full suite) following the existing real-window fixture pattern of `nsh/shots/filings.test.tsx`: no top-level
`bir` import, opening `../FilingsDemo.tsx` (which does its bir imports inside the host, where they are legal) at 390x844,
platform macos, light, `host: { params: { generated: true } }`. It drives the same selectors as the headless test —
Draft tab, `filings-filters-open`, `filings-statuses-trigger`, `filings-statuses-opt-Draft`, `filings-sheet-done` — and shoots
the three proof states. `generated: true` adds one extra generated Queued row, so totals are 8/5 rather than the headless
test's 7/4; the states proven (tab visibility and fallback) are the same. The test itself asserted Draft-gone and Draft-back
boolean visibility and passed (15.3 s).

Command run: `just nsh-test shots/filings-unticking-proof.test.tsx --target macos --display 1` with
`screencapture -V 120 -k` started just before it.

## Inspector's verdict

The stills genuinely show the three required states — Draft visible/selected before filtering, Draft gone with the list
falling back to All after unticking, Draft visible again after reticking — and the mp4 shows the same flow live, including
the Filters sheet's statuses list.
