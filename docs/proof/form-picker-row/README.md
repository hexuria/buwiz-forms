# Form-picker row-select proof

Live Linux NSH evidence for the form-picker row-select fix. `before/` is `origin/main` commit `6bb90ad1316e0b2b6dc3c2440af607623dd60e5d`; `after/` is branch head `72b34f5d068aa86a1524bf4c50a6bbbafd7785c1`. Both runs used the demo taxpayer Juana Santos, test/dry-run environment variables, and scratch databases. The scratch databases were seeded with the two demo 2026 year-form rows needed for the picker (2551Q and 1601C). No return was queued.

The row used in every pictured tap sequence is **2551Q — Quarterly Percentage Tax Return**. The picker is in edit mode and the search box is filled with `2551Q` so the same row is visible at every width. This means the pictures prove the row-hit behavior in a filtered picker list; they do not prove scrolling of the full unfiltered list.

## Screenshot names

In each of `before/` and `after/`:

- `1440x900-untapped.png` — desktop-width picker before a row-text tap.
- `1440x900-first-tap.png` — after one physical click on the row's text.
- `1440x900-second-tap.png` — after a second physical click on the same text.
- `834x1112-*.png` — the same three states at tablet width.
- `390x844-*.png` — the same three states at the requested phone size. The Linux host enforces its minimum width, so the actual logical width is **420 px**, not 390 px; height is 844 px.

At `main`, all three before images in each width remain at `2 selected`: text taps did not toggle. At the fixed head, the first text tap moves the count to `1 selected` and the checkbox is visibly empty; the second text tap returns to `2 selected` with the checkbox visibly filled. Rows otherwise keep the same content/layout structure.

`before/results.json` is the machine-readable desktop click matrix and keyboard probe at `main`. `after/results.json` and `after/phone-results.json` summarize the fixed-head count transitions.

## Main click matrix result

At `main`, filtered to 2551Q in edit mode at desktop width:

- row text/name: no change (`2 selected`, checkbox stayed checked)
- form code: no change
- empty part of the row: no change
- checkbox itself: toggled once (`2 selected` -> `1 selected`, checkbox became unchecked)

Physical keyboard delivery on this Linux/GPUI window is unreliable. Eighteen physical Tab presses produced no focused node anywhere in the semantic tree, so I could not prove whether a row is keyboard-reachable. Following Space and Enter did not change the count or checkbox. Treat the keyboard result as a Linux input-delivery limitation, not as app proof.

## Recordings

- `after/desktop-flow.mp4` — fixed-head desktop-width flow, 11 seconds.
- `after/phone-flow.mp4` — fixed-head phone-width flow, 7 seconds.

Both are assembled from `gpui-agent` app-surface still frames at **1 fps**. X11 desktop capture cannot see the GPUI window on this machine, so these are app-only low-frame-rate evidence videos, not continuous desktop recordings; sub-second transitions can be absent. The phone recording starts on Juana's Forms tab rather than showing a complete home-to-taxpayer navigation path; it does show entering edit mode, filtering to 2551Q, the two row-text taps, and save.

## What this does not show

- It does not test Android or Windows input directly; all evidence is from the Linux NSH desktop host.
- It does not prove keyboard accessibility because physical Tab delivery never produced focus in this Linux window.
- It does not prove behavior of every form row, only the documented 2551Q row.
- It does not prove the unfiltered long list or scrolling; search filtering was used to keep the same row visible.
- The requested 390 px phone width is approximated by the host-enforced 420 px minimum.
