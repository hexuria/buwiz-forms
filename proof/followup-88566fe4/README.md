# follow-up 88566fe4 real-key proof (win-3)

Build: hexuria/buwiz-forms fix/money-box-follow-up @ 88566fe43c29811343eb73f4b642da0d1c4e6a44 (rev-parse confirmed), own checkout `C:\Users\Administrator\370follow\buwiz-forms`, binary from `C:\Users\Administrator\370follow\buwiz-forms\target\nsh-store` (CARGO_TARGET_DIR explicit inside the checkout), kit sibling pin 8c78cfd6d, toolchain cargo 1.99.0, RUSTC_WRAPPER empty. Real window (pid-verified single window, no stacking), OS-level keys via python ctypes keybd_event/mouse_event. 2550M Oct 2026, demo taxpayer, invented values; db values read from form_drafts after each run. Per-run reset: clear 13A (ctrl+a, Backspace, Tab — the recompute override resets, verified: retype recomputes 13B=120.00), then type 1000 fresh, Tab into the box the form just recomputed.

## 1. recompute replace — 20/20 RIGHT

| timing | runs | 13B saved |
|---|---|---|
| first key ~0 ms after Tab | 10/10 | 999.0 every run |
| first key 120 ms after Tab | 10/10 | 999.0 every run |

The first key replaced the recomputed 120.00 in all twenty runs.

## 2. Z3 Right-arrow guard — 10/10 FAULT

Tab into recomputed 13B (120.00, fully selected — fu-right-before.png), press Right within ~100 ms, then 9. Screenshots confirm the Right keypress reaches the box and collapses the caret (fu-right-after.png: no selection). Expected: the 9 appends ("120.009" → 120.01). Actual: **all ten runs saved 9.0** — the reselect fired despite the arrow keypress and the digit replaced the whole amount. The guard does not count the arrow key as a key press.

## 3. focused comma text, app quit, reopen — RIGHT

`1500,50` typed into 13A, window closed (WM_CLOSE, the app's own quit) without leaving the box. Draft after close: sales_to_government 1500.5. Reopened return: 13A shows 1,500.50 (fu-reopened.png), 13B recomputed 180.06.

## 4. grey comma note — RIGHT

`1500,50` typed into the zero box item 14, Tab away: grey note under the box verbatim "Item 14: 1500,50 was read as 1,500.50." (fu-c4-note.png), box shows 1,500.50.

## notes

- fu-part2b.png: Part II layout, boxes 13A/13B.
- Harness: per-run timing by keybd_event bursts; db read ~0.9 s after leaving.

## case-2 three-way re-run (same build, same rig, 2026-11-27 ~17:15 local)

Ten runs each, per-run reset as before. Mid-type screenshots taken while the box still had focus; saved values read from form_drafts after Tab out.

### (a) Right pressed ~100 ms after the Tab, then 9

| run | 13B saved | box mid-type |
|---|---|---|
| a0 | 9.0 | `9` |
| a1 | 9.0 | — |
| a2 | 9.0 | — |
| a3 | 9.0 | — |
| a4 | **120.01** | `120.009` selected (fu2-a-mid-appended.png) |
| a5 | 9.0 | — |
| a6 | 9.0 | — |
| a7 | **120.01** | — |
| a8 | 9.0 | — |
| a9 | 9.0 | — |

**8/10 replaced (9.0), 2/10 appended (120.01)** — mixed. The 100 ms arrow lands inside a race: when the reselect beats the arrow the digit replaces; when the arrow's collapse is taken as the platform collapse first (or the reselect does not fire), the digit appends. fu2-a-mid-9.png shows the replaced case (box `9`); fu2-a-mid-appended.png shows the appended case (`120.009` with the text selected — a late reselect after the key).

### (b) Right pressed ~300 ms after the Tab, then 9

**10/10 appended**: box shows `120.009` mid-type (fu2-b-mid.png), saved **120.01** every run. Exactly as expected — past the 150 ms window the reselect never fires, the arrow's collapse stands, the digit appends.

### (c) printable key first: Tab, then 9 then 9 within ~100 ms

**10/10 appended**: box shows `120.0099` mid-type (fu2-c-mid.png — note the first characters `120.00` shown selected under the appended digits in some shots), saved **120.01** every run (120.0099 rounds half-up to 120.01). The expected 99 (first 9 replaces, second appends) did NOT occur in any run: the first printable key at ~80 ms after the Tab did not replace the recomputed amount. Consistent with the platform collapse landing before ~80 ms while the reselect write had not yet been applied — the keys fell in the dead window between the platform's collapse and the guard's re-selection, so both digits appended at the caret. (In the earlier T1 runs at 120 ms the first key DID replace 10/10 — at 120 ms the reselect has landed; at ~80 ms it has not.)

Screenshots: fu2-a-mid-9.png, fu2-a-mid-appended.png, fu2-b-mid.png, fu2-c-mid.png. Harness: 370fu-run3.py (same keybd_event/mouse_event method as before, timings 100/300/80 ms).
