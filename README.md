driver: gpui-agent-cli 0.4.1
recipes: penalty-mark.json (phone: pen-setup, pen-firstpage, pen-untouched, pen-surcharge); penalty-mark-desktop.json (desktop: pen-desktop-setup, pen-desktop-answer, pen-desktop-reopen, pen-desktop-shot)

# PR #386 penalty computed mark — recipe-driven proof pictures

Every picture on this branch was driven ONLY by gpui-agent-cli 0.4.1 recipes
(no real mouse or keys, no CGEvent, no simctl taps, no osascript). Screen
recordings are the Simulator's own `simctl io recordVideo` (kept per recipe by
the e2e runner's `--record`). The recipe files are under `agent-recipes/`;
each run's `report.json`/`report.md`, per-step screenshots, receipts and
recordings are under `evidence/<run>/`.

## Commits

| picture set | commit | evidence |
|---|---|---|
| phone BEFORE | 287c3095e7f841abfa8c5d5eee18cbc67b8e9fae | evidence/phone-before/ |
| phone AFTER | 0947bc9b077b6a4dc5a1edd4aec28fc350baace3 | evidence/phone-after/ |
| desktop BEFORE | 287c3095e7f841abfa8c5d5eee18cbc67b8e9fae | evidence/desktop-before/ |
| desktop AFTER | 0947bc9b077b6a4dc5a1edd4aec28fc350baace3 | evidence/desktop-after/ |

(report.json in each evidence folder carries the run's own `app_commit`.)

## What each picture shows

- phone-before/02-pen-firstpage: an ordinary form row of 1701's first page at 287c3095.
- phone-before/03-pen-untouched: Items 27–29 untouched (006-upper/008-mid/010-lower) at 287c3095 — the mark sits OUTSIDE the box in a wrapper, column A widened.
- phone-before/04-pen-surcharge: after typing only the surcharge at 287c3095.
- phone-after/*: the same at 0947bc9b077b6a4dc5a1edd4aec28fc350baace3 — the mark sits INSIDE the box's trailing slot, columns unchanged.
- desktop-before/04-pen-desktop-shot/009-align-shot.png: joint 1701 at 287c3095 — penalty rows' A column sits right of the rows above; the mark is outside the box.
- desktop-after/04-pen-desktop-shot/009-align-shot.png: joint 1701 at 0947bc9b077b6a4dc5a1edd4aec28fc350baace3 — Items 24–29 aligned, nonzero computed interest (3,436.44) showing WHOLE beside its mark inside the box; the amount's digits are not covered.

## Known gaps (no OS-input fallback used anywhere)

- hexuria/nsh-gpui-agent#15 (money box typing on macOS/iOS): the surcharge
  typing step runs unpaced; keystrokes in the money box can land reversed
  (the typed shot reads "0001" for 1000). The step is marked here as the
  deviation; the mark lifting it demonstrates is unaffected.
