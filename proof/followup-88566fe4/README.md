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
